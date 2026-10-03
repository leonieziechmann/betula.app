package store

import (
	"bytes"
	"database/sql"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

var t0 = time.Date(2026, 9, 19, 10, 0, 0, 0, time.UTC)

func openTestStore(t *testing.T) *Store {
	t.Helper()
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = s.Close() })
	return s
}

// putBlob stores body and returns its info.
func putBlob(t *testing.T, s *Store, body string) BlobInfo {
	t.Helper()
	b, err := s.PutBlob(strings.NewReader(body), "", 0)
	if err != nil {
		t.Fatalf("PutBlob failed: %v", err)
	}
	return b
}

// fetched builds a fetch of rawURL that returned body (stored as a blob) with status.
func fetched(t *testing.T, s *Store, rawURL string, status int, body string, at time.Time) Fetched {
	t.Helper()
	key, u, host, err := Canonical(rawURL, "", "")
	if err != nil {
		t.Fatalf("Canonical(%q) failed: %v", rawURL, err)
	}
	b := putBlob(t, s, body)
	return Fetched{Key: key, URL: u, Host: host, Source: "module_page", Status: status, Hash: b.Hash, Size: b.Size,
		Header: http.Header{"Content-Type": {"text/html; charset=utf-8"}}, At: at}
}

func record(t *testing.T, s *Store, f Fetched) (Entry, Version, bool, JournalEntry) {
	t.Helper()
	e, v, changed, je, err := s.RecordFetch(f)
	if err != nil {
		t.Fatalf("RecordFetch(%s) failed: %v", f.URL, err)
	}
	return e, v, changed, je
}

// dump returns every row of every table, in key order, as text: two stores with the same dump
// hold the same index.
func dump(t *testing.T, s *Store) map[string][]string {
	t.Helper()
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make(map[string][]string)
	for _, table := range []string{"meta", "entry", "version", "file", "file_version", "journal", "sqlite_sequence"} {
		rows, err := s.r.Query("SELECT * FROM " + table + " ORDER BY 1")
		if err != nil {
			t.Fatalf("dump %s: %v", table, err)
		}
		cols, _ := rows.Columns()
		for rows.Next() {
			values := make([]any, len(cols))
			ptrs := make([]any, len(cols))
			for i := range values {
				ptrs[i] = &values[i]
			}
			if err := rows.Scan(ptrs...); err != nil {
				t.Fatalf("dump %s: %v", table, err)
			}
			var line bytes.Buffer
			for i, v := range values {
				if b, ok := v.([]byte); ok {
					v = string(b)
				}
				fmt.Fprintf(&line, "%s=%#v ", cols[i], v)
			}
			result[table] = append(result[table], line.String())
		}
		if err := rows.Err(); err != nil {
			t.Fatalf("dump %s: %v", table, err)
		}
		rows.Close()
	}
	return result
}

func sameDump(t *testing.T, what string, got, want map[string][]string) {
	t.Helper()
	for table, rows := range want {
		if len(got[table]) != len(rows) {
			t.Fatalf("%s: table %s has %d rows, want %d", what, table, len(got[table]), len(rows))
		}
		for i := range rows {
			if got[table][i] != rows[i] {
				t.Fatalf("%s: table %s row %d:\n got %s\nwant %s", what, table, i, got[table][i], rows[i])
			}
		}
	}
	for table := range got {
		if _, ok := want[table]; !ok {
			t.Fatalf("%s: unexpected table %s", what, table)
		}
	}
}

func tmpFiles(t *testing.T, s *Store) []string {
	t.Helper()
	entries, err := os.ReadDir(s.tmpDir())
	if err != nil {
		t.Fatalf("ReadDir tmp: %v", err)
	}
	var names []string
	for _, e := range entries {
		names = append(names, e.Name())
	}
	return names
}

func TestOpenCreatesTheLayoutAndEmptiesTmp(t *testing.T) {
	dir := t.TempDir()
	s, err := Open(dir)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	for _, p := range []string{"index.db", "blobs/sha256", "tmp"} {
		if _, err := os.Stat(filepath.Join(dir, p)); err != nil {
			t.Errorf("%s missing: %v", p, err)
		}
	}
	var mode string
	if err := s.r.QueryRow("PRAGMA journal_mode").Scan(&mode); err != nil || mode != "wal" {
		t.Errorf("journal_mode = %q (err %v), want wal", mode, err)
	}
	var fk int
	if err := s.w.QueryRow("PRAGMA foreign_keys").Scan(&fk); err != nil || fk != 1 {
		t.Errorf("foreign_keys = %d (err %v), want 1", fk, err)
	}
	if err := s.Close(); err != nil {
		t.Fatalf("Close failed: %v", err)
	}

	// What a write in progress left behind is gone at the next open; the index stays.
	if err := os.WriteFile(filepath.Join(dir, "tmp", "blob-123"), []byte("half"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(dir, "tmp", "dir"), 0755); err != nil {
		t.Fatal(err)
	}
	s, err = Open(dir)
	if err != nil {
		t.Fatalf("reopen failed: %v", err)
	}
	defer s.Close()
	if names := tmpFiles(t, s); len(names) != 0 {
		t.Fatalf("tmp not emptied: %v", names)
	}
	if err := s.Ping(t.Context()); err != nil {
		t.Fatalf("Ping failed: %v", err)
	}
}

func TestOpenRefusesANewerSchema(t *testing.T) {
	dir := t.TempDir()
	s, err := Open(dir)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	if _, err := s.w.Exec("PRAGMA user_version = 99"); err != nil {
		t.Fatal(err)
	}
	_ = s.Close()
	if _, err := Open(dir); err == nil || !strings.Contains(err.Error(), "schema version 99") {
		t.Fatalf("Open of a newer index: err %v, want a schema version error", err)
	}
}

func TestClosedStoreRefusesIndexOperations(t *testing.T) {
	s := openTestStore(t)
	b := putBlob(t, s, "x")
	if err := s.Close(); err != nil {
		t.Fatal(err)
	}
	if _, _, err := s.Lookup("GET http://example.com/"); err != ErrClosed {
		t.Errorf("Lookup after Close: %v, want ErrClosed", err)
	}
	if err := s.WaitAfter(t.Context(), 0); err != ErrClosed {
		t.Errorf("WaitAfter after Close: %v, want ErrClosed", err)
	}
	// The blobs are files and stay readable.
	if !s.HasBlob(b.Hash) {
		t.Errorf("blob gone after Close")
	}
}

func TestCanonicalNormalizesOnlyWhatIsSafe(t *testing.T) {
	for _, tc := range []struct {
		raw, accept, lang string
		key, url, host    string
		wantErr           bool
	}{
		{raw: "HTTP://Example.COM", url: "http://example.com/", host: "example.com"},
		{raw: "https://example.com:443/a?b=1#frag", url: "https://example.com/a?b=1", host: "example.com"},
		{raw: "http://example.com:80", url: "http://example.com/", host: "example.com"},
		{raw: "http://example.com:8080/x", url: "http://example.com:8080/x", host: "example.com"},
		{raw: "https://example.com:80/", url: "https://example.com:80/", host: "example.com"},
		{raw: "http://EXAMPLE.com:/p", url: "http://example.com/p", host: "example.com"},
		{raw: "http://example.com?b=2&a=1", url: "http://example.com/?b=2&a=1", host: "example.com"},
		{raw: "http://example.com#top", url: "http://example.com/", host: "example.com"},
		{raw: "https://qis.b-tu.de/qisserver/rds?state=verpublish&publishContainer=lectureContainer&moduleCall=webInfo&publishConfFile=webInfo&publishSubDir=veranstaltung&veranstaltung.veranstid=123",
			url:  "https://qis.b-tu.de/qisserver/rds?state=verpublish&publishContainer=lectureContainer&moduleCall=webInfo&publishConfFile=webInfo&publishSubDir=veranstaltung&veranstaltung.veranstid=123",
			host: "qis.b-tu.de"},
		{raw: "http://example.com/A/%7e/a%2Fb?x=%20y&&z", url: "http://example.com/A/%7e/a%2Fb?x=%20y&&z", host: "example.com"},
		{raw: "http://example.com/%C3%9Cbung?q=%c3%bc", url: "http://example.com/%C3%9Cbung?q=%c3%bc", host: "example.com"},
		{raw: "http://[::1]:8080/", url: "http://[::1]:8080/", host: "::1"},
		{raw: "https://[2001:DB8::1]:443", url: "https://[2001:db8::1]/", host: "2001:db8::1"},
		{raw: "http://example.com/", accept: "text/html", url: "http://example.com/", host: "example.com",
			key: "GET http://example.com/\naccept: text/html"},
		{raw: "http://example.com/", lang: "de-DE", url: "http://example.com/", host: "example.com",
			key: "GET http://example.com/\naccept-language: de-DE"},
		{raw: "http://example.com/", accept: "application/pdf", lang: "de, en;q=0.5", url: "http://example.com/", host: "example.com",
			key: "GET http://example.com/\naccept: application/pdf\naccept-language: de, en;q=0.5"},
		{raw: "ftp://example.com/", wantErr: true},
		{raw: "http://user:pw@example.com/", wantErr: true},
		{raw: "http://user@example.com/", wantErr: true},
		{raw: "mailto:info@example.com", wantErr: true},
		{raw: "/relative/path", wantErr: true},
		{raw: "http:///path", wantErr: true},
		{raw: "http://example.com:99999/", wantErr: true},
		{raw: "http://exa mple.com/", wantErr: true},
		{raw: "http://example.com/\n", wantErr: true},
		{raw: "http://example.com/", accept: "text/html\naccept-language: de", wantErr: true},
		// Text that is not UTF-8 would reach the journal as U+FFFD (review 1), and a host that is
		// not ASCII is one name under two spellings (its punycode form is the one to use).
		{raw: "https://qis.b-tu.de/x?name=M\xfcller", wantErr: true},
		{raw: "https://qis.b-tu.de/M\xe4ller", wantErr: true},
		{raw: "http://example.com/", accept: "text/html\xff", wantErr: true},
		{raw: "http://example.com/", lang: "de-\xe4", wantErr: true},
		{raw: "http://exämple.com/", wantErr: true},
		{raw: "http://ex%C3%A4mple.com/", wantErr: true},
		{raw: "http://EXÄMPLE.com/", wantErr: true},
		{raw: "http://xn--exmple-cua.com/Übung?q=ü", url: "http://xn--exmple-cua.com/Übung?q=ü", host: "xn--exmple-cua.com"},
		{raw: "http://example.com/", accept: "text/html; q=0.9, application/xhtml+xml", lang: "de-DE, en;q=0.5, fr-ç",
			url: "http://example.com/", host: "example.com",
			key: "GET http://example.com/\naccept: text/html; q=0.9, application/xhtml+xml\naccept-language: de-DE, en;q=0.5, fr-ç"},
	} {
		key, u, host, err := Canonical(tc.raw, tc.accept, tc.lang)
		if tc.wantErr {
			if err == nil {
				t.Errorf("Canonical(%q, %q, %q) = %q, want an error", tc.raw, tc.accept, tc.lang, key)
			}
			continue
		}
		want := tc.key
		if want == "" {
			want = "GET " + tc.url
		}
		if err != nil || key != want || u != tc.url || host != tc.host {
			t.Errorf("Canonical(%q, %q, %q) = %q, %q, %q (err %v), want %q, %q, %q",
				tc.raw, tc.accept, tc.lang, key, u, host, err, want, tc.url, tc.host)
		}
	}
}

func TestValidFileNameFollowsTheNamingRules(t *testing.T) {
	for _, tc := range []struct {
		name string
		ok   bool
	}{
		{"models/multilingual-e5-small/model.onnx", true},
		{"a", true},
		{"Übung/ß.txt", true},
		{"with space/and.dots..txt", true},
		{strings.Repeat("a", 1024), true},
		{strings.Repeat("a", 1025), false},
		{"", false},
		{"/leading", false},
		{"trailing/", false},
		{"a//b", false},
		{"a/./b", false},
		{"a/../b", false},
		{"..", false},
		{"tab\there", false},
		{"nul\x00", false},
		{"del\x7f", false},
		{"c1\u0085", false},
		{"bad\xffutf8", false},
	} {
		if got := ValidFileName(tc.name); got != tc.ok {
			t.Errorf("ValidFileName(%q) = %v, want %v", tc.name, got, tc.ok)
		}
	}
}

func TestTimesAreFixedWidthMicrosecondsUTC(t *testing.T) {
	berlin := time.FixedZone("CEST", 2*3600)
	at := time.Date(2026, 10, 2, 12, 30, 0, 123456789, berlin)
	if got := FormatTime(at); got != "2026-10-02T10:30:00.123456Z" {
		t.Fatalf("FormatTime = %q", got)
	}
	if got := FormatTime(time.Date(2026, 10, 2, 10, 30, 0, 0, time.UTC)); got != "2026-10-02T10:30:00.000000Z" {
		t.Fatalf("FormatTime of a whole second = %q", got)
	}
	back, err := parseTime(FormatTime(at))
	if err != nil || !back.Equal(stamp(at)) || back.Location() != time.UTC {
		t.Fatalf("parseTime = %v (err %v), want %v in UTC", back, err, stamp(at))
	}
}

// rawExec runs SQL on the writer, for tests that need a state no write produces.
func rawExec(t *testing.T, s *Store, query string, args ...any) sql.Result {
	t.Helper()
	res, err := s.w.Exec(query, args...)
	if err != nil {
		t.Fatalf("%s: %v", query, err)
	}
	return res
}
