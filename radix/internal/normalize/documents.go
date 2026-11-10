package normalize

import (
	"fmt"
	"net/url"
	"path"
	"regexp"
	"strconv"
	"strings"
)

// The topics of the BTU's documents for every program that Folia names (general_document.topic):
// what the rules of GeneralTopic recognise on its page of Grundsatzordnungen, German and English.
const (
	TopicHigherEducationAct = "higher_education_act" // the Brandenburgisches Hochschulgesetz
	TopicFrameworkBachelor  = "framework_bachelor"   // the Rahmenordnung for Bachelor-Studiengänge, with its amendments
	TopicFrameworkMaster    = "framework_master"     // the Rahmenordnung for Master-Studiengänge, with its amendments
	TopicEnrolment          = "enrolment"            // the Immatrikulationsordnung, with its amendments
	TopicFUES               = "fues"                 // the Ordnung zum Fachübergreifenden Studium
)

// GeneralTopics are the topics in the order Folia shows them.
var GeneralTopics = []string{TopicFrameworkBachelor, TopicFrameworkMaster, TopicEnrolment, TopicFUES, TopicHigherEducationAct}

// GeneralTopic is what a document of the BTU's page of Grundsatzordnungen is about, by the heading
// of its section and its title, in German or English: the section „Rahmenordnung für
// Bachelor-Studiengänge" („Framework regulations for Bachelor´s degree programs") and „… für
// Master-Studiengänge" (not those for doctorates), the section „Immatrikulationsordnung"
// („Matriculation Regulations"), a title naming the Hochschulgesetz („Higher Education Act") or
// the Fachübergreifendes Studium („General Studies"). "" for every other document of the page.
func GeneralTopic(section, title string) string {
	s, t := fold(section), fold(title)
	switch {
	case containsAny(t, "hochschulgesetz", "higher education act"):
		return TopicHigherEducationAct
	case containsAny(t, "fachübergreifend", "general studies"):
		return TopicFUES
	case containsAny(s, "rahmenordnung", "framework regulation") && !containsAny(s, "promotion", "doctoral"):
		bachelor, master := strings.Contains(s, "bachelor"), strings.Contains(s, "master")
		switch {
		case bachelor && !master:
			return TopicFrameworkBachelor
		case master && !bachelor:
			return TopicFrameworkMaster
		}
	case containsAny(s, "immatrikulationsordnung", "matriculation regulation", "enrolment regulation", "enrollment regulation"):
		return TopicEnrolment
	}
	return ""
}

// GeneralKind is what a document is in its section, by the fold the page puts it in (fold, ""
// for none): 'statute' in the section itself, 'amendment' in a fold of amendments
// („Änderungssatzungen", „Amendment statutes"), 'other' in any other fold.
func GeneralKind(fold string) string {
	switch f := strings.ToLower(fold); {
	case f == "":
		return "statute"
	case containsAny(f, "änderung", "aenderung", "amendment", "amending"):
		return "amendment"
	}
	return "other"
}

var (
	// The Amtsblatt's own names before the number of an issue: „AMbl-13_2016-RahmenO-BA.pdf",
	// „Amtsblatt_36_2017_…", „Abl_12.pdf".
	reIssuePrefix = regexp.MustCompile(`(?i)^(?:ambl|abl|amtsblatt)[-_ ]?`)
	// The number of the issue, and the year where the name of the file states it after it:
	// „13_2016", „1_2021", „39_24"; „06_3.AES…" and „29_RahmenO" state none.
	reIssueLead = regexp.MustCompile(`^0*(\d{1,3})(?:[-_]((?:19|20)\d{2}|\d{2})(?:\D|$))?`)
	// The year the old OPUS filed a document under: „opus.kobv.de/btu/volltexte/2010/1748/…".
	reOPUSYear = regexp.MustCompile(`/volltexte/((?:19|20)\d{2})/`)
	// A date in a title: „12. September 2016", „26. September.2021", „26 January 2021", „29.08.2024".
	reTitleDate = regexp.MustCompile(`(?i)\b\d{1,2}\.?\s*(?:januar|january|februar|february|märz|march|april|mai|may|juni|june|juli|july|august|september|oktober|october|november|dezember|december)[\s.]*((?:19|20)\d{2})\b|\b\d{1,2}\.\d{1,2}\.((?:19|20)\d{2})\b`)
)

// GazetteIssue is the issue of the BTU's Amtsblatt a document was published in, „13/2016", from
// its address (and its title), or "" where it cannot be told. The number of the issue begins the
// name of the file („AMbl-13_2016-RahmenO-BA.pdf", „29_RahmenO.pdf"), of a file on OPUS (where the
// BTU publishes its Amtsblatt) or one named after the Amtsblatt („AMbl-…", „Amtsblatt_…") anywhere
// else; a translation the BTU keeps on its own server („29-RahmnO BA_5-Aenderungssatzung_EN.pdf")
// has none, though its name begins with a number. The year is the one the name of the file states
// after the number („13_2016", „39_24"), else the year the old OPUS filed it under
// („volltexte/2010/"), else the year of the first date in its title („vom 25. April 2023"): the
// date of the regulation, whose issue is of the same year as a rule. A title alone never decides,
// and the dates the BTU's page writes are not always right („Zweite Änderungssatzung … vom 12.
// September 2016" of 17/2022, which the file's name says).
func GazetteIssue(address, title string) string {
	u, err := url.Parse(address)
	if err != nil {
		return ""
	}
	file := path.Base(u.Path)
	if unescaped, err := url.PathUnescape(file); err == nil {
		file = unescaped
	}
	host := strings.ToLower(u.Hostname())
	onOPUS := host == "opus4.kobv.de" || host == "opus.kobv.de"
	rest := reIssuePrefix.ReplaceAllString(file, "")
	if rest == file && !onOPUS {
		return ""
	}
	m := reIssueLead.FindStringSubmatch(rest)
	if m == nil {
		return ""
	}
	number, _ := strconv.Atoi(m[1])
	if number == 0 {
		return ""
	}
	year := 0
	switch {
	case m[2] != "":
		year, _ = strconv.Atoi(m[2])
		if year < 100 {
			year += 2000
		}
	case reOPUSYear.MatchString(u.Path):
		year, _ = strconv.Atoi(reOPUSYear.FindStringSubmatch(u.Path)[1])
	default:
		if d := reTitleDate.FindStringSubmatch(title); d != nil {
			year, _ = strconv.Atoi(d[1] + d[2])
		}
	}
	if year == 0 {
		return ""
	}
	return fmt.Sprintf("%02d/%d", number, year)
}
