package catalogbuild

import (
	"regexp"
	"strings"

	"github.com/leonieziechmann/btu-scraper/internal/normalize"
)

const (
	kindCompulsory = normalize.KindCompulsory
	kindElective   = normalize.KindElective
	kindThesis     = normalize.KindThesis
	kindInternship = normalize.KindInternship
	kindFUES       = normalize.KindFUES
)

// remarkStatement is one sentence of a module page's remarks that places the
// module in a program, e.g.
//
//	„Studiengang Mathematik B.Sc.: Wahlpflichtmodul im Komplex „Vertiefung""
//	"Study programme Informatik M.Sc.: Compulsory elective module in complex „Praktische Informatik""
type remarkStatement struct {
	programName string
	degreeLabel string // normalized, „B.Sc."
	kind        string // "" when the sentence names no kind or names several
	area        string
}

var (
	remarkHeader = regexp.MustCompile(`(?:Studiengang|Study programme|Study program)\s+(.{2,80}?)\s+((?:B|M)\.\s?(?:Sc|A|Eng|Ed|Mus)\.|LL\.\s?(?:B|M)\.)[^:•]{0,20}:`)
	remarkArea   = regexp.MustCompile(`(?i)(?:modulkomplex|komplex|complex)\s*:?\s*[„“"”‚']\s*([^„“"”‚']{2,120}?)\s*[“"”‘']`)
)

func parseRemarkStatements(remarks string) []remarkStatement {
	headers := remarkHeader.FindAllStringSubmatchIndex(remarks, -1)
	var result []remarkStatement
	for i, h := range headers {
		end := len(remarks)
		if i+1 < len(headers) {
			end = headers[i+1][0]
		}
		body := remarks[h[1]:end]
		if j := strings.Index(body, "•"); j >= 0 {
			body = body[:j]
		}

		labels := normalize.ShortDegreeLabels(remarks[h[4]:h[5]])
		if len(labels) == 0 {
			continue
		}
		st := remarkStatement{
			programName: strings.TrimSpace(remarks[h[2]:h[3]]),
			degreeLabel: labels[0],
			kind:        statedKindInText(body),
		}
		if m := remarkArea.FindStringSubmatch(body); m != nil {
			st.area = strings.TrimSpace(m[1])
		}
		result = append(result, st)
	}
	return result
}

// statedKindInText returns a kind only when the text names exactly one.
// „Pflichtmodul in Studienrichtung A, Wahlpflichtmodul in Studienrichtung B" names two.
func statedKindInText(text string) string {
	low := strings.ToLower(text)
	elective := containsAny(low, "wahlpflicht", "wahlmodul", "wahflicht", "compulsory elective", "elective module", "optional module")
	// Remove the elective words before looking for the compulsory ones they contain.
	for _, w := range []string{"wahlpflicht", "compulsory elective"} {
		low = strings.ReplaceAll(low, w, "")
	}
	compulsory := containsAny(low, "pflichtmodul", "pflicht-modul", "compulsory module", "mandatory module")
	switch {
	case elective && !compulsory:
		return kindElective
	case compulsory && !elective:
		return kindCompulsory
	}
	return ""
}

// „Wahlpflicht", „Wahlbereich", „Wahlmodule" …, but not „Auswahl".
var (
	electiveLabel   = regexp.MustCompile(`\bwahl|\bwpf\b|elective|optional`)
	compulsoryLabel = regexp.MustCompile(`\bpflicht|compulsory|mandatory|obligator`)
)

var thesisTitle = regexp.MustCompile(`(?i)\b(bachelor|master|diplom)[- ]?(arbeit|thesis)\b|abschlussarbeit`)

// kindFromAreaLabels reads the kind from the labels of a tree path, deepest label
// first. A path without such a word states nothing: there is no default.
func kindFromAreaLabels(labels []string) string {
	for i := len(labels) - 1; i >= 0; i-- {
		low := strings.ToLower(labels[i])
		switch {
		case electiveLabel.MatchString(low):
			return kindElective
		case compulsoryLabel.MatchString(low):
			return kindCompulsory
		case containsAny(low, "abschlussarbeit", "bachelor-arbeit", "bachelorarbeit", "master-arbeit", "masterarbeit", "thesis", "abschlussmodul"):
			return kindThesis
		case containsAny(low, "berufspraktikum", "industriepraktikum", "fachpraktikum", "praxisphase", "praxissemester", "internship"):
			return kindInternship
		case containsAny(low, "fachübergreifend", "füs", "general studies"):
			return kindFUES
		}
	}
	return ""
}

// sectionFromAreaLabels reads the study section from a tree path, top label first.
func sectionFromAreaLabels(labels []string) string {
	for _, label := range labels {
		low := strings.ToLower(label)
		switch {
		case containsAny(low, "grundstudium", "basisstudium"):
			return "basic"
		case containsAny(low, "fachstudium", "hauptstudium"):
			return "main"
		case strings.Contains(low, "vertiefungsstudium"):
			return "specialization"
		case strings.Contains(low, "kernstudium"):
			return "core"
		}
	}
	return ""
}

func containsAny(s string, subs ...string) bool {
	for _, sub := range subs {
		if strings.Contains(s, sub) {
			return true
		}
	}
	return false
}
