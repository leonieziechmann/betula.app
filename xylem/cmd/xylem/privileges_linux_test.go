//go:build linux

package main

import (
	"context"
	"net/netip"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
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

const unprivileged = `Name:	xylem
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
	if err := checkUnprivileged(writeStatus(t, unprivileged), xylemID, xylemID); err != nil {
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
		if err := checkUnprivileged(writeStatus(t, body), xylemID, xylemID); err == nil {
			t.Errorf("%s: taken as unprivileged", name)
		}
	}
	if err := checkUnprivileged(filepath.Join(t.TempDir(), "missing"), xylemID, xylemID); err == nil {
		t.Error("a missing file")
	}
}

// What nft lists of firewall.nft (nftables 1.1), and what must not pass for it.
const listed = `table inet xylem {
	set special {
		type ipv4_addr
		flags interval
		elements = { 0.0.0.0/8, 10.0.0.0/8 }
	}

	chain input {
		type filter hook input priority filter; policy drop;
		tcp dport 8300 ct state new accept
		counter packets 0 bytes 0 drop comment "a new connection to another port, or anything else"
	}

	chain output {
		type filter hook output priority filter; policy drop;
		meta nfproto ipv6 counter packets 0 bytes 0 drop comment "IPv6: none, either way"
		ct state established,related accept
		oif "lo" ip saddr 127.0.0.1 ip daddr 127.0.0.1 accept
		ip daddr 127.0.0.11 meta l4proto { tcp, udp } ct original proto-dst 53 accept comment "a name asked of Docker's resolver: a push service's, the site's"
		meta skuid 0 ip daddr 192.0.2.1 udp dport 9 drop comment "xylem start's own check, as root before the drop: not counted"
		ip daddr @special counter packets 0 bytes 0 drop comment "no address on the internet: the house, the host, the metadata service"
		tcp dport 443 ct state new accept comment "a push service, or the site's feed"
		counter packets 0 bytes 0 drop comment "another port, a datagram, anything else out"
	}
}
`

func TestCheckRules(t *testing.T) {
	if err := checkRules(listed); err != nil {
		t.Fatalf("the table as firewall.nft makes it: %v", err)
	}
	special := `		ip daddr @special counter packets 0 bytes 0 drop comment "no address on the internet: the house, the host, the metadata service"` + "\n"
	web := `		tcp dport 443 ct state new accept comment "a push service, or the site's feed"` + "\n"
	for name, change := range map[string][2]string{
		"policy accept":        {"hook output priority filter; policy drop;", "hook output priority filter; policy accept;"},
		"no chain output":      {"chain output {", "chain elsewhere {"},
		"no way out":           {web, ""},
		"the house reachable":  {special, ""},
		"443 before the house": {special + web, web + special},
		"443 dropped":          {"tcp dport 443 ct state new accept", "tcp dport 443 ct state new drop"},
		"the house accepted":   {"ip daddr @special counter packets 0 bytes 0 drop", "ip daddr @special counter packets 0 bytes 0 accept"},
	} {
		text := strings.Replace(listed, change[0], change[1], 1)
		if text == listed {
			t.Fatalf("%s: the change does not apply", name)
		}
		if err := checkRules(text); err == nil {
			t.Errorf("%s: taken", name)
		}
	}
}

func TestProbeTakesOnlyAddresses(t *testing.T) {
	_, errOut := capture(t)
	if code := runProbe(context.Background(), []string{"traefik:8080"}); code != 2 || !strings.Contains(errOut.String(), "ip:port") {
		t.Errorf("a name: %d %s", code, errOut)
	}
	_, errOut = capture(t)
	if code := runProbe(context.Background(), []string{"--web", "example.com:443"}); code != 2 || !strings.Contains(errOut.String(), "ip:port") {
		t.Errorf("--web with a name: %d %s", code, errOut)
	}
	t.Setenv("XYLEM_SITE", "https://canary.betula.app")
	if got := siteHost(); got != "canary.betula.app" {
		t.Errorf("siteHost = %q", got)
	}
	t.Setenv("XYLEM_SITE", "")
	if got := siteHost(); got != "betula.app" {
		t.Errorf("siteHost without a site = %q", got)
	}
}

func TestStartIsRootsAlone(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("as root, start would load a firewall")
	}
	_, errOut := capture(t)
	if code := run([]string{"start", "--log-format", "text"}); code != 1 {
		t.Errorf("start as a user: %d %s", code, errOut)
	}
}

// The firewall's own addresses that are none on the internet are those of Xylem's dialer
// (internal/server/endpoint.go, special): the two lists are one rule, kept twice.
func TestTheFirewallNamesTheDialersAddresses(t *testing.T) {
	rules, err := os.ReadFile("../../deploy/firewall.nft")
	if err != nil {
		t.Fatal(err)
	}
	m := regexp.MustCompile(`elements = \{([^}]*)\}`).FindSubmatch(rules)
	if m == nil {
		t.Fatal("firewall.nft has no set with elements")
	}
	var nft []string
	for _, f := range strings.Split(string(m[1]), ",") {
		p, err := netip.ParsePrefix(strings.TrimSpace(f))
		if err != nil {
			t.Fatalf("firewall.nft: %q: %v", f, err)
		}
		nft = append(nft, p.String())
	}
	dialer, err := os.ReadFile("../../internal/server/endpoint.go")
	if err != nil {
		t.Fatal(err)
	}
	var own []string
	for _, m := range regexp.MustCompile(`netip\.MustParsePrefix\("([^"]+)"\)`).FindAllSubmatch(dialer, -1) {
		own = append(own, string(m[1]))
	}
	if len(own) < 10 || strings.Join(nft, " ") != strings.Join(own, " ") {
		t.Errorf("firewall.nft drops\n  %s\nthe dialer refuses\n  %s", strings.Join(nft, " "), strings.Join(own, " "))
	}
	for _, word := range []string{"tcp dport 8300 ct state new accept", "tcp dport 443 ct state new accept", checkTargetRule()} {
		if !strings.Contains(string(rules), word) {
			t.Errorf("firewall.nft has no rule %q", word)
		}
	}
}

// checkTargetRule is the rule of firewall.nft that start's own datagram meets.
func checkTargetRule() string {
	ap := netip.MustParseAddrPort(checkTarget)
	return "meta skuid 0 ip daddr " + ap.Addr().String() + " udp dport " + strings.TrimPrefix(checkTarget, ap.Addr().String()+":") + " drop"
}

// TestTheFirewallFileLoads checks firewall.nft in nft's dry run, where nft can (as root, with
// nftables in the kernel); elsewhere it says why it cannot.
func TestTheFirewallFileLoads(t *testing.T) {
	nft, err := exec.LookPath("nft")
	if err != nil || os.Geteuid() != 0 {
		t.Skip("needs nft and root")
	}
	rules, _ := filepath.Abs("../../deploy/firewall.nft")
	out, err := exec.Command(nft, "-c", "-f", rules).CombinedOutput()
	if err != nil {
		if strings.Contains(string(out), "Operation not permitted") {
			t.Skip("nft may not ask the kernel here")
		}
		t.Errorf("%v: %s", err, out)
	}
}
