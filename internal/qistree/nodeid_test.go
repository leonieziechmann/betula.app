package qistree

import "testing"

func TestParseNodeID(t *testing.T) {
	po := "https://www.b-tu.de/qisserver3/rds?state=modulBeschrGast&nodeID=auswahlBaum%7Cstudiengang%3Astg%3D079%7Cabschluss%3Aabschl%3D82%7CstgSpecials%3Avert%3D%2Cschwp%3D%2Ckzfa%3DH%2Cpversion%3D2008&expand=0#x"
	id := ParseNodeID(po)
	if id.Stg != "079" || id.Abschl != "82" || id.PVersion != "2008" || id.Kzfa != "H" || !id.IsPO() {
		t.Fatalf("PO node = %+v", id)
	}
	if got := id.ProgramID(); got != "079-82-2008" {
		t.Fatalf("ProgramID = %q", got)
	}

	area := ParseNodeID(nodeBase + "auswahlBaum|studiengang:stg=234|abschluss:abschl=A2|stgSpecials:vert=,schwp=,kzfa=H,pversion=2026|kontoOnTop:pordnr=9922|konto:pordnr=1")
	if area.IsPO() || area.Depth != 2 || area.ProgramID() != "234-A2-2026" {
		t.Fatalf("area node = %+v (%s)", area, area.ProgramID())
	}

	degree := ParseNodeID(nodeBase + "auswahlBaum|studiengang:stg=079|abschluss:abschl=82")
	if degree.IsPO() || degree.Depth != -1 || degree.Stg != "079" {
		t.Fatalf("degree node = %+v", degree)
	}

	special := NodeID{Stg: "079", Abschl: "82", PVersion: "2030", Schwp: "KI", Kzfa: "N"}
	if got := special.ProgramID(); got != "079-82-2030-sKI-kN" {
		t.Fatalf("ProgramID with specials = %q", got)
	}
}
