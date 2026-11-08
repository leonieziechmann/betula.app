//go:build linux

package main

import (
	"context"
	"net/netip"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
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

func TestTurnFirewallComesFromServesSettings(t *testing.T) {
	set := func(env map[string]string) {
		for _, k := range []string{"PHLOEM_TURN_HOST", "PHLOEM_TURN_PORT", "PHLOEM_TURN_RELAY_PORTS", "PHLOEM_TURN_IP"} {
			t.Setenv(k, env[k])
		}
	}
	set(nil)
	if tf, err := turnFirewallFromEnv(); err != nil || tf.on() {
		t.Fatalf("without a host: %v %v", tf, err)
	}
	good := map[string]string{"PHLOEM_TURN_HOST": "betula.app", "PHLOEM_TURN_RELAY_PORTS": "49160-49223", "PHLOEM_TURN_IP": "88.198.1.2"}
	set(good)
	tf, err := turnFirewallFromEnv()
	if err != nil || tf.String() != "port 3478 relays 49160-49223 at 88.198.1.2" {
		t.Fatalf("got %v %v", tf, err)
	}
	if got := tf.defines(); got != "define turn_port = 3478\ndefine relay_ports = 49160-49223\ndefine turn_ip = 88.198.1.2\n" {
		t.Fatalf("defines %q", got)
	}
	for name, change := range map[string][2]string{
		"port":          {"PHLOEM_TURN_PORT", "70000"},
		"port in range": {"PHLOEM_TURN_PORT", "49170"},
		"no relays":     {"PHLOEM_TURN_RELAY_PORTS", ""},
		"low relays":    {"PHLOEM_TURN_RELAY_PORTS", "80-90"},
		"backwards":     {"PHLOEM_TURN_RELAY_PORTS", "49223-49160"},
		"words":         {"PHLOEM_TURN_RELAY_PORTS", "49160-x; drop table"},
		"no ip":         {"PHLOEM_TURN_IP", ""},
		"ipv6":          {"PHLOEM_TURN_IP", "2001:db8::1"},
		"a name":        {"PHLOEM_TURN_IP", "betula.app"},
	} {
		env := map[string]string{}
		for k, v := range good {
			env[k] = v
		}
		env[change[0]] = change[1]
		set(env)
		if _, err := turnFirewallFromEnv(); err == nil {
			t.Errorf("%s: taken", name)
		}
	}
}

// TestTheFirewallFilesLoad checks firewall.nft alone and with turn.nft in nft's dry run, where
// nft can (as root, with nftables in the kernel); elsewhere it says why it cannot.
func TestTheFirewallFilesLoad(t *testing.T) {
	nft, err := exec.LookPath("nft")
	if err != nil || os.Geteuid() != 0 {
		t.Skip("needs nft and root")
	}
	base, _ := filepath.Abs("../../deploy/firewall.nft")
	turnRules, _ := filepath.Abs("../../deploy/turn.nft")
	tf := turnFirewall{port: 3478, relayMin: 49160, relayMax: 49223, ip: netip.MustParseAddr("88.198.1.2")}
	for name, script := range map[string]string{
		"alone":     "include " + strconv.Quote(base) + "\n",
		"with TURN": tf.defines() + "include " + strconv.Quote(base) + "\ninclude " + strconv.Quote(turnRules) + "\n",
	} {
		cmd := exec.Command(nft, "-c", "-f", "-")
		cmd.Stdin = strings.NewReader(script)
		if out, err := cmd.CombinedOutput(); err != nil {
			if strings.Contains(string(out), "Operation not permitted") {
				t.Skip("nft may not ask the kernel here")
			}
			t.Errorf("%s: %v: %s", name, err, out)
		}
	}
}
