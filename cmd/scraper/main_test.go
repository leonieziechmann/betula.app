package main

import (
	"flag"
	"testing"
)

func TestCurriculumValueFlagsRemainPaired(t *testing.T) {
	fs := flag.NewFlagSet("test", flag.ContinueOnError)
	id := fs.String("program-id", "", "")
	pdf := fs.String("pdf", "", "")
	term := fs.String("start-term", "", "")
	tol := fs.Float64("credit-tolerance", 0, "")
	report := fs.String("report-dir", "", "")
	dry := fs.Bool("dry-run", false, "")
	err := fs.Parse(reorderFlags([]string{"--program-id", "abc", "--pdf", "some file.pdf", "--start-term", "winter", "--credit-tolerance", "6", "--report-dir", "reports", "--dry-run"}))
	if err != nil || *id != "abc" || *pdf != "some file.pdf" || *term != "winter" || *tol != 6 || *report != "reports" || !*dry {
		t.Fatalf("flags lost values: %v %v", fs.Args(), err)
	}
}
