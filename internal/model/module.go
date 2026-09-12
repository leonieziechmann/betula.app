package model

import "time"

// ModuleSummary represents basic information discovered from the module catalog page.
type ModuleSummary struct {
	ID        string    `json:"id"`
	Code      string    `json:"code"`
	Title     string    `json:"title"`
	URL       string    `json:"url"`
	ScrapedAt time.Time `json:"scraped_at"`
}

// StudyProgram represents a degree programme associated with a module.
type StudyProgram struct {
	Degree            string `json:"degree,omitempty"`              // e.g. "Bachelor (universitär)"
	Program           string `json:"program,omitempty"`             // e.g. "Mathematik"
	Regulation        string `json:"regulation,omitempty"`          // e.g. "PO 2023"
	Raw               string `json:"raw"`                           // full raw line
	OfficialProgramID string `json:"official_program_id,omitempty"` // links to official_study_programs.id
}

// TeachingForm represents a form of teaching and its workload.
type TeachingForm struct {
	Type     string `json:"type"`     // e.g. "Vorlesung", "Übung", "Selbststudium"
	Workload string `json:"workload"` // e.g. "4 SWS" or "150 Stunden"
}

// ModuleEvent represents a lecture/seminar/exam event for the current semester.
type ModuleEvent struct {
	Title string `json:"title"`
	URL   string `json:"url,omitempty"`
}

// ModuleDetail represents full course/module details parsed from b-tu.de/modul/<id>.
type ModuleDetail struct {
	ID                       string         `json:"id"`
	Code                     string         `json:"code"`
	TitleDE                  string         `json:"title_de"`
	TitleEN                  string         `json:"title_en,omitempty"`
	IsPhaseOut               bool           `json:"is_phase_out"`
	Department               string         `json:"department,omitempty"`
	ResponsiblePersons       []string       `json:"responsible_persons,omitempty"`
	Language                 string         `json:"language,omitempty"`
	Duration                 string         `json:"duration,omitempty"`
	Turnus                   string         `json:"turnus,omitempty"` // frequency of offer
	Credits                  float64        `json:"credits,omitempty"`
	CreditsRaw               string         `json:"credits_raw,omitempty"`
	LearningOutcomes         string         `json:"learning_outcomes,omitempty"`
	Contents                 string         `json:"contents,omitempty"`
	PrerequisitesRecommended string         `json:"prerequisites_recommended,omitempty"`
	PrerequisitesMandatory   string         `json:"prerequisites_mandatory,omitempty"`
	TeachingForms            []TeachingForm `json:"teaching_forms,omitempty"`
	Literature               []string       `json:"literature,omitempty"`
	ExamType                 string         `json:"exam_type,omitempty"`
	ExamDetails              string         `json:"exam_details,omitempty"`
	Grading                  string         `json:"grading,omitempty"`
	Limitation               string         `json:"limitation,omitempty"`
	StudyPrograms            []StudyProgram `json:"study_programs,omitempty"`
	Remarks                  string         `json:"remarks,omitempty"`
	AssociatedCourses        []string       `json:"associated_courses,omitempty"`
	CurrentSemesterEvents    []ModuleEvent  `json:"current_semester_events,omitempty"`
	CrossDisciplinary        bool           `json:"cross_disciplinary"` // zugelassen für fachübergreifendes Studium
	IsFUES                   bool           `json:"is_fues"`
	RawURL                   string         `json:"raw_url"`
	LastScrapedAt            time.Time      `json:"last_scraped_at"`
}
