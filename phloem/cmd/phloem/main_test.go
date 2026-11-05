package main

import (
	"bytes"
	"context"
	"encoding/json"
	"log/slog"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/leonieziechmann/betula/phloem/internal/version"
)

// capture replaces stdout and stderr for one command.
func capture(t *testing.T) (out, errOut *bytes.Buffer) {
	t.Helper()
	out, errOut = new(bytes.Buffer), new(bytes.Buffer)
	oldOut, oldErr := stdout, stderr
	stdout, stderr = out, errOut
	t.Cleanup(func() { stdout, stderr = oldOut, oldErr })
	return out, errOut
}

func TestMain(m *testing.M) {
	setKeepRoot()
	os.Exit(m.Run())
}

func TestHelpVersionAndUnknownCommands(t *testing.T) {
	out, _ := capture(t)
	if code := run([]string{"help"}); code != 0 || !strings.Contains(out.String(), "probe") || !strings.Contains(out.String(), version.Version) {
		t.Errorf("help: %d %s", code, out)
	}
	out.Reset()
	if code := run([]string{"version"}); code != 0 || !strings.HasPrefix(out.String(), "Phloem "+version.Version) {
		t.Errorf("version: %d %s", code, out)
	}
	_, errOut := capture(t)
	if code := run([]string{"dance"}); code != 2 || !strings.Contains(errOut.String(), "Unknown command") {
		t.Errorf("unknown: %d %s", code, errOut)
	}
	if code := run([]string{"serve", "--no-such-flag"}); code != 2 {
		t.Errorf("a wrong flag: %d", code)
	}
	if code := run([]string{"serve", "extra"}); code != 2 {
		t.Errorf("an argument: %d", code)
	}
	if code := run([]string{"serve", "--help"}); code != 0 {
		t.Errorf("serve --help: %d", code)
	}
}

func freeAddr(t *testing.T) string {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer ln.Close()
	return ln.Addr().String()
}

// The one test that runs serve: its metrics live in the process's registry, once.
func TestServeHealthcheckAndStatus(t *testing.T) {
	data, stats := t.TempDir(), t.TempDir()
	addr := freeAddr(t)
	capture(t)
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan int, 1)
	go func() {
		done <- runServe(ctx, []string{"--addr", addr, "--data", data, "--stats-dir", stats, "--stats-interval", "50ms",
			"--stack", "phloem-test", "--origins", "https://betula.app", "--budget", "1000000", "--log-format", "text"})
	}()
	deadline := time.Now().Add(10 * time.Second)
	for {
		resp, err := http.Get("http://" + addr + "/healthz")
		if err == nil {
			resp.Body.Close()
			if resp.StatusCode == http.StatusOK {
				break
			}
		}
		if time.Now().After(deadline) {
			t.Fatalf("serve did not come up: %v", err)
		}
		time.Sleep(20 * time.Millisecond)
	}
	out, errOut := capture(t)
	if code := runHealthcheck(ctx, []string{"--url", "http://" + addr + "/healthz"}); code != 0 || strings.TrimSpace(out.String()) != "ok" {
		t.Errorf("healthcheck: %d %q %s", code, out, errOut)
	}
	out.Reset()
	if code := runStatus(ctx, []string{"--url", "http://" + addr + "/status"}); code != 0 {
		t.Fatalf("status: %d %s", code, errOut)
	}
	var st struct {
		Version string `json:"version"`
		Budget  int64  `json:"budget"`
		Stats   string `json:"stats_dir"`
	}
	if err := json.Unmarshal(out.Bytes(), &st); err != nil || st.Version != version.Version || st.Budget != 1000000 || st.Stats != stats {
		t.Errorf("status %s: %v", out, err)
	}
	if code := runHealthcheck(ctx, []string{"--url", "http://" + addr + "/nothing"}); code != 1 {
		t.Errorf("healthcheck of a 404: %d", code)
	}
	if code := runStatus(ctx, []string{"--url", "http://" + freeAddr(t) + "/status"}); code != 1 {
		t.Errorf("status of nothing: %d", code)
	}
	// A problem in the log is counted in the stats file (serve installed the logger).
	slog.Warn("a test's warning", "component", "test", "event", "test.warned")
	prom := filepath.Join(stats, "phloem.prom")
	for deadline := time.Now().Add(5 * time.Second); ; {
		if data, err := os.ReadFile(prom); err == nil && strings.Contains(string(data), `phloem_up{stack="phloem-test"} 1`) {
			break
		}
		if time.Now().After(deadline) {
			t.Fatal("no stats file")
		}
		time.Sleep(20 * time.Millisecond)
	}
	cancel()
	select {
	case code := <-done:
		if code != 0 {
			t.Errorf("serve ended with %d", code)
		}
	case <-time.After(20 * time.Second):
		t.Fatal("serve did not stop")
	}
	if data, _ := os.ReadFile(prom); !strings.Contains(string(data), `phloem_up{stack="phloem-test"} 0`) ||
		!strings.Contains(string(data), `phloem_log_problems_total{stack="phloem-test",level="WARN",event="test.warned"} 1`) {
		t.Errorf("the last stats file:\n%s", data)
	}
}

func TestServeRefusesABadConfiguration(t *testing.T) {
	capture(t)
	for name, args := range map[string][]string{
		"origin":    {"--origins", "betula.app"},
		"budget":    {"--budget", "0"},
		"log level": {"--log-level", "loud"},
	} {
		args = append(args, "--addr", freeAddr(t), "--data", t.TempDir(), "--log-format", "text")
		if code := runServe(context.Background(), args); code != 2 {
			t.Errorf("%s: exit %d", name, code)
		}
	}
	if code := runServe(context.Background(), []string{"--addr", "256.0.0.1:1", "--data", t.TempDir(), "--log-format", "text"}); code != 1 {
		t.Errorf("an address that cannot listen: exit %d", code)
	}
}

func TestEnvironmentDefaults(t *testing.T) {
	t.Setenv("PHLOEM_ADDR", "0.0.0.0:8200")
	t.Setenv("PHLOEM_BUDGET_BYTES", strconv.Itoa(42))
	t.Setenv("PHLOEM_STATS_INTERVAL", "1m")
	fs, o := newServeFlags()
	if err := fs.Parse(nil); err != nil {
		t.Fatal(err)
	}
	if *o.addr != "0.0.0.0:8200" || *o.budget != 42 || *o.statsInterval != time.Minute || *o.data != "phloem-data" {
		t.Errorf("defaults %q %d %v %q", *o.addr, *o.budget, *o.statsInterval, *o.data)
	}
}
