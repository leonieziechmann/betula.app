package catalogdb

import (
	"path/filepath"
	"testing"
	"time"
)

func openTestDB(t *testing.T) *DB {
	t.Helper()
	db, err := Open(filepath.Join(t.TempDir(), "v2.db"))
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })
	return db
}

func TestOpenAppliesAllMigrationsOnce(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v2.db")
	db, err := Open(path)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	migrations, err := loadMigrations()
	if err != nil {
		t.Fatalf("loadMigrations failed: %v", err)
	}
	version, err := db.SchemaVersion()
	if err != nil || version != len(migrations) {
		t.Fatalf("schema version = %d (err %v), want %d", version, err, len(migrations))
	}
	_ = db.Close()

	// Reopening must not re-run migrations (CREATE TABLE would fail).
	db, err = Open(path)
	if err != nil {
		t.Fatalf("reopen failed: %v", err)
	}
	defer db.Close()
}

func TestForeignKeysAreEnforced(t *testing.T) {
	db := openTestDB(t)
	var on int
	if err := db.SQL().QueryRow("PRAGMA foreign_keys").Scan(&on); err != nil || on != 1 {
		t.Fatalf("foreign_keys = %d (err %v), want 1", on, err)
	}
}

func TestPutPageRoundTripAndChangeTracking(t *testing.T) {
	db := openTestDB(t)
	first := time.Date(2026, 9, 19, 10, 0, 0, 0, time.UTC)
	second := first.Add(24 * time.Hour)
	third := second.Add(24 * time.Hour)

	put := func(at time.Time, body string) {
		t.Helper()
		err := db.PutPage(RawPage{Source: SourceModulePage, Key: "11101", URL: "https://www.b-tu.de/modul/11101",
			FetchedAt: at, HTTPStatus: 200, Body: []byte(body)})
		if err != nil {
			t.Fatalf("PutPage failed: %v", err)
		}
	}

	put(first, "<html>Übung</html>")
	put(second, "<html>Übung</html>")
	p, err := db.GetPage(SourceModulePage, "11101")
	if err != nil {
		t.Fatalf("GetPage failed: %v", err)
	}
	if string(p.Body) != "<html>Übung</html>" || p.HTTPStatus != 200 {
		t.Fatalf("unexpected page: %+v", p)
	}
	if !p.FetchedAt.Equal(second) || !p.ChangedAt.Equal(first) {
		t.Fatalf("unchanged body: fetched_at=%v changed_at=%v, want %v / %v", p.FetchedAt, p.ChangedAt, second, first)
	}

	put(third, "<html>Vorlesung</html>")
	p, _ = db.GetPage(SourceModulePage, "11101")
	if !p.ChangedAt.Equal(third) {
		t.Fatalf("changed body: changed_at=%v, want %v", p.ChangedAt, third)
	}

	var rows int
	_ = db.SQL().QueryRow("SELECT COUNT(*) FROM raw_page").Scan(&rows)
	if rows != 1 {
		t.Fatalf("raw_page rows = %d, want 1 (latest version only)", rows)
	}
}

func TestGetPageNotFoundAndBodylessPage(t *testing.T) {
	db := openTestDB(t)
	if _, err := db.GetPage(SourceModulePage, "99999"); err != ErrNotFound {
		t.Fatalf("GetPage error = %v, want ErrNotFound", err)
	}

	if err := db.PutPage(RawPage{Source: SourceModulePage, Key: "99999", URL: "u", HTTPStatus: 404}); err != nil {
		t.Fatalf("PutPage failed: %v", err)
	}
	p, err := db.GetPage(SourceModulePage, "99999")
	if err != nil || p.HTTPStatus != 404 || p.Body != nil || p.Hash != "" {
		t.Fatalf("bodyless page = %+v (err %v)", p, err)
	}
}
