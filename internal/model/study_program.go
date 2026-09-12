package model

import "time"

// ProgramRegulationDocument represents a statute or amendment document (e.g. from OPUS 4).
type ProgramRegulationDocument struct {
	Title          string `json:"title"`           // e.g. "Prüfungsordnung ABl. 12/2024" or "Satzungsänderung ABl. 13/2021 (1. SÄ)"
	DocType        string `json:"doc_type"`        // "statute" (Prüfungsordnung / Studienordnung), "amendment" (Satzungsänderung), or "other"
	URL            string `json:"url"`             // e.g. "https://opus4.kobv.de/opus4-btu/files/6707/12_Informatik_B.Sc.pdf"
	Icon           string `json:"icon,omitempty"`  // e.g. "pruefungsordnung.gif", "satzungsaenderung1.gif"
	LocalPath      string `json:"local_path,omitempty"`
	DownloadStatus string `json:"download_status"` // "downloaded", "blocked_bot_checker", "not_attempted", "error"
}

// OfficialStudyProgram represents an official study program branch down to its PO-version in QIS.
type OfficialStudyProgram struct {
	ID          string                      `json:"id"`           // e.g. "stg=749_abschl=88_pversion=2019"
	ProgramName string                      `json:"program_name"` // e.g. "Informatik"
	ProgramCode string                      `json:"program_code"` // e.g. "749"
	Degree      string                      `json:"degree"`       // e.g. "Bachelor (universitär)"
	DegreeCode  string                      `json:"degree_code"`  // e.g. "88"
	POVersion   string                      `json:"po_version"`   // e.g. "2008 - 2. SÄ 2024"
	QISNodeID   string                      `json:"qis_node_id"`  // full QIS nodeID
	QISURL      string                      `json:"qis_url"`      // URL to the PO view in QIS
	Documents   []ProgramRegulationDocument `json:"documents"`
	ScrapedAt   time.Time                   `json:"scraped_at"`
}
