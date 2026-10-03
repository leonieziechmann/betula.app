package client

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"hash"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"
)

// FileInfo describes one version of a stored file (PUT /v1/files/{name}).
type FileInfo struct {
	Name        string    `json:"name"`
	Version     int64     `json:"version"`
	SHA256      string    `json:"sha256"` // sha256:<hex> of the content
	Size        int64     `json:"size"`
	ContentType string    `json:"content_type"`
	CreatedAt   time.Time `json:"created_at"`
}

// PutOptions are the parameters of PutFile.
type PutOptions struct {
	ContentType string // kept with the version; "": application/octet-stream
	Expect      string // sha256:<hex>: Cortex refuses content with another hash
	IfMatch     string // sha256:<hex>: store only if the current version has this hash; "*": only if there is a current version
}

// PutFile stores body as the file name and returns the version that is current afterwards,
// and whether it is a new one (false: the content and its type were those of the current
// version already). A body that can be read again (a *bytes.Buffer, or one that can seek
// and read at an offset, as *os.File, *bytes.Reader and *strings.Reader) is sent to the next
// instance on a fail-over; any other body, a pipe for instance, to one instance only.
// A fail-over happens only when no instance can have stored it: one could not be connected
// to, or answered 503 no-leader. When the connection breaks after the request was sent, the
// error is ErrOutcomeUnknown: the file may have been stored (GetFile tells), and an answer
// to the same request sent again would mislead (412 for an If-Match that won, created
// false for the version this call made).
func (c *Client) PutFile(ctx context.Context, name string, body io.Reader, o PutOptions) (FileInfo, bool, error) {
	target, err := filePath(name)
	if err != nil {
		return FileInfo{}, false, err
	}
	if o.Expect != "" {
		target += "?expect=" + url.QueryEscape(o.Expect)
	}
	r := request{method: http.MethodPut, target: target, header: make(http.Header)}
	if o.ContentType != "" {
		r.header.Set("Content-Type", o.ContentType)
	}
	switch {
	case o.IfMatch == "*":
		r.header.Set("If-Match", "*")
	case o.IfMatch != "":
		r.header.Set("If-Match", `"`+o.IfMatch+`"`)
	}
	if r.body, r.once, err = bodyOf(body); err != nil {
		return FileInfo{}, false, err
	}

	resp, err := c.do(ctx, r)
	if err != nil {
		return FileInfo{}, false, err
	}
	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusCreated {
		return FileInfo{}, false, errorFrom(resp)
	}
	defer resp.Body.Close()
	var info FileInfo
	if err := json.NewDecoder(io.LimitReader(resp.Body, maxErrorBody)).Decode(&info); err != nil {
		return FileInfo{}, false, fmt.Errorf("cortex: the answer to PUT %s: %w", name, err)
	}
	return info, resp.StatusCode == http.StatusCreated, nil
}

// bodyOf returns how to get body for each attempt, and whether it can be sent once only.
func bodyOf(body io.Reader) (func() (io.ReadCloser, int64, error), bool, error) {
	switch b := body.(type) {
	case nil:
		return nil, false, nil
	case *bytes.Buffer:
		data := b.Bytes()
		return func() (io.ReadCloser, int64, error) {
			return io.NopCloser(bytes.NewReader(data)), int64(len(data)), nil
		}, false, nil
	case interface {
		io.ReaderAt
		io.Seeker
	}:
		// Every attempt reads its own section: an attempt that failed may still be reading
		// its copy while the next one starts.
		if start, err := b.Seek(0, io.SeekCurrent); err == nil {
			end, err := b.Seek(0, io.SeekEnd)
			if err != nil {
				return nil, false, err
			}
			if _, err := b.Seek(start, io.SeekStart); err != nil {
				return nil, false, err
			}
			return func() (io.ReadCloser, int64, error) {
				return io.NopCloser(io.NewSectionReader(b, start, end-start)), end - start, nil
			}, false, nil
		}
	}
	// A pipe or a stream: read as it comes, once. The caller closes what it gave.
	sent := false
	return func() (io.ReadCloser, int64, error) {
		if sent {
			return nil, 0, errors.New("cortex: the body was sent already and cannot be read again")
		}
		sent = true
		return io.NopCloser(body), -1, nil
	}, true, nil
}

// GetFileOptions pick the version GetFile reads.
type GetFileOptions struct {
	At      time.Time // the version that was current at that time; zero: the current one
	Version int64     // this version (FileInfo.Version); 0: by At, or the current one
}

// File is the content of a stored file and what its answer says about it.
type File struct {
	FileInfo // Size is -1 when the answer did not state it; CreatedAt is to the second
	// Body is the content; the caller closes it. Reading it to its end checks the content
	// against its hash: a difference is an error instead of io.EOF.
	Body io.ReadCloser
}

// GetFile reads the file name: the current version, or the one o picks. ErrNotFound when
// there is none, or it was deleted.
func (c *Client) GetFile(ctx context.Context, name string, o GetFileOptions) (*File, error) {
	target, err := filePath(name)
	if err != nil {
		return nil, err
	}
	switch {
	case o.Version != 0:
		target += "?version=" + strconv.FormatInt(o.Version, 10)
	case !o.At.IsZero():
		target += "?at=" + url.QueryEscape(o.At.UTC().Format(time.RFC3339Nano))
	}
	resp, err := c.do(ctx, request{method: http.MethodGet, target: target})
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != http.StatusOK {
		return nil, errorFrom(resp)
	}

	f := &File{FileInfo: FileInfo{Name: name, Size: resp.ContentLength, ContentType: resp.Header.Get("Content-Type")}, Body: resp.Body}
	f.Version, _ = strconv.ParseInt(resp.Header.Get("Cortex-Version"), 10, 64)
	if at, err := http.ParseTime(resp.Header.Get("Last-Modified")); err == nil {
		f.CreatedAt = at
	}
	if tag := strings.Trim(resp.Header.Get("ETag"), `"`); strings.HasPrefix(tag, "sha256:") {
		f.SHA256 = tag
		if want, err := hex.DecodeString(strings.TrimPrefix(tag, "sha256:")); err == nil && len(want) == sha256.Size {
			f.Body = &verified{ReadCloser: resp.Body, hash: sha256.New(), want: want, name: name}
		}
	}
	return f, nil
}

// verified hashes what is read and compares at the end.
type verified struct {
	io.ReadCloser
	hash hash.Hash
	want []byte
	name string
}

func (v *verified) Read(p []byte) (int, error) {
	n, err := v.ReadCloser.Read(p)
	v.hash.Write(p[:n])
	if err == io.EOF && !bytes.Equal(v.hash.Sum(nil), v.want) {
		return n, fmt.Errorf("cortex: the content of %s does not match its hash sha256:%x", v.name, v.want)
	}
	return n, err
}

// DeleteFile deletes the file name: its versions stay, the current one is a deletion.
// ErrNotFound when there is no such file, or it was deleted already. As with PutFile, a
// delete goes to another instance only when none can have carried it out, and a connection
// that breaks after it was sent gives ErrOutcomeUnknown rather than a 404 from a repeat.
func (c *Client) DeleteFile(ctx context.Context, name string) error {
	target, err := filePath(name)
	if err != nil {
		return err
	}
	resp, err := c.do(ctx, request{method: http.MethodDelete, target: target})
	if err != nil {
		return err
	}
	if resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusOK {
		return errorFrom(resp)
	}
	_ = resp.Body.Close()
	return nil
}

// filePath is the path of a file in the API. A name is 1 to 1024 bytes of UTF-8 without
// control characters, in segments separated by "/", none of them empty, "." or "..".
func filePath(name string) (string, error) {
	bad := func(why string) (string, error) {
		return "", fmt.Errorf("cortex: invalid file name %q: %s", name, why)
	}
	switch {
	case name == "":
		return bad("empty")
	case len(name) > 1024:
		return bad("longer than 1024 bytes")
	case !utf8.ValidString(name):
		return bad("not UTF-8")
	case strings.IndexFunc(name, unicode.IsControl) >= 0:
		return bad("a control character")
	}
	segments := strings.Split(name, "/")
	for i, s := range segments {
		if s == "" || s == "." || s == ".." {
			return bad(`an empty, "." or ".." segment`)
		}
		segments[i] = url.PathEscape(s)
	}
	return "/v1/files/" + strings.Join(segments, "/"), nil
}

// Status is what an instance says about itself (GET /status). Raw holds the whole answer,
// with what the fields here leave out (hosts, store, last prune) or could not read.
type Status struct {
	Instance  string          `json:"instance"`
	Role      string          `json:"role"` // leader or follower
	Epoch     int64           `json:"epoch"`
	Seq       int64           `json:"seq"`
	Leader    *LeaderInfo     `json:"leader"`
	Follower  *FollowerInfo   `json:"follower"`
	Version   string          `json:"version"`
	StartedAt time.Time       `json:"started_at"`
	Raw       json.RawMessage `json:"-"`
}

// LeaderInfo names the leader an instance knows.
type LeaderInfo struct {
	Instance string    `json:"instance"`
	URL      string    `json:"url"`
	Since    time.Time `json:"since"`
}

// FollowerInfo is how far a follower is behind its leader.
type FollowerInfo struct {
	LagSeconds float64 `json:"lag_seconds"`
	LagEntries int64   `json:"lag_entries"`
	State      string  `json:"state"`
}

// Status asks the instance that answers (the first one that can be reached) about itself.
func (c *Client) Status(ctx context.Context) (*Status, error) {
	resp, err := c.do(ctx, request{method: http.MethodGet, target: "/status"})
	if err != nil {
		return nil, err
	}
	if resp.StatusCode != http.StatusOK {
		return nil, errorFrom(resp)
	}
	defer resp.Body.Close()
	data, err := io.ReadAll(io.LimitReader(resp.Body, 16<<20))
	if err != nil {
		return nil, fmt.Errorf("cortex: reading the status: %w", err)
	}
	// A field of another type than these is left empty rather than failing the call: Raw
	// still has it.
	var st Status
	var typeErr *json.UnmarshalTypeError
	if err := json.Unmarshal(data, &st); err != nil && !errors.As(err, &typeErr) {
		return nil, fmt.Errorf("cortex: the status: %w", err)
	}
	st.Raw = data
	return &st, nil
}

// StepDown asks the instance that answers to give up the lead (POST /v1/admin/step-down);
// it stays a follower for 15 s, and the other instance takes over. ErrNotLeader when that
// instance is a follower.
func (c *Client) StepDown(ctx context.Context) error {
	resp, err := c.do(ctx, request{method: http.MethodPost, target: "/v1/admin/step-down"})
	if err != nil {
		return err
	}
	if resp.StatusCode < 200 || resp.StatusCode > 299 {
		return errorFrom(resp)
	}
	_, _ = io.Copy(io.Discard, io.LimitReader(resp.Body, maxErrorBody))
	_ = resp.Body.Close()
	return nil
}
