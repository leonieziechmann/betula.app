// Package curriculumscan turns the regulation documents of one study program
// into validated curriculum rows. It has no database or network dependency, so
// the command line tool, tests and batch jobs share exactly the same logic.
package curriculumscan

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/leonieziechmann/betula/internal/gemini"
	"github.com/leonieziechmann/betula/internal/model"
)

// Outcome statuses. Only the first two are written as a validated plan.
const (
	StatusSaved         = "saved"
	StatusSavedWarnings = "saved_with_warnings"
	StatusNeedsReview   = "needs_review"
	StatusNoPlan        = "no_plan"
	StatusMissingSource = "missing_source"
)

type Options struct {
	StartTerm string  // auto, winter, summer, unknown
	Tolerance float64 // LP tolerance for the semester plausibility warning
	PDFPath   string  // explicit document; overrides the program's documents
	PlanPages []int
	// Client extracts the plan; nil uses the offline (deterministic) reader.
	Client *gemini.Client
	// ResolvePath maps a stored document path to a readable file (optional).
	ResolvePath func(string) string
}

type Outcome struct {
	ProgramID  string
	Status     string
	Message    string
	Source     string
	Modules    []model.CurriculumModule
	LayoutJSON string
	Validation gemini.ValidationReport
	Reviews    []gemini.AmendmentReview
	Extraction *gemini.CurriculumExtractionResult
}

// Saveable reports whether the outcome may replace the stored plan.
func (o Outcome) Saveable() bool {
	return (o.Status == StatusSaved || o.Status == StatusSavedWarnings) && len(o.Modules) > 0
}

// NormalizePath makes a stored document path usable on the running host. The
// downloader writes the separator of the machine it ran on, so a database built
// on Windows carries "statutes\\Informatik\\x.pdf"; the same database is meant to
// be served from a Linux host as well.
func NormalizePath(p string) string {
	if filepath.Separator == '/' {
		return strings.ReplaceAll(p, "\\", "/")
	}
	return strings.ReplaceAll(p, "/", "\\")
}

func (o Options) resolve(p string) string {
	if o.ResolvePath != nil {
		return o.ResolvePath(p)
	}
	return NormalizePath(p)
}

func available(prog model.OfficialStudyProgram, opt Options) []model.ProgramRegulationDocument {
	var docs []model.ProgramRegulationDocument
	seen := map[string]bool{}
	for _, d := range prog.Documents {
		path := opt.resolve(d.LocalPath)
		if path == "" || seen[path] {
			continue
		}
		if fi, err := os.Stat(path); err == nil && fi.Size() > 0 {
			seen[path] = true
			d.LocalPath = path
			docs = append(docs, d)
		}
	}
	return docs
}

func publicationRank(d model.ProgramRegulationDocument) string { return d.Title }

// chooseSources decides which document holds the study plan. The
// Prüfungsordnung is authoritative; amendments are reviewed, not parsed.
func chooseSources(prog model.OfficialStudyProgram, docs []model.ProgramRegulationDocument) (gemini.RegulationSelection, []string, error) {
	var notes []string
	var statutes, amendments []model.ProgramRegulationDocument
	for _, d := range docs {
		if d.DocType == "statute" {
			statutes = append(statutes, d)
		} else {
			amendments = append(amendments, d)
		}
	}
	switch {
	case len(statutes) == 1:
		sel, err := gemini.SelectRegulationSources(prog, docs)
		return sel, notes, err
	case len(statutes) > 1:
		// Several base documents (e.g. old and new version): the latest publication wins.
		sort.SliceStable(statutes, func(i, j int) bool { return publicationRank(statutes[i]) < publicationRank(statutes[j]) })
		base := statutes[len(statutes)-1]
		notes = append(notes, fmt.Sprintf("Mehrere Prüfungsordnungen vorhanden; die zuletzt veröffentlichte wurde verwendet: %s", base.Title))
		sel, err := gemini.SelectRegulationSources(prog, append([]model.ProgramRegulationDocument{base}, amendments...))
		return sel, notes, err
	}
	// Only amendments were published for this program version.
	if len(amendments) == 0 {
		return gemini.RegulationSelection{}, notes, fmt.Errorf("no regulation documents")
	}
	sel := gemini.RegulationSelection{Plan: amendments[len(amendments)-1]}
	sel.Base = sel.Plan
	notes = append(notes, "Nur Satzungsänderung(en) veröffentlicht; keine eigene Prüfungsordnung zu dieser Version")
	return sel, notes, nil
}

// Scan extracts and validates the study plan of one program. The catalog is the
// one the program sees: every module, and the ones the program itself claims.
func Scan(ctx context.Context, prog model.OfficialStudyProgram, catalog model.CurriculumCatalog, opt Options) Outcome {
	out := Outcome{ProgramID: prog.ID}
	client := opt.Client
	if client == nil {
		client = gemini.NewClient("", "")
	}
	client.PlanPages = append([]int(nil), opt.PlanPages...)

	var sel gemini.RegulationSelection
	var notes []string
	var candidates []string
	if opt.PDFPath != "" {
		sel.Plan = model.ProgramRegulationDocument{LocalPath: opt.PDFPath}
		candidates = []string{opt.PDFPath}
	} else {
		docs := available(prog, opt)
		if len(docs) == 0 {
			out.Status, out.Message = StatusMissingSource, "Keine heruntergeladene Prüfungsordnung vorhanden"
			return out
		}
		var err error
		sel, notes, err = chooseSources(prog, docs)
		if err != nil {
			out.Status, out.Message = StatusNeedsReview, err.Error()
			return out
		}
		client.PlanPages = append([]int(nil), sel.Pages...)
		candidates = []string{sel.Plan.LocalPath}
		// Only amendments: try the others too if the newest has no plan table.
		if sel.Plan.DocType != "statute" {
			candidates = nil
			for i := len(docs) - 1; i >= 0; i-- {
				candidates = append(candidates, docs[i].LocalPath)
			}
		}
	}

	hint := prog.ProgramName + " / " + prog.Degree + " / PO " + prog.POVersion
	var res *gemini.CurriculumExtractionResult
	var err error
	var degraded string
	for _, path := range candidates {
		out.Source = path
		res, err = client.ExtractCurriculumFromPDF(ctx, path, hint)
		if err != nil && !gemini.IsLayoutError(err) {
			// The document was read; only the enrichment API failed. Semester
			// columns, credits and names come from the PDF cells either way, so
			// the plan is kept and the lost enrichment is reported.
			if fallback, offlineErr := client.ExtractCurriculumOffline(ctx, path, hint); offlineErr == nil {
				degraded = err.Error()
				res, err = fallback, nil
			}
		}
		if err == nil {
			break
		}
	}
	if err != nil {
		out.Message = err.Error()
		if strings.Contains(out.Message, "no supported ruled semester") {
			out.Status = StatusNoPlan
		} else {
			out.Status = StatusNeedsReview
		}
		return out
	}
	if len(sel.Reviews) > 0 {
		if applyErr := sel.Apply(res); applyErr != nil {
			sel.Issues = append(sel.Issues, gemini.ValidationIssue{Severity: "error", Code: "amendment_requires_patch", Message: applyErr.Error()})
		}
	}
	out.Reviews = sel.Reviews
	out.Extraction = res
	v := gemini.ValidateCurriculum(res, catalog, opt.StartTerm, opt.Tolerance)
	v.Issues = append(v.Issues, sel.Issues...)
	if degraded != "" {
		v.Issues = append(v.Issues, gemini.ValidationIssue{Severity: "warning", Code: "enrichment_unavailable",
			Message: "Studienplan aus den PDF-Zellen übernommen, ohne KI-Anreicherung: " + degraded})
	}
	for _, n := range notes {
		v.Issues = append(v.Issues, gemini.ValidationIssue{Severity: "info", Code: "source_note", Message: n})
	}
	for _, issue := range sel.Issues {
		if issue.Severity == "error" {
			v.Valid = false
		}
	}
	out.Validation = v
	if !v.Valid {
		out.Status = StatusNeedsReview
		for _, issue := range v.Issues {
			if issue.Severity == "error" {
				out.Message = issue.Code + ": " + issue.Message
				break
			}
		}
		return out
	}
	out.Modules = BuildModules(prog, res, catalog, out.Source)
	layout, _ := json.Marshal(res.Layout)
	out.LayoutJSON = string(layout)
	out.Status = StatusSaved
	for _, issue := range v.Issues {
		if issue.Severity == "warning" {
			out.Status = StatusSavedWarnings
			out.Message = issue.Code + ": " + issue.Message
			break
		}
	}
	return out
}

// BuildModules converts extracted requirements into database rows.
func BuildModules(prog model.OfficialStudyProgram, res *gemini.CurriculumExtractionResult, catalog model.CurriculumCatalog, source string) []model.CurriculumModule {
	evidence := map[string]string{}
	if res.Layout != nil {
		for _, cell := range res.Layout.Cells {
			data, _ := json.Marshal(cell)
			evidence[cell.ID] = string(data)
		}
	}
	var rows []model.CurriculumModule
	for _, m := range res.Modules {
		rows = append(rows, model.CurriculumModule{
			SourceEvidence: evidence[m.SourceCell], ProgramID: prog.ID, ModuleID: gemini.LinkModule(m, catalog),
			ProgramName: prog.ProgramName, Degree: prog.Degree, POVersion: prog.POVersion,
			ModuleCode: m.ModuleCode, ModuleName: m.ModuleName, ModuleNameEN: m.ModuleNameEN,
			RecommendedSemester: m.RecommendedSemester, RecommendedSemesterRaw: m.RecommendedSemesterRaw,
			SemesterSpan: m.SemesterSpan, StartSemester: m.StartSemester, EndSemester: m.EndSemester,
			Credits: m.Credits, MinCredits: m.MinCredits, MaxCredits: m.MaxCredits,
			ModuleType: m.ModuleType, StudySection: m.StudySection, SubjectArea: m.SubjectArea,
			AreaRules: m.AreaRules, Specialization: m.Specialization, SWS: m.SWS, ExamType: m.ExamType,
			Graded: m.Graded, Prerequisites: m.Prerequisites, Remarks: m.Remarks, SourceFile: source,
		})
	}
	return rows
}
