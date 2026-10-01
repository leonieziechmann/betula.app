package semantic

import "testing"

func TestPassage(t *testing.T) {
	text := Text{TitleDE: " Werkstofftechnik ", TitleEN: "Materials\nEngineering", Contents: "Metalle,  Keramik.", Outcomes: "Werkstoffe wählen."}
	if got, want := Passage(text, nil), "Werkstofftechnik / Materials Engineering. Metalle, Keramik. Werkstoffe wählen."; got != want {
		t.Errorf("without a summary:\n got %q\nwant %q", got, want)
	}
	summary := &Summary{DE: "Eigenschaften von Werkstoffen.", EN: "Properties of materials.", Keywords: []string{"werkstoffe", " ", "materials science"}}
	if got, want := Passage(text, summary), "Werkstofftechnik / Materials Engineering. Eigenschaften von Werkstoffen. Properties of materials. werkstoffe, materials science. Metalle, Keramik. Werkstoffe wählen."; got != want {
		t.Errorf("with a summary:\n got %q\nwant %q", got, want)
	}
	// The same title twice, one title only, no description: what semantic::module_text makes.
	for _, c := range []struct {
		text Text
		want string
	}{
		{Text{TitleDE: "Analysis III", TitleEN: "Analysis III", Outcomes: "Beweise."}, "Analysis III. Beweise."},
		{Text{TitleEN: "Robotics"}, "Robotics. "},
	} {
		if got := Passage(c.text, nil); got != c.want {
			t.Errorf("Passage(%+v) = %q, want %q", c.text, got, c.want)
		}
	}
}

func TestHashes(t *testing.T) {
	a := Text{TitleDE: "Statik", Contents: "Kräfte  und Momente"}
	b := Text{TitleDE: "Statik ", Contents: "Kräfte und\tMomente"}
	if a.Hash() != b.Hash() {
		t.Error("white space changes the text's hash")
	}
	if a.Hash() == (Text{TitleDE: "Statik", Outcomes: "Kräfte und Momente"}).Hash() {
		t.Error("the same words in another field have the same hash")
	}
	if PassageHash(Passage(a, nil)) == PassageHash(Passage(a, &Summary{DE: "x", EN: "y"})) {
		t.Error("a summary does not change the passage's hash")
	}
	if len(a.Hash()) != 64 || a.Hash() == PassageHash(Passage(a, nil)) {
		t.Error("hashes are not sha256 of their own domain")
	}
}

// The Markdown Radix writes of a module text (internal/parser/markdown_test.go) as plain words.
func TestPlain(t *testing.T) {
	for _, c := range []struct{ markdown, want string }{
		{"Die Studierenden sollen\n\n- sichere Kenntnisse erwerben\n- Gleichungssysteme lösen können",
			"Die Studierenden sollen sichere Kenntnisse erwerben Gleichungssysteme lösen können"},
		{"**Modulabschlussprüfung:**\n\n- Klausur, 90 min. **ODER**\n- mündliche Prüfung, 30 min.",
			"Modulabschlussprüfung: Klausur, 90 min. ODER mündliche Prüfung, 30 min."},
		{"1. Drei Präsentationen (45%):\n   1. Präsentation (33%), 15 min\n2. Seminararbeit\\\n   (80% Umsetzung)",
			"Drei Präsentationen (45%): Präsentation (33%), 15 min Seminararbeit (80% Umsetzung)"},
		{"- (1) Wissen und Verstehen\n  - Bestimmungsgründe zu *identifizieren*,", "(1) Wissen und Verstehen Bestimmungsgründe zu identifizieren,"},
		{`Ein \*Stern\*, ein \# und C\+\+.`, "Ein *Stern*, ein # und C++."},
		{"", ""},
	} {
		if got := Plain(c.markdown); got != c.want {
			t.Errorf("Plain(%q)\n got %q\nwant %q", c.markdown, got, c.want)
		}
	}
}
