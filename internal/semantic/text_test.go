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
