package logger

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// CurriculumAudit keeps every program outcome and validation issue on disk.
// It is deliberately independent of the web server's process-local ring buffer.
type CurriculumAudit struct {
	Dir      string
	file     *os.File
	secrets  []string
	Events   []CurriculumEvent
	Outcomes map[string]CurriculumEvent
	Started  time.Time
}

type CurriculumEvent struct {
	Time      time.Time `json:"time"`
	Level     string    `json:"level"`
	Code      string    `json:"code"`
	ProgramID string    `json:"program_id,omitempty"`
	Program   string    `json:"program,omitempty"`
	Source    string    `json:"source,omitempty"`
	Module    string    `json:"module,omitempty"`
	Message   string    `json:"message"`
	Action    string    `json:"next_action,omitempty"`
	Report    string    `json:"report,omitempty"`
	Status    string    `json:"status,omitempty"`
}

func NewCurriculumAudit(base string, secrets ...string) (*CurriculumAudit, error) {
	dir := filepath.Join(base, "run-"+time.Now().UTC().Format("20060102T150405.000000000Z"))
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, err
	}
	f, err := os.OpenFile(filepath.Join(dir, "events.jsonl"), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return nil, err
	}
	return &CurriculumAudit{Dir: dir, file: f, secrets: secrets, Outcomes: map[string]CurriculumEvent{}, Started: time.Now().UTC()}, nil
}

// OpenCurriculumAudit reconstructs a completed/interrupted journal for offline
// report regeneration or additional checks, without making new model requests.
// Callers must wait until the original scan has stopped writing.
func OpenCurriculumAudit(dir string, secrets ...string) (*CurriculumAudit, error) {
	path := filepath.Join(dir, "events.jsonl")
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	a := &CurriculumAudit{Dir: dir, secrets: secrets, Outcomes: map[string]CurriculumEvent{}}
	decoder := json.NewDecoder(f)
	for {
		var e CurriculumEvent
		err = decoder.Decode(&e)
		if err == io.EOF {
			break
		}
		if err != nil {
			f.Close()
			return nil, fmt.Errorf("invalid audit journal: %w", err)
		}
		if a.Started.IsZero() {
			a.Started = e.Time
		}
		a.Events = append(a.Events, e)
		if e.ProgramID != "" && e.Status != "" {
			a.Outcomes[e.ProgramID] = e
		}
	}
	if err = f.Close(); err != nil {
		return nil, err
	}
	a.file, err = os.OpenFile(path, os.O_APPEND|os.O_WRONLY, 0600)
	if err != nil {
		return nil, err
	}
	return a, nil
}

func (a *CurriculumAudit) Clean(s string) string {
	for _, secret := range a.secrets {
		if secret != "" {
			s = strings.ReplaceAll(s, secret, "[REDACTED]")
		}
	}
	return s
}

func (a *CurriculumAudit) Record(e CurriculumEvent) error {
	e.Time = time.Now().UTC()
	e.Message = a.Clean(e.Message)
	e.Source = a.Clean(e.Source)
	e.Action = a.Clean(e.Action)
	if err := json.NewEncoder(a.file).Encode(e); err != nil {
		return err
	}
	// Flush each event, so an interrupted scan retains its last known progress.
	if err := a.file.Sync(); err != nil {
		return err
	}
	a.Events = append(a.Events, e)
	if e.Status != "" && e.ProgramID != "" {
		a.Outcomes[e.ProgramID] = e
	}
	return nil
}

func (a *CurriculumAudit) Finish(status string) error {
	counts := map[string]int{}
	issues := map[string]int{}
	var outcomes []CurriculumEvent
	for _, e := range a.Outcomes {
		counts[e.Status]++
		outcomes = append(outcomes, e)
	}
	sort.Slice(outcomes, func(i, j int) bool { return outcomes[i].ProgramID < outcomes[j].ProgramID })
	for _, e := range a.Events {
		if e.Level == "warning" || e.Level == "error" {
			issues[e.Code]++
		}
	}
	summary := struct {
		Status      string            `json:"status"`
		Started     time.Time         `json:"started_at"`
		Finished    time.Time         `json:"finished_at"`
		Counts      map[string]int    `json:"counts"`
		IssueCounts map[string]int    `json:"issue_counts"`
		Programs    []CurriculumEvent `json:"programs"`
	}{status, a.Started, time.Now().UTC(), counts, issues, outcomes}
	b, err := json.MarshalIndent(summary, "", "  ")
	if err != nil {
		return err
	}
	if err = os.WriteFile(filepath.Join(a.Dir, "summary.json"), b, 0600); err != nil {
		return err
	}
	var md strings.Builder
	fmt.Fprintf(&md, "# Studienplan-Prüfbericht\n\nLauf: %s · Status: %s\n\n", a.Started.Format(time.RFC3339), status)
	keys := make([]string, 0, len(counts))
	for k := range counts {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		fmt.Fprintf(&md, "- %s: %d\n", k, counts[k])
	}
	md.WriteString("\n## Häufigste Befunde\n\n| Befund | Anzahl |\n| --- | ---: |\n")
	issueKeys := make([]string, 0, len(issues))
	for k := range issues {
		issueKeys = append(issueKeys, k)
	}
	sort.Slice(issueKeys, func(i, j int) bool {
		if issues[issueKeys[i]] == issues[issueKeys[j]] {
			return issueKeys[i] < issueKeys[j]
		}
		return issues[issueKeys[i]] > issues[issueKeys[j]]
	})
	for _, k := range issueKeys {
		fmt.Fprintf(&md, "| %s | %d |\n", k, issues[k])
	}
	md.WriteString("\n## Auffälligkeiten und Nacharbeit\n\nAuch gespeicherte Pläne können Warnungen enthalten. Eine bestandene Plausibilitätsprüfung ersetzt keine vollständige fachliche Prüfung.\n\n")
	for _, e := range a.Events {
		if e.Level != "warning" && e.Level != "error" {
			continue
		}
		fmt.Fprintf(&md, "### %s — %s\n\n- Studiengang-ID: `%s`\n- Schweregrad: %s\n- Quelle: %s\n- Modul: %s\n- Befund: %s\n- Nächster Schritt: %s\n", e.Program, e.Code, e.ProgramID, e.Level, e.Source, e.Module, strings.ReplaceAll(e.Message, "\n", " "), e.Action)
		if e.Report != "" {
			fmt.Fprintf(&md, "- Detailbericht: %s\n", e.Report)
		}
		md.WriteString("\n")
	}
	return os.WriteFile(filepath.Join(a.Dir, "review.md"), []byte(md.String()), 0600)
}

func (a *CurriculumAudit) Close() error { return a.file.Close() }

func CurriculumIssueAction(code string) string {
	switch code {
	case "unknown_intake":
		return "Studienbeginn in der Ordnung feststellen; nur bei gesicherter Angabe mit --start-term winter oder summer erneut prüfen."
	case "semester_bounds":
		return "Regelstudienzeit mit dem PDF abgleichen; falsch erkannte Studienform oder vermischte Planvarianten prüfen."
	case "source_total_conflict", "missing_source_totals", "missing_semester_total", "incomplete_table":
		return "Summenzeilen, Alternativen, zusammengefasste Zellen und Fortsetzungsseiten im PDF prüfen."
	case "catalog_identity_conflict":
		return "Modulnummer und Titel im Original vergleichen; historische Versionen und fehlerhafte Katalogverknüpfungen prüfen."
	case "catalog_credit_conflict", "catalog_total_conflict":
		return "ECTS der damaligen Ordnung mit dem aktuellen Katalog vergleichen; historische Werte nicht automatisch überschreiben."
	case "catalog_suggests_layout_error":
		return "Der vollständige Katalogvergleich passt zur gedruckten Summe, die Zellwerte nicht: Layout, Fußnoten und Teilmodule prüfen."
	case "catalog_title_match":
		return "Eindeutigen Texttreffer mit Originaltitel und historischem Modulstand vergleichen."
	case "unmatched_title":
		return "Titel, Übersetzung und historische Module prüfen; bei ähnlichen Kandidaten die Modulidentität klären."
	case "amendment_requires_patch":
		return "Genannte Änderungsfundstelle auf den Basisplan anwenden und die dokumentierte Prüfung mit dem Datei-Hash aktualisieren."
	case "unmatched_code":
		return "Modulnummer im QIS-Katalog prüfen und fehlendes oder historisches Modul nachladen."
	case "catalog_coverage":
		return "Fehlende Katalogverknüpfungen prüfen: historische Module nachladen und Titelunterschiede klären; keine unsicheren Treffer automatisch verknüpfen."
	case "season_conflict":
		return "Studienbeginn, Lehrturnus und mehrsemestrige Lehre mit der Originalzelle abgleichen."
	case "biennial_offering":
		return "Zweijährigen Lehrturnus mit dem tatsächlichen Jahr des Studienbeginns abgleichen."
	case "semester_load":
		return "Semesterumfang anhand der Originalsumme prüfen; Arbeitsaufwand, Anrechnung und gemeinsame Wahlpflichtbudgets beachten."
	default:
		return "Quellzelle im PDF mit Semester, ECTS und Katalogangebot vergleichen."
	}
}

// ClassifyCurriculumFailure provides stable categories and actionable next steps.
func ClassifyCurriculumFailure(message string) (string, string) {
	s := strings.ToLower(message)
	switch {
	case strings.Contains(s, "429"), strings.Contains(s, "quota"), strings.Contains(s, "rate limit"):
		return "api_quota", "API-Kontingent prüfen und betroffenen Studiengang später erneut ausführen."
	case strings.Contains(s, "deadline"), strings.Contains(s, "timeout"), strings.Contains(s, "connection"):
		return "api_transport", "Netzwerk/API-Erreichbarkeit prüfen und erneut versuchen."
	case strings.Contains(s, "incomplete table"), strings.Contains(s, "whole-plan total"):
		return "incomplete_table", "Fortsetzungsseiten und Summenzeilen im PDF prüfen; Tabellenparser ergänzen."
	case strings.Contains(s, "unsupported"), strings.Contains(s, "no supported"), strings.Contains(s, "ambiguous pdf"), strings.Contains(s, "orientation"), strings.Contains(s, "horizontal"):
		return "unsupported_layout", "PDF-Seiten und Zellgeometrie prüfen; Unterstützung für dieses Layout ergänzen."
	case strings.Contains(s, "model"), strings.Contains(s, "gemini"), strings.Contains(s, "json"):
		return "model_response", "Modellantwort und Quellbelege auf Vollständigkeit und Zuordnung prüfen."
	default:
		return "extraction_failed", "Fehlermeldung und Original-PDF prüfen; gezielt mit --program-id erneut ausführen."
	}
}
