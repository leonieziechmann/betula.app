package model

import "time"

// CurriculumCatalogModule is a catalog snapshot used to validate a study plan.
type CurriculumCatalogModule struct {
	ID       string
	Code     string
	TitleDE  string
	TitleEN  string
	Turnus   string
	Duration string
	Credits  float64
}

// CurriculumCatalog is the module catalog as the study plan of one program is
// matched against it: every module of the university, and the modules this
// program already claims on its own pages. Titles repeat across the university
// (68 modules are called "Bachelor-Arbeit", two "Grundlagen der Elektrotechnik"
// are both current), so a plan row that prints no module number is told apart
// by what the program claims. Claims may be empty; the row then stays unlinked.
type CurriculumCatalog struct {
	Modules []CurriculumCatalogModule
	Claims  map[string]bool // module ids the program's own pages name
}

// CurriculumModule represents an extracted module recommendation from an official study regulation (PO / SO).
type CurriculumModule struct {
	SourceEvidence         string    `json:"source_evidence,omitempty"`
	ID                     int64     `json:"id,omitempty"`
	ProgramID              string    `json:"program_id"`
	ProgramName            string    `json:"program_name"`
	Degree                 string    `json:"degree,omitempty"`
	POVersion              string    `json:"po_version,omitempty"`
	ModuleID               string    `json:"module_id,omitempty"` // Matched module ID in modules table (e.g. "12104")
	ModuleCode             string    `json:"module_code,omitempty"`
	ModuleName             string    `json:"module_name"`
	ModuleNameEN           string    `json:"module_name_en,omitempty"`
	RecommendedSemester    int       `json:"recommended_semester"` // 1, 2, 3...
	RecommendedSemesterRaw string    `json:"recommended_semester_raw,omitempty"`
	SemesterSpan           string    `json:"semester_span,omitempty"` // e.g. "5-6", "3-4"
	StartSemester          int       `json:"start_semester,omitempty"`
	EndSemester            int       `json:"end_semester,omitempty"`
	Credits                float64   `json:"credits"`
	MinCredits             float64   `json:"min_credits,omitempty"`   // For range rules (e.g. 10.0 in "10-24")
	MaxCredits             float64   `json:"max_credits,omitempty"`   // For range rules (e.g. 24.0 in "10-24")
	ModuleType             string    `json:"module_type"`             // "Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"
	StudySection           string    `json:"study_section,omitempty"` // "Grundstudium", "Fachstudium", "Vertiefungsstudium"
	SubjectArea            string    `json:"subject_area,omitempty"`  // "Grundlagen der Informatik", "Praktische Informatik", "Angewandte und Technische Informatik", "Nebenfach"
	AreaRules              string    `json:"area_rules,omitempty"`    // e.g. "Im Nebenfach müssen alle Module aus demselben Bereich belegt werden"
	Specialization         string    `json:"specialization,omitempty"`
	SWS                    string    `json:"sws,omitempty"`
	ExamType               string    `json:"exam_type,omitempty"`
	Graded                 string    `json:"graded,omitempty"`
	Prerequisites          string    `json:"prerequisites,omitempty"`
	Remarks                string    `json:"remarks,omitempty"`
	SourceFile             string    `json:"source_file,omitempty"`
	ExtractedAt            time.Time `json:"extracted_at"`
	SourceCell             string    `json:"source_cell,omitempty"`       // the PDF cell this row was read from
	SourcePage             int       `json:"source_page,omitempty"`       // the page of the regulation this row stands on
	SourceTable            string    `json:"source_table,omitempty"`      // the plan table it was read from („p9t1")
	SourcePlanLabel        string    `json:"source_plan_label,omitempty"` // the heading that table stands under („Anlage 2.1 Regelstudienplan …")
}

// CurriculumTotal is a sum the regulation prints over rows of its own study
// plan: what the whole plan of a semester costs („Summe Studium"), or what a
// group of rows has to reach together („Summe Komplexe des Fachstudiums 44"
// over three rows that each say „10-24 LP"). Without it a plan with elective
// budgets cannot be added up: its rows only give a range.
type CurriculumTotal struct {
	Label          string  `json:"label"`
	Scope          string  `json:"scope"` // "plan": the whole plan of these semesters; "section": a part of it
	Specialization string  `json:"specialization,omitempty"`
	StartSemester  int     `json:"start_semester"`
	EndSemester    int     `json:"end_semester"`
	Credits        float64 `json:"credits"`     // what the plan prints; its lower bound where the plan prints a range
	CreditsMax     float64 `json:"credits_max"` // the upper bound of that range, equal to Credits where the plan prints a number
	MinCredits     float64 `json:"min_credits"` // what the rows inside these semesters come to …
	MaxCredits     float64 `json:"max_credits"` // … and what they and a row reaching into them could come to
	IsChoice       bool    `json:"is_choice,omitempty"`
	SourceEvidence string  `json:"source_evidence,omitempty"`
	Entries        []int   `json:"entries"` // 1-based positions of the rows it counts
}
