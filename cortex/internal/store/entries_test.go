package store

import (
	"encoding/json"
	"errors"
	"net/http"
	"reflect"
	"strings"
	"testing"
	"time"
)

func countRows(t *testing.T, s *Store, table string) int {
	t.Helper()
	var n int
	if err := s.r.QueryRow("SELECT COUNT(*) FROM " + table).Scan(&n); err != nil {
		t.Fatalf("count %s: %v", table, err)
	}
	return n
}

func TestUnchangedFetchOnlyMovesCheckedAt(t *testing.T) {
	s := openTestStore(t)
	f := fetched(t, s, "https://www.b-tu.de/modul/11101", 200, "<html>Übung</html>", t0)
	e1, v1, changed, j1 := record(t, s, f)
	if !changed || j1.Op != OpVersion || j1.Seq != 1 || j1.Blob != f.Hash {
		t.Fatalf("first fetch: changed %v, journal %+v, want a version entry 1 naming the blob", changed, j1)
	}
	if !v1.FetchedAt.Equal(t0) || !v1.CheckedAt.Equal(t0) || !v1.SupersededAt.IsZero() || e1.CurrentVersion != v1.ID {
		t.Fatalf("first version %+v of entry %+v", v1, e1)
	}
	blobs := blobFiles(t, s)

	g := fetched(t, s, "https://www.b-tu.de/modul/11101", 200, "<html>Übung</html>", t0.Add(24*time.Hour))
	e2, v2, changed, j2 := record(t, s, g)
	if changed || j2.Op != OpCheck || j2.Seq != 2 || j2.Blob != "" {
		t.Fatalf("unchanged fetch: changed %v, journal %+v, want one check entry", changed, j2)
	}
	if v2.ID != v1.ID || !v2.FetchedAt.Equal(t0) || !v2.CheckedAt.Equal(t0.Add(24*time.Hour)) {
		t.Fatalf("unchanged fetch: version %+v, want %d with only checked_at moved", v2, v1.ID)
	}
	if e2 != e1 {
		t.Fatalf("unchanged fetch changed the entry: %+v, was %+v", e2, e1)
	}
	if n := countRows(t, s, "version"); n != 1 {
		t.Fatalf("%d versions, want 1", n)
	}
	if got := blobFiles(t, s); len(got) != len(blobs) {
		t.Fatalf("blob files %v, were %v: no new blob for the same content", got, blobs)
	}
	if seq, _ := s.Position(); seq != 2 {
		t.Fatalf("Position = %d, want 2", seq)
	}

	// A fetch that finished before the last check does not move checked_at back.
	h := g
	h.At = t0.Add(time.Hour)
	_, v3, _, _ := record(t, s, h)
	if !v3.CheckedAt.Equal(t0.Add(24 * time.Hour)) {
		t.Fatalf("checked_at moved back to %v", v3.CheckedAt)
	}

	_, cur, err := s.Lookup(f.Key)
	if err != nil || cur.ID != v1.ID || !cur.CheckedAt.Equal(t0.Add(24*time.Hour)) {
		t.Fatalf("Lookup = %+v (err %v)", cur, err)
	}
	if cur.Header.Get("Content-Type") != "text/html; charset=utf-8" {
		t.Fatalf("kept headers = %v", cur.Header)
	}
}

func TestChangedContentAddsAVersionAndKeepsTheHistory(t *testing.T) {
	s := openTestStore(t)
	const u = "https://qis.b-tu.de/qisserver/rds?state=verpublish&veranstaltung.veranstid=1"
	t1, t2, t3 := t0, t0.Add(48*time.Hour), t0.Add(96*time.Hour)
	_, v1, _, _ := record(t, s, fetched(t, s, u, 200, "Montag 9 Uhr", t1))
	record(t, s, fetched(t, s, u, 200, "Montag 9 Uhr", t1.Add(time.Hour)))
	e, v2, changed, j := record(t, s, fetched(t, s, u, 200, "Dienstag 11 Uhr", t2))
	if !changed || j.Op != OpVersion || v2.ID == v1.ID || e.CurrentVersion != v2.ID {
		t.Fatalf("changed content: changed %v, op %s, version %+v, entry %+v", changed, j.Op, v2, e)
	}
	_, v3, _, _ := record(t, s, fetched(t, s, u, 200, "Mittwoch 13 Uhr", t3))

	entry, versions, err := s.Versions(e.Key)
	if err != nil || entry.ID != e.ID || len(versions) != 3 {
		t.Fatalf("Versions = %d versions (err %v), want 3", len(versions), err)
	}
	if versions[0].ID != v3.ID || versions[1].ID != v2.ID || versions[2].ID != v1.ID {
		t.Fatalf("Versions not newest first: %d, %d, %d", versions[0].ID, versions[1].ID, versions[2].ID)
	}
	if !versions[2].SupersededAt.Equal(t2) || !versions[2].CheckedAt.Equal(t1.Add(time.Hour)) {
		t.Fatalf("first version %+v, want superseded at %v", versions[2], t2)
	}
	if !versions[1].SupersededAt.Equal(t3) || !versions[0].SupersededAt.IsZero() {
		t.Fatalf("superseded_at of the later versions: %v, %v", versions[1].SupersededAt, versions[0].SupersededAt)
	}

	for _, tc := range []struct {
		at   time.Time
		want int64
	}{
		{t1, v1.ID},
		{t1.Add(time.Hour), v1.ID},
		{t2.Add(-time.Microsecond), v1.ID},
		{t2, v2.ID}, // superseded at t2 is no longer current at t2
		{t3.Add(-time.Second), v2.ID},
		{t3, v3.ID},
		{t3.Add(1000 * time.Hour), v3.ID},
	} {
		_, v, err := s.LookupAt(e.Key, tc.at)
		if err != nil || v.ID != tc.want {
			t.Errorf("LookupAt(%v) = version %d (err %v), want %d", tc.at, v.ID, err, tc.want)
		}
	}
	if _, _, err := s.LookupAt(e.Key, t1.Add(-time.Microsecond)); !errors.Is(err, ErrNotFound) {
		t.Errorf("LookupAt before the first fetch: err %v, want ErrNotFound", err)
	}
	if _, _, err := s.Lookup("GET https://qis.b-tu.de/other"); !errors.Is(err, ErrNotFound) {
		t.Errorf("Lookup of an unknown key: err %v, want ErrNotFound", err)
	}
	if _, _, err := s.Versions("GET https://qis.b-tu.de/other"); !errors.Is(err, ErrNotFound) {
		t.Errorf("Versions of an unknown key: err %v, want ErrNotFound", err)
	}
}

func TestStatusChangeCountsAsAChange(t *testing.T) {
	s := openTestStore(t)
	const u = "https://www.b-tu.de/modul/99999"
	_, v1, _, _ := record(t, s, fetched(t, s, u, 200, "", t0))
	_, v2, changed, j := record(t, s, fetched(t, s, u, 404, "", t0.Add(time.Hour)))
	if !changed || j.Op != OpVersion || v2.ID == v1.ID || v2.Status != 404 || v2.Hash != v1.Hash {
		t.Fatalf("200 → 404 with the same body: changed %v, op %s, version %+v", changed, j.Op, v2)
	}
	_, v3, changed, _ := record(t, s, fetched(t, s, u, 404, "", t0.Add(2*time.Hour)))
	if changed || v3.ID != v2.ID {
		t.Fatalf("404 again: changed %v, version %d, want the 404 version %d checked", changed, v3.ID, v2.ID)
	}
}

func TestRecordFetchKeepsTheEntryUpToDate(t *testing.T) {
	s := openTestStore(t)
	key, u, host, err := Canonical("https://opus4.kobv.de/files/1.pdf", "application/pdf", "de")
	if err != nil {
		t.Fatal(err)
	}
	b := putBlob(t, s, "%PDF-1.7")
	f := Fetched{Key: key, URL: u, Host: host, Source: "unknown", Accept: "application/pdf", AcceptLanguage: "de",
		Status: 200, Hash: b.Hash, Size: b.Size, At: t0}
	e, v, _, _ := record(t, s, f)
	if e.Accept != "application/pdf" || e.AcceptLanguage != "de" || e.Source != "unknown" || !e.CreatedAt.Equal(t0) {
		t.Fatalf("entry %+v", e)
	}
	if len(v.Header) != 0 {
		t.Fatalf("headers %v, want none", v.Header)
	}
	f.Source, f.At = "statute", t0.Add(time.Hour)
	if e, _, _, _ = record(t, s, f); e.Source != "statute" || !e.CreatedAt.Equal(t0) {
		t.Fatalf("entry after a fetch with another source: %+v", e)
	}
	f.Source, f.At = "", t0.Add(2*time.Hour)
	if e, _, _, _ = record(t, s, f); e.Source != "statute" {
		t.Fatalf("a fetch without a source changed it: %+v", e)
	}

	for _, bad := range []Fetched{
		{Key: key, URL: u, Status: 200, Hash: "nothex", At: t0},
		{Key: key, URL: u, Status: 200, Hash: sha([]byte("not stored")), At: t0},
		{Key: key, URL: u, Status: 0, Hash: b.Hash, At: t0},
		{URL: u, Status: 200, Hash: b.Hash, At: t0},
	} {
		if _, _, _, _, err := s.RecordFetch(bad); err == nil {
			t.Errorf("RecordFetch(%+v) succeeded", bad)
		}
	}
	if seq, _ := s.Position(); seq != 3 {
		t.Fatalf("Position = %d, want 3: refused fetches are not journaled", seq)
	}
}

func TestDeleteEntryRemovesTheEntryAndItsVersions(t *testing.T) {
	s := openTestStore(t)
	const u = "https://www.b-tu.de/modul/11101"
	record(t, s, fetched(t, s, u, 200, "a", t0))
	e, _, _, _ := record(t, s, fetched(t, s, u, 200, "b", t0.Add(time.Hour)))
	record(t, s, fetched(t, s, "https://www.b-tu.de/modul/11102", 200, "c", t0))

	j, err := s.DeleteEntry(e.Key)
	if err != nil || j.Op != OpEntryDelete {
		t.Fatalf("DeleteEntry = %+v (err %v)", j, err)
	}
	var p entryDeletePayload
	if err := json.Unmarshal(j.Payload, &p); err != nil || p.ID != e.ID || p.Key != e.Key {
		t.Fatalf("payload %s (err %v)", j.Payload, err)
	}
	if _, _, err := s.Lookup(e.Key); !errors.Is(err, ErrNotFound) {
		t.Fatalf("Lookup after DeleteEntry: err %v", err)
	}
	if n := countRows(t, s, "version"); n != 1 {
		t.Fatalf("%d versions left, want the other entry's 1", n)
	}
	if _, err := s.DeleteEntry(e.Key); !errors.Is(err, ErrNotFound) {
		t.Fatalf("second DeleteEntry: err %v, want ErrNotFound", err)
	}
	// A new fetch starts a new entry with a new id: ids are never used twice.
	e2, _, _, _ := record(t, s, fetched(t, s, u, 200, "b", t0.Add(2*time.Hour)))
	if e2.ID <= e.ID {
		t.Fatalf("new entry id %d, want above %d", e2.ID, e.ID)
	}
}

func TestListEntriesFiltersAndPages(t *testing.T) {
	s := openTestStore(t)
	add := func(rawURL, source, body string, at time.Time) {
		t.Helper()
		f := fetched(t, s, rawURL, 200, body, at)
		f.Source = source
		f.Header = http.Header{}
		record(t, s, f)
	}
	for i, u := range []string{"a", "b", "c", "d", "e"} {
		add("https://qis.b-tu.de/"+u, "qis_tree", u, t0.Add(time.Duration(i)*time.Hour))
	}
	add("https://www.b-tu.de/modul/1", "module_page", "m1", t0)
	add("https://www.b-tu.de/modul/2", "module_page", "m2", t0)
	add("https://qis.b-tu.de/a", "qis_tree", "a, changed", t0.Add(10*time.Hour))

	var all []EntryInfo
	cursor := ""
	for page := 0; ; page++ {
		got, next, err := s.ListEntries(EntryFilter{Host: "qis.b-tu.de"}, cursor, 2)
		if err != nil {
			t.Fatalf("ListEntries failed: %v", err)
		}
		all = append(all, got...)
		if next == "" {
			break
		}
		if page > 5 {
			t.Fatalf("paging does not end")
		}
		cursor = next
	}
	if len(all) != 5 {
		t.Fatalf("qis.b-tu.de: %d entries, want 5", len(all))
	}
	for i := 1; i < len(all); i++ {
		if all[i].ID <= all[i-1].ID {
			t.Fatalf("entries not in id order")
		}
	}
	if all[0].URL != "https://qis.b-tu.de/a" || all[0].Current.Hash != sha([]byte("a, changed")) {
		t.Fatalf("first entry %+v, want /a with its current version", all[0])
	}

	got, next, err := s.ListEntries(EntryFilter{Source: "module_page"}, "", 0)
	if err != nil || len(got) != 2 || next != "" {
		t.Fatalf("module_page: %d entries, next %q (err %v)", len(got), next, err)
	}
	got, _, err = s.ListEntries(EntryFilter{Host: "qis.b-tu.de", Source: "qis_tree", ChangedSince: t0.Add(3 * time.Hour)}, "", 0)
	if err != nil || len(got) != 3 {
		t.Fatalf("changed since 3 h: %d entries (err %v), want d, e and the changed a", len(got), err)
	}
	if got, _, err := s.ListEntries(EntryFilter{Host: "example.com"}, "", 0); err != nil || len(got) != 0 {
		t.Fatalf("unknown host: %d entries (err %v)", len(got), err)
	}
	if _, _, err := s.ListEntries(EntryFilter{}, "x", 0); !errors.Is(err, ErrInvalidCursor) {
		t.Fatalf("bad cursor: err %v, want ErrInvalidCursor", err)
	}
	if got, _, _ := s.ListEntries(EntryFilter{}, "", 100000); len(got) != 7 {
		t.Fatalf("all: %d entries, want 7", len(got))
	}
}

// Review 1 (non-utf8-journal-divergence, journalafter-unbounded-bytes): a kept header value is
// valid UTF-8 and at most 8 KiB in the index; a longer one is left out (every check entry
// carries the whole version row again, and a host may send a megabyte of ETag).
func TestKeptHeadersAreValidUTF8AndAtMost8KiB(t *testing.T) {
	s := openTestStore(t)
	f := fetched(t, s, "https://opus4.kobv.de/files/1.pdf", 200, "%PDF-1.7", t0)
	f.Header = http.Header{
		"Content-Type":        {"application/pdf"},
		"Etag":                {`"` + strings.Repeat("x", 9000) + `"`},
		"Content-Disposition": {strings.Repeat("y", 8<<10)},
		"Last-Modified":       {"Mon, 21 Sep 2026 10:00:00 GMT\xff\xfe"},
		"Content-Language":    {strings.Repeat("€", 3000)}, // 3000 characters, 9000 bytes
	}
	_, v, _, _ := record(t, s, f)
	want := http.Header{
		"Content-Type":        {"application/pdf"},
		"Content-Disposition": {strings.Repeat("y", 8<<10)},
		"Last-Modified":       {"Mon, 21 Sep 2026 10:00:00 GMT\uFFFD"},
	}
	if !reflect.DeepEqual(v.Header, want) {
		t.Fatalf("kept headers %q, want %q", v.Header, want)
	}
	if _, cur, err := s.Lookup(f.Key); err != nil || !reflect.DeepEqual(cur.Header, want) {
		t.Fatalf("Lookup = %q (err %v), want %q", cur.Header, err, want)
	}
}

// Review 2 (validators-final-hop-sent-to-first-hop): the validators of a version came from the
// URL that answered after redirects, but nothing recorded which one that was. The version keeps
// it now (FinalURL, under an internal header that Header never shows), and validators and URL
// are replaced together when the same content comes from another URL.
func TestAVersionKeepsTheURLItsValidatorsCameFrom(t *testing.T) {
	s := openTestStore(t)
	f := fetched(t, s, "https://www.b-tu.de/latest", 200, "v3", t0)
	f.Header.Set("ETag", `"v3"`)
	f.Header.Set(FinalURLHeader, "https://evil.example/") // only Fetched.FinalURL counts
	f.FinalURL = "https://www.b-tu.de/v3"
	_, v, _, _ := record(t, s, f)
	if v.FinalURL != f.FinalURL || v.Header.Get(FinalURLHeader) != "" || v.Header.Get("ETag") != `"v3"` {
		t.Fatalf("version: final URL %q, headers %v", v.FinalURL, v.Header)
	}
	if _, cur, err := s.Lookup(f.Key); err != nil || cur.FinalURL != f.FinalURL || cur.Header.Get(FinalURLHeader) != "" {
		t.Fatalf("Lookup: %+v (err %v)", cur, err)
	}

	// The same content again from the same URL: a check, the headers as they were.
	again := f
	again.Header = http.Header{"Etag": {`"other"`}}
	again.At = t0.Add(time.Hour)
	if _, v, changed, _ := record(t, s, again); changed || v.Header.Get("ETag") != `"v3"` {
		t.Fatalf("a check from the same URL: changed %v, headers %v", changed, v.Header)
	}
	// The same content from the URL itself now: a check that takes this answer's validators.
	direct := f
	direct.FinalURL = "https://www.b-tu.de/latest"
	direct.Header = http.Header{"Etag": {`"latest"`}, "Content-Type": {"text/html"}}
	direct.At = t0.Add(2 * time.Hour)
	_, v, changed, je := record(t, s, direct)
	if changed || je.Op != OpCheck || v.FinalURL != direct.FinalURL || v.Header.Get("ETag") != `"latest"` {
		t.Fatalf("a check from another URL: changed %v, op %s, final URL %q, headers %v", changed, je.Op, v.FinalURL, v.Header)
	}
	// A follower gets the same.
	follower := openTestStore(t)
	follow(t, s, follower)
	if _, fv, err := follower.Lookup(f.Key); err != nil || fv.FinalURL != direct.FinalURL || fv.Header.Get("ETag") != `"latest"` {
		t.Fatalf("follower: %+v (err %v)", fv, err)
	}
}
