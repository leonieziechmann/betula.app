//! „Notenübersicht einlesen" (owner, 2026-11-18: „bitte bau die Option den Studiengang aus der
//! Notenübersicht automatisch befüllen zu lassen. Das soll strikt im Browser laufen die datei darf
//! das Endnutzer Gerät nicht verlassen und soll auch nicht irgendwie persistent gespeichert werden.
//! Nur einmal die module extrahieren+status und dann datei vergessen."): one dialog from the first
//! word to what was read (the same day: „mach das mal so, dass sich ein Dialog öffnet, bei dem ganz
//! kurz erklärt wird, dass die Module aus dem Dokument lokal ausgelesen werden, auch mit einem link
//! zur seite wo man das runterladen kann und dann der Knopf mit Notenübersicht auswählen. Und wenn
//! das dann eingelesen wird mach bitte eine progressbar … Die idee ist halt den nutzer aufzuklären,
//! was das genau macht und dann dass es sich gut und satisfying anfühlt"). Its three views
//! (`Flow`): what it does, with the way to the BTU's Online-Portal and „Notenübersicht auswählen";
//! the file being read, a bar and its steps ticked off one by one, each standing a moment however
//! quick the reading is (`PACE`); and what was read, or why nothing was.
//!
//! The file goes from the picker to a Web Worker of its own (`window.betulaTranscript`,
//! folia/assets/boot.js, which starts folia/crates/transcript/js/worker.js for this one file and
//! ends it once it has answered), and the picker is emptied at once: the page never holds the
//! file's bytes, the worker's memory goes with the worker, nothing is sent and nothing is stored.
//! The page gets what the worker read (`folia_transcript::Transcript::lines`: the program, since
//! when, each module's titles, status, credits, attempt and semester — not the grades, not the
//! student) and asks the data worker, beside it in the browser, which catalog modules those are
//! (`TranscriptAsk`, `DataClient::ask_once`: kept nowhere). „Übernehmen" puts them into the study
//! (`folia_plans::transcript::take`), with the program and its Studienbeginn where the study had
//! none or another, and „Rückgängig" takes all of it back.

use folia_calendar::semester::SemesterKey;
use folia_model::rows::Program;
use folia_pages::transcript::MatchedModule;
use folia_plans::transcript::{self as taking, Row, Taken};
use folia_transcript::Status as Mark;
use leptos::prelude::*;

use folia_design::nav;
use folia_design::ui::Icon;
use folia_stores::myprogram::program_name;

use super::dialog::DialogHead;
use super::focus::Status;
use super::overview::tally_mark;
use super::{btu, n, now_secs, Dialog, StudyCtx, Undo};
use crate::i18n::{self, Texts};

/// A file larger than this is no Notenübersicht (the BTU's are a few hundred kilobytes): it is
/// not even handed to the worker.
#[cfg(feature = "csr")]
const MAX_FILE: f64 = 20.0 * 1024.0 * 1024.0;

/// How long each step of the reading stands at least, in milliseconds: opening, reading, finding
/// in the catalog, done (owner, 2026-11-18: „Du kannst die auch ein kleines wenig langsamer
/// machen, so dass es sich anfühlt, als würde die app was machen"). The reading itself takes a few
/// hundredths of a second, which a bar would not show; about two seconds in all. A quarter of it
/// where less motion is asked for.
#[cfg(feature = "csr")]
const PACE: [f64; 4] = [450.0, 550.0, 550.0, 500.0];

/// Tells a reading from one begun after it (the dialog opened anew while one was on its way).
#[cfg(feature = "csr")]
static RUNS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// Where „Notenübersicht einlesen" stands: the dialog's view. A file is read in the browser alone.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(crate) enum Flow {
    /// What it does, where the file comes from, and the button that picks it.
    #[default]
    Intro,
    /// A file being read.
    Reading(Progress),
    /// What was read, or why nothing was.
    Done(Box<Reading>),
}

/// How far a file is read, and what is known of it already.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Progress {
    /// Which reading (`RUNS`).
    pub run: u32,
    pub step: Step,
    /// How many modules were read, once the worker has answered.
    pub read: Option<usize>,
    /// How many of them the catalog has, once the data worker has answered.
    pub found: Option<usize>,
}

/// The steps of a reading, in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(crate) enum Step {
    Opening,
    Reading,
    Matching,
    Finished,
}

impl Step {
    /// The share of the bar it has filled by its end.
    fn share(self) -> f64 {
        match self {
            Step::Opening => 0.24,
            Step::Reading => 0.58,
            Step::Matching => 0.86,
            Step::Finished => 1.0,
        }
    }
}

/// What was read, or why nothing was.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Reading {
    pub outcome: Result<Read, Problem>,
}

/// Why a file was not read (in the browser alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub(crate) enum Problem {
    NotPdf,
    Locked,
    NotTranscript,
    TooLarge,
    /// The worker did not answer (its WASM did not load: offline before it was ever loaded), or the
    /// data worker did not.
    Failed,
}

/// A Notenübersicht as the catalog has it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Read {
    /// The program it names, as the catalog has it.
    pub program: Option<Program>,
    /// Its name and degree as it writes them, for where the catalog has no such program.
    pub named: Option<(String, Option<String>)>,
    /// The semester of its „seit".
    pub start: Option<SemesterKey>,
    pub rows: Vec<ReadRow>,
}

/// A row of it: its module in the catalog, if found, and what it says of it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReadRow {
    /// Its title as the Notenübersicht writes it.
    pub title: String,
    pub module: Option<MatchedModule>,
    pub mark: Mark,
    pub term: Option<SemesterKey>,
    pub attempt: Option<u8>,
    pub credits: Option<f64>,
    pub recognised: bool,
}

/// Opens the dialog at its first view.
pub(super) fn open(ctx: StudyCtx) {
    ctx.transcript.set(Flow::Intro);
    ctx.open(Dialog::Transcript);
}

/// The way to it: a button that opens the dialog. `line` with the line under it on a phone's
/// overview (as its other ways).
#[component]
pub(super) fn TranscriptWay(ctx: StudyCtx, #[prop(optional)] line: bool, id: &'static str) -> impl IntoView {
    let t = i18n::t();
    let x = &t.study_transcript;
    view! {
        // „Befehle" finds it by its name (R25), not by the line under it.
        <button class="action" class:st-way=line type="button" id=id aria-haspopup="dialog" data-command=x.way data-command-icon="file-input" on:click=move |_| open(ctx)>
            <Icon name="file-input"/>
            <span>{x.way}<small>{x.way_line}</small></span>
            <Icon name="chevron-right"/>
        </button>
    }
}

/// The same on the first visit: a box beside the one that fills the semesters from the plan, as
/// wide and as tall.
#[component]
pub(super) fn TranscriptBox(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let x = &t.study_transcript;
    view! {
        <button class="st-check big st-fill st-tr-box" type="button" id="st-transcript-setup" aria-haspopup="dialog" on:click=move |_| open(ctx)>
            <span class="st-tr-icon" aria-hidden="true"><Icon name="file-input"/></span>
            <span><b>{x.setup_title}</b><span>{x.setup_line}</span></span>
        </button>
    }
}

/// The file picker, out of sight: PDFs. Its button is in the dialog.
#[component]
fn Picker(ctx: StudyCtx, input: NodeRef<leptos::html::Input>) -> impl IntoView {
    view! { <input class="visually-hidden" type="file" accept=".pdf,application/pdf" tabindex="-1" aria-hidden="true" node_ref=input on:change=move |_| read_picked(ctx, input)/> }
}

/// Opens the picker.
fn pick(input: NodeRef<leptos::html::Input>) {
    if let Some(input) = input.get_untracked() {
        input.click();
    }
}

/// The dialog: the view `Flow` is at. Each view is built once while it stands, so that the bar
/// of the reading moves rather than being drawn anew at every step.
#[component]
pub(super) fn TranscriptDialog(ctx: StudyCtx) -> impl IntoView {
    let view = Memo::new(move |_| {
        ctx.transcript.with(|flow| match flow {
            Flow::Intro => 0,
            Flow::Reading(_) => 1,
            Flow::Done(_) => 2,
        })
    });
    move || match view.get() {
        0 => view! { <Intro ctx/> }.into_any(),
        1 => view! { <Progressing ctx/> }.into_any(),
        _ => match ctx.transcript.get_untracked() {
            Flow::Done(reading) => view! { <Outcome ctx reading=*reading/> }.into_any(),
            _ => ().into_any(),
        },
    }
}

/// What it does: Betula reads the PDF here, in the browser, and fills the study from it; where the
/// PDF comes from, with the way to the BTU's Online-Portal; that the file stays on the device. Each
/// part at a glance, an icon to each and little to read (owner, 2026-11-19: „mach mal den modal
/// weniger wie eine wall of text durch bessere visuelle Hierarchie und Icons"): the PDF going into
/// „Mein Studium" with a line in bold, the three steps on a line of their own, and three points.
#[component]
fn Intro(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let x = &t.study_transcript;
    let input = NodeRef::<leptos::html::Input>::new();
    let step = |icon: &'static str, title: &'static str, line: &'static str| view! {
        <span class="st-tr-n" aria-hidden="true"><Icon name=icon/></span>
        <span class="st-tr-step"><b>{title}</b><small>{line}</small></span>
    };
    let point = |icon: &'static str, text: &'static str| view! { <li><Icon name=icon/><span>{text}</span></li> };
    view! {
        <DialogHead ctx title=x.title sub=x.intro_sub/>
        <div class="st-dlg-body st-transcript st-tr-intro">
            <div class="st-tr-hero">
                // The PDF into „Mein Studium".
                <span class="st-tr-flow" aria-hidden="true">
                    <span class="st-tr-icon"><Icon name="file-text"/></span>
                    <Icon name="arrow-right"/>
                    <span class="st-tr-icon"><Icon name="graduation-cap"/></span>
                </span>
                <span class="st-tr-hero-text"><b>{x.intro_lead}</b><small>{x.intro_what}</small></span>
            </div>
            <ol class="st-tr-steps">
                <li>
                    {step("download", x.step_download, x.step_download_line)}
                    <a class="st-tr-portal" id="st-transcript-portal" href=btu::ONLINE_PORTAL target="_blank" rel="noopener noreferrer"><span>{x.portal}</span><Icon name="arrow-up-right"/></a>
                </li>
                <li>{step("file-input", x.step_pick, x.step_pick_line)}</li>
                <li>{step("list-checks", x.step_check, x.step_check_line)}</li>
            </ol>
            <div class="st-tr-stays">
                <p class="st-tr-stays-title"><Icon name="shield-check"/><b>{x.stays_title}</b></p>
                <ul>
                    {point("monitor-smartphone", x.stays_here)}
                    {point("cloud-off", x.stays_sent)}
                    {point("eye-off", x.stays_grades)}
                </ul>
            </div>
        </div>
        <footer class="st-dlg-foot st-tr-foot">
            <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{x.cancel}</button>
            <button class="btn primary" type="button" id="st-transcript-pick" on:click=move |_| pick(input)><Icon name="file-input"/>{x.pick}</button>
            <Picker ctx input/>
        </footer>
    }
}

/// The file being read: the bar, and its steps ticked off one by one with what each found.
#[component]
fn Progressing(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let x = &t.study_transcript;
    let progress = Memo::new(move |_| {
        ctx.transcript.with(|flow| match flow {
            Flow::Reading(progress) => Some(*progress),
            _ => None,
        })
    });
    let step = move || progress.get().map_or(Step::Opening, |progress| progress.step);
    let share = move || step().share();
    let percent = move || (share() * 100.0).round() as u32;
    let finished = move || step() == Step::Finished;
    // A step's line: what it does while it runs, what it found once it is done.
    let line = move |which: Step| {
        let now = step();
        let state = if now > which || now == Step::Finished { "done" } else if now == which { "now" } else { "next" };
        let words = move || {
            let progress = progress.get();
            let read = progress.and_then(|progress| progress.read).unwrap_or(0);
            let found = progress.and_then(|progress| progress.found).unwrap_or(0);
            let done = step() > which || step() == Step::Finished;
            match (which, done) {
                (Step::Opening, false) => x.opening.to_string(),
                (Step::Opening, true) => x.opened.to_string(),
                (Step::Reading, false) => x.reading_modules.to_string(),
                (Step::Reading, true) => (x.modules_read)(read),
                (Step::Matching, false) => x.matching.to_string(),
                (Step::Matching, true) => (x.found)(found, read),
                (Step::Finished, _) => String::new(),
            }
        };
        view! {
            <li class=format!("st-tr-stage {state}")>
                <span class="st-tr-tick" aria-hidden="true"><Icon name="check"/></span>
                <span>{words}</span>
            </li>
        }
    };
    let stages = move || [Step::Opening, Step::Reading, Step::Matching].into_iter().map(line).collect_view();
    view! {
        <DialogHead ctx title=x.title sub=x.reading_sub/>
        <div class="st-dlg-body st-transcript st-tr-reading">
            <div class="st-tr-progress">
                <div class="st-tr-file">
                    // The file, and once it is read a check in its place.
                    <span class="st-tr-icon" class:done=finished aria-hidden="true">
                        {move || if finished() { view! { <Icon name="check"/> }.into_any() } else { view! { <Icon name="file-input"/> }.into_any() }}
                    </span>
                    <span><b>{move || if finished() { x.read_done } else { x.reading_title }}</b><small>{x.reading_here}</small></span>
                </div>
                <div class="st-tr-bar" class:done=finished role="progressbar" aria-label=x.progress aria-valuemin="0" aria-valuemax="100" aria-valuenow=move || percent().to_string()>
                    <span class="st-tr-fill" style=move || format!("--share: {:.2}", share())></span>
                </div>
                <ol class="st-tr-stages" aria-live="polite">{stages}</ol>
            </div>
            <p class="st-safe"><Icon name="shield-check"/><span>{x.reading_safe}</span></p>
        </div>
        <footer class="st-dlg-foot st-tr-foot">
            <p class="st-foot-state">{move || format!("{} %", percent())}</p>
            <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{x.cancel}</button>
        </footer>
    }
}

/// Reads the file picked, in the worker, the dialog showing how far. Only in the browser.
#[cfg(feature = "csr")]
fn read_picked(ctx: StudyCtx, input: NodeRef<leptos::html::Input>) {
    let Some(input) = input.get_untracked() else { return };
    let Some(file) = input.files().and_then(|files| files.get(0)) else { return };
    // The picker lets go of the file at once (and the same file picked again is a change again).
    input.set_value("");
    if file.size() > MAX_FILE {
        ctx.transcript.set(Flow::Done(Box::new(Reading { outcome: Err(Problem::TooLarge) })));
        return;
    }
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ctx.transcript.set(Flow::Reading(Progress { run, step: Step::Opening, read: None, found: None }));
    leptos::task::spawn_local(async move {
        // `None`: another reading took its place, or the dialog was opened anew.
        if let Some(outcome) = read(ctx, run, file).await {
            if ours(ctx, run) {
                ctx.transcript.set(Flow::Done(Box::new(Reading { outcome })));
            }
        }
    });
}

#[cfg(not(feature = "csr"))]
fn read_picked(_: StudyCtx, _: NodeRef<leptos::html::Input>) {}

/// Whether the dialog still shows reading `run`: it is open, and was not opened anew since. A
/// reading it left behind stops at its next step („Abbrechen" while it reads: nothing of it comes
/// back).
#[cfg(feature = "csr")]
fn ours(ctx: StudyCtx, run: u32) -> bool {
    ctx.dialog.with_untracked(|dialog| matches!(dialog, Some(Dialog::Transcript))) && ctx.transcript.with_untracked(|flow| matches!(flow, Flow::Reading(progress) if progress.run == run))
}

/// Reading `run` moved on; `None` where it is not the dialog's any more.
#[cfg(feature = "csr")]
fn advance(ctx: StudyCtx, run: u32, change: impl FnOnce(&mut Progress)) -> Option<()> {
    let mut progress = ctx.transcript.with_untracked(|flow| match flow {
        Flow::Reading(progress) if ours(ctx, run) => Some(*progress),
        _ => None,
    })?;
    change(&mut progress);
    ctx.transcript.set(Flow::Reading(progress));
    Some(())
}

/// Waits `ms` milliseconds.
#[cfg(feature = "csr")]
async fn sleep(ms: f64) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms.max(0.0) as i32);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Waits until `at` (`js_sys::Date::now`).
#[cfg(feature = "csr")]
async fn until(at: f64) {
    let left = at - js_sys::Date::now();
    if left > 0.0 {
        sleep(left).await;
    }
}

/// Less motion is asked for.
#[cfg(feature = "csr")]
fn calm() -> bool {
    web_sys::window().and_then(|window| window.match_media("(prefers-reduced-motion: reduce)").ok().flatten()).is_some_and(|query| query.matches())
}

/// What the worker reads of `file`, and what the catalog has of it, the dialog's bar following
/// each step; `None` where the dialog left the reading behind.
#[cfg(feature = "csr")]
async fn read(ctx: StudyCtx, run: u32, file: web_sys::File) -> Option<Result<Read, Problem>> {
    use folia_pages::ask::TranscriptAsk;
    use folia_pages::transcript::TranscriptRow;
    use folia_transcript::{Error, Transcript};
    use wasm_bindgen::JsCast;

    let slow = if calm() { 0.25 } else { 1.0 };
    let pace = |step: usize| PACE.get(step).copied().unwrap_or(0.0) * slow;
    let started = js_sys::Date::now();
    // The worker starts at once; the first step stands meanwhile.
    let promise = (|| {
        let window = web_sys::window()?;
        let reader: js_sys::Function = js_sys::Reflect::get(&window, &"betulaTranscript".into()).ok()?.dyn_into().ok()?;
        reader.call1(&window, &file).ok()
    })();
    drop(file);
    until(started + pace(0)).await;
    let Some(promise) = promise else { return Some(Err(Problem::Failed)) };
    advance(ctx, run, |progress| progress.step = Step::Reading)?;
    let lines = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise)).await.ok().and_then(|answer| answer.as_string());
    until(started + pace(0) + pace(1)).await;
    let Some(lines) = lines else { return Some(Err(Problem::Failed)) };
    let transcript = match Transcript::from_lines(&lines) {
        Ok(transcript) => transcript,
        Err(Error::NotPdf) => return Some(Err(Problem::NotPdf)),
        Err(Error::Locked) => return Some(Err(Problem::Locked)),
        Err(Error::NotTranscript) => return Some(Err(Problem::NotTranscript)),
    };
    drop(lines);
    let read = transcript.entries.len();
    advance(ctx, run, |progress| {
        progress.read = Some(read);
        progress.step = Step::Matching;
    })?;
    let matching = js_sys::Date::now();
    let Some(source) = ctx.source.get_value() else { return Some(Err(Problem::Failed)) };
    let mine = ctx.mine.and_then(|mine| untrack(|| mine.with(|doc| doc.program.clone())));
    let asked = TranscriptAsk {
        mine,
        degree: transcript.degree.clone(),
        program: transcript.program.clone(),
        profile: transcript.profile.clone(),
        // The semester the study began in, for the PO it is of.
        since: transcript.since.map(|since| since.term().code()),
        rows: transcript.entries.iter().map(|entry| TranscriptRow { title: entry.title.clone(), other: entry.other.clone(), credits: entry.credits }).collect(),
    };
    let found = source.ask_once(asked).await;
    until(matching + pace(2)).await;
    let Ok(found) = found else { return Some(Err(Problem::Failed)) };
    let count = found.modules.iter().flatten().count();
    advance(ctx, run, |progress| {
        progress.found = Some(count);
        progress.step = Step::Finished;
    })?;
    sleep(pace(3)).await;
    let key = |term: folia_transcript::Term| SemesterKey::new(term.year, term.winter);
    let rows = transcript
        .entries
        .into_iter()
        .zip(found.modules.into_iter().chain(std::iter::repeat(None)))
        .map(|(entry, module)| ReadRow { title: entry.title, module, mark: entry.status, term: entry.term.and_then(key), attempt: entry.attempt, credits: entry.credits, recognised: entry.recognised })
        .collect();
    Some(Ok(Read { program: found.program, named: transcript.program.map(|name| (name, transcript.degree)), start: transcript.since.map(|since| since.term()).and_then(key), rows }))
}

/// What „Mein Studiengang" is set to (not `Set`: that is leptos' trait of `set`).
#[derive(Clone, Debug, PartialEq)]
enum Setting {
    /// Nothing: the program the Notenübersicht names is the one kept, with its Studienbeginn.
    Nothing,
    /// The Studienbeginn of the program kept, which had none.
    Start(SemesterKey),
    /// Another program (or the first), with the Studienbeginn of the Notenübersicht.
    Study(Box<Program>, SemesterKey),
}

/// What becomes of the program, as the dialog says it.
#[derive(Clone, Debug, PartialEq)]
enum Becomes {
    Same,
    New,
    Instead(String),
}

/// What taking a Notenübersicht over does, worked out on a copy of the study as it stands: what
/// „Mein Studiengang" is set to, the Studienbeginn (where the rows without a semester go), and
/// each row's change.
#[derive(Clone, Debug, PartialEq)]
struct Plan {
    set: Setting,
    becomes: Becomes,
    /// The program's name after it.
    program: Option<String>,
    start: SemesterKey,
    now: SemesterKey,
    /// The rows of modules found (their index among the rows read), as `taking` takes them, with
    /// what each did.
    rows: Vec<(usize, Row, Option<Taken>)>,
}

impl Plan {
    /// `None` where there is no program to take the rows into: none kept, and none of the catalog.
    fn of(ctx: StudyCtx, read: &Read) -> Option<Plan> {
        let now = ctx.now.get_untracked();
        let (kept, kept_start, kept_name) = ctx.mine.map(|mine| untrack(|| mine.with(|doc| (doc.program.clone(), doc.start, doc.name.clone())))).unwrap_or_default();
        let earliest = read.rows.iter().filter_map(|row| row.term).min();
        let named_start = read.start.or(earliest).unwrap_or(now);
        let (set, becomes, start, program) = match (&read.program, &kept) {
            (Some(program), Some(id)) if *id == program.id => {
                let set = match kept_start {
                    Some(_) => Setting::Nothing,
                    None => Setting::Start(named_start),
                };
                (set, Becomes::Same, kept_start.unwrap_or(named_start), Some(program_name(program)))
            }
            (Some(program), Some(_)) => {
                let becomes = kept_name.clone().filter(|name| !name.trim().is_empty()).map_or(Becomes::New, Becomes::Instead);
                (Setting::Study(Box::new(program.clone()), named_start), becomes, named_start, Some(program_name(program)))
            }
            (Some(program), None) => (Setting::Study(Box::new(program.clone()), named_start), Becomes::New, named_start, Some(program_name(program))),
            // Not in the catalog: into the program kept.
            (None, Some(_)) => (Setting::Nothing, Becomes::Same, kept_start.unwrap_or(named_start), kept_name),
            (None, None) => return None,
        };
        let rows: Vec<(usize, Row)> = read
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| row.module.as_ref().map(|module| (index, Row { module_id: module.id.clone(), status: row.mark.clone(), term: row.term, attempt: row.attempt })))
            .collect();
        let mut doc = ctx.plan.map(|plan| plan.with_untracked(Clone::clone)).unwrap_or_default();
        let taken = taking::take(&mut doc, &rows.iter().map(|(_, row)| row.clone()).collect::<Vec<_>>(), now, start, 0);
        let rows = rows.into_iter().zip(taken).map(|((index, row), taken)| (index, row, taken)).collect();
        Some(Plan { set, becomes, program, start, now, rows })
    }
}

/// The mark a row will have in the study, by the semester it goes into.
fn status_of(mark: &Mark, s: SemesterKey, now: SemesterKey) -> Status {
    match mark {
        Mark::Passed => Status::Passed,
        _ if s > now => Status::Planned,
        Mark::Registered => Status::Running,
        _ if s == now => Status::Running,
        _ => Status::Failed,
    }
}

/// What was read and what taking it over does; or why nothing was read.
#[component]
fn Outcome(ctx: StudyCtx, reading: Reading) -> impl IntoView {
    let t = i18n::t();
    let x = &t.study_transcript;
    match reading.outcome {
        Err(problem) => {
            let why = match problem {
                Problem::NotPdf => x.not_pdf,
                Problem::Locked => x.locked,
                Problem::NotTranscript => x.not_transcript,
                Problem::TooLarge => x.too_large,
                Problem::Failed => x.failed,
            };
            let input = NodeRef::<leptos::html::Input>::new();
            view! {
                <DialogHead ctx title=x.title/>
                <div class="st-dlg-body st-transcript">
                    <p class="st-tr-problem" role="alert"><Icon name="triangle-alert"/><span>{why}</span></p>
                    <p class="st-safe"><Icon name="shield-check"/><span>{x.privacy}</span></p>
                </div>
                <footer class="st-dlg-foot st-tr-foot">
                    <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{x.close}</button>
                    <button class="btn primary" type="button" id="st-transcript-again" on:click=move |_| pick(input)>{x.try_again}</button>
                    <Picker ctx input/>
                </footer>
            }
            .into_any()
        }
        Ok(read) => view! { <ReadDialog ctx read/> }.into_any(),
    }
}

#[component]
fn ReadDialog(ctx: StudyCtx, read: Read) -> impl IntoView {
    let t = i18n::t();
    let x = &t.study_transcript;
    let plan = Plan::of(ctx, &read);
    let locale = t.locale;
    let count = |mark: fn(&Mark) -> bool| read.rows.iter().filter(|row| mark(&row.mark)).count();
    let (passed, failed, registered) = (count(|mark| *mark == Mark::Passed), count(|mark| matches!(mark, Mark::Failed | Mark::FinallyFailed)), count(|mark| *mark == Mark::Registered));
    let read_line = [Some((x.modules_read)(read.rows.len())), (passed > 0).then(|| (x.n_passed)(passed)), (failed > 0).then(|| (x.n_failed)(failed)), (registered > 0).then(|| (x.n_registered)(registered))]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let missing: Vec<&ReadRow> = read.rows.iter().filter(|row| row.module.is_none()).collect();
    let missing_view = (!missing.is_empty()).then(|| {
        let rows = missing
            .iter()
            .map(|row| {
                let when = row.term.map_or_else(|| x.no_term.to_string(), |term| term.short(locale));
                view! { <li><div class="st-area-row"><span class="st-wmk" aria-hidden="true"><Icon name="circle-help"/></span><span class="st-area-nm"><b>{row.title.clone()}</b><small>{when}</small></span><span class="st-lp"></span></div></li> }
            })
            .collect_view();
        view! {
            <section class="st-tr-missing">
                <h3>{x.not_found}</h3>
                <p class="st-dlg-note">{x.not_found_hint}</p>
                <ul class="st-area-items">{rows}</ul>
            </section>
        }
    });
    let Some(plan) = plan else {
        // Neither a program kept nor one of the catalog: nothing can be taken over.
        let name = read.named.as_ref().map(|(name, degree)| named(name, degree.as_deref(), t)).unwrap_or_default();
        return view! {
            <DialogHead ctx title=x.title sub=x.read_here/>
            <div class="st-dlg-body st-transcript">
                <p class="st-tr-problem" role="alert"><Icon name="triangle-alert"/><span>{(x.program_missing)(&name)}" "{x.program_needed}</span></p>
                <p class="st-tr-counts">{read_line}</p>
                <p class="st-safe"><Icon name="shield-check"/><span>{x.privacy}</span></p>
            </div>
            <footer class="st-dlg-foot st-tr-foot">
                <button class="btn primary" type="button" on:click=move |_| ctx.dialog.set(None)>{x.close}</button>
            </footer>
        }
        .into_any();
    };

    // The program, and what becomes of it.
    let program_line = match &plan.becomes {
        Becomes::Same => x.program_same.to_string(),
        Becomes::New => x.program_new.to_string(),
        Becomes::Instead(before) => (x.program_instead)(before),
    };
    let missing_program = read.program.is_none().then(|| read.named.as_ref().map(|(name, degree)| (x.program_missing)(&named(name, degree.as_deref(), t)))).flatten();
    let program_view = view! {
        <div class="st-tr-program">
            <span class="st-tr-icon" aria-hidden="true"><Icon name="graduation-cap"/></span>
            <span>
                <b>{plan.program.clone().unwrap_or_default()}" "<span class="st-tr-since">{(x.since)(&plan.start.label(locale))}</span></b>
                <small>{program_line}</small>
                {missing_program.map(|text| view! { <small class="st-tr-warn">{text}</small> })}
            </span>
        </div>
    };

    // What each row does.
    let (mut new, mut kept, mut moved) = (0, 0, 0);
    // The place of a row in the list: the first rows come in one after the other (app.css).
    let mut shown = 0usize;
    let rows = plan
        .rows
        .iter()
        .filter_map(|(index, _, taken)| {
            let source = read.rows.get(*index)?;
            let module = source.module.as_ref()?;
            let mut notes: Vec<String> = Vec::new();
            let (status, when) = match taken {
                Some(taken) => {
                    let s = taken.semester.unwrap_or(plan.start);
                    if taken.kept {
                        kept += 1;
                        notes.push(x.change_kept.to_string());
                    } else if let Some(from) = taken.moved_from {
                        moved += 1;
                        notes.push((x.change_moved)(&from.short(locale)));
                    } else if taken.full {
                        notes.push(x.change_full.to_string());
                    } else {
                        new += 1;
                        notes.push(x.change_new.to_string());
                    }
                    notes.extend(taken.removed.iter().map(|other| (x.change_removed)(&other.short(locale))));
                    if taken.unmarked {
                        notes.push(x.change_unmarked.to_string());
                    }
                    (Some(status_of(&source.mark, s, plan.now)), s.short(locale))
                }
                None => {
                    notes.push((x.unknown_status)(source.mark.code()));
                    (None, source.term.map_or_else(|| x.no_term.to_string(), |term| term.short(locale)))
                }
            };
            if source.mark == Mark::Registered && source.term.is_some_and(|term| term < plan.now) {
                notes.push((x.registered_then)(&source.term.map(|term| term.short(locale)).unwrap_or_default()));
            }
            if source.mark == Mark::FinallyFailed {
                notes.push(x.finally_failed.to_string());
            }
            if source.recognised {
                notes.push(x.recognised.to_string());
            }
            let word = status.map_or(x.no_term, |status| status.word(t));
            let mark = match status {
                Some(status) => tally_mark(Some(status)),
                None => view! { <Icon name="circle-help"/> }.into_any(),
            };
            let credits = source.credits.filter(|credits| *credits > 0.0).map(|credits| n(credits, t));
            let place = shown;
            shown += 1;
            Some(view! {
                <li style=format!("--n: {place}")>
                    <div class="st-area-row">
                        <span class="st-wmk" title=word>{mark}<span class="visually-hidden">{word}</span></span>
                        <span class="st-area-nm"><b>{module.title.clone()}</b><small>{std::iter::once(when).chain(notes).collect::<Vec<_>>().join(" · ")}</small></span>
                        <span class="st-lp">{credits}</span>
                    </div>
                </li>
            })
        })
        .collect::<Vec<_>>();
    // What was read as the strip of „Mein Studium" (`tally_strip`): each mark with its number over
    // its word, every part always (one without a module faint); under it how many were read and what
    // taking them over changes.
    let tally = [(Status::Passed, passed, x.tally_passed), (Status::Failed, failed, x.tally_failed), (Status::Running, registered, x.tally_registered)]
        .into_iter()
        .map(|(status, count, word)| {
            view! {
                <li class:none=count == 0>
                    <span class="st-tally-n">{tally_mark(Some(status))}<b>{count}</b></span>
                    <span class="st-tally-w">{word}</span>
                </li>
            }
        })
        .collect_view();
    let summary = std::iter::once((x.modules_read)(read.rows.len()))
        .chain([(new > 0).then(|| (x.n_new)(new)), (kept > 0).then(|| (x.n_kept)(kept)), (moved > 0).then(|| (x.n_moved)(moved))].into_iter().flatten())
        .collect::<Vec<_>>()
        .join(" · ");
    let taken_count = plan.rows.iter().filter(|(_, _, taken)| taken.is_some()).count();
    let apply_label = if plan.becomes == Becomes::New { x.apply_setup } else { x.apply };
    let plan = StoredValue::new(plan);
    let apply = move |_| {
        let plan = plan.get_value();
        apply(ctx, plan, taken_count, t);
    };
    view! {
        <DialogHead ctx title=x.title sub=x.read_here/>
        <div class="st-dlg-body st-transcript">
            {program_view}
            <ul class="st-tally st-tr-tally" aria-label=x.tally_label>{tally}</ul>
            <p class="st-tr-summary">{summary}</p>
            <section>
                <h3>{x.taken}</h3>
                <ul class="st-area-items">{rows}</ul>
            </section>
            {missing_view}
            <p class="st-safe"><Icon name="shield-check"/><span>{x.privacy}</span></p>
        </div>
        <footer class="st-dlg-foot st-tr-foot">
            <button class="btn secondary" type="button" on:click=move |_| ctx.dialog.set(None)>{x.cancel}</button>
            <button class="btn primary" type="button" id="st-transcript-apply" disabled=taken_count == 0 on:click=apply>{apply_label}</button>
        </footer>
    }
    .into_any()
}

/// „„Informatik“ (Bachelor)": a program as the Notenübersicht names it.
fn named(name: &str, degree: Option<&str>, t: &Texts) -> String {
    let quoted = (t.study_transcript.quoted)(name);
    match degree {
        Some(degree) => format!("{quoted} ({degree})"),
        None => quoted,
    }
}

/// „Übernehmen": the program and its Studienbeginn where they change, then the rows into the
/// study, after the next frame (R21); „Rückgängig" takes both back.
fn apply(ctx: StudyCtx, plan: Plan, count: usize, t: &'static Texts) {
    let (Some(store), Some(mine)) = (ctx.plan, ctx.mine) else { return };
    ctx.dialog.set(None);
    nav::after_paint(move || {
        let mine_before = untrack(|| mine.get());
        match &plan.set {
            Setting::Nothing => {}
            Setting::Start(start) => mine.set_start(Some(*start)),
            // Another program with its first plan, as a program picked in the card starts.
            Setting::Study(program, start) => mine.set_study(&program.id, &program_name(program), "", None, *start),
        }
        let rows: Vec<Row> = plan.rows.iter().map(|(_, row, _)| row.clone()).collect();
        let before = store.update(|doc| {
            let before = doc.clone();
            taking::take(doc, &rows, plan.now, plan.start, now_secs());
            before
        });
        let turn = super::next_turn();
        ctx.undo.try_set(Some(Undo { note: (t.study_transcript.done)(count), before, mine: Some(mine_before), turn }));
    });
}
