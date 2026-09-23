package gemini

import (
	"context"
	"crypto/sha256"
	"fmt"
	"os"
	"regexp"
	"strings"
)

var (
	// A name that is a thesis, not one that mentions it: „PhD Thesis Writing
	// Skills" and „Status Seminar ERM: Progress Reports PhD Thesis" are courses.
	thesisTitle = regexp.MustCompile(`(?i)\b(bachelor|master|diplom|doctoral)(?:'s|’s)?[- ]?(arbeit|thesis)\b|\babschlussarbeit\b|\bdissertation\b|\bkolloquium\b`)
	// An internship is named by where it is served or by what it is in the
	// degree: „Berufspraktikum", „Industriefachpraktikum", „Außeruniversitäres
	// Praktikum", „Pflichtpraktikum", „Industrial Internship", a bare
	// „Praktikum", and „Praktikum Maschinenbau", the internship of the program
	// of that name. A lab course is named by its subject („Programmierpraktikum",
	// „Physikalisches Praktikum I", „Praktikum Maschinelles Lernen"), and a
	// Lehramt module by the school practicum it contains („Fachdidaktik
	// Mathematik (beinhaltet fachdidaktisches Tagespraktikum, fTP)").
	internshipName = regexp.MustCompile(`(?i)\b(?:bachelor-?|berufs(?:feld)?|betriebs|forschungs|industrie(?:fach)?|ingenieur|integrations|pflicht|wirtschafts)praktikum\b|\bau(?:ß|ss)eruniversitäres\s+praktikum\b|^(?:\d{5,6}\s+)?praktikum\W*$|\bpraktikum\s+(?:dual|maschinenbau|wirtschaftsingenieurwesen)\b|\bpraxisphase\b|\bpraxismodul\b|\binternship\b`)
	// The Fachübergreifendes Studium itself, not every module that crosses
	// subjects: „Fachübergreifende Projektarbeit" is a module of its own. A
	// wrapped label may break the word („Fachübergreifenden Stu- / dium").
	fuesName = regexp.MustCompile(`(?i)f(?:ach|ächer)übergreifende[ns]?\s+stu(?:-\s*/?\s*)?dium|\bFÜS\b`)
)

// ClassifyRequirement derives the module type from what the source itself
// shows: the printed style legend (Elective) and unmistakable module names.
// It never guesses from position or credits.
func ClassifyRequirement(c SourceCell) string {
	switch {
	case thesisTitle.MatchString(c.Row):
		return "Abschlussarbeit"
	case internshipName.MatchString(c.Row):
		return "Praktikum"
	case fuesName.MatchString(c.Row):
		return "FÜS"
	case c.Elective || c.AltGroup > 0 || c.SharedRows:
		return "Wahlpflicht"
	}
	return "Pflicht"
}

// ExtractCurriculumOffline builds the study plan from the PDF geometry alone.
// Semester, credits, names and requirement type never depended on the language
// model (BindSourceCells overwrites them from the source cells), so the API is
// optional: no key, no network and the same, reproducible result.
func (c *Client) ExtractCurriculumOffline(ctx context.Context, pdfPath, programHint string) (*CurriculumExtractionResult, error) {
	pdfBytes, err := os.ReadFile(pdfPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read PDF file %s: %w", pdfPath, err)
	}
	var layout *PDFLayout
	if c.layoutLoader != nil {
		layout, err = c.layoutLoader(ctx, pdfPath)
	} else {
		layout, err = ReadPDFLayoutForProgram(ctx, pdfPath, c.PlanPages, programHint)
	}
	if err != nil {
		return nil, err
	}
	selectProgramMode(layout, programHint)
	res := &CurriculumExtractionResult{}
	for _, cell := range layout.Cells {
		res.Modules = append(res.Modules, ExtractedModule{SourceCell: cell.ID, ModuleName: cell.Row, ModuleType: ClassifyRequirement(cell)})
	}
	if err := BindSourceCells(res, layout); err != nil {
		return nil, err
	}
	res.SourceSHA256 = fmt.Sprintf("%x", sha256.Sum256(pdfBytes))
	return res, nil
}

// layoutErrorMarkers identify a rejection by the deterministic PDF reader, as
// opposed to a failure of the enrichment API.
var layoutErrorMarkers = []string{
	"no supported ruled semester",
	"ambiguous PDF layout",
	"incomplete table",
	"unsupported or malformed PDF content",
	"failed to read PDF file",
}

// IsLayoutError reports whether the document itself was rejected. Such a result
// is authoritative and must not be retried without the model; every other
// failure (API, quota, network, malformed model answer) may fall back to the
// deterministic reader.
func IsLayoutError(err error) bool {
	if err == nil {
		return false
	}
	msg := err.Error()
	for _, marker := range layoutErrorMarkers {
		if strings.Contains(msg, marker) {
			return true
		}
	}
	return false
}
