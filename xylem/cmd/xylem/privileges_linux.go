//go:build linux

package main

import (
	"bufio"
	"bytes"
	"context"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"syscall"
	"time"
)

// xylemID is the user and group Xylem runs as (65532, distroless' "nonroot"); /data and /stats
// belong to it.
const xylemID = 65532

// firewallTable is the table of xylem/deploy/firewall.nft.
const firewallTable = "xylem"

// checkTarget is where start sends the one datagram that its firewall has to drop: an address of
// the documentation range (RFC 5737, nobody's), the discard port. firewall.nft names both.
const checkTarget = "192.0.2.1:9"

// applyFirewall loads the rules with nft -f: one transaction, the table made anew.
func applyFirewall(ctx context.Context, nft, rules string) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, nft, "-f", rules)
	cmd.Env = []string{} // nft needs nothing of the environment
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("%s -f %s: %w: %s", nft, rules, err, strings.TrimSpace(string(out)))
	}
	return nil
}

// checkFirewall says whether the table is there and holds: it lets port 443 out and nothing
// beside it, and a datagram to an address that is none on the internet is dropped on its way
// out.
func checkFirewall(ctx context.Context, nft string) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, nft, "list", "table", "inet", firewallTable)
	cmd.Env = []string{}
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("the table inet %s is missing: %w: %s", firewallTable, err, strings.TrimSpace(string(out)))
	}
	if err := checkRules(string(out)); err != nil {
		return err
	}
	if err := sendDropped("udp4", checkTarget); err != nil {
		return fmt.Errorf("a datagram to %s gets out: %w", checkTarget, err)
	}
	return nil
}

// checkRules reads what nft lists of the table, for what Xylem cannot do without and what must
// stand in front of it: the chain output drops by its policy, and lets port 443 out only behind
// the drop of every address that is none on the internet. (The words it looks for are nft's own
// in every version; how nft writes the rest of a rule is not relied on.)
func checkRules(listing string) error {
	var output []string
	in := false
	for _, line := range strings.Split(listing, "\n") {
		line = strings.TrimSpace(line)
		switch {
		case strings.HasPrefix(line, "chain "):
			in = strings.HasPrefix(line, "chain output ")
		case in && line == "}":
			in = false
		case in:
			output = append(output, line)
		}
	}
	if len(output) == 0 {
		return errors.New("the table has no chain output")
	}
	if !strings.Contains(output[0], "policy drop") {
		return fmt.Errorf("the chain output does not drop by its policy (%s)", output[0])
	}
	special := false
	for _, line := range output[1:] {
		rule, _, _ := strings.Cut(line, " comment ")
		switch {
		case strings.Contains(rule, "@special") && strings.HasSuffix(rule, " drop"):
			special = true
		case strings.Contains(rule, "dport 443") && strings.HasSuffix(rule, " accept"):
			if !special {
				return errors.New("the chain output lets port 443 out before it drops the addresses that are none on the internet")
			}
			return nil
		}
	}
	return errors.New("the chain output does not let port 443 out: Xylem could reach no push service")
}

// sendDropped sends one datagram and wants the firewall to drop it here: EPERM, which the
// kernel returns for a packet a filter of this host dropped on its way out.
func sendDropped(network, addr string) error {
	c, err := net.Dial(network, addr)
	if err != nil {
		if errors.Is(err, syscall.EPERM) {
			return nil
		}
		return err
	}
	defer c.Close()
	_, err = c.Write([]byte("xylem"))
	switch {
	case err == nil:
		return errors.New("the datagram was sent")
	case errors.Is(err, syscall.EPERM):
		return nil
	default:
		return err
	}
}

// dropPrivileges makes every thread of the process uid:gid without supplementary groups; as a
// user other than root it keeps no capability (the kernel clears them with the uid).
func dropPrivileges(uid, gid int) error {
	if err := syscall.Setgroups([]int{}); err != nil {
		return fmt.Errorf("setgroups: %w", err)
	}
	if err := syscall.Setresgid(gid, gid, gid); err != nil {
		return fmt.Errorf("setresgid %d: %w", gid, err)
	}
	if err := syscall.Setresuid(uid, uid, uid); err != nil {
		return fmt.Errorf("setresuid %d: %w", uid, err)
	}
	if err := syscall.Setresuid(0, 0, 0); err == nil {
		return errors.New("root came back after the drop")
	}
	return nil
}

// noNewPrivileges sets no_new_privs for every thread of the process (it belongs to a thread, and
// what a thread executes keeps it): no setuid binary and no file capability can give a
// privilege back.
func noNewPrivileges() error {
	const prSetNoNewPrivs = 38
	if _, _, errno := syscall.AllThreadsSyscall(syscall.SYS_PRCTL, prSetNoNewPrivs, 1, 0); errno != 0 {
		return fmt.Errorf("prctl(PR_SET_NO_NEW_PRIVS): %w", errno)
	}
	return nil
}

// dropIfRoot drops to Xylem's user when a command runs as root (docker exec, the health check),
// so that only start ever uses root: the commands then read the data as its owner, and what they
// send passes the firewall as Xylem's own.
func dropIfRoot() error {
	if os.Geteuid() != 0 || keepRootInTests {
		return nil
	}
	return dropPrivileges(xylemID, xylemID)
}

// keepRootInTests is set by the tests of this package, which may run as root: the drop would
// take the whole test process down to Xylem's user.
var keepRootInTests = false

// procStatus is what /proc/<pid>/status says of a process's privileges.
type procStatus struct {
	uids, gids   []int
	groups       string
	noNewPrivs   string
	capabilities map[string]string // CapInh, CapPrm, CapEff, CapBnd, CapAmb
}

func readStatus(path string) (procStatus, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return procStatus{}, err
	}
	st := procStatus{capabilities: map[string]string{}}
	sc := bufio.NewScanner(bytes.NewReader(data))
	for sc.Scan() {
		key, value, ok := strings.Cut(sc.Text(), ":")
		if !ok {
			continue
		}
		value = strings.TrimSpace(value)
		switch {
		case key == "Uid" || key == "Gid":
			var ids []int
			for _, f := range strings.Fields(value) {
				n, err := strconv.Atoi(f)
				if err != nil {
					return procStatus{}, fmt.Errorf("%s: %s: %q", path, key, value)
				}
				ids = append(ids, n)
			}
			if key == "Uid" {
				st.uids = ids
			} else {
				st.gids = ids
			}
		case key == "Groups":
			st.groups = value
		case key == "NoNewPrivs":
			st.noNewPrivs = value
		case strings.HasPrefix(key, "Cap"):
			st.capabilities[key] = value
		}
	}
	return st, sc.Err()
}

// checkUnprivileged says whether the process of path runs as uid:gid only, without groups,
// without any capability it could use, and with no_new_privs.
func checkUnprivileged(path string, uid, gid int) error {
	st, err := readStatus(path)
	if err != nil {
		return err
	}
	var problems []string
	for _, ids := range []struct {
		name string
		got  []int
		want int
	}{{"uid", st.uids, uid}, {"gid", st.gids, gid}} {
		if len(ids.got) != 4 {
			problems = append(problems, fmt.Sprintf("%s unknown", ids.name))
			continue
		}
		for _, id := range ids.got {
			if id != ids.want {
				problems = append(problems, fmt.Sprintf("%s %v, not %d", ids.name, ids.got, ids.want))
				break
			}
		}
	}
	if st.groups != "" {
		problems = append(problems, "groups "+st.groups)
	}
	for _, c := range []string{"CapInh", "CapPrm", "CapEff", "CapAmb"} {
		v, ok := st.capabilities[c]
		if n, err := strconv.ParseUint(v, 16, 64); !ok || err != nil || n != 0 {
			problems = append(problems, fmt.Sprintf("%s %q", c, v))
		}
	}
	if st.noNewPrivs != "1" {
		problems = append(problems, "NoNewPrivs "+st.noNewPrivs)
	}
	if len(problems) > 0 {
		return fmt.Errorf("%s: %s", path, strings.Join(problems, ", "))
	}
	return nil
}
