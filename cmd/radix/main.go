// Command radix is the collector of Betula: it keeps a catalog of BTU Cottbus-Senftenberg
// modules, programs, study plans and events up to date and publishes it as SQLite snapshots.
// See docs/operations.md.
package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/secrets"
	"github.com/leonieziechmann/betula/internal/version"
)

const defaultDBPath = "radix.db"

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
		{"crawl-qis-modules", "Archive the QIS module descriptions (the source b-tu.de copies)", runCrawlQISModules},
		{"crawl-tree", "Walk the QIS program tree; fetch what is missing or stale", runCrawlTree},
		{"crawl-events", "Archive the QIS event pages that module pages link", runCrawlEvents},
		{"prune", "Remove what is not the current dataset: past events, archive pages nothing leads to", runPrune},
		{"build", "Derive the canonical tables from the raw page archive (no network)", runBuild},
		{"validate", "Check invariants, source conflicts and count baselines; exit 1 on failures", runValidate},
		{"export", "Write the read-optimized snapshot", runExport},
		{"serve-snapshot", "Publish exported snapshots over HTTP without running the service", runServeSnapshot},
	},
	{
		{"download-statutes", "Download the regulation PDFs of the programs (OPUS)", runDownloadStatutes},
		{"scan-curriculum", "Extract validated study plans from the regulation PDFs (Gemini enrichment optional)", runScanCurriculum},
		{"relink-plans", "Match the stored study plans against the catalog again; rewrites only the module links", runRelinkPlans},
		{"secret", "Manage credentials in the OS credential store: set | status | delete | migrate-config", runSecret},
	},
	{
		{"raw-vocab", "Print the distinct raw values of the normalized module fields", runRawVocab},
	},
}

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(2)
	}
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	// Development convenience: a git-ignored .env file. The real environment wins.
	if _, err := secrets.LoadDotEnv(envOr("RADIX_ENV_FILE", ".env")); err != nil {
		fmt.Fprintln(os.Stderr, "Error:", err)
		os.Exit(2)
	}

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
	fmt.Println("Radix " + version.Radix + ", the collector of Betula (catalog of BTU Cottbus-Senftenberg)\n\nUsage:\n  radix <command> [flags]      (radix <command> --help lists the flags)\n\nCommands:")
	for _, group := range commands {
		for _, c := range group {
			fmt.Printf("  %-20s %s\n", c.name, c.summary)
		}
		fmt.Println()
	}
	fmt.Println("Every command takes --db (default " + defaultDBPath + "), --log-format text|json, --log-level, --log-file.\n" +
		"Credentials are never read from flags or configuration files; see `radix secret status`.")
}

func openDB(path string) *catalogdb.DB {
	db, err := catalogdb.Open(path)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error opening database (%s): %v\n", path, err)
		os.Exit(1)
	}
	return db
}
