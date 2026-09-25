package abbrev

import (
	_ "embed"
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"unicode"
)

//go:embed overrides.tsv
var overridesTSV string

// Override is a line of overrides.tsv: a curated abbreviation. It is the module's first
// candidate, but it takes part in a program's resolution like any other candidate: where
// another module contests it, the module falls back to its derived candidates.
type Override struct {
	Line     int
	ModuleID string         // a module number, or
	Pattern  *regexp.Regexp // a pattern over the module's title
	Program  string         // "" = every program
	Abbrev   string         // {n} is the title's series number
	Source   string         // owner, page or common
	Note     string
}

var (
	reModuleID  = regexp.MustCompile(`^\d{5}$`)
	reProgramID = regexp.MustCompile(`^[0-9A-Z]{3}-[0-9A-Z]{2}-\d{4}$`) // 079-82-2008, G29-82-2025, 768-O8-2026
)

// Overrides returns the curated lines of the embedded overrides.tsv.
func Overrides() ([]Override, error) { return ParseOverrides(overridesTSV) }

// ParseOverrides reads the format of overrides.tsv.
func ParseOverrides(text string) ([]Override, error) {
	var out []Override
	seen := map[string]int{}
	for i, line := range strings.Split(text, "\n") {
		line = strings.TrimRight(line, "\r")
		if strings.TrimSpace(line) == "" || strings.HasPrefix(line, "#") {
			continue
		}
		cols := strings.Split(line, "\t")
		if len(cols) < 4 || len(cols) > 5 {
			return nil, fmt.Errorf("overrides line %d: want match, program, abbrev, source and an optional note separated by tabs, not %d columns", i+1, len(cols))
		}
		o := Override{Line: i + 1, Program: cols[1], Abbrev: cols[2], Source: cols[3]}
		if len(cols) > 4 {
			o.Note = cols[4]
		}
		if o.Program != "" && !reProgramID.MatchString(o.Program) {
			return nil, fmt.Errorf("overrides line %d: program %q is not a program id like 079-82-2008", i+1, o.Program)
		}
		switch match := cols[0]; {
		case reModuleID.MatchString(match):
			o.ModuleID = match
			k := match + "\t" + o.Program
			if prev, dup := seen[k]; dup {
				return nil, fmt.Errorf("overrides line %d: module %s is already on line %d", i+1, match, prev)
			}
			seen[k] = i + 1
		case len(match) > 2 && strings.HasPrefix(match, "/") && strings.HasSuffix(match, "/"):
			rx, err := regexp.Compile(match[1 : len(match)-1])
			if err != nil {
				return nil, fmt.Errorf("overrides line %d: %w", i+1, err)
			}
			o.Pattern = rx
			k := match + "\t" + o.Program
			if prev, dup := seen[k]; dup {
				return nil, fmt.Errorf("overrides line %d: pattern %s is already on line %d", i+1, match, prev)
			}
			seen[k] = i + 1
		default:
			return nil, fmt.Errorf("overrides line %d: match %q is neither a module number nor a /pattern/", i+1, match)
		}
		if bare := strings.ReplaceAll(o.Abbrev, "{n}", ""); runes(bare) < 2 || runes(bare) > 10 || strings.IndexFunc(bare, unicode.IsSpace) >= 0 {
			return nil, fmt.Errorf("overrides line %d: abbreviation %q is not 2 to 10 characters without spaces", i+1, o.Abbrev)
		}
		switch o.Source {
		case "owner", "page", "common":
		default:
			return nil, fmt.Errorf("overrides line %d: source %q is not owner, page or common", i+1, o.Source)
		}
		out = append(out, o)
	}
	return out, nil
}

// apply renders the line for a title. A pattern line applies only where it matches; {n} is
// its first group (Roman numerals as digits), and the rest of the title's designator follows.
// A module-number line takes the whole designator for {n} and adds nothing.
func (o *Override) apply(p *parsed) (string, bool) {
	if o.Pattern == nil {
		return strings.ReplaceAll(o.Abbrev, "{n}", p.desigStr), true
	}
	m := o.Pattern.FindStringSubmatch(p.title)
	if m == nil {
		return "", false
	}
	if !strings.Contains(o.Abbrev, "{n}") {
		return o.Abbrev + p.desigStr, true
	}
	n := ""
	if len(m) > 1 && m[1] != "" {
		n = m[1]
		if r, ok := roman[n]; ok {
			n = strconv.Itoa(r)
		}
	}
	rest := strings.TrimPrefix(p.desigStr, n)
	return strings.ReplaceAll(o.Abbrev, "{n}", n) + rest, true
}
