package store

import (
	"bytes"
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"reflect"
	"slices"
	"strconv"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// The operations of the journal.
const (
	OpEpoch       = "epoch"        // a leader's term began
	OpVersion     = "version"      // a fetch returned new content or a new status
	OpCheck       = "check"        // a fetch returned the current content again
	OpEntryDelete = "entry_delete" // an entry was forgotten
	OpFileVersion = "file_version" // a file got a new version or a tombstone
	OpPrune       = "prune"        // retention ran
)

// JournalEntry is one change of the index. Payload holds the complete rows the change wrote,
// ids included, so that Apply is a pure function of the entry.
type JournalEntry struct {
	Seq     int64           `json:"seq"`
	Epoch   int64           `json:"epoch"`
	At      time.Time       `json:"at"`
	Op      string          `json:"op"`
	Payload json.RawMessage `json:"payload"`
	Blob    string          `json:"blob,omitempty"` // sha256 hex the entry needs; the follower fetches it first
}

type versionPayload struct {
	Entry      entryRow   `json:"entry"`
	Version    versionRow `json:"version"`
	Superseded *supersede `json:"superseded,omitempty"`
}

type entryDeletePayload struct {
	ID  int64  `json:"id"`
	Key string `json:"key"`
}

type fileVersionPayload struct {
	File       fileRow        `json:"file"`
	Version    fileVersionRow `json:"version"`
	Superseded *supersede     `json:"superseded,omitempty"`
}

type prunePayload struct {
	Cutoff string `json:"cutoff"`
}

type epochPayload struct {
	Epoch    int64  `json:"epoch"`
	Instance string `json:"instance"`
	URL      string `json:"url"`
	Since    string `json:"since"`
}

// commit appends the journal entry of a write to the transaction and commits it. The entry
// gets the next sequence number and the current epoch.
func (s *Store) commit(tx *sql.Tx, op string, at time.Time, payload any, blob string) (JournalEntry, error) {
	data, err := json.Marshal(payload)
	if err != nil {
		return JournalEntry{}, fmt.Errorf("failed to encode a journal entry: %w", err)
	}
	// The writes check their input, but a payload JSON does not carry as it is (a string that
	// is not UTF-8 becomes U+FFFD) would give the follower other rows than the ones this
	// transaction wrote, so it is refused here too: nothing is written.
	if !survivesJSON(payload, data) {
		return JournalEntry{}, fmt.Errorf("journal entry %s: %w: the rows do not survive JSON (text that is not UTF-8?)", op, ErrInvalidInput)
	}
	seq, err := lastSeq(tx)
	if err != nil {
		return JournalEntry{}, err
	}
	epoch, err := metaEpoch(tx)
	if err != nil {
		return JournalEntry{}, err
	}
	e := JournalEntry{Seq: seq + 1, Epoch: epoch, At: stamp(at), Op: op, Payload: data, Blob: blob}
	if err := insertJournal(tx, e); err != nil {
		return JournalEntry{}, err
	}
	if err := s.commitAndAdvance(tx, e); err != nil {
		return JournalEntry{}, fmt.Errorf("failed to commit %s: %w", op, err)
	}
	return e, nil
}

// commitAndAdvance commits tx, whose newest journal entry is e, and moves the cached position
// to e before any other writer can commit (commitMu). The directories of imported blobs are
// synced first: a row that names a blob is never durable before the blob's name is.
func (s *Store) commitAndAdvance(tx *sql.Tx, e JournalEntry) error {
	if err := s.syncPending(); err != nil {
		return err
	}
	s.commitMu.Lock()
	defer s.commitMu.Unlock()
	if err := tx.Commit(); err != nil {
		return err
	}
	s.advance(e)
	return nil
}

// Sum is the checksum of a journal entry: sha256 hex of its seq, epoch, op, payload (compacted,
// as Apply stores it) and blob. Two instances with the same entry at a seq have the same sum
// whatever the epoch numbers say: a leader that started an epoch number another one had used
// (leader.json lost) still writes other payloads (review 2: epoch-reuse-hides-divergence). The
// time is left out: the payload holds the times of the rows already.
func (e JournalEntry) Sum() string {
	var payload bytes.Buffer
	if err := json.Compact(&payload, e.Payload); err != nil {
		payload.Reset()
		payload.Write(e.Payload) // not JSON: the bytes as they are, which no other entry has either
	}
	h := sha256.New()
	fmt.Fprintf(h, "cortex-journal-sum-1\n%d\n%d\n%s\n%d\n", e.Seq, e.Epoch, e.Op, payload.Len())
	h.Write(payload.Bytes())
	fmt.Fprintf(h, "\n%s", e.Blob)
	return hex.EncodeToString(h.Sum(nil))
}

// survivesJSON says whether data, the JSON of payload (a struct), decodes to payload exactly.
func survivesJSON(payload any, data []byte) bool {
	back := reflect.New(reflect.TypeOf(payload))
	if err := json.Unmarshal(data, back.Interface()); err != nil {
		return false
	}
	return reflect.DeepEqual(back.Elem().Interface(), payload)
}

func insertJournal(tx *sql.Tx, e JournalEntry) error {
	_, err := tx.Exec(`INSERT INTO journal (seq, epoch, at, op, payload, blob) VALUES (?, ?, ?, ?, ?, ?)`,
		e.Seq, e.Epoch, FormatTime(e.At), e.Op, string(e.Payload), e.Blob)
	if err != nil {
		return fmt.Errorf("failed to append journal entry %d: %w", e.Seq, err)
	}
	return nil
}

func lastSeq(tx *sql.Tx) (int64, error) {
	var seq int64
	if err := tx.QueryRow(`SELECT COALESCE(MAX(seq), 0) FROM journal`).Scan(&seq); err != nil {
		return 0, fmt.Errorf("failed to read the journal position: %w", err)
	}
	return seq, nil
}

func metaEpoch(q querier) (int64, error) {
	var v string
	err := q.QueryRow(`SELECT value FROM meta WHERE key = 'epoch'`).Scan(&v)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, nil
	}
	if err != nil {
		return 0, fmt.Errorf("failed to read the epoch: %w", err)
	}
	n, err := strconv.ParseInt(v, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("invalid epoch %q in the index", v)
	}
	return n, nil
}

// advance moves the cached position to a committed entry and wakes WaitAfter. Writes commit
// one after the other, but may get here in another order, so the position only moves forward.
func (s *Store) advance(e JournalEntry) {
	s.posMu.Lock()
	defer s.posMu.Unlock()
	if e.Seq > s.pos.seq {
		s.pos.seq, s.pos.epoch, s.pos.at = e.Seq, e.Epoch, e.At
	}
	if s.pos.oldest == 0 {
		s.pos.oldest = e.Seq
	}
	close(s.changed)
	s.changed = make(chan struct{})
}

func loadPosition(db *sql.DB) (position, error) {
	var p position
	var at string
	err := db.QueryRow(`SELECT seq, epoch, at FROM journal ORDER BY seq DESC LIMIT 1`).Scan(&p.seq, &p.epoch, &at)
	if errors.Is(err, sql.ErrNoRows) {
		return position{}, nil
	}
	if err != nil {
		return position{}, fmt.Errorf("failed to read the journal position: %w", err)
	}
	if p.at, err = parseTime(at); err != nil {
		return position{}, err
	}
	if err := db.QueryRow(`SELECT MIN(seq) FROM journal`).Scan(&p.oldest); err != nil {
		return position{}, fmt.Errorf("failed to read the journal position: %w", err)
	}
	return p, nil
}

// Position returns the sequence number and the epoch of the newest journal entry (0, 0 for an
// empty journal).
func (s *Store) Position() (seq, epoch int64) {
	s.posMu.Lock()
	defer s.posMu.Unlock()
	return s.pos.seq, s.pos.epoch
}

// Head is Position with the time of the newest journal entry (zero for an empty journal).
// Every entry up to it is committed; one being committed may not be in it yet.
func (s *Store) Head() (seq, epoch int64, at time.Time) {
	s.posMu.Lock()
	defer s.posMu.Unlock()
	return s.pos.seq, s.pos.epoch, s.pos.at
}

// CommittedHead is Head after the commit in progress, if any, has moved it: it waits for the
// writer that is between its commit and the move (a moment), so that the head it returns is
// the newest entry the database has.
func (s *Store) CommittedHead() (seq, epoch int64, at time.Time) {
	s.commitMu.Lock()
	defer s.commitMu.Unlock()
	return s.Head()
}

// OldestSeq returns the sequence number of the oldest journal entry TrimJournal has kept (0 for
// an empty journal).
func (s *Store) OldestSeq() int64 {
	s.posMu.Lock()
	defer s.posMu.Unlock()
	return s.pos.oldest
}

// WaitAfter blocks until the journal has an entry after seq, or ctx ends (its error), or the
// store is closed (ErrClosed).
func (s *Store) WaitAfter(ctx context.Context, seq int64) error {
	for {
		s.posMu.Lock()
		cur, changed, closed := s.pos.seq, s.changed, s.closed
		s.posMu.Unlock()
		if cur > seq {
			return nil
		}
		if closed {
			return ErrClosed
		}
		select {
		case <-changed:
		case <-ctx.Done():
			return ctx.Err()
		}
	}
}

// journalPageBytes is about how much payload one JournalAfter returns: the leader holds a page
// in memory while it sends it, in a container of 256 MB (review 1).
var journalPageBytes = 8 << 20

// JournalAfter returns the journal entries after seq in order, up to the head (Head) as it
// was when it was called, at most limit (≤ 0 or more than MaxLimit: MaxLimit), and no more
// than about 8 MiB of payload: it stops before the entry that would go past that, but returns
// at least one entry, however large.
func (s *Store) JournalAfter(seq int64, limit int) ([]JournalEntry, error) {
	head, _, _ := s.Head()
	return s.JournalBetween(seq, head, limit)
}

// JournalBetween is JournalAfter up to the entry upTo, which a caller has read with Head
// before: a committed entry is in the database a moment before the head includes it, and a
// page that went past the head it is sent with would tell a follower that is current that it
// is ahead of the leader (E2E-2).
func (s *Store) JournalBetween(seq, upTo int64, limit int) ([]JournalEntry, error) {
	if limit <= 0 || limit > MaxLimit {
		limit = MaxLimit
	}
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return nil, err
	}
	rows, err := s.r.Query(`SELECT seq, epoch, at, op, payload, blob FROM journal WHERE seq > ? AND seq <= ? ORDER BY seq LIMIT ?`,
		seq, upTo, limit)
	if err != nil {
		return nil, fmt.Errorf("failed to read the journal: %w", err)
	}
	defer rows.Close()
	entries := []JournalEntry{}
	total := 0
	for rows.Next() {
		e, err := scanJournal(rows)
		if err != nil {
			return nil, err
		}
		if total += len(e.Payload); len(entries) > 0 && total > journalPageBytes {
			break // the next page begins with it
		}
		entries = append(entries, e)
	}
	return entries, rows.Err()
}

// Continuation is what CheckJournal finds about a follower's position.
type Continuation int

const (
	// Continues: the leader's journal has the follower's newest entry (or the follower has
	// none and the journal begins at 1), so the entries after it continue the follower's index.
	Continues Continuation = iota
	// Diverged: the leader's entry at the follower's seq is another one, or the leader has
	// none there: the follower's index is not a prefix of the leader's.
	Diverged
	// Trimmed: the journal no longer reaches back to the follower's seq.
	Trimmed
)

// trimmedKey is the meta row in which TrimJournal keeps the newest entry it removed, as a
// journalMark: the journal is checked against it for a follower at exactly OldestSeq-1.
const trimmedKey = "journal_trimmed"

// journalMark identifies a journal entry for CheckJournal.
type journalMark struct {
	Seq   int64  `json:"seq"`
	Epoch int64  `json:"epoch"`
	Sum   string `json:"sum"`
}

// CheckJournal decides, against the database and not the cached head, whether the journal
// continues the index of a follower whose newest entry is seq, of epoch epoch (< 0: not
// given) and with the checksum sum (JournalEntry.Sum; "": not given). For seq OldestSeq-1,
// whose entry is trimmed, it compares with what TrimJournal kept of that entry. The string
// says why when the answer is not Continues.
func (s *Store) CheckJournal(seq, epoch int64, sum string) (Continuation, string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return 0, "", err
	}
	// One read transaction: a trim between two of the reads would make an entry that was
	// there look missing, and missing reads as diverged.
	tx, err := s.r.Begin()
	if err != nil {
		return 0, "", err
	}
	defer func() { _ = tx.Rollback() }()
	var oldest, newest int64
	if err := tx.QueryRow(`SELECT COALESCE(MIN(seq), 0), COALESCE(MAX(seq), 0) FROM journal`).Scan(&oldest, &newest); err != nil {
		return 0, "", fmt.Errorf("failed to read the journal position: %w", err)
	}
	if seq == 0 {
		if oldest > 1 {
			return Trimmed, fmt.Sprintf("the journal begins at %d", oldest), nil
		}
		return Continues, "", nil
	}
	if seq > newest {
		return Diverged, fmt.Sprintf("the follower is at %d, the leader's journal ends at %d", seq, newest), nil
	}
	e, err := scanJournal(tx.QueryRow(`SELECT seq, epoch, at, op, payload, blob FROM journal WHERE seq = ?`, seq))
	switch {
	case err == nil:
		return compareMark(journalMark{Seq: e.Seq, Epoch: e.Epoch, Sum: e.Sum()}, epoch, sum)
	case !errors.Is(err, sql.ErrNoRows):
		return 0, "", fmt.Errorf("failed to read journal entry %d: %w", seq, err)
	}
	mark, ok, err := trimmedMark(tx)
	if err != nil {
		return 0, "", err
	}
	if ok && mark.Seq == seq {
		return compareMark(mark, epoch, sum)
	}
	if seq < oldest {
		return Trimmed, fmt.Sprintf("the journal begins at %d, after %d is trimmed", oldest, seq), nil
	}
	return Diverged, fmt.Sprintf("the leader has no entry %d", seq), nil
}

func compareMark(m journalMark, epoch int64, sum string) (Continuation, string, error) {
	if epoch >= 0 && m.Epoch != epoch {
		return Diverged, fmt.Sprintf("entry %d is of epoch %d here, not %d", m.Seq, m.Epoch, epoch), nil
	}
	if sum != "" && m.Sum != sum {
		return Diverged, fmt.Sprintf("entry %d of epoch %d is another one here (sum %s, not %s)", m.Seq, m.Epoch, m.Sum, sum), nil
	}
	return Continues, "", nil
}

// trimmedMark reads what TrimJournal kept of the newest entry it removed.
func trimmedMark(q querier) (journalMark, bool, error) {
	var v string
	err := q.QueryRow(`SELECT value FROM meta WHERE key = ?`, trimmedKey).Scan(&v)
	if errors.Is(err, sql.ErrNoRows) {
		return journalMark{}, false, nil
	}
	if err != nil {
		return journalMark{}, false, fmt.Errorf("failed to read %s: %w", trimmedKey, err)
	}
	var m journalMark
	if err := json.Unmarshal([]byte(v), &m); err != nil {
		return journalMark{}, false, fmt.Errorf("invalid %s %q: %w", trimmedKey, v, err)
	}
	return m, true, nil
}

// LastSum returns the newest journal entry's seq, epoch and checksum (JournalEntry.Sum); 0, 0
// and "" for an empty journal. A follower sends them as its position (after, epoch, sum), so
// that the leader can tell its entry from another one with the same seq and epoch.
func (s *Store) LastSum() (seq, epoch int64, sum string, err error) {
	seq, _ = s.Position()
	if seq == 0 {
		return 0, 0, "", nil
	}
	e, err := s.JournalEntryAt(seq)
	if err != nil {
		return 0, 0, "", err
	}
	return e.Seq, e.Epoch, e.Sum(), nil
}

// JournalEntryAt returns the journal entry seq. ErrNotFound when the journal does not have it
// (not yet, or trimmed).
func (s *Store) JournalEntryAt(seq int64) (JournalEntry, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return JournalEntry{}, err
	}
	e, err := scanJournal(s.r.QueryRow(`SELECT seq, epoch, at, op, payload, blob FROM journal WHERE seq = ?`, seq))
	if errors.Is(err, sql.ErrNoRows) {
		return JournalEntry{}, ErrNotFound
	}
	return e, err
}

func scanJournal(sc scanner) (JournalEntry, error) {
	var e JournalEntry
	var at, payload string
	if err := sc.Scan(&e.Seq, &e.Epoch, &at, &e.Op, &payload, &e.Blob); err != nil {
		return JournalEntry{}, err
	}
	var err error
	if e.At, err = parseTime(at); err != nil {
		return JournalEntry{}, fmt.Errorf("journal entry %d: %w", e.Seq, err)
	}
	e.Payload = json.RawMessage(payload)
	return e, nil
}

// Apply writes a journal entry of the leader on a follower: exactly the rows the leader wrote
// (same ids, same times) and the same journal row, in one transaction. The entry must follow
// the local journal (e.Seq is the local seq + 1), else ErrOutOfOrder. An entry that does not
// fit the local index returns ErrDiverged. An entry whose rows reference the blob e.Blob when
// it is not stored returns ErrBlobMissing (a *BlobMissingError; after ErrDiverged: a snapshot
// makes the entry moot); the follower fetches the blob and applies the entry again. Nothing is
// written on an error.
func (s *Store) Apply(e JournalEntry) error {
	return s.ApplyBatch([]JournalEntry{e})
}

// BlobMissingError is the ErrBlobMissing of Apply and ApplyBatch: the blobs the entries
// reference and the store lacks, Hash (of entry Seq) the first of them.
type BlobMissingError struct {
	Seq    int64
	Op     string
	Hash   string
	Hashes []string // every missing blob of the batch, in entry order, Hash first
}

func (e *BlobMissingError) Error() string {
	msg := fmt.Sprintf("journal entry %d (%s): blob %s: %v", e.Seq, e.Op, e.Hash, ErrBlobMissing)
	if len(e.Hashes) > 1 {
		msg += fmt.Sprintf(" (and %d more of the batch)", len(e.Hashes)-1)
	}
	return msg
}

// Unwrap makes errors.Is(err, ErrBlobMissing) true.
func (e *BlobMissingError) Unwrap() error { return ErrBlobMissing }

// ApplyBatch is Apply for consecutive entries in one transaction: one commit for the batch
// instead of one per entry, which is what kept a follower that fell behind from catching up
// (E2E-3). The first entry must follow the local journal and each the one before, else
// ErrOutOfOrder. The errors are Apply's, of the first entry that fails; ErrDiverged of any
// entry comes before ErrBlobMissing, which names the first missing blob and lists them all.
// Nothing is written on an error.
func (s *Store) ApplyBatch(entries []JournalEntry) error {
	if len(entries) == 0 {
		return nil
	}
	batch := make([]JournalEntry, len(entries))
	for i, e := range entries {
		var payload bytes.Buffer
		if err := json.Compact(&payload, e.Payload); err != nil {
			return fmt.Errorf("journal entry %d: invalid payload: %w", e.Seq, err)
		}
		e.Payload = payload.Bytes()
		e.At = stamp(e.At)
		batch[i] = e
	}

	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	last, err := lastSeq(tx)
	if err != nil {
		return err
	}
	pruned := make([]PruneStats, 0, len(batch))
	var missing *BlobMissingError
	for _, e := range batch {
		if e.Seq != last+1 {
			return fmt.Errorf("%w: entry %d after local %d", ErrOutOfOrder, e.Seq, last)
		}
		need, p, err := applyEntry(tx, e)
		if err != nil {
			return err
		}
		if need != "" {
			// Checked and renewed under blobMu: a GCBlobs that read the references before this
			// commit then leaves the blob, or removed it already and the entry waits for it.
			// The rows of the later entries are still applied, for an ErrDiverged among them.
			switch err := s.renewBlob(need); {
			case errors.Is(err, ErrBlobMissing):
				if missing == nil {
					missing = &BlobMissingError{Seq: e.Seq, Op: e.Op, Hash: need}
				}
				if !slices.Contains(missing.Hashes, need) {
					missing.Hashes = append(missing.Hashes, need)
				}
			case err != nil:
				return fmt.Errorf("journal entry %d (%s): %w", e.Seq, e.Op, err)
			}
		}
		if err := insertJournal(tx, e); err != nil {
			return err
		}
		pruned = append(pruned, p)
		last = e.Seq
	}
	if missing != nil {
		return missing
	}
	if err := s.commitAndAdvance(tx, batch[len(batch)-1]); err != nil {
		return fmt.Errorf("failed to commit journal entries %d to %d: %w", batch[0].Seq, last, err)
	}
	for _, p := range pruned {
		p.count()
	}
	return nil
}

// applyEntry writes the rows of one entry in tx and returns the blob they reference anew.
func applyEntry(tx *sql.Tx, e JournalEntry) (need string, pruned PruneStats, err error) {
	switch e.Op {
	case OpVersion, OpCheck:
		var p versionPayload
		if err := decodePayload(e, &p); err != nil {
			return "", PruneStats{}, err
		}
		if e.Op == OpVersion {
			need = p.Version.SHA256
		}
		err = applyVersion(tx, e.Op, p)
	case OpEntryDelete:
		var p entryDeletePayload
		if err := decodePayload(e, &p); err != nil {
			return "", PruneStats{}, err
		}
		err = applyEntryDelete(tx, p)
	case OpFileVersion:
		var p fileVersionPayload
		if err := decodePayload(e, &p); err != nil {
			return "", PruneStats{}, err
		}
		if p.Version.Deleted == 0 {
			need = p.Version.SHA256
		}
		err = applyFileVersion(tx, p)
	case OpPrune:
		var p prunePayload
		if err := decodePayload(e, &p); err != nil {
			return "", PruneStats{}, err
		}
		pruned, err = applyPrune(tx, p.Cutoff)
	case OpEpoch:
		var p epochPayload
		if err := decodePayload(e, &p); err != nil {
			return "", PruneStats{}, err
		}
		err = applyEpoch(tx, p)
	default:
		return "", PruneStats{}, fmt.Errorf("journal entry %d: unknown op %q", e.Seq, e.Op)
	}
	if err != nil {
		if isConstraint(err) && !errors.Is(err, ErrDiverged) {
			err = fmt.Errorf("%w: %v", ErrDiverged, err)
		}
		return "", PruneStats{}, fmt.Errorf("journal entry %d (%s): %w", e.Seq, e.Op, err)
	}
	// The follower fetches e.Blob before it applies again; the rows must name that one.
	if need != e.Blob {
		return "", PruneStats{}, fmt.Errorf("journal entry %d (%s): it names blob %q, its rows %q", e.Seq, e.Op, e.Blob, need)
	}
	return need, pruned, nil
}

// decodePayload refuses a field this binary does not know: a follower that dropped it would
// not be a copy of the leader.
func decodePayload(e JournalEntry, v any) error {
	dec := json.NewDecoder(bytes.NewReader(e.Payload))
	dec.DisallowUnknownFields()
	if err := dec.Decode(v); err != nil {
		return fmt.Errorf("journal entry %d (%s): invalid payload: %w", e.Seq, e.Op, err)
	}
	return nil
}

// applyVersion writes the rows of a version or a check entry: the entry row, the superseded
// version, and the new version (version) or the updated current one (check).
func applyVersion(tx *sql.Tx, op string, p versionPayload) error {
	e := p.Entry
	_, err := tx.Exec(`INSERT INTO entry (id, key, url, host, source, accept, accept_language, created_at, current_version)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(id) DO UPDATE SET key = excluded.key, url = excluded.url, host = excluded.host,
			source = excluded.source, accept = excluded.accept, accept_language = excluded.accept_language,
			created_at = excluded.created_at, current_version = excluded.current_version`,
		e.ID, e.Key, e.URL, e.Host, e.Source, e.Accept, e.AcceptLanguage, e.CreatedAt, e.CurrentVersion)
	if err != nil {
		return fmt.Errorf("failed to write entry %d: %w", e.ID, err)
	}
	if p.Superseded != nil {
		if err := execOne(tx, `UPDATE version SET superseded_at = ? WHERE id = ? AND entry_id = ? AND superseded_at IS NULL`,
			p.Superseded.At, p.Superseded.ID, e.ID); err != nil {
			return fmt.Errorf("failed to supersede version %d: %w", p.Superseded.ID, err)
		}
	}
	v := p.Version
	if op == OpCheck {
		err = execOne(tx, `UPDATE version SET entry_id = ?, status = ?, sha256 = ?, size = ?, headers = ?,
			fetched_at = ?, checked_at = ?, superseded_at = ? WHERE id = ?`,
			v.EntryID, v.Status, v.SHA256, v.Size, v.Headers, v.FetchedAt, v.CheckedAt, v.SupersededAt, v.ID)
	} else {
		_, err = tx.Exec(`INSERT INTO version (id, entry_id, status, sha256, size, headers, fetched_at, checked_at, superseded_at)
			VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			v.ID, v.EntryID, v.Status, v.SHA256, v.Size, v.Headers, v.FetchedAt, v.CheckedAt, v.SupersededAt)
	}
	if err != nil {
		return fmt.Errorf("failed to write version %d: %w", v.ID, err)
	}
	return nil
}

func applyEntryDelete(tx *sql.Tx, p entryDeletePayload) error {
	if err := execOne(tx, `DELETE FROM entry WHERE id = ? AND key = ?`, p.ID, p.Key); err != nil {
		return fmt.Errorf("failed to delete entry %d: %w", p.ID, err)
	}
	return nil
}

func applyEpoch(tx *sql.Tx, p epochPayload) error {
	for _, kv := range [][2]string{
		{"epoch", strconv.FormatInt(p.Epoch, 10)},
		{"leader_instance", p.Instance},
		{"leader_url", p.URL},
		{"leader_since", p.Since},
	} {
		if _, err := tx.Exec(`INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
			kv[0], kv[1]); err != nil {
			return fmt.Errorf("failed to write %s: %w", kv[0], err)
		}
	}
	return nil
}

// execOne runs a statement that must change exactly one row; any other count means that the
// index is not the one the statement was written for.
func execOne(tx *sql.Tx, query string, args ...any) error {
	res, err := tx.Exec(query, args...)
	if err != nil {
		return err
	}
	n, err := res.RowsAffected()
	if err != nil {
		return err
	}
	if n != 1 {
		return fmt.Errorf("%w: %d rows changed, want 1", ErrDiverged, n)
	}
	return nil
}

// StartEpoch begins a leader's term: the epoch becomes max(the index's epoch, floor) + 1, with
// the instance and its URL as the leader (op epoch). floor is the epoch the leader election
// last announced, which a follower that missed entries may not have seen.
func (s *Store) StartEpoch(instance, url string, floor int64, at time.Time) (int64, JournalEntry, error) {
	at = stamp(at)
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return 0, JournalEntry{}, err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return 0, JournalEntry{}, err
	}
	defer func() { _ = tx.Rollback() }()
	epoch, err := metaEpoch(tx)
	if err != nil {
		return 0, JournalEntry{}, err
	}
	var journalEpoch int64
	if err := tx.QueryRow(`SELECT COALESCE(MAX(epoch), 0) FROM journal`).Scan(&journalEpoch); err != nil {
		return 0, JournalEntry{}, fmt.Errorf("failed to read the journal's epoch: %w", err)
	}
	epoch = max(epoch, journalEpoch, floor) + 1
	p := epochPayload{Epoch: epoch, Instance: instance, URL: url, Since: FormatTime(at)}
	if err := applyEpoch(tx, p); err != nil {
		return 0, JournalEntry{}, err
	}
	e, err := s.commit(tx, OpEpoch, at, p, "")
	if err != nil {
		return 0, JournalEntry{}, err
	}
	return epoch, e, nil
}

// TrimJournal removes the oldest journal entries up to the newest one older than before, but
// never the newest entry (which holds the position). It is local and not journaled: each
// instance keeps its own journal as long as it wants.
//
// It trims a prefix, so the journal stays contiguous from OldestSeq: journal.at is the time a
// write was given, which need not grow with seq (writes wait for the writer in any order), and
// trimming by time alone left holes that a follower was served across (review 1). The prefix
// ends at the newest old entry, not before the oldest young one: one entry stamped while the
// clock was ahead would otherwise stop all trimming until the clock passed it (review 2:
// trimjournal-stalls-behind-future-at); a young entry before an old one goes a little early,
// which costs a follower that far behind a snapshot, nothing else.
//
// The newest entry removed is kept in meta (seq, epoch, checksum), so that CheckJournal can
// still check a follower at exactly OldestSeq-1. Log events: journal.trimmed.
func (s *Store) TrimJournal(before time.Time) (int, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return 0, err
	}
	tx, err := s.w.Begin()
	if err != nil {
		return 0, err
	}
	defer func() { _ = tx.Rollback() }()
	var upTo, oldest, newest int64
	if err := tx.QueryRow(`SELECT COALESCE((SELECT MAX(seq) FROM journal WHERE at < ?), 0),
			COALESCE(MIN(seq), 0), COALESCE(MAX(seq), 0) FROM journal`, FormatTime(stamp(before))).Scan(&upTo, &oldest, &newest); err != nil {
		return 0, fmt.Errorf("failed to read the journal position: %w", err)
	}
	upTo = min(upTo, newest-1)
	var n int64
	if upTo >= oldest && upTo > 0 {
		last, err := scanJournal(tx.QueryRow(`SELECT seq, epoch, at, op, payload, blob FROM journal WHERE seq = ?`, upTo))
		if err != nil {
			return 0, fmt.Errorf("failed to read journal entry %d: %w", upTo, err)
		}
		mark, err := json.Marshal(journalMark{Seq: last.Seq, Epoch: last.Epoch, Sum: last.Sum()})
		if err != nil {
			return 0, err
		}
		if _, err := tx.Exec(`INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
			trimmedKey, string(mark)); err != nil {
			return 0, fmt.Errorf("failed to keep the trimmed entry: %w", err)
		}
		res, err := tx.Exec(`DELETE FROM journal WHERE seq <= ?`, upTo)
		if err != nil {
			return 0, fmt.Errorf("failed to trim the journal: %w", err)
		}
		if n, err = res.RowsAffected(); err != nil {
			return 0, err
		}
		oldest = upTo + 1
	}
	if err := tx.Commit(); err != nil {
		return 0, fmt.Errorf("failed to trim the journal: %w", err)
	}
	s.posMu.Lock()
	s.pos.oldest = max(s.pos.oldest, oldest)
	s.posMu.Unlock()
	prunedTotal.Add(float64(n), "journal_entries")
	oplog.For("store").Info("journal trimmed", "event", "journal.trimmed", "removed", n, "oldest_seq", oldest,
		"before", FormatTime(before))
	return int(n), nil
}
