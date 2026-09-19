// Command scraper keeps a catalog of BTU Cottbus-Senftenberg modules, programs,
// study plans and events up to date and publishes it as SQLite snapshots.
// See docs/operations.md.
package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/leonieziechmann/btu-scraper/internal/catalogdb"
)

const defaultDBPath = "btu_scraper.db"

type command struct {
	name    string
	summary string
	run     func(ctx context.Context, args []string)
}

// Grouped as the usage text shows them.
var commands = [][]command{
	{
		{"run", "Service: crawl politely, build, validate, export, serve /snapshot, /healthz, /status [--once]", runService},
		{"healthcheck", "Exit 0 if a running service reports healthy (for container health checks)", runHealthcheck},
	},
	{
		{"crawl-modules", "Archive the module catalog list, the FÜS list and all module pages", runCrawlModules},
		{"crawl-tree", "Walk the QIS program tree; fetch what is missing or stale", runCrawlTree},
		{"crawl-events", "Archive the QIS event pages that module pages link", runCrawlEvents},
		{"prune-events", "Remove events one month after their last date", runPruneEvents},
		{"build", "Derive the canonical tables from the raw page archive (no network)", runBuild},
		{"validate", "Check invariants, source conflicts and count baselines; exit 1 on failures", runValidate},
		{"export", "Write the read-optimized snapshot", runExport},
		{"serve-snapshot", "Publish exported snapshots over HTTP without running the service", runServeSnapshot},
	},
	{
		{"download-statutes", "Download the regulation PDFs of the programs (OPUS)", runDownloadStatutes},
		{"scan-curriculum", "Extract validated study plans from the regulation PDFs (Gemini enrichment optional)", runScanCurriculum},
		{"secret", "Manage credentials in the OS credential store: set | status | delete | migrate-config", runSecret},
	},
	{
		{"raw-vocab", "Print the distinct raw values of the normalized module fields", runRawVocab},
		{"import-legacy-plans", "Copy the validated study plans of a schema v1 database (one-time)", runImportLegacyPlans},
	},
}

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(2)
	}
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	name := os.Args[1]
	if name == "help" || name == "--help" || name == "-h" {
		printUsage()
		return
	}
	for _, group := range commands {
		for _, c := range group {
			if c.name == name {
				c.run(ctx, os.Args[2:])
				return
			}
		}
	}
	fmt.Fprintf(os.Stderr, "Unknown command: %s\n\n", name)
	printUsage()
	os.Exit(2)
}

func printUsage() {
	fmt.Println("BTU catalog scraper\n\nUsage:\n  scraper <command> [flags]      (scraper <command> --help lists the flags)\n\nCommands:")
	for _, group := range commands {
		for _, c := range group {
			fmt.Printf("  %-20s %s\n", c.name, c.summary)
		}
		fmt.Println()
	}
	fmt.Println("Every command takes --db (default " + defaultDBPath + "), --log-format text|json, --log-level, --log-file.\n" +
		"Credentials are never read from flags or configuration files; see `scraper secret status`.")
}

func openDB(path string) *catalogdb.DB {
	db, err := catalogdb.Open(path)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error opening database (%s): %v\n", path, err)
		os.Exit(1)
	}
	return db
}
