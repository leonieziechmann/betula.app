package model

import "time"

// CurriculumModule represents an extracted module recommendation from an official study regulation (PO / SO).
type CurriculumModule struct {
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
	Credits                float64   `json:"credits"`
	ModuleType             string    `json:"module_type"` // "Pflicht", "Wahlpflicht", "Wahl", "FÜS", "Abschlussarbeit", "Praktikum"
	Specialization         string    `json:"specialization,omitempty"`
	SWS                    string    `json:"sws,omitempty"`
	ExamType               string    `json:"exam_type,omitempty"`
	Graded                 string    `json:"graded,omitempty"`
	Prerequisites          string    `json:"prerequisites,omitempty"`
	Remarks                string    `json:"remarks,omitempty"`
	SourceFile             string    `json:"source_file,omitempty"`
	ExtractedAt            time.Time `json:"extracted_at"`
}
