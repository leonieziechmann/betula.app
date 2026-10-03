package qistree

import (
	"net/url"
	"strings"
)

// NodeID is the position a QIS tree URL encodes in its nodeID parameter:
// auswahlBaum|studiengang:stg=079|abschluss:abschl=82|stgSpecials:vert=,schwp=,kzfa=H,pversion=2008|…
type NodeID struct {
	Stg      string // program code
	Abschl   string // degree code; often not numeric („D8", „F2")
	PVersion string // PO code, the base year
	Vert     string // Vertiefung, empty in all programs seen so far
	Schwp    string // Schwerpunkt, empty in all programs seen so far
	Kzfa     string // „H" in all programs seen so far
	Depth    int    // segments below the PO; 0 for the PO page itself, -1 above a PO
}

// ParseNodeID reads the nodeID of a tree page URL. Unknown segments are ignored.
func ParseNodeID(pageURL string) NodeID {
	id := NodeID{Depth: -1}
	u, err := url.Parse(pageURL)
	if err != nil {
		return id
	}
	segments := strings.Split(u.Query().Get("nodeID"), "|")
	for i, seg := range segments {
		kind, params, _ := strings.Cut(seg, ":")
		switch kind {
		case "studiengang":
			id.Stg = param(params, "stg")
		case "abschluss":
			id.Abschl = param(params, "abschl")
		case "stgSpecials":
			id.Vert = param(params, "vert")
			id.Schwp = param(params, "schwp")
			id.Kzfa = param(params, "kzfa")
			id.PVersion = param(params, "pversion")
			id.Depth = len(segments) - 1 - i
		}
	}
	return id
}

// IsPO reports whether the URL is the page of a PO version itself.
func (id NodeID) IsPO() bool {
	return id.PVersion != "" && id.Depth == 0
}

// ProgramID is the stable identifier of a PO version: program, degree and PO code.
// The specials only become part of it if QIS ever uses them, so that IDs stay unique.
func (id NodeID) ProgramID() string {
	parts := []string{id.Stg, id.Abschl, id.PVersion}
	if id.Vert != "" {
		parts = append(parts, "v"+id.Vert)
	}
	if id.Schwp != "" {
		parts = append(parts, "s"+id.Schwp)
	}
	if id.Kzfa != "" && id.Kzfa != "H" {
		parts = append(parts, "k"+id.Kzfa)
	}
	return strings.Join(parts, "-")
}

func param(params, name string) string {
	for _, kv := range strings.Split(params, ",") {
		k, v, _ := strings.Cut(kv, "=")
		if k == name {
			return v
		}
	}
	return ""
}
