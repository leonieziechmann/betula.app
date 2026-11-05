//go:build linux

package main

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func setKeepRoot() { keepRootInTests = true }

func writeStatus(t *testing.T, body string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "status")
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

const unprivileged = `Name:	phloem
Umask:	0022
State:	S (sleeping)
Uid:	65532	65532	65532	65532
Gid:	65532	65532	65532	65532
Groups:
NoNewPrivs:	1
CapInh:	0000000000000000
CapPrm:	0000000000000000
CapEff:	0000000000000000
CapBnd:	00000000000030c0
CapAmb:	0000000000000000
`

func TestCheckUnprivileged(t *testing.T) {
	if err := checkUnprivileged(writeStatus(t, unprivileged), phloemID, phloemID); err != nil {
		t.Fatalf("the service as it should run: %v", err)
	}
	for name, change := range map[string][2]string{
		"root":          {"Uid:	65532	65532	65532	65532", "Uid:	0	0	0	0"},
		"saved root":    {"Uid:	65532	65532	65532	65532", "Uid:	65532	65532	0	65532"},
		"group root":    {"Gid:	65532	65532	65532	65532", "Gid:	0	65532	65532	65532"},
		"groups":        {"Groups:\n", "Groups:\t0 10\n"},
		"net admin":     {"CapEff:	0000000000000000", "CapEff:	0000000000001000"},
		"permitted":     {"CapPrm:	0000000000000000", "CapPrm:	00000000000000c0"},
		"ambient":       {"CapAmb:	0000000000000000", "CapAmb:	0000000000001000"},
		"inheritable":   {"CapInh:	0000000000000000", "CapInh:	0000000000001000"},
		"new privs":     {"NoNewPrivs:	1", "NoNewPrivs:	0"},
		"no cap fields": {"CapEff:	0000000000000000\n", ""},
	} {
		body := strings.Replace(unprivileged, change[0], change[1], 1)
		if body == unprivileged {
			t.Fatalf("%s: the change does not apply", name)
		}
		if err := checkUnprivileged(writeStatus(t, body), phloemID, phloemID); err == nil {
			t.Errorf("%s: taken as unprivileged", name)
		}
	}
	if err := checkUnprivileged(filepath.Join(t.TempDir(), "missing"), phloemID, phloemID); err == nil {
		t.Error("a missing file")
	}
}

func TestDNSQueryIsAQuestionForBetula(t *testing.T) {
	q := dnsQuery()
	if len(q) != 12+1+6+1+3+1+4 || q[5] != 1 || string(q[13:19]) != "betula" || string(q[20:23]) != "app" {
		t.Errorf("query % x", q)
	}
}

func TestProbeTakesOnlyAddresses(t *testing.T) {
	_, errOut := capture(t)
	if code := runProbe(context.Background(), []string{"traefik:8080"}); code != 2 || !strings.Contains(errOut.String(), "ip:port") {
		t.Errorf("a name: %d %s", code, errOut)
	}
}
