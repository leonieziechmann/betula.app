// Package catalogdb is the schema v2 database: a raw page archive, the canonical
// catalog model derived from it, and the read views that are the only contract
// for consumers. See docs/data-sources.md and docs/backend-data-overhaul.md.
package catalogdb

import (
	"database/sql"
	"embed"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/leonieziechmann/btu-scraper/internal/oplog"
	_ "modernc.org/sqlite"
)

//go:embed migrations/*.sql
var migrationFS embed.FS

// DB is a schema v2 database handle.
type DB struct {
	sql  *sql.DB
	path string
}

// Open opens (or creates) the database at path, enforces foreign keys on every
// connection and applies all pending migrations.
func Open(path string) (*DB, error) {
	if dir := filepath.Dir(path); dir != "" && dir != "." {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return nil, fmt.Errorf("failed to create db directory: %w", err)
		}
	}

	// busy_timeout: a build holds the write lock for about 20 s; a crawler that runs
	// next to it (CLI beside the service) must wait for it instead of failing.
	dsn := fmt.Sprintf("%s?_pragma=busy_timeout(60000)&_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)&_pragma=foreign_keys(ON)", path)
	sqlDB, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("failed to open sqlite database: %w", err)
	}

	db := &DB{sql: sqlDB, path: path}
	if err := db.migrate(); err != nil {
		_ = sqlDB.Close()
		return nil, err
	}
	return db, nil
}

// SQL returns the underlying sql.DB instance.
func (db *DB) SQL() *sql.DB {
	return db.sql
}

// Path returns the database file path.
func (db *DB) Path() string {
	return db.path
}

// Close closes the database connection.
func (db *DB) Close() error {
	return db.sql.Close()
}

// SchemaVersion returns the number of the last applied migration.
func (db *DB) SchemaVersion() (int, error) {
	var v int
	err := db.sql.QueryRow("PRAGMA user_version").Scan(&v)
	return v, err
}

type migration struct {
	version int
	name    string
	sql     string
}

// loadMigrations reads migrations/NNNN_name.sql in version order. A gap or a
// duplicate number is an error: the version is the only record of the DB shape.
func loadMigrations() ([]migration, error) {
	entries, err := fs.ReadDir(migrationFS, "migrations")
	if err != nil {
		return nil, err
	}
	var result []migration
	for _, e := range entries {
		name := e.Name()
		if e.IsDir() || !strings.HasSuffix(name, ".sql") {
			continue
		}
		prefix, _, ok := strings.Cut(name, "_")
		if !ok {
			return nil, fmt.Errorf("migration %q: expected NNNN_name.sql", name)
		}
		version, err := strconv.Atoi(prefix)
		if err != nil || version <= 0 {
			return nil, fmt.Errorf("migration %q: invalid version prefix", name)
		}
		body, err := migrationFS.ReadFile("migrations/" + name)
		if err != nil {
			return nil, err
		}
		result = append(result, migration{version: version, name: name, sql: string(body)})
	}
	sort.Slice(result, func(i, j int) bool { return result[i].version < result[j].version })
	for i, m := range result {
		if m.version != i+1 {
			return nil, fmt.Errorf("migration %q: expected version %d", m.name, i+1)
		}
	}
	return result, nil
}

func (db *DB) migrate() error {
	migrations, err := loadMigrations()
	if err != nil {
		return fmt.Errorf("failed to load migrations: %w", err)
	}
	current, err := db.SchemaVersion()
	if err != nil {
		return fmt.Errorf("failed to read schema version: %w", err)
	}
	if current > len(migrations) {
		return fmt.Errorf("database %s has schema version %d, this binary only knows %d", db.path, current, len(migrations))
	}

	for _, m := range migrations[current:] {
		tx, err := db.sql.Begin()
		if err != nil {
			return err
		}
		if _, err := tx.Exec(m.sql); err != nil {
			_ = tx.Rollback()
			return fmt.Errorf("migration %s failed: %w", m.name, err)
		}
		// user_version lives in the database header and is part of the transaction.
		if _, err := tx.Exec(fmt.Sprintf("PRAGMA user_version = %d", m.version)); err != nil {
			_ = tx.Rollback()
			return fmt.Errorf("migration %s: failed to set version: %w", m.name, err)
		}
		if err := tx.Commit(); err != nil {
			return fmt.Errorf("migration %s: commit failed: %w", m.name, err)
		}
		oplog.For("db").Info("migration applied", "event", "db.migrated", "migration", m.name, "schema_version", m.version, "db", db.path)
	}
	return nil
}
