//! Where a page gets its data from.
//!
//! The host provides a `Source` through context: the web server a pool of rusqlite
//! connections on the active snapshot, the browser the downloaded snapshot in sql.js.
//! Pages run the loaders of `folia_pages` through it and never see the difference.

use std::sync::Arc;

use folia_model::{Database, DbError};
use folia_pages::ask::{Ask, Kept, Lane};
use leptos::prelude::*;

pub trait CatalogSource: Send + Sync {
    /// Runs `job` with a database on one snapshot, so everything a page loads is consistent.
    fn with_db(&self, job: &mut dyn FnMut(&dyn Database)) -> Result<(), DbError>;
}

#[derive(Clone)]
pub struct Source(pub Arc<dyn CatalogSource>);

/// The map of the programs on the landing page (`folia_pages::graph`). The web server lays it out
/// once per snapshot and hands it to the pages it renders; the browser app gets the same map as
/// `/api/map.json` (`boot.js`). Nobody computes it while a page renders; a host without a map
/// simply provides none and the page leaves the section out.
#[derive(Clone)]
pub struct ProgramMapHandle(pub Arc<folia_pages::graph::ProgramMap>);

/// One module the semantic search found: its id and how close its description is to the query
/// (the cosine of their vectors, higher is closer; folia/crates/semantic/README.md).
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticHit {
    pub module_id: String,
    pub score: f32,
}

/// What a search answers later: the semantic search runs in a Web Worker.
pub type Later<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T>>>;

/// The semantic search (folia/crates/semantic/README.md): the modules whose descriptions mean what a query
/// says, the catalog's „Ähnliche Module" under the results of a search. Only the browser app has
/// it (`client`, the model in a Web Worker that `boot.js` loads once the app runs); on the server,
/// and in a browser that has none, there is no `Semantic` in the context.
pub trait SemanticSearch: Send + Sync {
    /// Whether the search can answer: false for good when this browser has none (no model on the
    /// server, no vectors in the catalog yet, data saving, little memory). Resolves once loading
    /// has ended either way; until then a search waits.
    fn ready(&self) -> Later<bool>;
    /// The `k` modules closest to `query`, best first. `None` when a newer query took its place
    /// before this one ran (typing fast never piles up work), or when there is no search.
    fn search(&self, query: &str, k: usize) -> Later<Option<Vec<SemanticHit>>>;
}

#[derive(Clone)]
pub struct Semantic(pub Arc<dyn SemanticSearch>);

pub use folia_pages::ask::DataError;

impl Source {
    pub fn run<T>(&self, load: impl FnOnce(&dyn Database) -> Result<T, DbError>) -> Result<T, DataError> {
        let mut load = Some(load);
        let mut outcome = None;
        self.0.with_db(&mut |db| {
            if let Some(load) = load.take() {
                outcome = Some(load(db));
            }
        })?;
        match outcome {
            Some(result) => Ok(result?),
            None => Err(DbError::Unavailable("the data source did not run the query".to_string()).into()),
        }
    }
}

/// The source the host provided. Call it in the component body, not inside a future.
pub fn use_source() -> Result<Source, DataError> {
    use_context::<Source>()
        .ok_or_else(|| DbError::Unavailable("no data source was provided".to_string()).into())
}

/// Where the answers come from when the client has none at hand: the data worker (the browser,
/// `client`), asked a question by its name and its fields in JSON; the answer is the JSON of a
/// `Result<Answer, DataError>`, an empty text where a newer question of the same lane took the
/// place of this one before it ran, and `None` where none came (the worker went away).
pub trait Answerer: Send + Sync {
    fn ask(&self, name: &'static str, lane: Lane, question: String) -> Later<Option<String>>;
}

enum Backend {
    /// The host's `Source` on this thread (the server; the browser until the worker answers),
    /// with what the answering side keeps besides answers (the finder's candidates).
    Local(Source, std::sync::Mutex<Kept>),
    Remote(Arc<dyn Answerer>),
}

/// What a question that a newer one of its lane replaced is answered.
const DROPPED: &str = "a newer question took its place";

/// How many answers a client keeps (the oldest goes); the answers of a visit's pages are a few
/// hundred kilobytes each at most.
const KEPT_ANSWERS: usize = 160;

struct Inner {
    backend: Backend,
    /// Answers by their question (`Ask::key`), with when each was last used.
    answers: std::sync::Mutex<(std::collections::HashMap<String, (Arc<dyn std::any::Any + Send + Sync>, u64)>, u64)>,
    /// Questions on their way, each with what tells those that asked it that its answer is there.
    asked: std::sync::Mutex<std::collections::HashMap<String, ArcTrigger>>,
    /// Counts the snapshots: every answer of the one before is forgotten, and what showed one
    /// asks again (`forget`).
    generation: ArcRwSignal<u64>,
    /// How many questions are on their way (the takeover and a change of page wait for none).
    waiting: ArcRwSignal<usize>,
}

/// Where a page asks its questions (`folia_pages::ask`, docs/folia/folia-refactor.md §6.4): every
/// page and component asks through it, never a `Database` itself. It keeps the answers by their
/// question until the snapshot changes. A local client answers at once; a remote one (the data
/// worker) later, and what asked is told when the answer is there.
#[derive(Clone)]
pub struct DataClient(Arc<Inner>);

impl DataClient {
    /// A client that answers from `source`, on this thread.
    pub fn new(source: Source) -> Self {
        Self::with(Backend::Local(source, std::sync::Mutex::new(Kept::default())))
    }

    /// A client that asks `answerer`.
    pub fn remote(answerer: Arc<dyn Answerer>) -> Self {
        Self::with(Backend::Remote(answerer))
    }

    fn with(backend: Backend) -> Self {
        Self(Arc::new(Inner {
            backend,
            answers: std::sync::Mutex::new((std::collections::HashMap::new(), 0)),
            asked: std::sync::Mutex::new(std::collections::HashMap::new()),
            generation: ArcRwSignal::new(0),
            waiting: ArcRwSignal::new(0),
        }))
    }

    fn kept<A: Ask>(&self, key: &str) -> Option<Result<A::Answer, DataError>> {
        let mut answers = self.0.answers.lock().ok()?;
        let (map, clock) = &mut *answers;
        *clock += 1;
        let (answer, used) = map.get_mut(key)?;
        *used = *clock;
        answer.downcast_ref::<Result<A::Answer, DataError>>().cloned()
    }

    /// Keeps an answer, a failed one too: what asked it shows the failure rather than asking
    /// again at once (a worker that went away would be asked without end). The next snapshot
    /// (`forget`) asks everything again.
    fn keep<A: Ask>(&self, key: String, answer: &Result<A::Answer, DataError>) {
        let Ok(mut answers) = self.0.answers.lock() else { return };
        let (map, clock) = &mut *answers;
        *clock += 1;
        if map.len() >= KEPT_ANSWERS && !map.contains_key(&key) {
            if let Some(oldest) = map.iter().min_by_key(|(_, (_, used))| *used).map(|(key, _)| key.clone()) {
                map.remove(&oldest);
            }
        }
        map.insert(key, (Arc::new(answer.clone()), *clock));
    }

    /// The answer, on this thread: `None` for a remote client.
    fn local<A: Ask>(&self, ask: &A) -> Option<Result<A::Answer, DataError>> {
        let Backend::Local(source, kept) = &self.0.backend else { return None };
        Some(source.run(|db| match kept.lock() {
            Ok(mut kept) => ask.run(db, &mut kept),
            Err(_) => ask.run(db, &mut Kept::default()),
        }))
    }

    /// The answer to `ask` if it is at hand (kept, or a local client's), else `None`, and then the
    /// question is on its way: whatever read this in a reactive scope runs again once the answer
    /// is there. Never waits.
    #[track_caller]
    pub fn get<A: Ask>(&self, ask: &A) -> Option<Result<A::Answer, DataError>> {
        self.0.generation.track();
        let key = ask.key();
        if let Some(answer) = self.kept::<A>(&key) {
            return Some(answer);
        }
        if let Some(answer) = self.local(ask) {
            self.keep::<A>(key, &answer);
            return Some(answer);
        }
        self.send(ask.clone(), key).track();
        None
    }

    /// The answer to `ask`, when it is there.
    pub async fn ask<A: Ask>(&self, ask: A) -> Result<A::Answer, DataError> {
        let key = ask.key();
        if let Some(answer) = self.kept::<A>(&key) {
            return answer;
        }
        if let Some(answer) = self.local(&ask) {
            self.keep::<A>(key, &answer);
            return answer;
        }
        let answer = self.from_remote(&ask).await;
        self.keep::<A>(key, &answer);
        answer
    }

    /// The answer at hand or none, without asking: what a page shows while it waits.
    pub fn peek<A: Ask>(&self, ask: &A) -> Option<Result<A::Answer, DataError>> {
        self.kept::<A>(&ask.key()).or_else(|| self.local(ask))
    }

    /// The answer to `ask`, now: kept, or worked out on this thread. A remote client that has
    /// none answers that it is on its way (`DataError::pending`), and asks for it meanwhile:
    /// what read this in a reactive scope runs again once the answer is there.
    #[track_caller]
    pub fn now<A: Ask>(&self, ask: &A) -> Result<A::Answer, DataError> {
        self.get(ask).unwrap_or_else(|| Err(DataError::pending()))
    }

    async fn from_remote<A: Ask>(&self, ask: &A) -> Result<A::Answer, DataError> {
        let Backend::Remote(answerer) = &self.0.backend else { return Err(DataError { unavailable: true, message: "no answerer".to_string() }) };
        let question = serde_json::to_string(ask).map_err(|error| DataError { unavailable: false, message: error.to_string() })?;
        match answerer.ask(A::NAME, A::LANE, question).await {
            Some(json) if json.is_empty() => Err(DataError { unavailable: true, message: DROPPED.to_string() }),
            Some(json) => serde_json::from_str(&json).unwrap_or_else(|error| Err(DataError { unavailable: false, message: format!("{}: {error}", A::NAME) })),
            None => Err(DataError { unavailable: true, message: "no answer came".to_string() }),
        }
    }

    /// Sends `ask` to the remote side, once while it is on its way; what tells that its answer is
    /// there.
    fn send<A: Ask>(&self, ask: A, key: String) -> ArcTrigger {
        let (trigger, first) = match self.0.asked.lock() {
            Ok(mut asked) => match asked.get(&key) {
                Some(trigger) => (trigger.clone(), false),
                None => {
                    let trigger = ArcTrigger::new();
                    asked.insert(key.clone(), trigger.clone());
                    (trigger, true)
                }
            },
            Err(_) => return ArcTrigger::new(),
        };
        if !first {
            return trigger;
        }
        self.0.waiting.update(|n| *n += 1);
        let client = self.clone();
        leptos::task::spawn_local(async move {
            let answer = client.from_remote(&ask).await;
            // Replaced by a newer question of its lane: nothing to keep, and nobody to tell (what
            // asked it asks the newer one).
            let dropped = matches!(&answer, Err(error) if error.message == DROPPED);
            if !dropped {
                client.keep::<A>(key.clone(), &answer);
            }
            let trigger = client.0.asked.lock().ok().and_then(|mut asked| asked.remove(&key));
            client.0.waiting.update(|n| *n = n.saturating_sub(1));
            if let Some(trigger) = trigger.filter(|_| !dropped) {
                trigger.notify();
            }
        });
        trigger
    }

    /// Whether the answers come from elsewhere (the data worker), so they take a moment.
    pub fn is_remote(&self) -> bool {
        matches!(self.0.backend, Backend::Remote(_))
    }

    /// How many questions are on their way (reactive).
    pub fn waiting(&self) -> usize {
        self.0.waiting.get()
    }

    /// Forgets every answer (a new snapshot): what shows one asks again.
    pub fn forget(&self) {
        if let Ok(mut answers) = self.0.answers.lock() {
            answers.0.clear();
        }
        self.0.generation.update(|n| *n = n.wrapping_add(1));
    }
}

/// The client the host provided, as `use_source` (call it in the component body). A host that
/// provides a `Source` and no client gets one over that source.
pub fn use_data() -> Result<DataClient, DataError> {
    if let Some(client) = use_context::<DataClient>() {
        return Ok(client);
    }
    use_source().map(DataClient::new)
}

/// The answer to the question `ask` makes, following what it reads: `None` where it asks nothing
/// and until the first answer is there. While the answer to a new question is on its way, the
/// answer to the one before stays (what a page shows stays until the new data is there, §6.4).
pub fn use_ask<A: Ask>(ask: impl Fn() -> Option<A> + Send + Sync + 'static) -> Memo<Option<Result<A::Answer, DataError>>> {
    let client = use_data();
    Memo::new(move |before: Option<&Option<Result<A::Answer, DataError>>>| {
        let question = ask()?;
        match client.as_ref() {
            Ok(client) => client.get(&question).or_else(|| before.cloned().flatten()),
            Err(error) => Some(Err(error.clone())),
        }
    })
}

/// Sets the HTTP status of a server-rendered page (404 for an unknown module, 503 without
/// a snapshot). Captured in the component body; does nothing in the browser.
#[derive(Clone)]
pub struct PageStatus {
    #[cfg(feature = "ssr")]
    response: Option<leptos_axum::ResponseOptions>,
}

impl PageStatus {
    pub fn capture() -> Self {
        Self {
            #[cfg(feature = "ssr")]
            response: use_context::<leptos_axum::ResponseOptions>(),
        }
    }

    #[allow(unused_variables)]
    pub fn set(&self, code: u16) {
        #[cfg(feature = "ssr")]
        if let (Some(response), Ok(status)) = (&self.response, http::StatusCode::from_u16(code)) {
            response.set_status(status);
        }
    }

    pub fn for_error(&self, error: &DataError) {
        self.set(if error.unavailable { 503 } else { 500 });
    }
}
