package main

import (
	"context"
	"flag"
	"fmt"
	"html"
	"os"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/history"
	"github.com/leonieziechmann/betula/radix/internal/privacy"
)

// openExistingDB opens radix.db, and never makes one: a fresh database has an empty register, and
// a history recorded or redacted with it would let back what the real one keeps out.
func openExistingDB(path string) *catalogdb.DB {
	if _, err := os.Stat(path); err != nil {
		fmt.Fprintf(os.Stderr, "Error: no database at %s (--db, env RADIX_DB: the radix.db whose privacy register applies)\n", path)
		os.Exit(1)
	}
	return openDB(path)
}

// multiFlag is a flag that may be given more than once.
type multiFlag []string

func (m *multiFlag) String() string     { return strings.Join(*m, ", ") }
func (m *multiFlag) Set(v string) error { *m = append(*m, v); return nil }

// runPrivacy manages the privacy register (docs/radix/privacy.md): the people who asked that
// their personal data be taken out of Betula.
func runPrivacy(ctx context.Context, args []string) {
	usage := func() {
		fmt.Fprintln(os.Stderr, `Usage:
  radix privacy add --surname S --given G [--form F]... [--note N]
                                 put a person into the register; the next build takes them out of the
                                 catalog, its export out of the snapshot, the history out of every version
  radix privacy list             the register
  radix privacy remove --id N    take a person out of the register (what the build makes again comes back with it)
  radix privacy find --surname S --given G [--form F]... [--history FILE] [--raw] [--limit N]
                                 where a person is named: the catalog, the history, the raw archive
                                 (a request for information, Art. 15 DSGVO); changes nothing

Every subcommand takes --db (default `+defaultDBPath+`, env RADIX_DB). --form adds a further form of the
name the patterns do not derive (an e-mail address, a maiden name); given name and surname are needed,
so that a namesake keeps their data.`)
		os.Exit(2)
	}
	if len(args) == 0 {
		usage()
	}
	fs := flag.NewFlagSet("privacy "+args[0], flag.ExitOnError)
	dbPath := fs.String("db", envOr("RADIX_DB", defaultDBPath), "Database path (env RADIX_DB)")
	surname := fs.String("surname", "", "Surname")
	given := fs.String("given", "", "Given name")
	var forms multiFlag
	fs.Var(&forms, "form", "A further form of the name (repeatable)")
	note := fs.String("note", "", "Where the request came from: a reference, never contact data")
	id := fs.Int64("id", 0, "Register id (privacy list)")
	historyPath := fs.String("history", envOr("RADIX_HISTORY_DB", ""), "History to search as well (env RADIX_HISTORY_DB)")
	raw := fs.Bool("raw", false, "Search the raw archive as well (every archived page; takes a while)")
	limit := fs.Int("limit", 50, "Places to print at most")

	switch args[0] {
	case "add":
		_ = fs.Parse(args[1:])
		db := openExistingDB(*dbPath)
		defer db.Close()
		p := privacy.Person{Surname: *surname, Given: *given, Forms: forms, Note: *note}
		newID, err := privacy.Add(ctx, db.SQL(), p, time.Now())
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error:", err)
			os.Exit(1)
		}
		p.ID = newID
		fmt.Printf("Added %d: %s\n", newID, p.Name())
		tables, err := privacy.CatalogTables(ctx, db.SQL())
		if err == nil {
			_, n, ferr := privacy.Find(ctx, db.SQL(), tables, []privacy.Person{p}, 0)
			err = ferr
			if err == nil {
				fmt.Printf("The catalog names them in %d places; the next build takes them out (the service's next cycle, at most its --interval).\n", n)
			}
		}
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error:", err)
			os.Exit(1)
		}
	case "list":
		_ = fs.Parse(args[1:])
		db := openExistingDB(*dbPath)
		defer db.Close()
		people, err := privacy.Load(ctx, db.SQL())
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error:", err)
			os.Exit(1)
		}
		if len(people) == 0 {
			fmt.Println("The register is empty.")
		}
		for _, p := range people {
			fmt.Printf("%4d  %-36s added %s", p.ID, p.Name(), p.AddedAt)
			if len(p.Forms) > 0 {
				fmt.Printf("  forms: %s", strings.Join(p.Forms, "; "))
			}
			if p.Note != "" {
				fmt.Printf("  note: %s", p.Note)
			}
			fmt.Println()
		}
		if len(people) > 0 {
			fmt.Printf("Digest %s\n", privacy.Digest(people))
		}
	case "remove":
		_ = fs.Parse(args[1:])
		if *id <= 0 {
			usage()
		}
		db := openExistingDB(*dbPath)
		defer db.Close()
		ok, err := privacy.Remove(ctx, db.SQL(), *id)
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error:", err)
			os.Exit(1)
		}
		if !ok {
			fmt.Fprintf(os.Stderr, "No person %d in the register.\n", *id)
			os.Exit(1)
		}
		fmt.Printf("Removed %d. The next build has their data again where it makes it from the archive; the study plans and the semantic search's summaries keep [entfernt], and what the history lost does not come back.\n", *id)
	case "find":
		_ = fs.Parse(args[1:])
		p := privacy.Person{Surname: *surname, Given: *given, Forms: forms}
		if err := p.Validate(); err != nil {
			fmt.Fprintln(os.Stderr, "Error:", err)
			os.Exit(2)
		}
		db := openExistingDB(*dbPath)
		defer db.Close()
		people := []privacy.Person{p}
		tables, err := privacy.CatalogTables(ctx, db.SQL())
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error:", err)
			os.Exit(1)
		}
		hits, n, err := privacy.Find(ctx, db.SQL(), tables, people, *limit)
		if err != nil {
			fmt.Fprintln(os.Stderr, "Error:", err)
			os.Exit(1)
		}
		fmt.Printf("Catalog (%s): %d places\n", *dbPath, n)
		printHits(hits)
		if *historyPath != "" {
			h, err := history.Open(*historyPath)
			if err != nil {
				fmt.Fprintln(os.Stderr, "Error:", err)
				os.Exit(1)
			}
			hits, n, err := h.Find(ctx, people, *limit)
			h.Close()
			if err != nil {
				fmt.Fprintln(os.Stderr, "Error:", err)
				os.Exit(1)
			}
			fmt.Printf("\nHistory (%s): %d places, all versions\n", *historyPath, n)
			printHits(hits)
		}
		if *raw {
			pages, places, err := findRaw(ctx, db, people, *limit)
			if err != nil {
				fmt.Fprintln(os.Stderr, "Error:", err)
				os.Exit(1)
			}
			fmt.Printf("\nRaw archive: %d places on %d pages (copies of the university's pages, read again by the crawl; never published)\n", places, len(pages))
			for _, pg := range pages {
				fmt.Println("  " + pg)
			}
		}
	default:
		usage()
	}
}

func printHits(hits []privacy.Hit) {
	for _, h := range hits {
		what := h.Column
		if h.Row {
			what = "row"
		}
		fmt.Printf("  %s (%s) %s: %s\n", h.Table, h.Key, what, h.Snippet)
	}
}

var tags = regexp.MustCompile(`(?s)<script.*?</script>|<style.*?</style>|<[^>]*>`)

// findRaw counts the places of the raw archive that name the people, by page.
func findRaw(ctx context.Context, db *catalogdb.DB, people []privacy.Person, limit int) ([]string, int, error) {
	rows, err := db.SQL().QueryContext(ctx, "SELECT DISTINCT source FROM raw_page ORDER BY 1")
	if err != nil {
		return nil, 0, err
	}
	var sources []string
	for rows.Next() {
		var s string
		if err := rows.Scan(&s); err != nil {
			rows.Close()
			return nil, 0, err
		}
		sources = append(sources, s)
	}
	rows.Close()
	var pages []string
	places := 0
	for _, source := range sources {
		err := db.EachPage(source, func(p *catalogdb.RawPage) error {
			if err := ctx.Err(); err != nil {
				return err
			}
			if len(p.Body) == 0 || strings.HasPrefix(string(p.Body[:min(len(p.Body), 5)]), "%PDF") {
				return nil
			}
			n := privacy.Mentions(html.UnescapeString(tags.ReplaceAllString(string(p.Body), " ")), people)
			if n > 0 {
				places += n
				if limit <= 0 || len(pages) < limit {
					pages = append(pages, fmt.Sprintf("%s %s: %d", p.Source, p.URL, n))
				}
			}
			return nil
		})
		if err != nil {
			return nil, 0, err
		}
	}
	sort.Strings(pages)
	return pages, places, nil
}

// runHistory works with the history of the published catalog (docs/radix/history.md).
func runHistory(ctx context.Context, args []string) {
	usage := func() {
		fmt.Fprintln(os.Stderr, `Usage:
  radix history status                         what the history holds
  radix history record --snapshot FILE|DIR     add a snapshot as the newest state (a DIR: its current.json);
                       [--valid-from TIME]     the privacy register of --db is applied first
  radix history at --time TIME --out FILE      the catalog as published at TIME, as a snapshot
  radix history log TABLE [COLUMN=VALUE]...    the versions of rows: radix history log module id=11881
  radix history export --out FILE              a copy of the whole history (the full export)
  radix history redact                         apply the privacy register of --db to every version

Every subcommand takes --history (env RADIX_HISTORY_DB) and --db (default `+defaultDBPath+`, env RADIX_DB), the
database whose privacy register applies. TIME is RFC 3339 (2026-11-19T12:00:00Z) or a date (2026-11-19, its end).`)
		os.Exit(2)
	}
	if len(args) == 0 {
		usage()
	}
	fs := flag.NewFlagSet("history "+args[0], flag.ExitOnError)
	historyPath := fs.String("history", envOr("RADIX_HISTORY_DB", "history.db"), "History (env RADIX_HISTORY_DB)")
	dbPath := fs.String("db", envOr("RADIX_DB", defaultDBPath), "Database whose privacy register applies (env RADIX_DB)")
	noRegister := fs.Bool("no-register", false, "record: there is no radix.db at hand (refused once a register was applied)")
	snapshot := fs.String("snapshot", "", "record: the snapshot, or a snapshot directory")
	validFrom := fs.String("valid-from", "", "record: since when the snapshot was published (default: its meta data_changed_at)")
	at := fs.String("time", "", "at: the time")
	out := fs.String("out", "", "at, export: the file to write")
	_ = fs.Parse(args[1:])

	fail := func(err error) {
		fmt.Fprintln(os.Stderr, "Error:", err)
		os.Exit(1)
	}
	register := func() ([]privacy.Person, string) {
		db := openExistingDB(*dbPath)
		defer db.Close()
		people, err := privacy.Load(ctx, db.SQL())
		if err != nil {
			fail(err)
		}
		return people, privacy.RegisterID(ctx, db.SQL())
	}
	h, err := history.Open(*historyPath)
	if err != nil {
		fail(err)
	}
	defer h.Close()

	switch args[0] {
	case "status":
		st, err := h.Status(ctx)
		if err != nil {
			fail(err)
		}
		fmt.Printf("History %s: %.1f MB\n", *historyPath, float64(st.Bytes)/1e6)
		fmt.Printf("States        %d", st.Snapshots)
		if st.First != nil {
			fmt.Printf(", from %s to %s", st.First.ValidFrom.Format(time.RFC3339), st.Last.ValidFrom.Format(time.RFC3339))
		}
		fmt.Printf("\nTables        %d\nRow versions  %d, %d of them in the newest state\n", st.Tables, st.RowVersions, st.CurrentRows)
		if st.PrivacyDigest != "" {
			fmt.Printf("Privacy       register %s applied\n", st.PrivacyDigest)
		}
		snaps, err := h.Snapshots(ctx)
		if err != nil {
			fail(err)
		}
		for _, s := range snaps[max(0, len(snaps)-10):] {
			fmt.Printf("  %5d  %s  schema %d  Radix %-8s  +%d −%d rows\n", s.Seq, s.ValidFrom.Format(time.RFC3339), s.SchemaVersion, s.RadixVersion, s.RowsOpened, s.RowsClosed)
		}
	case "record":
		path := *snapshot
		if path == "" {
			usage()
		}
		opt := history.RecordOptions{NoRegister: *noRegister}
		if info, err := os.Stat(path); err == nil && info.IsDir() {
			pointer, err := catalogdb.ReadSnapshotPointer(path)
			if err != nil {
				fail(err)
			}
			path = path + string(os.PathSeparator) + pointer.File
			opt.Published, _ = time.Parse(time.RFC3339, pointer.ExportedAt)
		}
		if !*noRegister {
			opt.People, opt.RegisterID = register()
		}
		if *validFrom != "" {
			if opt.ValidFrom, err = parseTime(*validFrom); err != nil {
				fail(err)
			}
		}
		s, err := h.Record(ctx, path, opt)
		if err != nil {
			fail(err)
		}
		if s == nil {
			fmt.Println("The newest state holds what this snapshot holds; nothing recorded.")
			return
		}
		fmt.Printf("Recorded state %d, valid from %s: %d rows opened, %d closed (schema %d, Radix %s)\n",
			s.Seq, s.ValidFrom.Format(time.RFC3339), s.RowsOpened, s.RowsClosed, s.SchemaVersion, s.RadixVersion)
	case "at":
		if *at == "" || *out == "" {
			usage()
		}
		t, err := parseTime(*at)
		if err != nil {
			fail(err)
		}
		start := time.Now()
		s, err := h.At(ctx, t, *out)
		if err != nil {
			fail(err)
		}
		fmt.Printf("Wrote %s: state %d, published from %s (schema %d), in %s\n", *out, s.Seq, s.ValidFrom.Format(time.RFC3339), s.SchemaVersion, time.Since(start).Round(time.Millisecond))
	case "log":
		rest := fs.Args()
		if len(rest) == 0 {
			usage()
		}
		filter := map[string]string{}
		for _, kv := range rest[1:] {
			k, v, ok := strings.Cut(kv, "=")
			if !ok {
				usage()
			}
			filter[k] = v
		}
		versions, err := h.Log(ctx, rest[0], filter)
		if err != nil {
			fail(err)
		}
		fmt.Printf("%s %s: %d versions\n", rest[0], strings.Join(rest[1:], " "), len(versions))
		for _, v := range versions {
			to := "now"
			if !v.To.IsZero() {
				to = v.To.Format(time.RFC3339)
			}
			fmt.Printf("\n%s – %s  %s", v.From.Format(time.RFC3339), to, v.Key)
			if v.Gone {
				fmt.Print("  (then gone)")
			}
			fmt.Println()
			cols := v.Changed
			if cols == nil {
				for c := range v.Values {
					cols = append(cols, c)
				}
				sort.Strings(cols)
			}
			for _, c := range cols {
				fmt.Printf("  %s: %s\n", c, shorten(fmt.Sprint(v.Values[c]), 160))
			}
		}
	case "export":
		if *out == "" {
			usage()
		}
		if err := h.Export(ctx, *out); err != nil {
			fail(err)
		}
		fmt.Printf("Wrote %s\n", *out)
	case "redact":
		people, registerID := register()
		res, err := h.Redact(ctx, people, registerID)
		if err != nil {
			fail(err)
		}
		fmt.Printf("Applied the register (%d people): %d rows of the lists of names and %d texts taken out\n", res.People, res.Rows, res.Values)
	default:
		usage()
	}
}

// parseTime reads RFC 3339, or a date, which means its end (the state the day ended with).
func parseTime(s string) (time.Time, error) {
	if t, err := time.Parse(time.RFC3339, s); err == nil {
		return t, nil
	}
	if d, err := time.ParseInLocation("2006-01-02", s, time.UTC); err == nil {
		return d.Add(24*time.Hour - time.Second), nil
	}
	return time.Time{}, fmt.Errorf("%q is no time: RFC 3339 (2026-11-19T12:00:00Z) or a date (2026-11-19)", s)
}

func shorten(s string, n int) string {
	s = strings.Join(strings.Fields(s), " ")
	if r := []rune(s); len(r) > n {
		return string(r[:n]) + "…"
	}
	return s
}
