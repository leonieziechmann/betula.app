package model

// GeneralDocument is a document the BTU's page of Grundsatzordnungen lists for every program
// (b-tu.de/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen,
// and its English page): a regulation, an amendment of one, a law. The page puts them in sections,
// each an <h2> heading followed by its links; a section's amendments sit in a fold
// („Änderungssatzungen") under it.
type GeneralDocument struct {
	Section string // the heading of its section, as the page writes it
	Fold    string // the title of the fold it sits in („Änderungssatzungen"), "" in the section itself
	Title   string // what the page writes for it, without „(PDF)"; its dates are the BTU's and can be wrong
	URL     string // its address, absolute
}
