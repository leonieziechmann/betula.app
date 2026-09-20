package main

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/leonieziechmann/betula/internal/catalogdb"
	"github.com/leonieziechmann/betula/internal/curriculumscan"
	"github.com/leonieziechmann/betula/internal/gemini"
	"github.com/leonieziechmann/betula/internal/model"
	"github.com/leonieziechmann/betula/internal/oplog"
	"github.com/leonieziechmann/betula/internal/planaudit"
	"github.com/leonieziechmann/betula/internal/secrets"
	"github.com/leonieziechmann/betula/internal/statutes"
)

// runDownloadStatutes fetches the regulation PDFs the programs link (OPUS).
func runDownloadStatutes(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("download-statutes", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	dir := fs.String("statutes-dir", envOr("RADIX_STATUTES_DIR", "statutes"), "Directory for the regulation PDFs (env RADIX_STATUTES_DIR)")
	nameFilter := fs.String("name", "", "Only programs whose name contains this")
	degreeFilter := fs.String("degree", "", "Only programs whose degree contains this")
	programID := fs.String("program-id", "", "Only this program")
	delayMs := fs.Int("delay", 1000, "Pause after each download in milliseconds")
	force := fs.Bool("force", false, "Download again even if a local copy exists")
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()
	log := oplog.For("statutes")

	db := openDB(*dbPath)
	defer db.Close()
	programs, err := db.ScanPrograms(*nameFilter, *degreeFilter, *programID)
	if err != nil {
		log.Error("cannot read the programs", "event", "statutes.failed", oplog.Err(err))
		os.Exit(1)
	}

	client := &http.Client{Timeout: 60 * time.Second}
	seen := make(map[string]bool)
	var downloaded, present, blocked, failed int
	for _, p := range programs {
		for _, d := range p.Documents {
			if ctx.Err() != nil {
				os.Exit(130)
			}
			if seen[d.URL] {
				continue
			}
			seen[d.URL] = true
			path, cached, err := statutes.Download(ctx, client, *dir, p.ProgramName, d.URL, *force)
			switch {
			case errors.Is(err, statutes.ErrBotProtection):
				blocked++
				log.Warn("document is behind bot protection; download it by hand", "event", "statutes.blocked", "program", p.ProgramName, "url", d.URL,
					"target", statutes.LocalPath(*dir, p.ProgramName, d.URL))
			case err != nil:
				failed++
				log.Error("download failed", "event", "statutes.download_failed", "program", p.ProgramName, "url", d.URL, oplog.Err(err))
			case cached:
				present++
			default:
				downloaded++
				log.Info("document downloaded", "event", "statutes.downloaded", "program", p.ProgramName, "path", path)
				time.Sleep(time.Duration(*delayMs) * time.Millisecond)
			}
		}
	}
	log.Info("statutes up to date", "event", "statutes.finished", "programs", len(programs), "documents", len(seen),
		"downloaded", downloaded, "already_present", present, "blocked", blocked, "failed", failed)
	if failed > 0 {
		os.Exit(1)
	}
}

// runScanCurriculum extracts the study plan of each program from its regulation PDF,
// validates it against the PDF geometry and the module catalog, and stores it as the
// program's validated plan. A plan that fails validation never replaces a stored one.
func runScanCurriculum(ctx context.Context, args []string) {
	fs := flag.NewFlagSet("scan-curriculum", flag.ExitOnError)
	dbPath := fs.String("db", defaultDBPath, "Database path")
	dir := fs.String("statutes-dir", envOr("RADIX_STATUTES_DIR", "statutes"), "Directory with the regulation PDFs (env RADIX_STATUTES_DIR)")
	nameFilter := fs.String("name", "", "Only programs whose name contains this")
	degreeFilter := fs.String("degree", "", "Only programs whose degree contains this")
	programID := fs.String("program-id", "", "Only this program (e.g. 079-82-2008)")
	offline := fs.Bool("offline", false, "Do not call Gemini even if a key is available (deterministic PDF reader only)")
	modelName := fs.String("model", envOr("GEMINI_MODEL", gemini.DefaultModel), "Gemini model (env GEMINI_MODEL)")
	delayMs := fs.Int("delay", 500, "Pause between programs in milliseconds")
	force := fs.Bool("force", false, "Scan again even if a validated plan is stored")
	asJSON := fs.Bool("json", false, "Write extraction and validation reports as JSON Lines to stdout")
	dryRun := fs.Bool("dry-run", false, "Validate and write reports without changing the database")
	startTerm := fs.String("start-term", "auto", "Intake: auto, winter, summer, unknown")
	tolerance := fs.Float64("credit-tolerance", 6, "Allowed deviation from the expected credits per semester (warning only)")
	reportDir := fs.String("report-dir", filepath.Join("logs", "curriculum"), "Directory for extraction and validation evidence")
	pdfPath := fs.String("pdf", "", "Explicit regulation PDF (requires --program-id)")
	planPages := fs.String("plan-pages", "", "Physical PDF pages with complete plan variants, e.g. 7,9 (requires --pdf)")
	logs := addLogFlags(fs)
	_ = fs.Parse(args)
	_, closeLog := logs.setup()
	defer closeLog()
	log := oplog.For("scan")

	fail := func(msg string, args ...any) {
		log.Error(msg, append([]any{"event", "scan.failed"}, args...)...)
		os.Exit(1)
	}
	if *pdfPath != "" && *programID == "" {
		fail("--pdf requires --program-id, so that one document is not assigned to unrelated programs")
	}
	if *planPages != "" && *pdfPath == "" {
		fail("--plan-pages requires --pdf")
	}
	if (*startTerm != "auto" && *startTerm != "winter" && *startTerm != "summer" && *startTerm != "unknown") || *tolerance < 0 {
		fail("invalid --start-term or negative --credit-tolerance")
	}
	var pages []int
	for _, part := range strings.Split(*planPages, ",") {
		if part = strings.TrimSpace(part); part == "" {
			continue
		}
		n, err := strconv.Atoi(part)
		if err != nil || n < 1 {
			fail("invalid --plan-pages: use comma-separated positive page numbers")
		}
		pages = append(pages, n)
	}

	// The key comes from the secret sources only, never from a flag or a config file.
	apiKey := ""
	if !*offline {
		key, source, err := secrets.Resolve(secrets.GeminiAPIKey)
		switch {
		case err == nil:
			apiKey = key
			log.Info("Gemini enrichment enabled", "event", "scan.gemini_enabled", "model", *modelName, "key_source", string(source))
		case errors.Is(err, secrets.ErrNotFound):
			// Semester columns, credits, names and requirement kinds come from the PDF cells
			// either way; without a key only the model's extra fields are missing.
			log.Warn("no Gemini API key; continuing with the deterministic PDF reader", "event", "scan.gemini_disabled", "how_to", secrets.HowTo(secrets.GeminiAPIKey))
		default:
			fail("cannot read the Gemini API key", oplog.Err(err))
		}
	}
	client := gemini.NewClient(apiKey, *modelName)
	client.PlanPages = pages

	db := openDB(*dbPath)
	defer db.Close()
	catalog, err := db.ScanCatalog()
	if err != nil {
		fail("cannot read the module catalog", oplog.Err(err))
	}
	if len(catalog) == 0 {
		fail("the module catalog is empty; run `radix build` first")
	}
	programs, err := db.ScanPrograms(*nameFilter, *degreeFilter, *programID)
	if err != nil {
		fail("cannot read the programs", oplog.Err(err))
	}
	if len(programs) == 0 {
		fail("no program matches the filter")
	}
	for i := range programs {
		for j := range programs[i].Documents {
			d := &programs[i].Documents[j]
			if path, ok := statutes.Locate(*dir, programs[i].ProgramName, d.URL); ok {
				d.LocalPath, d.DownloadStatus = path, "downloaded"
			}
		}
	}

	audit, err := planaudit.NewCurriculumAudit(*reportDir, apiKey)
	if err != nil {
		fail("cannot create the audit directory", oplog.Err(err))
	}
	defer audit.Close()
	record := func(e planaudit.CurriculumEvent) {
		if err := audit.Record(e); err != nil {
			fail("cannot write the audit log", oplog.Err(err))
		}
	}
	log.Info("scan started", "event", "scan.started", "programs", len(programs), "gemini", apiKey != "", "dry_run", *dryRun, "audit_dir", audit.Dir)

	var saved, skipped, withoutPlan, rejected int
	for _, prog := range programs {
		if ctx.Err() != nil {
			_ = audit.Finish("cancelled")
			os.Exit(130)
		}
		plog := log.With("program_id", prog.ID, "program", prog.ProgramName, "degree", prog.Degree, "po", prog.POVersion)

		if !*force {
			has, err := db.HasPlan(prog.ID)
			if err != nil {
				fail("cannot read the stored plans", oplog.Err(err))
			}
			if has {
				skipped++
				record(planaudit.CurriculumEvent{Level: "info", Code: "already_validated", ProgramID: prog.ID, Program: prog.ProgramName, Message: "Existing validated plan preserved", Status: "skipped_validated"})
				continue
			}
		}

		outcome := curriculumscan.Scan(ctx, prog, catalog, curriculumscan.Options{
			StartTerm: *startTerm, Tolerance: *tolerance, PDFPath: *pdfPath, PlanPages: pages, Client: client,
		})
		message := audit.Clean(outcome.Message)
		setStatus := func() {
			if *dryRun {
				return
			}
			if err := db.SetPlanScanStatus(prog.ID, outcome.Status, message, outcome.Source); err != nil {
				plog.Error("cannot store the scan status", "event", "scan.status_failed", oplog.Err(err))
			}
		}
		for _, review := range outcome.Reviews {
			record(planaudit.CurriculumEvent{Level: "info", Code: "amendment_review", ProgramID: prog.ID, Program: prog.ProgramName, Source: review.Source, Message: review.Decision + ": " + review.Evidence + " SHA256=" + review.SHA256})
		}

		if outcome.Status == curriculumscan.StatusMissingSource || outcome.Status == curriculumscan.StatusNoPlan {
			// Not a failure: discontinued programs and amendment-only entries have no plan table.
			withoutPlan++
			plog.Info("no study plan", "event", "scan.no_plan", "status", outcome.Status, "reason", message)
			record(planaudit.CurriculumEvent{Level: "info", Code: outcome.Status, ProgramID: prog.ID, Program: prog.ProgramName, Source: outcome.Source, Message: message, Status: outcome.Status})
			setStatus()
			continue
		}
		if outcome.Extraction == nil {
			rejected++
			code, action := planaudit.ClassifyCurriculumFailure(message)
			plog.Error("the document could not be read", "event", "scan.extraction_failed", "source", outcome.Source, "code", code, "reason", message)
			record(planaudit.CurriculumEvent{Level: "error", Code: code, ProgramID: prog.ID, Program: prog.ProgramName, Source: outcome.Source, Message: message, Action: action, Status: "needs_review"})
			setStatus()
			continue
		}

		report := struct {
			ProgramID  string                             `json:"program_id"`
			Source     string                             `json:"source"`
			Extraction *gemini.CurriculumExtractionResult `json:"extraction"`
			Validation gemini.ValidationReport            `json:"validation"`
			Model      string                             `json:"model"`
			CheckedAt  time.Time                          `json:"checked_at"`
		}{prog.ID, outcome.Source, outcome.Extraction, outcome.Validation, *modelName, time.Now().UTC()}
		reportPath := filepath.Join(audit.Dir, fmt.Sprintf("%x.json", sha256.Sum256([]byte(prog.ID+"|"+outcome.Source))))
		if data, err := json.MarshalIndent(report, "", "  "); err != nil || os.WriteFile(reportPath, data, 0644) != nil {
			fail("cannot write the validation evidence", "path", reportPath)
		}
		if *asJSON {
			_ = json.NewEncoder(os.Stdout).Encode(report)
		}

		warnings, errorsFound := 0, 0
		for _, issue := range outcome.Validation.Issues {
			switch issue.Severity {
			case "warning":
				warnings++
			case "error":
				errorsFound++
			}
			record(planaudit.CurriculumEvent{Level: issue.Severity, Code: issue.Code, ProgramID: prog.ID, Program: prog.ProgramName, Source: outcome.Source, Module: issue.Module, Message: issue.Message, Report: reportPath, Action: planaudit.CurriculumIssueAction(issue.Code)})
		}

		if !outcome.Saveable() {
			rejected++
			plog.Warn("plan rejected by validation; the stored plan is unchanged", "event", "scan.rejected", "errors", errorsFound, "warnings", warnings, "reason", message, "evidence", reportPath)
			record(planaudit.CurriculumEvent{Level: "info", Code: "validation_rejected", ProgramID: prog.ID, Program: prog.ProgramName, Message: "Validation errors; existing records preserved", Status: "needs_review", Report: reportPath})
			setStatus()
			continue
		}

		linked := linkedModules(outcome.Modules)
		if *dryRun {
			saved++
			plog.Info("plan is valid (dry run, not stored)", "event", "scan.valid", "entries", len(outcome.Modules), "linked", linked, "warnings", warnings, "evidence", reportPath)
			record(planaudit.CurriculumEvent{Level: "info", Code: "dry_run_valid", ProgramID: prog.ID, Program: prog.ProgramName, Message: "Valid extraction, not saved (dry run)", Status: "dry_run_valid"})
			continue
		}
		if err := db.SavePlan(catalogdb.PlanFromModules(prog.ID, outcome.Source, outcome.LayoutJSON, outcome.Modules)); err != nil {
			rejected++
			plog.Error("cannot store the plan; the previous plan is unchanged", "event", "scan.save_failed", oplog.Err(err))
			record(planaudit.CurriculumEvent{Level: "error", Code: "database_write", ProgramID: prog.ID, Program: prog.ProgramName, Message: err.Error(), Status: "failed", Action: "Datenbanktransaktion und Schreibrechte prüfen."})
			continue
		}
		saved++
		setStatus()
		plog.Info("plan stored", "event", "scan.saved", "status", outcome.Status, "entries", len(outcome.Modules), "linked", linked, "warnings", warnings, "evidence", reportPath)
		record(planaudit.CurriculumEvent{Level: "info", Code: "saved", ProgramID: prog.ID, Program: prog.ProgramName, Source: outcome.Source, Message: fmt.Sprintf("%d requirements, %d catalog links", len(outcome.Modules), linked), Status: outcome.Status, Report: reportPath})

		time.Sleep(time.Duration(*delayMs) * time.Millisecond)
	}

	status := "completed"
	if rejected > 0 {
		status = "completed_with_issues"
	}
	if err := audit.Finish(status); err != nil {
		fail("cannot write the audit summary", oplog.Err(err))
	}
	level := slog.LevelInfo
	if rejected > 0 {
		level = slog.LevelWarn
	}
	log.Log(ctx, level, "scan finished", "event", "scan.finished", "valid", saved, "already_validated", skipped,
		"without_plan", withoutPlan, "needs_review", rejected, "review_report", filepath.Join(audit.Dir, "review.md"))
	if !*dryRun && saved > 0 {
		log.Info("run `radix build` (or wait for the next service cycle) to publish the new plans", "event", "scan.hint")
	}
	if rejected > 0 {
		os.Exit(1)
	}
}

func linkedModules(modules []model.CurriculumModule) int {
	n := 0
	for _, m := range modules {
		if m.ModuleID != "" {
			n++
		}
	}
	return n
}
