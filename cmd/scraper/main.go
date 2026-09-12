package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/jakob/btu-scraper/internal/analytics"
	"github.com/jakob/btu-scraper/internal/cache"
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
	_ = fs.Parse(reorderFlags(args))

	moduleID := fs.Arg(0)
	if moduleID == "" {
		fmt.Fprintln(os.Stderr, "Error: missing module ID. Usage: scraper module-events <module-id>")
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
	fs := flag.NewFlagSet("serve", flag.ExitOnError)
	dbPath := fs.String("db", "btu_modules.db", "SQLite database path")
	analyticsDB := fs.String("analytics-db", "btu_analytics.db", "Analytics SQLite database path")
	logFile := fs.String("log-file", "btu_scraper.log", "Log file path")
	port := fs.String("port", "8080", "HTTP port to listen on (default 8080)")
	autoRefresh := fs.Bool("auto-refresh", true, "Enable background polite scheduled data refreshing")
	offpeakStart := fs.Int("offpeak-start", 1, "Off-peak scraping window start hour (24h)")
	offpeakEnd := fs.Int("offpeak-end", 6, "Off-peak scraping window end hour (24h)")
	_ = fs.Parse(reorderFlags(args))

	// 1. Initialize logger
	sysLog, err := logger.NewLogger(logger.Options{
		MinLevel:   logger.LevelInfo,
		FilePath:   *logFile,
		BufferSize: 300,
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to initialize log file (%s): %v\n", *logFile, err)
		sysLog = logger.Default()
	}
	defer sysLog.Close()

	// 2. Initialize app and main SQLite storage
	store, _, reg := setupApp(*dbPath, ".cache")
	defer store.Close()

	// 3. Initialize separate anonymous analytics DB (GDPR-compliant)
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

	// 4. Initialize polite refresher service
	var ref *refresher.Refresher
	if *autoRefresh {
		refCfg := refresher.DefaultConfig()
		refCfg.OffPeakStartHour = *offpeakStart
		refCfg.OffPeakEndHour = *offpeakEnd
		ref = refresher.NewRefresher(refCfg, store, catProv, detProv, eventProv, sysLog)
		go ref.Start(ctx)
	}

	// 5. Initialize web server
	srv, err := web.NewServer(
		store,
		eventProv,
		web.WithTracker(tracker),
		web.WithRefresher(ref),
		web.WithLogger(sysLog),
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
	progProvider := provider.NewBTUProgramTreeProvider(store, c, "", provider.DefaultProgramTreeTTL, "statutes")

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
	if d.IsPhaseOut {
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
		fmt.Printf("Responsible:     %s\n", strings.Join(d.ResponsiblePersons, ", "))
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
				case "db", "cache-dir", "search", "dept", "min-credits", "max-credits", "limit", "offset", "workers", "delay", "name", "degree", "port", "analytics-db", "log-file", "offpeak-start", "offpeak-end":
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
