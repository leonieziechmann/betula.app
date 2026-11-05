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

// phloemID is the user and group Phloem runs as (65532, distroless' "nonroot"); /data and
// /stats belong to it.
const phloemID = 65532

// firewallTable is the table of phloem/deploy/firewall.nft.
const firewallTable = "phloem"

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

// checkFirewall says whether the table is there and holds: a datagram to Docker's resolver
// (127.0.0.11, over loopback, so that a route always exists) must be dropped on its way out.
func checkFirewall(ctx context.Context, nft string) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, nft, "list", "table", "inet", firewallTable)
	cmd.Env = []string{}
	if out, err := cmd.CombinedOutput(); err != nil {
		return fmt.Errorf("the table inet %s is missing: %w: %s", firewallTable, err, strings.TrimSpace(string(out)))
	}
	if err := sendDropped("udp4", "127.0.0.11:53"); err != nil {
		return fmt.Errorf("DNS gets out: %w", err)
	}
	return nil
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
	_, err = c.Write(dnsQuery())
	switch {
	case err == nil:
		return errors.New("the datagram was sent")
	case errors.Is(err, syscall.EPERM):
		return nil
	default:
		return err
	}
}

// dnsQuery is a DNS query for betula.app (A), so that a datagram that does get out is a real
// question and not noise.
func dnsQuery() []byte {
	q := []byte{0x13, 0x37, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00}
	for _, label := range []string{"betula", "app"} {
		q = append(q, byte(len(label)))
		q = append(q, label...)
	}
	return append(q, 0x00, 0x00, 0x01, 0x00, 0x01)
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

// dropIfRoot drops to Phloem's user when the command runs as root (docker exec, the health
// check), so that only start ever uses root.
func dropIfRoot() error {
	if os.Geteuid() != 0 || keepRootInTests {
		return nil
	}
	return dropPrivileges(phloemID, phloemID)
}

// keepRootInTests is set by the tests of this package, which may run as root: the drop would
// take the whole test process down to Phloem's user.
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
