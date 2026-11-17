package fuescatalog

import (
	"bytes"
	"fmt"
	"sort"

	"github.com/ledongthuc/pdf"
)

// ReadRows reads the text rows of a PDF, page by page, top to bottom, each row's pieces left to
// right. A PDF the reader cannot take apart is an error, not a panic.
func ReadRows(body []byte) (rows []Row, err error) {
	defer func() {
		if r := recover(); r != nil {
			rows, err = nil, fmt.Errorf("read the PDF: %v", r)
		}
	}()
	reader, err := pdf.NewReader(bytes.NewReader(body), int64(len(body)))
	if err != nil {
		return nil, fmt.Errorf("open the PDF: %w", err)
	}
	for n := 1; n <= reader.NumPage(); n++ {
		page := reader.Page(n)
		if page.V.IsNull() {
			continue
		}
		byRow, err := page.GetTextByRow()
		if err != nil {
			return nil, fmt.Errorf("page %d: %w", n, err)
		}
		sort.SliceStable(byRow, func(i, j int) bool { return byRow[i].Position > byRow[j].Position })
		for _, r := range byRow {
			row := Row{Page: n, Y: float64(r.Position)}
			for _, t := range r.Content {
				row.Texts = append(row.Texts, Text{X: t.X, S: t.S})
			}
			sort.SliceStable(row.Texts, func(i, j int) bool { return row.Texts[i].X < row.Texts[j].X })
			rows = append(rows, row)
		}
	}
	return rows, nil
}
