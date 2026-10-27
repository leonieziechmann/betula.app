//! The day ahead: the start page's photo stands with the day on it as the page loads, before the
//! browser app has started (owner, 2026-10-26: „Damit leute das gerne benutzen muss das einfach
//! ready sein. Bitte cache einfach alle werte, die du dafür brauchst und sorge dafür, dass das als
//! erstes konstruiert wird, so dass eine person die nur wissen will was als nächstes ansteht das
//! sofort sieht"). Measured on canary before (a phone's processor, four times slower than a
//! notebook's, its files at hand): the server's page stood after a quarter of a second, the day on
//! it after two to four, the time the app's bundle, the catalog and the page's questions take.
//!
//! So the browser app, once the day stands (`keep`), writes what the photo shows from now to the
//! end of the next day with Termine (four days at most), as the page shows it: the photo of each
//! stretch of time in which nothing on it changes but what goes by the minute (a Termin begins or
//! ends, a new day, another greeting, another light), each built by the page's own `Hero` at the
//! minute of its start; and what goes by the minute within one (`Counts`): the minutes the sentence
//! counts down (written with a stand-in for the number, `SENTINEL`), what the card says minute by
//! minute, and how far the clock's running week is filled. It keeps the active profile's alone,
//! under one key (`folia_stores::profiles::AHEAD_KEY`), with what it was made of: the build, the
//! language, the profile, a hash of what „Mein Studiengang" and the Stundenplan keep (`made_of`),
//! and the catalog the page answered from. It writes them anew only where they no longer hold
//! from now on, a few stretches at a time (`SLICE`), so that the page's thread stays free.
//!
//! The server's page (the same for everybody, R9) holds the day's skeleton for a browser that
//! keeps a program (`html.mine`), and right after its photo a script (`SCRIPT`) that puts in the
//! stretch of now, gone on to this minute, where everything still holds: this build, this
//! language, this profile, the same program and plan, a stretch for now. Else the skeleton stays,
//! and the app's page comes as before. The app's page then takes the page over as it is (boot.js's
//! picture of it), and stands as it stood: built by the same `Hero`, of the same minute. What the
//! app's page answers from is the catalog this browser keeps, as the app's next start answers from
//! it; a newer one that comes writes the days anew.
//!
//! What it keeps is the visitor's (R20): derived from the Stundenplan, in this browser alone, gone
//! with the profile emptied or deleted.

// The server writes `SCRIPT` alone; the rest is the browser app's.
#![cfg_attr(not(feature = "csr"), allow(dead_code))]

use folia_calendar::day::Day;
use serde::Serialize;

#[cfg(feature = "csr")]
use leptos::prelude::*;

#[cfg(feature = "csr")]
use super::today::{card_minutes, focus, running_line, sentence, Ctx, Hero};

/// The version of what is kept; the script reads its own alone.
const VERSION: u32 = 1;

/// The minutes of a day.
const DAY: i32 = 1440;

/// The number the sentence of a running Termin is written with, for the script to put the
/// minutes left in its place.
const SENTINEL: u16 = 7777;

/// The days ahead at most: to the end of the next day with Termine (a Friday evening's Monday),
/// but no further.
#[cfg(feature = "csr")]
const MOST_DAYS: i32 = 4;

/// The most it keeps, in bytes of JSON: a day with many Termine is some 40 kB.
#[cfg(feature = "csr")]
const MOST_BYTES: usize = 400_000;

/// How long the page's day has to stand still before it is written, in ms: the questions of the
/// start answered, the stores written.
#[cfg(feature = "csr")]
const WAIT_MS: u64 = 2000;

/// Puts the stretch of now into the server's page in place of the skeleton's photo, right after
/// it, before the page is painted. Every check holds, or nothing happens. The build is the `?v=`
/// of the stylesheet, as the browser app reads it (folia-client, `build_of_page`); the minute is
/// the browser's, counted as `minute_index` counts it.
pub const SCRIPT: &str = r#"(function(){try{var d=document.documentElement,me=document.currentScript,sk=me&&me.previousElementSibling;if(!sk||!d.classList.contains("mine"))return;var c=JSON.parse(localStorage.getItem("betula.today.v1")||"null"),p=/^active\t([1-9][0-9]{0,2})$/m.exec(localStorage.getItem("betula.profiles.v1")||""),id=p?+p[1]:1,b=document.querySelector("link[rel=stylesheet]"),build=b?((b.getAttribute("href")||"").split("?v=")[1]||"").split(/[&#]/)[0]:"";if(!c||c.v!==1||c.profile!==id||c.lang!==d.lang||c.build!==build)return;var key=function(k){return id===1?"betula."+k:"betula.p"+id+"."+k},mp=localStorage.getItem(key("myprogram.v1"))||"",sp=sessionStorage.getItem(key("studyplan.program"))||"",own=(/^program\t(.*)$/m.exec(mp)||[])[1]||"",text=mp+"\n"+(localStorage.getItem(key("studyplan.v1"))||"")+"\n"+(sp!==own?sp:""),h=2166136261;for(var i=0;i<text.length;i++){h^=text.charCodeAt(i);h=Math.imul(h,16777619)>>>0}if(h!==c.hash)return;var n=new Date(),m=Math.round(Date.UTC(n.getFullYear(),n.getMonth(),n.getDate())/864e5)*1440+n.getHours()*60+n.getMinutes();for(var j=0;j<c.s.length;j++){var g=c.s[j];if(m<g[0]||m>=g[1])continue;var box=document.createElement("div");box.innerHTML=g[2];var hero=box.firstElementChild,o=g[3]||{},e,at=function(k){return hero.querySelector('[data-count="'+k+'"]')};if(!hero)return;if(o.line&&(e=at("line"))){var r=o.line.at-m;e.textContent=r===1?o.line.one:o.line.t.replace("7777",String(r))}if(o.card&&(e=at("card")))e.textContent=o.card[Math.min(m-g[0],o.card.length-1)];if(o.fill!=null&&m>g[0]&&(e=at("fill")))e.style.setProperty("--fill",Math.min(100,o.fill+(m-g[0])/100.8).toFixed(1)+"%");sk.replaceWith(hero);return}}catch(e){}})();"#;

/// What is kept: the photo's stretches of time ahead, and what they were made of. The script
/// reads the version, the profile, the language, the build and the hash; the app also the catalog
/// and the minutes they hold, `from` the minute they were written `to` the end of the last day.
#[derive(Serialize)]
struct Ahead {
    v: u32,
    profile: u16,
    lang: String,
    build: String,
    /// `fnv` of `made_of`.
    hash: u32,
    data: String,
    from: i32,
    to: i32,
    s: Vec<Stretch>,
}

/// From minute to minute (`minute_index`, the end not its own), the photo's HTML, and what goes
/// by the minute in it.
#[derive(Serialize)]
struct Stretch(i32, i32, String, Counts);

/// What goes by the minute within a stretch, each in the part of the photo marked for it
/// (`data-count`).
#[derive(Default, Serialize)]
struct Counts {
    /// The sentence of a running Termin („noch 55 Minuten").
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<Line>,
    /// What the card says of the minutes, minute by minute from the stretch's start.
    #[serde(skip_serializing_if = "Option::is_none")]
    card: Option<Vec<String>>,
    /// How far the clock's running week is filled at the stretch's start, in percent; a week is
    /// 10 080 minutes, so a minute fills 1/100.8 of a percent more (`super::clock`).
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<f64>,
}

/// The sentence with `SENTINEL` for the minutes left until `at` (`minute_index`), and as it reads
/// with one.
#[derive(Serialize)]
struct Line {
    t: String,
    one: String,
    at: i32,
}

/// What the photo is made of, as the script reads it: „Mein Studiengang"'s text, the Stundenplan's,
/// and the Stundenplan's program of the tab's session where it is another than „Mein
/// Studiengang"'s (taking a semester over sets it to that one, in that tab alone), a line between
/// each.
pub fn made_of(program: &str, plan: &str, session: &str) -> String {
    let own = program.split('\n').find_map(|line| line.strip_prefix("program\t")).unwrap_or_default();
    let other = if session == own { "" } else { session };
    format!("{program}\n{plan}\n{other}")
}

/// A minute as one number: the day's count since 1970-01-01 times 1440, and the minute of it.
/// The script counts the browser's local date and time the same way.
pub fn minute_index(day: Day, minute: u16) -> i32 {
    day.0 * DAY + i32::from(minute)
}

/// FNV-1a over the UTF-16 units of `text`, as the script's loop computes it.
pub fn fnv(text: &str) -> u32 {
    text.encode_utf16().fold(0x811c_9dc5_u32, |hash, unit| (hash ^ u32::from(unit)).wrapping_mul(0x0100_0193))
}

/// Writes the day ahead of the page's day `live` once it stands, and again when what it is made
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
        let ready = live.key.with(Option::is_some) && live.table.with(Option::is_some);
        live.facts.track();
        live.names.track();
        live.planned.track();
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
#[derive(PartialEq)]
struct Made {
    profile: u16,
    lang: String,
    build: String,
    hash: u32,
    /// The catalog the page answered from (boot.js's `betulaData.etag`): the script cannot ask it,
    /// but the app writes the days anew for another one.
    data: String,
}

/// The days kept as far as `write` reads them: what they are made of, and from when to when (the
/// stretches skipped, not copied).
#[cfg(feature = "csr")]
#[derive(serde::Deserialize)]
struct Head {
    v: u32,
    profile: u16,
    lang: String,
    build: String,
    hash: u32,
    data: String,
    from: i32,
    to: i32,
}

#[cfg(feature = "csr")]
impl Head {
    /// Whether they are made of `made` and hold the minutes from `start` to `end`.
    fn holds(self, made: &Made, start: i32, end: i32) -> bool {
        let Head { v, profile, lang, build, hash, data, from, to } = self;
        v == VERSION && Made { profile, lang, build, hash, data } == *made && from <= start && to >= end
    }
}

/// The stretches written in one go, before the page's thread is let go of again: a stretch takes
/// a phone a few milliseconds.
#[cfg(feature = "csr")]
const SLICE: usize = 4;

/// Works out the stretches from now on and keeps them, a few at a time (`SLICE`); nothing where
/// the days kept still hold from now on.
#[cfg(feature = "csr")]
fn write(live: Ctx, build: String, run: StoredValue<u32>) {
    use folia_design::nav;
    use folia_stores::profiles::{key_of, Profiles, AHEAD_KEY};

    let Some(this) = run.try_get_value() else { return };
    let t = crate::i18n::t();
    let Some(profiles) = Profiles::expect() else { return };
    let id = profiles.active_untracked();
    let (day, minute) = live.now.get_untracked();
    let start = minute_index(day, minute);
    let next = live.today.with_untracked(|today| today.next_day.as_ref().map(|(next, _)| *next));
    let last = next.map_or(day.plus(1), |next| next.max(day.plus(1))).min(day.plus(MOST_DAYS));
    let end = minute_index(last.plus(1), 0);
    let kept = |key: &str| nav::local_get(&key_of(key, id)).unwrap_or_default();
    let session = nav::session_get(&key_of(folia_stores::studyplan::PROGRAM_KEY, id)).unwrap_or_default();
    let text = made_of(&kept(folia_stores::myprogram::STORAGE_KEY), &kept(folia_stores::studyplan::STORAGE_KEY), &session);
    let made = Made { profile: id, lang: t.locale.code().to_string(), build, hash: fnv(&text), data: snapshot() };
    let held = nav::local_get(AHEAD_KEY).and_then(|text| serde_json::from_str::<Head>(&text).ok());
    if held.is_some_and(|held| held.holds(&made, start, end)) {
        return;
    }
    let cuts = cuts(live, day, last, start, end);
    let Some(child) = Owner::current().map(|owner| owner.child()) else { return };
    let at = child.with(|| live.at(RwSignal::new((day, minute))));
    let job = Job { at, child, pairs: cuts.iter().copied().zip(cuts.iter().copied().skip(1)).collect(), done: Vec::new(), made, from: start, to: end, run, this, t };
    slices(job, 0);
}

/// A run of `write`: the page's day at the minutes to come (`at`, of its own owner), the stretches
/// still to write and those written.
#[cfg(feature = "csr")]
struct Job {
    at: Ctx,
    child: Owner,
    pairs: Vec<(i32, i32)>,
    done: Vec<Stretch>,
    made: Made,
    from: i32,
    to: i32,
    run: StoredValue<u32>,
    this: u32,
    t: &'static crate::i18n::Texts,
}

/// Writes the stretches from `next` on, `SLICE` of them now and the rest after the page's thread
/// was free, and keeps them all once they are written, as long as no newer run came.
#[cfg(feature = "csr")]
fn slices(mut job: Job, next: usize) {
    use folia_design::nav;
    use folia_stores::profiles::AHEAD_KEY;

    if job.run.try_get_value() != Some(job.this) {
        job.child.cleanup();
        return;
    }
    let until = (next + SLICE).min(job.pairs.len());
    let (at, t) = (job.at, job.t);
    let written: Vec<Stretch> = job.child.with(|| job.pairs.get(next..until).unwrap_or_default().iter().filter_map(|&(from, to)| stretch(at, from, to, t)).collect());
    job.done.extend(written);
    if until < job.pairs.len() {
        set_timeout(move || slices(job, until), std::time::Duration::ZERO);
        return;
    }
    job.child.cleanup();
    let Made { profile, lang, build, hash, data } = job.made;
    let ahead = Ahead { v: VERSION, profile, lang, build, hash, data, from: job.from, to: job.to, s: job.done };
    if let Ok(json) = serde_json::to_string(&ahead) {
        if json.len() <= MOST_BYTES {
            nav::local_set(AHEAD_KEY, &json);
        }
    }
}

/// The catalog the page's answers are from: the ETag of its snapshot, as boot.js has it
/// (`window.betulaData.etag`); empty where there is none.
#[cfg(feature = "csr")]
fn snapshot() -> String {
    use web_sys::js_sys::Reflect;
    let data = Reflect::get(window().as_ref(), &"betulaData".into()).ok().filter(|data| data.is_object());
    data.and_then(|data| Reflect::get(&data, &"etag".into()).ok()).and_then(|etag| etag.as_string()).unwrap_or_default()
}

/// Where the photo changes from `start` to `end`: each midnight, each greeting and light of a
/// day, each Termin's beginning and end.
#[cfg(feature = "csr")]
fn cuts(live: Ctx, first: Day, last: Day, start: i32, end: i32) -> Vec<i32> {
    use std::collections::BTreeSet;

    use super::day::{self as daymodel, greeting};
    use super::photo;

    let t = crate::i18n::t();
    let words = daymodel::Words { locale: t.locale, exam: t.today.exam, own: t.termin.own_chip };
    let mut cuts = BTreeSet::from([start, end]);
    for n in first.0..=last.0 {
        let day = Day(n);
        cuts.insert(minute_index(day, 0));
        let mut before = None;
        for minute in 0..1440u16 {
            let state = (greeting(minute), photo::light(day, minute));
            if before.is_some_and(|before| before != state) {
                cuts.insert(minute_index(day, minute));
            }
            before = Some(state);
        }
        live.table.with_untracked(|table| {
            let Some(table) = table else { return };
            live.agenda.with_untracked(|agenda| {
                live.names.with_untracked(|(titles, names)| {
                    for entry in daymodel::today(table, agenda, titles, names, day, 0, &words).entries {
                        for minute in [entry.from, entry.to].into_iter().flatten() {
                            cuts.insert(minute_index(day, minute));
                        }
                    }
                });
            });
        });
    }
    cuts.into_iter().filter(|cut| (start..=end).contains(cut)).collect()
}

/// The photo from `from` to `to` as the page writes it at `from`, and how its marked parts count.
/// Built as the page builds it, into an element of no page, whose HTML is then read: the same
/// markup as the page's own.
#[cfg(feature = "csr")]
fn stretch(at: Ctx, from: i32, to: i32, t: &'static crate::i18n::Texts) -> Option<Stretch> {
    let day = Day(from.div_euclid(DAY));
    let minute = u16::try_from(from.rem_euclid(DAY)).ok()?;
    at.now.set((day, minute));
    let html = {
        let parent = document().create_element("div").ok()?;
        let mut built = view! { <Hero ctx=at on_setup=Callback::new(|()| {})/> }.build();
        built.mount(&parent, None);
        let html = parent.first_element_child().map(|hero| hero.outer_html());
        built.unmount();
        html?
    };
    let mut counts = Counts::default();
    // The sentence: counting down while a Termin runs.
    let running = at.today.with(|today| today.running().cloned());
    let said = sentence(at, t);
    counts.line = running.and_then(|entry| {
        let end = entry.to?;
        (said == running_line(&entry, end.saturating_sub(minute), t)).then(|| Line { t: running_line(&entry, SENTINEL, t), one: running_line(&entry, 1, t), at: minute_index(day, end) })
    });
    // The card's minutes, where it shows one of the day's Termine.
    counts.card = focus(at).map(|(entry, running)| (from..to).filter_map(|m| u16::try_from(m - minute_index(day, 0)).ok()).map(|m| card_minutes(&entry, running, m, t)).collect());
    // The clock's running week.
    counts.fill = at.clock.with(|clock| clock.as_ref().and_then(|clock| clock.pieces.iter().find_map(|piece| piece.fill)).map(|fill| fill * 100.0));
    Some(Stretch(from, to, html, counts))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script's hash is the app's: FNV-1a over UTF-16 units (an umlaut one unit, an emoji
    /// two), and its count of minutes is `minute_index`'s.
    #[test]
    fn the_script_counts_as_the_app_does() {
        assert_eq!(fnv(""), 0x811c_9dc5);
        assert_eq!(fnv("a"), 0xe40c_292c);
        assert_eq!(fnv("program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\n"), fnv("program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\n"));
        assert_ne!(fnv("ä"), fnv("a"));
        assert!(SCRIPT.contains("Math.imul(h,16777619)>>>0") && SCRIPT.contains("h=2166136261"), "the same FNV-1a");
        assert!(SCRIPT.contains("*1440+n.getHours()*60+n.getMinutes()") && SCRIPT.contains("/864e5"), "the same minutes");
        assert_eq!(minute_index(Day::from_ymd(2026, 10, 26).unwrap(), 9 * 60 + 15), 20_752 * 1440 + 555);
        // What it reads is what the stores keep, under the keys the profiles give them, in the
        // order of `made_of`.
        assert!(SCRIPT.contains(r#""betula.today.v1""#) && folia_stores::profiles::AHEAD_KEY == "betula.today.v1");
        assert!(SCRIPT.contains(r#"mp=localStorage.getItem(key("myprogram.v1"))||"",sp=sessionStorage.getItem(key("studyplan.program"))||"",own=(/^program\t(.*)$/m.exec(mp)||[])[1]||"",text=mp+"\n"+(localStorage.getItem(key("studyplan.v1"))||"")+"\n"+(sp!==own?sp:"")"#));
        assert_eq!(made_of("a", "b", "c"), "a\nb\nc");
        // The session's program counts where it is another than „Mein Studiengang"'s.
        let mine = "name\tInformatik\nprogram\t079-82-2008\nstart\t2026W\n";
        assert_eq!(made_of(mine, "m\t2026W\t12104\n", "079-82-2008"), made_of(mine, "m\t2026W\t12104\n", ""));
        assert_ne!(made_of(mine, "m\t2026W\t12104\n", "079-84-2008"), made_of(mine, "m\t2026W\t12104\n", ""));
        assert_eq!(folia_stores::myprogram::STORAGE_KEY, "betula.myprogram.v1");
        assert_eq!(folia_stores::studyplan::STORAGE_KEY, "betula.studyplan.v1");
        assert_eq!(folia_stores::studyplan::PROGRAM_KEY, "betula.studyplan.program");
        assert!(SCRIPT.contains(&format!("o.line.t.replace(\"{SENTINEL}\"")) && SCRIPT.contains(&format!("c.v!=={VERSION}")));
        // A week of minutes fills the clock's piece: a hundredth of it in 100.8 minutes.
        assert!(SCRIPT.contains("(m-g[0])/100.8") && (f64::from(7 * DAY) / 100.0 - 100.8).abs() < 1e-9);
        // Nothing in it ends the script element it stands in.
        assert!(!SCRIPT.contains("</"));
    }

    /// What is kept is what the script reads: a stretch is from, to, the HTML and what goes by the
    /// minute, by name and only where there is something.
    #[test]
    fn a_stretch_is_kept_as_the_script_reads_it() {
        let stretch = Stretch(1, 2, "<section></section>".into(), Counts { line: Some(Line { t: "noch 7777".into(), one: "noch 1".into(), at: 5 }), card: Some(vec!["in 1 min".into()]), fill: Some(12.5) });
        assert_eq!(serde_json::to_string(&stretch).unwrap(), r#"[1,2,"<section></section>",{"line":{"t":"noch 7777","one":"noch 1","at":5},"card":["in 1 min"],"fill":12.5}]"#);
        assert_eq!(serde_json::to_string(&Counts::default()).unwrap(), "{}");
        for part in ["o.line", "o.card", "o.fill", r#"at("line")"#, r#"at("card")"#, r#"at("fill")"#, "g[0]", "g[1]", "g[2]", "g[3]"] {
            assert!(SCRIPT.contains(part), "{part}");
        }
    }
}
