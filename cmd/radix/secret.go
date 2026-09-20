package main

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"os"
	"regexp"
	"strings"

	"golang.org/x/term"

	"github.com/leonieziechmann/betula/internal/secrets"
)

// runSecret manages credentials in the operating system's credential store. A
// secret is never accepted as a command line argument: arguments end up in the
// shell history and in the process list.
func runSecret(ctx context.Context, args []string) {
	usage := func() {
		fmt.Fprintln(os.Stderr, `Usage:
  radix secret set <name>         read the secret from the terminal (hidden) or from stdin and store it
  radix secret status             show where each secret is found; never prints a secret
  radix secret delete <name>      remove the secret from the credential store
  radix secret migrate-config [config.yaml]
                                    move gemini.api_key out of a v1 config file into the credential store

Names: `+strings.Join(secrets.Known, ", "))
		os.Exit(2)
	}
	if len(args) == 0 {
		usage()
	}

	known := func(name string) string {
		for _, k := range secrets.Known {
			if k == name {
				return name
			}
		}
		fmt.Fprintf(os.Stderr, "Unknown secret %q. Known: %s\n", name, strings.Join(secrets.Known, ", "))
		os.Exit(2)
		return ""
	}

	switch args[0] {
	case "status":
		for _, name := range secrets.Known {
			_, source, err := secrets.Resolve(name)
			switch {
			case err == nil:
				fmt.Printf("%-16s found: %s\n", name, source)
			case errors.Is(err, secrets.ErrNotFound):
				fmt.Printf("%-16s not configured; %s\n", name, secrets.HowTo(name))
			default:
				fmt.Printf("%-16s ERROR: %v\n", name, err)
			}
		}

	case "set":
		if len(args) != 2 {
			usage()
		}
		name := known(args[1])
		value, err := readSecret(name)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		if err := secrets.Store(name, value); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("%s stored in the operating system credential store.\n", name)

	case "delete":
		if len(args) != 2 {
			usage()
		}
		name := known(args[1])
		if err := secrets.Delete(name); errors.Is(err, secrets.ErrNotFound) {
			fmt.Printf("%s was not in the credential store.\n", name)
		} else if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		} else {
			fmt.Printf("%s removed from the credential store.\n", name)
		}

	case "migrate-config":
		path := "config.yaml"
		if len(args) > 1 {
			path = args[1]
		}
		if err := migrateConfigSecret(path); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

	default:
		usage()
	}
}

// readSecret reads without echo from a terminal, or one line from a pipe.
func readSecret(name string) (string, error) {
	if term.IsTerminal(int(os.Stdin.Fd())) {
		fmt.Fprintf(os.Stderr, "Enter %s (input is hidden): ", name)
		value, err := term.ReadPassword(int(os.Stdin.Fd()))
		fmt.Fprintln(os.Stderr)
		return string(value), err
	}
	line, err := bufio.NewReader(os.Stdin).ReadString('\n')
	if err != nil && line == "" {
		return "", fmt.Errorf("no secret on stdin: %w", err)
	}
	return strings.TrimSpace(line), nil
}

var configAPIKeyLine = regexp.MustCompile(`(?m)^(\s*api_key:\s*)(["']?)([^"'#\r\n]*)(["']?)(.*)$`)

// migrateConfigSecret moves gemini.api_key from a v1 config.yaml into the credential
// store. The key is removed from the file only after the store returned it again.
// The secret is never printed.
func migrateConfigSecret(path string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	m := configAPIKeyLine.FindSubmatch(data)
	if m == nil || strings.TrimSpace(string(m[3])) == "" {
		fmt.Printf("%s holds no api_key; nothing to migrate.\n", path)
		return nil
	}
	if err := secrets.Store(secrets.GeminiAPIKey, string(m[3])); err != nil {
		return fmt.Errorf("the key stays in %s: %w", path, err)
	}
	cleaned := configAPIKeyLine.ReplaceAll(data, []byte(`${1}""  # moved to the OS credential store: radix secret status`))
	if err := os.WriteFile(path, cleaned, 0600); err != nil {
		return fmt.Errorf("the key is stored, but %s could not be rewritten: %w", path, err)
	}
	fmt.Printf("%s moved from %s into the operating system credential store and removed from the file.\n", secrets.GeminiAPIKey, path)
	fmt.Println("If the file was ever committed or shared, the key is still in that history: rotate it.")
	return nil
}
