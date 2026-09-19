package main

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/jakob/btu-scraper/internal/analytics"
	"github.com/jakob/btu-scraper/internal/cache"
	"github.com/jakob/btu-scraper/internal/config"
	"github.com/jakob/btu-scraper/internal/gemini"
	"github.com/jakob/btu-scraper/internal/logger"
	"github.com/jakob/btu-scraper/internal/model"
	"github.com/jakob/btu-scraper/internal/provider"
	"github.com/jakob/btu-scraper/internal/refresher"
	"github.com/jakob/btu-scraper/internal/storage"
	"github.com/jakob/btu-scraper/internal/web"
)

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	subcommand := os.Args[1]
	subArgs := os.Args[2:]

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	switch subcommand {
	case "catalog":
		runCatalog(ctx, subArgs)
	case "detail":
		runDetail(ctx, subArgs)
	case "all":
		runAll(ctx, subArgs)
	case "list":
		runList(ctx, subArgs)
	case "show":
		runShow(ctx, subArgs)
	case "event":
		runEvent(ctx, subArgs)
	case "module-events":
		runModuleEvents(ctx, subArgs)
	case "show-events":
		runShowEvents(ctx, subArgs)
	case "fues":
		runFUES(ctx, subArgs)
	case "fues-majors":
		runFUESMajors(ctx, subArgs)
	case "fues-eligible":
		runFUESEligible(ctx, subArgs)
	case "programs":
		runPrograms(ctx, subArgs)
	case "download-statutes", "statutes":
		runDownloadStatutes(ctx, subArgs)
	case "scan-curriculum", "ai-curriculum":
		runScanCurriculum(ctx, subArgs)
	case "qis-tree", "qis-curriculum", "scan-qis":
		runQISCurriculum(ctx, subArgs)
	case "show-curriculum", "curriculum":
		runShowCurriculum(ctx, subArgs)
	case "program-modules":
		runProgramModules(ctx, subArgs)
	case "serve", "web":
		runServe(ctx, subArgs)
	case "providers":
		runProviders(ctx, subArgs)
	case "help", "--help", "-h":
		printUsage()
	default:
		fmt.Fprintf(os.Stderr, "Unknown command: %s\n\n", subcommand)
		printUsage()
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Print(`BTU Course & Module Scraper

Usage:
  scraper <command> [arguments]

Commands:
  serve                      Start the interactive HTMX-based Smart Module Catalog web app
                             [--port 8080] [--analytics-db btu_analytics.db] [--auto-refresh]
                             [--offpeak-start 1] [--offpeak-end 6] [--log-file btu_scraper.log]
  catalog                    Discover all modules from b-tu.de/modul and store summaries in SQLite
  detail <module-id>         Scrape full details for a specific module ID (e.g. 11101)
  all                        Scrape catalog and all module details with rate-limiting and caching
  list                       Query and filter stored modules from SQLite
  show <module-id>           Show formatted details of a stored module
  event <event-id | url>     Scrape lecture/event details and schedules from QIS
  module-events <module-id>  Scrape all current semester events linked to a module
  show-events <module-id>    Show timetable and room information for a module's events
  fues [--details]           Scrape approved FÜS list (use --details to also scrape study program links)
  fues-majors                List all study programs / majors present in the database
  fues-eligible <major>      List all FÜS modules NOT adjacent to the specified major
  programs                   Scrape/list official study programs, PO versions, and statute links
                             [--name <filter>] [--degree <filter>] [--download] [--from-db] [--refresh]
  download-statutes          Download study regulation PDFs (Studienordnungen & Prüfungsordnungen)
                             [--name <filter>] [--degree <filter>] [--workers 2] [--delay 200] [--out-dir <dir>] [--refresh]
  scan-curriculum            Extract study plan & semester progression from PDF statutes using Gemini AI
                             [--name <filter>] [--degree <filter>] [--delay 500] [--force] [--api-key <key>]
  qis-tree                   Extract complete study section, subject area & module tree from QISpos
                             [--name <filter>] [--degree <filter>] [--delay 300]
  show-curriculum <name|id>  Display structured semester study plan (Pflicht / Wahlpflicht / ECTS)
  program-modules <name|id>  List all modules associated with an official study program
  providers                  List all registered data providers

Global Flags:
  --db <path>                SQLite database file (default: btu_modules.db)
  --analytics-db <path>      Anonymous analytics SQLite database (default: btu_analytics.db)
  --cache-dir <path>         Local cache directory (default: .cache)
  --log-file <path>          Scraper and system log file (default: btu_scraper.log)
  --refresh                  Bypass cache and force re-fetching from source
`)
}

func runCatalog(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("catalog", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-fetch from web without using cache")
	_ = fs.Parse(reorderFlags(args))

	store, c, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	p, ok := reg.Get(provider.CatalogProviderName)
	if !ok {
		fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.CatalogProviderName)
		os.Exit(1)
	}

	catProvider := p.(*provider.BTUModuleCatalogProvider)
	fmt.Printf("[+] Scraping module catalog from %s (refresh=%v)...\n", provider.DefaultCatalogURL, *refresh)
	start := time.Now()

	count, err := catProvider.ScrapeCatalog(ctx, *refresh)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error scraping catalog: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("[✓] Discovered and saved %d modules in %s\n", count, time.Since(start).Round(time.Millisecond))
	_ = c
}

func runDetail(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("detail", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-fetch from web without using cache")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	moduleID := fs.Arg(0)
	if moduleID == "" {
		fmt.Fprintln(os.Stderr, "Error: missing module ID. Usage: scraper detail <module-id>")
		os.Exit(1)
	}

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	p, ok := reg.Get(provider.DetailProviderName)
	if !ok {
		fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.DetailProviderName)
		os.Exit(1)
	}

	detailProv := p.(*provider.BTUModuleDetailProvider)
	fmt.Printf("[+] Scraping module %s...\n", moduleID)
	if err := detailProv.ScrapeModule(ctx, moduleID, *refresh); err != nil {
		fmt.Fprintf(os.Stderr, "Error scraping module %s: %v\n", moduleID, err)
		os.Exit(1)
	}

	detail, err := store.GetModule(moduleID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error reading scraped module %s from DB: %v\n", moduleID, err)
		os.Exit(1)
	}

	if *asJSON {
		data, _ := json.MarshalIndent(detail, "", "  ")
		fmt.Println(string(data))
		return
	}

	printModuleCard(detail)
}

func runAll(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("all", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-fetch from web")
	limit := fs.Int("limit", 0, "Limit number of modules to scrape (0 for all)")
	workers := fs.Int("workers", 4, "Number of concurrent workers")
	delayMs := fs.Int("delay", 100, "Polite delay in milliseconds between requests per worker")
	_ = fs.Parse(reorderFlags(args))

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	// 1. Scrape catalog first
	catP, _ := reg.Get(provider.CatalogProviderName)
	catalogProvider := catP.(*provider.BTUModuleCatalogProvider)
	fmt.Println("[1/2] Discovering modules from catalog...")
	count, err := catalogProvider.ScrapeCatalog(ctx, *refresh)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error updating catalog: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("[✓] Catalog updated. %d modules in catalog.\n", count)

	// 2. Fetch list of modules from storage
	summaries, err := store.ListModules(storage.Filter{Limit: *limit})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error querying modules: %v\n", err)
		os.Exit(1)
	}

	total := len(summaries)
	fmt.Printf("[2/2] Scraping details for %d modules using %d workers...\n", total, *workers)

	detailP, _ := reg.Get(provider.DetailProviderName)
	detailProvider := detailP.(*provider.BTUModuleDetailProvider)

	jobs := make(chan model.ModuleSummary, total)
	for _, s := range summaries {
		jobs <- s
	}
	close(jobs)

	var wg sync.WaitGroup
	var completed uint64
	var failed uint64

	start := time.Now()

	for w := 1; w <= *workers; w++ {
		wg.Add(1)
		go func(workerID int) {
			defer wg.Done()
			for {
				select {
				case <-ctx.Done():
					return
				case item, ok := <-jobs:
					if !ok {
						return
					}

					err := detailProvider.ScrapeModule(ctx, item.ID, *refresh)
					if err != nil {
						atomic.AddUint64(&failed, 1)
						fmt.Printf("[Worker %d] Failed module %s: %v\n", workerID, item.ID, err)
					} else {
						curr := atomic.AddUint64(&completed, 1)
						if curr%25 == 0 || curr == uint64(total) {
							pct := float64(curr) / float64(total) * 100
							fmt.Printf("[Progress] %d/%d (%.1f%%) completed (elapsed: %s)\n",
								curr, total, pct, time.Since(start).Round(time.Second))
						}
					}

					if *delayMs > 0 {
						time.Sleep(time.Duration(*delayMs) * time.Millisecond)
					}
				}
			}
		}(w)
	}

	wg.Wait()
	fmt.Printf("\n[✓] Finished scraping: %d succeeded, %d failed in %s\n",
		completed, failed, time.Since(start).Round(time.Second))
}

func runList(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("list", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	query := fs.String("search", "", "Search term for code or title")
	department := fs.String("dept", "", "Filter by department")
	minCredits := fs.Float64("min-credits", 0, "Minimum credits (ECTS)")
	maxCredits := fs.Float64("max-credits", 0, "Maximum credits (ECTS)")
	limit := fs.Int("limit", 20, "Maximum number of results to display")
	_ = fs.Parse(reorderFlags(args))

	store, _, _ := setupApp(*dbPath, "")
	defer store.Close()

	modules, err := store.ListModules(storage.Filter{
		Query:      *query,
		Department: *department,
		MinCredits: *minCredits,
		MaxCredits: *maxCredits,
		Limit:      *limit,
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error listing modules: %v\n", err)
		os.Exit(1)
	}

	total, _ := store.Count()
	fmt.Printf("Total modules in DB: %d | Showing up to %d matches\n\n", total, len(modules))

	if len(modules) == 0 {
		fmt.Println("No modules found matching your query.")
		return
	}

	fmt.Printf("%-8s | %-55s | %s\n", "ID", "Title", "URL")
	fmt.Println(strings.Repeat("-", 90))
	for _, m := range modules {
		title := m.Title
		if len(title) > 52 {
			title = title[:49] + "..."
		}
		fmt.Printf("%-8s | %-55s | %s\n", m.ID, title, m.URL)
	}
}

func runShow(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("show", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	moduleID := fs.Arg(0)
	if moduleID == "" {
		fmt.Fprintln(os.Stderr, "Error: missing module ID. Usage: scraper show <module-id>")
		os.Exit(1)
	}

	store, _, _ := setupApp(*dbPath, "")
	defer store.Close()

	detail, err := store.GetModule(moduleID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Module %q not found in database: %v\n", moduleID, err)
		os.Exit(1)
	}

	if *asJSON {
		data, _ := json.MarshalIndent(detail, "", "  ")
		fmt.Println(string(data))
		return
	}

	printModuleCard(detail)

	curEntries, err := store.GetModuleCurriculumEntries(moduleID)
	if err == nil && len(curEntries) > 0 {
		fmt.Printf("\nOfficial Curriculum Study Plans (%d):\n", len(curEntries))
		for _, c := range curEntries {
			semStr := fmt.Sprintf("%d. Semester", c.RecommendedSemester)
			if c.RecommendedSemester <= 0 {
				semStr = "Semesterunabhängig / Wahlpflichtpool"
			}
			typeStr := c.ModuleType
			if typeStr == "" {
				typeStr = "Modul"
			}
			fmt.Printf("  • %s | %s (PO %s) -> %s | %s (%.1f LP)\n",
				c.Degree, c.ProgramName, c.POVersion, semStr, typeStr, c.Credits)
		}
		fmt.Println()
	}
}

func runProviders(ctx context.Context, args []string) {
	_, _, reg := setupApp("", "")
	fmt.Println("Registered Information Providers:")
	fmt.Println(strings.Repeat("-", 60))
	for _, p := range reg.List() {
		fmt.Printf("• %-15s : %s\n", p.Name(), p.Description())
	}
}

func runEvent(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("event", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-fetch from web")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	target := fs.Arg(0)
	if target == "" {
		fmt.Fprintln(os.Stderr, "Error: missing event ID or URL. Usage: scraper event <event-id | url>")
		os.Exit(1)
	}

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	p, ok := reg.Get(provider.EventProviderName)
	if !ok {
		fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.EventProviderName)
		os.Exit(1)
	}
	eventProv := p.(*provider.BTUEventProvider)

	var eventID, pageURL string
	if strings.HasPrefix(target, "http://") || strings.HasPrefix(target, "https://") {
		pageURL = target
	} else {
		eventID = target
	}

	fmt.Printf("[+] Scraping event %s...\n", target)
	event, err := eventProv.ScrapeEvent(ctx, eventID, pageURL, *refresh)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error scraping event: %v\n", err)
		os.Exit(1)
	}

	if *asJSON {
		data, _ := json.MarshalIndent(event, "", "  ")
		fmt.Println(string(data))
		return
	}

	printEventCard(event)
}

func runModuleEvents(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("module-events", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-fetch from web")
	limit := fs.Int("limit", 0, "Limit number of modules to scrape (0 for all)")
	delayMs := fs.Int("delay", 1000, "Delay between modules in milliseconds")
	_ = fs.Parse(reorderFlags(args))

	moduleID := fs.Arg(0)
	if moduleID == "" {
		fmt.Fprintln(os.Stderr, "Error: missing module ID. Usage: scraper module-events <module-id | all>")
		os.Exit(1)
	}

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	p, ok := reg.Get(provider.EventProviderName)
	if !ok {
		fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.EventProviderName)
		os.Exit(1)
	}
	eventProv := p.(*provider.BTUEventProvider)

	if strings.EqualFold(moduleID, "all") {
		rows, err := store.DB().QueryContext(ctx, `
			SELECT id FROM modules 
			WHERE current_semester_events IS NOT NULL AND current_semester_events != '' AND current_semester_events != '[]'
			ORDER BY CAST(id AS INTEGER), id ASC
		`)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error querying modules: %v\n", err)
			os.Exit(1)
		}
		defer rows.Close()

		var ids []string
		for rows.Next() {
			var id string
			if err := rows.Scan(&id); err == nil {
				ids = append(ids, id)
			}
		}

		if *limit > 0 && *limit < len(ids) {
			ids = ids[:*limit]
		}

		fmt.Printf("[+] Scraping current semester events for %d modules (delay: %dms)...\n", len(ids), *delayMs)
		totalEvents := 0
		for i, id := range ids {
			select {
			case <-ctx.Done():
				fmt.Println("\n[!] Scraping cancelled by user")
				return
			default:
			}

			count, err := eventProv.ScrapeEventsForModule(ctx, id, *refresh)
			if err != nil {
				fmt.Printf("[%d/%d] Modul %s: Fehler: %v\n", i+1, len(ids), id, err)
			} else {
				totalEvents += count
				if count > 0 {
					fmt.Printf("[%d/%d] Modul %s: %d Termine/Events gespeichert\n", i+1, len(ids), id, count)
				}
			}
			if *delayMs > 0 && i < len(ids)-1 {
				time.Sleep(time.Duration(*delayMs) * time.Millisecond)
			}
		}
		fmt.Printf("[✓] Fertig! Insgesamt %d Events für %d Module gespeichert.\n", totalEvents, len(ids))
		return
	}

	fmt.Printf("[+] Scraping current semester events for module %s...\n", moduleID)
	count, err := eventProv.ScrapeEventsForModule(ctx, moduleID, *refresh)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error scraping events for module %s: %v\n", moduleID, err)
		os.Exit(1)
	}

	fmt.Printf("[✓] Successfully scraped and stored %d events for module %s\n\n", count, moduleID)
	runShowEvents(ctx, []string{"-db", *dbPath, moduleID})
}

func runShowEvents(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("show-events", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	moduleID := fs.Arg(0)
	if moduleID == "" {
		fmt.Fprintln(os.Stderr, "Error: missing module ID. Usage: scraper show-events <module-id>")
		os.Exit(1)
	}

	store, _, _ := setupApp(*dbPath, "")
	defer store.Close()

	events, err := store.GetEventsForModule(moduleID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error loading events for module %s: %v\n", moduleID, err)
		os.Exit(1)
	}

	if len(events) == 0 {
		fmt.Printf("No events found in database for module %s. Try running: scraper module-events %s\n", moduleID, moduleID)
		return
	}

	if *asJSON {
		data, _ := json.MarshalIndent(events, "", "  ")
		fmt.Println(string(data))
		return
	}

	fmt.Printf("TIMETABLE & EVENTS FOR MODULE %s (%d events):\n", moduleID, len(events))
	fmt.Println(strings.Repeat("=", 80))
	for _, e := range events {
		printEventCard(&e)
	}
}

func printEventCard(e *model.EventDetail) {
	typeStr := e.EventType
	if typeStr == "" {
		typeStr = "Veranstaltung"
	}
	fmt.Printf("[%s] %s (%s)\n", typeStr, e.Title, e.EventNumber)
	if e.Semester != "" || e.SWS != "" {
		fmt.Printf("Semester: %-12s | SWS: %s\n", e.Semester, e.SWS)
	}
	if len(e.ResponsiblePersons) > 0 {
		var names []string
		for _, p := range e.ResponsiblePersons {
			if p.Role != "" {
				names = append(names, fmt.Sprintf("%s (%s)", p.Name, p.Role))
			} else {
				names = append(names, p.Name)
			}
		}
		fmt.Printf("Dozenten: %s\n", strings.Join(names, ", "))
	}

	if len(e.Schedules) > 0 {
		fmt.Println("Termine:")
		for _, sc := range e.Schedules {
			day := sc.DayOfWeek
			if day == "" {
				day = "-"
			}
			timeSlot := sc.TimeSlot
			if timeSlot == "" && sc.StartTime != "" {
				timeSlot = fmt.Sprintf("%s - %s", sc.StartTime, sc.EndTime)
			}
			rhythm := ""
			if sc.Rhythm != "" {
				rhythm = fmt.Sprintf("[%s]", sc.Rhythm)
			}
			fmt.Printf("  • %-4s %-20s %-12s\n", day, timeSlot, rhythm)
			if sc.Room != "" {
				fmt.Printf("    Raum:       %s\n", sc.Room)
			}
			if sc.Instructor != "" {
				fmt.Printf("    Lehrperson: %s\n", sc.Instructor)
			}
			if sc.Duration != "" {
				fmt.Printf("    Dauer:      %s\n", sc.Duration)
			}
			if sc.Comment != "" {
				fmt.Printf("    Bemerkung:  %s\n", sc.Comment)
			}
		}
	} else {
		fmt.Println("  (Keine festen Termine hinterlegt)")
	}

	if len(e.AssociatedModules) > 0 {
		fmt.Printf("Gehört zu Modulen: %s\n", strings.Join(e.AssociatedModules, ", "))
	}
	if e.RawURL != "" {
		fmt.Printf("QIS URL: %s\n", e.RawURL)
	}
	fmt.Println(strings.Repeat("-", 80))
}

func runFUES(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("fues", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-fetch from web without using cache")
	withDetails := fs.Bool("details", false, "Also scrape full course details & study program links for all FÜS modules")
	workers := fs.Int("workers", 2, "Number of concurrent workers when fetching details (default 2)")
	delayMs := fs.Int("delay", 500, "Delay in ms between requests per worker (default 500ms)")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	p, ok := reg.Get(provider.FUESProviderName)
	if !ok {
		fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.FUESProviderName)
		os.Exit(1)
	}

	fuesProv := p.(*provider.BTUFUESProvider)
	fmt.Printf("[+] Scraping Fachübergreifendes Studium (FÜS) modules from QIS (refresh=%v)...\n", *refresh)
	start := time.Now()

	modules, err := fuesProv.ScrapeFUES(ctx, *refresh)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error scraping FÜS: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("[✓] Discovered and registered %d FÜS modules in %s\n", len(modules), time.Since(start).Round(time.Millisecond))

	if *withDetails {
		total := len(modules)
		fmt.Printf("\n[+] Scraping full details & study program links for %d FÜS modules (%d workers)...\n", total, *workers)

		detailP, _ := reg.Get(provider.DetailProviderName)
		detailProvider := detailP.(*provider.BTUModuleDetailProvider)

		jobs := make(chan model.FUESModule, total)
		for _, m := range modules {
			jobs <- m
		}
		close(jobs)

		var wg sync.WaitGroup
		var completed uint64
		var failed uint64
		detStart := time.Now()

		for w := 1; w <= *workers; w++ {
			wg.Add(1)
			go func(workerID int) {
				defer wg.Done()
				for {
					select {
					case <-ctx.Done():
						return
					case item, ok := <-jobs:
						if !ok {
							return
						}

						err := detailProvider.ScrapeModule(ctx, item.ID, *refresh)
						if err != nil {
							atomic.AddUint64(&failed, 1)
							fmt.Printf("[Worker %d] Failed module %s: %v\n", workerID, item.ID, err)
						} else {
							curr := atomic.AddUint64(&completed, 1)
							if curr%20 == 0 || curr == uint64(total) {
								pct := float64(curr) / float64(total) * 100
								fmt.Printf("[Progress] %d/%d (%.1f%%) completed (elapsed: %s)\n",
									curr, total, pct, time.Since(detStart).Round(time.Second))
							}
						}

						if *delayMs > 0 {
							time.Sleep(time.Duration(*delayMs) * time.Millisecond)
						}
					}
				}
			}(w)
		}
		wg.Wait()
		fmt.Printf("[✓] Finished scraping FÜS details: %d succeeded, %d failed in %s\n\n",
			completed, failed, time.Since(detStart).Round(time.Second))
	}

	if *asJSON {
		data, _ := json.MarshalIndent(modules, "", "  ")
		fmt.Println(string(data))
		return
	}

	if !*withDetails {
		fmt.Printf("\n%-8s | %-6s | %-7s | %-55s\n", "ID", "ECTS", "Sprache", "Titel")
		fmt.Println(strings.Repeat("-", 85))
		for _, m := range modules {
			title := m.Title
			if len(title) > 52 {
				title = title[:49] + "..."
			}
			ects := m.CreditsRaw
			if ects == "" {
				ects = fmt.Sprintf("%.1f", m.Credits)
			}
			fmt.Printf("%-8s | %-6s | %-7s | %-55s\n", m.ID, ects, m.Language, title)
		}
	}
}

func runFUESMajors(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("fues-majors", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	store, _, _ := setupApp(*dbPath, "")
	defer store.Close()

	majors, err := store.ListMajors()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error listing study programs: %v\n", err)
		os.Exit(1)
	}

	if *asJSON {
		data, _ := json.MarshalIndent(majors, "", "  ")
		fmt.Println(string(data))
		return
	}

	fmt.Printf("Known Study Programs / Majors in Database (%d):\n", len(majors))
	fmt.Println(strings.Repeat("-", 60))
	for i, m := range majors {
		fmt.Printf("%3d. %s\n", i+1, m)
	}
	fmt.Println("\nTip: Use one of these majors with: scraper fues-eligible \"<major>\"")
}

func runFUESEligible(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("fues-eligible", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	minCredits := fs.Float64("min-credits", 0, "Minimum credits (ECTS)")
	limit := fs.Int("limit", 0, "Limit number of results (0 for all)")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	major := fs.Arg(0)
	if major == "" {
		fmt.Fprintln(os.Stderr, "Error: missing major/study program name.")
		fmt.Fprintln(os.Stderr, "Usage: scraper fues-eligible \"<major>\" (e.g. scraper fues-eligible \"Informatik\")")
		fmt.Fprintln(os.Stderr, "Run 'scraper fues-majors' to see available majors.")
		os.Exit(1)
	}

	store, _, _ := setupApp(*dbPath, "")
	defer store.Close()

	modules, err := store.GetFUESForMajor(major, *minCredits)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error retrieving eligible FÜS modules: %v\n", err)
		os.Exit(1)
	}

	if *limit > 0 && len(modules) > *limit {
		modules = modules[:*limit]
	}

	if *asJSON {
		data, _ := json.MarshalIndent(modules, "", "  ")
		fmt.Println(string(data))
		return
	}

	fmt.Printf("Eligible Fachübergreifendes Studium (FÜS) modules for major: %q\n", major)
	fmt.Printf("(All modules below are approved FÜS and NOT adjacent to %q)\n", major)
	fmt.Printf("Found: %d eligible modules\n\n", len(modules))

	if len(modules) == 0 {
		fmt.Println("No eligible modules found. Note: Make sure module details have been scraped via 'scraper fues' or 'scraper all' to populate program assignments.")
		return
	}

	fmt.Printf("%-8s | %-6s | %-10s | %-55s\n", "ID", "ECTS", "Sprache", "Titel")
	fmt.Println(strings.Repeat("-", 88))
	for _, m := range modules {
		title := m.TitleDE
		if title == "" {
			title = m.TitleEN
		}
		if len(title) > 52 {
			title = title[:49] + "..."
		}
		ects := m.CreditsRaw
		if ects == "" {
			ects = fmt.Sprintf("%.1f", m.Credits)
		}
		lang := m.Language
		if len(lang) > 10 {
			lang = lang[:10]
		}
		fmt.Printf("%-8s | %-6s | %-10s | %-55s\n", m.ID, ects, lang, title)
	}
}

func runPrograms(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("programs", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-fetch from web without using cache")
	nameFilter := fs.String("name", "", "Filter study program name (e.g. Informatik)")
	degreeFilter := fs.String("degree", "", "Filter degree (e.g. Bachelor, Master)")
	download := fs.Bool("download", false, "Attempt downloading statute/amendment PDFs (detects bot protection)")
	fromDB := fs.Bool("from-db", false, "List already stored study programs from database without querying QIS")
	workers := fs.Int("workers", 1, "Concurrent workers for crawling tree branches (default 1 for gentle QIS scraping)")
	delayMs := fs.Int("delay", 1000, "Polite delay in milliseconds between requests per worker (default 1000ms)")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	var programs []model.OfficialStudyProgram

	if *fromDB {
		var err error
		programs, err = store.ListOfficialPrograms(*nameFilter, *degreeFilter)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error loading study programs from database: %v\n", err)
			os.Exit(1)
		}

		if *download && len(programs) > 0 {
			p, ok := reg.Get(provider.ProgramTreeProviderName)
			if ok {
				progProv := p.(*provider.BTUProgramTreeProvider)
				fmt.Printf("[+] Downloading regulation PDFs for %d stored study programs...\n", len(programs))
				updated, stats, err := progProv.DownloadProgramDocuments(ctx, programs, *refresh, *workers, *delayMs)
				if err != nil {
					fmt.Fprintf(os.Stderr, "Warning during PDF downloads: %v\n", err)
				} else {
					programs = updated
					fmt.Printf("[✓] PDF download summary: %d downloaded, %d already cached, %d errors (across %d unique documents in %s)\n\n",
						stats.Downloaded, stats.SkippedCached, stats.Errors, stats.UniqueURLs, progProv.DownloadDir())
				}
			}
		}
	} else {
		p, ok := reg.Get(provider.ProgramTreeProviderName)
		if !ok {
			fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.ProgramTreeProviderName)
			os.Exit(1)
		}
		progProv := p.(*provider.BTUProgramTreeProvider)

		fmt.Printf("[+] Traversing official study program tree in QIS (filter=%q, download=%v, refresh=%v)...\n",
			*nameFilter, *download, *refresh)
		start := time.Now()

		var err error
		programs, err = progProv.ScrapeProgramTree(ctx, *nameFilter, *download, *workers, *delayMs, *refresh)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error traversing study program tree: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("[✓] Completed traversal: %d program branches scraped in %s\n\n", len(programs), time.Since(start).Round(time.Millisecond))
	}

	if *degreeFilter != "" && !*fromDB {
		var filtered []model.OfficialStudyProgram
		lowerDeg := strings.ToLower(*degreeFilter)
		for _, prog := range programs {
			if strings.Contains(strings.ToLower(prog.Degree), lowerDeg) {
				filtered = append(filtered, prog)
			}
		}
		programs = filtered
	}

	if *asJSON {
		data, _ := json.MarshalIndent(programs, "", "  ")
		fmt.Println(string(data))
		return
	}

	if len(programs) == 0 {
		fmt.Println("No study programs found matching the given criteria.")
		return
	}

	fmt.Printf("OFFICIAL STUDY PROGRAMS & REGULATIONS (%d):\n", len(programs))
	fmt.Println(strings.Repeat("=", 90))

	for _, p := range programs {
		fmt.Printf("🎓 %s | %s\n", p.ProgramName, p.Degree)
		fmt.Printf("   PO-Version: %s\n", p.POVersion)
		if len(p.Documents) == 0 {
			fmt.Println("   Dokumente: (Keine Prüfungsordnungs-Dokumente im QIS hinterlegt)")
		} else {
			fmt.Printf("   Dokumente (%d):\n", len(p.Documents))
			for _, doc := range p.Documents {
				typeTag := "Ordnung"
				if doc.DocType == "amendment" {
					typeTag = "Änderung"
				} else if doc.DocType == "statute" {
					typeTag = "Satzung"
				}

				statusTag := ""
				if doc.DownloadStatus == "downloaded" {
					statusTag = fmt.Sprintf(" [Heruntergeladen -> %s]", doc.LocalPath)
				} else if doc.DownloadStatus == "blocked_bot_checker" {
					statusTag = " [Bot-Schutz erkannt (OPUS 4) - Download übersprungen]"
				}

				fmt.Printf("     • [%s] %s%s\n", typeTag, doc.Title, statusTag)
				fmt.Printf("       URL: %s\n", doc.URL)
			}
		}
		if p.QISURL != "" {
			fmt.Printf("   QIS: %s\n", p.QISURL)
		}
		fmt.Println(strings.Repeat("-", 90))
	}
}

func runDownloadStatutes(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("download-statutes", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	refresh := fs.Bool("refresh", false, "Force re-downloading even if PDFs exist locally")
	nameFilter := fs.String("name", "", "Filter study program name (e.g. Informatik)")
	degreeFilter := fs.String("degree", "", "Filter degree (e.g. Bachelor, Master)")
	workers := fs.Int("workers", 2, "Number of concurrent download workers (default 2)")
	delayMs := fs.Int("delay", 200, "Polite delay in milliseconds between requests (default 200ms)")
	outDir := fs.String("out-dir", "", "Custom target directory for PDFs (defaults to config statutes_dir)")
	asJSON := fs.Bool("json", false, "Output results as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	p, ok := reg.Get(provider.ProgramTreeProviderName)
	if !ok {
		fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.ProgramTreeProviderName)
		os.Exit(1)
	}
	progProv := p.(*provider.BTUProgramTreeProvider)
	if *outDir != "" {
		progProv.SetDownloadDir(*outDir)
	}

	programs, err := store.ListOfficialPrograms(*nameFilter, *degreeFilter)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error loading study programs from database: %v\n", err)
		os.Exit(1)
	}

	if len(programs) == 0 {
		fmt.Println("No study programs found in database matching the given filter.")
		fmt.Println("Hint: Run 'scraper programs' first to discover programs from QIS.")
		return
	}

	fmt.Printf("[+] Automated download of regulation PDFs for %d study program branches...\n", len(programs))
	fmt.Printf("    Destination: %s | Workers: %d | Delay: %dms | ForceRefresh: %v\n\n",
		progProv.DownloadDir(), *workers, *delayMs, *refresh)

	start := time.Now()
	updatedPrograms, stats, err := progProv.DownloadProgramDocuments(ctx, programs, *refresh, *workers, *delayMs)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error downloading documents: %v\n", err)
	}

	if *asJSON {
		data, _ := json.MarshalIndent(updatedPrograms, "", "  ")
		fmt.Println(string(data))
		return
	}

	fmt.Printf("\n[✓] Completed in %s:\n", time.Since(start).Round(time.Millisecond))
	fmt.Printf("    • Unique PDF Documents: %d\n", stats.UniqueURLs)
	fmt.Printf("    • Newly Downloaded:     %d\n", stats.Downloaded)
	fmt.Printf("    • Reused Local Cache:   %d\n", stats.SkippedCached)
	if stats.Errors > 0 {
		fmt.Printf("    • Errors / Blocked:     %d\n", stats.Errors)
	}
	fmt.Printf("    • Target Directory:     %s\n", progProv.DownloadDir())
}

func runScanCurriculum(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("scan-curriculum", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	nameFilter := fs.String("name", "", "Filter study program name (e.g. Informatik)")
	degreeFilter := fs.String("degree", "", "Filter degree (e.g. Bachelor, Master)")
	apiKeyFlag := fs.String("api-key", "", "Gemini API key (defaults to config or GEMINI_API_KEY)")
	modelFlag := fs.String("model", "", "Gemini model (default: gemini-3.5-flash-lite)")
	delayMs := fs.Int("delay", 500, "Polite delay in milliseconds between requests (default: 500ms)")
	force := fs.Bool("force", false, "Force re-scan even if curriculum already extracted")
	asJSON := fs.Bool("json", false, "Output extraction and validation reports as JSON Lines")
	dryRun := fs.Bool("dry-run", false, "Validate and write reports without changing curriculum records")
	startTerm := fs.String("start-term", "auto", "Intake: auto, winter, summer, unknown")
	tolerance := fs.Float64("credit-tolerance", 6, "Allowed deviation from expected semester LP (warning only)")
	reportDir := fs.String("report-dir", ".cache/curriculum", "Directory for extraction and validation evidence")
	programID := fs.String("program-id", "", "Exact official program ID")
	pdfPath := fs.String("pdf", "", "Explicit regulation PDF (otherwise resolve base statute and reviewed amendments)")
	planPages := fs.String("plan-pages", "", "Physical PDF pages containing complete plan variants, e.g. 7,9 (requires --pdf)")
	_ = fs.Parse(reorderFlags(args))
	if *pdfPath != "" && *programID == "" {
		fmt.Fprintln(os.Stderr, "Error: --pdf requires --program-id to avoid assigning one document to unrelated programs")
		os.Exit(1)
	}

	if (*startTerm != "auto" && *startTerm != "winter" && *startTerm != "summer" && *startTerm != "unknown") || *tolerance < 0 {
		fmt.Fprintln(os.Stderr, "Error: invalid --start-term or negative --credit-tolerance")
		os.Exit(1)
	}
	cfg, _, err := config.Load("")
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	apiKey := *apiKeyFlag
	if apiKey == "" {
		apiKey = cfg.Gemini.APIKey
	}
	if apiKey == "" {
		apiKey = os.Getenv("GEMINI_API_KEY")
	}
	if apiKey == "" {
		fmt.Fprintln(os.Stderr, "Error: missing Gemini API key.")
		fmt.Fprintln(os.Stderr, "Provide via --api-key <key>, GEMINI_API_KEY env var, or gemini.api_key in config.yaml")
		os.Exit(1)
	}

	modelName := *modelFlag
	if modelName == "" {
		modelName = cfg.Gemini.Model
	}
	if modelName == "" {
		modelName = gemini.DefaultModel
	}

	store, _, _ := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	geminiClient := gemini.NewClient(apiKey, modelName)
	if *planPages != "" {
		if *pdfPath == "" {
			fmt.Fprintln(os.Stderr, "--plan-pages requires --pdf")
			os.Exit(1)
		}
		for _, part := range strings.Split(*planPages, ",") {
			n, err := strconv.Atoi(strings.TrimSpace(part))
			if err != nil || n < 1 {
				fmt.Fprintln(os.Stderr, "Invalid --plan-pages: use comma-separated positive page numbers")
				os.Exit(1)
			}
			geminiClient.PlanPages = append(geminiClient.PlanPages, n)
		}
	}
	selectedPages := append([]int(nil), geminiClient.PlanPages...)
	catalog, err := store.GetCurriculumCatalog()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	if err := os.MkdirAll(*reportDir, 0755); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	audit, err := logger.NewCurriculumAudit(*reportDir, apiKey)
	if err != nil {
		fmt.Fprintln(os.Stderr, "Cannot create curriculum log:", err)
		os.Exit(1)
	}
	defer audit.Close()
	*reportDir = audit.Dir
	fmt.Fprintln(os.Stderr, "Curriculum audit:", audit.Dir)
	logEvent := func(e logger.CurriculumEvent) {
		if err := audit.Record(e); err != nil {
			fmt.Fprintln(os.Stderr, "Cannot persist curriculum log:", err)
			os.Exit(1)
		}
	}
	finishAudit := func(status string) {
		if err := store.Checkpoint(); err != nil {
			logEvent(logger.CurriculumEvent{Level: "error", Code: "checkpoint_failed", Message: err.Error(), Action: "Datenbank-Snapshot vor dem Bereitstellen prüfen."})
		}
		if err := audit.Finish(status); err != nil {
			fmt.Fprintln(os.Stderr, "Cannot persist audit summary:", err)
			os.Exit(1)
		}
	}

	programs, err := store.ListOfficialPrograms(*nameFilter, *degreeFilter)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error loading study programs from database: %v\n", err)
		os.Exit(1)
	}

	if len(programs) == 0 {
		fmt.Fprintln(os.Stderr, "No study programs found matching filter.")
		finishAudit("no_programs")
		return
	}

	// Filter to programs that have downloaded statute PDFs
	var eligible []model.OfficialStudyProgram
	for _, p := range programs {
		if *programID != "" && p.ID != *programID {
			continue
		}
		if *pdfPath != "" {
			eligible = append(eligible, p)
			continue
		}
		// Missing documents are outcomes too, rather than silently omitted programs.
		eligible = append(eligible, p)
	}

	if len(eligible) == 0 {
		fmt.Fprintln(os.Stderr, "No downloaded statute PDFs found for matching study programs.")
		fmt.Fprintln(os.Stderr, "Hint: Run 'scraper download-statutes' first to download the regulation PDFs.")
		finishAudit("no_programs")
		return
	}

	fmt.Fprintf(os.Stderr, "[+] Starting AI curriculum scan with Gemini (%s) for %d study program branches...\n", modelName, len(eligible))
	fmt.Fprintf(os.Stderr, "    Delay: %dms | Force: %v\n\n", *delayMs, *force)

	start := time.Now()
	totalExtracted := 0
	totalMatched := 0
	programsScanned := 0
	failed := 0

	for idx, prog := range eligible {
		select {
		case <-ctx.Done():
			fmt.Fprintln(os.Stderr, "\nAborted by user.")
			finishAudit("cancelled")
			return
		default:
		}

		// Check if already scanned
		if !*force {
			verified, err := store.HasValidatedCurriculum(prog.ID)
			if err != nil {
				logEvent(logger.CurriculumEvent{Level: "error", Code: "database_read", ProgramID: prog.ID, Program: prog.ProgramName, Message: err.Error(), Status: "failed", Action: "Datenbankschema und Zugriff prüfen."})
				failed++
				continue
			}
			if verified {
				fmt.Fprintf(os.Stderr, "[%d/%d] Validated plan already present: %s\n", idx+1, len(eligible), prog.ProgramName)
				logEvent(logger.CurriculumEvent{Level: "info", Code: "already_validated", ProgramID: prog.ID, Program: prog.ProgramName, Message: "Existing validated plan preserved", Status: "skipped_validated"})
				continue
			}
		}

		// Never silently use an old base statute when an amendment is also present.
		var bestDoc *model.ProgramRegulationDocument
		var sourceSelection gemini.RegulationSelection
		geminiClient.PlanPages = selectedPages
		var available []model.ProgramRegulationDocument
		seenPaths := make(map[string]bool)
		for _, d := range prog.Documents {
			if fi, err := os.Stat(d.LocalPath); err == nil && fi.Size() > 0 && !seenPaths[d.LocalPath] {
				available = append(available, d)
				seenPaths[d.LocalPath] = true
			}
		}
		if *pdfPath != "" {
			bestDoc = &model.ProgramRegulationDocument{LocalPath: *pdfPath}
		} else if len(available) == 1 {
			bestDoc = &available[0]
		} else if len(available) > 1 {
			var selectionErr error
			sourceSelection, selectionErr = gemini.SelectRegulationSources(prog, available)
			if selectionErr != nil {
				logEvent(logger.CurriculumEvent{Level: "error", Code: "ambiguous_source", ProgramID: prog.ID, Program: prog.ProgramName, Message: selectionErr.Error(), Status: "needs_review"})
				failed++
				continue
			}
			bestDoc = &sourceSelection.Plan
			geminiClient.PlanPages = sourceSelection.Pages
			for _, review := range sourceSelection.Reviews {
				logEvent(logger.CurriculumEvent{Level: "info", Code: "amendment_review", ProgramID: prog.ID, Program: prog.ProgramName, Source: review.Source, Message: review.Decision + ": " + review.Evidence + " SHA256=" + review.SHA256})
			}
		}

		if bestDoc == nil {
			logEvent(logger.CurriculumEvent{Level: "warning", Code: "missing_pdf", ProgramID: prog.ID, Program: prog.ProgramName, Message: "No downloaded regulation PDF available", Status: "missing_source", Action: "download-statutes ausführen oder offizielle PDF-Quelle ergänzen."})
			failed++
			continue
		}
		logEvent(logger.CurriculumEvent{Level: "info", Code: "scan_started", ProgramID: prog.ID, Program: prog.ProgramName, Source: bestDoc.LocalPath, Message: "Extracting and validating study plan", Status: "running"})

		fmt.Fprintf(os.Stderr, "[%d/%d] 🤖 Scanning %s (%s) via %s...\n",
			idx+1, len(eligible), prog.ProgramName, prog.Degree, filepath.Base(bestDoc.LocalPath))

		res, err := geminiClient.ExtractCurriculumFromPDF(ctx, bestDoc.LocalPath, prog.ProgramName+" / "+prog.Degree+" / PO "+prog.POVersion)
		if err != nil {
			err = fmt.Errorf("%s", audit.Clean(err.Error()))
			fmt.Fprintf(os.Stderr, "    ❌ Error scanning %s: %v\n", bestDoc.LocalPath, err)
			errorReport := map[string]string{"program_id": prog.ID, "source": bestDoc.LocalPath, "status": "rejected", "error": err.Error()}
			data, _ := json.MarshalIndent(errorReport, "", "  ")
			path := filepath.Join(*reportDir, fmt.Sprintf("%x.error.json", sha256.Sum256([]byte(prog.ID+"|"+bestDoc.LocalPath))))
			if writeErr := os.WriteFile(path, data, 0644); writeErr != nil {
				fmt.Fprintln(os.Stderr, "Cannot save rejection report:", writeErr)
			}
			if *asJSON {
				_ = json.NewEncoder(os.Stdout).Encode(errorReport)
			}
			code, action := logger.ClassifyCurriculumFailure(err.Error())
			logEvent(logger.CurriculumEvent{Level: "error", Code: code, ProgramID: prog.ID, Program: prog.ProgramName, Source: bestDoc.LocalPath, Message: err.Error(), Action: action, Report: path, Status: "needs_review"})
			failed++
			continue
		}

		if len(sourceSelection.Reviews) > 0 {
			if applyErr := sourceSelection.Apply(res); applyErr != nil {
				sourceSelection.Issues = append(sourceSelection.Issues, gemini.ValidationIssue{Severity: "error", Code: "amendment_requires_patch", Message: applyErr.Error()})
			}
		}
		validation := gemini.ValidateCurriculum(res, catalog, *startTerm, *tolerance)
		validation.Issues = append(validation.Issues, sourceSelection.Issues...)
		if len(sourceSelection.Issues) > 0 {
			validation.Valid = false
		}
		report := struct {
			ProgramID  string                             `json:"program_id"`
			Source     string                             `json:"source"`
			Extraction *gemini.CurriculumExtractionResult `json:"extraction"`
			Validation gemini.ValidationReport            `json:"validation"`
			Model      string                             `json:"model"`
			CheckedAt  time.Time                          `json:"checked_at"`
		}{prog.ID, bestDoc.LocalPath, res, validation, modelName, time.Now().UTC()}
		reportBytes, reportErr := json.MarshalIndent(report, "", "  ")
		// Program IDs contain characters that are illegal in Windows filenames.
		reportName := fmt.Sprintf("%x.json", sha256.Sum256([]byte(prog.ID+"|"+bestDoc.LocalPath)))
		if reportErr == nil {
			reportErr = os.WriteFile(filepath.Join(*reportDir, reportName), reportBytes, 0644)
		}
		if reportErr != nil {
			fmt.Fprintln(os.Stderr, "Cannot save validation evidence:", reportErr)
			logEvent(logger.CurriculumEvent{Level: "error", Code: "report_write_failed", ProgramID: prog.ID, Program: prog.ProgramName, Message: reportErr.Error(), Status: "failed", Action: "Freien Speicher und Schreibrechte prüfen."})
			failed++
			continue
		}
		if *asJSON {
			_ = json.NewEncoder(os.Stdout).Encode(report)
		}
		for _, issue := range validation.Issues {
			fmt.Fprintf(os.Stderr, "    %s [%s] %s: %s\n", issue.Severity, issue.Code, issue.Module, issue.Message)
			logEvent(logger.CurriculumEvent{Level: issue.Severity, Code: issue.Code, ProgramID: prog.ID, Program: prog.ProgramName, Source: bestDoc.LocalPath, Module: issue.Module, Message: issue.Message, Report: filepath.Join(*reportDir, reportName), Action: logger.CurriculumIssueAction(issue.Code)})
		}
		fmt.Fprintf(os.Stderr, "    Evidence: %s\n", filepath.Join(*reportDir, reportName))
		if !validation.Valid {
			fmt.Fprintln(os.Stderr, "    Rejected: existing curriculum preserved.")
			logEvent(logger.CurriculumEvent{Level: "info", Code: "validation_rejected", ProgramID: prog.ID, Program: prog.ProgramName, Message: "Validation errors; existing records preserved", Status: "needs_review", Report: filepath.Join(*reportDir, reportName)})
			failed++
			continue
		}
		if *dryRun {
			logEvent(logger.CurriculumEvent{Level: "info", Code: "dry_run_valid", ProgramID: prog.ID, Program: prog.ProgramName, Message: "Valid extraction, not saved (dry run)", Status: "dry_run_valid"})
			programsScanned++
			totalExtracted += len(res.Modules)
			totalMatched += validation.Matched
			continue
		}
		var curModules []model.CurriculumModule
		cellEvidence := map[string]string{}
		for _, cell := range res.Layout.Cells {
			data, _ := json.Marshal(cell)
			cellEvidence[cell.ID] = string(data)
		}
		for _, m := range res.Modules {
			curModules = append(curModules, model.CurriculumModule{
				SourceEvidence:         cellEvidence[m.SourceCell],
				ProgramID:              prog.ID,
				ModuleID:               catalogModuleID(m, catalog),
				ProgramName:            prog.ProgramName,
				Degree:                 prog.Degree,
				POVersion:              prog.POVersion,
				ModuleCode:             m.ModuleCode,
				ModuleName:             m.ModuleName,
				ModuleNameEN:           m.ModuleNameEN,
				RecommendedSemester:    m.RecommendedSemester,
				RecommendedSemesterRaw: m.RecommendedSemesterRaw,
				SemesterSpan:           m.SemesterSpan,
				StartSemester:          m.StartSemester,
				EndSemester:            m.EndSemester,
				Credits:                m.Credits,
				MinCredits:             m.MinCredits,
				MaxCredits:             m.MaxCredits,
				ModuleType:             m.ModuleType,
				StudySection:           m.StudySection,
				SubjectArea:            m.SubjectArea,
				AreaRules:              m.AreaRules,
				Specialization:         m.Specialization,
				SWS:                    m.SWS,
				ExamType:               m.ExamType,
				Graded:                 m.Graded,
				Prerequisites:          m.Prerequisites,
				Remarks:                m.Remarks,
				SourceFile:             bestDoc.LocalPath,
			})
		}

		layoutData, _ := json.Marshal(res.Layout)
		if err := store.SaveValidatedCurriculumModules(prog.ID, prog.ProgramName, prog.Degree, prog.POVersion, curModules, bestDoc.LocalPath, string(layoutData)); err != nil {
			fmt.Fprintf(os.Stderr, "    ❌ Error saving curriculum for %s: %v\n", prog.ProgramName, err)
			logEvent(logger.CurriculumEvent{Level: "error", Code: "database_write", ProgramID: prog.ID, Program: prog.ProgramName, Message: err.Error(), Status: "failed", Action: "Datenbanktransaktion und Schreibrechte prüfen."})
			failed++
			continue
		}

		totalExt, matched := len(curModules), validation.Matched
		status := "saved"
		if len(validation.Issues) > 0 {
			status = "saved_with_warnings"
		}
		logEvent(logger.CurriculumEvent{Level: "info", Code: "saved", ProgramID: prog.ID, Program: prog.ProgramName, Source: bestDoc.LocalPath, Message: fmt.Sprintf("%d requirements, %d catalog links", totalExt, matched), Status: status, Report: filepath.Join(*reportDir, reportName)})

		programsScanned++
		totalExtracted += totalExt
		totalMatched += matched

		fmt.Fprintf(os.Stderr, "    ✓ Extracted %d curriculum modules (%d linked to catalog modules)\n",
			totalExt, matched)

		if *delayMs > 0 {
			time.Sleep(time.Duration(*delayMs) * time.Millisecond)
		}
	}

	fmt.Fprintf(os.Stderr, "\n[✓] AI Curriculum Scan completed in %s:\n", time.Since(start).Round(time.Millisecond))
	fmt.Fprintf(os.Stderr, "    • Study Programs Scanned:   %d\n", programsScanned)
	fmt.Fprintf(os.Stderr, "    • Modules Extracted:        %d\n", totalExtracted)
	fmt.Fprintf(os.Stderr, "    • Modules Linked to DB:     %d\n", totalMatched)
	status := "completed"
	if failed > 0 {
		status = "completed_with_issues"
	}
	finishAudit(status)
	fmt.Fprintln(os.Stderr, "Review report:", filepath.Join(audit.Dir, "review.md"))

	if failed > 0 {
		fmt.Fprintf(os.Stderr, "%d program(s) require review; existing records were preserved for rejected extractions.\n", failed)
		os.Exit(1)
	}
}

func runQISCurriculum(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("qis-tree", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	cacheDir := fs.String("cache-dir", ".cache", "Cache directory")
	nameFilter := fs.String("name", "", "Filter study programs by name (e.g. 'Informatik')")
	degreeFilter := fs.String("degree", "", "Filter study programs by degree")
	delayMs := fs.Int("delay", 300, "Delay in ms between QIS tree node requests (default 300ms)")
	_ = fs.Parse(reorderFlags(args))

	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	treeP, ok := reg.Get(provider.ProgramTreeProviderName)
	if !ok {
		fmt.Fprintf(os.Stderr, "Error: %s not registered\n", provider.ProgramTreeProviderName)
		os.Exit(1)
	}
	treeProvider := treeP.(*provider.BTUProgramTreeProvider)

	programs, err := store.ListOfficialPrograms(*nameFilter, *degreeFilter)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error querying programs: %v\n", err)
		os.Exit(1)
	}

	var eligible []model.OfficialStudyProgram
	for _, p := range programs {
		if p.QISURL != "" {
			eligible = append(eligible, p)
		}
	}

	if len(eligible) == 0 {
		fmt.Printf("No official study programs with QIS URL found matching filter %q\n", *nameFilter)
		return
	}

	fmt.Printf("[+] Starting QISpos tree curriculum scan for %d study programs (delay=%dms)...\n\n", len(eligible), *delayMs)

	totalModulesExtracted := 0
	totalModulesLinked := 0

	for idx, prog := range eligible {
		select {
		case <-ctx.Done():
			fmt.Println("\n[!] Scan interrupted by user.")
			return
		default:
		}

		fmt.Printf("[%d/%d] 🌲 Scraping QISpos tree for %s (%s, PO %s)...\n",
			idx+1, len(eligible), prog.ProgramName, prog.Degree, prog.POVersion)

		curModules, err := treeProvider.TraverseQISCurriculum(ctx, prog.QISURL, prog, *delayMs)
		if err != nil {
			fmt.Fprintf(os.Stderr, "    ❌ Error traversing QIS tree: %v\n", err)
			continue
		}

		if len(curModules) == 0 {
			fmt.Printf("    ℹ️ No module leaves found under QIS node.\n")
			continue
		}

		if err := store.SaveCurriculumModules(prog.ID, prog.ProgramName, prog.Degree, prog.POVersion, curModules, "qis_tree"); err != nil {
			fmt.Fprintf(os.Stderr, "    ❌ Error saving curriculum modules: %v\n", err)
			continue
		}

		tot, matched, err := store.MatchAndLinkCurriculumModules(prog.ID)
		if err != nil {
			fmt.Fprintf(os.Stderr, "    ⚠️ Error matching modules: %v\n", err)
		}

		fmt.Printf("    ✓ Found %d modules in QIS tree (%d linked to DB catalog)\n", len(curModules), matched)
		totalModulesExtracted += tot
		totalModulesLinked += matched
	}

	fmt.Printf("\n[✓] QISpos Tree Curriculum Scan completed:\n")
	fmt.Printf("    • Programs processed:  %d\n", len(eligible))
	fmt.Printf("    • Total modules found: %d\n", totalModulesExtracted)
	fmt.Printf("    • Linked to DB:        %d\n\n", totalModulesLinked)
}

func runShowCurriculum(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("show-curriculum", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	degreeFilter := fs.String("degree", "", "Filter by degree (e.g. Bachelor, Master)")
	asJSON := fs.Bool("json", false, "Output results as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	progNameOrID := fs.Arg(0)
	if progNameOrID == "" {
		fmt.Fprintln(os.Stderr, "Error: missing study program name or official ID.")
		fmt.Fprintln(os.Stderr, "Usage: scraper show-curriculum [--degree <deg>] \"<name|id>\" (e.g. scraper show-curriculum \"Informatik\")")
		os.Exit(1)
	}

	store, _, _ := setupApp(*dbPath, "")
	defer store.Close()

	programs, err := store.ListOfficialPrograms(progNameOrID, *degreeFilter)
	if err != nil || len(programs) == 0 {
		p, errFind := store.GetOfficialProgram(progNameOrID)
		if errFind == nil && p != nil {
			programs = []model.OfficialStudyProgram{*p}
		}
	}

	if len(programs) == 0 {
		fmt.Printf("No official study program found for %q.\n", progNameOrID)
		return
	}

	type progCurriculumView struct {
		Program    model.OfficialStudyProgram `json:"program"`
		Curriculum []model.CurriculumModule   `json:"curriculum"`
	}

	var allViews []progCurriculumView

	for _, p := range programs {
		cur, err := store.GetProgramCurriculum(p.ID)
		if err != nil || len(cur) == 0 {
			continue
		}
		allViews = append(allViews, progCurriculumView{
			Program:    p,
			Curriculum: cur,
		})
	}

	if len(allViews) == 0 {
		fmt.Printf("No extracted curriculum found for %q.\n", progNameOrID)
		fmt.Println("Hint: Run 'scraper scan-curriculum' first to extract curriculum data using AI.")
		return
	}

	if *asJSON {
		data, _ := json.MarshalIndent(allViews, "", "  ")
		fmt.Println(string(data))
		return
	}

	for _, v := range allViews {
		fmt.Println(strings.Repeat("=", 90))
		fmt.Printf("🎓 %s | %s (PO-Version: %s)\n", v.Program.ProgramName, v.Program.Degree, v.Program.POVersion)
		fmt.Printf("   Official ID: %s\n", v.Program.ID)
		fmt.Println(strings.Repeat("-", 90))

		bySemester := make(map[int][]model.CurriculumModule)
		var semesters []int
		for _, m := range v.Curriculum {
			sem := m.RecommendedSemester
			if len(bySemester[sem]) == 0 {
				semesters = append(semesters, sem)
			}
			bySemester[sem] = append(bySemester[sem], m)
		}
		sort.Ints(semesters)

		for _, sem := range semesters {
			semHeader := fmt.Sprintf("SEMESTER %d", sem)
			if sem == 0 {
				semHeader = "SEMESTERSPANNEN / OHNE FESTES SEMESTER"
			}
			fmt.Printf("\n📚 %s:\n", semHeader)
			fmt.Printf("   %-8s | %-12s | %-45s | %-6s | %s\n", "Code/ID", "Art", "Modulbezeichnung", "ECTS", "Status / Verknüpfung")
			fmt.Printf("   %s\n", strings.Repeat("-", 85))

			semECTS := 0.0
			for _, m := range bySemester[sem] {
				semECTS += m.Credits
				codeStr := m.ModuleCode
				if codeStr == "" {
					codeStr = "-"
				}
				nameStr := m.ModuleName
				if len(nameStr) > 43 {
					nameStr = nameStr[:40] + "..."
				}
				linkTag := "nicht verknüpft"
				if m.ModuleID != "" {
					linkTag = fmt.Sprintf("✓ Modul %s", m.ModuleID)
				}
				creditText := fmt.Sprintf("%.1f", m.Credits)
				if m.MaxCredits > 0 {
					creditText = fmt.Sprintf("%.1f–%.1f", m.MinCredits, m.MaxCredits)
				}
				if m.SemesterSpan != "" {
					linkTag += " | Semester " + m.SemesterSpan
				}
				fmt.Printf("   %-8s | %-12s | %-45s | %s LP | %s\n",
					codeStr, m.ModuleType, nameStr, creditText, linkTag)
			}
			if sem > 0 {
				fmt.Printf("   -> Semester-Summe: %.1f LP\n", semECTS)
			}
		}
		fmt.Println()
	}
}

func runProgramModules(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("program-modules", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	asJSON := fs.Bool("json", false, "Output as raw JSON")
	_ = fs.Parse(reorderFlags(args))

	progNameOrID := fs.Arg(0)
	if progNameOrID == "" {
		fmt.Fprintln(os.Stderr, "Error: missing study program name or official ID.")
		fmt.Fprintln(os.Stderr, "Usage: scraper program-modules \"<name|id>\" (e.g. scraper program-modules \"Informatik\")")
		os.Exit(1)
	}

	store, _, _ := setupApp(*dbPath, "")
	defer store.Close()

	modules, err := store.GetModulesForStudyProgram(progNameOrID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error retrieving modules for study program %q: %v\n", progNameOrID, err)
		os.Exit(1)
	}

	if *asJSON {
		data, _ := json.MarshalIndent(modules, "", "  ")
		fmt.Println(string(data))
		return
	}

	fmt.Printf("Modules associated with official study program: %q\n", progNameOrID)
	fmt.Printf("Total modules found: %d\n\n", len(modules))

	if len(modules) == 0 {
		fmt.Printf("No modules found linked to %q.\n", progNameOrID)
		fmt.Println("Tip: Ensure module details have been scraped (e.g. 'scraper detail <id>' or 'scraper all') so study programs are resolved and linked.")
		return
	}

	fmt.Printf("%-8s | %-55s | %s\n", "ID", "Title", "URL")
	fmt.Println(strings.Repeat("-", 90))
	for _, m := range modules {
		title := m.Title
		if len(title) > 52 {
			title = title[:49] + "..."
		}
		fmt.Printf("%-8s | %-55s | %s\n", m.ID, title, m.URL)
	}
}

func runServe(ctx context.Context, args []string) {
	// 1. Detect if a custom --config was specified in args
	configArg := ""
	for i, a := range args {
		if (a == "--config" || a == "-config") && i+1 < len(args) {
			configArg = args[i+1]
		} else if strings.HasPrefix(a, "--config=") || strings.HasPrefix(a, "-config=") {
			parts := strings.SplitN(a, "=", 2)
			if len(parts) == 2 {
				configArg = parts[1]
			}
		}
	}

	// 2. Load configuration: Config file > Environment variables > Defaults
	cfg, cfgPath, err := config.Load(configArg)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to load config (%s): %v. Using defaults.\n", configArg, err)
	} else if cfgPath != "" {
		fmt.Printf("[+] Configuration loaded from: %s\n", cfgPath)
	}

	fs := flag.NewFlagSet("serve", flag.ExitOnError)
	configFlag := fs.String("config", configArg, "Configuration file path (YAML or JSON)")
	dbPath := fs.String("db", cfg.Storage.DBPath, "SQLite database path")
	analyticsDB := fs.String("analytics-db", cfg.Storage.AnalyticsDBPath, "Analytics SQLite database path")
	cacheDir := fs.String("cache-dir", cfg.Storage.CacheDir, "Cache directory")
	logFile := fs.String("log-file", cfg.Logging.LogFile, "Log file path")
	port := fs.String("port", cfg.Server.Port, "HTTP port to listen on (default 8080)")
	staticDir := fs.String("static-dir", "frontend/dist", "Directory containing the frontend static assets (SPA)")
	autoRefresh := fs.Bool("auto-refresh", cfg.Refresher.AutoRefresh, "Enable background polite scheduled data refreshing")
	offpeakStart := fs.Int("offpeak-start", cfg.Refresher.OffPeakStartHour, "Off-peak scraping window start hour (24h)")
	offpeakEnd := fs.Int("offpeak-end", cfg.Refresher.OffPeakEndHour, "Off-peak scraping window end hour (24h)")
	_ = fs.Parse(reorderFlags(args))
	_ = configFlag

	// 3. Initialize logger
	minLvl := logger.LevelInfo
	switch strings.ToUpper(cfg.Logging.MinLevel) {
	case "DEBUG":
		minLvl = logger.LevelDebug
	case "WARN":
		minLvl = logger.LevelWarn
	case "ERROR":
		minLvl = logger.LevelError
	}

	sysLog, err := logger.NewLogger(logger.Options{
		MinLevel:   minLvl,
		FilePath:   *logFile,
		BufferSize: cfg.Logging.BufferSize,
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to initialize log file (%s): %v\n", *logFile, err)
		sysLog = logger.Default()
	}
	defer sysLog.Close()

	// 4. Initialize app and main SQLite storage
	store, _, reg := setupApp(*dbPath, *cacheDir)
	defer store.Close()

	// 5. Initialize separate anonymous analytics DB (GDPR-compliant)
	tracker, err := analytics.NewTracker(*analyticsDB)
	if err != nil {
		sysLog.Warn("ANALYTICS", "Failed to initialize analytics DB (%s): %v", *analyticsDB, err)
	} else {
		defer tracker.Close()
	}

	var eventProv *provider.BTUEventProvider
	if p, ok := reg.Get(provider.EventProviderName); ok {
		eventProv = p.(*provider.BTUEventProvider)
	}

	var catProv *provider.BTUModuleCatalogProvider
	if p, ok := reg.Get(provider.CatalogProviderName); ok {
		catProv = p.(*provider.BTUModuleCatalogProvider)
	}

	var detProv *provider.BTUModuleDetailProvider
	if p, ok := reg.Get(provider.DetailProviderName); ok {
		detProv = p.(*provider.BTUModuleDetailProvider)
	}

	// 6. Initialize polite refresher service worker (active by default)
	var ref *refresher.Refresher
	if *autoRefresh {
		refCfg := refresher.DefaultConfig()
		refCfg.OffPeakStartHour = *offpeakStart
		refCfg.OffPeakEndHour = *offpeakEnd
		refCfg.ModuleDetailDelay = cfg.Refresher.ModuleDelayDuration()
		refCfg.QISDelay = cfg.Refresher.QISDelayDuration()
		refCfg.CatalogInterval = cfg.Refresher.CatalogIntervalDuration()

		ref = refresher.NewRefresher(refCfg, store, catProv, detProv, eventProv, sysLog)
		go ref.Start(ctx)
	}

	// 7. Initialize web server
	var webOpts []web.ServerOption
	webOpts = append(webOpts, web.WithTracker(tracker), web.WithRefresher(ref), web.WithLogger(sysLog))
	if *staticDir != "" {
		if fi, err := os.Stat(*staticDir); err == nil && fi.IsDir() {
			webOpts = append(webOpts, web.WithStaticDir(*staticDir))
		}
	}

	srv, err := web.NewServer(
		store,
		eventProv,
		webOpts...,
	)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error initializing web server: %v\n", err)
		os.Exit(1)
	}

	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = srv.Shutdown(shutdownCtx)
	}()

	if err := srv.Start(*port); err != nil && !errors.Is(err, http.ErrServerClosed) {
		fmt.Fprintf(os.Stderr, "Web server error: %v\n", err)
		os.Exit(1)
	}
}

func setupApp(dbPath, cacheDir string) (*storage.Storage, cache.Cache, *provider.Registry) {
	if dbPath == "" {
		dbPath = "btu_modules.db"
	}
	if cacheDir == "" {
		cacheDir = ".cache"
	}

	store, err := storage.NewStorage(dbPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error initializing SQLite database (%s): %v\n", dbPath, err)
		os.Exit(1)
	}

	c, err := cache.NewDiskCache(cacheDir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to initialize disk cache: %v\n", err)
	}

	reg := provider.NewRegistry()
	catProvider := provider.NewBTUModuleCatalogProvider(store, c, "", provider.DefaultCatalogTTL)
	detailProvider := provider.NewBTUModuleDetailProvider(store, c, "", provider.DefaultDetailTTL)
	eventProvider := provider.NewBTUEventProvider(store, c, provider.DefaultEventTTL)
	fuesProvider := provider.NewBTUFUESProvider(store, c, "", provider.DefaultFUESTTL)
	cfg, _, _ := config.Load("")
	statutesDir := cfg.Storage.StatutesDir
	if statutesDir == "" {
		statutesDir = "statutes"
	}
	progProvider := provider.NewBTUProgramTreeProvider(store, c, "", provider.DefaultProgramTreeTTL, statutesDir)

	// Connect program tree provider so module scraping can resolve unindexed programs on demand
	detailProvider.SetProgramTreeProvider(progProvider)

	_ = reg.Register(catProvider)
	_ = reg.Register(detailProvider)
	_ = reg.Register(eventProvider)
	_ = reg.Register(fuesProvider)
	_ = reg.Register(progProvider)

	return store, c, reg
}

func printModuleCard(d *model.ModuleDetail) {
	fmt.Println(strings.Repeat("=", 80))
	phaseOutTag := ""
	if d.IsNotOffered {
		phaseOutTag = " [NICHT MEHR IM ANGEBOT]"
	} else if d.IsPhaseOut {
		phaseOutTag = " [PHASE-OUT]"
	}
	fmt.Printf("MODULE %s: %s%s\n", d.ID, d.TitleDE, phaseOutTag)
	if d.TitleEN != "" && d.TitleEN != d.TitleDE {
		fmt.Printf("English: %s\n", d.TitleEN)
	}
	fmt.Println(strings.Repeat("-", 80))

	if d.Department != "" {
		fmt.Printf("Department:      %s\n", d.Department)
	}
	if len(d.ResponsiblePersons) > 0 {
		var respNames []string
		for _, rp := range d.ResponsiblePersons {
			respNames = append(respNames, rp.FullName())
		}
		fmt.Printf("Responsible:     %s\n", strings.Join(respNames, ", "))
	}
	if len(d.SuccessorModules) > 0 {
		fmt.Printf("Successor:       %s\n", strings.Join(d.SuccessorModules, ", "))
	}
	if d.Language != "" {
		fmt.Printf("Language:        %s\n", d.Language)
	}
	if d.Credits > 0 || d.CreditsRaw != "" {
		fmt.Printf("Credits:         %.1f ECTS (%s)\n", d.Credits, d.CreditsRaw)
	}
	if d.Duration != "" {
		fmt.Printf("Duration:        %s\n", d.Duration)
	}
	if d.Turnus != "" {
		fmt.Printf("Turnus:          %s\n", d.Turnus)
	}
	if d.ExamType != "" {
		fmt.Printf("Exam Type:       %s\n", d.ExamType)
	}
	if d.Grading != "" {
		fmt.Printf("Grading:         %s\n", d.Grading)
	}
	if d.CrossDisciplinary {
		fmt.Printf("Cross-Discip.:   Yes (Fachübergreifendes Studium)\n")
	}

	if len(d.TeachingForms) > 0 {
		fmt.Println("\nTeaching Forms:")
		for _, tf := range d.TeachingForms {
			if tf.Workload != "" {
				fmt.Printf("  • %s (%s)\n", tf.Type, tf.Workload)
			} else {
				fmt.Printf("  • %s\n", tf.Type)
			}
		}
	}

	if d.LearningOutcomes != "" {
		fmt.Printf("\nLearning Outcomes:\n%s\n", indent(d.LearningOutcomes, "  "))
	}

	if d.Contents != "" {
		fmt.Printf("\nContents:\n%s\n", indent(d.Contents, "  "))
	}

	if d.PrerequisitesRecommended != "" {
		fmt.Printf("\nRecommended Prerequisites:\n%s\n", indent(d.PrerequisitesRecommended, "  "))
	}
	if d.PrerequisitesMandatory != "" {
		fmt.Printf("\nMandatory Prerequisites:\n%s\n", indent(d.PrerequisitesMandatory, "  "))
	}

	if len(d.StudyPrograms) > 0 {
		fmt.Printf("\nAssociated Study Programs (%d):\n", len(d.StudyPrograms))
		for _, sp := range d.StudyPrograms {
			officialTag := ""
			if sp.OfficialProgramID != "" {
				officialTag = fmt.Sprintf(" [Official ID: %s]", sp.OfficialProgramID)
			}
			if sp.Degree != "" && sp.Program != "" {
				reg := sp.Regulation
				if reg == "" {
					reg = "no PO"
				}
				fmt.Printf("  • %s | %s | %s%s\n", sp.Degree, sp.Program, reg, officialTag)
			} else {
				fmt.Printf("  • %s%s\n", sp.Raw, officialTag)
			}
		}
	}

	if len(d.CurrentSemesterEvents) > 0 {
		fmt.Printf("\nCurrent Semester Events (%d):\n", len(d.CurrentSemesterEvents))
		for _, evt := range d.CurrentSemesterEvents {
			if evt.URL != "" {
				fmt.Printf("  • %s\n    %s\n", evt.Title, evt.URL)
			} else {
				fmt.Printf("  • %s\n", evt.Title)
			}
		}
	}

	if d.RawURL != "" {
		fmt.Printf("\nURL: %s\n", d.RawURL)
	}
	fmt.Println(strings.Repeat("=", 80))
}

func indent(text, prefix string) string {
	lines := strings.Split(text, "\n")
	var result []string
	for _, l := range lines {
		if strings.TrimSpace(l) != "" {
			result = append(result, prefix+l)
		}
	}
	return strings.Join(result, "\n")
}

// reorderFlags places options starting with '-' before positional arguments
// so standard flag.FlagSet parses them regardless of their command-line position.
func reorderFlags(args []string) []string {
	var flags []string
	var nonFlags []string
	for i := 0; i < len(args); i++ {
		arg := args[i]
		if strings.HasPrefix(arg, "-") {
			flags = append(flags, arg)
			if !strings.Contains(arg, "=") && i+1 < len(args) && !strings.HasPrefix(args[i+1], "-") {
				flagName := strings.TrimLeft(arg, "-")
				switch flagName {
				case "db", "cache-dir", "config", "search", "dept", "min-credits", "max-credits", "limit", "offset", "workers", "delay", "name", "degree", "port", "analytics-db", "log-file", "offpeak-start", "offpeak-end", "out-dir", "api-key", "model", "static-dir", "start-term", "credit-tolerance", "report-dir", "program-id", "pdf", "plan-pages":
					i++
					flags = append(flags, args[i])
				}
			}
		} else {
			nonFlags = append(nonFlags, arg)
		}
	}
	return append(flags, nonFlags...)
}

func catalogModuleID(m gemini.ExtractedModule, catalog []model.CurriculumCatalogModule) string {
	if matched := gemini.MatchCatalogModule(m, catalog); matched != nil {
		return matched.ID
	}
	return ""
}
