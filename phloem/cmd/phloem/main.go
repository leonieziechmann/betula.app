// Command phloem is the service of Betula Sync (docs/phloem/phloem.md): it keeps the sealed
// records of the profiles that sync and hands them between the devices of a profile, live, and
// can open none of them. The image runs `phloem start`, which puts Phloem's firewall in place
// and drops every privilege before it becomes `phloem serve`.
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

	"github.com/leonieziechmann/betula/phloem/internal/dotenv"
	"github.com/leonieziechmann/betula/phloem/internal/version"
)

// The streams of the commands; the tests replace them.
var (
	stdout io.Writer = os.Stdout
	stderr io.Writer = os.Stderr
)

// command is one of phloem's commands; run returns the exit code: 0 success, 1 failure,
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
		{"serve", "Service: the HTTP API and the WebSockets on --addr (the default command)", runServe},
		{"start", "The image's entrypoint: as root, the firewall, then without privileges serve", runStart},
		{"healthcheck", "Exit 0 if the local instance is live (for container health checks)", runHealthcheck},
		{"status", "Print the status of the local instance as one JSON object (GET /status)", runStatus},
		{"probe", "Try what the firewall forbids, and check the privileges of the service; exit 1 if anything is open", runProbe},
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

	if err := loadDevEnv(name); err != nil {
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

// loadDevEnv reads the development convenience, a git-ignored .env file (PHLOEM_ENV_FILE); the
// real environment wins. Never for start: what configures the step that runs as root (the
// firewall, the nft it runs) comes from the image and the stack alone, not from a file that a
// Phloem taken over could have written.
func loadDevEnv(command string) error {
	if command == "start" {
		return nil
	}
	_, err := dotenv.LoadDotEnv(envOr("PHLOEM_ENV_FILE", ".env"))
	return err
}

func printUsage(w io.Writer) {
	fmt.Fprintln(w, "Phloem "+version.Version+", the service of Betula Sync\n\nUsage:\n  phloem [<command>] [flags]      (phloem <command> --help lists the flags)\n\nCommands:")
	for _, c := range commands {
		fmt.Fprintf(w, "  %-12s %s\n", c.name, c.summary)
	}
	fmt.Fprintln(w, "\nEvery flag has an environment variable (PHLOEM_…), named in its help; a .env file is read\n"+
		"from PHLOEM_ENV_FILE (default .env), and the real environment wins over it.")
}

func runHelp(ctx context.Context, args []string) int {
	printUsage(stdout)
	return 0
}

func runVersion(ctx context.Context, args []string) int {
	line := "Phloem " + version.Version
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
