package normalize

import (
	"regexp"
	"sort"
	"strings"
)

// Short room names. QIS names a room „Zentrales Hörsaalgebäude - Hörsaal A - Zentralcampus“
// or „Gebäude 14.C - SFB - 14C.103 Hörsaal - Campus Senftenberg“: 50 characters where a
// week grid has room for ten. The short form is „<building>/<room>[<attachment>]“ (owner,
// 2026-09-25: „ZHG/HS.C“, „weil sich das viel besser liest“):
//
//	ZHG/HS.A     VG1C/0.07     LG3A/324     LG10/211a+b     SFB/14C.103     SD/7.116     GHS
//
// BUILDING is a token without spaces from the legend of BTU's campus plan (Oct 2021) and the
// owner's decisions of 2026-09-25 (ZHG, HS., GHS „damit es nicht mit HG verwechselt wird“,
// VG1C/0.07), or the campus code SFB / SD, whose room numbers already start with their
// building. ROOM is the number as QIS prints it, or a kind abbreviation with its label
// (HS.A, SEM.4, AM.1). ATTACH tells rooms with one number apart (a+b, .1, +27, „ F1“).
// The slash between them is the only one (joinRoom): two rooms QIS writes as a pair or a
// range (2.26/2.27, 229/230, 211a/b) are joined by „+“, and the Lehrgebäude of Campus Nord
// are LG4-1, LG4-3 and LG4-4. A room with words keeps its spaces (ZB2CD/AT Oestreich M).
// docs/radix/schema-v2.md, „Short names“.

// roomBuildings maps QIS's building name, the part of a room name before the first " - ",
// to its token. An empty token stands for a building that is no building (the outdoor
// places): their rooms are all in roomOverrides.
var roomBuildings = map[string]string{
	"Zentrales Hörsaalgebäude":  "ZHG",
	"Hauptgebäude":              "HG",
	"Großer Hörsaal":            "GHS",
	"Hörsaal 3":                 "HS3",
	"Mehrzweckgebäude":          "MZG",
	"IKMZ":                      "IKMZ",
	"FMPA":                      "FMPA",
	"Verfügungsgebäude 1C":      "VG1C",
	"Lehrgebäude 1A":            "LG1A",
	"Lehrgebäude 2A":            "LG2A",
	"Lehrgebäude 2B":            "LG2B",
	"Lehrgebäude 2C":            "LG2C",
	"Lehrgebäude 2D":            "LG2D",
	"Lehrgebäude 3A":            "LG3A",
	"Lehrgebäude 3B":            "LG3B",
	"Lehrgebäude 10":            "LG10",
	"Lehrgebäude 4/1":           "LG4-1", // not LG4/1: the slash parts building and room
	"Lehrgebäude 4/3":           "LG4-3",
	"Lehrgebäude 4/4":           "LG4-4",
	"Laborgebäude 1B":           "LB1B",
	"Laborgebäude 4B":           "LB4B",
	"Laborhalle 3D":             "LH3D",
	"Forschungszentrum 3E":      "FZ3E",
	"Forschungszentrum 3H":      "FZ3H",
	"Anwendungsz. Fluiddynamik": "AZFD",
	"Besucherzentrum Intelligente Energie Netze": "BIENe",
	"Gründungszentrum Cottbus":                   "RCGC",
	"IBZ-Ludwig Leichhardt Haus":                 "IBZ",
	"Mensa Zentralcampus":                        "Mensa",
	// The annexes are reached through their Lehrgebäude; the campus plan has no token for them.
	"Anbau LG 2C":      "LG2C",
	"Anbau LG 2D":      "LG2D",
	"Zw. Bau LG 2C/2D": "ZB2CD", // QIS: „Zwischenbau 2CD“
	"Zwischenbau VI":   "ZB",    // QIS: „ZB VI.01“; the numeral stays with the room
	// Not on the campus plan's legend: made up here.
	"Panta Rhei Halle":            "PRH",
	"Sporthalle 1":                "SH1",
	"Lehrgebäude Bad-Saarow":      "Bad-Saarow",
	"Werner-von-Siemens-St. 7":    "WvS7",
	"An der Pastoa 13, Cottbus":   "Pastoa13",
	"Outdoor-Veranstaltungen ZC":  "",
	"Outdoor-Veranstaltungen SFB": "",
}

// roomOverrides are places that carry no number the rules could keep. Exact names,
// without the campus suffix of an event's room. A place of no building is its name alone.
var roomOverrides = map[string]string{
	"Outdoor-Veranstaltungen SFB - OD_Sfb_Gb9_Sportplatz":          "SFB/Sportplatz",
	"Outdoor-Veranstaltungen ZC - OR_1.1_Innenhof LG 1 A":          "LG1A/Innenhof",
	"Outdoor-Veranstaltungen ZC - OR_1.2_Innenhof VG 1C":           "VG1C/Innenhof",
	"Outdoor-Veranstaltungen ZC - OR_2.1_Fakultätsgarten":          "Fakultätsgarten",
	"Outdoor-Veranstaltungen ZC - OR_6.1_Zwischenbau 2CD Aula":     "ZB2CD/Aula",
	"Outdoor-Veranstaltungen ZC - OR_6.2_Innenhof K.-W.-Allee 2CD": "Innenhof 2CD",
	"Outdoor-Veranstaltungen ZC - OR_6.3_Baumgruppe FMPA":          "FMPA/Baumgruppe",
	"Outdoor-Veranstaltungen ZC - OR_6.4_Zwischenbau 2AB Aula":     "ZB2AB/Aula",
	"Outdoor-Veranstaltungen ZC - OR_6.5_Innenhof K.-W.-Allee 2AB": "Innenhof 2AB",
	"Outdoor-Veranstaltungen ZC - OR_6.6_Lehmbau":                  "Lehmbau",
	"Outdoor-Veranstaltungen ZC - OR_Forum_Kirschhain":             "Forum Kirschhain",
	"IKMZ - eAssessment Center IKMZ 1.UG":                          "IKMZ/eAssessment",
	"Anbau LG 2D - EG Bildhauerwerkstatt":                          "LG2D/Werkstatt",
}

// roomIsBuilding: the hall is a building of its own, and its short form is the token alone.
var roomIsBuilding = map[[2]string]bool{
	{"Großer Hörsaal", "Großer Hörsaal"}: true,
	{"Hörsaal 3", "Hörsaal 3"}:           true,
	{"Laborhalle 3D", "LH 3D"}:           true,
	{"Sporthalle 1", "Sporthalle 1 ZC"}:  true,
}

var (
	roomCampusSuffix = regexp.MustCompile(` - (Zentralcampus|Campus Senftenberg|Campus Sachsendorf|Campus Nord)$`)
	// Senftenberg and Sachsendorf: „Gebäude 14.C - SFB“, „Gebäude 11 - Hörsaal SFB“, „Gebäude 9 - SportH SFB“.
	roomCampusBuilding = regexp.MustCompile(`^Gebäude (\d+)(?:\.([A-Z]))? - (?:([^-]+?) )?(SFB|SD)$`)
	// Their room numbers: 6.210  14C.103  1.111.1  16.119/120  9.102.1
	roomCampusNumber = regexp.MustCompile(`^(\d+[A-Z]?\.\d+(?:\.\d+)?(?:/\d+)?)(.*)$`)
	// The same number anywhere in a name of another spelling („… Labor - 14.117 - Campus Senftenberg“).
	// RE2 has no lookaround, so the boundaries are matched and the number is the group.
	roomCampusNumberAnywhere = regexp.MustCompile(`(?:^|[^\d.])(\d+[A-Z]?\.\d{3}(?:\.\d+)?(?:/\d+)?)(?:$|[^\d.])`)
	roomField                = regexp.MustCompile(`\bFeld (\d+)`)
	// A printed room number and what follows it:
	// 0.16  324  185d  211a/b  A0.01/02  A0.25.1  A1.30B  AU.12  B0.27  B.3.16  2.26/2.27  K02  VI.01
	roomNumber        = regexp.MustCompile(`^((?:[A-Z]{1,2}\.?)?\d+(?:\.\d+)*[A-Za-z]?(?:/(?:[A-Z]?\d+(?:\.\d+)*[a-z]?|[a-z]))?)(.*)$`)
	roomLetterDot     = regexp.MustCompile(`^([A-Z])\.(\d\.\d+.*)$`)
	roomRepeatedRange = regexp.MustCompile(`^(.*\.)(\d+)/(.*\.)(\d+)$`)
)

// roomKinds stand where a number would: „Hörsaal A“ becomes „HS.A“ (the owner first wrote „ZHG HS.3“;
// since the slash, 2026-09-25, it is ZHG/HS.3).
var roomKinds = []struct {
	rx   *regexp.Regexp
	repl string // "" = the atelier rule with its side (links/mitte/rechts)
}{
	{regexp.MustCompile(`^Hörsaal ([A-Z0-9]{1,2})$`), "HS.$1"},
	{regexp.MustCompile(`^Seminarraum (\d+)$`), "SEM.$1"},
	{regexp.MustCompile(`^Audimax (\d+)$`), "AM.$1"},
	{regexp.MustCompile(`^IBZ-Seminarraum$`), "SEM"},
	{regexp.MustCompile(`^Foyer\b.*$`), "Foyer"},
	{regexp.MustCompile(`^(\d)\.OG AT\b.*$`), "AT $1.OG"},                      // „1.OG AT Stadtplaner“
	{regexp.MustCompile(`^AT (\S+)(?: \([^)]*\))? (links|mitte|rechts)$`), ""}, // „AT Oestreich mitte“ → AT Oestreich M
	{regexp.MustCompile(`^AT (\S+)(?: \([^)]*\))?$`), "AT $1"},                 // „AT Baller (OG1)“ → AT Baller
	{regexp.MustCompile(`^EG (\S+)$`), "$1"},                                   // EG = ground floor
}

// RoomShort returns the short form of a room name as QIS prints it, with or without the
// " - <campus>" suffix of an event's room. It is pure and total: a non-blank name never
// gives "". known is false when the building is not in the table: the short form then
// keeps the building's name as QIS spells it (no acronym is invented), and the build
// warns so that the table gets a line.
func RoomShort(room string) (short string, known bool) {
	name := strings.Join(strings.Fields(room), " ")
	campus := ""
	if m := roomCampusSuffix.FindStringSubmatchIndex(name); m != nil {
		campus = name[m[2]:m[3]]
		name = name[:m[0]]
	}
	if s, ok := roomOverrides[name]; ok {
		return s, true
	}
	code := ""
	switch campus {
	case "Campus Senftenberg":
		code = "SFB"
	case "Campus Sachsendorf":
		code = "SD"
	}

	building, rest, found := strings.Cut(name, " - ")
	if !found {
		// No building part: a label that still says where it is on SFB/SD.
		if code != "" {
			return joinRoom(code, name), true
		}
		return name, true
	}

	// Senftenberg and Sachsendorf: „Gebäude N[.X] - [<use> ]SFB|SD - <number> <description>“.
	if head, tail, ok := strings.Cut(rest, " - "); ok {
		if mb := roomCampusBuilding.FindStringSubmatch(building + " - " + head); mb != nil {
			code := mb[4]
			mn := roomCampusNumber.FindStringSubmatch(tail)
			if mn == nil {
				return joinRoom(code, tail), true
			}
			attach := ""
			if mf := roomField.FindStringSubmatch(mn[2]); mf != nil {
				attach = " F" + mf[1] // „9.151 Sporthalle, Feld 1 (Leerfeld)“
			}
			return joinRoom(code, mn[1]+attach), true
		}
	}

	// Senftenberg and Sachsendorf in another spelling: the number decides.
	if code != "" {
		if mn := roomCampusNumberAnywhere.FindStringSubmatch(rest); mn != nil {
			return joinRoom(code, mn[1]), true
		}
	}

	// Cottbus (Zentralcampus, Campus Nord) and elsewhere.
	token, known := roomBuildings[building]
	if !known {
		token = building
	}
	if token == "" {
		// An outdoor place no override names: the place alone, and a warning.
		return strings.ReplaceAll(rest, "/", "+"), false
	}
	if roomIsBuilding[[2]string{building, rest}] {
		return token, known
	}

	part := rest
	// The building's own printed prefix inside the room part: „HG 0.16“, „ZB VI.01“ (HG/0.16, ZB/VI.01).
	switch token {
	case "HG":
		part = strings.TrimPrefix(part, "HG ")
	case "ZB":
		part = strings.TrimPrefix(part, "ZB ")
	}

	for _, k := range roomKinds {
		m := k.rx.FindStringSubmatchIndex(part)
		if m == nil {
			continue
		}
		var r string
		if k.repl == "" {
			r = "AT " + part[m[2]:m[3]] + " " + strings.ToUpper(part[m[4]:m[4]+1])
		} else {
			r = string(k.rx.ExpandString(nil, k.repl, part, m))
		}
		return joinRoom(token, r), known
	}

	if mn := roomNumber.FindStringSubmatch(part); mn != nil {
		return joinRoom(token, roomNumberSpelling(mn[1])), known
	}
	return joinRoom(token, part), known
}

// joinRoom writes a short form: the building, a slash, the room. The slash is the only one in
// it: one inside the room part joins two rooms („229/230“ → 229+230, „211a/b“ → 211a+b), one
// in a building name the table lacks becomes a hyphen, as in LG4-3.
func joinRoom(building, room string) string {
	return strings.ReplaceAll(building, "/", "-") + "/" + strings.ReplaceAll(room, "/", "+")
}

// roomNumberSpelling writes one scheme one way: „B.3.16“ as „B3.16“, because LB 4B spells its
// other rooms „B3.18“, and „2.26/2.27“ as „2.26/27“, the way QIS itself writes „A0.27/28“
// (joinRoom then makes both 2.26+27 and A0.27+28).
func roomNumberSpelling(num string) string {
	if m := roomLetterDot.FindStringSubmatch(num); m != nil {
		num = m[1] + m[2]
	}
	if m := roomRepeatedRange.FindStringSubmatch(num); m != nil && m[1] == m[3] {
		num = m[1] + m[2] + "/" + m[4]
	}
	return num
}

// roomTokenLetters is the letter part of a building token: LG for LG3A, ZB for ZB2CD.
var roomTokenLetters = regexp.MustCompile(`^[A-ZÄÖÜ]{2,}`)

// BuildingTokens are the capitals a short room name opens with, which a reader takes for a
// place: those of every building token (ZHG, HG, LG, VG, ZB …) and the campus codes SFB and
// SD. A module abbreviation must not look like one (package abbrev): a week grid shows a
// module and its room side by side, and „HG“ next to „HG/0.16“ reads as the building. The
// room kinds (SEM.4, AM.1, AT Name) always follow a building token and are not listed; HS is,
// for the Hörsaal 3 that is a building of its own (HS3). Sorted, without duplicates.
func BuildingTokens() []string {
	seen := map[string]bool{"SFB": true, "SD": true}
	add := func(token string) {
		if m := roomTokenLetters.FindString(token); m != "" {
			seen[m] = true
		}
	}
	for _, token := range roomBuildings {
		add(token)
	}
	for _, short := range roomOverrides {
		building, _, _ := strings.Cut(short, "/")
		add(strings.Fields(building)[0])
	}
	out := make([]string, 0, len(seen))
	for t := range seen {
		out = append(out, t)
	}
	sort.Strings(out)
	return out
}
