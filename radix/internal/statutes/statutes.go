// Package statutes keeps local copies of the regulation PDFs (OPUS) that the
// study plan scan reads. The local path of a document is a pure function of the
// program name and the document URL, so nothing about downloads is stored in the
// database and a statutes directory can be shared between machines.
package statutes

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	cortexclient "github.com/leonieziechmann/betula/radix/internal/cortex/client"
)

// UserAgent is a plain bot identification. A desktop browser user agent makes the
// OPUS server answer with a JavaScript challenge instead of the PDF.
const UserAgent = "Betula-Radix/1.0 (+https://betula.app; info@betula.app)"

// Source names the downloads of regulations to Cortex (its metrics and logs).
const Source = "statute"

// ErrBotProtection means the server answered with a challenge page instead of a PDF.
// The challenge is not worked around; the document has to be fetched by hand.
var ErrBotProtection = errors.New("blocked by bot protection")

// LocalPath is where the PDF of docURL belongs: <dir>/<program>/<opus id>_<file>.pdf.
func LocalPath(dir, programName, docURL string) string {
	return filepath.Join(dir, slug(programName, "program"), fileName(docURL))
}

// Exists reports whether a non-empty local copy is present.
func Exists(path string) bool {
	fi, err := os.Stat(path)
	return err == nil && !fi.IsDir() && fi.Size() > 0
}

// Locate finds the local copy of docURL. A regulation is often shared by several
// programs (a subject and its dual variant) but stored once, so after the program's
// own directory every other program directory is searched; the OPUS number in the
// file name makes it unique.
func Locate(dir, programName, docURL string) (string, bool) {
	if own := LocalPath(dir, programName, docURL); Exists(own) {
		return own, true
	}
	matches, _ := filepath.Glob(filepath.Join(dir, "*", fileName(docURL)))
	for _, m := range matches {
		if Exists(m) {
			return m, true
		}
	}
	return "", false
}

// Download fetches docURL to its local path unless a copy exists (or force is set).
// It returns the path and whether the existing copy was used. client may be Cortex's
// (internal/cortex/client); its requests name the source "statute".
func Download(ctx context.Context, client *http.Client, dir, programName, docURL string, force bool) (string, bool, error) {
	if docURL == "" {
		return "", false, errors.New("empty document URL")
	}
	target := LocalPath(dir, programName, docURL)
	if existing, ok := Locate(dir, programName, docURL); ok && !force {
		return existing, true, nil
	}
	if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
		return "", false, err
	}
	if client == nil {
		client = http.DefaultClient
	}

	req, err := http.NewRequestWithContext(cortexclient.WithSource(ctx, Source), http.MethodGet, docURL, nil)
	if err != nil {
		return "", false, err
	}
	req.Header.Set("User-Agent", UserAgent)
	req.Header.Set("Accept", "application/pdf,application/octet-stream,*/*")

	resp, err := client.Do(req)
	if err != nil {
		return "", false, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		switch code := cortexclient.ErrorCode(resp); {
		case resp.StatusCode == http.StatusBadGateway && code == "wrong-type":
			// Cortex takes nothing but a PDF from OPUS (its expect_type): a challenge page
			// arrives as this error instead of as the page.
			return "", false, ErrBotProtection
		case code != "":
			return "", false, fmt.Errorf("unexpected status %d (Cortex: %s)", resp.StatusCode, code)
		}
		return "", false, fmt.Errorf("unexpected status %d", resp.StatusCode)
	}

	head := make([]byte, 4096)
	n, _ := io.ReadFull(resp.Body, head)
	head = head[:n]
	contentType := strings.ToLower(resp.Header.Get("Content-Type"))
	lowHead := strings.ToLower(string(head))
	if strings.Contains(contentType, "text/html") || strings.Contains(lowHead, "not a bot") ||
		strings.Contains(lowHead, "cloudflare") || strings.Contains(lowHead, "challenge-platform") {
		return "", false, ErrBotProtection
	}
	if !strings.HasPrefix(string(head), "%PDF-") {
		return "", false, fmt.Errorf("response is not a PDF (content type %q)", contentType)
	}

	tmp := target + ".tmp"
	out, err := os.Create(tmp)
	if err != nil {
		return "", false, err
	}
	_, err = out.Write(head)
	if err == nil {
		_, err = io.Copy(out, resp.Body)
	}
	if closeErr := out.Close(); err == nil {
		err = closeErr
	}
	if err != nil {
		_ = os.Remove(tmp)
		return "", false, err
	}
	_ = os.Remove(target)
	if err := os.Rename(tmp, target); err != nil {
		return "", false, err
	}
	return target, false, nil
}

// fileName keeps the OPUS document number in the name: different documents are often
// called the same („Satzungsaenderung.pdf").
func fileName(rawURL string) string {
	name, docID := "document.pdf", ""
	if parsed, err := url.Parse(rawURL); err == nil {
		parts := strings.Split(strings.Trim(parsed.Path, "/"), "/")
		if len(parts) > 0 && parts[len(parts)-1] != "" {
			name = parts[len(parts)-1]
		}
		for i := len(parts) - 2; i >= 0 && i >= len(parts)-4; i-- {
			if _, err := strconv.Atoi(parts[i]); err == nil {
				docID = parts[i]
				break
			}
		}
	} else {
		name = filepath.Base(rawURL)
	}
	if i := strings.IndexAny(name, "?#"); i >= 0 {
		name = name[:i]
	}
	if !strings.HasSuffix(strings.ToLower(name), ".pdf") {
		name += ".pdf"
	}
	if docID != "" && !strings.HasPrefix(name, docID+"_") {
		name = docID + "_" + name
	}
	return sanitize(name)
}

func sanitize(s string) string {
	for _, c := range []string{":", "*", "?", "\"", "<", ">", "|", "\\", "/"} {
		s = strings.ReplaceAll(s, c, "_")
	}
	return strings.TrimSpace(s)
}

func slug(s, fallback string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		s = fallback
	}
	for _, c := range []string{" ", "/", "\\", ":", "|", "*", "?", "\"", "<", ">"} {
		s = strings.ReplaceAll(s, c, "_")
	}
	return s
}
