//! The snapshot client: keeps the newest catalog snapshot of Radix on disk and active.
//!
//! GET with `If-None-Match` → 304 (nothing to do) or 200 (download to a temporary file, check
//! that it opens and answers the queries of the landing page, compress it once for browsers,
//! then switch). A snapshot that fails the check is rejected and the previous one stays active.
//! When Radix is down the last good snapshot keeps being served, also across restarts.
//!
//! Browsers download it compressed: gzip, made before the switch (a second), and brotli, made
//! after it in the background (`compress_active`: a minute and a half of one processor, and 4.4 MB
//! where gzip takes 7.6). Both are kept beside the file for the next start.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use catalog::native::NativeDatabase;
use catalog::rows::Meta;
use catalog::{Database, DbError};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::encoding::Kept;

const POINTER: &str = "current.json";
const POOL_SIZE: usize = 8;

/// One snapshot file, opened read-only by a small pool of connections.
pub struct Snapshot {
    /// Radix's ETag (content hash), passed on to browsers unchanged.
    pub etag: String,
    pub path: PathBuf,
    pub bytes: u64,
    /// The schema of the file (`PRAGMA user_version`, the number of Radix's last migration).
    /// `/api/status` names it, so that a browser does not download a copy that is older than
    /// the one its build reads (`catalog::SCHEMA_VERSION`).
    pub schema_version: i64,
    /// The same file gzip-compressed, made once per snapshot.
    pub gzip: Option<(PathBuf, u64)>,
    /// That file in memory (7.6 MB in September 2026): `/api/db` hands every browser the same
    /// bytes, instead of a file read and a buffer of its own per download — a lecture hall
    /// opening the app at once used to mean hundreds of both.
    pub gzip_bytes: Option<Bytes>,
    /// The same file in brotli, in memory like gzip (4.4 MB): read from beside the file, or made
    /// in the background once the snapshot is active (`compress_active`). Browsers get gzip until
    /// it is there.
    pub brotli: OnceLock<Bytes>,
    pub meta: Meta,
    pub activated_at: SystemTime,
    /// The map of the programs on the landing page, laid out once when the snapshot is opened
    /// (pages and `/api/map.json` only hand it on), with its JSON (compressed when first asked
    /// for) and its own ETag. The ETag is the content's, not the snapshot's: a new layout of the
    /// same catalog (a new Folia) must not be answered with „304, unchanged" from a browser's cache.
    pub program_map: Option<(Arc<catalog::graph::ProgramMap>, Kept, String)>,
    /// What the pickers of the catalog offer (every program, department and person), made once
    /// here instead of in every render of a page of the catalog (`app::pages::catalog`).
    pub pickers: Option<app::pages::catalog::PickerChoices>,
    /// The data of the program overview, the same for each of its filters (`app::pages::programs`).
    pub programs: Option<app::pages::programs::ProgramsReady>,
    /// `/sitemap.xml` as made on first request: the round of the warm-up whose dates it names
    /// (`lastmod::Changes::rounds`; made anew after the next), its ETag, and its XML.
    pub sitemap: Mutex<Option<(u64, String, Arc<Kept>)>>,
    pool: Mutex<Vec<NativeDatabase>>,
}

/// A strong ETag from the bytes themselves (FNV-1a, 64 bit: a fingerprint, not a secret), named by
/// what it is for (`map`, `ics`), so tags of two kinds of answer never meet.
pub(crate) fn content_etag(prefix: &str, bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3));
    format!("\"{prefix}-{hash:016x}\"")
}

impl Snapshot {
    fn open(path: PathBuf, etag: String) -> Result<Self, DbError> {
        let db = NativeDatabase::open(&path)?;
        let schema_version = db.schema_version()?;
        // The queries of the landing page touch modules, programs, semesters and meta.
        let overview = catalog::pages::overview(&db)?;
        if overview.modules == 0 || overview.programs == 0 {
            return Err(DbError::Unavailable(format!("{}: the catalog is empty", path.display())));
        }
        // A snapshot without a map is still a catalog: the landing page leaves the section out.
        let started = Instant::now();
        let program_map = match catalog::pages::program_map(&db).map_err(|e| e.to_string()).and_then(|map| serde_json::to_vec(&map).map(|json| (map, json)).map_err(|e| e.to_string())) {
            Ok((map, json)) => {
                tracing::info!(component = "snapshot", event = "snapshot.map_built", programs = map.programs.len(), links = map.links.len(), ms = started.elapsed().as_millis() as u64, "program map laid out");
                let etag = content_etag("map", &json);
                Some((Arc::new(map), Kept::new(Bytes::from(json)), etag))
            }
            Err(error) => {
                tracing::warn!(component = "snapshot", event = "snapshot.map_failed", error = %error, "the program map could not be built; the landing page goes without it");
                None
            }
        };
        let pickers = match catalog::pages::catalog_choices(&db) {
            Ok(choices) => Some(app::pages::catalog::PickerChoices::of(&choices)),
            Err(error) => {
                tracing::warn!(component = "snapshot", event = "snapshot.choices_failed", error = %error, "the pickers of the catalog are loaded per page");
                None
            }
        };
        let programs = catalog::pages::programs_overview(&db).ok().map(|data| app::pages::programs::ProgramsReady(Arc::new(data)));
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let gzip_path = beside(&path, GZIP);
        let gzip_bytes = std::fs::read(&gzip_path).ok().map(Bytes::from);
        let gzip = gzip_bytes.as_ref().map(|bytes| (gzip_path, bytes.len() as u64));
        let brotli = OnceLock::new();
        if let Some(made) = std::fs::read(beside(&path, BROTLI)).ok().filter(|made| !made.is_empty()) {
            let _ = brotli.set(Bytes::from(made));
        }
        Ok(Self {
            etag,
            path,
            bytes,
            schema_version,
            gzip,
            gzip_bytes,
            brotli,
            meta: overview.meta,
            activated_at: SystemTime::now(),
            program_map,
            pickers,
            programs,
            sitemap: Mutex::new(None),
            pool: Mutex::new(vec![db]),
        })
    }

    pub fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError> {
        let pooled = self.pool.lock().ok().and_then(|mut pool| pool.pop());
        let db = match pooled {
            Some(db) => db,
            None => NativeDatabase::open(&self.path)?,
        };
        job(&db);
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < POOL_SIZE {
                pool.push(db);
            }
        }
        Ok(())
    }
}

/// The endings of the compressed copies beside a snapshot file.
const GZIP: &str = ".gz";
const BROTLI: &str = ".br";

/// `path` with `ending` added: where a compressed copy of the file lies.
fn beside(path: &Path, ending: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(ending);
    PathBuf::from(name)
}

#[derive(Serialize, Deserialize)]
struct Pointer {
    file: String,
    etag: String,
}

/// The active snapshot and what is known about the link to Radix.
pub struct SnapshotStore {
    data_dir: PathBuf,
    current: RwLock<Option<Arc<Snapshot>>>,
    /// Changes with every activation; the HTML cache is only valid within one generation.
    generation: AtomicU64,
    /// Unix seconds of the last answer from Radix (200 or 304); 0 = never.
    last_contact: AtomicU64,
    started: Instant,
}

impl SnapshotStore {
    pub fn new(data_dir: PathBuf) -> std::io::Result<Arc<Self>> {
        std::fs::create_dir_all(&data_dir)?;
        Ok(Arc::new(Self {
            data_dir,
            current: RwLock::new(None),
            generation: AtomicU64::new(0),
            last_contact: AtomicU64::new(0),
            started: Instant::now(),
        }))
    }

    pub fn current(&self) -> Option<Arc<Snapshot>> {
        self.current.read().ok().and_then(|current| current.clone())
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Seconds since Radix last answered; `None` if it never did in this process.
    pub fn seconds_since_contact(&self) -> Option<u64> {
        match self.last_contact.load(Ordering::Relaxed) {
            0 => None,
            at => Some(unix_now().saturating_sub(at)),
        }
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    fn activate(&self, snapshot: Snapshot) {
        let etag = snapshot.etag.clone();
        let bytes = snapshot.bytes;
        let schema_version = snapshot.schema_version;
        let data_changed_at = snapshot.meta.data_changed_at.clone().unwrap_or_default();
        let keep = snapshot.path.clone();
        if let Ok(mut current) = self.current.write() {
            *current = Some(Arc::new(snapshot));
        }
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::info!(component = "snapshot", event = "snapshot.activated", etag = %etag, bytes, schema_version, data_changed_at = %data_changed_at, generation, "snapshot active");
        // Served all the same: the pages that do not need the newer columns work, and a refused
        // snapshot would leave the server without any data after a restart where Radix does not
        // export again (RADIX_CRAWL=off).
        if schema_version < catalog::SCHEMA_VERSION {
            tracing::error!(component = "snapshot", event = "snapshot.outdated", etag = %etag, schema_version, needs = catalog::SCHEMA_VERSION, "the snapshot is older than the schema this build reads: pages that need the newer columns fail, and browsers do not start the app on it until Radix exports a new one");
        }
        self.remove_other_files(&keep);
    }

    /// Reactivates the snapshot of the previous run. Returns false if there is none.
    pub fn restore(&self) -> bool {
        let pointer_path = self.data_dir.join(POINTER);
        let Ok(text) = std::fs::read_to_string(&pointer_path) else { return false };
        let pointer: Pointer = match serde_json::from_str(&text) {
            Ok(pointer) => pointer,
            Err(error) => {
                tracing::warn!(component = "snapshot", event = "snapshot.restore_failed", error = %error, "unreadable snapshot pointer; waiting for Radix");
                return false;
            }
        };
        match Snapshot::open(self.data_dir.join(&pointer.file), pointer.etag) {
            Ok(snapshot) => {
                tracing::info!(component = "snapshot", event = "snapshot.restored", file = %pointer.file, "using the snapshot of the previous run");
                self.activate(snapshot);
                true
            }
            Err(error) => {
                tracing::warn!(component = "snapshot", event = "snapshot.restore_failed", error = %error, "stored snapshot is unusable; waiting for Radix");
                false
            }
        }
    }

    /// Old snapshot files. One that is still open (Windows) stays until the next attempt.
    fn remove_other_files(&self, keep: &Path) {
        let kept = [keep.to_path_buf(), beside(keep, GZIP), beside(keep, BROTLI)];
        let Ok(entries) = std::fs::read_dir(&self.data_dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let ours = name.starts_with("catalog-") || name.starts_with("download-");
            if ours && !kept.contains(&path) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub enum Sync {
    Unchanged,
    Activated,
}

#[derive(Debug)]
pub enum SyncError {
    /// Radix did not answer or answered with an error: keep serving what we have.
    Fetch(String),
    /// Radix has no export yet (503).
    NotReady,
    /// The download is not a usable catalog: needs a human.
    Rejected(String),
    Io(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Fetch(why) => write!(f, "fetch failed: {why}"),
            SyncError::NotReady => write!(f, "Radix has not exported a snapshot yet"),
            SyncError::Rejected(why) => write!(f, "snapshot rejected: {why}"),
            SyncError::Io(why) => write!(f, "cannot store the snapshot: {why}"),
        }
    }
}

/// One check against Radix.
pub async fn sync_once(store: &Arc<SnapshotStore>, client: &reqwest::Client, url: &str) -> Result<Sync, SyncError> {
    let current = store.current();
    let mut request = client.get(url);
    if let Some(snapshot) = &current {
        request = request.header(reqwest::header::IF_NONE_MATCH, &snapshot.etag);
    }
    let response = request.send().await.map_err(|e| SyncError::Fetch(e.to_string()))?;

    match response.status() {
        reqwest::StatusCode::NOT_MODIFIED => {
            store.last_contact.store(unix_now(), Ordering::Relaxed);
            return Ok(Sync::Unchanged);
        }
        reqwest::StatusCode::SERVICE_UNAVAILABLE => return Err(SyncError::NotReady),
        status if !status.is_success() => return Err(SyncError::Fetch(format!("HTTP {status}"))),
        _ => {}
    }
    store.last_contact.store(unix_now(), Ordering::Relaxed);

    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| SyncError::Rejected("the response has no ETag".to_string()))?;
    if current.as_ref().is_some_and(|snapshot| snapshot.etag == etag) {
        return Ok(Sync::Unchanged);
    }

    // Download next to the final place, so the rename stays on one volume.
    let io = |e: std::io::Error| SyncError::Io(e.to_string());
    let started = Instant::now();
    let temporary = store.data_dir.join(format!("download-{}.tmp", unix_now()));
    let mut file = tokio::fs::File::create(&temporary).await.map_err(io)?;
    let mut stream = response.bytes_stream();
    let mut bytes: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                drop(file);
                let _ = tokio::fs::remove_file(&temporary).await;
                return Err(SyncError::Fetch(format!("download interrupted after {bytes} bytes: {error}")));
            }
        };
        bytes += chunk.len() as u64;
        file.write_all(&chunk).await.map_err(io)?;
    }
    file.flush().await.map_err(io)?;
    drop(file);
    tracing::info!(component = "snapshot", event = "snapshot.downloaded", etag = %etag, bytes, ms = started.elapsed().as_millis() as u64, "snapshot downloaded");

    // File names carry the content hash, like Radix's own export.
    let hash: String = etag.chars().filter(|c| c.is_ascii_alphanumeric()).take(16).collect();
    let file_name = format!("catalog-{hash}.db");
    let target = store.data_dir.join(&file_name);
    let data_dir = store.data_dir.clone();
    let prepared = tokio::task::spawn_blocking(move || -> Result<Snapshot, SyncError> {
        let reject = |why: String| {
            let _ = std::fs::remove_file(&temporary);
            SyncError::Rejected(why)
        };
        if let Err(error) = NativeDatabase::open(&temporary) {
            return Err(reject(error.to_string()));
        }
        let _ = std::fs::remove_file(&target);
        std::fs::rename(&temporary, &target).map_err(|e| SyncError::Io(e.to_string()))?;

        // Browsers download this file: 44 MB raw, 7.6 MB in gzip. Compress it once here instead
        // of per request; brotli follows once it is active (`compress_active`).
        let gzip_path = beside(&target, GZIP);
        if let Err(error) = gzip_file(&target, &gzip_path) {
            tracing::warn!(component = "snapshot", event = "snapshot.compress_failed", error = %error, "serving the snapshot uncompressed");
            let _ = std::fs::remove_file(&gzip_path);
        }

        let snapshot = Snapshot::open(target.clone(), etag.clone()).map_err(|e| {
            let _ = std::fs::remove_file(&target);
            let _ = std::fs::remove_file(&gzip_path);
            SyncError::Rejected(e.to_string())
        })?;
        let pointer = serde_json::to_string(&Pointer { file: file_name, etag }).map_err(|e| SyncError::Io(e.to_string()))?;
        std::fs::write(data_dir.join(POINTER), pointer).map_err(|e| SyncError::Io(e.to_string()))?;
        Ok(snapshot)
    })
    .await
    .map_err(|e| SyncError::Io(format!("the snapshot check did not finish: {e}")))??;

    store.activate(prepared);
    Ok(Sync::Activated)
}

fn gzip_file(source: &Path, target: &Path) -> std::io::Result<()> {
    let mut input = std::io::BufReader::new(std::fs::File::open(source)?);
    let output = std::io::BufWriter::new(std::fs::File::create(target)?);
    let mut encoder = flate2::write::GzEncoder::new(output, flate2::Compression::new(6));
    std::io::copy(&mut input, &mut encoder)?;
    encoder.finish()?.flush()
}

/// Makes the brotli copy of the active snapshot, when it has none (`Snapshot::brotli`): off the
/// threads that answer requests, at `quality`, into a file beside the snapshot (through a
/// temporary one, so that a crash leaves nothing half written for the next start), and into
/// memory. `run` makes it at brotli's best: measured 2026-09-30, 44 MB into 4.4 MB (gzip: 7.6 MB)
/// in 87 s of one processor, with about 120 MB of memory for the while. True when the active
/// snapshot has its copy now.
pub async fn compress_active(store: &SnapshotStore, quality: u32) -> bool {
    let Some(snapshot) = store.current() else { return false };
    if snapshot.brotli.get().is_some() {
        return true;
    }
    let started = Instant::now();
    let (source, target) = (snapshot.path.clone(), beside(&snapshot.path, BROTLI));
    let size = usize::try_from(snapshot.bytes).unwrap_or(usize::MAX);
    let made = {
        let target = target.clone();
        tokio::task::spawn_blocking(move || brotli_file(&source, &target, size, quality)).await
    };
    let error = match made {
        // Replaced meanwhile: its files went with the switch, and this one goes too.
        Ok(Ok(_)) if !store.current().is_some_and(|active| Arc::ptr_eq(&active, &snapshot)) => {
            let _ = std::fs::remove_file(&target);
            return false;
        }
        Ok(Ok(bytes)) => {
            tracing::info!(component = "snapshot", event = "snapshot.compressed", etag = %snapshot.etag, bytes = bytes.len(), ms = started.elapsed().as_millis() as u64, "the snapshot goes out in brotli");
            let _ = snapshot.brotli.set(bytes);
            return true;
        }
        Ok(Err(error)) => error.to_string(),
        Err(error) => error.to_string(),
    };
    tracing::warn!(component = "snapshot", event = "snapshot.compress_failed", error = %error, "the snapshot goes out in gzip, not in brotli");
    false
}

/// `source` in brotli at `quality`, written to `target` through a temporary file.
fn brotli_file(source: &Path, target: &Path, size: usize, quality: u32) -> std::io::Result<Bytes> {
    let mut compressed = Vec::with_capacity(size / 8);
    crate::encoding::brotli_stream(&mut std::io::BufReader::new(std::fs::File::open(source)?), &mut compressed, quality, size)?;
    let temporary = beside(target, ".tmp");
    let written = write_synced(&temporary, &compressed).and_then(|()| std::fs::rename(&temporary, target));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written.map(|()| Bytes::from(compressed))
}

/// `bytes` into a new file at `path`, on the disk before this returns.
fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Polls Radix until the process ends. Failures back off up to five minutes.
pub async fn run(store: Arc<SnapshotStore>, url: String, interval: Duration, stale_after: Option<Duration>) {
    let client = match reqwest::Client::builder().connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(600)).build() {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(component = "snapshot", event = "snapshot.client_failed", error = %error, "cannot create the HTTP client; snapshots will not update");
            return;
        }
    };
    tracing::info!(component = "snapshot", event = "snapshot.sync_started", url = %url, interval_s = interval.as_secs(), "watching Radix's snapshot endpoint");

    let mut failures: u32 = 0;
    // The snapshot whose brotli could not be made: not tried again and again.
    let mut uncompressed: Option<String> = None;
    loop {
        let wait = match sync_once(&store, &client, &url).await {
            Ok(Sync::Unchanged) => {
                tracing::debug!(component = "snapshot", event = "snapshot.unchanged", "snapshot unchanged");
                if failures > 0 {
                    tracing::info!(component = "snapshot", event = "snapshot.sync_recovered", after_failures = failures, "Radix answers again");
                }
                failures = 0;
                interval
            }
            Ok(Sync::Activated) => {
                failures = 0;
                interval
            }
            Err(error) => {
                failures += 1;
                let without_snapshot = store.current().is_none();
                let stale = stale_after.is_some_and(|limit| match store.seconds_since_contact() {
                    Some(seconds) => seconds > limit.as_secs(),
                    None => store.uptime() > limit,
                });
                // ERROR means a human has to act: a rejected snapshot, or the link has been down for too long.
                if matches!(error, SyncError::Rejected(_) | SyncError::Io(_)) {
                    tracing::error!(component = "snapshot", event = "snapshot.rejected", error = %error, "keeping the previous snapshot");
                } else if stale {
                    tracing::error!(component = "snapshot", event = "snapshot.stale", error = %error, failures, without_snapshot, "no answer from Radix for longer than the configured limit");
                } else {
                    tracing::warn!(component = "snapshot", event = "snapshot.fetch_failed", error = %error, failures, without_snapshot, "will retry");
                }
                // Without any snapshot the site is down: retry quickly. Otherwise back off.
                if without_snapshot {
                    Duration::from_secs(5)
                } else {
                    (interval * 2u32.saturating_pow(failures.min(4))).min(Duration::from_secs(300))
                }
            }
        };
        // The brotli of the active snapshot, one that came now or was restored without it: before
        // the next look at Radix, which it only delays.
        if let Some(active) = store.current().filter(|active| active.brotli.get().is_none() && uncompressed.as_deref() != Some(active.etag.as_str())) {
            if !compress_active(&store, crate::encoding::BEST).await && store.current().is_some_and(|now| Arc::ptr_eq(&now, &active)) {
                uncompressed = Some(active.etag.clone());
            }
        }
        tokio::time::sleep(wait).await;
    }
}
