// Command xylem is the service of Betula's notifications (docs/xylem/xylem.md): it keeps the
// push subscriptions of the browsers that asked for notifications, with the topics each listens
// to, and tells every one of a topic what is announced for it (the exam period begins, the next
// semester's dates are there, the lectures pause), through the browser's own push service. It
// knows no profile and no plan.
package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
	"time"
	_ "time/tzdata" // TZ=Europe/Berlin in an image without zoneinfo

	"github.com/leonieziechmann/betula/xylem/internal/dotenv"
	"github.com/leonieziechmann/betula/xylem/internal/version"
)

// The streams of the commands; the tests replace them.
var (
	stdout io.Writer = os.Stdout
	stderr io.Writer = os.Stderr
)

// command is one of xylem's commands; run returns the exit code: 0 success, 1 failure,
// 2 invalid flags or configuration, 130 interrupted.
type command struct {
	name    string
	summary string
	run     func(ctx context.Context, args []string) int
}

var commands []command

func init() {
	// In init, since help refers to the table.
	commands = []command{
		{"serve", "Service: the HTTP API on --addr and the sender (the default command)", runServe},
		{"announce", "Tell everybody who listens to a topic something, now or at a time", runAnnounce},
		{"announcements", "List the newest announcements and how far each has come", runAnnouncements},
		{"cancel", "End an announcement that is not finished: nobody else gets it", runCancel},
		{"status", "Print the status of the local instance as one JSON object (GET /status)", runStatus},
		{"key", "Print the public half of the VAPID key, which a browser subscribes with", runKey},
		{"healthcheck", "Exit 0 if the local instance is live (for container health checks)", runHealthcheck},
		{"version", "Print the version", runVersion},
		{"help", "Print this help", runHelp},
	}
}

func main() {
	os.Exit(run(os.Args[1:]))
}

// run runs the command of args (serve without one) and returns its exit code.
func run(args []string) int {
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	name := "serve"
	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		name, args = args[0], args[1:]
	}

	// The development convenience, a git-ignored .env file (XYLEM_ENV_FILE); the real
	// environment wins.
	if _, err := dotenv.LoadDotEnv(envOr("XYLEM_ENV_FILE", ".env")); err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return 2
	}
	if name == "--help" || name == "-h" {
		name = "help"
	}
	for _, c := range commands {
		if c.name == name {
			return c.run(ctx, args)
		}
	}
	fmt.Fprintf(stderr, "Unknown command: %s\n\n", name)
	printUsage(stderr)
	return 2
}

func printUsage(w io.Writer) {
	fmt.Fprintln(w, "Xylem "+version.Version+", the service of Betula's notifications\n\nUsage:\n  xylem [<command>] [flags]      (xylem <command> --help lists the flags)\n\nCommands:")
	for _, c := range commands {
		fmt.Fprintf(w, "  %-14s %s\n", c.name, c.summary)
	}
	fmt.Fprintln(w, "\nEvery flag has an environment variable (XYLEM_…), named in its help; a .env file is read\n"+
		"from XYLEM_ENV_FILE (default .env), and the real environment wins over it.")
}

func runHelp(ctx context.Context, args []string) int {
	printUsage(stdout)
	return 0
}

func runVersion(ctx context.Context, args []string) int {
	line := "Xylem " + version.Version
	if build := version.Build(); build != "" {
		line += " (build " + build + ")"
	}
	fmt.Fprintln(stdout, line)
	return 0
}

// envOr returns the environment variable name, or fallback when it is unset or empty.
func envOr(name, fallback string) string {
	if v := os.Getenv(name); v != "" {
		return v
	}
	return fallback
}

func envDuration(name string, fallback time.Duration) time.Duration {
	if v, err := time.ParseDuration(os.Getenv(name)); err == nil {
		return v
	}
	return fallback
}

func envInt64(name string, fallback int64) int64 {
	if v, err := strconv.ParseInt(os.Getenv(name), 10, 64); err == nil {
		return v
	}
	return fallback
}

// parse parses the flags of a command: ok false with the exit code when it is not to run
// (0 after --help, 2 for invalid flags or arguments beyond maxArgs; maxArgs -1: any).
func parse(fs *flag.FlagSet, args []string, minArgs, maxArgs int) (int, bool) {
	fs.SetOutput(stderr)
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return 0, false
		}
		return 2, false
	}
	if n := fs.NArg(); n < minArgs || (maxArgs >= 0 && n > maxArgs) {
		fmt.Fprintf(stderr, "Error: %s takes %d to %d arguments, got %d\n", fs.Name(), minArgs, maxArgs, n)
		fs.Usage()
		return 2, false
	}
	return 0, true
}
