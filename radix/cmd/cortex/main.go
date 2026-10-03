// Command cortex is the cache between Betula and the internet (docs/cortex/cortex.md): a service that
// fetches from public hosts on behalf of every client, under one floor per host, keeps every
// answer that differed with its history, stores named files, and replicates all of it to a
// second instance that takes over when the first one goes away.
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

	"github.com/leonieziechmann/betula/radix/internal/secrets"
	"github.com/leonieziechmann/betula/radix/internal/version"
)

// cortexVersion is the release of Cortex (GET /status, `cortex version`).
const cortexVersion = "1.0.0"

// The streams of the commands; the tests replace them.
var (
	stdin  io.Reader = os.Stdin
	stdout io.Writer = os.Stdout
	stderr io.Writer = os.Stderr
)

// command is one of cortex's commands; run returns the exit code: 0 success, 1 failure,
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
		{"serve", "Service: the HTTP API on --addr, leading or following (the default command)", runServe},
		{"healthcheck", "Exit 0 if the local instance is live (for container health checks)", runHealthcheck},
		{"status", "Print the status of an instance as one JSON object (GET /status)", runStatus},
		{"step-down", "Ask the leader to hand over to the other instance; non-zero exit if it does not lead", runStepDown},
		{"put", "Store a file: put NAME [FILE], the content from stdin without FILE", runPut},
		{"get", "Write the current content of a file to stdout: get NAME", runGet},
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

	// Development convenience: a git-ignored .env file. The real environment wins.
	if _, err := secrets.LoadDotEnv(envOr("CORTEX_ENV_FILE", ".env")); err != nil {
		fmt.Fprintln(stderr, "Error:", err)
		return 2
	}

	name := "serve"
	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		name, args = args[0], args[1:]
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
	fmt.Fprintln(w, "Cortex "+cortexVersion+", the cache between Betula and the internet\n\nUsage:\n  cortex [<command>] [flags]      (cortex <command> --help lists the flags)\n\nCommands:")
	for _, c := range commands {
		fmt.Fprintf(w, "  %-14s %s\n", c.name, c.summary)
	}
	fmt.Fprintln(w, "\nEvery flag has an environment variable (CORTEX_…), named in its help; a .env file is read\n"+
		"from CORTEX_ENV_FILE (default .env), and the real environment wins over it.")
}

func runHelp(ctx context.Context, args []string) int {
	printUsage(stdout)
	return 0
}

func runVersion(ctx context.Context, args []string) int {
	line := "Cortex " + cortexVersion
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

func envBool(name string, fallback bool) bool {
	if v, err := strconv.ParseBool(os.Getenv(name)); err == nil {
		return v
	}
	return fallback
}

// parse parses the flags of a command: ok false with the exit code when it is not to run
// (0 after --help, 2 for invalid flags or arguments beyond want).
func parse(fs *flag.FlagSet, args []string, minArgs, maxArgs int) (int, bool) {
	fs.SetOutput(stderr)
	if err := fs.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return 0, false
		}
		return 2, false
	}
	if n := fs.NArg(); n < minArgs || n > maxArgs {
		fmt.Fprintf(stderr, "Error: %s takes %d to %d arguments, got %d\n", fs.Name(), minArgs, maxArgs, n)
		fs.Usage()
		return 2, false
	}
	return 0, true
}
