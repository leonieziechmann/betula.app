//! The day ahead: the start page's photo stands with the day on it as the page loads, before the
//! browser app has started (owner, 2026-10-26: „Damit leute das gerne benutzen muss das einfach
//! ready sein. Bitte cache einfach alle werte, die du dafür brauchst und sorge dafür, dass das als
//! erstes konstruiert wird, so dass eine person die nur wissen will was als nächstes ansteht das
//! sofort sieht"); and always, even where what is kept is no longer quite right (owner,
//! 2026-11-01: „Aber das was eigentlich sein muss ist, dass es immer sofort lädt … Es ist an der
//! stelle auch erstmal egal ob die informationen outdated sind … Wenn die richtigen daten dann
//! geladen sind kann die cache ja nochmal überprüft werden um im zweifelsfall dann das ui element
//! rebuild werden").
//!
//! So the browser app, once the day stands (`keep`), writes what the photo shows for each of the
//! next `AHEAD_DAYS` days (three weeks: a break over Christmas included), a record per day under a
//! key of its own (`folia_stores::profiles::ahead_key`), so that the page reads the day of today
//! alone, a few kilobytes, however far ahead the days go. A day's record holds the photo of each
//! stretch of time in which nothing on it changes but what goes by the minute (a Termin begins or
//! ends, another greeting, another light), each built by the page's own `Hero` at the minute of
//! its start; the HTML of the stretches as pieces (`Pieces`: the photo's parts, its clock, its
//! card), each piece once, so that the clock the stretches of a day share is kept once; and what
//! goes by the minute within one (`Counts`): the minutes the sentence counts down (written with a
//! stand-in for the number, `SENTINEL`), what the card says (a pattern for its minutes, `Card`),
//! how far the clock's running week is filled. With each day what it was made of: the build, the
//! language, the profile, a hash of what „Mein Studiengang" and the Stundenplan keep (`made_of`),
//! and the catalog the page answered from. The app writes a day anew only where it no longer holds
//! (another plan, build or catalog), the nearest days first, a few stretches at a time (`SLICE`),
//! so that the page's thread stays free; once a day, the day that came into the three weeks.
//! Until 2026-11-01 it kept the days up to the next with Termine alone (four at most), under one
//! key, the card's minutes as a list, minute by minute: some 60 kB for a day and a half; now three
//! weeks are some 110 kB, a day 3 to 7 kB.
//!
//! The server's page (the same for everybody, R9) holds the day's skeleton for a browser that
//! keeps a program (`html.mine`), and right after its photo a script (`SCRIPT`) that puts in the
//! stretch of now, gone on to this minute, wherever the day of today is kept for this profile in
//! this language: of another build, another plan or another catalog as well. The app's page then
//! takes the page over and builds the photo of what is true now; where it is the same, nothing
//! moves, where the plan changed since, the photo changes to it (boot.js's picture of the page
//! stays in front until the app's first answers are in). And it writes the days anew.
//!
//! What it keeps is the visitor's (R20): derived from the Stundenplan, in this browser alone, gone
//! with the profile emptied or deleted.

// The server writes `SCRIPT` alone; the rest is the browser app's.
#![cfg_attr(not(feature = "csr"), allow(dead_code))]

use serde::Serialize;

#[cfg(feature = "csr")]
use folia_calendar::day::Day;
#[cfg(feature = "csr")]
use leptos::prelude::*;

#[cfg(feature = "csr")]
use super::today::{card_minutes, focus, running_line, sentence, Ctx, Hero};

/// The version of what is kept; the script reads its own alone.
const VERSION: u32 = 2;

/// The minutes of a day.
const DAY: u16 = 1440;

/// The number the sentence of a running Termin is written with, for the script to put the
/// minutes left in its place.
const SENTINEL: u16 = 7777;

/// The most a day keeps, in bytes of JSON: a day with many Termine is some 15 kB.
#[cfg(feature = "csr")]
const MOST_BYTES: usize = 200_000;

/// How long the page's day has to stand still before it is written, in ms: the questions of the
/// start answered, the stores written.
#[cfg(feature = "csr")]
const WAIT_MS: u64 = 2000;

/// Puts the stretch of now into the server's page in place of the skeleton's photo, right after
/// it, before the page is painted, wherever the day of today is kept for this profile in this
/// language. The day is the browser's local date counted from 1970-01-01, as `Day` counts it, its
/// key the slot `folia_stores::profiles::ahead_key` gives it; the minute the minute of the day.
pub const SCRIPT: &str = r#"(function(){try{var d=document.documentElement,me=document.currentScript,sk=me&&me.previousElementSibling;if(!sk||!d.classList.contains("mine"))return;var p=/^active\t([1-9][0-9]{0,2})$/m.exec(localStorage.getItem("betula.profiles.v1")||""),id=p?+p[1]:1,n=new Date(),day=Math.round(Date.UTC(n.getFullYear(),n.getMonth(),n.getDate())/864e5),m=n.getHours()*60+n.getMinutes(),c=JSON.parse(localStorage.getItem("betula.today.v2."+day%21)||"null");if(!c||c.v!==2||c.day!==day||c.profile!==id||c.lang!==d.lang)return;for(var j=0;j<c.s.length;j++){var g=c.s[j];if(m<g[0]||m>=g[1])continue;var box=document.createElement("div");box.innerHTML=g[2].map(function(i){return c.f[i]}).join("");var hero=box.firstElementChild,o=g[3]||{},e,r,k,at=function(x){return hero.querySelector('[data-count="'+x+'"]')};if(!hero)return;if(o.line&&(e=at("line"))){r=o.line.at-m;e.textContent=r===1?o.line.one:o.line.t.replace("7777",String(r))}if((k=o.card)&&(e=at("card"))){r=Math.max(0,k.at-m);e.textContent=k.l?(r<60?k.s.replace("{m}",r):k.l.replace("{h}",Math.floor(r/60)).replace("{mm}",(r%60<10?"0":"")+r%60)):k.t[Math.min(m-g[0],k.t.length-1)]}if(o.fill!=null&&(e=at("fill")))e.style.setProperty("--fill",Math.min(100,o.fill+(m-g[0])/100.8).toFixed(1)+"%");sk.replaceWith(hero);return}}catch(e){}})();"#;

/// What a day keeps: its photo's stretches of time, their pieces, and what they were made of. The
/// script reads the version, the day, the profile and the language; the app also the rest of
/// what it was made of, and `from`, the minute of the day it was written from (0 but for today).
#[derive(Serialize)]
struct Record {
    v: u32,
    profile: u16,
    lang: String,
    build: String,
    /// `fnv` of `made_of`.
    hash: u32,
    data: String,
    day: i32,
    from: u16,
    /// The pieces of the photo's HTML, each once.
    f: Vec<String>,
    s: Vec<Stretch>,
}

/// From minute to minute of the day (the end not its own), the photo's HTML as the numbers of its
/// pieces, and what goes by the minute in it.
#[derive(Serialize)]
struct Stretch(u16, u16, Vec<u32>, Counts);

/// What goes by the minute within a stretch, each in the part of the photo marked for it
/// (`data-count`), which is kept empty where it is counted here.
#[derive(Default, Serialize)]
struct Counts {
    /// The sentence of a running Termin („noch 55 Minuten").
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<Line>,
    /// What the card says of the minutes.
    #[serde(skip_serializing_if = "Option::is_none")]
    card: Option<Card>,
    /// How far the clock's running week is filled at the stretch's start, in percent; a week is
    /// 10 080 minutes, so a minute fills 1/100.8 of a percent more (`super::clock`).
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<f64>,
}

/// The sentence with `SENTINEL` for the minutes left until `at` (the minute of the day), and as it
/// reads with one.
#[derive(Serialize)]
struct Line {
    t: String,
    one: String,
    at: u16,
}

/// What the card says of the minutes: the minutes to `at` (the minute of the day; none once it
/// has come) in the patterns `pattern` makes of its words, or where they do not fit, what it says
/// minute by minute from the stretch's start.
#[derive(Serialize)]
#[serde(untagged)]
enum Card {
    Pattern { at: u16, s: String, l: String },
    List { t: Vec<String> },
}

/// The pieces of a day's HTML, each once, by number.
#[derive(Default)]
struct Pieces {
    all: Vec<String>,
    numbers: std::collections::HashMap<String, u32>,
}

impl Pieces {
    /// The number of `piece`, given it anew where it is new.
    fn number(&mut self, piece: String) -> u32 {
        if let Some(&number) = self.numbers.get(&piece) {
            return number;
        }
        let number = u32::try_from(self.all.len()).unwrap_or(u32::MAX);
        self.all.push(piece.clone());
        self.numbers.insert(piece, number);
        number
    }
}

/// The patterns of what the card says of `n` minutes (`in_minutes`, `minutes_left`): under an hour
/// with `{m}` for the minutes, from an hour on with `{h}` for the hours and `{mm}` for the minutes
/// in two digits; `None` where the words do not show the numbers once each.
pub fn pattern(words: fn(u16) -> String) -> Option<(String, String)> {
    let (short, long) = (words(37), words(23 * 60 + 47));
    if short.matches("37").count() != 1 || long.matches("23").count() != 1 || long.matches("47").count() != 1 || short.contains('{') || long.contains('{') {
        return None;
    }
    Some((short.replace("37", "{m}"), long.replace("23", "{h}").replace("47", "{mm}")))
}

/// `n` minutes in the patterns of `pattern`, as the script puts them in.
pub fn in_pattern(short: &str, long: &str, n: u16) -> String {
    if n < 60 {
        short.replace("{m}", &n.to_string())
    } else {
        long.replace("{h}", &(n / 60).to_string()).replace("{mm}", &format!("{:02}", n % 60))
    }
}

/// What the photo is made of, as the app compares it: „Mein Studiengang"'s text, the
/// Stundenplan's, and the Stundenplan's program of the tab's session where it is another than
/// „Mein Studiengang"'s (taking a semester over sets it to that one, in that tab alone), a line
/// between each.
pub fn made_of(program: &str, plan: &str, session: &str) -> String {
    let own = program.split('\n').find_map(|line| line.strip_prefix("program\t")).unwrap_or_default();
    let other = if session == own { "" } else { session };
    format!("{program}\n{plan}\n{other}")
}

/// FNV-1a over the UTF-16 units of `text`.
pub fn fnv(text: &str) -> u32 {
    text.encode_utf16().fold(0x811c_9dc5_u32, |hash, unit| (hash ^ u32::from(unit)).wrapping_mul(0x0100_0193))
}

/// Writes the days ahead of the page's day `live` once it stands, and again when what it is made
/// of changes, or the day.
#[cfg(feature = "csr")]
pub(super) fn keep(live: Ctx) {
    use leptos::leptos_dom::helpers::TimeoutHandle;
    use std::time::Duration;

    let Some(owner) = Owner::current() else { return };
    let build = use_context::<folia_design::BuildId>().map(|build| build.0.to_string()).unwrap_or_default();
    let day = Memo::new(move |_| live.now.get().0);
    let timer = StoredValue::new(None::<TimeoutHandle>);
    // The run that is writing: a newer one ends it between two of its slices, as the page gone.
    let run = StoredValue::new(0u32);
    Effect::new(move |_| {
        // What the others come of first (R16): the semester, then its modules, then the timetable,
        // the facts and the names their data makes. Read after them, a change of the plan that left
        // the timetable as it was (a module planned while its Termine are on their way) went unseen
        // until something else changed.
        let semester = live.key.with(Option::is_some);
        live.planned.track();
        let ready = semester && live.table.with(Option::is_some);
        live.facts.track();
        live.names.track();
        live.progress.track();
        day.track();
        run.update_value(|run| *run = run.wrapping_add(1));
        if let Some(handle) = timer.get_value() {
            handle.clear();
        }
        if !ready {
            return;
        }
        let (owner, build) = (owner.clone(), build.clone());
        timer.set_value(set_timeout_with_handle(move || owner.with(|| write(live, build, run)), Duration::from_millis(WAIT_MS)).ok());
    });
}

/// What the days kept are made of, beside the minutes they hold.
#[cfg(feature = "csr")]
#[derive(Clone, PartialEq)]
struct Made {
    profile: u16,
    lang: String,
    build: String,
    hash: u32,
    /// The catalog the page answered from (boot.js's `betulaData.etag`): the script does not ask
    /// for it, but the app writes the days anew for another one.
    data: String,
}

/// A day kept as far as `write` reads it: what it is made of, the day, and from when (the
/// pieces and stretches skipped, not copied).
#[cfg(feature = "csr")]
#[derive(serde::Deserialize)]
struct Head {
    v: u32,
    profile: u16,
    lang: String,
    build: String,
    hash: u32,
    data: String,
    day: i32,
    from: u16,
}

#[cfg(feature = "csr")]
impl Head {
    /// Whether it is `day` made of `made`, from `from` on.
    fn holds(self, made: &Made, day: Day, from: u16) -> bool {
        let Head { v, profile, lang, build, hash, data, day: kept, from: since } = self;
        v == VERSION && kept == day.0 && since <= from && Made { profile, lang, build, hash, data } == *made
    }
}

/// The stretches written in one go, before the page's thread is let go of again: a stretch takes
/// a phone a few milliseconds.
#[cfg(feature = "csr")]
const SLICE: usize = 4;

/// Works out the days from now on that no longer hold and keeps them, the nearest first, a few
/// stretches at a time (`SLICE`); nothing where they all still hold.
#[cfg(feature = "csr")]
fn write(live: Ctx, build: String, run: StoredValue<u32>) {
    use folia_design::nav;
    use folia_stores::profiles::{ahead_key, key_of, Profiles, AHEAD_DAYS, OLD_AHEAD_KEY};

    let Some(this) = run.try_get_value() else { return };
    let t = crate::i18n::t();
    let Some(profiles) = Profiles::expect() else { return };
    let id = profiles.active_untracked();
    let (day, minute) = live.now.get_untracked();
    let kept = |key: &str| nav::local_get(&key_of(key, id)).unwrap_or_default();
    let session = nav::session_get(&key_of(folia_stores::studyplan::PROGRAM_KEY, id)).unwrap_or_default();
    let text = made_of(&kept(folia_stores::myprogram::STORAGE_KEY), &kept(folia_stores::studyplan::STORAGE_KEY), &session);
    let made = Made { profile: id, lang: t.locale.code().to_string(), build, hash: fnv(&text), data: snapshot() };
    // What the days ahead were kept as until 2026-11-01: one key, read no more.
    nav::local_set(OLD_AHEAD_KEY, "");
    // The days to write, the nearest last: they are taken from the end.
    let days: Vec<(Day, u16)> = (0..AHEAD_DAYS)
        .rev()
        .map(|n| (day.plus(n), if n == 0 { minute } else { 0 }))
        .filter(|&(day, from)| {
            let held = nav::local_get(&ahead_key(day.0)).and_then(|text| serde_json::from_str::<Head>(&text).ok());
            !held.is_some_and(|held| held.holds(&made, day, from))
        })
        .collect();
    if days.is_empty() {
        return;
    }
    let Some(child) = Owner::current().map(|owner| owner.child()) else { return };
    let at = child.with(|| live.at(RwSignal::new((day, minute))));
    slices(Job { at, live, child, days, now: None, made, run, this, t });
}

/// A run of `write`: the page's day at the minutes to come (`at`, of its own owner), the days still
/// to write, and the day being written.
#[cfg(feature = "csr")]
struct Job {
    at: Ctx,
    live: Ctx,
    child: Owner,
    /// The days still to write and the minute each is written from, the next last.
    days: Vec<(Day, u16)>,
    now: Option<Writing>,
    made: Made,
    run: StoredValue<u32>,
    this: u32,
    t: &'static crate::i18n::Texts,
}

/// The day being written: its stretches still to write, those written and their pieces.
#[cfg(feature = "csr")]
struct Writing {
    day: Day,
    from: u16,
    pairs: Vec<(u16, u16)>,
    next: usize,
    pieces: Pieces,
    done: Vec<Stretch>,
}

/// Writes `SLICE` stretches, keeps a day once its stretches are written, and goes on after the
/// page's thread was free, as long as no newer run came.
#[cfg(feature = "csr")]
fn slices(mut job: Job) {
    use folia_design::nav;
    use folia_stores::profiles::ahead_key;

    if job.run.try_get_value() != Some(job.this) {
        job.child.cleanup();
        return;
    }
    let mut writing = match job.now.take() {
        Some(writing) => writing,
        None => {
            let Some((day, from)) = job.days.pop() else {
                job.child.cleanup();
                return;
            };
            let cuts = cuts(job.live, day, from);
            let pairs = cuts.iter().copied().zip(cuts.iter().copied().skip(1)).collect();
            Writing { day, from, pairs, next: 0, pieces: Pieces::default(), done: Vec::new() }
        }
    };
    let until = (writing.next + SLICE).min(writing.pairs.len());
    let (at, t, day) = (job.at, job.t, writing.day);
    let pairs: Vec<(u16, u16)> = writing.pairs.get(writing.next..until).unwrap_or_default().to_vec();
    let pieces = &mut writing.pieces;
    let written: Vec<Stretch> = job.child.with(|| pairs.iter().filter_map(|&(from, to)| stretch(at, day, from, to, t, pieces)).collect());
    writing.done.extend(written);
    writing.next = until;
    if until >= writing.pairs.len() {
        let Writing { day, from, pieces, done, .. } = writing;
        let Made { profile, lang, build, hash, data } = job.made.clone();
        let record = Record { v: VERSION, profile, lang, build, hash, data, day: day.0, from, f: pieces.all, s: done };
        if let Ok(json) = serde_json::to_string(&record) {
            if json.len() <= MOST_BYTES {
                nav::local_set(&ahead_key(day.0), &json);
            }
        }
    } else {
        job.now = Some(writing);
    }
    set_timeout(move || slices(job), std::time::Duration::ZERO);
}

/// The catalog the page's answers are from: the ETag of its snapshot, as boot.js has it
/// (`window.betulaData.etag`); empty where there is none.
#[cfg(feature = "csr")]
fn snapshot() -> String {
    use web_sys::js_sys::Reflect;
    let data = Reflect::get(window().as_ref(), &"betulaData".into()).ok().filter(|data| data.is_object());
    data.and_then(|data| Reflect::get(&data, &"etag".into()).ok()).and_then(|etag| etag.as_string()).unwrap_or_default()
}

/// Where the photo of `day` changes from minute `from` to its end: each greeting and light, each
/// Termin's beginning and end.
#[cfg(feature = "csr")]
fn cuts(live: Ctx, day: Day, from: u16) -> Vec<u16> {
    use std::collections::BTreeSet;

    use super::day::{self as daymodel, greeting};
    use super::photo;

    let t = crate::i18n::t();
    let words = daymodel::Words { locale: t.locale, exam: t.today.exam, own: t.termin.own_chip };
    let mut cuts = BTreeSet::from([from, DAY]);
    let mut before = None;
    for minute in 0..DAY {
        let state = (greeting(minute), photo::light(day, minute));
        if before.is_some_and(|before| before != state) {
            cuts.insert(minute);
        }
        before = Some(state);
    }
    live.table.with_untracked(|table| {
        let Some(table) = table else { return };
        live.agenda.with_untracked(|agenda| {
            live.names.with_untracked(|(titles, names)| {
                for entry in daymodel::today(table, agenda, titles, names, day, 0, &words).entries {
                    cuts.extend([entry.from, entry.to].into_iter().flatten());
                }
            });
        });
    });
    cuts.into_iter().filter(|cut| (from..=DAY).contains(cut)).collect()
}

/// The photo of `day` from `from` to `to` as the page writes it at `from`, as pieces, and how its
/// marked parts count. Built as the page builds it, into an element of no page, whose HTML is then
/// read: the same markup as the page's own, without Leptos's marks and with the parts counted
/// here empty.
#[cfg(feature = "csr")]
fn stretch(at: Ctx, day: Day, from: u16, to: u16, t: &'static crate::i18n::Texts, pieces: &mut Pieces) -> Option<Stretch> {
    use leptos::wasm_bindgen::JsCast;

    at.now.set((day, from));
    let hero = {
        let parent = document().create_element("div").ok()?;
        let mut built = view! { <Hero ctx=at on_setup=Callback::new(|()| {})/> }.build();
        built.mount(&parent, None);
        let copy = parent.first_element_child().and_then(|hero| hero.clone_node_with_deep(true).ok());
        built.unmount();
        copy?.dyn_into::<web_sys::Element>().ok()?
    };
    let mut counts = Counts::default();
    // The sentence: counting down while a Termin runs.
    let running = at.today.with(|today| today.running().cloned());
    let said = sentence(at, t);
    counts.line = running.and_then(|entry| {
        let end = entry.to?;
        (said == running_line(&entry, end.saturating_sub(from), t)).then(|| Line { t: running_line(&entry, SENTINEL, t), one: running_line(&entry, 1, t), at: end })
    });
    // The card's minutes, where it shows one of the day's Termine: in a pattern where it fits
    // what the card says at each minute of the stretch.
    counts.card = focus(at).map(|(entry, running)| {
        let list: Vec<String> = (from..to).map(|minute| card_minutes(&entry, running, minute, t)).collect();
        let words = if running { t.today.minutes_left } else { t.today.in_minutes };
        let goal = if running { entry.to } else { entry.from };
        let fits = |short: &str, long: &str, goal: u16| (from..to).zip(&list).all(|(minute, said)| in_pattern(short, long, goal.saturating_sub(minute)) == *said);
        match (pattern(words), goal) {
            (Some((s, l)), Some(goal)) if fits(&s, &l, goal) => Card::Pattern { at: goal, s, l },
            _ => Card::List { t: list },
        }
    });
    // The clock's running week.
    counts.fill = at.clock.with(|clock| clock.as_ref().and_then(|clock| clock.pieces.iter().find_map(|piece| piece.fill)).map(|fill| fill * 100.0));
    bare(&hero, &counts);
    let mut html = Vec::new();
    split(&hero, 2, &mut html);
    Some(Stretch(from, to, html.into_iter().map(|piece| pieces.number(piece)).collect(), counts))
}

/// Takes Leptos's marks (empty comments) out of the photo, and empties what the script counts:
/// the sentence and the card's minutes where they count, the clock's fill.
#[cfg(feature = "csr")]
fn bare(hero: &web_sys::Element, counts: &Counts) {
    fn marks(node: &web_sys::Node) {
        let mut child = node.first_child();
        while let Some(this) = child {
            child = this.next_sibling();
            if this.node_type() == web_sys::Node::COMMENT_NODE {
                let _ = node.remove_child(&this);
            } else {
                marks(&this);
            }
        }
    }
    marks(hero);
    let part = |name: &str| hero.query_selector(&format!("[data-count=\"{name}\"]")).ok().flatten();
    if counts.line.is_some() {
        if let Some(line) = part("line") {
            line.set_text_content(None);
        }
    }
    if counts.card.is_some() {
        if let Some(card) = part("card") {
            card.set_text_content(None);
        }
    }
    if counts.fill.is_some() {
        if let Some(fill) = part("fill") {
            let _ = fill.remove_attribute("style");
        }
    }
}

/// `element`'s HTML in pieces, `depth` levels down: its opening tag, each child's (and what stands
/// between them), its closing tag; whole where it does not come apart into exactly its HTML.
#[cfg(feature = "csr")]
fn split(element: &web_sys::Element, depth: u8, out: &mut Vec<String>) {
    let outer = element.outer_html();
    if depth == 0 || element.first_element_child().is_none() {
        out.push(outer);
        return;
    }
    let inner = element.inner_html();
    let close = format!("</{}>", element.tag_name().to_ascii_lowercase());
    let Some(open) = outer.strip_suffix(close.as_str()).and_then(|rest| rest.strip_suffix(inner.as_str())) else {
        out.push(outer);
        return;
    };
    let mut parts = vec![open.to_string()];
    let mut rest = inner.as_str();
    let mut child = element.first_element_child();
    while let Some(this) = child {
        let html = this.outer_html();
        let Some(at) = rest.find(html.as_str()) else {
            out.push(outer);
            return;
        };
        if at > 0 {
            parts.push(rest.get(..at).unwrap_or_default().to_string());
        }
        split(&this, depth - 1, &mut parts);
        rest = rest.get(at + html.len()..).unwrap_or_default();
        child = this.next_element_sibling();
    }
    if !rest.is_empty() {
        parts.push(rest.to_string());
    }
    parts.push(close);
    if parts.concat() == outer {
        out.extend(parts);
    } else {
        out.push(outer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script reads what the app keeps: the day as `Day` counts it, under the slot the profiles
    /// give it, this version.
    #[test]
    fn the_script_counts_as_the_app_does() {
        assert_eq!(fnv(""), 0x811c_9dc5);
        assert_eq!(fnv("a"), 0xe40c_292c);
        assert_ne!(fnv("ä"), fnv("a"));
        assert!(SCRIPT.contains("Math.round(Date.UTC(n.getFullYear(),n.getMonth(),n.getDate())/864e5)") && SCRIPT.contains("m=n.getHours()*60+n.getMinutes()"), "the same day and minute");
        let day = folia_calendar::day::Day::from_ymd(2026, 10, 26).unwrap();
        assert_eq!(day.0, 20_752);
        // The key of a day: the slot of `ahead_key`.
        let days = folia_stores::profiles::AHEAD_DAYS;
        assert!(SCRIPT.contains(&format!(r#"localStorage.getItem("{}."+day%{days})"#, folia_stores::profiles::AHEAD_KEY)));
        assert_eq!(folia_stores::profiles::ahead_key(day.0), format!("betula.today.v2.{}", 20_752 % days));
        assert!(SCRIPT.contains(&format!("o.line.t.replace(\"{SENTINEL}\"")) && SCRIPT.contains(&format!("c.v!=={VERSION}")));
        // What it checks: the day, the profile, the language; not the build, the plan or the
        // catalog, which the app's page puts right.
        assert!(SCRIPT.contains("c.day!==day||c.profile!==id||c.lang!==d.lang") && !SCRIPT.contains("build") && !SCRIPT.contains("hash"));
        // A week of minutes fills the clock's piece: a hundredth of it in 100.8 minutes.
        assert!(SCRIPT.contains("(m-g[0])/100.8") && (f64::from(7 * u32::from(DAY)) / 100.0 - 100.8).abs() < 1e-9);
        assert_eq!(made_of("a", "b", "c"), "a\nb\nc");
        // The session's program counts where it is another than „Mein Studiengang"'s.
        let mine = "name\tInformatik\nprogram\t079-82-2008\nstart\t2026W\n";
        assert_eq!(made_of(mine, "m\t2026W\t12104\n", "079-82-2008"), made_of(mine, "m\t2026W\t12104\n", ""));
        assert_ne!(made_of(mine, "m\t2026W\t12104\n", "079-84-2008"), made_of(mine, "m\t2026W\t12104\n", ""));
        // Nothing in it ends the script element it stands in.
        assert!(!SCRIPT.contains("</"));
    }

    /// What a day keeps is what the script reads: a stretch is from, to, the numbers of its pieces
    /// and what goes by the minute, by name and only where there is something.
    #[test]
    fn a_stretch_is_kept_as_the_script_reads_it() {
        let stretch = Stretch(1, 2, vec![0, 1, 0], Counts { line: Some(Line { t: "noch 7777".into(), one: "noch 1".into(), at: 5 }), card: Some(Card::Pattern { at: 600, s: "in {m} min".into(), l: "in {h} h {mm} min".into() }), fill: Some(12.5) });
        assert_eq!(serde_json::to_string(&stretch).unwrap(), r#"[1,2,[0,1,0],{"line":{"t":"noch 7777","one":"noch 1","at":5},"card":{"at":600,"s":"in {m} min","l":"in {h} h {mm} min"},"fill":12.5}]"#);
        assert_eq!(serde_json::to_string(&Card::List { t: vec!["in 1 min".into()] }).unwrap(), r#"{"t":["in 1 min"]}"#);
        assert_eq!(serde_json::to_string(&Counts::default()).unwrap(), "{}");
        for part in ["o.line", "o.card", "o.fill", r#"at("line")"#, r#"at("card")"#, r#"at("fill")"#, "g[0]", "g[1]", "g[2].map(function(i){return c.f[i]})", "g[3]", "k.at", "k.s", "k.l", "k.t"] {
            assert!(SCRIPT.contains(part), "{part}");
        }
        let mut pieces = Pieces::default();
        assert_eq!([pieces.number("a".into()), pieces.number("b".into()), pieces.number("a".into())], [0, 1, 0]);
        assert_eq!(pieces.all, ["a", "b"]);
    }

    /// The card's minutes in both languages fit their patterns, at every minute of a day; the
    /// script puts them in as `in_pattern` does.
    #[test]
    fn the_cards_minutes_fit_their_patterns() {
        for locale in [folia_locale::Locale::De, folia_locale::Locale::En] {
            let t = crate::i18n::texts(locale);
            for words in [t.today.in_minutes, t.today.minutes_left] {
                let (short, long) = pattern(words).unwrap();
                for n in 0..=DAY {
                    assert_eq!(in_pattern(&short, &long, n), words(n), "{n}");
                }
            }
        }
        assert_eq!(pattern(|n| format!("{n} {n}")), None, "a number twice");
        assert!(SCRIPT.contains(r#"r<60?k.s.replace("{m}",r):k.l.replace("{h}",Math.floor(r/60)).replace("{mm}",(r%60<10?"0":"")+r%60)"#));
    }
}
