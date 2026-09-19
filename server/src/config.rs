//! Configuration: flags with `BTU_*` environment variables, like the scraper service.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "btu-server", about = "Web server of the BTU catalog", version)]
pub struct Config {
    /// Address to listen on.
    #[arg(long, env = "BTU_WEB_ADDR", default_value = "127.0.0.1:8080")]
    pub addr: SocketAddr,

    /// The scraper's snapshot endpoint: the only interface between scraper and web server.
    #[arg(long, env = "BTU_SNAPSHOT_URL", default_value = "http://127.0.0.1:8090/snapshot/catalog.db")]
    pub snapshot_url: String,

    /// Where downloaded snapshots are kept, so a restart works while the scraper is down.
    #[arg(long, env = "BTU_WEB_DATA_DIR", default_value = "web-data")]
    pub data_dir: PathBuf,

    /// Seconds between checks for a new snapshot.
    #[arg(long, env = "BTU_SNAPSHOT_POLL", default_value_t = 60)]
    pub poll_seconds: u64,

    /// /healthz fails when the scraper could not be reached for this many seconds (0: never).
    #[arg(long, env = "BTU_SNAPSHOT_STALE_AFTER", default_value_t = 6 * 60 * 60)]
    pub stale_after_seconds: u64,

    /// Rendered pages kept in memory, in MiB (0 disables the cache).
    #[arg(long, env = "BTU_HTML_CACHE_MB", default_value_t = 128)]
    pub html_cache_mb: usize,

    /// Directory with the browser bundle (`pkg/`), served under `/pkg`. Optional until phase 2.
    #[arg(long, env = "BTU_SITE_ROOT", default_value = "site")]
    pub site_root: PathBuf,

    /// `text` or `json`.
    #[arg(long, env = "BTU_LOG_FORMAT", default_value = "text")]
    pub log_format: String,

    /// `error`, `warn`, `info`, `debug` or a tracing filter expression.
    #[arg(long, env = "BTU_LOG_LEVEL", default_value = "info")]
    pub log_level: String,
}

impl Config {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_seconds.max(5))
    }

    pub fn stale_after(&self) -> Option<Duration> {
        (self.stale_after_seconds > 0).then(|| Duration::from_secs(self.stale_after_seconds))
    }
}
