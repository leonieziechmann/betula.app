package gemini

import (
	"github.com/jakob/btu-scraper/internal/model"
	"math"
	"regexp"
	"strings"
	"unicode"
)

var titleQualifier = regexp.MustCompile(`\s*\((?:beinhaltet|includes|inkl\.|einschließlich)[^)]*\)`)

func matchTitleKey(s string) string {
	return normalizedTitle(titleQualifier.ReplaceAllString(strings.ToLower(s), ""))
}

// Course levels and numbered sequences are identity-bearing. A near-identical
// title must never turn Mathematik II into Mathematik III.
func titleLevels(s string) string {
	var levels []string
	for _, w := range strings.FieldsFunc(strings.ToLower(s), func(r rune) bool { return !unicode.IsLetter(r) && !unicode.IsDigit(r) }) {
		if strings.ContainsAny(w, "0123456789") || w == "i" || w == "ii" || w == "iii" || w == "iv" || w == "v" {
			levels = append(levels, w)
		}
	}
	return strings.Join(levels, "|")
}

func titleSimilarity(a, b string) float64 {
	if titleLevels(a) != titleLevels(b) {
		return 0
	}
	x, y := []rune(matchTitleKey(a)), []rune(matchTitleKey(b))
	if len(x) == 0 || len(y) == 0 {
		return 0
	}
	if string(x) == string(y) {
		return 1
	}
	// Long common prefixes must not conceal a different subject, e.g.
	// "Fachwissenschaftliche Vertiefung Deutsch" versus "... Englisch".
	words := func(s string) []string {
		return strings.FieldsFunc(titleQualifier.ReplaceAllString(strings.ToLower(s), ""), func(r rune) bool { return !unicode.IsLetter(r) && !unicode.IsDigit(r) })
	}
	ax, by := words(a), words(b)
	if len(ax) != len(by) {
		return 0
	}
	for i, w := range ax {
		if w == by[i] {
			continue
		}
		short, long := []rune(w), []rune(by[i])
		if len(short) > len(long) {
			short, long = long, short
		}
		if len(short) < 6 || len(long)-len(short) > 1 {
			return 0
		}
		same := 0
		for j := 0; j < len(short); j++ {
			if short[j] == long[j] {
				same++
			}
		}
		if same*2 < len(short) {
			return 0
		}
	}
	if len(x) < 12 || len(y) < 12 {
		return 0
	}
	prev := make([]int, len(y)+1)
	for j := range prev {
		prev[j] = j
	}
	for i, c := range x {
		curr := make([]int, len(y)+1)
		curr[0] = i + 1
		for j, d := range y {
			cost := 0
			if c != d {
				cost = 1
			}
			curr[j+1] = min(curr[j]+1, prev[j+1]+1, prev[j]+cost)
		}
		prev = curr
	}
	return 1 - float64(prev[len(y)])/float64(max(len(x), len(y)))
}

func matchSimilarTitle(name string, catalog []model.CurriculumCatalogModule) *model.CurriculumCatalogModule {
	best, second, index := 0.0, 0.0, -1
	for i, c := range catalog {
		score := math.Max(titleSimilarity(name, c.TitleDE), titleSimilarity(name, c.TitleEN))
		if score > best {
			second, best, index = best, score, i
		} else if score > second {
			second = score
		}
	}
	if index < 0 || best < .92 || best-second < .06 {
		return nil
	}
	return &catalog[index]
}

func catalogTotalForSpan(res *CurriculumExtractionResult, catalog []model.CurriculumCatalogModule, total SourceCell) (float64, int, int) {
	byCell := map[string]ExtractedModule{}
	for _, m := range res.Modules {
		byCell[m.SourceCell] = m
	}
	count := map[string]int{}
	for _, s := range res.Layout.Cells {
		count[s.Table+"|"+s.Row]++
	}
	sum, covered, required := 0.0, 0, 0
	for _, s := range res.Layout.Cells {
		if s.Table != total.Table {
			continue
		}
		a, b := s.Semesters[0], s.Semesters[len(s.Semesters)-1]
		if s.CreditSemester > 0 {
			a, b = s.CreditSemester, s.CreditSemester
		}
		if a > total.Semesters[len(total.Semesters)-1] || b < total.Semesters[0] {
			continue
		}
		required++
		if a < total.Semesters[0] || b > total.Semesters[len(total.Semesters)-1] || s.Min != s.Max || s.SharedRows || count[s.Table+"|"+s.Row] != 1 {
			continue
		}
		m, ok := byCell[s.ID]
		if !ok {
			continue
		}
		c := MatchCatalogModule(m, catalog)
		if c == nil || c.Credits <= 0 {
			continue
		}
		sum += c.Credits
		covered++
	}
	return sum, covered, required
}

// IdentityConflict reports a printed module code that the catalog assigns to a
// module with a different title. Such a link must not be written.
func IdentityConflict(m ExtractedModule, c model.CurriculumCatalogModule) bool {
	return m.ModuleCode != "" && c.TitleDE != "" &&
		normalizedTitle(m.ModuleName) != normalizedTitle(c.TitleDE) &&
		normalizedTitle(m.ModuleName) != normalizedTitle(c.TitleEN)
}
