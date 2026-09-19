package parser

import (
	"strings"
	"testing"
)

func TestParseRootPrograms(t *testing.T) {
	sampleHTML := `
	<ul class="treelist">
		<li class="treelist">
			<a class="regular" href="/qisserver3/rds?state=modulBeschrGast&amp;nodeID=auswahlBaum%7Cstudiengang%3Astg%3D749">Studiengang: Angewandte Mathematik</a>
		</li>
		<li class="treelist">
			<a class="regular" href="/qisserver3/rds?state=modulBeschrGast&amp;nodeID=auswahlBaum%7Cstudiengang%3Astg%3D049">Studiengang: Angewandte Naturwissenschaften</a>
		</li>
	</ul>
	`
	parser := NewProgramTreeParser()
	programs, err := parser.ParseRootPrograms(strings.NewReader(sampleHTML), "https://www.b-tu.de")
	if err != nil {
		t.Fatalf("ParseRootPrograms failed: %v", err)
	}

	if len(programs) != 2 {
		t.Fatalf("expected 2 programs, got %d", len(programs))
	}
	if programs[0].Name != "Angewandte Mathematik" || programs[0].Code != "749" {
		t.Errorf("unexpected program 0: %+v", programs[0])
	}
	if programs[1].Name != "Angewandte Naturwissenschaften" || programs[1].Code != "049" {
		t.Errorf("unexpected program 1: %+v", programs[1])
	}
}

func TestParseDegrees(t *testing.T) {
	sampleHTML := `
	<ul class="treelist">
		<li class="treelist">
			<a class="regular" href="/qisserver3/rds?nodeID=auswahlBaum%7Cstudiengang%3Astg%3D749%7Cabschluss%3Aabschl%3D88">Module für Abschluss: Master (universitär)</a>
		</li>
	</ul>
	`
	parser := NewProgramTreeParser()
	degrees, err := parser.ParseDegrees(strings.NewReader(sampleHTML), "https://www.b-tu.de")
	if err != nil {
		t.Fatalf("ParseDegrees failed: %v", err)
	}

	if len(degrees) != 1 {
		t.Fatalf("expected 1 degree, got %d", len(degrees))
	}
	if degrees[0].Degree != "Master (universitär)" || degrees[0].Code != "88" {
		t.Errorf("unexpected degree: %+v", degrees[0])
	}
}

func TestParsePOVersionsAndDocuments(t *testing.T) {
	sampleHTML := `
	<div class="Kruemelpfad">
		<a class="regular" href="/qisserver3/rds?nodeID=auswahlBaum">Oberste Ebene</a>
		<a class="regular" href="/qisserver3/rds?nodeID=auswahlBaum|studiengang:stg=749">Studiengang: Angewandte Mathematik</a>
	</div>
	<span class="Tree"> PO-Version: 2019 - 1. SÄ 2021</span>
	<a class="regular" href="https://opus4.kobv.de/opus4-btu/files/5583/AMbl-13_2021_AeSen_Ma_WiMa_AngMa.pdf" target="_blank" title="Satzungsänderung ABl. 13/2021 (1. SÄ)"><img src="/QIS/images//satzungsaenderung1.gif" alt="Satzungsänderung ABl. 13/2021 (1. SÄ)" border="0"></a>
	<a class="regular" href="https://opus4.kobv.de/opus4-btu/files/5006/AMbl-25_2019_AnMa_M.Sc.pdf" target="_blank" title="Prüfungsordnung ABl. 25/2019"><img src="/QIS/images//pruefungsordnung.gif" alt="Prüfungsordnung ABl. 25/2019" border="0"></a>
	<ul class="treelist">
		<li class="treelist">
			<a class="regular" href="/qisserver3/rds?nodeID=auswahlBaum%7Cstudiengang%3Astg%3D749%7Cabschluss%3Aabschl%3D88%7CstgSpecials%3Avert%3D%2Cschwp%3D%2Ckzfa%3DH%2Cpversion%3D2019">PO-Version: 2019 - 1. SÄ 2021</a>
		</li>
	</ul>
	`
	parser := NewProgramTreeParser()
	pos, err := parser.ParsePOVersions(strings.NewReader(sampleHTML), "https://www.b-tu.de")
	if err != nil {
		t.Fatalf("ParsePOVersions failed: %v", err)
	}

	if len(pos) != 1 {
		t.Fatalf("expected 1 PO version, got %d", len(pos))
	}
	if pos[0].POVersion != "2019 - 1. SÄ 2021" {
		t.Errorf("unexpected PO version: %s", pos[0].POVersion)
	}

	docs, err := parser.ParsePODocuments(strings.NewReader(sampleHTML), "https://www.b-tu.de")
	if err != nil {
		t.Fatalf("ParsePODocuments failed: %v", err)
	}

	if len(docs) != 2 {
		t.Fatalf("expected 2 documents, got %d", len(docs))
	}
	if docs[0].DocType != "amendment" || !strings.Contains(docs[0].Title, "Satzungsänderung") {
		t.Errorf("unexpected doc 0: %+v", docs[0])
	}
	if docs[1].DocType != "statute" || !strings.Contains(docs[1].Title, "Prüfungsordnung") {
		t.Errorf("unexpected doc 1: %+v", docs[1])
	}
}

func TestParsePOContext(t *testing.T) {
	page := `<html><body>
		<div class="Kruemelpfad">
			<div class="KruemelpfadEintrag"><a class="regular" href="?nodeID=auswahlBaum">Oberste Ebene</a></div>
			<div class="KruemelpfadEintrag"><a class="regular" href="?nodeID=auswahlBaum%7Cstudiengang%3Astg%3D079">Studiengang: Informatik</a></div>
			<div class="KruemelpfadEintrag"><a class="regular" href="?nodeID=x">Module für Abschluss: Bachelor (universitär)</a></div>
			<div class="KruemelpfadEintrag"> PO-Version: 2008 - 2. SÄ 2024 </div>
		</div>
		<span class="Tree">PO-Version: 2008 - 2. SÄ 2024</span>
	</body></html>`
	ctx, err := NewProgramTreeParser().ParsePOContext(strings.NewReader(page))
	if err != nil {
		t.Fatalf("ParsePOContext failed: %v", err)
	}
	want := POContext{ProgramName: "Informatik", Degree: "Bachelor (universitär)", POVersion: "2008 - 2. SÄ 2024"}
	if ctx != want {
		t.Fatalf("ParsePOContext = %+v, want %+v", ctx, want)
	}
}
