package model

import "time"

// FUESModule represents an approved Fachübergreifendes Studium module parsed from QIS.
type FUESModule struct {
	ID         string    `json:"id"`
	Title      string    `json:"title"`
	Language   string    `json:"language,omitempty"`
	Credits    float64   `json:"credits,omitempty"`
	CreditsRaw string    `json:"credits_raw,omitempty"`
	IsFUES     bool      `json:"is_fues"`
	Limitation string    `json:"limitation,omitempty"`
	QISURL     string    `json:"qis_url,omitempty"`
	ScrapedAt  time.Time `json:"scraped_at"`
}
