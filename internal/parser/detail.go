package parser

import (
	"fmt"
	"io"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/jakob/btu-scraper/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

var (
	creditNumberRegex = regexp.MustCompile(`([0-9]+(?:[\.,][0-9]+)?)`)
)

// DetailParser parses a BTU course detail page (b-tu.de/modul/<id>).
type DetailParser struct{}

// NewDetailParser creates a new DetailParser.
func NewDetailParser() *DetailParser {
	return &DetailParser{}
}

// Parse parses the HTML input for a specific course page.
func (p *DetailParser) Parse(r io.Reader, fallbackID, pageURL string) (*model.ModuleDetail, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	detail := &model.ModuleDetail{
		ID:            fallbackID,
		Code:          fallbackID,
		RawURL:        pageURL,
		LastScrapedAt: time.Now().UTC(),
	}

	// 1. Extract H1 header if present: "11101 - Lineare Algebra und analytische Geometrie I"
	h1 := FindFirstByTag(doc, atom.H1)
	if h1 != nil {
		h1Text := CleanSingleLine(NodeText(h1))
		// Remove sub-tags like <small>Modulübersicht</small> from title if needed
		if idx := strings.Index(strings.ToLower(h1Text), "modulübersicht"); idx != -1 {
			h1Text = strings.TrimSpace(h1Text[:idx])
		}
		if idx := strings.Index(strings.ToLower(h1Text), "module overview"); idx != -1 {
			h1Text = strings.TrimSpace(h1Text[:idx])
		}

		if parts := strings.SplitN(h1Text, " - ", 2); len(parts) == 2 {
			if detail.ID == "" || detail.ID == fallbackID {
				detail.ID = strings.TrimSpace(parts[0])
				detail.Code = detail.ID
			}
			detail.TitleDE = strings.TrimSpace(parts[1])
		} else if h1Text != "" {
			detail.TitleDE = h1Text
		}
	}

	// 2. Find detail table
	// Look for table inside div.tx-btusysteme
	var table *html.Node
	txBtusysteme := FindAllByClass(doc, "tx-btusysteme")
	for _, container := range txBtusysteme {
		if t := FindFirstByTag(container, atom.Table); t != nil {
			table = t
			break
		}
	}
	if table == nil {
		table = FindFirstByTag(doc, atom.Table)
	}

	if table == nil {
		return detail, nil
	}

	rows := FindAllByTag(table, atom.Tr)
	var prevKey string

	for _, tr := range rows {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) < 2 {
			// Single cell row or malformed row
			if len(tds) == 1 {
				singleTxt := CleanSingleLine(NodeText(tds[0]))
				if strings.Contains(strings.ToLower(singleTxt), "fachübergreifende studium zugelassen") ||
					strings.Contains(strings.ToLower(singleTxt), "cross-disciplinary") {
					detail.CrossDisciplinary = true
				}
			}
			continue
		}

		rawKey := CleanSingleLine(NodeText(tds[0]))
		valNode := tds[1]
		valText := CleanText(valNode)
		valSingle := CleanSingleLine(NodeText(valNode))

		normKey := normalizeKey(rawKey)

		// Handle secondary title row right under title row
		if (normKey == "" || rawKey == "") && (prevKey == "title" || prevKey == "moduletitle") {
			if valSingle != "" {
				if detail.TitleEN == "" {
					detail.TitleEN = valSingle
				} else if detail.TitleDE == "" {
					detail.TitleDE = valSingle
				}
			}
			continue
		}

		if normKey == "" && strings.Contains(strings.ToLower(valSingle), "fachübergreifende studium zugelassen") {
			detail.CrossDisciplinary = true
			continue
		}

		prevKey = normKey

		switch normKey {
		case "modulenumber":
			lowVal := strings.ToLower(valSingle)
			if strings.Contains(lowVal, "phase-out") ||
				strings.Contains(lowVal, "auslauf") {
				detail.IsPhaseOut = true
			}
			// Extract clean module number
			cleanNum := strings.TrimSpace(strings.Split(valSingle, "-")[0])
			if cleanNum != "" {
				detail.ID = cleanNum
				detail.Code = cleanNum
			}

		case "moduletitle":
			if detail.TitleDE == "" {
				detail.TitleDE = valSingle
			} else if detail.TitleEN == "" && detail.TitleDE != valSingle {
				detail.TitleEN = valSingle
			}

		case "department":
			detail.Department = valSingle

		case "responsible":
			items := ExtractListItems(valNode)
			if len(items) > 0 {
				detail.ResponsiblePersons = items
			} else if valSingle != "" {
				detail.ResponsiblePersons = []string{valSingle}
			}

		case "language":
			detail.Language = valSingle

		case "duration":
			detail.Duration = valSingle

		case "turnus":
			detail.Turnus = valSingle

		case "credits":
			detail.CreditsRaw = valSingle
			detail.Credits = parseCredits(valSingle)

		case "learningoutcomes":
			detail.LearningOutcomes = valText

		case "contents":
			detail.Contents = valText

		case "prerequisitesrecommended":
			detail.PrerequisitesRecommended = valText

		case "prerequisitesmandatory":
			detail.PrerequisitesMandatory = valText

		case "teachingforms":
			items := ExtractListItems(valNode)
			for _, item := range items {
				parts := strings.SplitN(item, "/", 2)
				tf := model.TeachingForm{}
				if len(parts) == 2 {
					tf.Type = strings.TrimSpace(parts[0])
					tf.Workload = strings.TrimSpace(parts[1])
				} else {
					tf.Type = strings.TrimSpace(parts[0])
				}
				detail.TeachingForms = append(detail.TeachingForms, tf)
			}

		case "literature":
			items := ExtractListItems(valNode)
			if len(items) > 0 {
				detail.Literature = items
			} else if valSingle != "" {
				detail.Literature = []string{valSingle}
			}

		case "moduleexam":
			detail.ExamType = valSingle

		case "examdetails":
			detail.ExamDetails = valText

		case "grading":
			detail.Grading = valSingle

		case "limitation":
			detail.Limitation = valSingle

		case "studyprograms":
			items := ExtractListItems(valNode)
			for _, item := range items {
				sp := parseStudyProgram(item)
				detail.StudyPrograms = append(detail.StudyPrograms, sp)
			}

		case "remarks":
			detail.Remarks = valText
			if strings.Contains(strings.ToLower(valText), "auslaufmodul") || strings.Contains(strings.ToLower(valText), "phase-out module") {
				detail.IsPhaseOut = true
			}

		case "nachfolgemodul":
			detail.IsPhaseOut = true

		case "courses":
			items := ExtractListItems(valNode)
			if len(items) > 0 {
				detail.AssociatedCourses = items
			} else if valSingle != "" {
				detail.AssociatedCourses = []string{valSingle}
			}

		case "currentevents":
			lis := FindAllByTag(valNode, atom.Li)
			for _, li := range lis {
				a := FindFirstByTag(li, atom.A)
				evt := model.ModuleEvent{
					Title: CleanSingleLine(NodeText(li)),
				}
				if a != nil {
					evt.URL = GetAttr(a, "href")
					aText := CleanSingleLine(NodeText(a))
					if aText != "" {
						evt.Title = aText
					}
				}
				if evt.Title != "" {
					detail.CurrentSemesterEvents = append(detail.CurrentSemesterEvents, evt)
				}
			}
		}
	}

	return detail, nil
}

func normalizeKey(raw string) string {
	raw = strings.TrimSuffix(strings.TrimSpace(raw), ":")
	k := strings.ToLower(raw)
	k = strings.ReplaceAll(k, " ", "")
	k = strings.ReplaceAll(k, "-", "")
	k = strings.ReplaceAll(k, "_", "")
	k = strings.ReplaceAll(k, "/", "")
	k = strings.ReplaceAll(k, "(", "")
	k = strings.ReplaceAll(k, ")", "")

	switch {
	case strings.Contains(k, "modulnummer") || strings.Contains(k, "modulenumber"):
		return "modulenumber"
	case strings.Contains(k, "modultitel") || strings.Contains(k, "moduletitle"):
		return "moduletitle"
	case strings.Contains(k, "einrichtung") || strings.Contains(k, "department") || strings.Contains(k, "faculty"):
		return "department"
	case strings.Contains(k, "verantwortlich") || strings.Contains(k, "responsiblestaff"):
		return "responsible"
	case strings.Contains(k, "sprache") || strings.Contains(k, "language"):
		return "language"
	case strings.Contains(k, "dauer") || strings.Contains(k, "duration"):
		return "duration"
	case strings.Contains(k, "turnus") || strings.Contains(k, "frequency"):
		return "turnus"
	case strings.Contains(k, "leistungspunkte") || strings.Contains(k, "credits") || strings.Contains(k, "ects"):
		return "credits"
	case strings.Contains(k, "lernziele") || strings.Contains(k, "learningoutcome"):
		return "learningoutcomes"
	case strings.Contains(k, "inhalte") || strings.Contains(k, "contents"):
		return "contents"
	case strings.Contains(k, "empfohlenevoraussetzungen") || strings.Contains(k, "recommendedprerequisites"):
		return "prerequisitesrecommended"
	case strings.Contains(k, "zwingendevoraussetzungen") || strings.Contains(k, "mandatoryprerequisites"):
		return "prerequisitesmandatory"
	case strings.Contains(k, "lehrformen") || strings.Contains(k, "formsofteaching"):
		return "teachingforms"
	case strings.Contains(k, "literatur") || strings.Contains(k, "instructionmaterial"):
		return "literature"
	case strings.Contains(k, "modulprüfung") || strings.Contains(k, "modulpruefung") || strings.Contains(k, "moduleexam"):
		return "moduleexam"
	case strings.Contains(k, "prüfungsleistung") || strings.Contains(k, "pruefungsleistung") || strings.Contains(k, "assessmentmethod"):
		return "examdetails"
	case strings.Contains(k, "bewertung") || strings.Contains(k, "grading"):
		return "grading"
	case strings.Contains(k, "teilnehmerbeschränkung") || strings.Contains(k, "teilnehmerbeschraenkung") || strings.Contains(k, "limitationofparticipation"):
		return "limitation"
	case strings.Contains(k, "studiengängen") || strings.Contains(k, "studiengaengen") || strings.Contains(k, "associatedstudyprogrammes"):
		return "studyprograms"
	case strings.Contains(k, "bemerkungen") || strings.Contains(k, "remarks"):
		return "remarks"
	case strings.Contains(k, "veranstaltungenzummodul") || strings.Contains(k, "coursesformodule"):
		return "courses"
	case strings.Contains(k, "veranstaltungenimaktuellen") || strings.Contains(k, "coursesincurrentsemester"):
		return "currentevents"
	case strings.Contains(k, "nachfolge"):
		return "nachfolgemodul"
	default:
		return ""
	}
}

func parseCredits(s string) float64 {
	m := creditNumberRegex.FindString(s)
	if m == "" {
		return 0
	}
	m = strings.ReplaceAll(m, ",", ".")
	f, _ := strconv.ParseFloat(m, 64)
	return f
}

func parseStudyProgram(raw string) model.StudyProgram {
	sp := model.StudyProgram{Raw: raw}
	parts := strings.Split(raw, "/")
	if len(parts) >= 3 {
		sp.Degree = strings.TrimSpace(parts[0])
		sp.Program = strings.TrimSpace(parts[1])
		sp.Regulation = strings.TrimSpace(parts[2])
	} else if len(parts) == 2 {
		sp.Degree = strings.TrimSpace(parts[0])
		sp.Program = strings.TrimSpace(parts[1])
	} else {
		sp.Program = strings.TrimSpace(raw)
	}
	return sp
}
