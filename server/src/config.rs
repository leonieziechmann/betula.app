//! Configuration: flags with `FOLIA_*` environment variables, like Radix.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "folia", about = "Folia, the web server of Betula (catalog of BTU Cottbus-Senftenberg)", version)]
pub struct Config {
    /// Without a command the server runs.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Address to listen on.
    #[arg(long, env = "FOLIA_ADDR", default_value = "127.0.0.1:8080")]
    pub addr: SocketAddr,

    /// Radix's snapshot endpoint: the only interface between Radix and Folia.
    #[arg(long, env = "FOLIA_SNAPSHOT_URL", default_value = "http://127.0.0.1:8090/snapshot/catalog.db")]
    pub snapshot_url: String,

    /// Where downloaded snapshots are kept, so a restart works while Radix is down.
    #[arg(long, env = "FOLIA_DATA_DIR", default_value = "web-data")]
    pub data_dir: PathBuf,

    /// Seconds between checks for a new snapshot.
    #[arg(long, env = "FOLIA_SNAPSHOT_POLL", default_value_t = 60)]
    pub poll_seconds: u64,

    /// /healthz fails when Radix could not be reached for this many seconds (0: never).
    #[arg(long, env = "FOLIA_SNAPSHOT_STALE_AFTER", default_value_t = 6 * 60 * 60)]
    pub stale_after_seconds: u64,

    /// Rendered pages kept in memory, in MiB (0 disables the cache).
    #[arg(long, env = "FOLIA_HTML_CACHE_MB", default_value_t = 128)]
    pub html_cache_mb: usize,

    /// Worker threads of the runtime. 0: one more than the processors the container may use (its
    /// CPU limit), so that one is free for connections while every processor renders.
    #[arg(long, env = "FOLIA_WORKERS", default_value_t = 0)]
    pub workers: usize,

    /// Pages rendered at the same time; a page that finds every place taken waits (`busy`). 0: one
    /// per processor the container may use.
    #[arg(long, env = "FOLIA_RENDER_PLACES", default_value_t = 0)]
    pub render_places: usize,

    /// How long a page waits for a place to be rendered in before it is answered 503 with
    /// `Retry-After`, in milliseconds.
    #[arg(long, env = "FOLIA_RENDER_WAIT_MS", default_value_t = 3000)]
    pub render_wait_ms: u64,

    /// Calendar feeds made at the same time (0: one per processor); a feed waits for a place at
    /// most ten seconds.
    #[arg(long, env = "FOLIA_FEED_PLACES", default_value_t = 0)]
    pub feed_places: usize,

    /// After each new snapshot, render every page of the sitemap into the cache while the server
    /// is idle (`on`/`off`).
    #[arg(long, env = "FOLIA_WARM_CACHE", default_value_t = true, num_args = 0..=1, default_missing_value = "true", action = clap::ArgAction::Set, value_parser = clap::builder::BoolishValueParser::new())]
    pub warm_cache: bool,

    /// Finished link-preview cards (`/cards/…png`) kept in memory, in MiB.
    #[arg(long, env = "FOLIA_CARD_CACHE_MB", default_value_t = 64)]
    pub card_cache_mb: usize,

    /// Directory with the browser bundle (`pkg/`), served under `/pkg`. Optional until phase 2.
    #[arg(long, env = "FOLIA_SITE_ROOT", default_value = "site")]
    pub site_root: PathBuf,

    /// The address of the site as the world sees it: canonical links, link previews and the
    /// sitemap are written with it.
    #[arg(long, env = "FOLIA_PUBLIC_URL", default_value = app::seo::DEFAULT_SITE_URL)]
    pub public_url: String,

    /// Closed testing: the whole site asks for one shared password (`on`/`off`). The password is a
    /// secret and never a flag: a Docker secret or systemd credential named `folia-access-password`,
    /// the file named by `FOLIA_ACCESS_PASSWORD_FILE`, or `FOLIA_ACCESS_PASSWORD` (src/access.rs).
    #[arg(long, env = "FOLIA_ACCESS_GATE", default_value_t = false, num_args = 0..=1, default_missing_value = "true", value_parser = clap::builder::BoolishValueParser::new())]
    pub access_gate: bool,

    /// Search engines may list the site (`on`/`off`): `robots.txt` names the sitemap, and no
    /// answer says `noindex`. Off (the default: a site is listed only when somebody says so),
    /// every answer carries `X-Robots-Tag: noindex, nofollow` and `robots.txt` names no sitemap.
    /// The access gate keeps crawlers out either way.
    #[arg(long, env = "FOLIA_INDEXING", default_value_t = false, num_args = 0..=1, default_missing_value = "true", value_parser = clap::builder::BoolishValueParser::new())]
    pub indexing: bool,

    /// `text` or `json`.
    #[arg(long, env = "FOLIA_LOG_FORMAT", default_value = "text")]
    pub log_format: String,

    /// `error`, `warn`, `info`, `debug` or a tracing filter expression.
    #[arg(long, env = "FOLIA_LOG_LEVEL", default_value = "info")]
    pub log_level: String,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum Command {
    /// The probe of a container's HEALTHCHECK (like `radix healthcheck`): asks the server that
    /// listens on `FOLIA_ADDR` for `/livez` and exits with 0 when it answers. An image built with
    /// Nix has no curl or wget to do that.
    Healthcheck,
}

impl Config {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_seconds.max(5))
    }

    pub fn stale_after(&self) -> Option<Duration> {
        (self.stale_after_seconds > 0).then(|| Duration::from_secs(self.stale_after_seconds))
    }
}
