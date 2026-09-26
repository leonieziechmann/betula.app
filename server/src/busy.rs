//! How much the server does at once. A page that is not in the cache is rendered and a calendar
//! feed is made from the snapshot: each costs the processor tens of milliseconds, and each used
//! to start the moment it was asked for. A crawler asking for filtered lists faster than they
//! are made (measured 2026-09-26 on one processor: 45 a second, while Traefik lets one address
//! ask 50) built a queue that only grew: after half a minute every answer, `/livez` included,
//! took 6 to 13 seconds, and the container's healthcheck would have had it restarted with an
//! empty cache — the same crawler then meeting an even slower server.
//!
//! Now each kind of work has its places, one per worker thread of the runtime by default. A
//! request that finds them taken waits for one, at most `wait`, and is then answered `503` with
//! `Retry-After`: crawlers and calendar services come back later (Google slows its crawl on
//! 503s), and whoever waits never holds a thread. Cached pages, files and `/livez` need no
//! place and stay fast however long the queue is.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::sync::{Semaphore, SemaphorePermit};

/// What `x-cache` says on an answer the server turned away: the access log writes it as a
/// warning, not as an error a human has to act on.
pub const BUSY: &str = "busy";

pub struct Places {
    /// For the log: `render` or `calendar`.
    what: &'static str,
    places: Semaphore,
    count: usize,
    wait: Duration,
    waiting: AtomicUsize,
    /// Beyond this many waiting, a request is turned away at once: the queue would not be
    /// through within `wait` anyway.
    max_waiting: usize,
    turned_away: AtomicU64,
    reported: Mutex<Option<Instant>>,
}

impl Places {
    pub fn new(what: &'static str, count: usize, wait: Duration) -> Self {
        let count = count.max(1);
        Self {
            what,
            places: Semaphore::new(count),
            count,
            wait,
            waiting: AtomicUsize::new(0),
            max_waiting: count * 64,
            turned_away: AtomicU64::new(0),
            reported: Mutex::new(None),
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// A place, after waiting at most `wait` for one; `None` when the server is too busy.
    pub async fn enter(&self) -> Option<SemaphorePermit<'_>> {
        if let Ok(place) = self.places.try_acquire() {
            return Some(place);
        }
        if self.waiting.fetch_add(1, Ordering::SeqCst) >= self.max_waiting {
            self.waiting.fetch_sub(1, Ordering::SeqCst);
            self.turn_away();
            return None;
        }
        let place = tokio::time::timeout(self.wait, self.places.acquire()).await;
        self.waiting.fetch_sub(1, Ordering::SeqCst);
        match place {
            Ok(Ok(place)) => Some(place),
            _ => {
                self.turn_away();
                None
            }
        }
    }

    /// A place only while every place is free and nobody waits: for work nobody asked for (the
    /// warm-up of the cache), which must never keep a visitor waiting.
    pub fn enter_idle(&self) -> Option<SemaphorePermit<'_>> {
        if self.waiting.load(Ordering::SeqCst) > 0 || self.places.available_permits() < self.count {
            return None;
        }
        self.places.try_acquire().ok()
    }

    /// Counts a request that was turned away; says so in the log at most once a minute.
    fn turn_away(&self) {
        let total = self.turned_away.fetch_add(1, Ordering::Relaxed) + 1;
        let Ok(mut reported) = self.reported.lock() else { return };
        if reported.is_some_and(|at| at.elapsed() < Duration::from_secs(60)) {
            return;
        }
        *reported = Some(Instant::now());
        tracing::warn!(
            component = "http",
            event = "server.busy",
            what = self.what,
            places = self.count,
            wait_ms = self.wait.as_millis() as u64,
            turned_away = total,
            "every place was taken for longer than the wait: requests are answered 503 (turned_away counts since the start)"
        );
    }
}

/// The answer to a request the server has no place for: come back in `retry_after` seconds.
pub fn busy(retry_after: u64, page: bool) -> Response {
    let (content_type, body) = if page {
        (
            "text/html; charset=utf-8",
            "<!DOCTYPE html><html lang=\"de\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <meta name=\"robots\" content=\"noindex\"><title>Gerade viel los · Betula</title></head><body>\
             <h1>Gerade ist viel los</h1><p>Betula bekommt gerade mehr Anfragen, als es auf einmal beantworten kann. \
             Bitte lade die Seite in ein paar Sekunden neu.</p></body></html>\n",
        )
    } else {
        ("text/plain; charset=utf-8", "Betula ist gerade ausgelastet. Bitte später noch einmal versuchen.\n")
    };
    let mut response = (StatusCode::SERVICE_UNAVAILABLE, body).into_response();
    let out = response.headers_mut();
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    out.insert(header::RETRY_AFTER, HeaderValue::from(retry_after));
    out.insert("x-cache", HeaderValue::from_static(BUSY));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_request_waits_a_little_and_is_then_turned_away() {
        let places = Places::new("render", 1, Duration::from_millis(50));
        let first = places.enter().await;
        assert!(first.is_some());
        assert!(places.enter_idle().is_none(), "the warm-up takes no place while a visitor has one");
        let started = Instant::now();
        assert!(places.enter().await.is_none(), "the only place is taken");
        assert!(started.elapsed() >= Duration::from_millis(50));
        drop(first);
        assert!(places.enter().await.is_some());
        assert!(places.enter_idle().is_some());
    }

    #[tokio::test]
    async fn a_place_that_frees_up_goes_to_the_one_who_waits() {
        let places = std::sync::Arc::new(Places::new("calendar", 1, Duration::from_secs(5)));
        let first = places.enter().await;
        let waiter = {
            let places = places.clone();
            tokio::spawn(async move { places.enter().await.is_some() })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(first);
        assert!(waiter.await.unwrap_or(false));
    }
}
