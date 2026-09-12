package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestConfigDefaults(t *testing.T) {
	cfg := Default()
	if !cfg.Refresher.AutoRefresh {
		t.Errorf("expected AutoRefresh to be true by default")
	}
	if cfg.Server.Port != "8080" {
		t.Errorf("expected default port 8080, got %s", cfg.Server.Port)
	}
	if cfg.Storage.DBPath != "btu_modules.db" {
		t.Errorf("expected default DBPath btu_modules.db, got %s", cfg.Storage.DBPath)
	}
	if cfg.Storage.AnalyticsDBPath != "btu_analytics.db" {
		t.Errorf("expected default AnalyticsDBPath btu_analytics.db, got %s", cfg.Storage.AnalyticsDBPath)
	}
}

func TestConfigHierarchy_FileOverridesEnv(t *testing.T) {
	tmpDir := t.TempDir()
	yamlFile := filepath.Join(tmpDir, "config.yaml")

	yamlContent := `
server:
  port: "9999"
storage:
  db_path: "custom.db"
refresher:
  auto_refresh: true
  offpeak_start_hour: 2
  offpeak_end_hour: 5
`
	if err := os.WriteFile(yamlFile, []byte(yamlContent), 0644); err != nil {
		t.Fatalf("failed to write yaml file: %v", err)
	}

	// Set environment variables that should be OVERRIDDEN by the config file
	os.Setenv("BTU_PORT", "7777")
	os.Setenv("BTU_DB", "env.db")
	// Set an environment variable not in the config file (should be preserved from env)
	os.Setenv("BTU_ANALYTICS_DB", "env_analytics.db")
	defer func() {
		os.Unsetenv("BTU_PORT")
		os.Unsetenv("BTU_DB")
		os.Unsetenv("BTU_ANALYTICS_DB")
	}()

	cfg, resolved, err := Load(yamlFile)
	if err != nil {
		t.Fatalf("failed to load config: %v", err)
	}

	if resolved != yamlFile {
		t.Errorf("expected resolved path %s, got %s", yamlFile, resolved)
	}

	// 1. Config file must override environment variable (9999 over 7777)
	if cfg.Server.Port != "9999" {
		t.Errorf("expected port from config file '9999', got %s", cfg.Server.Port)
	}
	// 2. Config file must override environment variable (custom.db over env.db)
	if cfg.Storage.DBPath != "custom.db" {
		t.Errorf("expected db_path 'custom.db', got %s", cfg.Storage.DBPath)
	}
	// 3. Env variable not set in config file must override default (env_analytics.db over btu_analytics.db)
	if cfg.Storage.AnalyticsDBPath != "env_analytics.db" {
		t.Errorf("expected analytics_db_path from env 'env_analytics.db', got %s", cfg.Storage.AnalyticsDBPath)
	}
}

func TestConfigJSON(t *testing.T) {
	tmpDir := t.TempDir()
	jsonFile := filepath.Join(tmpDir, "config.json")

	jsonContent := `{
		"server": { "port": "3000" },
		"refresher": { "auto_refresh": false }
	}`
	if err := os.WriteFile(jsonFile, []byte(jsonContent), 0644); err != nil {
		t.Fatalf("failed to write json file: %v", err)
	}

	cfg, _, err := Load(jsonFile)
	if err != nil {
		t.Fatalf("failed to load json config: %v", err)
	}

	if cfg.Server.Port != "3000" {
		t.Errorf("expected port 3000, got %s", cfg.Server.Port)
	}
	if cfg.Refresher.AutoRefresh != false {
		t.Errorf("expected auto_refresh to be false")
	}
}
