package model

import (
	"encoding/json"
	"regexp"
	"strings"
	"time"
)

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

// ResponsiblePerson represents an instructor or module coordinator with title and name separated.
type ResponsiblePerson struct {
	Title string `json:"title,omitempty"` // e.g. "Prof. Dr.", "Dr. rer. nat."
	Name  string `json:"name"`            // e.g. "Köhler, Ekkehard"
	Raw   string `json:"raw,omitempty"`   // original raw string
}

func (r ResponsiblePerson) FullName() string {
	if r.Title != "" {
		return r.Title + " " + r.Name
	}
	return r.Name
}

func (r ResponsiblePerson) String() string {
	return r.FullName()
}

// UnmarshalJSON supports both plain string (legacy JSON/DB) and structured object.
func (r *ResponsiblePerson) UnmarshalJSON(data []byte) error {
	var s string
	if err := json.Unmarshal(data, &s); err == nil {
		*r = SplitResponsiblePerson(s)
		return nil
	}
	type Alias ResponsiblePerson
	var a Alias
	if err := json.Unmarshal(data, &a); err != nil {
		return err
	}
	*r = ResponsiblePerson(a)
	return nil
}

var (
	// titleTokenRegex matches any individual title word/token (ignoring surrounding dots/hyphens)
	titleTokenRegex = regexp.MustCompile(`(?i)^(?:` +
		// Professor & Dozent variations
		`apl|außerplanmäßige[rn]?|hon|honorarprof(?:essor(?:in)?)?|jun|juniorprof(?:essor(?:in)?)?|univ|universitätsprof(?:essor(?:in)?)?|gast|gastprof(?:essor(?:in)?)?|senior|seniorprof(?:essor(?:in)?)?|vertr|vertretungsprof(?:essor(?:in)?)?|prof(?:essor(?:in)?)?|pd|priv|doz(?:ent(?:in)?)?|` +
		// Doctor variations
		`dr|dres|` +
		// Disciplines & syllables (rer, nat, publ, pol, oec, phil, med, ing, etc.)
		`rer|nat|publ|pol|oec|phil|med|medic|iur|jur|soc|agr|silv|techn|paed|päd|theol|sc|scient|disc|ing|` +
		// Habilitation
		`habil|` +
		// Honorary / mult
		`h|c|hc|mult|` +
		// Diplom & Degrees
		`dipl(?:-[a-zäöüß]+)*|msc|bsc|meng|beng|ma|ba|mba|llm` +
		`)$`)
	spacesRegex   = regexp.MustCompile(`\s+`)
	spaceDotRegex = regexp.MustCompile(`\s+\.`)
	drIngRegex    = regexp.MustCompile(`Dr\.\s*-\s*Ing`)
	diplIngRegex  = regexp.MustCompile(`Dipl\.\s*-\s*Ing`)
)

func isTitleToken(token string) bool {
	clean := strings.Trim(token, ".,- \t")
	if clean == "" {
		return true // punctuation between words
	}
	// Also allow parenthesized qualifiers like (NMU, UA) or (I)
	if strings.HasPrefix(clean, "(") && strings.HasSuffix(clean, ")") {
		return true
	}
	// Check compound dot- or hyphen-separated words like Dr.h.c. or Prof.Dr.rer.nat. or Dipl.-Ing
	subParts := strings.FieldsFunc(clean, func(r rune) bool {
		return r == '.' || r == '-' || r == ' '
	})
	if len(subParts) > 1 {
		allMatch := true
		for _, sp := range subParts {
			sp = strings.TrimSpace(sp)
			if sp != "" && !titleTokenRegex.MatchString(sp) {
				allMatch = false
				break
			}
		}
		if allMatch {
			return true
		}
	}
	return titleTokenRegex.MatchString(clean)
}

func cleanTitle(t string) string {
	t = strings.TrimSpace(t)
	t = spacesRegex.ReplaceAllString(t, " ")
	t = spaceDotRegex.ReplaceAllString(t, ".")
	t = drIngRegex.ReplaceAllString(t, "Dr.-Ing")
	t = diplIngRegex.ReplaceAllString(t, "Dipl.-Ing")
	t = strings.TrimRight(t, ", ")
	return t
}

// SplitResponsiblePerson separates academic titles from a person's name.
func SplitResponsiblePerson(raw string) ResponsiblePerson {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return ResponsiblePerson{}
	}

	// 1. Check for suffix title (e.g. "Schmid, Reiner , Prof. Dr. rer. nat.")
	var suffixTitle string
	workingRaw := raw
	parts := strings.Split(raw, ",")
	if len(parts) >= 2 {
		lastPart := strings.TrimSpace(parts[len(parts)-1])
		words := strings.Fields(lastPart)
		if len(words) > 0 {
			allTitle := true
			for _, w := range words {
				if !isTitleToken(w) {
					allTitle = false
					break
				}
			}
			if allTitle {
				suffixTitle = lastPart
				var nameParts []string
				for _, np := range parts[:len(parts)-1] {
					if t := strings.TrimSpace(np); t != "" {
						nameParts = append(nameParts, t)
					}
				}
				workingRaw = strings.Join(nameParts, ", ")
			}
		}
	}

	// 2. Check for prefix title in workingRaw
	words := strings.Fields(workingRaw)
	if len(words) == 0 {
		return ResponsiblePerson{
			Title: cleanTitle(suffixTitle),
			Raw:   raw,
		}
	}

	var titleWords []string
	var nameWords []string
	titlePhase := true

	for i := 0; i < len(words); i++ {
		w := words[i]
		if titlePhase {
			// Check if this token starts a parenthesized group (e.g. "(NMU, UA)" or "(I)")
			if strings.HasPrefix(w, "(") {
				parenGroup := []string{w}
				for !strings.HasSuffix(parenGroup[len(parenGroup)-1], ")") && i+1 < len(words) {
					i++
					parenGroup = append(parenGroup, words[i])
				}
				titleWords = append(titleWords, strings.Join(parenGroup, " "))
				continue
			}

			if isTitleToken(w) {
				titleWords = append(titleWords, w)
			} else {
				titlePhase = false
				nameWords = append(nameWords, words[i:]...)
				break
			}
		}
	}

	if len(titleWords) == 0 && suffixTitle == "" {
		return ResponsiblePerson{
			Name: raw,
			Raw:  raw,
		}
	}

	prefixTitle := strings.Join(titleWords, " ")
	name := strings.Join(nameWords, " ")

	var fullTitle string
	if prefixTitle != "" && suffixTitle != "" {
		fullTitle = cleanTitle(prefixTitle + " " + suffixTitle)
	} else if prefixTitle != "" {
		fullTitle = cleanTitle(prefixTitle)
	} else {
		fullTitle = cleanTitle(suffixTitle)
	}

	name = strings.Trim(name, ", ")

	return ResponsiblePerson{
		Title: fullTitle,
		Name:  name,
		Raw:   raw,
	}
}

// ModuleDetail represents full course/module details parsed from b-tu.de/modul/<id>.
type ModuleDetail struct {
	ID                       string              `json:"id"`
	Code                     string              `json:"code"`
	TitleDE                  string              `json:"title_de"`
	TitleEN                  string              `json:"title_en,omitempty"`
	IsPhaseOut               bool                `json:"is_phase_out"`
	IsNotOffered             bool                `json:"is_not_offered"` // Modul nicht mehr im Angebot
	Department               string              `json:"department,omitempty"`
	ResponsiblePersons       []ResponsiblePerson `json:"responsible_persons,omitempty"`
	SuccessorModules         []string            `json:"successor_modules,omitempty"`   // IDs of successor module(s)
	PredecessorModules       []string            `json:"predecessor_modules,omitempty"` // IDs of the module(s) this one replaces
	Language                 string              `json:"language,omitempty"`
	Duration                 string              `json:"duration,omitempty"`
	Turnus                   string              `json:"turnus,omitempty"` // frequency of offer
	Credits                  float64             `json:"credits,omitempty"`
	CreditsRaw               string              `json:"credits_raw,omitempty"`
	LearningOutcomes         string              `json:"learning_outcomes,omitempty"`
	Contents                 string              `json:"contents,omitempty"`
	PrerequisitesRecommended string              `json:"prerequisites_recommended,omitempty"`
	PrerequisitesMandatory   string              `json:"prerequisites_mandatory,omitempty"`
	TeachingForms            []TeachingForm      `json:"teaching_forms,omitempty"`
	Literature               []string            `json:"literature,omitempty"`
	ExamType                 string              `json:"exam_type,omitempty"`
	ExamDetails              string              `json:"exam_details,omitempty"`
	Grading                  string              `json:"grading,omitempty"`
	Limitation               string              `json:"limitation,omitempty"`
	StudyPrograms            []StudyProgram      `json:"study_programs,omitempty"`
	Remarks                  string              `json:"remarks,omitempty"`
	AssociatedCourses        []string            `json:"associated_courses,omitempty"`
	CurrentSemesterEvents    []ModuleEvent       `json:"current_semester_events,omitempty"`
	CrossDisciplinary        bool                `json:"cross_disciplinary"` // zugelassen für fachübergreifendes Studium
	IsFUES                   bool                `json:"is_fues"`
	RawURL                   string              `json:"raw_url"`
	LastScrapedAt            time.Time           `json:"last_scraped_at"`
	// Markdown holds the free texts above as the catalog keeps them; the plain ones stay what
	// facts are read from (the exam's kinds, the programs a remark names, the prerequisites).
	Markdown ModuleTexts `json:"markdown"`
}

// ModuleTexts are the free texts of a module description as CommonMark (parser.Markdown):
// paragraphs, lists, strong and emphasized text, as the page sets them (docs/schema-v2.md §3,
// „Module texts").
type ModuleTexts struct {
	LearningOutcomes         string `json:"learning_outcomes,omitempty"`
	Contents                 string `json:"contents,omitempty"`
	PrerequisitesRecommended string `json:"prerequisites_recommended,omitempty"`
	PrerequisitesMandatory   string `json:"prerequisites_mandatory,omitempty"`
	ExamDetails              string `json:"exam_details,omitempty"`
	Remarks                  string `json:"remarks,omitempty"`
}
