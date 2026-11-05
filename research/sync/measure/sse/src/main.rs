//! Idle server-sent-event connections: what each costs in memory (axum 0.8 + tokio, as Folia).
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{extract::State, routing::get, Router};
use futures::stream::{self, Stream};
use tokio::sync::broadcast;

#[derive(Clone)]
struct App {
    tx: broadcast::Sender<u64>,
    open: Arc<AtomicUsize>,
}

struct Guard(Arc<AtomicUsize>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

async fn events(State(app): State<App>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    app.open.fetch_add(1, Ordering::Relaxed);
    let guard = Guard(app.open.clone());
    let rx = app.tx.subscribe();
    let s = stream::unfold((rx, guard), |(mut rx, guard)| async move {
        match rx.recv().await {
            Ok(v) => Some((Ok(Event::default().data(v.to_string())), (rx, guard))),
            Err(_) => None,
        }
    });
    Sse::new(s).keep_alive(KeepAlive::new().interval(Duration::from_secs(25)))
}

#[tokio::main(worker_threads = 2)]
async fn main() {
    let (tx, _) = broadcast::channel(16);
    let app = App { tx, open: Arc::new(AtomicUsize::new(0)) };
    let open = app.open.clone();
    let router = Router::new().route("/events", get(events)).with_state(app);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let rss = std::fs::read_to_string("/proc/self/status").unwrap_or_default()
                .lines().find(|l| l.starts_with("VmRSS")).map(|l| l.to_string()).unwrap_or_default();
            eprintln!("open={} {}", open.load(Ordering::Relaxed), rss);
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18777").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
