package parser

import (
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/model"
	"golang.org/x/net/html"
	"golang.org/x/net/html/atom"
)

// Cell classes of the QIS module description: every fact sits in a row of a label
// cell and a value cell, and nothing else on the page uses these two classes.
const (
	qisLabelClass = "tabelle1_alignleft"
	qisValueClass = "tabelle2inhalt"
)

// QISModuleParser reads the module description of QIS
// (state=modulBeschrDetailInfo), the page b-tu.de/modul copies. It carries the
// same labels and list shapes as the copy, so the rows go through applyRow, and
// both sources end up in the same model.ModuleDetail.
//
// The difference that matters: the copy keeps the events of the semester it was
// generated in, while this page names the events of the current one.
type QISModuleParser struct{}

// NewQISModuleParser creates a QISModuleParser.
func NewQISModuleParser() *QISModuleParser {
	return &QISModuleParser{}
}

// Parse reads one archived QIS module description. fallbackID is the module number
// the page was fetched for; the page states its own, which wins.
func (p *QISModuleParser) Parse(r io.Reader, fallbackID, pageURL string) (*model.ModuleDetail, error) {
	doc, err := html.Parse(r)
	if err != nil {
		return nil, fmt.Errorf("failed to parse HTML: %w", err)
	}

	detail := &model.ModuleDetail{
		ID:                       fallbackID,
		Code:                     fallbackID,
		RawURL:                   pageURL,
		LastScrapedAt:            time.Now().UTC(),
		PrerequisitesRecommended: "-",
		PrerequisitesMandatory:   "-",
	}

	var prevKey string
	rows := 0
	for _, tr := range FindAllByTag(doc, atom.Tr) {
		label, value := qisRowCells(tr)
		if label == nil {
			continue
		}
		rows++
		applyRow(detail, CleanSingleLine(NodeText(label)), value, &prevKey)
	}
	if rows == 0 {
		return nil, fmt.Errorf("no module description on the page; the QIS layout may have changed")
	}

	// The German view states the English title in the row below the German one,
	// without a label of its own; applyRow files it as the secondary title.
	if detail.TitleDE == "" && detail.TitleEN != "" {
		detail.TitleDE, detail.TitleEN = detail.TitleEN, ""
	}
	if detail.PrerequisitesRecommended == "" {
		detail.PrerequisitesRecommended = "-"
	}
	if detail.PrerequisitesMandatory == "" {
		detail.PrerequisitesMandatory = "-"
	}
	return detail, nil
}

// qisRowCells returns the label and value cell of a description row, or nil for a
// row that is not one (the page nests tables for its navigation).
func qisRowCells(tr *html.Node) (label, value *html.Node) {
	for _, td := range FindAllByTag(tr, atom.Td) {
		classes := strings.Fields(GetAttr(td, "class"))
		for _, c := range classes {
			switch c {
			case qisLabelClass:
				if label == nil && value == nil {
					label = td
				}
			case qisValueClass:
				if label != nil && value == nil {
					value = td
				}
			}
		}
	}
	if label == nil || value == nil {
		return nil, nil
	}
	return label, value
}
