package parser

import (
	"fmt"
	"io"
	"net/url"
	"regexp"
	"strings"
	"time"

	"github.com/jakob/btu-scraper/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

var (
	timeSplitRegex = regexp.MustCompile(`(?i)\s*bis\s*|\s*-\s*`)
)

// EventParser parses an event detail page from BTU QIS system.
type EventParser struct{}

// NewEventParser creates a new EventParser.
func NewEventParser() *EventParser {
	return &EventParser{}
}

// Parse extracts full event and schedule details from HTML.
func (p *EventParser) Parse(r io.Reader, fallbackID, pageURL string) (*model.EventDetail, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse event HTML: %w", err)
	}

	eventID := fallbackID
	if eventID == "" && pageURL != "" {
		eventID = extractVeranstID(pageURL)
	}

	detail := &model.EventDetail{
		ID:            eventID,
		RawURL:        pageURL,
		LastScrapedAt: time.Now().UTC(),
	}

	// 1. Title from H1
	h1 := FindFirstByTag(doc, atom.H1)
	if h1 != nil {
		// Remove img or tooltip divs inside H1
		title := CleanSingleLine(NodeText(h1))
		if idx := strings.Index(title, " - Einzelansicht"); idx != -1 {
			title = strings.TrimSpace(title[:idx])
		}
		if idx := strings.Index(title, " - Single view"); idx != -1 {
			title = strings.TrimSpace(title[:idx])
		}
		detail.Title = title
	}

	// 2. Process all tables by summary or caption
	tables := FindAllByTag(doc, atom.Table)
	for _, tbl := range tables {
		summary := strings.ToLower(GetAttr(tbl, "summary"))
		captionNode := FindFirstByTag(tbl, atom.Caption)
		caption := ""
		if captionNode != nil {
			caption = strings.ToLower(CleanSingleLine(NodeText(captionNode)))
		}

		switch {
		case strings.Contains(summary, "grunddaten") || strings.Contains(caption, "grunddaten"):
			p.parseGrunddaten(tbl, detail)

		case strings.Contains(summary, "veranstaltungstermine") || strings.Contains(caption, "termine"):
			p.parseTermine(tbl, detail)

		case strings.Contains(summary, "dozenten") || strings.Contains(caption, "zugeordnete person"):
			p.parsePersons(tbl, detail)

		case strings.Contains(summary, "prüfungen") || strings.Contains(caption, "gehört zu modul") || strings.Contains(caption, "gehoert zu modul"):
			p.parseModules(tbl, detail)

		case strings.Contains(summary, "studiengänge") || strings.Contains(caption, "studiengänge") || strings.Contains(summary, "studiengaenge"):
			p.parseStudyPrograms(tbl, detail)

		case strings.Contains(summary, "einrichtungen") || strings.Contains(caption, "zuordnung zu einrichtungen"):
			p.parseInstitutions(tbl, detail)

		case strings.Contains(summary, "weitere angaben") || strings.Contains(caption, "inhalt"):
			p.parseContent(tbl, detail)
		}
	}

	return detail, nil
}

func (p *EventParser) parseGrunddaten(tbl *html.Node, d *model.EventDetail) {
	rows := FindAllByTag(tbl, atom.Tr)
	for _, tr := range rows {
		ths := FindAllByTag(tr, atom.Th)
		tds := FindAllByTag(tr, atom.Td)

		// Th and Td may be paired: th[0]->td[0], th[1]->td[1]
		count := len(ths)
		if len(tds) < count {
			count = len(tds)
		}

		for i := 0; i < count; i++ {
			k := strings.ToLower(CleanSingleLine(NodeText(ths[i])))
			v := CleanSingleLine(NodeText(tds[i]))

			switch {
			case strings.Contains(k, "veranstaltungsart") || strings.Contains(k, "type"):
				d.EventType = v
			case strings.Contains(k, "veranstaltungsnummer") || strings.Contains(k, "number"):
				d.EventNumber = v
			case strings.Contains(k, "semester"):
				d.Semester = v
			case strings.Contains(k, "sws"):
				d.SWS = v
			case strings.Contains(k, "erwartete teilnehmer"):
				d.ExpectedParticipants = v
			case strings.Contains(k, "max. teilnehmer"):
				d.MaxParticipants = v
			case strings.Contains(k, "hyperlink"):
				a := FindFirstByTag(tds[i], atom.A)
				if a != nil {
					d.Hyperlink = GetAttr(a, "href")
				} else {
					d.Hyperlink = v
				}
			}
		}
	}
}

func (p *EventParser) parseTermine(tbl *html.Node, d *model.EventDetail) {
	groupName := ""
	captionNode := FindFirstByTag(tbl, atom.Caption)
	if captionNode != nil {
		capText := CleanSingleLine(NodeText(captionNode))
		if idx := strings.Index(capText, "Gruppe:"); idx != -1 {
			groupName = strings.TrimSpace(capText[idx+len("Gruppe:"):])
			// Strip any trailing export labels
			if endIdx := strings.Index(groupName, "iCalendar"); endIdx != -1 {
				groupName = strings.TrimSpace(groupName[:endIdx])
			}
		}
	}

	rows := FindAllByTag(tbl, atom.Tr)
	if len(rows) < 2 {
		return
	}

	// First row is headers
	var colHeaders []string
	thNodes := FindAllByTag(rows[0], atom.Th)
	for _, th := range thNodes {
		colHeaders = append(colHeaders, strings.ToLower(CleanSingleLine(NodeText(th))))
	}

	for _, tr := range rows[1:] {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) == 0 {
			continue
		}

		sched := model.EventSchedule{
			GroupName: groupName,
		}

		for idx, td := range tds {
			header := ""
			if idx < len(colHeaders) {
				header = colHeaders[idx]
			}

			val := CleanSingleLine(NodeText(td))

			switch {
			case strings.Contains(header, "tag") || strings.Contains(header, "day"):
				sched.DayOfWeek = val

			case strings.Contains(header, "zeit") || strings.Contains(header, "time"):
				sched.TimeSlot = val
				parts := timeSplitRegex.Split(val, 2)
				if len(parts) == 2 {
					sched.StartTime = strings.TrimSpace(parts[0])
					sched.EndTime = strings.TrimSpace(parts[1])
				} else if len(parts) == 1 {
					sched.StartTime = strings.TrimSpace(parts[0])
				}

			case strings.Contains(header, "rhythmus") || strings.Contains(header, "rhythm"):
				sched.Rhythm = val

			case strings.Contains(header, "dauer") || strings.Contains(header, "duration"):
				sched.Duration = val

			case strings.Contains(header, "raum") && !strings.Contains(header, "plan"):
				sched.Room = val
				a := FindFirstByTag(td, atom.A)
				if a != nil {
					sched.RoomURL = GetAttr(a, "href")
					aText := CleanSingleLine(NodeText(a))
					if aText != "" {
						sched.Room = aText
					}
				}

			case strings.Contains(header, "lehrperson") || strings.Contains(header, "person") || strings.Contains(header, "instructor"):
				sched.Instructor = val
				a := FindFirstByTag(td, atom.A)
				if a != nil {
					sched.InstructorURL = GetAttr(a, "href")
					aText := CleanSingleLine(NodeText(a))
					if aText != "" {
						sched.Instructor = aText
					}
				}

			case strings.Contains(header, "bemerkung") || strings.Contains(header, "comment"):
				sched.Comment = val

			case strings.Contains(header, "fällt aus") || strings.Contains(header, "cancelled"):
				sched.CancelledDates = val
			}
		}

		if sched.DayOfWeek != "" || sched.TimeSlot != "" || sched.Room != "" {
			d.Schedules = append(d.Schedules, sched)
		}
	}
}

func (p *EventParser) parsePersons(tbl *html.Node, d *model.EventDetail) {
	rows := FindAllByTag(tbl, atom.Tr)
	for _, tr := range rows {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) == 0 {
			continue
		}

		person := model.EventResponsiblePerson{}
		a := FindFirstByTag(tds[0], atom.A)
		if a != nil {
			person.Name = CleanSingleLine(NodeText(a))
			person.URL = GetAttr(a, "href")
		} else {
			person.Name = CleanSingleLine(NodeText(tds[0]))
		}

		if len(tds) > 1 {
			person.Role = CleanSingleLine(NodeText(tds[1]))
		}

		if person.Name != "" {
			d.ResponsiblePersons = append(d.ResponsiblePersons, person)
		}
	}
}

func (p *EventParser) parseModules(tbl *html.Node, d *model.EventDetail) {
	rows := FindAllByTag(tbl, atom.Tr)
	seen := make(map[string]bool)
	for _, tr := range rows {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) == 0 {
			continue
		}
		modNum := CleanSingleLine(NodeText(tds[0]))
		if modNum != "" && !seen[modNum] {
			seen[modNum] = true
			d.AssociatedModules = append(d.AssociatedModules, modNum)
		}
	}
}

func (p *EventParser) parseStudyPrograms(tbl *html.Node, d *model.EventDetail) {
	rows := FindAllByTag(tbl, atom.Tr)
	for _, tr := range rows {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) == 0 {
			continue
		}
		var parts []string
		for _, td := range tds {
			t := CleanSingleLine(NodeText(td))
			if t != "" {
				parts = append(parts, t)
			}
		}
		if len(parts) > 0 {
			d.StudyPrograms = append(d.StudyPrograms, strings.Join(parts, " | "))
		}
	}
}

func (p *EventParser) parseInstitutions(tbl *html.Node, d *model.EventDetail) {
	rows := FindAllByTag(tbl, atom.Tr)
	for _, tr := range rows {
		tds := FindAllByTag(tr, atom.Td)
		for _, td := range tds {
			inst := CleanSingleLine(NodeText(td))
			if inst != "" {
				d.Institutions = append(d.Institutions, inst)
			}
		}
	}
}

func (p *EventParser) parseContent(tbl *html.Node, d *model.EventDetail) {
	text := CleanText(tbl)
	if text != "" {
		d.Description = text
	}
}

func extractVeranstID(rawURL string) string {
	u, err := url.Parse(rawURL)
	if err != nil {
		return ""
	}
	val := u.Query().Get("veranstaltung.veranstid")
	if val != "" {
		return val
	}
	return u.Query().Get("veranstid")
}
