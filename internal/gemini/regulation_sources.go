package gemini

import (
	"context"
	"crypto/sha256"
	"fmt"
	"github.com/jakob/btu-scraper/internal/model"
	"os"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

type AmendmentReview struct {
	Source   string `json:"source"`
	SHA256   string `json:"sha256"`
	Decision string `json:"decision"`
	Evidence string `json:"evidence"`
}
type RegulationSelection struct {
	Base             model.ProgramRegulationDocument
	Plan             model.ProgramRegulationDocument
	Pages            []int
	Reviews          []AmendmentReview
	Issues           []ValidationIssue
	Corrections      map[string]string
	TextReplacements map[string]string
}

var publicationNumber = regexp.MustCompile(`(\d{1,2})/(\d{4})`)

func publicationOrder(d model.ProgramRegulationDocument) int {
	m := publicationNumber.FindStringSubmatch(d.Title)
	if m == nil {
		return 0
	}
	n, _ := strconv.Atoi(m[1])
	y, _ := strconv.Atoi(m[2])
	return y*100 + n
}

// Reviews are pinned to file content, not a filename or a document-type icon.
// These decisions came from reading the complete amendment. A changed or new
// amendment remains visible for review, never silently classified as irrelevant.
func SelectRegulationSources(p model.OfficialStudyProgram, docs []model.ProgramRegulationDocument) (RegulationSelection, error) {
	s := RegulationSelection{Corrections: map[string]string{}, TextReplacements: map[string]string{}}
	if len(docs) == 0 {
		return s, fmt.Errorf("no regulation documents")
	}
	if len(docs) == 1 {
		s.Base, s.Plan = docs[0], docs[0]
		return s, nil
	}
	count := 0
	for _, d := range docs {
		if d.DocType == "statute" {
			s.Base = d
			count++
		}
	}
	if count != 1 {
		return s, fmt.Errorf("expected one base regulation, found %d", count)
	}
	s.Plan = s.Base
	ordered := append([]model.ProgramRegulationDocument(nil), docs...)
	sort.SliceStable(ordered, func(i, j int) bool { return publicationOrder(ordered[i]) < publicationOrder(ordered[j]) })
	master := strings.Contains(strings.ToLower(p.Degree), "master")
	for _, d := range ordered {
		if d.LocalPath == s.Base.LocalPath {
			continue
		}
		b, e := os.ReadFile(d.LocalPath)
		if e != nil {
			return s, e
		}
		hash := fmt.Sprintf("%x", sha256.Sum256(b))
		r := AmendmentReview{Source: d.LocalPath, SHA256: hash, Decision: "not_applied", Evidence: "Amendment not applied: project decision - amendments only add new rules, the Prüfungsordnung is authoritative"}
		switch hash {
		case "41ab9948f515c96f364dba44dbab2be9110641376e80510e06484adf2f322f79":
			r.Decision = "no_plan_change"
			r.Evidence = "AMbl. 06/2022, p.2, Art.1: only Anlage 7 participation fees; semester allocation and credits unchanged."
		case "196c8e932d816b809cbec1e380731c5b6c54b30fa2db35e8d17c618b4e5d0629":
			if p.ProgramName == "Angewandte Mathematik" && master {
				r.Decision = "no_plan_change"
				r.Evidence = "AMbl. 13/2021, p.4, Art.1: learning objectives and 30 hours per LP; 120 LP remain unchanged."
			}
		case "ec3be0d87ae9cfffcd0ad4120655aa754942807dcee7915f1e87866ae6925e46":
			r.Decision = "no_plan_change"
			r.Evidence = "AMbl. 14/2021, p.2–3, Art.1: learning objectives and workload definition; four semesters, 120 LP remain unchanged."
		case "728ecf4219bbd10506fd1cd16d70c15c249953f77309c351719abf7ceeb22b1f":
			r.Decision = "no_plan_change"
			r.Evidence = "AMbl. 04/2023, p.2, Art.1: internship learning objectives and sentence numbering only."
		case "6b290d04eed8ba68fd2f8a4bee122a6737b8f4d7adfb8ef407f62e35d75a70b0":
			r.Decision = "no_plan_change"
			r.Evidence = "AMbl. 03/2023, p.2, Art.1: Anlage 3 internship learning objectives only."
		case "4fa28befc2de6854686ec6b9081b364a43904fff01395d5b101332e2dccfa685":
			r.Decision = "code_correction"
			r.Evidence = "AMbl. 16/2021, p.2: Bachelor Anlage 6, Abb.6.2 corrects 13151 to 12951; p.3–4 changes profile terminology only."
			if !master {
				s.Corrections["13151"] = "12951"
			}
		case "0d3478a09a52ec915e7c3737fbc6bbe45dfe61bbd45d07ba92eca4297e65c209":
			r.Decision = "consolidated_regulation"
			r.Evidence = "AMbl. 28/2024: amendment concerns entrance assessment; complete consolidated regulation follows from p.4, incorporating previous amendments."
			s.Plan = d
			s.Pages = nil
		case "bdf8668f175281ae9c1f0a4bd58284c27d781a896677335c6373c5e570f27b44":
			if master {
				r.Decision = "replacement_plan"
				r.Evidence = "AMbl. 02/2023, Art.1 no.4, p.3: replaces Anlage 2 in full."
				s.Plan = d
				s.Pages = []int{3}
			}
		case "4185bd38a5bdaf6ff497390fb2959eb95b5ccab849c04449651e86bc05400d39":
			if master {
				r.Decision = "replacement_plan"
				r.Evidence = "AMbl. 15/2020, p.3, Art.1 no.9: replacement Master plan; superseded when a later replacement is available."
				s.Plan = d
				s.Pages = []int{3}
			} else {
				r.Decision = "module_replacement"
				r.Evidence = "AMbl. 15/2020, p.2, Art.1 no.2–5: in the MIT/EET alternative slot replace Grundzüge der Mikrocontrollertechnik (12839, MIT) by Datenbanken (12330, MIT); EET alternative and semester/LP stay unchanged. Removed electives do not change the elective LP budget."
				s.TextReplacements["Grundzüge der Mikrocontrollertechnik"] = "Datenbanken"
			}
		case "7321d4efc57ba626c7472f52fda852fb2f7fcc613c9597e9bc19bce03dc23820":
			if !master {
				r.Decision = "replacement_plan"
				r.Evidence = "AMbl. 14/2007, Art.1 no.6, p.3: replacement Bachelor semester/credit matrix."
				s.Plan = d
				s.Pages = []int{3}
			}
		}
		if r.Decision == "not_applied" {
			// An unreviewed amendment is harmless unless it carries a study plan
			// of its own; then a human has to decide which one applies.
			if _, perr := ReadPDFLayout(context.Background(), d.LocalPath); perr == nil {
				r.Decision = "needs_review"
				r.Evidence = "Amendment contains a study plan table; check whether it replaces the plan of the Prüfungsordnung"
				s.Issues = append(s.Issues, ValidationIssue{Severity: "warning", Code: "amendment_contains_plan", Message: r.Source + ": " + r.Evidence})
			}
		}
		s.Reviews = append(s.Reviews, r)
	}
	return s, nil
}

func (s RegulationSelection) Apply(res *CurriculumExtractionResult) error {
	res.Layout.Amendments = s.Reviews
	for old, newTitle := range s.TextReplacements {
		found := 0
		for i := range res.Layout.Cells {
			c := &res.Layout.Cells[i]
			if strings.Contains(c.Row, old) {
				found++
				c.OriginalRow = c.Row
				c.Row = strings.ReplaceAll(c.Row, old, newTitle)
				c.Row = strings.ReplaceAll(c.Row, "12839 ", "12330 ")
			}
		}
		if found == 0 {
			return fmt.Errorf("amendment replacement source title not found: %s", old)
		}
	}
	for i := range res.Layout.Cells {
		c := &res.Layout.Cells[i]
		m := sourceModuleCode.FindStringSubmatch(c.Row)
		if m == nil {
			continue
		}
		if replacement := s.Corrections[m[1]]; replacement != "" {
			c.OriginalRow = c.Row
			c.Row = replacement + " " + m[2]
		}
	}
	return BindSourceCells(res, res.Layout)
}
