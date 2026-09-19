package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/zalando/go-keyring"

	"github.com/leonieziechmann/btu-scraper/internal/secrets"
)

func TestMigrateConfigSecretMovesTheKeyOutOfTheFile(t *testing.T) {
	keyring.MockInit()
	path := filepath.Join(t.TempDir(), "config.yaml")
	config := "server:\n  port: \"8080\"\ngemini:\n  # API Key\n  api_key: \"sk-test-0123456789\"\n\n  model: \"gemini-3.5-flash-lite\"\n"
	if err := os.WriteFile(path, []byte(config), 0644); err != nil {
		t.Fatal(err)
	}

	if err := migrateConfigSecret(path); err != nil {
		t.Fatalf("migrateConfigSecret failed: %v", err)
	}
	value, source, err := secrets.Resolve(secrets.GeminiAPIKey)
	if err != nil || value != "sk-test-0123456789" || !strings.Contains(string(source), "credential store") {
		t.Fatalf("after migration: source %q, err %v", source, err)
	}
	cleaned, _ := os.ReadFile(path)
	if strings.Contains(string(cleaned), "sk-test") {
		t.Fatalf("the key is still in the file:\n%s", cleaned)
	}
	if !strings.Contains(string(cleaned), `port: "8080"`) || !strings.Contains(string(cleaned), `model: "gemini-3.5-flash-lite"`) {
		t.Errorf("the rest of the file was damaged:\n%s", cleaned)
	}

	// Running it again finds nothing and keeps the stored key.
	if err := migrateConfigSecret(path); err != nil {
		t.Fatalf("second run failed: %v", err)
	}
	if value, _, _ := secrets.Resolve(secrets.GeminiAPIKey); value != "sk-test-0123456789" {
		t.Error("the second run damaged the stored key")
	}
}

func TestMigrateConfigSecretKeepsTheFileWhenTheStoreFails(t *testing.T) {
	keyring.MockInitWithError(os.ErrPermission)
	path := filepath.Join(t.TempDir(), "config.yaml")
	config := "gemini:\n  api_key: sk-test-0123456789\n"
	_ = os.WriteFile(path, []byte(config), 0644)

	if err := migrateConfigSecret(path); err == nil {
		t.Fatal("expected an error when the credential store is unavailable")
	}
	if kept, _ := os.ReadFile(path); string(kept) != config {
		t.Errorf("the file was changed although the key was not stored:\n%s", kept)
	}
}
