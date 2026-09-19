package main

import (
	"bytes"
	"context"
	"crypto/sha1"
	"database/sql"
	"encoding/hex"
	"errors"
	"flag"
	"fmt"
	"os"
	"sort"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/cache"
	"github.com/leonieziechmann/btu-scraper/internal/catalogdb"
	"github.com/leonieziechmann/btu-scraper/internal/crawl"
	"github.com/leonieziechmann/btu-scraper/internal/parser"
	"github.com/leonieziechmann/btu-scraper/internal/provider"
	"github.com/leonieziechmann/btu-scraper/internal/qistree"
)

// Commands of the schema v2 pipeline: crawl (network → raw archive), then
// build / validate / export, which never touch the network.

const defaultV2DBPath = "btu_v2.db"

func openV2(path string) *catalogdb.DB {
	db, err := catalogdb.Open(path)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error opening schema v2 database (%s): %v\n", path, err)
		os.Exit(1)
	}
	return db
}

// runCrawlModules archives the module catalog list and every module page.
func runCrawlModules(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("crawl-modules", flag.ExitOnError)
	dbPath := fs.String("db", defaultV2DBPath, "Schema v2 database path")
	workers := fs.Int("workers", 4, "Parallel fetchers")
	delayMs := fs.Int("delay", 500, "Pause per worker after each request in milliseconds (±30% jitter)")
	maxAge := fs.Duration("max-age", 24*time.Hour, "Skip module pages archived more recently than this (0 refetches everything)")
	limit := fs.Int("limit", 0, "Limit number of module pages (0 for all)")
	_ = fs.Parse(reorderFlags(args))

	db := openV2(*dbPath)
	defer db.Close()

	opt := crawl.Options{
		Workers: *workers,
		Delay:   time.Duration(*delayMs) * time.Millisecond,
		MaxAge:  *maxAge,
	}

	fmt.Println("[1/2] Archiving module catalog list...")
	listJob := crawl.Job{Source: catalogdb.SourceModuleCatalog, Key: "list", URL: provider.DefaultCatalogURL}
	listOpt := opt
	listOpt.MaxAge = time.Hour
	if _, err := crawl.Run(ctx, db, []crawl.Job{listJob}, listOpt); err != nil {
		fmt.Fprintf(os.Stderr, "Error fetching catalog list: %v\n", err)
		os.Exit(1)
	}

	ids, err := archivedModuleIDs(db)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error reading module IDs: %v\n", err)
		os.Exit(1)
	}
	if *limit > 0 && len(ids) > *limit {
		ids = ids[:*limit]
	}

	jobs := make([]crawl.Job, 0, len(ids))
	for _, id := range ids {
		jobs = append(jobs, crawl.Job{
			Source: catalogdb.SourceModulePage,
			Key:    id,
			URL:    fmt.Sprintf(provider.DefaultDetailURLTmpl, id),
		})
	}

	fmt.Printf("[2/2] Archiving %d module pages (%d workers, %d ms delay, skip if newer than %s)...\n",
		len(jobs), opt.Workers, *delayMs, *maxAge)
	start := time.Now()
	opt.Progress = func(done, total int, s crawl.Stats) {
		if done%100 == 0 || done == total {
			fmt.Printf("[Progress] %d/%d  fetched=%d skipped=%d not-found=%d failed=%d  (%s)\n",
				done, total, s.Fetched, s.Skipped, s.NotFound, s.Failed, time.Since(start).Round(time.Second))
		}
	}
	stats, err := crawl.Run(ctx, db, jobs, opt)
	fmt.Printf("\n[✓] fetched=%d skipped=%d not-found=%d failed=%d in %s\n",
		stats.Fetched, stats.Skipped, stats.NotFound, stats.Failed, time.Since(start).Round(time.Second))
	if err != nil {
		if errors.Is(err, context.Canceled) {
			fmt.Fprintln(os.Stderr, "Interrupted. Run the command again to resume; archived pages are skipped.")
		} else {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		}
		os.Exit(1)
	}
	if stats.Failed > 0 {
		os.Exit(1)
	}
}

// archivedModuleIDs is the union of the module IDs named by the archived
// catalog list and the archived FÜS list. A few FÜS modules are not in the catalog.
func archivedModuleIDs(db *catalogdb.DB) ([]string, error) {
	seen := make(map[string]bool)

	list, err := db.GetPage(catalogdb.SourceModuleCatalog, "list")
	if err != nil {
		return nil, fmt.Errorf("catalog list is not archived: %w", err)
	}
	summaries, err := parser.NewCatalogParser("").Parse(bytes.NewReader(list.Body))
	if err != nil {
		return nil, err
	}
	for _, s := range summaries {
		seen[s.ID] = true
	}
	if len(seen) == 0 {
		return nil, fmt.Errorf("catalog list contains no module IDs")
	}

	if fues, err := db.GetPage(catalogdb.SourceQISFUESList, "list"); err == nil {
		modules, err := parser.NewFUESParser().Parse(bytes.NewReader(fues.Body))
		if err != nil {
			return nil, err
		}
		for _, m := range modules {
			seen[m.ID] = true
		}
	} else if !errors.Is(err, catalogdb.ErrNotFound) {
		return nil, err
	}

	ids := make([]string, 0, len(seen))
	for id := range seen {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	return ids, nil
}

// runImportCache copies still-valid pages from the legacy disk cache into the raw
// page archive, so they do not have to be requested again.
func runImportCache(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("import-cache", flag.ExitOnError)
	dbPath := fs.String("db", defaultV2DBPath, "Schema v2 database path")
	cacheDir := fs.String("cache-dir", ".cache", "Legacy cache directory")
	legacyDB := fs.String("legacy-db", "btu_modules.db", "Legacy database that names the event IDs to look up (optional)")
	_ = fs.Parse(reorderFlags(args))

	db := openV2(*dbPath)
	defer db.Close()

	c, err := cache.NewDiskCache(*cacheDir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error opening cache: %v\n", err)
		os.Exit(1)
	}

	imported := make(map[string]int)
	importKey := func(cacheKey, source, key, pageURL string) {
		body, ok, err := c.Get(cacheKey)
		if err != nil || !ok {
			return
		}
		storedAt, _ := c.StoredAt(cacheKey)
		storedAt = storedAt.Truncate(time.Second) // the archive keeps seconds
		if existing, _ := db.PageFetchedAt(source, key); !existing.Before(storedAt) {
			return
		}
		if err := db.PutPage(catalogdb.RawPage{Source: source, Key: key, URL: pageURL, FetchedAt: storedAt, HTTPStatus: 200, Body: body}); err != nil {
			fmt.Fprintf(os.Stderr, "Error archiving %s: %v\n", cacheKey, err)
			os.Exit(1)
		}
		imported[source]++
	}

	importKey("fues:catalog", catalogdb.SourceQISFUESList, "list", provider.DefaultFUESURL)
	importKey("catalog:list", catalogdb.SourceModuleCatalog, "list", provider.DefaultCatalogURL)

	if ids, err := archivedModuleIDs(db); err == nil {
		for _, id := range ids {
			importKey("module:"+id, catalogdb.SourceModulePage, id, fmt.Sprintf(provider.DefaultDetailURLTmpl, id))
		}
	}

	if eventIDs, err := legacyColumn(*legacyDB, "SELECT id FROM events ORDER BY id"); err == nil {
		for _, id := range eventIDs {
			importKey("event:"+id, catalogdb.SourceQISEvent, id, fmt.Sprintf(provider.DefaultQISBaseURL, id))
		}
	}

	// QIS tree pages are cached under a hash of their URL, so they can only be
	// found by walking the tree from the root.
	fetchCached := func(_ context.Context, pageURL string) ([]byte, error) {
		h := sha1.Sum([]byte(pageURL))
		cacheKey := "qis:tree:" + hex.EncodeToString(h[:])
		body, ok, err := c.Get(cacheKey)
		if err != nil || !ok {
			return nil, qistree.ErrPageMissing
		}
		importKey(cacheKey, catalogdb.SourceQISTree, pageURL, pageURL)
		return body, nil
	}
	ignore := func(qistree.Page) error { return nil }

	treeResult, err := qistree.Walk(ctx, provider.DefaultProgramTreeURL, fetchCached, ignore)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error walking cached QIS tree: %v\n", err)
		os.Exit(1)
	}

	// A PO subtree is unreachable from the root once its program or degree page
	// expired. The legacy database still knows the PO URLs.
	if poURLs, err := legacyColumn(*legacyDB, "SELECT qis_url FROM official_study_programs WHERE qis_url <> '' ORDER BY id"); err == nil {
		for _, poURL := range poURLs {
			r, err := qistree.WalkPO(ctx, qistree.Page{URL: poURL}, fetchCached, ignore)
			if err != nil {
				fmt.Fprintf(os.Stderr, "Error walking cached PO subtree: %v\n", err)
				os.Exit(1)
			}
			treeResult.Pages += r.Pages
			treeResult.Missing = append(treeResult.Missing, r.Missing...)
		}
	}

	for _, source := range []string{catalogdb.SourceQISFUESList, catalogdb.SourceModuleCatalog, catalogdb.SourceModulePage, catalogdb.SourceQISEvent, catalogdb.SourceQISTree} {
		fmt.Printf("%-16s %d pages imported\n", source, imported[source])
	}
	fmt.Printf("QIS tree walk: %d pages reachable in cache, %d not cached\n", treeResult.Pages, len(treeResult.Missing))
	for i, u := range treeResult.Missing {
		if i == 5 {
			fmt.Printf("  ... and %d more\n", len(treeResult.Missing)-5)
			break
		}
		fmt.Printf("  missing: %s\n", u)
	}
}

// legacyColumn reads one text column from the legacy database, opened read-only.
func legacyColumn(path, query string) ([]string, error) {
	if _, err := os.Stat(path); err != nil {
		return nil, err
	}
	legacy, err := sql.Open("sqlite", path+"?mode=ro&_pragma=query_only(1)")
	if err != nil {
		return nil, err
	}
	defer legacy.Close()

	rows, err := legacy.Query(query)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var values []string
	for rows.Next() {
		var v string
		if err := rows.Scan(&v); err != nil {
			return nil, err
		}
		values = append(values, v)
	}
	return values, rows.Err()
}

// runRawVocab prints the distinct raw values of the module-page fields that get
// normalized, most frequent first. It is the evidence for the normalization rules.
func runRawVocab(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("raw-vocab", flag.ExitOnError)
	dbPath := fs.String("db", defaultV2DBPath, "Schema v2 database path")
	top := fs.Int("top", 25, "Values to print per field")
	_ = fs.Parse(reorderFlags(args))

	db := openV2(*dbPath)
	defer db.Close()

	fields := []string{"exam_type", "grading", "limitation", "degree", "regulation", "language", "duration", "turnus", "teaching_form"}
	counts := make(map[string]map[string]int)
	for _, f := range fields {
		counts[f] = make(map[string]int)
	}

	pages := 0
	p := parser.NewDetailParser()
	err := db.EachPage(catalogdb.SourceModulePage, func(page *catalogdb.RawPage) error {
		if page.HTTPStatus != 200 {
			return nil
		}
		d, err := p.Parse(bytes.NewReader(page.Body), page.Key, page.URL)
		if err != nil {
			return err
		}
		pages++
		counts["exam_type"][d.ExamType]++
		counts["grading"][d.Grading]++
		counts["limitation"][d.Limitation]++
		counts["language"][d.Language]++
		counts["duration"][d.Duration]++
		counts["turnus"][d.Turnus]++
		for _, sp := range d.StudyPrograms {
			counts["degree"][sp.Degree]++
			counts["regulation"][sp.Regulation]++
		}
		for _, tf := range d.TeachingForms {
			counts["teaching_form"][tf.Type]++
		}
		return nil
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("%d module pages parsed\n", pages)
	for _, f := range fields {
		type kv struct {
			value string
			n     int
		}
		var list []kv
		for v, n := range counts[f] {
			list = append(list, kv{v, n})
		}
		sort.Slice(list, func(i, j int) bool {
			if list[i].n != list[j].n {
				return list[i].n > list[j].n
			}
			return list[i].value < list[j].value
		})
		fmt.Printf("\n== %s (%d distinct)\n", f, len(list))
		for i, e := range list {
			if i == *top {
				break
			}
			fmt.Printf("%6d  %q\n", e.n, e.value)
		}
	}
}
