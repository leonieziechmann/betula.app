package normalize

import (
	"bufio"
	"os"
	"regexp"
	"strings"
	"testing"
)

func TestRoomShort(t *testing.T) {
	cases := []struct {
		room, short string
		known       bool
	}{
		// The owner's examples (2026-09-25), building and room parted by a slash („ZHG/HS.C“).
		{"Zentrales Hörsaalgebäude - Hörsaal A - Zentralcampus", "ZHG/HS.A", true},
		{"Zentrales Hörsaalgebäude - Hörsaal C - Zentralcampus", "ZHG/HS.C", true},
		{"Verfügungsgebäude 1C - 0.07 - Zentralcampus", "VG1C/0.07", true},
		{"Großer Hörsaal - Großer Hörsaal - Zentralcampus", "GHS", true},
		{"Mehrzweckgebäude - 130 - Zentralcampus", "MZG/130", true},
		{"Lehrgebäude 3A - 406 - Zentralcampus", "LG3A/406", true},
		// Kinds with their label.
		{"Zentrales Hörsaalgebäude - Seminarraum 4 - Zentralcampus", "ZHG/SEM.4", true},
		{"Zentrales Hörsaalgebäude - Audimax 1 - Zentralcampus", "ZHG/AM.1", true},
		{"Zentrales Hörsaalgebäude - Foyer Zentrales Hörsaalgebäude - Zentralcampus", "ZHG/Foyer", true},
		{"IBZ-Ludwig Leichhardt Haus - IBZ-Seminarraum", "IBZ/SEM", true},
		// The hall is the building.
		{"Hörsaal 3 - Hörsaal 3", "HS3", true},
		{"Laborhalle 3D - LH 3D", "LH3D", true},
		{"Laborhalle 3D - 06", "LH3D/06", true},
		{"Sporthalle 1 - Sporthalle 1 ZC", "SH1", true},
		{"Großer Hörsaal - Foyer Großer Hörsaal", "GHS/Foyer", true},
		// The building's own prefix in the room part.
		{"Hauptgebäude - HG 0.16 - Zentralcampus", "HG/0.16", true},
		{"Zwischenbau VI - ZB VI.01 - Zentralcampus", "ZB/VI.01", true},
		{"Zwischenbau VI - ZB VI.2 Weiterbildung", "ZB/VI.2", true},
		// Numbers, as printed, with what tells rooms apart.
		{"Lehrgebäude 10 - 211a/b - Zentralcampus", "LG10/211a+b", true},
		{"Lehrgebäude 2A - A0.01/02", "LG2A/A0.01+02", true},
		{"Lehrgebäude 2A - AU.14 (Techn.Labor (o.Absaug.))", "LG2A/AU.14", true},
		{"Mensa Zentralcampus - 0.33.1 - Meeting Room - Zentralcampus", "Mensa/0.33.1", true},
		{"Lehrgebäude 4/3 - 322 - Campus Nord", "LG4-3/322", true},
		{"Panta Rhei Halle - B200 - Zentralcampus", "PRH/B200", true},
		{"Zw. Bau LG 2C/2D - 229/230 - Zentralcampus", "ZB2CD/229+230", true},
		{"Lehrgebäude Bad-Saarow - K02", "Bad-Saarow/K02", true},
		// One scheme, one spelling; the slash is the only one: two rooms are joined by +.
		{"Laborgebäude 4B - B.3.16 - Zentralcampus", "LB4B/B3.16", true},
		{"Forschungszentrum 3E - 2.26/2.27 - Zentralcampus", "FZ3E/2.26+27", true},
		// Ateliers and named rooms.
		{"Zw. Bau LG 2C/2D - AT Oestreich mitte - Zentralcampus", "ZB2CD/AT Oestreich M", true},
		{"Anbau LG 2D - AT Baller (OG1) - Zentralcampus", "LG2D/AT Baller", true},
		{"Lehrgebäude 2A - 1.OG AT Stadtplaner - Zentralcampus", "LG2A/AT 1.OG", true},
		{"Anbau LG 2C - EG Zeichensaal - Zentralcampus", "LG2C/Zeichensaal", true},
		// Overrides.
		{"Anbau LG 2D - EG Bildhauerwerkstatt - Zentralcampus", "LG2D/Werkstatt", true},
		{"Outdoor-Veranstaltungen ZC - OR_2.1_Fakultätsgarten - Zentralcampus", "Fakultätsgarten", true},
		{"Outdoor-Veranstaltungen SFB - OD_Sfb_Gb9_Sportplatz", "SFB/Sportplatz", true},
		// Senftenberg and Sachsendorf: campus and number; the description goes.
		{"Gebäude 14.C - SFB - 14C.103 Hörsaal - Campus Senftenberg", "SFB/14C.103", true},
		{"Gebäude 11 - Hörsaal SD - 11.301 Hörsaal C - Campus Sachsendorf", "SD/11.301", true},
		{"Gebäude 1 - SFB - 1.208 Skills Lab - Neurologie - Campus Senftenberg", "SFB/1.208", true},
		{"Gebäude 9 - SportH SFB - 9.151 Sporthalle, Feld 2 (Volleyball) - Campus Senftenberg", "SFB/9.151 F2", true},
		{"Gebäude 9 - SD - 9.102.1", "SD/9.102.1", true},
		{"Gebäude 16 - SFB - 16.119/120", "SFB/16.119+120", true},
		// Shapes no room has today.
		{"Gebäude 20 - Mensa SFB - 20.010 Speisesaal - Campus Senftenberg", "SFB/20.010", true},
		{"Lehrgebäude 3A - Hörsaal 4 - Zentralcampus", "LG3A/HS.4", true},
		{"Zentrales Hörsaalgebäude - Seminarraum 5 - Zentralcampus", "ZHG/SEM.5", true},
		{"Lehrgebäude 2A - A1.30C PC-Pool - hinten - Zentralcampus", "LG2A/A1.30C", true},
		{"Hauptgebäude - HG 1.07/1.08 - Zentralcampus", "HG/1.07+08", true},
		{"Lehrgebäude 2C - Dachatelier - Zentralcampus", "LG2C/Dachatelier", true},
		{"Neues Laborgebäude 5X - 1.23 Labor Robotik - Zentralcampus", "Neues Laborgebäude 5X/1.23", false},
		{"Outdoor-Veranstaltungen ZC - OR_7.1_Neuer Garten", "OR_7.1_Neuer Garten", false},
		// The strings Radix's own tests use.
		{"Lehrgebäude 1A - 0.22 - Zentralcampus", "LG1A/0.22", true},
		{"Hörsaal 3 - Campus Sachsendorf", "SD/Hörsaal 3", true},
		{"Allgemeine Elektrotechnik Labor - 14.117 - Campus Senftenberg", "SFB/14.117", true},
		{"Gebäude 7 - 7.135 - Campus Sachsendorf", "SD/7.135", true},
		// Whitespace as a page may bring it; no building part at all.
		{"  Zentrales   Hörsaalgebäude -  Hörsaal B  - Zentralcampus ", "ZHG/HS.B", true},
		{"Online", "Online", true},
	}
	for _, c := range cases {
		short, known := RoomShort(c.room)
		if short != c.short || known != c.known {
			t.Errorf("RoomShort(%q) = %q, %v; want %q, %v", c.room, short, known, c.short, c.known)
		}
	}
}

var roomSFBSDBuilding = regexp.MustCompile(`^Gebäude (\d+)(?:\.([A-Z]))? - `)

// testdata/rooms.tsv holds every room of QIS's room list of 2026-09-25 (505) and the three
// rooms events name that the list lacks, with the short forms the design's reference
// implementation gives them.
func TestRoomShortQISList(t *testing.T) {
	f, err := os.Open("testdata/rooms.tsv")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	byShort := map[string]string{}
	n := 0
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimRight(scanner.Text(), "\r")
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		name, want, ok := strings.Cut(line, "\t")
		if !ok {
			t.Fatalf("bad line %q", line)
		}
		n++
		got, known := RoomShort(name)
		if got != want {
			t.Errorf("RoomShort(%q) = %q, want %q", name, got, want)
		}
		if !known {
			t.Errorf("RoomShort(%q): building not in the table", name)
		}
		if other, dup := byShort[got]; dup {
			t.Errorf("%q and %q both give %q", other, name, got)
		}
		byShort[got] = name
		// One slash, between building and room: ZHG/HS.A, LG10/211a+b, LG4-3/322.
		if n := strings.Count(got, "/"); n > 1 || n == 0 && strings.Contains(got, " ") && !strings.HasPrefix(name, "Outdoor-") {
			t.Errorf("RoomShort(%q) = %q: not one slash between building and room", name, got)
		}
		// On SFB and SD the number starts with the building: „Gebäude 14.C“ → 14C.xxx.
		if m := roomSFBSDBuilding.FindStringSubmatch(name); m != nil && (strings.HasPrefix(got, "SFB/") || strings.HasPrefix(got, "SD/")) {
			_, number, _ := strings.Cut(got, "/")
			if prefix, _, _ := strings.Cut(number, "."); prefix != m[1]+m[2] {
				t.Errorf("RoomShort(%q) = %q: the number does not start with building %s%s", name, got, m[1], m[2])
			}
		}
	}
	if err := scanner.Err(); err != nil {
		t.Fatal(err)
	}
	if n != 508 {
		t.Errorf("%d rooms in testdata/rooms.tsv, want 508", n)
	}
}

func FuzzRoomShort(f *testing.F) {
	for _, s := range []string{
		"Zentrales Hörsaalgebäude - Hörsaal A - Zentralcampus",
		"Gebäude 14.C - SFB - 14C.103 Hörsaal - Campus Senftenberg",
		"Hörsaal 3 - Campus Sachsendorf",
		"Zw. Bau LG 2C/2D - AT Oestreich mitte - Zentralcampus",
		"Lehrgebäude 4/3 - 229/230/231 - Campus Nord",
		"Outdoor-Veranstaltungen ZC - OR_x",
		" - Zentralcampus",
		"a - b - c - d",
	} {
		f.Add(s)
	}
	f.Fuzz(func(t *testing.T, room string) {
		short, _ := RoomShort(room)
		if strings.TrimSpace(room) != "" && short == "" {
			t.Fatalf("RoomShort(%q) is empty", room)
		}
		// A name with a building part gives at most one slash, the one before the room.
		if name := roomCampusSuffix.ReplaceAllString(strings.Join(strings.Fields(room), " "), ""); strings.Contains(name, " - ") && strings.Count(short, "/") > 1 {
			t.Fatalf("RoomShort(%q) = %q: more than one slash", room, short)
		}
		// A short form without " - " is its own short form: nothing is shortened twice.
		if !strings.Contains(short, " - ") {
			if again, _ := RoomShort(short); again != short {
				t.Fatalf("RoomShort(%q) = %q, but RoomShort(%q) = %q", room, short, short, again)
			}
		}
	})
}

// BuildingTokens are what a short room name opens with, in capitals; the room kinds that
// follow a building (SEM.4, AM.1, AT Name) are not among them.
func TestBuildingTokens(t *testing.T) {
	got := map[string]bool{}
	for _, tok := range BuildingTokens() {
		got[tok] = true
	}
	for _, want := range []string{"ZHG", "HG", "GHS", "MZG", "VG", "LG", "ZB", "IKMZ", "SFB", "SD"} {
		if !got[want] {
			t.Errorf("BuildingTokens lacks %s: %v", want, BuildingTokens())
		}
	}
	for _, not := range []string{"SEM", "AM", "AT", "Mensa"} { // HS is there: the building HS3
		if got[not] {
			t.Errorf("BuildingTokens has %s", not)
		}
	}
	// every token a building gives is covered: its capitals are in the list
	for building, token := range roomBuildings {
		if m := roomTokenLetters.FindString(token); token != "" && m != "" && !got[m] {
			t.Errorf("%s (%s) is not covered", building, token)
		}
	}
}
