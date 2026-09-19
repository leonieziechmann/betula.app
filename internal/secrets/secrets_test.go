package secrets

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/zalando/go-keyring"
)

func TestResolveOrder(t *testing.T) {
	keyring.MockInit() // in-memory keyring: tests never touch the real credential store
	dir := t.TempDir()
	dockerSecretsDir = filepath.Join(dir, "no-docker")
	t.Cleanup(func() { dockerSecretsDir = "/run/secrets" })

	if _, _, err := Resolve(GeminiAPIKey); !errors.Is(err, ErrNotFound) {
		t.Fatalf("nothing configured: err = %v, want ErrNotFound", err)
	}

	if err := Store(GeminiAPIKey, "  from-keyring\n"); err != nil {
		t.Fatalf("Store failed: %v", err)
	}
	if v, src, err := Resolve(GeminiAPIKey); err != nil || v != "from-keyring" || src != "operating system credential store" {
		t.Fatalf("keyring: %q, %q, %v", v, src, err)
	}

	// Anything configured explicitly for the process wins over the developer's keyring.
	t.Setenv("GEMINI_API_KEY", "from-env")
	if v, src, _ := Resolve(GeminiAPIKey); v != "from-env" || src != "environment variable GEMINI_API_KEY" {
		t.Fatalf("env: %q, %q", v, src)
	}

	t.Setenv("CREDENTIALS_DIRECTORY", dir)
	if v, _, _ := Resolve(GeminiAPIKey); v != "from-env" {
		t.Fatalf("an empty credentials directory must not hide the other sources, got %q", v)
	}
	if err := os.WriteFile(filepath.Join(dir, GeminiAPIKey), []byte("from-systemd\n"), 0600); err != nil {
		t.Fatal(err)
	}
	if v, src, _ := Resolve(GeminiAPIKey); v != "from-systemd" || src != "systemd credential" {
		t.Fatalf("systemd: %q, %q", v, src)
	}

	// Docker Swarm mounts the secret of a service without any configuration.
	swarm := t.TempDir()
	dockerSecretsDir = swarm
	t.Cleanup(func() { dockerSecretsDir = "/run/secrets" })
	if err := os.WriteFile(filepath.Join(swarm, "gemini_api_key"), []byte("from-swarm\n"), 0400); err != nil {
		t.Fatal(err)
	}
	if v, src, _ := Resolve(GeminiAPIKey); v != "from-swarm" || src != "Docker secret gemini_api_key" {
		t.Fatalf("docker secret: %q, %q", v, src)
	}

	secretFile := filepath.Join(dir, "docker-secret")
	if err := os.WriteFile(secretFile, []byte("from-file\n"), 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("GEMINI_API_KEY_FILE", secretFile)
	if v, src, _ := Resolve(GeminiAPIKey); v != "from-file" || src != "file named by GEMINI_API_KEY_FILE" {
		t.Fatalf("file: %q, %q", v, src)
	}

	// A configured but broken source is an error, not a silent fallback to another credential.
	t.Setenv("GEMINI_API_KEY_FILE", filepath.Join(dir, "missing"))
	if _, _, err := Resolve(GeminiAPIKey); err == nil || errors.Is(err, ErrNotFound) {
		t.Fatalf("missing secret file: err = %v, want a real error", err)
	}
}

func TestStoreAndDelete(t *testing.T) {
	keyring.MockInit()
	if err := Store(GeminiAPIKey, "   "); err == nil {
		t.Error("an empty secret was stored")
	}
	if err := Delete(GeminiAPIKey); !errors.Is(err, ErrNotFound) {
		t.Errorf("Delete of a missing secret = %v, want ErrNotFound", err)
	}
	if err := Store(GeminiAPIKey, "value"); err != nil {
		t.Fatal(err)
	}
	if err := Delete(GeminiAPIKey); err != nil {
		t.Fatalf("Delete failed: %v", err)
	}
	if _, _, err := Resolve(GeminiAPIKey); !errors.Is(err, ErrNotFound) {
		t.Errorf("after Delete: err = %v", err)
	}
}
