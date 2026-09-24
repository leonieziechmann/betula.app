package parser

import (
	"bytes"
	"fmt"
	"io"
	"net/url"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// The event search of QIS (state=wsearchv&search=1) prints, in its long view (P.vx=lang),
// every event it finds as one entry: its title, a line of basic data, and a table of dates
// per group. It filters by a comma-separated list of event IDs (veranstaltung.veranstid)
// across semesters, so one request states the dates of a few hundred events.
//
// An entry states everything the build reads from an event page but three things: the
// remark of a date (the „Bemerkung" column of the list holds the maximum of participants
// of the date instead), the first names and roles of the persons, and the campus of a
// room. The event page stays the source of those.

// UnnamedGroup is what an event page calls the dates that belong to no group. The list
// heads them „Termin" or „Termine"; both parsers name them alike.
const UnnamedGroup = "[unbenannt]"

// EventListEntry is the entry of one event, as it stands in the list.
type EventListEntry struct {
	ID   string // the QIS event ID (veranstid; the links of the list call it publishid)
	HTML []byte // the markup of the entry, which is what the archive keeps
}

// EventList is one page of the event search.
type EventList struct {
	Hits    int // how many events the search found, as the page states it
	Entries []EventListEntry
}

var reHits = regexp.MustCompile(`(\d+)\s+Treffer`)

// SplitEventList takes a page of the event search apart into its entries. A page that
// does not state its number of hits is not a result of the search (an error page, a
// login, a new layout); that is an error, not an empty result.
func SplitEventList(r io.Reader) (*EventList, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse the event list: %w", err)
	}
	m := reHits.FindStringSubmatch(CleanSingleLine(NodeText(FindFirstByClass(doc, "InfoLeiste"))))
	if m == nil {
		return nil, fmt.Errorf("the page states no number of hits; it is not a result of the event search, or its layout changed")
	}
	list := &EventList{}
	list.Hits, _ = strconv.Atoi(m[1])

	// Each entry follows a spacer of its own, and one more spacer ends the list.
	for _, spacer := range FindAllByClass(doc, "abstand_veranstaltung") {
		var nodes []*html.Node
		for n := spacer.NextSibling; n != nil && !(n.Type == html.ElementNode && HasClass(n, "abstand_veranstaltung")); n = n.NextSibling {
			nodes = append(nodes, n)
		}
		id := ""
		for _, n := range nodes {
			if link := eventLink(n); link != nil {
				id = eventLinkID(link)
				break
			}
		}
		if id == "" {
			continue // the end of the list, or the links to its further pages
		}
		var buf bytes.Buffer
		for _, n := range nodes {
			if err := html.Render(&buf, n); err != nil {
				return nil, fmt.Errorf("event %s: %w", id, err)
			}
		}
		list.Entries = append(list.Entries, EventListEntry{ID: id, HTML: bytes.TrimSpace(buf.Bytes())})
	}
	return list, nil
}

// eventLink finds the link to an event page below n: the title of an entry.
func eventLink(n *html.Node) *html.Node {
	for _, a := range FindAllByTag(n, atom.A) {
		if href := GetAttr(a, "href"); strings.Contains(href, "publishSubDir=veranstaltung") && eventLinkID(a) != "" {
			return a
		}
	}
	return nil
}

func eventLinkID(a *html.Node) string {
	u, err := url.Parse(GetAttr(a, "href"))
	if err != nil {
		return ""
	}
	q := u.Query()
	for _, key := range []string{"publishid", "veranstaltung.veranstid", "veranstid"} {
		if v := q.Get(key); v != "" {
			return v
		}
	}
	return ""
}

// EventListParser reads the entry of one event in the event search.
type EventListParser struct{}

// NewEventListParser creates an EventListParser.
func NewEventListParser() *EventListParser {
	return &EventListParser{}
}

var (
	reSemesterToken = regexp.MustCompile(`^(?:WS|SS|WiSe|SoSe)\s*\d{2,4}`)
	reEventNumber   = regexp.MustCompile(`^\S*\d\S*$`)
	reSWSToken      = regexp.MustCompile(`^(\d+(?:[.,]\d+)?)\s*SWS$`)
	reExpected      = regexp.MustCompile(`^(\d+)\s*erwartet$`)
	reMaximum       = regexp.MustCompile(`^(\d+)\s*maximal$`)
	// The basic data line separates its items by line breaks and runs of non-breaking
	// spaces; a single space belongs to an item („WS 2026/27", „2 SWS").
	reBasicSeparator = regexp.MustCompile(`[ \t\r\n\x{00a0}]*(?:\n|\x{00a0}{2,})[ \t\r\n\x{00a0}]*`)
	// The rhythm column of the list repeats the days: „A/B 07.10.2026 bis 27.01.2027",
	// „Einzel am 20.11.2026", „A von 05.10.2026". An event page states the rhythm alone.
	reRhythmDays  = regexp.MustCompile(`\s*(?:\b(?:am|von|ab|bis)\s+)?\d{1,2}\.\d{1,2}\.\d{4}.*$`)
	reRhythmTail  = regexp.MustCompile(`\s+(?:am|von|ab|bis)$`)
	reCancelEntry = regexp.MustCompile(`^(\d{1,2}\.\d{1,2}\.\d{4})\s+(.+)$`)
)

// Parse reads one archived entry (EventListEntry.HTML). fallbackID is the event the entry
// was archived for, pageURL the address of the event's own page, which is what an event
// names as its source.
func (p *EventListParser) Parse(r io.Reader, fallbackID, pageURL string) (*model.EventDetail, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse the event entry: %w", err)
	}
	title := eventLink(doc)
	if title == nil {
		return nil, fmt.Errorf("no event entry: the title link is missing")
	}
	detail := &model.EventDetail{
		ID:            fallbackID,
		Title:         CleanSingleLine(NodeText(title)),
		RawURL:        pageURL,
		LastScrapedAt: time.Now().UTC(),
	}
	if detail.ID == "" {
		detail.ID = eventLinkID(title)
	}

	titleBlock := title
	for titleBlock.Parent != nil && titleBlock.Parent.DataAtom != atom.Body {
		titleBlock = titleBlock.Parent
	}
	for n := titleBlock.NextSibling; n != nil; n = n.NextSibling {
		if n.Type == html.ElementNode && n.DataAtom == atom.Div {
			p.parseBasicData(n, detail)
			break
		}
	}

	group := UnnamedGroup
	var visit func(*html.Node)
	visit = func(n *html.Node) {
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			if c.Type != html.ElementNode {
				continue
			}
			switch {
			case c.DataAtom == atom.H3:
				switch heading := CleanSingleLine(NodeText(c)); {
				case heading == "Termin" || heading == "Termine":
					group = UnnamedGroup
				case strings.HasPrefix(heading, "Gruppe ") && eventLink(c) == nil:
					group = strings.TrimSpace(strings.TrimPrefix(strings.TrimPrefix(heading, "Gruppe "), ":"))
				}
			case c.DataAtom == atom.Table && strings.Contains(strings.ToLower(GetAttr(c, "summary")), "veranstaltungstermine"):
				p.parseDates(c, group, detail)
			default:
				visit(c)
			}
		}
	}
	visit(doc)
	return detail, nil
}

type basicToken struct {
	text string
	link *html.Node // nil for plain text
}

// parseBasicData reads the line under the title: semester, number and type of the event
// in this order, then its hours per week, its link, its institutions, the participants
// expected and allowed, and its persons.
func (p *EventListParser) parseBasicData(div *html.Node, d *model.EventDetail) {
	var tokens []basicToken
	var walk func(*html.Node)
	walk = func(n *html.Node) {
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			switch {
			case c.Type == html.TextNode:
				for _, part := range reBasicSeparator.Split(c.Data, -1) {
					if t := CleanSingleLine(part); t != "" {
						tokens = append(tokens, basicToken{text: t})
					}
				}
			case c.Type == html.ElementNode && c.DataAtom == atom.A:
				tokens = append(tokens, basicToken{text: CleanSingleLine(NodeText(c)), link: c})
			case c.Type == html.ElementNode:
				walk(c)
			}
		}
	}
	walk(div)

	// The label of an institution („Lehrstuhl", „Fakultät") is followed by its link; the
	// type of the event only ever by an empty one, which QIS prints for an event without
	// an institution.
	labelsInstitution := func(i int) bool {
		next := i + 1
		return next < len(tokens) && tokens[next].link != nil && tokens[next].text != "" &&
			strings.Contains(GetAttr(tokens[next].link, "href"), "publishSubDir=einrichtung")
	}
	rest := 0
	if rest < len(tokens) && tokens[rest].link == nil && reSemesterToken.MatchString(tokens[rest].text) {
		d.Semester = tokens[rest].text
		rest++
	}
	if rest < len(tokens) && tokens[rest].link == nil && reEventNumber.MatchString(tokens[rest].text) {
		d.EventNumber = tokens[rest].text
		rest++
	}
	if rest < len(tokens) && tokens[rest].link == nil && !reSWSToken.MatchString(tokens[rest].text) && !labelsInstitution(rest) {
		d.EventType = tokens[rest].text
		rest++
	}

	for _, t := range tokens[rest:] {
		if t.link == nil {
			if m := reSWSToken.FindStringSubmatch(t.text); m != nil {
				d.SWS = m[1]
			} else if m := reExpected.FindStringSubmatch(t.text); m != nil {
				d.ExpectedParticipants = m[1]
			} else if m := reMaximum.FindStringSubmatch(t.text); m != nil {
				d.MaxParticipants = m[1]
			}
			continue
		}
		href := GetAttr(t.link, "href")
		switch {
		case strings.Contains(href, "publishSubDir=einrichtung"):
			if t.text != "" {
				d.Institutions = append(d.Institutions, t.text)
			}
		case strings.Contains(href, "publishSubDir=personal"):
			if t.text != "" {
				d.ResponsiblePersons = append(d.ResponsiblePersons, model.EventResponsiblePerson{Name: t.text, URL: href})
			}
		case GetAttr(t.link, "target") == "_blank":
			d.Hyperlink = href
		}
	}
}

// parseDates reads the dates of one group. The columns are the ones of an event page but
// for the room plan, and the list names the room by its building and its number only.
func (p *EventListParser) parseDates(tbl *html.Node, group string, d *model.EventDetail) {
	rows := FindAllByTag(tbl, atom.Tr)
	if len(rows) < 2 {
		return
	}
	var headers []string
	for _, th := range FindAllByTag(rows[0], atom.Th) {
		headers = append(headers, strings.ToLower(CleanSingleLine(NodeText(th))))
	}

	for _, tr := range rows[1:] {
		tds := FindAllByTag(tr, atom.Td)
		if len(tds) == 0 {
			continue
		}
		s := model.EventSchedule{GroupName: group}
		for i, td := range tds {
			if i >= len(headers) {
				break
			}
			val := CleanSingleLine(NodeText(td))
			switch h := headers[i]; {
			case h == "tag":
				if val != "keine Angabe" {
					s.DayOfWeek = val
				}
			case h == "zeit":
				s.TimeSlot = val
				if parts := timeSplitRegex.Split(val, 2); len(parts) == 2 {
					s.StartTime, s.EndTime = strings.TrimSpace(parts[0]), strings.TrimSpace(parts[1])
				} else if val != "" {
					s.StartTime = val
				}
			case h == "rhythmus":
				s.Rhythm = reRhythmTail.ReplaceAllString(reRhythmDays.ReplaceAllString(val, ""), "")
			case h == "dauer":
				s.Duration = val
			case strings.HasPrefix(h, "fällt aus"):
				s.CancelledDates = cancelledDates(td)
			case h == "lehrperson":
				s.Instructor = val
				if a := FindFirstByTag(td, atom.A); a != nil {
					s.InstructorURL = GetAttr(a, "href")
					if text := CleanSingleLine(NodeText(a)); text != "" {
						s.Instructor = text
					}
				}
			case h == "raum":
				s.Room, s.RoomURL = listRoom(td)
			}
		}
		if s.DayOfWeek != "" || s.TimeSlot != "" || s.Room != "" {
			d.Schedules = append(d.Schedules, s)
		}
	}
}

// listRoom reads the room of a date: „Forschungszentrum 3H / 1.05" in the list, which an
// event page writes „Forschungszentrum 3H - 1.05 - Zentralcampus". The building and the
// room are joined the way the page joins them; the campus is not in the list.
func listRoom(td *html.Node) (room, roomURL string) {
	a := FindFirstByTag(td, atom.A)
	if a == nil {
		return CleanSingleLine(NodeText(td)), ""
	}
	u, err := url.Parse(GetAttr(a, "href"))
	if err != nil || u.Query().Get("raum.rgid") == "" {
		return "", "" // the list links every date to a room, an empty one for a date without
	}
	text := NodeText(a)
	if building, number, ok := strings.Cut(text, " /"); ok {
		building, number = CleanSingleLine(building), CleanSingleLine(number)
		switch {
		case building == "":
			return number, GetAttr(a, "href")
		case number == "":
			return building, GetAttr(a, "href")
		}
		return building + " - " + number, GetAttr(a, "href")
	}
	return CleanSingleLine(text), GetAttr(a, "href")
}

// cancelledDates reads the dates a slot is cancelled on, one per line, each with its
// note: „14.10.2026 findet ersatzweise im HS 11.301 statt." The page writes the same
// „14.10.2026: findet ersatzweise im HS 11.301 statt.", so the list is brought to that.
func cancelledDates(td *html.Node) string {
	var lines []string
	var line strings.Builder
	var walk func(*html.Node)
	walk = func(n *html.Node) {
		for c := n.FirstChild; c != nil; c = c.NextSibling {
			switch {
			case c.Type == html.TextNode:
				line.WriteString(c.Data)
				line.WriteString(" ")
			case c.Type == html.ElementNode && c.DataAtom == atom.Br:
				lines = append(lines, line.String())
				line.Reset()
			case c.Type == html.ElementNode:
				walk(c)
			}
		}
	}
	walk(td)
	lines = append(lines, line.String())

	var out []string
	for _, l := range lines {
		l = CleanSingleLine(l)
		if l == "" {
			continue
		}
		if m := reCancelEntry.FindStringSubmatch(l); m != nil {
			l = m[1] + ": " + m[2]
		}
		out = append(out, l)
	}
	return strings.Join(out, " ")
}
