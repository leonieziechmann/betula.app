// Package secrets resolves credentials without ever storing them in the
// repository, in a configuration file or on a command line.
//
// A secret named "gemini-api-key" is looked up in this order:
//
//  1. GEMINI_API_KEY_FILE      path of a file that holds the secret (Kubernetes secrets,
//     sops-nix, agenix, or a Docker secret under another name).
//  2. /run/secrets/<name>      Docker Swarm / Compose secrets, found without any
//     configuration: "gemini-api-key" or "gemini_api_key".
//  3. $CREDENTIALS_DIRECTORY   systemd credentials (LoadCredential= /
//     LoadCredentialEncrypted=), file name "gemini-api-key".
//  4. GEMINI_API_KEY           environment variable, for CI and one-off shells. During
//     development it may come from a git-ignored .env file (LoadDotEnv).
//  5. the operating system's credential store: Windows Credential Manager, macOS
//     Keychain, or the Secret Service (GNOME Keyring, KWallet) on Linux. This is the
//     place for a developer machine; `scraper secret set` writes it.
//
// An explicitly configured source wins over the keyring, so that a service never
// silently picks up a developer's personal credential.
package secrets

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/zalando/go-keyring"
)

// GeminiAPIKey is the name of the Gemini API credential.
const GeminiAPIKey = "gemini-api-key"

const keyringService = "btu-scraper"

// dockerSecretsDir is where Docker mounts the secrets of a service (tmpfs).
var dockerSecretsDir = "/run/secrets"

// ErrNotFound means no source holds the secret.
var ErrNotFound = errors.New("secret not found")

// Known lists the secrets this program uses, for `secret status`.
var Known = []string{GeminiAPIKey}

// Source says where a secret came from. It never contains the secret.
type Source string

// envName turns "gemini-api-key" into "GEMINI_API_KEY".
func envName(name string) string {
	return strings.ToUpper(strings.ReplaceAll(name, "-", "_"))
}

// Resolve returns the secret and where it was found.
func Resolve(name string) (string, Source, error) {
	env := envName(name)

	if path := os.Getenv(env + "_FILE"); path != "" {
		value, err := readSecretFile(path)
		if err != nil {
			return "", "", fmt.Errorf("%s_FILE: %w", env, err)
		}
		return value, Source("file named by " + env + "_FILE"), nil
	}

	for _, file := range []string{name, strings.ReplaceAll(name, "-", "_")} {
		value, err := readSecretFile(filepath.Join(dockerSecretsDir, file))
		if err == nil {
			return value, Source("Docker secret " + file), nil
		}
		if !errors.Is(err, os.ErrNotExist) {
			return "", "", fmt.Errorf("Docker secret %s: %w", file, err)
		}
	}

	if dir := os.Getenv("CREDENTIALS_DIRECTORY"); dir != "" {
		value, err := readSecretFile(filepath.Join(dir, name))
		if err == nil {
			return value, "systemd credential", nil
		}
		if !errors.Is(err, os.ErrNotExist) {
			return "", "", fmt.Errorf("systemd credential %s: %w", name, err)
		}
	}

	if value := strings.TrimSpace(os.Getenv(env)); value != "" {
		if loaded, ok := fromDotEnv[env]; ok && strings.TrimSpace(loaded.value) == value {
			return value, Source("environment variable " + env + " (development file " + loaded.file + ")"), nil
		}
		return value, Source("environment variable " + env), nil
	}

	value, err := keyring.Get(keyringService, name)
	if err == nil && strings.TrimSpace(value) != "" {
		return strings.TrimSpace(value), "operating system credential store", nil
	}
	// A missing entry and a missing keyring (a container has none) are the same to the caller.
	return "", "", ErrNotFound
}

func readSecretFile(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	value := strings.TrimSpace(string(data))
	if value == "" {
		return "", fmt.Errorf("%s is empty", path)
	}
	return value, nil
}

// Store writes a secret to the operating system's credential store.
func Store(name, value string) error {
	value = strings.TrimSpace(value)
	if value == "" {
		return errors.New("refusing to store an empty secret")
	}
	if err := keyring.Set(keyringService, name, value); err != nil {
		return fmt.Errorf("the operating system credential store is not available: %w", err)
	}
	// Read it back: a store that silently drops the value must not pass for success.
	stored, err := keyring.Get(keyringService, name)
	if err != nil || strings.TrimSpace(stored) != value {
		return errors.New("the credential store did not return the stored secret")
	}
	return nil
}

// Delete removes a secret from the operating system's credential store.
func Delete(name string) error {
	err := keyring.Delete(keyringService, name)
	if errors.Is(err, keyring.ErrNotFound) {
		return ErrNotFound
	}
	return err
}

// HowTo explains, for an error message, how to provide a secret.
func HowTo(name string) string {
	env := envName(name)
	return fmt.Sprintf("provide it with `scraper secret set %s` (developer machine), "+
		"a Docker secret named %q, %s_FILE=/path/to/secret (Kubernetes, sops-nix, agenix), "+
		"a systemd credential named %q, or the %s environment variable", name, name, env, name, env)
}
