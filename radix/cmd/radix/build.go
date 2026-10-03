package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogbuild"
	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/metrics"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
	"github.com/leonieziechmann/betula/radix/internal/service"
	"github.com/leonieziechmann/betula/radix/internal/snapshothttp"
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
	fmt.Printf("Events         %d (%d event links on module pages are not archived yet, %d name events QIS has removed)\n",
		r.Events, r.EventLinksNoArchive, len(r.EventLinksGone))
	for _, link := range r.EventLinksGone {
		fmt.Printf("                 left out: %s\n", link)
	}
	fmt.Printf("Short names    %d rooms without a known building, %d rooms kept their long form; abbreviations: %d pairs fell back, %d twins, %d changed since the last build\n",
		len(r.RoomsUnknownBuilding), len(r.RoomShortCollisions), r.AbbrevFellBack, r.AbbrevTwins, r.AbbrevChanged)
	for _, line := range r.AbbrevOverridesUnused {
		fmt.Printf("                 unused override %s\n", line)
	}
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
			fmt.Fprintln(os.Stderr, "Error: the database fails validation; 'radix validate' says why. A database a new release has migrated needs 'radix build' first (a migration adds columns, the build fills them). --skip-validate exports it anyway, with what fails.")
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
// client of this endpoint; it shares no files with Radix.
//
// With --db it also keeps the snapshot in step with this release: when the database was
// built by another binary, it builds it again from the archive, validates and exports,
// while the snapshot it has is served on (service.Rebuild). Nothing is fetched. A build or
// validation that fails logs an ERROR and leaves the snapshot as it was.
func runServeSnapshot(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("serve-snapshot", flag.ExitOnError)
	dir := fs.String("dir", "snapshot", "Snapshot directory written by 'export'")
	addr := fs.String("addr", "127.0.0.1:8090", "Listen address")
	dbPath := fs.String("db", "", "Database to build a new snapshot from when another release built it, and to compute the semantic search's vectors in, with --embed-model (no network); empty: only serve")
	semanticOpts := addEncoderFlags(fs)
	logs := addLogFlags(fs)
	_ = fs.Parse(args)

	_, closeLog := logs.setup()
	defer closeLog()

	if *dbPath != "" {
		db := openDB(*dbPath)
		defer db.Close()
		cfg := service.DefaultConfig()
		cfg.SnapshotDir = *dir
		// Offline the catalog stays as it was exported: no event ages out, no page is removed.
		cfg.EventRetention, cfg.ArchiveGrace = 0, 0
		semantic, closeSemantic, err := semanticOpts.setup(ctx)
		if err != nil {
			slog.Error("cannot start the semantic search's encoder", "component", "cli", "event", "cli.failed", oplog.Err(err))
			os.Exit(2)
		}
		defer closeSemantic()
		cfg.Semantic = semantic
		svc := service.New(db, cfg, nil)
		offline := make(chan struct{})
		go func() { defer close(offline); svc.RunOffline(ctx) }()
		// Before the encoder and the database close: a stop cancels the build, which rolls back,
		// and the vectors, which are stored as they come.
		defer func() { <-offline }()
	}

	declareBuildInfo("serve-snapshot")
	mux := http.NewServeMux()
	mux.Handle("/snapshot/", snapshothttp.Handler(*dir))
	mux.Handle("GET /metrics", metrics.Default.Handler())
	server := &http.Server{Addr: *addr, Handler: mux, ReadHeaderTimeout: 10 * time.Second}
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
