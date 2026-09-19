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
}
