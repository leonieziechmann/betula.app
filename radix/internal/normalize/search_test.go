package normalize

import (
	"bufio"
	"os"
	"strings"
	"testing"
)

// TestSearchTerms holds the folding of the build to testdata/search.tsv, which Folia's tests hold
// folia_search::fold to as well: a query and the titles it is compared with fold alike.
func TestSearchTerms(t *testing.T) {
	f, err := os.Open("testdata/search.tsv")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()

	kinds := map[string]func(string) string{
		"fold":     SearchFold,
		"words":    func(s string) string { return strings.Join(SearchWords(s), " ") },
		"text":     SearchText,
		"initials": SearchInitials,
		"abbrev":   SearchAbbrev,
		"fillers":  func(string) string { return strings.Join(SearchFillers, " ") },
	}
	empty := func(s string) string {
		if s == "∅" {
			return ""
		}
		return s
	}
	seen := map[string]int{}
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := scanner.Text()
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		fields := strings.Split(line, "\t")
		if len(fields) != 3 {
			t.Fatalf("testdata/search.tsv: %q is not kind<TAB>input<TAB>expected", line)
		}
		fn, ok := kinds[fields[0]]
		if !ok {
			t.Fatalf("testdata/search.tsv: unknown kind %q", fields[0])
		}
		seen[fields[0]]++
		if got, want := fn(empty(fields[1])), empty(fields[2]); got != want {
			t.Errorf("%s(%q) = %q, want %q", fields[0], fields[1], got, want)
		}
	}
	if err := scanner.Err(); err != nil {
		t.Fatal(err)
	}
	for kind := range kinds {
		if seen[kind] == 0 {
			t.Errorf("testdata/search.tsv has no line of %s", kind)
		}
	}
}
