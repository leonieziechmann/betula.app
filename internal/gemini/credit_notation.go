package gemini

import (
	"regexp"
	"strconv"
	"strings"
)

// Footnote indices are accepted only after a closed workload expression, never
// after a bare number (where 61 could genuinely mean 61 LP).
var workloadPrefix = regexp.MustCompile(`^(\d+(?:[.,]\d+)?)\s*\((\d+(?:[.,]\d+)?(?:\s*\+\s*\d+(?:[.,]\d+)?)+)\)(?:[1-9]\)?)?$`)

var unitAmount = regexp.MustCompile(`(\d+(?:[.,]\d+)?)\s*(?:LP|KP|CP|ECTS)`)

func joinedCreditAmounts(s string) (float64, bool) {
	m := unitAmount.FindAllStringSubmatch(s, -1)
	if len(m) < 2 || strings.TrimSpace(unitAmount.ReplaceAllString(s, "")) != "" {
		return 0, false
	}
	sum := 0.0
	for _, p := range m {
		a, _, ok := parseCreditAmount(p[1])
		if !ok {
			return 0, false
		}
		sum += a
	}
	return sum, true
}

func workloadParts(s string) []string {
	s = strings.TrimRight(s, "⁰¹²³⁴⁵⁶⁷⁸⁹")
	var found []string
	if p := workloadAmount.FindStringSubmatch(s); p != nil {
		found = p
	} else if p := workloadPrefix.FindStringSubmatch(s); p != nil {
		found = []string{s, p[2], p[1]}
	}
	if found != nil && bracketSum(found[1]) == amountOf(found[2]) {
		return found
	}
	if p := footnotedWorkload(s); p != nil {
		return p
	}
	return found
}

func amountOf(s string) float64 {
	v, _, _ := parseCreditAmount(s)
	return v
}

func bracketSum(s string) float64 {
	sum := 0.0
	for _, piece := range strings.Split(s, "+") {
		sum += amountOf(strings.TrimSpace(piece))
	}
	return sum
}

var (
	workloadGroup = regexp.MustCompile(`\(\s*\d+(?:[.,]\d+)?(?:\s*\+\s*\d+(?:[.,]\d+)?)+\s*\)`)
	plainNumber   = regexp.MustCompile(`^\d+(?:[.,]\d+)?$`)
	// "6(1+2)5)": the credit value is printed in front, the bracket is a
	// teaching-hours or footnote remark that does not add up to it.
	creditWithRemark = regexp.MustCompile(`^(\d+(?:[.,]\d+)?)\s*\(\s*\d+(?:\s*\+\s*\d+)+\s*\)\s*(?:[1-9]\)|[⁰¹²³⁴⁵⁶⁷⁸⁹]+\)?)?$`)
)

// footnotedWorkload reads "(3+3) 61", "12 1 (6 + 6)" or "1 18 (9 + 9)". The
// total is the printed number that equals the bracket sum, possibly with one
// footnote digit glued on; any other token must be a single footnote digit.
func footnotedWorkload(s string) []string {
	loc := workloadGroup.FindStringIndex(s)
	if loc == nil {
		return nil
	}
	group := strings.Trim(s[loc[0]:loc[1]], "() ")
	sum := 0.0
	for _, piece := range strings.Split(group, "+") {
		v, _, ok := parseCreditAmount(strings.TrimSpace(piece))
		if !ok {
			return nil
		}
		sum += v
	}
	want := strconv.FormatFloat(sum, 'f', -1, 64)
	tokens := strings.Fields(strings.TrimRight(s[:loc[0]], " ") + " " + strings.TrimLeft(s[loc[1]:], " "))
	total, notes := "", 0
	for _, tok := range tokens {
		tok = strings.TrimRight(tok, "⁰¹²³⁴⁵⁶⁷⁸⁹)")
		if !plainNumber.MatchString(tok) {
			return nil
		}
		switch {
		case total == "" && strings.ReplaceAll(tok, ",", ".") == want:
			total = want
		case total == "" && len(tok) == len(want)+1 && strings.HasPrefix(tok, want):
			total = want // footnote digit glued to the total
		case len(tok) == 1 && tok != "0":
			notes++
		default:
			return nil
		}
	}
	if total == "" || notes > 1 {
		return nil
	}
	return []string{s, group, total}
}

// creditBeforeRemark returns N of "N(a+b)k)" cells.
func creditBeforeRemark(s string) (float64, bool) {
	p := creditWithRemark.FindStringSubmatch(strings.TrimSpace(s))
	if p == nil {
		return 0, false
	}
	v, _, ok := parseCreditAmount(p[1])
	return v, ok
}

var optionalCredit = regexp.MustCompile(`^\(\s*(\d+(?:[.,]\d+)?)\s*\)$`)

// optionalPlacement returns N of a bracketed "(N)" cell. Such a cell marks one
// of several semesters in which the module may be placed.
func optionalPlacement(s string) (float64, bool) {
	p := optionalCredit.FindStringSubmatch(strings.TrimSpace(s))
	if p == nil {
		return 0, false
	}
	v, _, ok := parseCreditAmount(p[1])
	return v, ok
}

var markedCreditCell = regexp.MustCompile(`^(\()?\s*(\d+(?:[.,]\d+)?)\s*\+\s*(\))?$`)

// markedCredit reads "12+" and "(6+)": the credit value followed by the "+"
// legend mark ("to be taken together with an integration module"). A bracket
// means the usual optional placement in several semesters.
func markedCredit(s string) (value float64, optional, ok bool) {
	p := markedCreditCell.FindStringSubmatch(strings.TrimSpace(s))
	if p == nil || (p[1] == "") != (p[3] == "") {
		return 0, false, false
	}
	v, _, valid := parseCreditAmount(p[2])
	return v, p[1] != "", valid
}
