package parser

import (
	"fmt"
	"io"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

var (
	creditNumberRegex = regexp.MustCompile(`([0-9]+(?:[\.,][0-9]+)?)`)
	reModuleID        = regexp.MustCompile(`\b\d{5}\b`)
	reNachfolge       = regexp.MustCompile(`(?i)Nachfolge(?:modul(?:e)?)?[^\d\n]{0,50}(\d{5})`)
	// reSucceeds is where a remark names the module this one succeeds: „Nachfolgemodul zu
	// 31423", „Nachfolgemodul für Modul 24410", „ein Nachfolgemodul des Moduls …".
	reSucceeds = regexp.MustCompile(`(?i)Nachfolge-?\s?modul(?:e)?\s+(?:zu|zum|für|fuer|von|vom|des|der)\s`)
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
		ID:                       fallbackID,
		Code:                     fallbackID,
		RawURL:                   pageURL,
		LastScrapedAt:            time.Now().UTC(),
		PrerequisitesRecommended: "-",
		PrerequisitesMandatory:   "-",
	}

	// 1. Extract H1 header if present: "11101 - Lineare Algebra und analytische Geometrie I"
	h1 := FindFirstByTag(doc, atom.H1)
	if h1 != nil {
		h1Text := CleanSingleLine(NodeText(h1))
		if containsNotOffered(h1Text) {
			detail.IsNotOffered = true
			detail.IsPhaseOut = true
		}

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
				if isCrossDisciplinaryNote(singleTxt) {
					detail.CrossDisciplinary = true
				}
				if containsNotOffered(singleTxt) {
					detail.IsNotOffered = true
					detail.IsPhaseOut = true
				}
			}
			continue
		}

		applyRow(detail, CleanSingleLine(NodeText(tds[0])), tds[1], &prevKey)
	}

	// Final normalization for prerequisites if not set
	if detail.PrerequisitesRecommended == "" {
		detail.PrerequisitesRecommended = "-"
	}
	if detail.PrerequisitesMandatory == "" {
		detail.PrerequisitesMandatory = "-"
	}

	return detail, nil
}

func containsNotOffered(s string) bool {
	low := strings.ToLower(s)
	return strings.Contains(low, "nicht mehr im angebot") ||
		strings.Contains(low, "kein lehrangebot mehr") ||
		strings.Contains(low, "kein angebot mehr") ||
		strings.Contains(low, "vorerst kein lehrangebot") ||
		strings.Contains(low, "derzeit kein lehrangebot") ||
		strings.Contains(low, "no longer offered")
}

// isCrossDisciplinaryNote matches the unlabelled row that approves a module for FÜS.
func isCrossDisciplinaryNote(s string) bool {
	low := strings.ToLower(s)
	return strings.Contains(low, "fachübergreifende studium zugelassen") ||
		strings.Contains(low, "approved for the general studies") ||
		strings.Contains(low, "cross-disciplinary")
}

// IsNoAssignment matches the placeholder shown instead of an empty list
// („keine Zuordnung vorhanden" / "no assignment").
func IsNoAssignment(s string) bool {
	low := strings.ToLower(s)
	return strings.Contains(low, "keine zuordnung vorhanden") || strings.Contains(low, "no assignment")
}

func normalizePrereqText(s string) string {
	trimmed := strings.TrimSpace(s)
	low := strings.ToLower(trimmed)
	if trimmed == "" || low == "keine" || low == "none" || low == "-" || low == "entfällt" || low == "k.a." || low == "keine." || low == "nein" {
		return "-"
	}
	return trimmed
}

// linkedModuleIDs reads the module numbers a row of successors or of replaced modules
// names. Only b-tu.de/modul/<id> addresses a module by its number; a QIS link carries the
// internal number of the description, which is five digits too and would name a module
// that does not exist, so a QIS link counts by its text („38105 Allgemeine
// Betriebswirtschaftslehre I").
func linkedModuleIDs(valNode *html.Node, valText string) []string {
	var ids []string
	for _, l := range FindAllByTag(valNode, atom.A) {
		href := GetAttr(l, "href")
		if !strings.Contains(href, "/modul/") {
			continue
		}
		if m := reModuleID.FindString(href); m != "" {
			ids = appendUnique(ids, m)
		}
	}
	for _, m := range reModuleID.FindAllString(valText, -1) {
		ids = appendUnique(ids, m)
	}
	return ids
}

func appendUnique(slice []string, val string) []string {
	for _, s := range slice {
		if s == val {
			return slice
		}
	}
	return append(slice, val)
}

// applyRow maps one label/value row of a module description onto detail. The CMS
// page on b-tu.de and the QIS page it copies carry the same labels in the same
// shapes, so both parsers read a row through this function.
func applyRow(detail *model.ModuleDetail, rawKey string, valNode *html.Node, prevKey *string) {
	valText := CleanText(valNode)
	valSingle := CleanSingleLine(NodeText(valNode))

	if containsNotOffered(valSingle) || containsNotOffered(rawKey) {
		detail.IsNotOffered = true
		detail.IsPhaseOut = true
	}

	normKey := normalizeKey(rawKey)

	// Handle secondary title row right under title row
	if (normKey == "" || rawKey == "") && (*prevKey == "title" || *prevKey == "moduletitle") {
		if valSingle != "" {
			if detail.TitleEN == "" {
				detail.TitleEN = valSingle
			} else if detail.TitleDE == "" {
				detail.TitleDE = valSingle
			}
		}
		return
	}

	if normKey == "" && isCrossDisciplinaryNote(valSingle) {
		detail.CrossDisciplinary = true
		return
	}

	*prevKey = normKey

	switch normKey {
	case "modulenumber":
		lowVal := strings.ToLower(valSingle)
		if strings.Contains(lowVal, "phase-out") ||
			strings.Contains(lowVal, "auslauf") ||
			containsNotOffered(lowVal) {
			detail.IsPhaseOut = true
		}
		if containsNotOffered(lowVal) {
			detail.IsNotOffered = true
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
		var respList []model.ResponsiblePerson
		if len(items) > 0 {
			for _, it := range items {
				respList = append(respList, model.SplitResponsiblePerson(it))
			}
		} else if valSingle != "" {
			respList = append(respList, model.SplitResponsiblePerson(valSingle))
		}
		detail.ResponsiblePersons = respList

	case "language":
		detail.Language = valSingle

	case "duration":
		detail.Duration = valSingle

	case "turnus":
		detail.Turnus = valSingle
		if containsNotOffered(valSingle) {
			detail.IsNotOffered = true
			detail.IsPhaseOut = true
		}

	case "credits":
		detail.CreditsRaw = valSingle
		detail.Credits = parseCredits(valSingle)

	case "learningoutcomes":
		detail.LearningOutcomes = valText
		detail.Markdown.LearningOutcomes = Markdown(valNode)

	case "contents":
		detail.Contents = valText
		detail.Markdown.Contents = Markdown(valNode)

	case "prerequisitesrecommended":
		detail.PrerequisitesRecommended = normalizePrereqText(valText)
		detail.Markdown.PrerequisitesRecommended = Markdown(valNode)

	case "prerequisitesmandatory":
		detail.PrerequisitesMandatory = normalizePrereqText(valText)
		detail.Markdown.PrerequisitesMandatory = Markdown(valNode)

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
		detail.Markdown.ExamDetails = Markdown(valNode)

	case "grading":
		detail.Grading = valSingle

	case "limitation":
		detail.Limitation = valSingle

	case "studyprograms":
		items := ExtractListItems(valNode)
		if len(items) == 0 && valSingle != "" {
			items = []string{valSingle}
		}
		for _, item := range items {
			detail.StudyPrograms = append(detail.StudyPrograms, parseStudyProgram(item))
		}

	case "remarks":
		detail.Remarks = valText
		detail.Markdown.Remarks = Markdown(valNode)
		lowRemarks := strings.ToLower(valText)
		if strings.Contains(lowRemarks, "auslaufmodul") || strings.Contains(lowRemarks, "phase-out module") || containsNotOffered(lowRemarks) {
			detail.IsPhaseOut = true
		}
		if containsNotOffered(lowRemarks) {
			detail.IsNotOffered = true
		}
		// „Siehe Nachfolge-Modul 11523", „stattdessen Nachfolgemodul 11787": the module has a
		// successor. „Nachfolgemodul zu 31423": it is the successor of the module named, and
		// does not phase out for saying so.
		if n := strings.Count(lowRemarks, "nachfolge"); n > 0 {
			if len(reSucceeds.FindAllStringIndex(valText, -1)) < n {
				detail.IsPhaseOut = true
			}
			for _, m := range reNachfolge.FindAllStringSubmatchIndex(valText, -1) {
				id := valText[m[2]:m[3]]
				if id == detail.ID {
					continue
				}
				if loc := reSucceeds.FindStringIndex(valText[m[0]:]); loc != nil && loc[0] == 0 {
					detail.PredecessorModules = appendUnique(detail.PredecessorModules, id)
				} else {
					detail.SuccessorModules = appendUnique(detail.SuccessorModules, id)
				}
			}
		}

	case "nachfolgemodul":
		// This module phases out („Auslaufmodul ab: 21.04.2017"); the row links its successors.
		detail.IsPhaseOut = true
		for _, m := range linkedModuleIDs(valNode, valText) {
			if m != detail.ID {
				detail.SuccessorModules = appendUnique(detail.SuccessorModules, m)
			}
		}

	case "auslaufmodul":
		// This module is the successor („Nachfolgemodul seit: 21.04.2017") of the modules
		// the row links, which QIS marks in pink. It does not phase out itself.
		for _, m := range linkedModuleIDs(valNode, valText) {
			if m != detail.ID {
				detail.PredecessorModules = appendUnique(detail.PredecessorModules, m)
			}
		}

	case "courses":
		items := ExtractListItems(valNode)
		if len(items) > 0 {
			detail.AssociatedCourses = items
		} else if valSingle != "" {
			detail.AssociatedCourses = []string{valSingle}
		}

	case "currentevents":
		if IsNoAssignment(valSingle) {
			// Explicitly no event
			break
		}

		lis := FindAllByTag(valNode, atom.Li)
		for _, li := range lis {
			a := FindFirstByTag(li, atom.A)
			if a == nil {
				// "If there are no links to qis then you can assume there is no event"
				continue
			}
			href := GetAttr(a, "href")
			if href == "" || href == "#" {
				continue
			}
			aText := CleanSingleLine(NodeText(a))
			if aText == "" {
				aText = CleanSingleLine(NodeText(li))
			}
			if IsNoAssignment(aText) {
				continue
			}
			detail.CurrentSemesterEvents = append(detail.CurrentSemesterEvents, model.ModuleEvent{
				Title: aText,
				URL:   href,
			})
		}

		// If no <li> found, check if there are <a> tags directly in valNode
		if len(detail.CurrentSemesterEvents) == 0 {
			aNodes := FindAllByTag(valNode, atom.A)
			for _, a := range aNodes {
				href := GetAttr(a, "href")
				if href == "" || href == "#" {
					continue
				}
				aText := CleanSingleLine(NodeText(a))
				if aText != "" && !IsNoAssignment(aText) {
					detail.CurrentSemesterEvents = append(detail.CurrentSemesterEvents, model.ModuleEvent{
						Title: aText,
						URL:   href,
					})
				}
			}
		}
	}
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
	case strings.Contains(k, "empfohlenevoraussetzungen") || strings.Contains(k, "empfohlenen") || strings.Contains(k, "empfohlenevorkenntnisse") || strings.Contains(k, "inhaltlichevoraussetzungen") || strings.Contains(k, "vorkenntnisse") || strings.Contains(k, "recommendedprerequisites") || strings.Contains(k, "recommended"):
		return "prerequisitesrecommended"
	case strings.Contains(k, "zwingendevoraussetzungen") || strings.Contains(k, "zwingenden") || strings.Contains(k, "formalevoraussetzungen") || strings.Contains(k, "verpflichtendevoraussetzungen") || strings.Contains(k, "mandatoryprerequisites") || strings.Contains(k, "mandatory"):
		return "prerequisitesmandatory"
	case strings.Contains(k, "voraussetzungen") || strings.Contains(k, "prerequisites"):
		return "prerequisitesmandatory"
	case strings.Contains(k, "lehrformen") || strings.Contains(k, "formsofteaching"):
		return "teachingforms"
	case strings.Contains(k, "literatur") || strings.Contains(k, "instructionmaterial"):
		return "literature"
	// „Prüfungsleistung/en für Modulprüfung" and „Bewertung der Modulprüfung" (EN: "Assessment
	// Mode for …" / "Evaluation of Module Examination") contain the plain exam label, so they
	// have to be tested before it.
	case strings.Contains(k, "prüfungsleistung") || strings.Contains(k, "pruefungsleistung") || strings.Contains(k, "assessmentmode") || strings.Contains(k, "assessmentmethod"):
		return "examdetails"
	case strings.Contains(k, "bewertung") || strings.Contains(k, "evaluationof") || strings.Contains(k, "grading"):
		return "grading"
	case strings.Contains(k, "modulprüfung") || strings.Contains(k, "modulpruefung") || strings.Contains(k, "moduleexam"):
		return "moduleexam"
	case strings.Contains(k, "teilnehmerbeschränkung") || strings.Contains(k, "teilnehmerbeschraenkung") || strings.Contains(k, "limitednumberofparticipants") || strings.Contains(k, "limitationofparticipation"):
		return "limitation"
	case strings.Contains(k, "studiengängen") || strings.Contains(k, "studiengaengen") || strings.Contains(k, "studyprogramme"):
		return "studyprograms"
	case strings.Contains(k, "bemerkungen") || strings.Contains(k, "remarks"):
		return "remarks"
	case strings.Contains(k, "veranstaltungenimaktuellen") || strings.Contains(k, "currentsemester"):
		return "currentevents"
	case strings.Contains(k, "veranstaltungenzummodul") || strings.Contains(k, "modulecomponents") || strings.Contains(k, "coursesformodule"):
		return "courses"
	// A replacement is stated on both modules, each naming the other, and the label names
	// what the linked module is: the module that phases out links its successors under
	// „Nachfolgemodul/e" ("Follow-up Module/s"), its successor links it under
	// „Auslaufmodul" ("Phase-out Module"). The copy on b-tu.de carries the same rows.
	case strings.Contains(k, "nachfolge") || strings.Contains(k, "followupmodul"):
		return "nachfolgemodul"
	case strings.Contains(k, "auslaufmodul") || strings.Contains(k, "phaseoutmodul"):
		return "auslaufmodul"
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

// parseStudyProgram splits „Abschluss / Studiengang / PO". The separator is " / " with
// spaces: a bare slash belongs to the value („LA Bachelor Grundstufe/Primarstufe").
// A program name may itself contain " / ", so it is everything between first and last part.
func parseStudyProgram(raw string) model.StudyProgram {
	sp := model.StudyProgram{Raw: raw}
	parts := strings.Split(raw, " / ")
	if len(parts) >= 3 {
		sp.Degree = strings.TrimSpace(parts[0])
		sp.Program = strings.TrimSpace(strings.Join(parts[1:len(parts)-1], " / "))
		sp.Regulation = strings.TrimSpace(parts[len(parts)-1])
	} else if len(parts) == 2 {
		sp.Degree = strings.TrimSpace(parts[0])
		sp.Program = strings.TrimSpace(parts[1])
	} else {
		sp.Program = strings.TrimSpace(raw)
	}
	return sp
}
