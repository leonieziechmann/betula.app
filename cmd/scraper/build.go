package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/btu-scraper/internal/catalogbuild"
	"github.com/leonieziechmann/btu-scraper/internal/catalogdb"
	"github.com/leonieziechmann/btu-scraper/internal/snapshothttp"
)

// runBuild derives the canonical tables from the raw page archive. No network.
func runBuild(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("build", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	_ = fs.Parse(args)

	db := openDB(*dbPath)
	defer db.Close()

	start := time.Now()
	report, err := catalogbuild.Build(ctx, db)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	printBuildReport(report, time.Since(start))
}

func printBuildReport(r *catalogbuild.Report, took time.Duration) {
	fmt.Printf("Build finished in %s\n\n", took.Round(time.Millisecond))
	fmt.Printf("Modules        %d (%d known from a list only, no module page)\n", r.Modules, r.ModulesWithoutPage)
	fmt.Printf("Departments    %d (%d English names without a German counterpart)\n", r.Departments, len(r.UnpairedEnglishDep))
	for _, name := range r.UnpairedEnglishDep {
		fmt.Printf("                 unpaired: %s\n", name)
	}
	fmt.Printf("Programs       %d (%d with a stated short degree label)\n", r.Programs, r.ProgramsWithLabel)
	fmt.Printf("Tree           %d areas, %d module leaves, %d area pages missing from the archive\n", r.Areas, r.TreeLeaves, r.MissingTreePages)
	printTop("                 leaves without catalog module", r.TreeLeavesNoModule, 8)
	fmt.Printf("Page refs      %d (%d „Abschluss im Ausland\", %d unresolved)\n", r.PageRefs, r.PageRefsAbroad, sum(r.PageRefsUnresolved))
	printTop("                 unresolved", r.PageRefsUnresolved, 12)
	fmt.Printf("Assertions     module_page=%d qis_tree=%d pdf_plan=%d\n", r.Assertions["module_page"], r.Assertions["qis_tree"], r.Assertions["pdf_plan"])
	fmt.Printf("Plans          %d entries point to modules not in the catalog; %d plans without a program %v\n",
		r.PlanEntriesUnknownModule, len(r.PlansWithoutProgram), r.PlansWithoutProgram)
	fmt.Printf("Events         %d (%d event links on module pages are not archived yet)\n", r.Events, r.EventLinksNoArchive)
}

func sum(m map[string]int) int {
	total := 0
	for _, n := range m {
		total += n
	}
	return total
}

func printTop(title string, m map[string]int, limit int) {
	if len(m) == 0 {
		return
	}
	type kv struct {
		key string
		n   int
	}
	list := make([]kv, 0, len(m))
	for k, n := range m {
		list = append(list, kv{k, n})
	}
	sort.Slice(list, func(i, j int) bool {
		if list[i].n != list[j].n {
			return list[i].n > list[j].n
		}
		return list[i].key < list[j].key
	})
	fmt.Printf("%s (%d distinct):\n", title, len(list))
	for i, e := range list {
		if i == limit {
			fmt.Printf("                   … and %d more\n", len(list)-limit)
			break
		}
		fmt.Printf("                   %5d  %s\n", e.n, e.key)
	}
}

// runImportLegacyPlans copies the validated study plans of the schema v1 database.
func runImportLegacyPlans(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("import-legacy-plans", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	legacyDB := fs.String("legacy-db", "btu_modules.db", "Schema v1 database to read the validated plans from")
	_ = fs.Parse(args)

	if _, err := os.Stat(*legacyDB); err != nil {
		fmt.Fprintf(os.Stderr, "Error: legacy database %s: %v\n", *legacyDB, err)
		os.Exit(1)
	}
	db := openDB(*dbPath)
	defer db.Close()

	result, err := db.ImportLegacyPlans(*legacyDB)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Imported %d validated plans with %d entries and %d scan statuses.\n", result.Plans, result.Entries, result.ScanStatuses)
	if len(result.SkippedNoQISID) > 0 {
		fmt.Printf("Skipped %d legacy programs without a usable QIS URL: %v\n", len(result.SkippedNoQISID), result.SkippedNoQISID)
	}
	fmt.Println("Run 'scraper build' to derive the plan-based membership statements.")
}

// runValidate reports the invariants of a built database and fails on regressions.
func runValidate(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("validate", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	noBaselines := fs.Bool("no-baselines", false, "Skip the BTU count baselines (for partial or test databases)")
	_ = fs.Parse(args)

	db := openDB(*dbPath)
	defer db.Close()

	baselines := catalogdb.BTUBaselines
	if *noBaselines {
		baselines = nil
	}
	checks, err := db.Validate(ctx, baselines)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	for _, c := range checks {
		detail := ""
		if c.Detail != "" {
			detail = "  (" + c.Detail + ")"
		}
		fmt.Printf("%-5s %8d  %s%s\n", strings.ToUpper(c.Status), c.Value, c.Name, detail)
		for _, s := range c.Samples {
			fmt.Printf("                  · %s\n", s)
		}
	}
	if catalogdb.HasFailures(checks) {
		fmt.Fprintln(os.Stderr, "\nvalidate: FAILED")
		os.Exit(1)
	}
	fmt.Println("\nvalidate: no failures")
}

// runExport writes the snapshot that /api/db serves.
func runExport(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("export", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	outDir := fs.String("out", "snapshot", "Snapshot directory (catalog-<hash>.db and current.json)")
	skipValidate := fs.Bool("skip-validate", false, "Export even if validate reports failures")
	_ = fs.Parse(args)

	db := openDB(*dbPath)
	defer db.Close()

	if !*skipValidate {
		checks, err := db.Validate(ctx, nil)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		if catalogdb.HasFailures(checks) {
			fmt.Fprintln(os.Stderr, "Error: the database fails validation; run 'scraper validate' (or pass --skip-validate).")
			os.Exit(1)
		}
	}

	snap, err := db.Export(ctx, *outDir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Snapshot %s  %.1f MB  ETag %s\n", filepath.Join(*outDir, snap.File), float64(snap.Bytes)/1024/1024, snap.ETag)
}

// runServeSnapshot publishes the exported snapshots over HTTP. The web server is a
// client of this endpoint; it shares no files with the scraper.
func runServeSnapshot(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("serve-snapshot", flag.ExitOnError)
	dir := fs.String("dir", "snapshot", "Snapshot directory written by 'export'")
	addr := fs.String("addr", "127.0.0.1:8090", "Listen address")
	_ = fs.Parse(args)

	server := &http.Server{Addr: *addr, Handler: snapshothttp.Handler(*dir), ReadHeaderTimeout: 10 * time.Second}
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()

	fmt.Printf("Serving snapshots from %s on http://%s%s\n", *dir, *addr, snapshothttp.DatabasePath)
	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}
}
