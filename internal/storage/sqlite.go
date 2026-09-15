package storage

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"

	_ "modernc.org/sqlite"
)

// Storage handles SQLite database connection and operations.
type Storage struct {
	db     *sql.DB
	dbPath string
}

// NewStorage creates or connects to a SQLite database and runs migrations.
func NewStorage(dbPath string) (*Storage, error) {
	if dir := filepath.Dir(dbPath); dir != "" && dir != "." {
		if err := os.MkdirAll(dir, 0755); err != nil {
			return nil, fmt.Errorf("failed to create db directory: %w", err)
		}
	}

	dsn := fmt.Sprintf("%s?_pragma=busy_timeout(5000)&_pragma=journal_mode(WAL)&_pragma=synchronous(NORMAL)", dbPath)
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("failed to open sqlite database: %w", err)
	}

	s := &Storage{db: db, dbPath: dbPath}
	if err := s.migrate(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("failed to migrate database: %w", err)
	}

	return s, nil
}

// DB returns the underlying sql.DB instance.
func (s *Storage) DB() *sql.DB {
	return s.db
}

// Path returns the SQLite database file path.
func (s *Storage) Path() string {
	return s.dbPath
}

// Checkpoint flushes WAL pages into the database file so it is safe to copy or serve.
func (s *Storage) Checkpoint() error {
	_, err := s.db.Exec("PRAGMA wal_checkpoint(PASSIVE);")
	return err
}

// Close closes the database connection.
func (s *Storage) Close() error {
	return s.db.Close()
}

func (s *Storage) migrate() error {
	schema := `
	CREATE TABLE IF NOT EXISTS modules (
		id TEXT PRIMARY KEY,
		code TEXT,
		title_de TEXT,
		title_en TEXT,
		is_phase_out INTEGER DEFAULT 0,
		department TEXT,
		responsible_persons TEXT,
		language TEXT,
		duration TEXT,
		turnus TEXT,
		credits REAL,
		credits_raw TEXT,
		learning_outcomes TEXT,
		contents TEXT,
		prerequisites_recommended TEXT,
		prerequisites_mandatory TEXT,
		teaching_forms TEXT,
		literature TEXT,
		exam_type TEXT,
		exam_details TEXT,
		grading TEXT,
		limitation TEXT,
		study_programs TEXT,
		remarks TEXT,
		associated_courses TEXT,
		current_semester_events TEXT,
		cross_disciplinary INTEGER DEFAULT 0,
		is_fues INTEGER DEFAULT 0,
		is_not_offered INTEGER DEFAULT 0,
		successor_modules TEXT,
		raw_url TEXT,
		last_scraped_at DATETIME,
		created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
		updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
	);

	CREATE INDEX IF NOT EXISTS idx_modules_code ON modules(code);
	CREATE INDEX IF NOT EXISTS idx_modules_title_de ON modules(title_de);
	CREATE INDEX IF NOT EXISTS idx_modules_title_en ON modules(title_en);
	CREATE INDEX IF NOT EXISTS idx_modules_department ON modules(department);
	CREATE INDEX IF NOT EXISTS idx_modules_credits ON modules(credits);
	CREATE INDEX IF NOT EXISTS idx_modules_language ON modules(language);

	CREATE TABLE IF NOT EXISTS events (
		id TEXT PRIMARY KEY,
		event_number TEXT,
		title TEXT,
		event_type TEXT,
		semester TEXT,
		sws TEXT,
		expected_participants TEXT,
		max_participants TEXT,
		hyperlink TEXT,
		description TEXT,
		responsible_persons TEXT,
		associated_modules TEXT,
		study_programs TEXT,
		institutions TEXT,
		raw_url TEXT,
		last_scraped_at DATETIME,
		created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
		updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
	);

	CREATE TABLE IF NOT EXISTS event_schedules (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		event_id TEXT,
		group_name TEXT,
		day_of_week TEXT,
		time_slot TEXT,
		start_time TEXT,
		end_time TEXT,
		rhythm TEXT,
		duration TEXT,
		room TEXT,
		room_url TEXT,
		instructor TEXT,
		instructor_url TEXT,
		comment TEXT,
		cancelled_dates TEXT,
		FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
	);

	CREATE TABLE IF NOT EXISTS module_events (
		module_id TEXT,
		event_id TEXT,
		PRIMARY KEY (module_id, event_id),
		FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE,
		FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
	);

	CREATE INDEX IF NOT EXISTS idx_events_number ON events(event_number);
	CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
	CREATE INDEX IF NOT EXISTS idx_schedules_event_id ON event_schedules(event_id);
	CREATE INDEX IF NOT EXISTS idx_schedules_day_time ON event_schedules(day_of_week, start_time);
	CREATE INDEX IF NOT EXISTS idx_schedules_room ON event_schedules(room);
	CREATE INDEX IF NOT EXISTS idx_module_events_mod ON module_events(module_id);
	CREATE INDEX IF NOT EXISTS idx_module_events_evt ON module_events(event_id);

	CREATE TABLE IF NOT EXISTS official_study_programs (
		id TEXT PRIMARY KEY,
		program_name TEXT NOT NULL,
		program_code TEXT,
		degree TEXT NOT NULL,
		degree_code TEXT,
		po_version TEXT NOT NULL,
		qis_node_id TEXT,
		qis_url TEXT,
		documents TEXT,
		scraped_at DATETIME DEFAULT CURRENT_TIMESTAMP,
		updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
	);

	CREATE INDEX IF NOT EXISTS idx_official_stg_name ON official_study_programs(program_name);
	CREATE INDEX IF NOT EXISTS idx_official_stg_degree ON official_study_programs(degree);
	CREATE INDEX IF NOT EXISTS idx_official_stg_po ON official_study_programs(po_version);

	CREATE TABLE IF NOT EXISTS module_study_programs (
		module_id TEXT NOT NULL,
		program_id TEXT NOT NULL,
		program_name TEXT NOT NULL,
		degree TEXT,
		regulation TEXT,
		PRIMARY KEY (module_id, program_id),
		FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE CASCADE,
		FOREIGN KEY (program_id) REFERENCES official_study_programs(id) ON DELETE CASCADE
	);

	CREATE INDEX IF NOT EXISTS idx_msp_module ON module_study_programs(module_id);
	CREATE INDEX IF NOT EXISTS idx_msp_program ON module_study_programs(program_id);
	CREATE INDEX IF NOT EXISTS idx_msp_prog_name ON module_study_programs(program_name);

	CREATE TABLE IF NOT EXISTS program_curriculum_modules (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		program_id TEXT NOT NULL,
		program_name TEXT NOT NULL,
		degree TEXT,
		po_version TEXT,
		module_id TEXT,
		module_code TEXT,
		module_name TEXT NOT NULL,
		module_name_en TEXT,
		recommended_semester INTEGER DEFAULT 0,
		recommended_semester_raw TEXT,
		credits REAL DEFAULT 0,
		module_type TEXT NOT NULL,
		specialization TEXT,
		sws TEXT,
		exam_type TEXT,
		graded TEXT,
		prerequisites TEXT,
		remarks TEXT,
		source_file TEXT,
		extracted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
		FOREIGN KEY (module_id) REFERENCES modules(id) ON DELETE SET NULL,
		FOREIGN KEY (program_id) REFERENCES official_study_programs(id) ON DELETE CASCADE
	);

	CREATE INDEX IF NOT EXISTS idx_pcm_program ON program_curriculum_modules(program_id);
	CREATE INDEX IF NOT EXISTS idx_pcm_module ON program_curriculum_modules(module_id);
	CREATE INDEX IF NOT EXISTS idx_pcm_semester ON program_curriculum_modules(recommended_semester);
	CREATE INDEX IF NOT EXISTS idx_pcm_type ON program_curriculum_modules(module_type);
	`
	if _, err := s.db.Exec(schema); err != nil {
		return err
	}
	_, _ = s.db.Exec("ALTER TABLE modules ADD COLUMN is_fues INTEGER DEFAULT 0")
	_, _ = s.db.Exec("CREATE INDEX IF NOT EXISTS idx_modules_is_fues ON modules(is_fues)")
	_, _ = s.db.Exec("ALTER TABLE modules ADD COLUMN is_not_offered INTEGER DEFAULT 0")
	_, _ = s.db.Exec("ALTER TABLE modules ADD COLUMN successor_modules TEXT")
	_, _ = s.db.Exec("DELETE FROM module_study_programs WHERE degree = 'Abschluss im Ausland'")

	// Enrich module_study_programs with AI-verified curriculum fields
	_, _ = s.db.Exec("ALTER TABLE module_study_programs ADD COLUMN recommended_semester INTEGER DEFAULT 0")
	_, _ = s.db.Exec("ALTER TABLE module_study_programs ADD COLUMN module_type TEXT")
	_, _ = s.db.Exec("ALTER TABLE module_study_programs ADD COLUMN specialization TEXT")
	_, _ = s.db.Exec("ALTER TABLE module_study_programs ADD COLUMN credits REAL DEFAULT 0")
	_, _ = s.db.Exec("ALTER TABLE module_study_programs ADD COLUMN source TEXT")
	return nil
}
