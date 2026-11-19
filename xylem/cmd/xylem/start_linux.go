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

	"github.com/leonieziechmann/betula/xylem/internal/oplog"
)

// runStart is the image's entrypoint (docs/xylem/xylem.md, „On the server"). As root with
// NET_ADMIN, SETUID and SETGID (and nothing else), in the container's own network namespace:
//
//  1. nft -f /etc/xylem/firewall.nft: in only 8300; out a name asked of Docker's resolver and
//     port 443 of an address on the internet, nothing inside the house, no IPv6;
//  2. the table is there with that way out, and a datagram to an address that is none on the
//     internet is dropped;
//  3. no_new_privs, no groups, uid and gid 65532: the kernel clears every capability;
//  4. /proc says so (uid, gid, CapInh, CapPrm, CapEff, CapAmb, NoNewPrivs), and root does not
//     come back;
//  5. exec xylem serve with the arguments after start's flags: the same process (PID 1, which
//     the stop signal reaches), no shell.
//
// Xylem then cannot change the firewall. Anything that fails ends start with exit 1 before
// Xylem serves: without its firewall, Xylem does not run.
//
// Log events: start.firewall, start.dropped, start.failed (ERROR).
func runStart(ctx context.Context, args []string) int {
	// One thread from here to the exec: the checks of /proc/thread-self look at the thread that
	// becomes serve.
	runtime.LockOSThread()
	fs := flag.NewFlagSet("start", flag.ContinueOnError)
	rules := fs.String("firewall", envOr("XYLEM_FIREWALL", "/etc/xylem/firewall.nft"),
		"The rules nft loads before Xylem serves (env XYLEM_FIREWALL)")
	nft := fs.String("nft", envOr("XYLEM_NFT", "nft"), "The nft binary, by path or in PATH (env XYLEM_NFT)")
	logs := addLogFlags(fs)
	fs.Usage = func() {
		fmt.Fprintln(stderr, "Usage: xylem start [flags] [serve flags]")
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
	if err := applyFirewall(ctx, nftPath, *rules); err != nil {
		return fail("cannot load the firewall", err)
	}
	if err := checkFirewall(ctx, nftPath); err != nil {
		return fail("the firewall does not hold", err)
	}
	log.Info("firewall in place", "event", "start.firewall", "rules", *rules)

	self, err := os.Executable()
	if err != nil {
		return fail("cannot find the executable", err)
	}
	if err := noNewPrivileges(); err != nil {
		return fail("cannot drop the privileges", err)
	}
	if err := dropPrivileges(xylemID, xylemID); err != nil {
		return fail("cannot drop the privileges", err)
	}
	for _, path := range []string{"/proc/self/status", "/proc/thread-self/status"} {
		if err := checkUnprivileged(path, xylemID, xylemID); err != nil {
			return fail("privileges left after the drop", err)
		}
	}
	log.Info("running as Xylem's user without capabilities", "event", "start.dropped", "uid", xylemID, "gid", xylemID)
	closeLog()
	argv := append([]string{"xylem", "serve"}, fs.Args()...)
	err = syscall.Exec(self, argv, os.Environ())
	return fail("cannot exec serve", err) // only when exec failed
}
