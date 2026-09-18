package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

// Config represents all application configuration settings.
type Config struct {
	Server    ServerConfig    `json:"server" yaml:"server"`
	Storage   StorageConfig   `json:"storage" yaml:"storage"`
	Refresher RefresherConfig `json:"refresher" yaml:"refresher"`
	Logging   LoggingConfig   `json:"logging" yaml:"logging"`
	Gemini    GeminiConfig    `json:"gemini" yaml:"gemini"`
}

// GeminiConfig configures the Gemini AI model and API key.
type GeminiConfig struct {
	APIKey string `json:"api_key" yaml:"api_key"`
	Model  string `json:"model" yaml:"model"`
}

// ServerConfig configures the HTTP web server.
type ServerConfig struct {
	Port string `json:"port" yaml:"port"`
}

// StorageConfig configures SQLite database paths and caching.
type StorageConfig struct {
	DBPath          string `json:"db_path" yaml:"db_path"`
	AnalyticsDBPath string `json:"analytics_db_path" yaml:"analytics_db_path"`
	CacheDir        string `json:"cache_dir" yaml:"cache_dir"`
	StatutesDir     string `json:"statutes_dir" yaml:"statutes_dir"`
}

// RefresherConfig configures the background polite scraping worker.
type RefresherConfig struct {
	AutoRefresh          bool `json:"auto_refresh" yaml:"auto_refresh"`
	OffPeakStartHour     int  `json:"offpeak_start_hour" yaml:"offpeak_start_hour"`
	OffPeakEndHour       int  `json:"offpeak_end_hour" yaml:"offpeak_end_hour"`
	ModuleDelayMs        int  `json:"module_delay_ms" yaml:"module_delay_ms"`
	QISDelayMs           int  `json:"qis_delay_ms" yaml:"qis_delay_ms"`
	CatalogIntervalHours int  `json:"catalog_interval_hours" yaml:"catalog_interval_hours"`
}

// LoggingConfig configures system and scraper logging.
type LoggingConfig struct {
	LogFile    string `json:"log_file" yaml:"log_file"`
	MinLevel   string `json:"min_level" yaml:"min_level"`
	BufferSize int    `json:"buffer_size" yaml:"buffer_size"`
}

// Default returns the default configuration.
func Default() Config {
	return Config{
		Server: ServerConfig{
			Port: "8080",
		},
		Storage: StorageConfig{
			DBPath:          "btu_modules.db",
			AnalyticsDBPath: "btu_analytics.db",
			CacheDir:        ".cache",
			StatutesDir:     "statutes",
		},
		Refresher: RefresherConfig{
			AutoRefresh:          true, // Default: background service worker runs automatically on serve
			OffPeakStartHour:     1,    // 01:00
			OffPeakEndHour:       6,    // 06:00
			ModuleDelayMs:        500,  // 500ms delay (~2 req/s)
			QISDelayMs:           500,  // 500ms delay (~2 req/s)
			CatalogIntervalHours: 12,   // 12 hours
		},
		Logging: LoggingConfig{
			LogFile:    "btu_scraper.log",
			MinLevel:   "INFO",
			BufferSize: 300,
		},
		Gemini: GeminiConfig{
			APIKey: "",
			Model:  "gemini-3.5-flash-lite",
		},
	}
}

// Load loads configuration using the priority hierarchy:
// Config File > Environment Variables > Defaults.
// If configPath is empty, it automatically searches for config.yaml, config.yml, or config.json.
func Load(configPath string) (Config, string, error) {
	cfg := Default()

	// 1. Apply Environment Variables
	applyEnvVars(&cfg)

	// 2. Locate config file if not explicitly specified
	resolvedPath := configPath
	if resolvedPath == "" {
		candidates := []string{"config.yaml", "config.yml", "config.json"}
		for _, cand := range candidates {
			if _, err := os.Stat(cand); err == nil {
				resolvedPath = cand
				break
			}
		}
	}

	// 3. Load and apply Config File (Config > Env Vars)
	if resolvedPath != "" {
		if err := loadConfigFile(resolvedPath, &cfg); err != nil {
			return cfg, resolvedPath, fmt.Errorf("failed to load config file %s: %w", resolvedPath, err)
		}
	}

	return cfg, resolvedPath, nil
}

func applyEnvVars(cfg *Config) {
	// Server
	if v := os.Getenv("BTU_PORT"); v != "" {
		cfg.Server.Port = v
	}

	// Storage
	if v := os.Getenv("BTU_DB"); v != "" {
		cfg.Storage.DBPath = v
	}
	if v := os.Getenv("BTU_ANALYTICS_DB"); v != "" {
		cfg.Storage.AnalyticsDBPath = v
	}
	if v := os.Getenv("BTU_CACHE_DIR"); v != "" {
		cfg.Storage.CacheDir = v
	}
	if v := os.Getenv("BTU_STATUTES_DIR"); v != "" {
		cfg.Storage.StatutesDir = v
	}

	// Refresher
	if v := os.Getenv("BTU_AUTO_REFRESH"); v != "" {
		if b, err := strconv.ParseBool(v); err == nil {
			cfg.Refresher.AutoRefresh = b
		}
	}
	if v := os.Getenv("BTU_OFFPEAK_START"); v != "" {
		if i, err := strconv.Atoi(v); err == nil {
			cfg.Refresher.OffPeakStartHour = i
		}
	}
	if v := os.Getenv("BTU_OFFPEAK_END"); v != "" {
		if i, err := strconv.Atoi(v); err == nil {
			cfg.Refresher.OffPeakEndHour = i
		}
	}
	if v := os.Getenv("BTU_MODULE_DELAY_MS"); v != "" {
		if i, err := strconv.Atoi(v); err == nil {
			cfg.Refresher.ModuleDelayMs = i
		}
	}
	if v := os.Getenv("BTU_QIS_DELAY_MS"); v != "" {
		if i, err := strconv.Atoi(v); err == nil {
			cfg.Refresher.QISDelayMs = i
		}
	}
	if v := os.Getenv("BTU_CATALOG_INTERVAL_HOURS"); v != "" {
		if i, err := strconv.Atoi(v); err == nil {
			cfg.Refresher.CatalogIntervalHours = i
		}
	}

	// Logging
	if v := os.Getenv("BTU_LOG_FILE"); v != "" {
		cfg.Logging.LogFile = v
	}
	if v := os.Getenv("BTU_LOG_LEVEL"); v != "" {
		cfg.Logging.MinLevel = strings.ToUpper(v)
	}
	if v := os.Getenv("BTU_LOG_BUFFER_SIZE"); v != "" {
		if i, err := strconv.Atoi(v); err == nil {
			cfg.Logging.BufferSize = i
		}
	}

	// Gemini
	if v := os.Getenv("GEMINI_API_KEY"); v != "" {
		cfg.Gemini.APIKey = v
	}
	if v := os.Getenv("GEMINI_MODEL"); v != "" {
		cfg.Gemini.Model = v
	}
}

func loadConfigFile(path string, target *Config) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	ext := strings.ToLower(filepath.Ext(path))
	switch ext {
	case ".json":
		return json.Unmarshal(data, target)
	case ".yaml", ".yml":
		return yaml.Unmarshal(data, target)
	default:
		// Try YAML first (which also decodes JSON cleanly)
		if err := yaml.Unmarshal(data, target); err == nil {
			return nil
		}
		return json.Unmarshal(data, target)
	}
}

// ModuleDelayDuration returns the module delay as time.Duration.
func (c *RefresherConfig) ModuleDelayDuration() time.Duration {
	if c.ModuleDelayMs <= 0 {
		return 1500 * time.Millisecond
	}
	return time.Duration(c.ModuleDelayMs) * time.Millisecond
}

// QISDelayDuration returns the QIS delay as time.Duration.
func (c *RefresherConfig) QISDelayDuration() time.Duration {
	if c.QISDelayMs <= 0 {
		return 4500 * time.Millisecond
	}
	return time.Duration(c.QISDelayMs) * time.Millisecond
}

// CatalogIntervalDuration returns the catalog scan interval as time.Duration.
func (c *RefresherConfig) CatalogIntervalDuration() time.Duration {
	if c.CatalogIntervalHours <= 0 {
		return 12 * time.Hour
	}
	return time.Duration(c.CatalogIntervalHours) * time.Hour
}
