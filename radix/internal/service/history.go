package service

import (
	"context"
	"path/filepath"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/catalogdb"
	"github.com/leonieziechmann/betula/radix/internal/history"
	"github.com/leonieziechmann/betula/radix/internal/oplog"
	"github.com/leonieziechmann/betula/radix/internal/privacy"
)

// historyStage records what is published in the history (Config.History, docs/radix/history.md):
// first the privacy register, when it changed since the history last applied it (every version),
// then the published snapshot, when the history's newest state is not it. Every cycle that did not
// fail, after the export or in its place: a snapshot an earlier cycle published but the history
// did not record (it was enabled later, or its stage failed) is recorded by the next. A failure
// degrades the cycle; what is published is intact.
//
// Log events: history.redacted, history.recorded, stage.failed (ERROR).
func (s *Service) historyStage(ctx context.Context, result *CycleResult) {
	if s.cfg.History == "" {
		result.Stages = append(result.Stages, StageResult{Name: "history", Skipped: "no history"})
		return
	}
	log := oplog.For("history")
	start := s.now()
	err := s.recordHistory(ctx)
	stage := StageResult{Name: "history", DurationMS: s.now().Sub(start).Milliseconds()}
	switch {
	case ctx.Err() != nil:
		stage.Interrupted = true // a Record is one transaction: the next cycle records it
	case err != nil:
		stage.Error = err.Error()
		if result.Result == "ok" {
			result.Result = "degraded"
		}
		log.Error("history stage failed", "event", "stage.failed", "stage", "history", oplog.Err(err))
	}
	result.Stages = append(result.Stages, stage)
}

func (s *Service) recordHistory(ctx context.Context) error {
	log := oplog.For("history")
	people, err := privacy.Load(ctx, s.db.SQL())
	if err != nil {
		return err
	}
	h, err := history.Open(s.cfg.History)
	if err != nil {
		return err
	}
	defer h.Close()
	if done, res, err := h.Sync(ctx, people); err != nil {
		return err
	} else if done {
		log.Info("privacy register applied to every version of the history", "event", "history.redacted",
			"people", res.People, "rows", res.Rows, "values", res.Values)
	}
	pointer, err := catalogdb.ReadSnapshotPointer(s.cfg.SnapshotDir)
	if err != nil {
		return nil // nothing published yet
	}
	snap, err := h.Record(ctx, filepath.Join(s.cfg.SnapshotDir, pointer.File), history.RecordOptions{People: people, Now: s.now})
	if err != nil || snap == nil {
		return err
	}
	log.Info("published snapshot recorded in the history", "event", "history.recorded", "state", snap.Seq,
		"valid_from", snap.ValidFrom.Format(time.RFC3339), "rows_opened", snap.RowsOpened, "rows_closed", snap.RowsClosed,
		"schema", snap.SchemaVersion, "file", pointer.File)
	return nil
}
