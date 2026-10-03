package store

import (
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"io"
	"math/rand/v2"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"testing"
	"time"
)

func sha(b []byte) string {
	sum := sha256.Sum256(b)
	return hex.EncodeToString(sum[:])
}

func randomBytes(seed uint64, n int) []byte {
	r := rand.New(rand.NewPCG(seed, seed^0x9e3779b97f4a7c15))
	b := make([]byte, n)
	for i := range b {
		b[i] = byte(r.Uint32())
	}
	return b
}

func readBlob(t *testing.T, s *Store, hash string) []byte {
	t.Helper()
	rc, err := s.OpenBlob(hash)
	if err != nil {
		t.Fatalf("OpenBlob(%s) failed: %v", hash, err)
	}
	defer rc.Close()
	b, err := io.ReadAll(rc)
	if err != nil {
		t.Fatalf("reading blob %s: %v", hash, err)
	}
	return b
}

func blobFiles(t *testing.T, s *Store) []string {
	t.Helper()
	var names []string
	err := filepath.WalkDir(s.blobRoot(), func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() {
			names = append(names, d.Name())
		}
		return nil
	})
	if err != nil {
		t.Fatalf("walking the blobs: %v", err)
	}
	return names
}

func TestPutBlobCompressesTextAndKeepsRandomBytesRaw(t *testing.T) {
	s := openTestStore(t)
	text := []byte(strings.Repeat("<tr><td>Grundlagen der Informatik</td><td>6 LP</td></tr>\n", 2000))
	random := randomBytes(1, 200<<10)

	tb, err := s.PutBlob(bytes.NewReader(text), sha(text), 0)
	if err != nil {
		t.Fatalf("PutBlob(text) failed: %v", err)
	}
	if !tb.Gzip || tb.Hash != sha(text) || tb.Size != int64(len(text)) || tb.Stored >= tb.Size/10 {
		t.Fatalf("text blob = %+v, want gzip, much smaller than %d", tb, len(text))
	}
	rb, err := s.PutBlob(bytes.NewReader(random), "", 0)
	if err != nil {
		t.Fatalf("PutBlob(random) failed: %v", err)
	}
	if rb.Gzip || rb.Stored != int64(len(random)) || rb.Size != int64(len(random)) {
		t.Fatalf("random blob = %+v, want raw", rb)
	}
	eb := putBlob(t, s, "")
	if eb.Gzip || eb.Size != 0 || eb.Hash != sha(nil) {
		t.Fatalf("empty blob = %+v, want raw and empty", eb)
	}

	// Round trips: the original bytes whatever the form.
	if got := readBlob(t, s, tb.Hash); !bytes.Equal(got, text) {
		t.Fatalf("text blob reads back %d bytes, want the original %d", len(got), len(text))
	}
	if got := readBlob(t, s, rb.Hash); !bytes.Equal(got, random) {
		t.Fatalf("random blob does not read back")
	}
	if got := readBlob(t, s, eb.Hash); len(got) != 0 {
		t.Fatalf("empty blob reads back %d bytes", len(got))
	}
	if _, err := os.Stat(s.blobPath(tb.Hash, true)); err != nil {
		t.Fatalf("text blob not at <hh>/<hex>.gz: %v", err)
	}
	if _, err := os.Stat(s.blobPath(rb.Hash, false)); err != nil {
		t.Fatalf("random blob not at <hh>/<hex>: %v", err)
	}

	// OpenRaw only for a raw blob; OpenStored gives the bytes on disk.
	if _, err := s.OpenRaw(tb.Hash); !errors.Is(err, ErrCompressed) {
		t.Fatalf("OpenRaw(compressed) err %v, want ErrCompressed", err)
	}
	f, err := s.OpenRaw(rb.Hash)
	if err != nil {
		t.Fatalf("OpenRaw(raw) failed: %v", err)
	}
	part := make([]byte, 100)
	if _, err := f.ReadAt(part, 1000); err != nil || !bytes.Equal(part, random[1000:1100]) {
		t.Fatalf("OpenRaw is not seekable to the original bytes (err %v)", err)
	}
	_ = f.Close()
	rc, gz, stored, err := s.OpenStored(tb.Hash)
	if err != nil || !gz || stored != tb.Stored {
		t.Fatalf("OpenStored = gzip %v, %d bytes (err %v), want gzip, %d", gz, stored, err, tb.Stored)
	}
	zr, err := gzip.NewReader(rc)
	if err != nil {
		t.Fatalf("stored form is not gzip: %v", err)
	}
	if got, _ := io.ReadAll(zr); !bytes.Equal(got, text) {
		t.Fatalf("stored form does not decompress to the original")
	}
	_ = rc.Close()

	// StatBlob knows the original size of a compressed blob that no row names (gzip's trailer).
	info, err := s.StatBlob(tb.Hash)
	if err != nil || info != tb {
		t.Fatalf("StatBlob = %+v (err %v), want %+v", info, err, tb)
	}

	missing := sha([]byte("missing"))
	if s.HasBlob(missing) || !s.HasBlob(tb.Hash) || !s.HasBlob(rb.Hash) {
		t.Fatalf("HasBlob is wrong")
	}
	if _, err := s.OpenBlob(missing); !errors.Is(err, ErrNotFound) {
		t.Fatalf("OpenBlob(missing) err %v, want ErrNotFound", err)
	}
	if _, _, _, err := s.OpenStored(missing); !errors.Is(err, ErrNotFound) {
		t.Fatalf("OpenStored(missing) err %v, want ErrNotFound", err)
	}
	if _, err := s.OpenBlob("../../index.db"); !errors.Is(err, ErrInvalidHash) {
		t.Fatalf("OpenBlob(path) err %v, want ErrInvalidHash", err)
	}
	if names := tmpFiles(t, s); len(names) != 0 {
		t.Fatalf("tmp keeps %v", names)
	}
}

func TestPutBlobKeepsGzipOnlyWhenItSavesOnePercent(t *testing.T) {
	if !smallEnough(99, 100) || smallEnough(100, 100) || smallEnough(991, 1000) || !smallEnough(990, 1000) {
		t.Fatalf("smallEnough does not apply the 1 %% rule")
	}
	s := openTestStore(t)
	// Random bytes with a compressible tail of about 0.5 %: gzip saves a little, not enough.
	body := append(randomBytes(2, 400<<10), bytes.Repeat([]byte{'a'}, 2<<10)...)
	b, err := s.PutBlob(bytes.NewReader(body), "", 0)
	if err != nil {
		t.Fatalf("PutBlob failed: %v", err)
	}
	if b.Gzip {
		t.Fatalf("blob = %+v, want raw: gzip saves less than 1 %%", b)
	}
	// With a tail of about 5 % it does.
	body = append(randomBytes(3, 400<<10), bytes.Repeat([]byte{'a'}, 20<<10)...)
	if b, err = s.PutBlob(bytes.NewReader(body), "", 0); err != nil || !b.Gzip {
		t.Fatalf("blob = %+v (err %v), want gzip", b, err)
	}
}

func TestPutBlobStopsCompressingAStreamThatDoesNotShrink(t *testing.T) {
	s := openTestStore(t)
	defer func(n int64) { probeBytes = n }(probeBytes)
	probeBytes = 1 << 20

	random := randomBytes(4, 3<<20)
	b, err := s.PutBlob(bytes.NewReader(random), "", 0)
	if err != nil || b.Gzip || b.Size != int64(len(random)) {
		t.Fatalf("PutBlob(random) = %+v (err %v), want raw", b, err)
	}
	// Text passes every probe and is compressed to its end.
	text := []byte(strings.Repeat("Modulhandbuch Informatik, Pflichtmodul im 1. Semester. ", 80000))
	b, err = s.PutBlob(bytes.NewReader(text), "", 0)
	if err != nil || !b.Gzip {
		t.Fatalf("PutBlob(text) = %+v (err %v), want gzip", b, err)
	}
	if got := readBlob(t, s, b.Hash); !bytes.Equal(got, text) {
		t.Fatalf("text does not read back after the probes' flushes")
	}
}

// patternReader produces n bytes of compressible text without holding them.
type patternReader struct {
	n, off int64
	hash   interface{ Write([]byte) (int, error) }
}

const pattern = "Lehrveranstaltung 12345 Vorlesung Mo 09:15-10:45 Raum HG 1.23, Prof. Dr. Beispiel; "

func (p *patternReader) Read(b []byte) (int, error) {
	if p.off >= p.n {
		return 0, io.EOF
	}
	n := 0
	for n < len(b) && p.off < p.n {
		c := pattern[p.off%int64(len(pattern))]
		if p.off%4099 == 0 {
			c = byte('0' + p.off%10)
		}
		b[n] = c
		n++
		p.off++
	}
	p.hash.Write(b[:n])
	return n, nil
}

func TestPutBlobStreamsAMultiMegabyteBodyInBoundedMemory(t *testing.T) {
	s := openTestStore(t)
	const size = 24 << 20
	h := sha256.New()
	in := &patternReader{n: size, hash: h}

	var before, after runtime.MemStats
	runtime.GC()
	runtime.ReadMemStats(&before)
	b, err := s.PutBlob(in, "", 0)
	runtime.ReadMemStats(&after)
	if err != nil {
		t.Fatalf("PutBlob failed: %v", err)
	}
	if alloc := after.TotalAlloc - before.TotalAlloc; alloc > 8<<20 {
		t.Fatalf("PutBlob of %d MB allocated %d MB, want it streamed", size>>20, alloc>>20)
	}
	if b.Size != size || b.Hash != hex.EncodeToString(h.Sum(nil)) || !b.Gzip {
		t.Fatalf("blob = %+v, want %d bytes, compressed, the stream's hash", b, size)
	}

	rc, err := s.OpenBlob(b.Hash)
	if err != nil {
		t.Fatalf("OpenBlob failed: %v", err)
	}
	defer rc.Close()
	back := sha256.New()
	runtime.ReadMemStats(&before)
	n, err := io.Copy(back, rc)
	runtime.ReadMemStats(&after)
	if err != nil || n != size || hex.EncodeToString(back.Sum(nil)) != b.Hash {
		t.Fatalf("reading back: %d bytes (err %v), want %d with the same hash", n, err, size)
	}
	if alloc := after.TotalAlloc - before.TotalAlloc; alloc > 8<<20 {
		t.Fatalf("OpenBlob of %d MB allocated %d MB, want it streamed", size>>20, alloc>>20)
	}
}

func TestPutBlobLeavesNothingBehindOnAnError(t *testing.T) {
	s := openTestStore(t)
	body := []byte(strings.Repeat("Prüfungsordnung ", 1000))

	_, err := s.PutBlob(bytes.NewReader(body), sha([]byte("something else")), 0)
	if !errors.Is(err, ErrHashMismatch) {
		t.Fatalf("PutBlob with a wrong expect: err %v, want ErrHashMismatch", err)
	}
	_, err = s.PutBlob(bytes.NewReader(body), "", int64(len(body)-1))
	if !errors.Is(err, ErrTooLarge) {
		t.Fatalf("PutBlob over maxBytes: err %v, want ErrTooLarge", err)
	}
	_, err = s.PutBlob(io.MultiReader(bytes.NewReader(body), &failingReader{}), "", 0)
	if err == nil || !strings.Contains(err.Error(), "connection reset") {
		t.Fatalf("PutBlob of a broken body: err %v, want the read error", err)
	}
	if _, err := s.PutBlob(bytes.NewReader(body), "SHA", 0); !errors.Is(err, ErrInvalidHash) {
		t.Fatalf("PutBlob with an invalid expect: err %v, want ErrInvalidHash", err)
	}
	if names := tmpFiles(t, s); len(names) != 0 {
		t.Fatalf("tmp keeps %v", names)
	}
	if names := blobFiles(t, s); len(names) != 0 {
		t.Fatalf("blobs after failed writes: %v", names)
	}
	if s.HasBlob(sha(body)) {
		t.Fatalf("the body was stored")
	}

	// Exactly maxBytes is allowed, and the right expect.
	b, err := s.PutBlob(bytes.NewReader(body), sha(body), int64(len(body)))
	if err != nil || b.Hash != sha(body) {
		t.Fatalf("PutBlob at maxBytes = %+v (err %v)", b, err)
	}
}

type failingReader struct{}

func (failingReader) Read([]byte) (int, error) {
	return 0, errors.New("read tcp: connection reset by peer")
}

func TestPutBlobOfAnExistingBlobRenewsItsTime(t *testing.T) {
	s := openTestStore(t)
	b := putBlob(t, s, "Satzung")
	path := s.blobPath(b.Hash, b.Gzip)
	old := time.Now().Add(-30 * 24 * time.Hour)
	if err := os.Chtimes(path, old, old); err != nil {
		t.Fatal(err)
	}
	again := putBlob(t, s, "Satzung")
	if again != b {
		t.Fatalf("second PutBlob = %+v, want %+v", again, b)
	}
	info, err := os.Stat(path)
	if err != nil || time.Since(info.ModTime()) > time.Hour {
		t.Fatalf("existing blob's time not renewed: %v (err %v)", info.ModTime(), err)
	}
	if names := blobFiles(t, s); len(names) != 1 {
		t.Fatalf("blob files %v, want one", names)
	}
}

func TestImportStoredChecksTheContent(t *testing.T) {
	leader := openTestStore(t)
	follower := openTestStore(t)
	text := []byte(strings.Repeat("Studienordnung Informatik ", 500))
	random := randomBytes(5, 64<<10)
	for _, body := range [][]byte{text, random} {
		b, err := leader.PutBlob(bytes.NewReader(body), "", 0)
		if err != nil {
			t.Fatal(err)
		}
		rc, gz, _, err := leader.OpenStored(b.Hash)
		if err != nil {
			t.Fatal(err)
		}
		if err := follower.ImportStored(b.Hash, rc, gz); err != nil {
			t.Fatalf("ImportStored(gzip %v) failed: %v", gz, err)
		}
		_ = rc.Close()
		if got := readBlob(t, follower, b.Hash); !bytes.Equal(got, body) {
			t.Fatalf("imported blob does not read back")
		}
		fb, err := follower.StatBlob(b.Hash)
		if err != nil || fb != b {
			t.Fatalf("imported blob = %+v (err %v), want the leader's form %+v", fb, err, b)
		}
		// Importing it again keeps the one there.
		rc, gz, _, _ = leader.OpenStored(b.Hash)
		if err := follower.ImportStored(b.Hash, rc, gz); err != nil {
			t.Fatalf("second ImportStored failed: %v", err)
		}
		_ = rc.Close()
	}

	other := sha([]byte("other"))
	if err := follower.ImportStored(other, bytes.NewReader(random), false); !errors.Is(err, ErrHashMismatch) {
		t.Fatalf("ImportStored of other content: err %v, want ErrHashMismatch", err)
	}
	if err := follower.ImportStored(sha(random), bytes.NewReader(random), true); !errors.Is(err, ErrHashMismatch) {
		t.Fatalf("ImportStored of raw bytes as gzip: err %v, want ErrHashMismatch", err)
	}
	var packed bytes.Buffer
	zw := gzip.NewWriter(&packed)
	_, _ = zw.Write([]byte("Prüfung"))
	_ = zw.Close()
	truncated := packed.Bytes()[:packed.Len()-6]
	if err := follower.ImportStored(sha([]byte("Prüfung")), bytes.NewReader(truncated), true); !errors.Is(err, ErrHashMismatch) {
		t.Fatalf("ImportStored of a truncated gzip stream: err %v, want ErrHashMismatch", err)
	}
	trailing := append(append([]byte{}, packed.Bytes()...), "junk"...)
	if err := follower.ImportStored(sha([]byte("Prüfung")), bytes.NewReader(trailing), true); !errors.Is(err, ErrHashMismatch) {
		t.Fatalf("ImportStored with junk after the gzip stream: err %v, want ErrHashMismatch", err)
	}
	// A connection that breaks is not bad content.
	broken := io.MultiReader(bytes.NewReader(packed.Bytes()[:10]), failingReader{})
	if err := follower.ImportStored(sha([]byte("Prüfung")), broken, true); err == nil || errors.Is(err, ErrHashMismatch) ||
		!strings.Contains(err.Error(), "connection reset") {
		t.Fatalf("ImportStored over a broken connection: err %v, want the read error", err)
	}
	if follower.HasBlob(other) || follower.HasBlob(sha([]byte("Prüfung"))) {
		t.Fatalf("a refused import was stored")
	}
	if names := tmpFiles(t, follower); len(names) != 0 {
		t.Fatalf("tmp keeps %v", names)
	}
}

func TestGCBlobsRespectsReferencesAndGrace(t *testing.T) {
	s := openTestStore(t)
	now := time.Now()
	old := now.Add(-8 * 24 * time.Hour)
	const grace = 7 * 24 * time.Hour

	current := fetched(t, s, "https://www.b-tu.de/modul/11101", 200, "current", t0)
	superseded := fetched(t, s, "https://www.b-tu.de/modul/11102", 200, "superseded", t0)
	record(t, s, current)
	record(t, s, superseded)
	newer := fetched(t, s, "https://www.b-tu.de/modul/11102", 200, "newer", t0.Add(time.Hour))
	record(t, s, newer)
	fileBlob := putBlob(t, s, "file content")
	if _, _, _, err := s.PutFile("models/a.onnx", fileBlob, "application/octet-stream", t0); err != nil {
		t.Fatal(err)
	}
	deletedBlob := putBlob(t, s, "deleted file content")
	if _, _, _, err := s.PutFile("models/b.onnx", deletedBlob, "application/octet-stream", t0); err != nil {
		t.Fatal(err)
	}
	if _, err := s.DeleteFile("models/b.onnx", t0.Add(time.Hour)); err != nil {
		t.Fatal(err)
	}
	orphanOld := putBlob(t, s, "orphan, old")
	orphanYoung := putBlob(t, s, "orphan, young")
	reused := putBlob(t, s, "orphan, used again")

	for _, b := range []string{current.Hash, superseded.Hash, newer.Hash, fileBlob.Hash, deletedBlob.Hash, orphanOld.Hash, reused.Hash} {
		if err := os.Chtimes(s.blobPath(b, false), old, old); err != nil {
			t.Fatal(err)
		}
	}
	putBlob(t, s, "orphan, used again") // a fetch that returned it again, before its RecordFetch

	removed, bytes, err := s.GCBlobs(grace, now)
	if err != nil {
		t.Fatalf("GCBlobs failed: %v", err)
	}
	if removed != 1 || bytes != orphanOld.Stored {
		t.Fatalf("GCBlobs removed %d blobs, %d bytes, want 1, %d", removed, bytes, orphanOld.Stored)
	}
	if s.HasBlob(orphanOld.Hash) {
		t.Fatalf("an old unreferenced blob is kept")
	}
	for name, h := range map[string]string{"current": current.Hash, "superseded": superseded.Hash, "newer": newer.Hash,
		"file": fileBlob.Hash, "deleted file's version": deletedBlob.Hash, "young": orphanYoung.Hash, "reused": reused.Hash} {
		if !s.HasBlob(h) {
			t.Errorf("GCBlobs removed the %s blob", name)
		}
	}

	// Once retention dropped the superseded version and the deleted file, their blobs go too.
	if _, _, err := s.Prune(t0.Add(181*24*time.Hour+time.Hour), 180*24*time.Hour); err != nil {
		t.Fatal(err)
	}
	removed, _, err = s.GCBlobs(grace, now)
	if err != nil || removed != 2 || s.HasBlob(superseded.Hash) || s.HasBlob(deletedBlob.Hash) {
		t.Fatalf("GCBlobs after Prune removed %d (err %v), want the superseded and the deleted file's blob", removed, err)
	}
	if !s.HasBlob(current.Hash) || !s.HasBlob(newer.Hash) || !s.HasBlob(fileBlob.Hash) {
		t.Fatalf("GCBlobs after Prune removed a current blob")
	}
}

// ageBlob sets the time of the blob hash back by d, in whichever form it is stored.
func ageBlob(t *testing.T, s *Store, hash string, d time.Duration) {
	t.Helper()
	old := time.Now().Add(-d)
	for _, gz := range []bool{false, true} {
		if err := os.Chtimes(s.blobPath(hash, gz), old, old); err != nil && !errors.Is(err, os.ErrNotExist) {
			t.Fatal(err)
		}
	}
}

// blobAge is how long ago the blob hash was written or renewed.
func blobAge(t *testing.T, s *Store, hash string) time.Duration {
	t.Helper()
	for _, gz := range []bool{false, true} {
		if info, err := os.Stat(s.blobPath(hash, gz)); err == nil {
			return time.Since(info.ModTime())
		}
	}
	t.Fatalf("blob %s is not stored", hash)
	return 0
}

// gzipped compresses b as one gzip stream, quickly.
func gzipped(t *testing.T, b []byte) []byte {
	t.Helper()
	var out bytes.Buffer
	zw, _ := gzip.NewWriterLevel(&out, gzip.BestSpeed)
	if _, err := zw.Write(b); err != nil {
		t.Fatal(err)
	}
	if err := zw.Close(); err != nil {
		t.Fatal(err)
	}
	return out.Bytes()
}

// Review 1 (gc-vs-apply-dangling-blob): a write that references a blob checks under blobMu that
// it is there and renews its time before it commits, so that GCBlobs leaves it for another grace
// period; a blob that is not there is ErrBlobMissing, and nothing is written.
func TestWritesRefuseAMissingBlobAndRenewAPresentOne(t *testing.T) {
	s := openTestStore(t)
	missing := sha([]byte("never stored"))
	key, u, host, _ := Canonical("https://www.b-tu.de/modul/1", "", "")
	f := Fetched{Key: key, URL: u, Host: host, Source: "module_page", Status: 200, Hash: missing, Size: 12, At: t0}
	if _, _, _, _, err := s.RecordFetch(f); !errors.Is(err, ErrBlobMissing) {
		t.Errorf("RecordFetch of a missing blob: err %v, want ErrBlobMissing", err)
	}
	if _, _, _, err := s.PutFile("a", BlobInfo{Hash: missing, Size: 12}, "text/plain", t0); !errors.Is(err, ErrBlobMissing) {
		t.Errorf("PutFile of a missing blob: err %v, want ErrBlobMissing", err)
	}
	if seq, _ := s.Position(); seq != 0 || countRows(t, s, "entry") != 0 || countRows(t, s, "file") != 0 {
		t.Fatalf("a write of a missing blob changed the index (seq %d)", seq)
	}

	const month = 30 * 24 * time.Hour
	b := putBlob(t, s, "Prüfungsordnung")
	ageBlob(t, s, b.Hash, month)
	f.Hash, f.Size = b.Hash, b.Size
	record(t, s, f)
	if age := blobAge(t, s, b.Hash); age > time.Hour {
		t.Errorf("RecordFetch left the blob %s old", age)
	}
	ageBlob(t, s, b.Hash, month)
	if _, _, _, err := s.PutFile("a", b, "text/plain", t0); err != nil {
		t.Fatal(err)
	}
	if age := blobAge(t, s, b.Hash); age > time.Hour {
		t.Errorf("PutFile left the blob %s old", age)
	}

	// A follower: the entry names a blob it does not have yet.
	follower := openTestStore(t)
	entries, err := s.JournalAfter(0, 0)
	if err != nil || len(entries) != 2 {
		t.Fatalf("JournalAfter = %d entries (err %v)", len(entries), err)
	}
	if err := follower.Apply(entries[0]); !errors.Is(err, ErrBlobMissing) {
		t.Fatalf("Apply without the blob: err %v, want ErrBlobMissing", err)
	}
	if seq, _ := follower.Position(); seq != 0 || countRows(t, follower, "entry") != 0 || countRows(t, follower, "version") != 0 {
		t.Fatalf("Apply without the blob changed the index (seq %d)", seq)
	}
	if err := copyBlob(s, follower, entries[0].Blob); err != nil {
		t.Fatal(err)
	}
	ageBlob(t, follower, b.Hash, month)
	for _, e := range entries {
		if err := follower.Apply(e); err != nil {
			t.Fatalf("Apply(%d) with the blob failed: %v", e.Seq, err)
		}
	}
	if age := blobAge(t, follower, b.Hash); age > time.Hour {
		t.Errorf("Apply left the blob %s old", age)
	}
	sameDump(t, "after the blob came", dump(t, follower), dump(t, s))
}

// Review 1 (gc-durability-order): with WAL and synchronous=NORMAL a commit is durable only once
// the WAL is synced, so GC removed a blob (durably) on a reference whose removal a power loss
// could still undo. The index is made durable before the first removal now.
func TestGCMakesTheIndexDurableBeforeItRemovesABlob(t *testing.T) {
	s := openTestStore(t)
	f := fetched(t, s, "https://www.b-tu.de/modul/11101", 200, "the page", t0)
	record(t, s, f)
	ageBlob(t, s, f.Hash, 8*24*time.Hour)
	if _, err := s.DeleteEntry(f.Key); err != nil { // the reference goes, in the WAL only
		t.Fatal(err)
	}

	var synced []string // the files synced while the blob was still there
	saved := syncFile
	t.Cleanup(func() { syncFile = saved })
	syncFile = func(file *os.File) error {
		if s.HasBlob(f.Hash) {
			synced = append(synced, filepath.Base(file.Name()))
		}
		return saved(file)
	}
	removed, _, err := s.GCBlobs(7*24*time.Hour, time.Now())
	if err != nil || removed != 1 || s.HasBlob(f.Hash) {
		t.Fatalf("GCBlobs removed %d (err %v), want the unreferenced blob", removed, err)
	}
	if !slices.Contains(synced, "index.db-wal") {
		t.Fatalf("GCBlobs removed a blob before it synced the index; it synced %v first", synced)
	}
}

// Review 1 (statblob-isize-mod-4gib): the original size of a compressed blob that no row names
// was taken from gzip's trailer, which counts modulo 4 GiB, and served as Content-Length. It is
// used now only where the stream is too short to give 4 GiB; otherwise the size is unknown (-1).
func TestStatBlobDoesNotGuessTheSizeOfALargeCompressedBlob(t *testing.T) {
	s := openTestStore(t)
	content := randomBytes(9, 5<<20)
	stored := gzipped(t, content) // more than 4 GiB / 1032 bytes: deflate could make 4 GiB of them
	hash := sha(content)
	if err := s.ImportStored(hash, bytes.NewReader(stored), true); err != nil {
		t.Fatal(err)
	}
	info, err := s.StatBlob(hash)
	if want := (BlobInfo{Hash: hash, Size: -1, Stored: int64(len(stored)), Gzip: true}); err != nil || info != want {
		t.Fatalf("StatBlob of a large unreferenced gzip blob = %+v (err %v), want %+v", info, err, want)
	}
	// Once a row names it, the index knows its size.
	if _, _, _, err := s.PutFile("big", BlobInfo{Hash: hash, Size: int64(len(content))}, "application/octet-stream", t0); err != nil {
		t.Fatal(err)
	}
	if info, err := s.StatBlob(hash); err != nil || info.Size != int64(len(content)) {
		t.Fatalf("StatBlob of a referenced blob = %+v (err %v), want size %d", info, err, len(content))
	}
	// A short stream's trailer is exact.
	small := []byte(strings.Repeat("Studienordnung ", 1000))
	smallHash := sha(small)
	if err := s.ImportStored(smallHash, bytes.NewReader(gzipped(t, small)), true); err != nil {
		t.Fatal(err)
	}
	if info, err := s.StatBlob(smallHash); err != nil || info.Size != int64(len(small)) {
		t.Fatalf("StatBlob of a short gzip blob = %+v (err %v), want size %d", info, err, len(small))
	}
}

// The size rule above holds for one gzip stream; a second one after it (its own trailer) is not
// a blob the store wrote.
func TestImportStoredTakesOneGzipStreamOnly(t *testing.T) {
	s := openTestStore(t)
	two := append(gzipped(t, []byte("Prüfung ")), gzipped(t, []byte("bestanden"))...)
	hash := sha([]byte("Prüfung bestanden"))
	if err := s.ImportStored(hash, bytes.NewReader(two), true); !errors.Is(err, ErrHashMismatch) {
		t.Fatalf("ImportStored of two gzip streams: err %v, want ErrHashMismatch", err)
	}
	if s.HasBlob(hash) || len(tmpFiles(t, s)) != 0 {
		t.Fatalf("a refused import left files behind")
	}
}
