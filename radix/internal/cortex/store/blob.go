package store

import (
	"bufio"
	"compress/gzip"
	"crypto/sha256"
	"database/sql"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/leonieziechmann/betula/radix/internal/oplog"
)

// BlobInfo describes a stored blob.
type BlobInfo struct {
	Hash   string `json:"sha256"` // of the original bytes, lower-case hex
	Size   int64  `json:"size"`   // original bytes
	Stored int64  `json:"stored"` // bytes on disk
	Gzip   bool   `json:"gzip"`   // stored compressed
}

// chunkSize is how much of a body PutBlob reads at a time; with the two write buffers and
// the compressor it bounds what a write holds in memory to about 2 MB, whatever the size.
const chunkSize = 256 << 10

// probeBytes: every this many bytes PutBlob checks whether the compressed stream so far is at
// least 1 % smaller than its input, and stops compressing when it is not. An already
// compressed file (a PDF, model weights in an archive) is then not run through gzip's best
// compression to its end, which costs minutes of CPU for a file of gigabytes.
var probeBytes int64 = 32 << 20

// gzipWriters keeps compressors for reuse: one at BestCompression holds about 1 MB.
var gzipWriters = sync.Pool{New: func() any {
	w, _ := gzip.NewWriterLevel(io.Discard, gzip.BestCompression)
	return w
}}

// smallEnough is the rule for keeping the compressed form: at least 1 % smaller.
func smallEnough(compressed, original int64) bool {
	return compressed*100 <= original*99
}

func (s *Store) blobPath(hash string, gz bool) string {
	p := filepath.Join(s.blobRoot(), hash[:2], hash)
	if gz {
		p += ".gz"
	}
	return p
}

// parseBlobName returns the hash of a file name in a blob directory.
func parseBlobName(name string) (hash string, gz bool, ok bool) {
	hash, gz = strings.CutSuffix(name, ".gz")
	return hash, gz, ValidHash(hash)
}

// PutBlob stores the content of r and returns its hash. The body is streamed to DIR/tmp while
// it is hashed and compressed (gzip, best compression), so memory stays bounded for bodies of
// any size; the compressed form is kept when it is at least 1 % smaller than the original,
// otherwise the original. The file is synced and renamed into place; a blob that exists
// already is kept as it is (and its time of last use renewed, so that GCBlobs leaves it for
// another grace period).
//
// expect is the sha256 hex the content must have, or "": a different hash returns
// ErrHashMismatch. More than maxBytes bytes return ErrTooLarge (maxBytes ≤ 0: no limit).
// Nothing is left behind on an error.
func (s *Store) PutBlob(r io.Reader, expect string, maxBytes int64) (BlobInfo, error) {
	if expect != "" && !ValidHash(expect) {
		return BlobInfo{}, fmt.Errorf("expected hash %q: %w", expect, ErrInvalidHash)
	}
	raw, err := s.createTemp("blob-*")
	if err != nil {
		return BlobInfo{}, err
	}
	defer raw.discard()
	packed, err := s.createTemp("blob-*.gz")
	if err != nil {
		return BlobInfo{}, err
	}
	defer packed.discard()

	zw := gzipWriters.Get().(*gzip.Writer)
	zw.Reset(packed)
	defer func() {
		zw.Reset(io.Discard) // drop the file before the compressor goes back to the pool
		gzipWriters.Put(zw)
	}()
	compressing := true

	hasher := sha256.New()
	buf := make([]byte, chunkSize)
	var total int64
	nextProbe := probeBytes
	for {
		n, rerr := r.Read(buf)
		if n > 0 {
			total += int64(n)
			if maxBytes > 0 && total > maxBytes {
				return BlobInfo{}, fmt.Errorf("%w: more than %d bytes", ErrTooLarge, maxBytes)
			}
			hasher.Write(buf[:n])
			if _, err := raw.Write(buf[:n]); err != nil {
				return BlobInfo{}, fmt.Errorf("failed to write a blob: %w", err)
			}
			if compressing {
				if _, err := zw.Write(buf[:n]); err != nil {
					return BlobInfo{}, fmt.Errorf("failed to write a blob: %w", err)
				}
				if total >= nextProbe {
					if err := zw.Flush(); err != nil {
						return BlobInfo{}, fmt.Errorf("failed to write a blob: %w", err)
					}
					compressing = smallEnough(packed.n, total)
					nextProbe += probeBytes
				}
			}
		}
		if rerr == io.EOF {
			break
		}
		if rerr != nil {
			return BlobInfo{}, fmt.Errorf("failed to read the body: %w", rerr)
		}
	}
	if compressing {
		if err := zw.Close(); err != nil {
			return BlobInfo{}, fmt.Errorf("failed to write a blob: %w", err)
		}
	}

	hash := hex.EncodeToString(hasher.Sum(nil))
	if expect != "" && hash != expect {
		return BlobInfo{}, fmt.Errorf("%w: got sha256:%s, want sha256:%s", ErrHashMismatch, hash, expect)
	}
	keep, gz := raw, false
	if compressing && smallEnough(packed.n, total) {
		keep, gz = packed, true
	}
	stored, gz, err := s.place(hash, keep, gz, false)
	if err != nil {
		return BlobInfo{}, err
	}
	return BlobInfo{Hash: hash, Size: total, Stored: stored, Gzip: gz}, nil
}

// ImportStored stores a blob in the form another instance stored it (gzip or raw), as
// OpenStored gave it there: it writes r to DIR/tmp, checks that the content (decompressed when
// gz) has the sha256 hash, and renames it into place. A blob that exists already is kept.
// Content with another hash, or anything but one whole gzip stream, returns ErrHashMismatch.
//
// The file is synced, but the directory it is renamed into is not, which cost a follower an
// fsync per blob (E2E-3): the next commit of the index syncs the directories of every blob
// imported since (syncPending), before any row that names the blob can be durable, and so do
// SyncImported and Close. A crash before that loses at most the name of a blob no durable
// row references yet, which a follower fetches again (it counts what its index lacks).
func (s *Store) ImportStored(hash string, r io.Reader, gz bool) error {
	if !ValidHash(hash) {
		return fmt.Errorf("blob %q: %w", hash, ErrInvalidHash)
	}
	t, err := s.createTemp("import-*")
	if err != nil {
		return err
	}
	defer t.discard()

	hasher := sha256.New()
	in := &readErr{r: r}
	src := io.TeeReader(in, t)
	if gz {
		// One gzip stream and nothing after it, as PutBlob writes it: StatBlob takes the size
		// from the trailer, which a second stream would replace with the count of its own part.
		// A bufio.Reader is a ByteReader, so the decompressor reads no further than the stream.
		br := bufio.NewReader(src)
		zr, err := gzip.NewReader(br)
		if err == nil {
			zr.Multistream(false)
			_, err = io.Copy(hasher, zr)
			_ = zr.Close()
		}
		if err == nil {
			if _, perr := br.Peek(1); perr == nil {
				err = errors.New("more follows the gzip stream")
			} else if perr != io.EOF {
				err = perr
			}
		}
		if err != nil {
			switch {
			case in.err != nil:
				return fmt.Errorf("failed to read blob %s: %w", hash, in.err)
			case t.err != nil:
				return fmt.Errorf("failed to write blob %s: %w", hash, t.err)
			}
			return fmt.Errorf("%w: blob %s is not one whole gzip stream: %v", ErrHashMismatch, hash, err)
		}
	} else if _, err := io.Copy(hasher, src); err != nil {
		return fmt.Errorf("failed to import blob %s: %w", hash, err)
	}
	if got := hex.EncodeToString(hasher.Sum(nil)); got != hash {
		return fmt.Errorf("%w: got sha256:%s, want sha256:%s", ErrHashMismatch, got, hash)
	}
	_, _, err = s.place(hash, t, gz, true)
	return err
}

// SyncImported makes the blobs ImportStored placed durable now (their directories synced),
// for a follower that back-fills blobs its index already references: no commit follows those.
func (s *Store) SyncImported() error {
	return s.syncPending()
}

// syncPending syncs the blob directories ImportStored renamed files into since the last time.
// syncMu is held through the syncs, so that a commit that comes while another goroutine syncs
// waits for it instead of finding the set empty and committing first.
func (s *Store) syncPending() error {
	s.syncMu.Lock()
	defer s.syncMu.Unlock()
	s.pendingMu.Lock()
	dirs := s.pendingDirs
	s.pendingDirs = nil
	s.pendingMu.Unlock()
	for dir := range dirs {
		if err := syncDir(dir); err != nil {
			s.pendingMu.Lock() // again next time; the commit that needed it fails meanwhile
			if s.pendingDirs == nil {
				s.pendingDirs = make(map[string]struct{})
			}
			for d := range dirs {
				s.pendingDirs[d] = struct{}{}
			}
			s.pendingMu.Unlock()
			return fmt.Errorf("failed to sync %s: %w", dir, err)
		}
	}
	return nil
}

// readErr remembers the first error of a reader other than io.EOF, to tell a broken
// connection from broken content.
type readErr struct {
	r   io.Reader
	err error
}

func (e *readErr) Read(p []byte) (int, error) {
	n, err := e.r.Read(p)
	if err != nil && err != io.EOF && e.err == nil {
		e.err = err
	}
	return n, err
}

// place makes a finished temporary file the blob hash: synced, renamed into place, the
// directory synced (or, with later, noted for syncPending). When the blob exists already (in
// either form), the temporary file is dropped, the existing file's time renewed, and its size
// and form returned.
func (s *Store) place(hash string, t *tempFile, gz, later bool) (stored int64, isGzip bool, err error) {
	// A blob that exists needs neither the sync nor the rename.
	if stored, isGzip, ok, err := s.reuse(hash); err != nil || ok {
		return stored, isGzip, err
	}
	if err := t.finish(); err != nil {
		return 0, false, fmt.Errorf("failed to write blob %s: %w", hash, err)
	}
	dir := filepath.Join(s.blobRoot(), hash[:2])
	if err := os.Mkdir(dir, 0755); err == nil {
		if err := syncDir(s.blobRoot()); err != nil {
			return 0, false, fmt.Errorf("failed to sync %s: %w", s.blobRoot(), err)
		}
	} else if !errors.Is(err, fs.ErrExist) {
		return 0, false, fmt.Errorf("failed to create %s: %w", dir, err)
	}

	s.blobMu.Lock()
	if stored, isGzip, ok, err := s.reuseLocked(hash); err != nil || ok {
		s.blobMu.Unlock()
		return stored, isGzip, err
	}
	err = os.Rename(t.f.Name(), s.blobPath(hash, gz))
	s.blobMu.Unlock()
	if err != nil {
		return 0, false, fmt.Errorf("failed to store blob %s: %w", hash, err)
	}
	t.placed = true
	if later {
		s.pendingMu.Lock()
		if s.pendingDirs == nil {
			s.pendingDirs = make(map[string]struct{})
		}
		s.pendingDirs[dir] = struct{}{}
		s.pendingMu.Unlock()
		return t.n, gz, nil
	}
	if err := syncDir(dir); err != nil {
		return 0, false, fmt.Errorf("failed to sync %s: %w", dir, err)
	}
	return t.n, gz, nil
}

// reuse renews the time of the blob hash when it exists and returns its size and form.
func (s *Store) reuse(hash string) (stored int64, gz bool, ok bool, err error) {
	s.blobMu.Lock()
	defer s.blobMu.Unlock()
	return s.reuseLocked(hash)
}

// renewBlob is for a write about to reference the blob hash: under blobMu it checks that the
// blob is stored and renews its time, so that GCBlobs, which looks at the time again under
// blobMu before it removes a blob, leaves it for another grace period whatever references it
// read before the write. ErrBlobMissing when the blob is not stored (GC may have been first).
func (s *Store) renewBlob(hash string) error {
	if !ValidHash(hash) {
		return fmt.Errorf("blob %q: %w", hash, ErrInvalidHash)
	}
	_, _, ok, err := s.reuse(hash)
	if err != nil {
		return err
	}
	if !ok {
		return fmt.Errorf("blob %s: %w", hash, ErrBlobMissing)
	}
	return nil
}

func (s *Store) reuseLocked(hash string) (stored int64, gz bool, ok bool, err error) {
	for _, gz := range []bool{false, true} {
		path := s.blobPath(hash, gz)
		info, err := os.Stat(path)
		if err == nil {
			now := time.Now()
			if err := os.Chtimes(path, now, now); err != nil {
				return 0, false, false, fmt.Errorf("failed to touch blob %s: %w", hash, err)
			}
			return info.Size(), gz, true, nil
		}
		if !errors.Is(err, fs.ErrNotExist) {
			return 0, false, false, fmt.Errorf("failed to stat blob %s: %w", hash, err)
		}
	}
	return 0, false, false, nil
}

// OpenBlob returns the original bytes of a blob (decompressed when it is stored compressed).
func (s *Store) OpenBlob(hash string) (io.ReadCloser, error) {
	f, gz, err := s.openEither(hash)
	if err != nil {
		return nil, err
	}
	if !gz {
		return f, nil
	}
	zr, err := gzip.NewReader(bufio.NewReaderSize(f, 64<<10))
	if err != nil {
		_ = f.Close()
		return nil, fmt.Errorf("blob %s: %w", hash, err)
	}
	return &gzipFile{Reader: zr, f: f}, nil
}

// gzipFile reads a compressed blob and closes its file.
type gzipFile struct {
	*gzip.Reader
	f *os.File
}

func (g *gzipFile) Close() error {
	_ = g.Reader.Close()
	return g.f.Close()
}

// OpenRaw returns the file of a blob stored as it is, seekable for a Range request. A blob
// stored compressed returns ErrCompressed.
func (s *Store) OpenRaw(hash string) (*os.File, error) {
	f, gz, err := s.openEither(hash)
	if err != nil {
		return nil, err
	}
	if gz {
		_ = f.Close()
		return nil, fmt.Errorf("blob %s: %w", hash, ErrCompressed)
	}
	return f, nil
}

// OpenStored returns a blob as it is stored, for a follower and for a client that accepts
// gzip: the bytes on disk, whether they are gzip, and how many there are.
func (s *Store) OpenStored(hash string) (rc io.ReadCloser, gzip bool, stored int64, err error) {
	f, gz, err := s.openEither(hash)
	if err != nil {
		return nil, false, 0, err
	}
	info, err := f.Stat()
	if err != nil {
		_ = f.Close()
		return nil, false, 0, fmt.Errorf("blob %s: %w", hash, err)
	}
	return f, gz, info.Size(), nil
}

// HasBlob says whether the blob is stored, in either form.
func (s *Store) HasBlob(hash string) bool {
	if !ValidHash(hash) {
		return false
	}
	for _, gz := range []bool{false, true} {
		if _, err := os.Stat(s.blobPath(hash, gz)); err == nil {
			return true
		}
	}
	return false
}

// maxDeflateRatio is the most original bytes one byte of deflate data can give: a match of 258
// bytes in two bits of codes.
const maxDeflateRatio = 1032

// StatBlob describes a stored blob. The original size of a compressed blob comes from the
// index. For a compressed blob no row names (one uploaded by hash, or one retention dropped)
// it comes from gzip's trailer, which counts modulo 4 GiB, so only when the stream is too short
// to give 4 GiB; otherwise Size is -1, unknown (a Content-Length taken from the trailer would
// cut a longer body short without an error).
func (s *Store) StatBlob(hash string) (BlobInfo, error) {
	f, gz, err := s.openEither(hash)
	if err != nil {
		return BlobInfo{}, err
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil {
		return BlobInfo{}, fmt.Errorf("blob %s: %w", hash, err)
	}
	b := BlobInfo{Hash: hash, Size: info.Size(), Stored: info.Size(), Gzip: gz}
	if !gz {
		return b, nil
	}
	if size, ok, err := s.indexedSize(hash); err != nil {
		return BlobInfo{}, err
	} else if ok {
		b.Size = size
		return b, nil
	}
	isize, err := gzipTrailerSize(f, info.Size())
	if err != nil {
		return BlobInfo{}, fmt.Errorf("blob %s: %w", hash, err)
	}
	// One stream (PutBlob and ImportStored keep no other) of fewer bytes than this gives less
	// than 4 GiB, which the trailer counts exactly.
	b.Size = -1
	if info.Size() < (1<<32)/maxDeflateRatio {
		b.Size = isize
	}
	return b, nil
}

// indexedSize is the original size of a blob as a version or a file version records it.
func (s *Store) indexedSize(hash string) (int64, bool, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return 0, false, err
	}
	var size int64
	err := s.r.QueryRow(`SELECT size FROM version WHERE sha256 = ?1
		UNION ALL SELECT size FROM file_version WHERE sha256 = ?1 LIMIT 1`, hash).Scan(&size)
	if errors.Is(err, sql.ErrNoRows) {
		return 0, false, nil
	}
	if err != nil {
		return 0, false, fmt.Errorf("failed to look up blob %s: %w", hash, err)
	}
	return size, true, nil
}

// gzipTrailerSize reads ISIZE, the last four bytes of a gzip file: the original size modulo 2³².
func gzipTrailerSize(f *os.File, size int64) (int64, error) {
	if size < 18 {
		return 0, errors.New("gzip file too short")
	}
	var b [4]byte
	if _, err := f.ReadAt(b[:], size-4); err != nil {
		return 0, err
	}
	return int64(binary.LittleEndian.Uint32(b[:])), nil
}

// openEither opens the raw or the compressed file of a blob.
func (s *Store) openEither(hash string) (*os.File, bool, error) {
	if !ValidHash(hash) {
		return nil, false, fmt.Errorf("blob %q: %w", hash, ErrInvalidHash)
	}
	for _, gz := range []bool{false, true} {
		f, err := os.Open(s.blobPath(hash, gz))
		if err == nil {
			return f, gz, nil
		}
		if !errors.Is(err, fs.ErrNotExist) {
			return nil, false, fmt.Errorf("blob %s: %w", hash, err)
		}
	}
	return nil, false, fmt.Errorf("blob %s: %w", hash, ErrNotFound)
}

// GCBlobs removes blob files that no version and no file version references and that were not
// written or reused (PutBlob, ImportStored) or referenced anew (RecordFetch, PutFile, Apply)
// within grace before now. The grace protects a blob between PutBlob and the write that
// references it. The index is made durable before a blob it no longer names is removed. Log
// events: gc.finished.
func (s *Store) GCBlobs(grace time.Duration, now time.Time) (removed int, bytes int64, err error) {
	start := time.Now()
	cutoff := now.Add(-grace)
	dirs, err := os.ReadDir(s.blobRoot())
	if err != nil {
		return 0, 0, fmt.Errorf("failed to read %s: %w", s.blobRoot(), err)
	}
	for _, d := range dirs {
		if !d.IsDir() || !isHexPair(d.Name()) {
			continue
		}
		n, b, err := s.gcDir(d.Name(), cutoff)
		removed += n
		bytes += b
		if err != nil {
			prunedTotal.Add(float64(removed), "blobs")
			return removed, bytes, err
		}
	}
	prunedTotal.Add(float64(removed), "blobs")
	oplog.For("store").Info("blob collection finished", "event", "gc.finished", "removed", removed, "bytes", bytes,
		"grace_s", int64(grace.Seconds()), "duration_ms", time.Since(start).Milliseconds())
	return removed, bytes, nil
}

// gcDir collects one directory blobs/sha256/<hh>. It holds the index for reading from the
// look-up of the references to the last removal, so that ReplaceIndex cannot bring in a
// reference in between.
func (s *Store) gcDir(hh string, cutoff time.Time) (int, int64, error) {
	dir := filepath.Join(s.blobRoot(), hh)
	files, err := os.ReadDir(dir)
	if err != nil {
		return 0, 0, fmt.Errorf("failed to read %s: %w", dir, err)
	}
	var candidates []string
	for _, f := range files {
		hash, _, ok := parseBlobName(f.Name())
		if !ok || hash[:2] != hh {
			continue
		}
		info, err := f.Info()
		if errors.Is(err, fs.ErrNotExist) {
			continue
		}
		if err != nil {
			return 0, 0, fmt.Errorf("failed to stat %s: %w", f.Name(), err)
		}
		if info.Mode().IsRegular() && info.ModTime().Before(cutoff) {
			candidates = append(candidates, f.Name())
		}
	}
	if len(candidates) == 0 {
		return 0, 0, nil
	}

	s.mu.RLock()
	defer s.mu.RUnlock()
	if err := s.ready(); err != nil {
		return 0, 0, err
	}
	refs, err := s.referencedWithPrefix(hh)
	if err != nil {
		return 0, 0, err
	}
	var unreferenced []string
	for _, name := range candidates {
		hash, _, _ := parseBlobName(name)
		if _, ok := refs[hash]; !ok {
			unreferenced = append(unreferenced, name)
		}
	}
	if len(unreferenced) == 0 {
		return 0, 0, nil
	}
	// A reference these rows lack may have gone in a write that is not durable yet: it must be
	// before its blob goes, or a power loss brings back the row without the blob. (PutBlob is
	// the same rule the other way round: the blob is durable before the row that names it.)
	if err := s.syncIndex(); err != nil {
		return 0, 0, err
	}
	removed, bytes := 0, int64(0)
	for _, name := range unreferenced {
		n, ok, err := s.removeIfStale(filepath.Join(dir, name), cutoff)
		if err != nil {
			return removed, bytes, err
		}
		if ok {
			removed++
			bytes += n
		}
	}
	if removed > 0 {
		if err := syncDir(dir); err != nil {
			return removed, bytes, fmt.Errorf("failed to sync %s: %w", dir, err)
		}
	}
	return removed, bytes, nil
}

// removeIfStale removes a blob file unless PutBlob, ImportStored or a write that references it
// (renewBlob) renewed it since it was found old.
func (s *Store) removeIfStale(path string, cutoff time.Time) (int64, bool, error) {
	s.blobMu.Lock()
	defer s.blobMu.Unlock()
	info, err := os.Lstat(path)
	if errors.Is(err, fs.ErrNotExist) {
		return 0, false, nil
	}
	if err != nil {
		return 0, false, fmt.Errorf("failed to stat %s: %w", path, err)
	}
	if !info.ModTime().Before(cutoff) {
		return 0, false, nil
	}
	if err := os.Remove(path); err != nil {
		return 0, false, fmt.Errorf("failed to remove %s: %w", path, err)
	}
	return info.Size(), true, nil
}

// referencedWithPrefix returns the original size of every blob whose hash starts with hh and
// that a version or a file version references. The caller holds mu for reading.
func (s *Store) referencedWithPrefix(hh string) (map[string]int64, error) {
	rows, err := s.r.Query(`SELECT sha256, MAX(size) FROM (
			SELECT sha256, size FROM version WHERE sha256 >= ?1 AND sha256 < ?2
			UNION ALL
			SELECT sha256, size FROM file_version WHERE sha256 >= ?1 AND sha256 < ?2
		) GROUP BY sha256`, hh, hh+"g") // every hex digit sorts before "g"
	if err != nil {
		return nil, fmt.Errorf("failed to read the blob references: %w", err)
	}
	defer rows.Close()
	refs := make(map[string]int64)
	for rows.Next() {
		var hash string
		var size int64
		if err := rows.Scan(&hash, &size); err != nil {
			return nil, err
		}
		refs[hash] = size
	}
	return refs, rows.Err()
}

// isHexPair says whether name is a blob directory: two lower-case hex digits.
func isHexPair(name string) bool {
	return len(name) == 2 && isHexDigit(name[0]) && isHexDigit(name[1])
}

// tempFile is a file being written in DIR/tmp: buffered, counted, removed unless placed.
type tempFile struct {
	f      *os.File
	buf    *bufio.Writer
	n      int64 // bytes written
	err    error // the first write error
	placed bool
}

func (s *Store) createTemp(pattern string) (*tempFile, error) {
	f, err := os.CreateTemp(s.tmpDir(), pattern)
	if err != nil {
		return nil, fmt.Errorf("failed to create a temporary file: %w", err)
	}
	return &tempFile{f: f, buf: bufio.NewWriterSize(f, chunkSize)}, nil
}

func (t *tempFile) Write(p []byte) (int, error) {
	n, err := t.buf.Write(p)
	t.n += int64(n)
	if err != nil && t.err == nil {
		t.err = err
	}
	return n, err
}

// finish flushes, syncs and closes the file.
func (t *tempFile) finish() error {
	if err := t.buf.Flush(); err != nil {
		return err
	}
	if err := syncFile(t.f); err != nil {
		return err
	}
	return t.f.Close()
}

// discard closes and removes the file unless it was placed.
func (t *tempFile) discard() {
	_ = t.f.Close()
	if !t.placed {
		_ = os.Remove(t.f.Name())
	}
}
