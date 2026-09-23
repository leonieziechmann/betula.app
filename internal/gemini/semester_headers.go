package gemini

import (
	"regexp"
	"strconv"
	"strings"
)

var (
	// "1", "1.", "1. Sem", "1. Semester (WiSe)", "Semester 1", "Sem. 1", "1. FS" ...
	semesterHeaderPrefix = regexp.MustCompile(`(?i)^(\d{1,2})\.?\s*(se|sem|sem\.|semester|fs|fachsemester)?\.?\s*(?:\(\s*(wise|sose|ws|ss)\s*\))?$`)
	semesterHeaderSuffix = regexp.MustCompile(`(?i)^(?:semester|sem\.?|fachsemester)\s*(\d{1,2})\.?\s*(?:\(\s*(wise|sose|ws|ss)\s*\))?$`)
	romanSemesters       = map[string]int{"I": 1, "II": 2, "III": 3, "IV": 4, "V": 5, "VI": 6, "VII": 7, "VIII": 8, "IX": 9, "X": 10, "XI": 11, "XII": 12}
	creditSubHeader      = map[string]bool{"lp": true, "cp": true, "kp": true, "ects": true, "sws": true, "sem": true, "sem.": true, "semester": true}
)

// parseSemesterHeader recognizes the column headings used for semester columns.
// explicit reports that the cell itself says "semester" (or is a Roman numeral),
// so no separate "Semester" caption is required elsewhere in the header.
// term is "winter" or "summer" when the heading names the season of that semester.
func parseSemesterHeader(text string) (n int, explicit bool, term string, ok bool) {
	text = cleanPDFText(text)
	if text == "" {
		return 0, false, "", false
	}
	if v, found := romanSemesters[text]; found {
		return v, true, "", true
	}
	if m := semesterHeaderPrefix.FindStringSubmatch(text); m != nil {
		n, _ = strconv.Atoi(m[1])
		return n, m[2] != "", seasonOf(m[3]), n > 0
	}
	if m := semesterHeaderSuffix.FindStringSubmatch(text); m != nil {
		n, _ = strconv.Atoi(m[1])
		return n, true, seasonOf(m[2]), n > 0
	}
	return 0, false, "", false
}

func seasonOf(s string) string {
	switch strings.ToLower(s) {
	case "wise", "ws":
		return "winter"
	case "sose", "ss":
		return "summer"
	}
	return ""
}

// isCreditSubHeader reports a row that only repeats "LP"/"CP" under the semester
// captions; it carries no requirement.
func isCreditSubHeader(row []*string, from, to int) bool {
	seen := 0
	for ci, s := range row {
		if ci < from || ci > to {
			continue
		}
		v := strings.ToLower(cellText(s))
		if v == "" {
			continue
		}
		if !creditSubHeader[v] {
			return false
		}
		seen++
	}
	return seen > 0
}

// A plan run across partner universities prints, beside the semester number,
// the place that semester is spent: „1 ECN", „2 UNIZG", „3 BTU", „4 Thesis".
// The number is still the semester's own and the label names nothing else, so
// the column means what a bare „1" means — and the label is worth keeping, for
// it is all the plan says about where that semester happens.
//
// The label is a single short name, never a unit of credit or a period of study:
// „1. Studienjahr" spans two semesters and reading it as the first one would put
// every module in the wrong semester and still add up, which is the one error
// ValidateCurriculum cannot catch.
var semesterHeaderSiteLabel = regexp.MustCompile(`^(\d{1,2})\.?\s+(\p{L}[\p{L}\-.]{1,15})$`)

var notASemesterLabel = regexp.MustCompile(`(?i)^(?:jahr|studienjahr|ausbildungsjahr|year|block|blockwoche|woche|monat|quartal|trimester|abschnitt|phase|stufe|teil|modul|module|lp|kp|cp|ects|sws|std|h)\.?$`)

// parseSemesterHeaderSite is parseSemesterHeader with the site label allowed,
// returning it as caption. It is only used as a second pass: a table whose
// semester columns are already headed by plain numbers is read exactly as before.
func parseSemesterHeaderSite(text string, withSite bool) (n int, explicit bool, term, caption string, ok bool) {
	if n, explicit, term, ok = parseSemesterHeader(text); ok || !withSite {
		return n, explicit, term, "", ok
	}
	m := semesterHeaderSiteLabel.FindStringSubmatch(cleanPDFText(text))
	if m == nil || notASemesterLabel.MatchString(m[2]) {
		return 0, false, "", "", false
	}
	n, _ = strconv.Atoi(m[1])
	return n, false, "", m[2], n > 0
}
