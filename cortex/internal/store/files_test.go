package store

import (
	"errors"
	"fmt"
	"testing"
	"time"
)

func putFile(t *testing.T, s *Store, name, body, contentType string, at time.Time) (FileVersion, bool, JournalEntry) {
	t.Helper()
	fv, created, e, err := s.PutFile(name, putBlob(t, s, body), contentType, at)
	if err != nil {
		t.Fatalf("PutFile(%s) failed: %v", name, err)
	}
	return fv, created, e
}

func TestFilesKeepTheirVersionsAndTombstones(t *testing.T) {
	s := openTestStore(t)
	const name = "models/multilingual-e5-small/model.onnx"
	t1, t2, t3, t4 := t0, t0.Add(time.Hour), t0.Add(2*time.Hour), t0.Add(3*time.Hour)

	v1, created, e := putFile(t, s, name, "weights 1", "application/octet-stream", t1)
	if !created || e.Op != OpFileVersion || e.Seq != 1 || e.Blob != v1.Hash || v1.Name != name || !v1.CreatedAt.Equal(t1) {
		t.Fatalf("first put: %+v, created %v, journal %+v", v1, created, e)
	}

	// The same content and type again: nothing changes, nothing is journaled.
	same, created, e := putFile(t, s, name, "weights 1", "application/octet-stream", t2)
	if created || same.ID != v1.ID || e.Seq != 0 || !same.CreatedAt.Equal(t1) {
		t.Fatalf("same content: %+v, created %v, journal %+v, want version %d unchanged and no entry", same, created, e, v1.ID)
	}
	if seq, _ := s.Position(); seq != 1 {
		t.Fatalf("Position = %d after an unchanged put, want 1", seq)
	}
	// Another content type is a new version.
	v2, created, _ := putFile(t, s, name, "weights 1", "application/x-onnx", t2)
	if !created || v2.ID == v1.ID {
		t.Fatalf("new content type: %+v, created %v", v2, created)
	}
	v3, created, _ := putFile(t, s, name, "weights 2", "application/x-onnx", t3)
	if !created || v3.Hash == v2.Hash {
		t.Fatalf("new content: %+v, created %v", v3, created)
	}
	if got, err := s.GetFile(name); err != nil || got.ID != v3.ID || !got.SupersededAt.IsZero() {
		t.Fatalf("GetFile = %+v (err %v), want version %d", got, err, v3.ID)
	}

	e, err := s.DeleteFile(name, t4)
	if err != nil || e.Op != OpFileVersion || e.Blob != "" {
		t.Fatalf("DeleteFile = %+v (err %v)", e, err)
	}
	if _, err := s.GetFile(name); !errors.Is(err, ErrNotFound) {
		t.Fatalf("GetFile of a deleted file: err %v, want ErrNotFound", err)
	}
	if _, err := s.DeleteFile(name, t4.Add(time.Hour)); !errors.Is(err, ErrNotFound) {
		t.Fatalf("DeleteFile of a deleted file: err %v, want ErrNotFound", err)
	}
	if _, err := s.DeleteFile("models/never", t4); !errors.Is(err, ErrNotFound) {
		t.Fatalf("DeleteFile of an unknown file: err %v, want ErrNotFound", err)
	}

	// The history stays readable by time and by id.
	for _, tc := range []struct {
		at   time.Time
		want int64
	}{
		{t1, v1.ID}, {t2.Add(-time.Microsecond), v1.ID}, {t2, v2.ID}, {t3, v3.ID}, {t4.Add(-time.Microsecond), v3.ID},
	} {
		got, err := s.GetFileAt(name, tc.at)
		if err != nil || got.ID != tc.want {
			t.Errorf("GetFileAt(%v) = %d (err %v), want %d", tc.at, got.ID, err, tc.want)
		}
	}
	for _, at := range []time.Time{t1.Add(-time.Microsecond), t4, t4.Add(time.Hour)} {
		if _, err := s.GetFileAt(name, at); !errors.Is(err, ErrNotFound) {
			t.Errorf("GetFileAt(%v): err %v, want ErrNotFound (before the first, or deleted)", at, err)
		}
	}
	if got, err := s.GetFileVersion(name, v2.ID); err != nil || got.Hash != v2.Hash || !got.SupersededAt.Equal(t3) {
		t.Fatalf("GetFileVersion(%d) = %+v (err %v)", v2.ID, got, err)
	}
	versions, err := s.FileVersions(name)
	if err != nil || len(versions) != 4 {
		t.Fatalf("FileVersions = %d (err %v), want 4", len(versions), err)
	}
	tomb := versions[0]
	if !tomb.Deleted || tomb.Hash != "" || tomb.Size != 0 || !tomb.CreatedAt.Equal(t4) || versions[3].ID != v1.ID {
		t.Fatalf("FileVersions not newest first with the tombstone on top: %+v", versions)
	}
	if _, err := s.GetFileVersion(name, tomb.ID); !errors.Is(err, ErrNotFound) {
		t.Fatalf("GetFileVersion(tombstone): err %v, want ErrNotFound", err)
	}
	if _, err := s.GetFileVersion("other", v2.ID); !errors.Is(err, ErrNotFound) {
		t.Fatalf("GetFileVersion of another name: err %v, want ErrNotFound", err)
	}
	if _, err := s.FileVersions("other"); !errors.Is(err, ErrNotFound) {
		t.Fatalf("FileVersions of an unknown file: err %v, want ErrNotFound", err)
	}

	// A put after the delete brings the file back as a new version of the same file.
	v5, created, _ := putFile(t, s, name, "weights 1", "application/octet-stream", t4.Add(time.Hour))
	if !created || v5.FileID != v1.FileID || v5.ID <= tomb.ID {
		t.Fatalf("put after delete: %+v, created %v", v5, created)
	}

	if _, _, _, err := s.PutFile("/bad", putBlob(t, s, "x"), "text/plain", t1); err == nil {
		t.Fatalf("PutFile with a bad name succeeded")
	}
	if _, _, _, err := s.PutFile("missing-blob", BlobInfo{Hash: sha([]byte("nope"))}, "text/plain", t1); err == nil {
		t.Fatalf("PutFile of a blob that is not stored succeeded")
	}
}

func TestListFilesByPrefixAndCursor(t *testing.T) {
	s := openTestStore(t)
	names := []string{"models/a", "models/b", "models/c/d", "models0", "other/x", "modelz", "models/ü"}
	for i, n := range names {
		putFile(t, s, n, fmt.Sprint("content ", i), "text/plain", t0)
	}
	if _, err := s.DeleteFile("models/b", t0.Add(time.Hour)); err != nil {
		t.Fatal(err)
	}

	var got []string
	cursor := ""
	for {
		page, next, err := s.ListFiles("models/", cursor, 2)
		if err != nil {
			t.Fatalf("ListFiles failed: %v", err)
		}
		for _, fv := range page {
			got = append(got, fv.Name)
		}
		if next == "" {
			break
		}
		cursor = next
	}
	want := []string{"models/a", "models/c/d", "models/ü"}
	if fmt.Sprint(got) != fmt.Sprint(want) {
		t.Fatalf("ListFiles(models/) = %v, want %v", got, want)
	}
	all, next, err := s.ListFiles("", "", 0)
	if err != nil || len(all) != 6 || next != "" {
		t.Fatalf("ListFiles() = %d files, next %q (err %v), want 6", len(all), next, err)
	}
	if _, _, err := s.ListFiles("", "!!", 0); !errors.Is(err, ErrInvalidCursor) {
		t.Fatalf("bad cursor: err %v", err)
	}
	if prefixEnd("ab") != "ac" || prefixEnd("a\xff") != "b" || prefixEnd("\xff") != "" || prefixEnd("") != "" {
		t.Fatalf("prefixEnd is wrong")
	}
}

// Review 1 (non-utf8-journal-divergence): a content type is text the follower must write as the
// leader did, and a header value when the file is served.
func TestPutFileRefusesAContentTypeThatIsNotText(t *testing.T) {
	s := openTestStore(t)
	b := putBlob(t, s, "body")
	for _, ct := range []string{"text/plain; name=\xfc", "text/plain\x00", "text/plain\r\nX-Other: 1", "text/plain\x7f"} {
		if _, _, _, err := s.PutFile("a", b, ct, t0); !errors.Is(err, ErrInvalidInput) {
			t.Errorf("PutFile with content type %q: err %v, want ErrInvalidInput", ct, err)
		}
	}
	if _, _, _, err := s.PutFile("/a", b, "text/plain", t0); !errors.Is(err, ErrInvalidInput) {
		t.Errorf("PutFile with an invalid name: err %v, want ErrInvalidInput", err)
	}
	if seq, _ := s.Position(); seq != 0 {
		t.Fatalf("refused puts moved the journal to %d", seq)
	}
	for i, ct := range []string{"text/plain; name=\"Müller.txt\"", "text/plain;\tcharset=utf-8"} {
		if _, _, _, err := s.PutFile(fmt.Sprint("ok/", i), b, ct, t0); err != nil {
			t.Errorf("PutFile with content type %q failed: %v", ct, err)
		}
	}
}
