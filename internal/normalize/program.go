package normalize

import (
	"regexp"
	"strings"
)

// Degree levels.
const (
	LevelBachelor         = "bachelor"
	LevelMaster           = "master"
	LevelTeachingBachelor = "teaching_bachelor" // Lehramt
	LevelTeachingMaster   = "teaching_master"
	LevelDoctoral         = "doctoral"
	LevelAbroad           = "abroad" // „Abschluss im Ausland": exchange students, not a program of its own
	LevelNone             = "none"   // „keine Abschlussprüfung möglich"
	LevelOther            = "other"
)

// Degree types.
const (
	TypeUniversity = "university" // universitär / research-oriented
	TypeApplied    = "applied"    // anwendungsbezogen / applied
)

// Study variants. The regular form of study is the empty string.
const (
	VariantDualPractice = "dual_practice" // Duales Studium, praxisintegrierend
	VariantDualTraining = "dual_training" // Duales Studium, ausbildungsintegrierend
	VariantDoubleDegree = "double_degree"
	VariantExtended     = "extended" // erweiterte Fachsemester
	VariantReduced      = "reduced"  // verringerte Fachsemester
	VariantDistance     = "distance"
	VariantPartTime     = "part_time"
	VariantOther        = "other" // a suffix this code does not know yet
)

// DegreeInfo is the structure inside a QIS degree string such as
// „Bachelor (universitär) - Duales Studium, praxisintegrierend" or its English
// form "Bachelor (research-oriented) - Co-Op Programme with Practical Placement".
type DegreeInfo struct {
	Level   string
	Type    string
	Variant string
}

// Key identifies a degree independent of the page language, so that a German
// program list and an English module page meet on the same value.
func (d DegreeInfo) Key() string {
	return d.Level + "|" + d.Type + "|" + d.Variant
}

// Degree never fails: an unknown string becomes LevelOther and a new suffix
// becomes VariantOther. QIS truncates long English labels ("… Practical Place"),
// so variants are matched on their leading words.
func Degree(raw string) DegreeInfo {
	s := fold(raw)
	head, suffix, _ := strings.Cut(s, " - ")

	var d DegreeInfo
	switch {
	case s == "":
		return DegreeInfo{}
	case containsAny(s, "ausland", "abroad"):
		return DegreeInfo{Level: LevelAbroad}
	case containsAny(s, "keine abschlussprüfung", "no final exam"):
		return DegreeInfo{Level: LevelNone}
	case containsAny(head, "promotion", "doctoral", "doktor"):
		d.Level = LevelDoctoral
	case containsAny(head, "la bachelor", "bachelor of education"):
		d.Level = LevelTeachingBachelor
	case containsAny(head, "la master", "master of education"):
		d.Level = LevelTeachingMaster
	case strings.Contains(head, "bachelor"):
		d.Level = LevelBachelor
	case strings.Contains(head, "master"):
		d.Level = LevelMaster
	default:
		d.Level = LevelOther
	}

	switch {
	case containsAny(head, "universit", "research-oriented", "research oriented"):
		d.Type = TypeUniversity
	case containsAny(head, "anwendungsbezogen", "applied"):
		d.Type = TypeApplied
	}

	switch {
	case suffix == "":
	case containsAny(suffix, "praxisintegrierend", "practical place"):
		d.Variant = VariantDualPractice
	case containsAny(suffix, "ausbildungsintegrierend", "vocational training"):
		d.Variant = VariantDualTraining
	case containsAny(suffix, "doppelabschluss", "double degree"):
		d.Variant = VariantDoubleDegree
	case containsAny(suffix, "erweiterte", "extended"):
		d.Variant = VariantExtended
	case containsAny(suffix, "verringerte", "reduced"):
		d.Variant = VariantReduced
	case containsAny(suffix, "fernstudium", "distance"):
		d.Variant = VariantDistance
	case containsAny(suffix, "teilzeit", "part-time", "part time"):
		d.Variant = VariantPartTime
	default:
		d.Variant = VariantOther
	}
	return d
}

// LevelLabel is the readable fallback when no source states a short degree label.
func LevelLabel(level string) string {
	switch level {
	case LevelBachelor:
		return "Bachelor"
	case LevelMaster:
		return "Master"
	case LevelTeachingBachelor:
		return "Lehramt Bachelor"
	case LevelTeachingMaster:
		return "Lehramt Master"
	case LevelDoctoral:
		return "Promotion"
	}
	return ""
}

// shortDegree matches the degree abbreviations that module remarks and statute
// titles use: B.Sc., M. Sc., B.A., M.Eng., LL.M., B.Ed. …
var shortDegree = regexp.MustCompile(`\b(B|M)\.\s?(Sc|A|Eng|Ed|Mus|F\.?A)\.|\bLL\.\s?(B|M)\.`)

// ShortDegreeLabels returns the normalized abbreviations found in a text, e.g. "B.Sc.".
func ShortDegreeLabels(text string) []string {
	var labels []string
	for _, m := range shortDegree.FindAllStringSubmatch(text, -1) {
		if m[3] != "" {
			labels = append(labels, "LL."+m[3]+".")
			continue
		}
		labels = append(labels, m[1]+"."+strings.ReplaceAll(m[2], ".", "")+".")
	}
	return labels
}

// LabelMatchesLevel guards the majority vote: a „M.Sc." mention never labels a bachelor program.
func LabelMatchesLevel(label, level string) bool {
	switch level {
	case LevelBachelor, LevelTeachingBachelor:
		return strings.HasPrefix(label, "B.") || label == "LL.B."
	case LevelMaster, LevelTeachingMaster:
		return strings.HasPrefix(label, "M.") || label == "LL.M."
	}
	return false
}

// Department splits „Fakultät 1 - MINT - Mathematik, …", "Faculty 1 - …",
// „ZES - Zentrale Einrichtung Sprachen" into a unit code and a name.
func Department(raw string) (code, name string, english bool) {
	s := spaces.ReplaceAllString(strings.TrimSpace(raw), " ")
	if s == "" || s == "-" {
		return "", "", false
	}
	head, rest, found := strings.Cut(s, " - ")
	if !found {
		return "", s, false
	}
	low := strings.ToLower(head)
	english = strings.HasPrefix(low, "faculty") || containsAny(strings.ToLower(rest), "centre", "center", " and ", "faculty of")
	for _, prefix := range []string{"fakultät ", "faculty ", "fak. "} {
		if strings.HasPrefix(low, prefix) {
			head = strings.TrimSpace(head[len(prefix):])
			break
		}
	}
	return head, strings.TrimSpace(rest), english
}

var (
	slugReplacer = strings.NewReplacer("ä", "ae", "ö", "oe", "ü", "ue", "ß", "ss", "é", "e", "è", "e", "á", "a", "ó", "o", "&", " und ")
	slugInvalid  = regexp.MustCompile(`[^a-z0-9]+`)
)

// Slug transliterates to lowercase ASCII words joined by "-".
func Slug(s string) string {
	s = slugReplacer.Replace(strings.ToLower(s))
	return strings.Trim(slugInvalid.ReplaceAllString(s, "-"), "-")
}
