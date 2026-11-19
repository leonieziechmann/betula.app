package privacy

import (
	"context"
	"database/sql"
	"strings"
	"testing"
	"time"

	_ "modernc.org/sqlite"
)

// The persons of these tests are made up; no entry of a real register belongs in the repository.
var maxM = Person{ID: 1, Surname: "Mustermann", Given: "Max", Forms: []string{"maxi@example.org"}}

func redactOne(t *testing.T, p Person, s string, bare bool) string {
	t.Helper()
	ps := newPass([]Person{p})
	out, _ := ps.redact(s, []bool{bare}, make([]int, 1))
	return out
}

func TestFoldKeepsTheOffsets(t *testing.T) {
	for in, want := range map[string]string{"Müller": "muller", "MÜLLER": "muller", "Straße": "strasse", "Mu\u0308ller": "muller", "a\u00a0b": "a b"} {
		got, at := fold(in)
		if got != want {
			t.Errorf("fold(%q) = %q, want %q", in, got, want)
		}
		if len(at) != len(got)+1 || at[len(got)] != len(in) {
			t.Errorf("fold(%q): offsets %v do not end at %d", in, at, len(in))
		}
	}
	s := "Prof. Dr. Jürgen Groß!"
	f, at := fold(s)
	i := strings.Index(f, "jurgen gross")
	if got := s[at[i]:at[i+len("jurgen gross")]]; got != "Jürgen Groß" {
		t.Fatalf("a match of the folded text maps back to %q", got)
	}
}

func TestTheFormsOfAName(t *testing.T) {
	cases := []struct{ in, want string }{
		{"Bei Interesse melden Sie sich bei Prof. Dr. Max Mustermann.", "Bei Interesse melden Sie sich bei [entfernt]."},
		{"Ansprechpartner: Herr Mustermann (max.mustermann@b-tu.de)", "Ansprechpartner: [entfernt] ([entfernt])"},
		{"M. Mustermann: Lehrbuch der Informatik, 2019", "[entfernt]: Lehrbuch der Informatik, 2019"},
		{"Mustermann, Max , Prof. Dr. rer. nat.", "[entfernt]"},
		{"dates will be announced by mustermann@b-tu.de", "dates will be announced by [entfernt]"},
		{"Fragen an maxi@example.org.", "Fragen an [entfernt]."},
		{"MAX MUSTERMANN und Erika Musterfrau", "[entfernt] und Erika Musterfrau"},
		{"Max W. Mustermann", "[entfernt]"},
		// Not him: part of another word, another name joined by a hyphen, the surname alone
		// outside his own rows, another given name.
		{"Mustermannstraße 5", "Mustermannstraße 5"},
		{"Prof. Mustermann-Schulz", "Prof. Mustermann-Schulz"},
		{"Nach Mustermann (1990) gilt", "Nach Mustermann (1990) gilt"},
		{"Erika Mustermann", "Erika Mustermann"},
		{"erika.mustermann@b-tu.de", "erika.mustermann@b-tu.de"},
	}
	for _, c := range cases {
		if got := redactOne(t, maxM, c.in, false); got != c.want {
			t.Errorf("redact(%q)\n got %q\nwant %q", c.in, got, c.want)
		}
	}
	if got := redactOne(t, maxM, "Nach Mustermann (1990) gilt", true); got != "Nach [entfernt] (1990) gilt" {
		t.Errorf("in his own rows the surname alone is his: %q", got)
	}
	umlaut := Person{ID: 2, Surname: "Müller-Lüdenscheidt", Given: "Jürgen"}
	for in, want := range map[string]string{
		"Prof. Jürgen Müller-Lüdenscheidt":      "[entfernt]",
		"Mueller-Luedenscheidt, Juergen":        "[entfernt]",
		"J. Müller Lüdenscheidt":                "[entfernt]",
		"juergen.mueller-luedenscheidt@b-tu.de": "[entfernt]",
	} {
		if got := redactOne(t, umlaut, in, false); got != want {
			t.Errorf("redact(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestValidateRefusesASurnameAlone(t *testing.T) {
	if err := (Person{Surname: "Mustermann"}).Validate(); err == nil {
		t.Fatal("a surname alone would take out every namesake")
	}
	if err := maxM.Validate(); err != nil {
		t.Fatal(err)
	}
}

func TestDigest(t *testing.T) {
	a := Person{ID: 1, Surname: "Mustermann", Given: "Max"}
	b := Person{ID: 2, Surname: "Musterfrau", Given: "Erika", Forms: []string{"y", "x"}}
	if Digest(nil) != "" {
		t.Fatal("an empty register has the empty digest")
	}
	if Digest([]Person{a, b}) != Digest([]Person{b, a}) {
		t.Fatal("the digest depends on the order of the register")
	}
	b2 := b
	b2.Forms = []string{"x", "y"}
	if Digest([]Person{a, b}) != Digest([]Person{a, b2}) {
		t.Fatal("the digest depends on the order of the forms")
	}
	if Digest([]Person{a}) == Digest([]Person{a, b}) {
		t.Fatal("another register, the same digest")
	}
}

const miniCatalog = `
CREATE TABLE module (id TEXT PRIMARY KEY, title TEXT NOT NULL, remarks TEXT, source_url TEXT);
CREATE TABLE module_person (module_id TEXT NOT NULL, ord INTEGER NOT NULL, name TEXT NOT NULL, title TEXT, PRIMARY KEY (module_id, ord)) WITHOUT ROWID;
CREATE TABLE event (id TEXT PRIMARY KEY, title TEXT NOT NULL);
CREATE TABLE event_person (event_id TEXT NOT NULL, ord INTEGER NOT NULL, name TEXT NOT NULL, role TEXT, PRIMARY KEY (event_id, ord)) WITHOUT ROWID;
CREATE TABLE event_date (id INTEGER PRIMARY KEY, event_id TEXT NOT NULL, ord INTEGER NOT NULL, instructor TEXT, comment TEXT);
CREATE TABLE raw_page (source TEXT, key TEXT, body BLOB, PRIMARY KEY (source, key));
INSERT INTO module VALUES ('1', 'Informatik I', 'Fragen an Mustermann, Sprechzeit nach Vereinbarung', 'https://example.org/mustermann'),
                          ('2', 'Informatik II', 'Nach Mustermann (1990) gilt', NULL);
INSERT INTO module_person VALUES ('1', 1, 'Mustermann, Max', 'Prof. Dr.'), ('1', 2, 'Musterfrau, Erika', 'Dr.'), ('2', 1, 'Mustermann, Erika', 'Prof.');
INSERT INTO event VALUES ('100', 'Informatik I'), ('200', 'Kolloquium bei Prof. Mustermann');
INSERT INTO event_person VALUES ('100', 1, 'Mustermann, Max , Prof. Dr.', 'verantwort'), ('200', 1, 'Musterfrau, Erika , Dr.', NULL);
INSERT INTO event_date VALUES (1, '100', 1, 'Mustermann', 'online'), (2, '100', 2, 'Mustermann, Musterfrau', NULL), (3, '200', 1, 'Mustermann', 'bei Max Mustermann melden');
`

func openMini(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	db.SetMaxOpenConns(1)
	if _, err := db.Exec(miniCatalog); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	return db
}

func rowsOf(t *testing.T, db *sql.DB, q string) []string {
	t.Helper()
	rows, err := db.Query(q)
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	var out []string
	for rows.Next() {
		var s sql.NullString
		if err := rows.Scan(&s); err != nil {
			t.Fatal(err)
		}
		if s.Valid {
			out = append(out, s.String)
		} else {
			out = append(out, "NULL")
		}
	}
	return out
}

func TestApplyTakesThePersonOutAndOnlyThem(t *testing.T) {
	ctx := context.Background()
	db := openMini(t)
	tables, err := CatalogTables(ctx, db)
	if err != nil {
		t.Fatal(err)
	}
	for _, tb := range tables {
		if tb.Name == "raw_page" {
			t.Fatal("the raw archive is not searched")
		}
		if tb.Name == "module" && strings.Contains(strings.Join(tb.Text, ","), "source_url") {
			t.Fatal("addresses are not searched")
		}
	}
	people := []Person{{ID: 7, Surname: "Mustermann", Given: "Max"}}
	res, err := Apply(ctx, db, tables, people)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string][]string{
		"SELECT name FROM module_person ORDER BY module_id, ord": {"Musterfrau, Erika", "Mustermann, Erika"},
		"SELECT name FROM event_person ORDER BY event_id, ord":   {"Musterfrau, Erika , Dr."},
		"SELECT remarks FROM module ORDER BY id":                 {"Fragen an [entfernt], Sprechzeit nach Vereinbarung", "Nach Mustermann (1990) gilt"},
		"SELECT instructor FROM event_date ORDER BY id":          {"NULL", "[entfernt], Musterfrau", "Mustermann"},
		"SELECT comment FROM event_date ORDER BY id":             {"online", "NULL", "bei [entfernt] melden"},
		"SELECT title FROM event ORDER BY id":                    {"Informatik I", "Kolloquium bei [entfernt]"},
		"SELECT source_url FROM module WHERE id = '1'":           {"https://example.org/mustermann"},
	}
	for q, w := range want {
		if got := rowsOf(t, db, q); strings.Join(got, "|") != strings.Join(w, "|") {
			t.Errorf("%s\n got %q\nwant %q", q, got, w)
		}
	}
	if res.Rows != 2 || res.Modules != 1 || res.Events != 1 || res.ByPerson[7] == 0 {
		t.Errorf("result %+v", res)
	}

	again, err := Apply(ctx, db, tables, people)
	if err != nil {
		t.Fatal(err)
	}
	if again.Rows != 0 || again.Values != 0 {
		t.Fatalf("Apply is not idempotent: %+v", again)
	}
	hits, n, err := Find(ctx, db, tables, people, 10)
	if err != nil {
		t.Fatal(err)
	}
	if n != 0 {
		t.Fatalf("after Apply, Find still finds %d: %+v", n, hits)
	}
}

func TestFindReportsWithoutChanging(t *testing.T) {
	ctx := context.Background()
	db := openMini(t)
	tables, _ := CatalogTables(ctx, db)
	hits, n, err := Find(ctx, db, tables, []Person{{Surname: "Mustermann", Given: "Max"}}, 0)
	if err != nil {
		t.Fatal(err)
	}
	if n != len(hits) || n < 6 {
		t.Fatalf("found %d: %+v", n, hits)
	}
	rows := 0
	for _, h := range hits {
		if h.Row {
			rows++
		}
	}
	if rows != 2 {
		t.Errorf("the two rows of the lists of names: %d", rows)
	}
	if got := rowsOf(t, db, "SELECT COUNT(*) FROM module_person"); got[0] != "3" {
		t.Fatal("Find changed the data")
	}
}

func TestTheRegister(t *testing.T) {
	ctx := context.Background()
	db := openMini(t)
	if _, err := Load(ctx, db); err != ErrNoRegister {
		t.Fatalf("a database without the table: %v", err)
	}
	if _, err := db.Exec(`CREATE TABLE privacy_person (id INTEGER PRIMARY KEY, surname TEXT NOT NULL, given TEXT NOT NULL, forms TEXT, note TEXT, added_at TEXT NOT NULL);
		CREATE TABLE privacy_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;`); err != nil {
		t.Fatal(err)
	}
	if _, err := Add(ctx, db, Person{Surname: "Mustermann"}, time.Now()); err == nil {
		t.Fatal("a surname alone was taken")
	}
	id, err := Add(ctx, db, Person{Surname: " Mustermann ", Given: "Max", Forms: []string{"maxi@example.org", " "}, Note: "Mail vom 2026-11-19"}, time.Date(2026, 11, 19, 12, 0, 0, 0, time.UTC))
	if err != nil {
		t.Fatal(err)
	}
	people, err := Load(ctx, db)
	if err != nil || len(people) != 1 {
		t.Fatalf("%v %v", people, err)
	}
	p := people[0]
	if p.ID != id || p.Surname != "Mustermann" || len(p.Forms) != 1 || p.AddedAt != "2026-11-19T12:00:00Z" {
		t.Fatalf("%+v", p)
	}
	if err := SetMeta(ctx, db, "snapshots_digest", "abc"); err != nil {
		t.Fatal(err)
	}
	if err := SetMeta(ctx, db, "snapshots_digest", "def"); err != nil || Meta(ctx, db, "snapshots_digest") != "def" {
		t.Fatalf("meta: %v %q", err, Meta(ctx, db, "snapshots_digest"))
	}
	if ok, err := Remove(ctx, db, id); !ok || err != nil {
		t.Fatalf("remove: %v %v", ok, err)
	}
	if people, _ := Load(ctx, db); len(people) != 0 {
		t.Fatal("still in the register")
	}
}
