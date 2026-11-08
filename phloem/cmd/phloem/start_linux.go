//go:build linux

package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"syscall"

	"github.com/leonieziechmann/betula/phloem/internal/oplog"
)

// runStart is the image's entrypoint (docs/phloem/phloem.md, „Start"). As root with NET_ADMIN,
// SETUID and SETGID (and nothing else), in the container's own network namespace:
//
//  1. nft -f /etc/phloem/firewall.nft: no new connection out, no DNS, no IPv6, in only 8200;
//     with TURN (PHLOEM_TURN_HOST set), /etc/phloem/turn.nft in the same transaction: STUN and
//     TURN in, a relay's datagram to a public address out, nothing else;
//  2. the table is there (with TURN's rules where TURN is served), and a datagram to Docker's
//     resolver is dropped;
//  3. no_new_privs, no groups, uid and gid 65532: the kernel clears every capability;
//  4. /proc says so (uid, gid, CapInh, CapPrm, CapEff, CapAmb, NoNewPrivs), and root does not
//     come back;
//  5. exec phloem serve with the arguments after start's flags: the same process (PID 1, which
//     the stop signal reaches), no shell.
//
// Phloem then cannot change the firewall. Anything that fails ends start with exit 1 before
// Phloem serves: without its firewall, Phloem does not run.
//
// Log events: start.firewall, start.dropped, start.failed (ERROR).
func runStart(ctx context.Context, args []string) int {
	// One thread from here to the exec: the checks of /proc/thread-self look at the thread that
	// becomes serve.
	runtime.LockOSThread()
	fs := flag.NewFlagSet("start", flag.ContinueOnError)
	rules := fs.String("firewall", envOr("PHLOEM_FIREWALL", "/etc/phloem/firewall.nft"),
		"The rules nft loads before Phloem serves (env PHLOEM_FIREWALL)")
	turnRules := fs.String("firewall-turn", envOr("PHLOEM_FIREWALL_TURN", "/etc/phloem/turn.nft"),
		"The rules for TURN, loaded with them when PHLOEM_TURN_HOST is set (env PHLOEM_FIREWALL_TURN)")
	nft := fs.String("nft", envOr("PHLOEM_NFT", "nft"), "The nft binary, by path or in PATH (env PHLOEM_NFT)")
	logs := addLogFlags(fs)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: phloem start [flags] [serve flags]")
		fs.PrintDefaults()
	}
	if code, ok := parse(fs, args, 0, -1); !ok {
		return code
	}
	closeLog, ok := logs.setup()
	if !ok {
		return 2
	}
	defer closeLog()
	log := oplog.For("start")
	fail := func(msg string, err error) int {
		log.Error(msg, "event", "start.failed", oplog.Err(err))
		return 1
	}
	if os.Geteuid() != 0 {
		return fail("start runs as root (the image's user) and drops every privilege itself", errors.New("not root"))
	}
	nftPath, err := exec.LookPath(*nft)
	if err != nil {
		return fail("cannot find nft", err)
	}
	turn, err := turnFirewallFromEnv()
	if err != nil {
		return fail("invalid TURN configuration", err)
	}
	if err := applyFirewall(ctx, nftPath, *rules, *turnRules, turn); err != nil {
		return fail("cannot load the firewall", err)
	}
	if err := checkFirewall(ctx, nftPath, turn); err != nil {
		return fail("the firewall does not hold", err)
	}
	log.Info("firewall in place", "event", "start.firewall", "rules", *rules, "turn", turn.String())

	self, err := os.Executable()
	if err != nil {
		return fail("cannot find the executable", err)
	}
	if err := noNewPrivileges(); err != nil {
		return fail("cannot drop the privileges", err)
	}
	if err := dropPrivileges(phloemID, phloemID); err != nil {
		return fail("cannot drop the privileges", err)
	}
	for _, path := range []string{"/proc/self/status", "/proc/thread-self/status"} {
		if err := checkUnprivileged(path, phloemID, phloemID); err != nil {
			return fail("privileges left after the drop", err)
		}
	}
	log.Info("running as Phloem's user without capabilities", "event", "start.dropped", "uid", phloemID, "gid", phloemID)
	closeLog()
	argv := append([]string{"phloem", "serve"}, fs.Args()...)
	err = syscall.Exec(self, argv, os.Environ())
	return fail("cannot exec serve", err) // only when exec failed
}
