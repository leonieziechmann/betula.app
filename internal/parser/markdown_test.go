package parser

import (
	"strings"
	"testing"

	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// markdownOf reads a value cell the way applyRow gets it.
func markdownOf(t *testing.T, cell string) string {
	t.Helper()
	nodes, err := html.ParseFragment(strings.NewReader(cell), &html.Node{Type: html.ElementNode, Data: "td", DataAtom: atom.Td})
	if err != nil {
		t.Fatalf("ParseFragment: %v", err)
	}
	td := &html.Node{Type: html.ElementNode, Data: "td", DataAtom: atom.Td}
	for _, n := range nodes {
		td.AppendChild(n)
	}
	return Markdown(td)
}

// The cells below are taken from the live pages (QIS and b-tu.de/modul, 2026-10-01), shortened.
func TestMarkdown(t *testing.T) {
	for _, tc := range []struct{ name, cell, want string }{
		{
			"a list of the page after the line that leads to it (11101)",
			"Die Studierenden sollen<br />\n<ul>\n<li>sichere Kenntnisse über grundlegende Begriffe erwerben</li>\n" +
				"<li>lineare Gleichungssysteme lösen können</li>\n</ul>",
			"Die Studierenden sollen\n\n- sichere Kenntnisse über grundlegende Begriffe erwerben\n- lineare Gleichungssysteme lösen können",
		},
		{
			"strong headings, and the text after a list is no part of its last item (11101)",
			"<strong>Voraussetzung für die Modulabschlussprüfung:</strong><br />\n<ul>\n<li>erfolgreiche Bearbeitung von Hausaufgaben</li>\n</ul>\n" +
				"<strong>Modulabschlussprüfung:</strong><br />\n<ul>\n<li>Klausur, 90 min. <strong>ODER</strong> </li>\n<li>mündliche Prüfung, 30 min.</li>\n</ul>\n" +
				"In der ersten Lehrveranstaltung wird bekanntgegeben, ob die Prüfungsleistung in schriftlicher oder mündlicher Form zu erbringen ist.",
			"**Voraussetzung für die Modulabschlussprüfung:**\n\n- erfolgreiche Bearbeitung von Hausaufgaben\n\n**Modulabschlussprüfung:**\n\n" +
				"- Klausur, 90 min. **ODER**\n- mündliche Prüfung, 30 min.\n\n" +
				"In der ersten Lehrveranstaltung wird bekanntgegeben, ob die Prüfungsleistung in schriftlicher oder mündlicher Form zu erbringen ist.",
		},
		{
			"typed numbers with typed dashes under them, and the lines a PDF broke (12659)",
			"1. Entwicklung einer professionellen Identität und Haltung.<br /> 2. Kennenlernen:<br /> - der Geschichte der Sozialen Arbeit<br />" +
				" - grundlegender Fachtermini der Sozialarbeitswissenschaften<br /> 3. Auseinandersetzung mit:&nbsp;&nbsp;&nbsp; <br />" +
				" - ausgewählten Theorien und Handlungskonzepten Sozialer<br /> Arbeit, aus den entsprechenden Grundentwicklungsrichtungen und sie <br />" +
				" hinsichtlich ihres Anwendungskontextes kritisch, antidiskriminierend und menschenrechtsorientiert diskutieren<br /> können.&nbsp;&nbsp; <br />" +
				" -Techniken und Verfahren in der Sozialen Arbeit",
			"1. Entwicklung einer professionellen Identität und Haltung.\n2. Kennenlernen:\n   - der Geschichte der Sozialen Arbeit\n" +
				"   - grundlegender Fachtermini der Sozialarbeitswissenschaften\n3. Auseinandersetzung mit:\n" +
				"   - ausgewählten Theorien und Handlungskonzepten Sozialer\\\n     Arbeit, aus den entsprechenden Grundentwicklungsrichtungen und sie\n" +
				"     hinsichtlich ihres Anwendungskontextes kritisch, antidiskriminierend und menschenrechtsorientiert diskutieren\n     können.\n" +
				"   - Techniken und Verfahren in der Sozialen Arbeit",
		},
		{
			"numbers in brackets label a list, and the lists of the page under them belong to them (13915)",
			"Die Studierenden sind fähig:<br /><br /> (1)&nbsp; Wissen und Verstehen<br />\n<ul>\n<li>Bestimmungsgründe zu identifizieren,</li>\n</ul>\n" +
				"(2)&nbsp; Anwenden und Analysieren<br />\n<ul>\n<li>Argumente anzuwenden,</li>\n</ul>",
			"Die Studierenden sind fähig:\n\n- (1) Wissen und Verstehen\n  - Bestimmungsgründe zu identifizieren,\n" +
				"- (2) Anwenden und Analysieren\n  - Argumente anzuwenden,",
		},
		{
			"typed letters inside an item of an <ol> (11222)",
			"<ol>\n<li>Insolation</li>\n<li>Crash course in semiconductor physics:<br />  (a) Absorption     <br />  (b) Electrons and holes<br />  (c) p-n junction</li>\n" +
				"<li>Solar cell materials</li>\n</ol>",
			"1. Insolation\n2. Crash course in semiconductor physics:\n   - (a) Absorption\n   - (b) Electrons and holes\n   - (c) p-n junction\n3. Solar cell materials",
		},
		{
			"what stands between two typed numbers belongs to the first (11650)",
			"1. Drei Präsentationen (45%):<br /><ol>\n<li>Präsentation der Themeninhalte (33%), 15 min</li>\n<li>Abschlusspräsentation (34%), 20 min</li>\n" +
				"</ol>     (jeweils maximal 5 Punkte)<br /><br /><br />2. Abgabe einer Seminararbeit (55%), ca. 20-25 Seiten<br />    (80% inhaltliche Umsetzung)",
			"1. Drei Präsentationen (45%):\n   1. Präsentation der Themeninhalte (33%), 15 min\n   2. Abschlusspräsentation (34%), 20 min\n\n" +
				"   (jeweils maximal 5 Punkte)\n2. Abgabe einer Seminararbeit (55%), ca. 20-25 Seiten\\\n   (80% inhaltliche Umsetzung)",
		},
		{
			"blank lines and lines of &nbsp; end paragraphs; a heading keeps its text on the next line (12949)",
			"<strong>Rationale</strong><br /> Geoecology is an interdisciplinary science.<br /> <b>&nbsp;</b><br /> <b>Lecture:</b><br /> " +
				"The lecture of ecotoxicology deals with chemicals. Students will learn about:<br /> - aspects of geoecology<br /> -field sampling",
			"**Rationale**\\\nGeoecology is an interdisciplinary science.\n\n**Lecture:**\\\n" +
				"The lecture of ecotoxicology deals with chemicals. Students will learn about:\n\n- aspects of geoecology\n- field sampling",
		},
		{
			"a list for every item is one list, and an item that only holds a list nests under the one before (13143)",
			"<ul>\n<li>2 aufeinander aufbauende Lehrproben (je 20%)\n<ul>\n<li>in der Früherziehung</li>\n</ul>\n<strong>o d e r</strong><br />\n" +
				"<ul>\n<li>in der Grundausbildung</li>\n</ul>\n</li>\n</ul>\n<ul>\n<li>9 Hospitationen (20%)</li>\n</ul>\n<ul>\n<li>Kolloquium, 15 Min (10%)</li>\n</ul>\n" +
				"Die Prüfung kann in einer anderen Form erfolgen.",
			"- 2 aufeinander aufbauende Lehrproben (je 20%)\n  - in der Früherziehung\n\n  **o d e r**\n  - in der Grundausbildung\n" +
				"- 9 Hospitationen (20%)\n- Kolloquium, 15 Min (10%)\n\nDie Prüfung kann in einer anderen Form erfolgen.",
		},
		{
			"a dash after „und“ shares a word and begins no item; the line goes on (11277)",
			"häufig systemtheoretische und <br> –analytische  Betrachtungen.",
			"häufig systemtheoretische und\n–analytische Betrachtungen.",
		},
		{
			"a new sentence after a long line begins a paragraph, after a short one it stays a line",
			"Die Studierenden erwerben die Grundlagen des wissenschaftlichen Denkens und Arbeitens und erlernen Methoden des Arbeitens.<br />" +
				"Kompetenzen: Texte analysieren.<br />Die Dauer beträgt 6 Wochen.<br />Bitte melden Sie sich an.",
			"Die Studierenden erwerben die Grundlagen des wissenschaftlichen Denkens und Arbeitens und erlernen Methoden des Arbeitens.\n\n" +
				"Kompetenzen: Texte analysieren.\\\nDie Dauer beträgt 6 Wochen.\\\nBitte melden Sie sich an.",
		},
		{
			"a heading after the list of the last numbered item begins something new; section numbers are a list (12663)",
			"1. Gruppenarbeit:<br /> - Entstehungsgeschichte<br /> 2. Gemeinwesenarbeit:<br /> - Entstehung des Handelns<br /> Teil 2:<br />" +
				" 2.1. Konzeptentwicklung:<br /> - Elemente eines Konzepts<br /> 2.2. Rekonstruktive Zugänge<br /> - Forschungsethische Haltungen",
			"1. Gruppenarbeit:\n   - Entstehungsgeschichte\n2. Gemeinwesenarbeit:\n   - Entstehung des Handelns\n\nTeil 2:\n\n" +
				"- 2.1. Konzeptentwicklung:\n  - Elemente eines Konzepts\n- 2.2. Rekonstruktive Zugänge\n  - Forschungsethische Haltungen",
		},
		{
			"text after the last typed item follows the list where the items have no more than a line (12672)",
			"Es handelt sich um zwei Projektformen:<br />a) Handlungsbezogene Projekte und<br />b) Forschungsbezogene Projekte<br />" +
				"Die konkreten Projektthemen werden zu Beginn des Semesters ausgewiesen.",
			"Es handelt sich um zwei Projektformen:\n\n- a) Handlungsbezogene Projekte und\n- b) Forschungsbezogene Projekte\n\n" +
				"Die konkreten Projektthemen werden zu Beginn des Semesters ausgewiesen.",
		},
		{
			"numbers typed into every item of a list of the page number it (14888)",
			"<ul>\n<li>1) Continuous assessment (50 %)</li>\n<li>2) final written exam (50%), 80 minutes</li>\n</ul>",
			"1. Continuous assessment (50 %)\n2. final written exam (50%), 80 minutes",
		},
		{
			"a list of one item that only holds a list is that list (21103)",
			"<ul><li><ul><li>Grundlagen in Kunst</li><li>Umgang mit Computeranwendungen</li></ul></li></ul>",
			"- Grundlagen in Kunst\n- Umgang mit Computeranwendungen",
		},
		{
			"a number right before its word, and a sequence that numbers on (13329)",
			"1. Gegenstand<br />2. Struktur<br />3.Entwicklungspfade<br />4. Methodische Entwicklungen",
			"1. Gegenstand\n2. Struktur\n3. Entwicklungspfade\n4. Methodische Entwicklungen",
		},
		{
			"a number that begins no sequence stays text, escaped",
			"1. Semester: Grundlagen<br />Danach Vertiefung",
			"1\\. Semester: Grundlagen\\\nDanach Vertiefung",
		},
		{
			"a footnote is no list, and what CommonMark would read as markup is text (12241)",
			"Self-study Units (Learning Hub*):<br /><ul><li>Scientific Writing (1h 15m)</li></ul>* The Hub is an online platform.<br />" +
				"# 3 &lt;b&gt; [1] a_b _c_ `x` &amp;amp; \\ 5 &lt; 6",
			"Self-study Units (Learning Hub\\*):\n\n- Scientific Writing (1h 15m)\n\n\\* The Hub is an online platform.\\\n" +
				"\\# 3 \\<b> \\[1\\] a_b \\_c\\_ \\`x\\` \\&amp; \\\\ 5 < 6",
		},
		{
			"lines that would begin a block are escaped",
			"Teil A<br />+ 3<br />&gt; Zitat<br />----<br />2) Zwei<br />=",
			"Teil A\\\n\\+ 3\\\n\\> Zitat\\\n\\----\\\n2\\) Zwei\\\n\\=",
		},
		{
			"two of a rare bullet are a list, one is text",
			"o&nbsp;&nbsp; measurement<br /> o&nbsp;&nbsp; selected toxicity tests<br />o alone is a word here",
			"- measurement\n- selected toxicity tests\n- alone is a word here",
		},
		{
			"a bullet alone takes the text on the next line",
			"Inhalte:<br />•<br />Systemtheoretische Grundlagen<br />•<br />Systemische Grundhaltungen",
			"Inhalte:\n\n- Systemtheoretische Grundlagen\n- Systemische Grundhaltungen",
		},
		{
			"strong and emphasis hug their text, and never touch (11839, 11360)",
			"<li><em>11277 Logistikseminar</em><strong>AND</strong></li>" +
				"<b>Voraussetzung:</b>Text <b> Lecture:&nbsp; </b> und <em>11359: Analog IC Design<br></em>(siehe Bemerkungen) <u>unterstrichen</u>",
			"- 11277 Logistikseminar**AND**\n\n**Voraussetzung**:Text **Lecture:** und *11359: Analog IC Design*\\\n(siehe Bemerkungen) **unterstrichen**",
		},
		{
			"a page's paragraphs, and the empty ones it pads with",
			"<p>None</p>",
			"None",
		},
		{
			"an empty cell",
			" &nbsp; <br /> ",
			"",
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if got := markdownOf(t, tc.cell); got != tc.want {
				t.Errorf("Markdown:\n%s\n--- want:\n%s", got, tc.want)
			}
		})
	}
}

// No line of the Markdown begins with the „•" of the plain texts: validate holds the catalog to it
// (catalogdb, „module texts are Markdown"), and a bullet the page types becomes a list item.
func TestMarkdownLeavesNoBulletAtTheStartOfALine(t *testing.T) {
	for _, cell := range []string{
		"• Klausur<br />•Hausarbeit<br />•<br />• <br />•",
		"<ul><li>• Klausur</li><li>•</li></ul>",
		"und<br />•Klausur",
		"<li>Stray</li><li>items</li>",
	} {
		for _, line := range strings.Split(markdownOf(t, cell), "\n") {
			if strings.HasPrefix(strings.TrimLeft(line, " "), "•") {
				t.Errorf("%q: a line begins with •: %q", cell, line)
			}
		}
	}
}
