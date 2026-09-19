package main

import (
	"bytes"
	"context"
	"flag"
	"fmt"
	"os"
	"sort"

	"github.com/leonieziechmann/btu-scraper/internal/catalogdb"
	"github.com/leonieziechmann/btu-scraper/internal/parser"
)

// runRawVocab prints the distinct raw values of the module-page fields that get
// normalized, most frequent first. It is the evidence for the normalization rules.
func runRawVocab(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("raw-vocab", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	top := fs.Int("top", 25, "Values to print per field")
	_ = fs.Parse(args)

	db := openDB(*dbPath)
	defer db.Close()

	fields := []string{"exam_type", "grading", "limitation", "degree", "regulation", "language", "duration", "turnus", "teaching_form"}
	counts := make(map[string]map[string]int)
	for _, f := range fields {
		counts[f] = make(map[string]int)
	}

	pages := 0
	p := parser.NewDetailParser()
	err := db.EachPage(catalogdb.SourceModulePage, func(page *catalogdb.RawPage) error {
		if page.HTTPStatus != 200 {
			return nil
		}
		d, err := p.Parse(bytes.NewReader(page.Body), page.Key, page.URL)
		if err != nil {
			return err
		}
		pages++
		counts["exam_type"][d.ExamType]++
		counts["grading"][d.Grading]++
		counts["limitation"][d.Limitation]++
		counts["language"][d.Language]++
		counts["duration"][d.Duration]++
		counts["turnus"][d.Turnus]++
		for _, sp := range d.StudyPrograms {
			counts["degree"][sp.Degree]++
			counts["regulation"][sp.Regulation]++
		}
		for _, tf := range d.TeachingForms {
			counts["teaching_form"][tf.Type]++
		}
		return nil
	})
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("%d module pages parsed\n", pages)
	for _, f := range fields {
		type kv struct {
			value string
			n     int
		}
		var list []kv
		for v, n := range counts[f] {
			list = append(list, kv{v, n})
		}
		sort.Slice(list, func(i, j int) bool {
			if list[i].n != list[j].n {
				return list[i].n > list[j].n
			}
			return list[i].value < list[j].value
		})
		fmt.Printf("\n== %s (%d distinct)\n", f, len(list))
		for i, e := range list {
			if i == *top {
				break
			}
			fmt.Printf("%6d  %q\n", e.n, e.value)
		}
	}
}
