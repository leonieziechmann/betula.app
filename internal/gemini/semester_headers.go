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
