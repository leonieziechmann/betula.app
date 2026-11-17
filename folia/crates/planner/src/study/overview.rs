//! The overview of „Mein Studium" (owner, 2026-10-04: „eine Progress bar … wie viele Credits schon
//! erfüllt sind, wie viele geplant sind und wie viele noch gar nicht allocated sind … unterteilt nach
//! den Bereichen … mehr Credits als overflow"): what is passed of what the plan asks, the credits
//! passed, planned and not planned yet, and a bar of the areas, each as wide as it asks (and what
//! goes beyond it, striped, with „+6"). On a desktop a card per area under it, all in one row and
//! each in one line (owner, 2026-10-04: „Die Cards dürfen nur eine Zeile haben. Aber idealer weise
//! auch ohne Scrolling"): its name and what is taken of what it asks („44/66"), the rest when the
//! pointer rests on it. Nothing else: what was not passed, a semester heavier than the plan, is
//! said where it is (owner, 2026-10-04: „Das muss da weg dafür gibt es andere Bereiche. Da soll
//! einfach nur zu sehen sein, wie steht es bei mir um meine LP"). A phone has the bar and its
//! legend at the top of its overview (`phone.rs`), and the areas as a sheet (`AreasSheet`). An
//! area opens what counts there (`AreaDialog`).
//!
//! The sheet and an area's dialog as the Wahlpflicht looks (owner, 2026-11-17: „Also ich hätte
//! gerne überall eine progressbar, wie beim Wahlpflicht Katalog. Und bitte verwende icons für den
//! Fortschritt also bestanden, aktuell belegt, noch nicht belegt etc."; until then the sheet said
//! the overview's headline, bar and legend again and listed the areas in words, and the dialog
//! had a bar of its own and a box to tick at each module): every area with what it holds of what
//! it asks („62 / 66 LP") and the Wahlpflicht's bar in its tone (`choose::bar`), and its credits
//! by where they stand, each with the mark the semesters' rows have (`Tally`, `focus::Glyph`):
//! passed, under way in the current semester, planned for a later one, and a dashed ring for what
//! is asked for and not planned yet. The same marks stand at an area's modules and at the plan's
//! rows still open.

use folia_plans::choices::Stand;
use folia_plans::study::{AreaKind, Pick, Progress, Study, When};
use folia_plans::variants;
use folia_routes::filter::{CatalogQuery, ProgramScope};
use folia_routes::url::{CatalogUrl, ProgramTab, ProgramUrl};
use leptos::prelude::*;

use folia_design::ui::Icon;

use super::choose::bar as stand_bar;
use super::dialog::DialogHead;
use super::focus::{Glyph, OpenGlyph, Status};
use super::{n, Dialog, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// A part of the bar: an area, or what counts towards none.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Segment {
    area: Option<usize>,
    tone: &'static str,
    /// The parts' weights: passed, planned, open, beyond need.
    passed: f64,
    planned: f64,
    open: f64,
    over: f64,
    label: String,
}

/// The credits of an area, or of the study, by where they stand: passed, under way in the current
/// semester („aktuell belegt"), planned for a later one, and what is asked for and not planned
/// yet. Each has its mark (`tally_mark`): the statuses of a module (`focus::Status`), and the
/// dashed ring of what stands open.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Tally {
    pub passed: f64,
    pub running: f64,
    pub planned: f64,
    pub open: f64,
}

impl Tally {
    /// The tallies of the areas of the progress, and of what counts towards none: what the items
    /// of the semesters bring, as the progress counts them (`study::Progress`: what was not passed
    /// counts for nothing, a row that modules fill for what they leave of it). An area's add up to
    /// what its progress holds — passed and planned, what goes beyond its need included.
    pub fn of(study: &Study) -> (Vec<Tally>, Tally) {
        let mut areas: Vec<Tally> = study.progress.iter().map(|progress| Tally { open: progress.open(), ..Tally::default() }).collect();
        let mut outside = Tally::default();
        for semester in &study.semesters {
            for item in semester.items.iter().filter(|item| item.passed || !item.failed) {
                let Some(credits) = item.counted().filter(|credits| *credits > 0.0) else { continue };
                let tally = item.area.and_then(|area| areas.get_mut(area)).unwrap_or(&mut outside);
                match (item.passed, semester.when) {
                    (true, _) => tally.passed += credits,
                    (false, When::Now) => tally.running += credits,
                    (false, _) => tally.planned += credits,
                }
            }
        }
        (areas, outside)
    }

    /// The areas' together.
    fn sum(tallies: &[Tally]) -> Tally {
        tallies.iter().fold(Tally::default(), |sum, tally| Tally { passed: sum.passed + tally.passed, running: sum.running + tally.running, planned: sum.planned + tally.planned, open: sum.open + tally.open })
    }

    /// Its parts in the order of a study: the mark, the credits, the word.
    fn parts(&self, t: &Texts) -> [(Option<Status>, f64, &'static str); 4] {
        let s = &t.study;
        [(Some(Status::Passed), self.passed, s.passed_short), (Some(Status::Running), self.running, s.running_short), (Some(Status::Planned), self.planned, s.planned_short), (None, self.open, s.open_short)]
    }

    /// „34 bestanden, 16 läuft, 12 geplant, 4 offen": its parts in words, for a screen reader.
    fn words(&self, t: &Texts) -> String {
        self.parts(t).into_iter().filter(|(_, credits, _)| *credits > 0.0).map(|(_, credits, word)| format!("{} {word}", n(credits, t))).collect::<Vec<_>>().join(", ")
    }
}

/// The mark of a part of a tally: a status's, or the dashed ring of what stands open.
fn tally_mark(status: Option<Status>) -> AnyView {
    match status {
        Some(status) => view! { <Glyph status=status/> }.into_any(),
        None => view! { <OpenGlyph/> }.into_any(),
    }
}

/// A tally as a strip of four (app.css `.st-tally`): each part's mark and credits over its word,
/// all four always, so that the strip says what the marks are wherever they stand under it.
fn tally_strip(tally: &Tally, t: &'static Texts) -> impl IntoView {
    view! {
        <ul class="st-tally" aria-label=t.study.tally_label>
            {tally.parts(t).into_iter().map(|(status, credits, word)| view! {
                <li class:none=credits <= 0.0>
                    <span class="st-tally-n">{tally_mark(status)}<b>{n(credits, t)}</b></span>
                    <span class="st-tally-w">{word}</span>
                </li>
            }).collect_view()}
        </ul>
    }
}

/// A tally in a line (app.css `.st-tally-line`): the marks of the parts it has with their
/// credits, „✓ 34 ◐ 16 ○ 12", and what is said after them („+6 LP über Bedarf", „laut Plan im
/// 5. FS").
fn tally_line(tally: &Tally, after: Option<(String, &'static str)>, t: &'static Texts) -> impl IntoView {
    let words = tally.words(t);
    view! {
        <span class="st-tally-line">
            <span class="visually-hidden">{words}</span>
            {tally.parts(t).into_iter().filter(|(_, credits, _)| *credits > 0.0).map(|(status, credits, word)| view! {
                <span class="st-tally-part" title=word aria-hidden="true">{tally_mark(status)}{n(credits, t)}</span>
            }).collect_view()}
            {after.map(|(text, class)| view! { <span class=format!("st-tally-after {class}")>{text}</span> })}
        </span>
    }
}

/// What a stand holds of what it asks, as the Wahlpflicht writes it: „62 / 66 LP"; where nothing
/// is asked (what counts towards no area), „6 LP".
fn figure_of(stand: &Stand, t: &'static Texts) -> impl IntoView {
    let unit = t.study_views.col_credits;
    let rest = if stand.least > 0.0 { format!(" / {}\u{a0}{unit}", n(stand.least, t)) } else { format!("\u{a0}{unit}") };
    view! { <span class="st-wfig"><b>{n(stand.taken(), t)}</b>{rest}</span> }
}

/// An area's stand as the Wahlpflicht's bar draws one (`choose::bar`): what it holds, what goes
/// beyond its need included, against what it asks; the modules beyond need hatched.
fn stand_of(progress: &Progress) -> Stand {
    let passed = progress.passed + progress.over_passed;
    Stand { passed, planned: progress.planned + progress.over - progress.over_passed, least: progress.area.required, most: progress.area.required, spare: progress.spare }
}

/// An area's card.
#[derive(Clone, Debug, PartialEq)]
struct Card {
    area: Option<usize>,
    name: String,
    tone: &'static str,
    /// What it holds against what it asks: its bar. What counts towards no area asks for nothing,
    /// all of it hatched.
    stand: Stand,
    tally: Tally,
    /// What its line says after its marks, and its class: what is beyond its need, or for an area
    /// nothing is planned for yet, where the plan puts it.
    after: Option<(String, &'static str)>,
    /// A desktop's card in one line: what is taken of what it asks („44/66"), and its class.
    figure: (String, &'static str),
    /// All of it in words, for the pointer resting on the card.
    title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Info {
    program: String,
    pub headline: String,
    /// „46 LP" passed, planned (and what of it goes beyond need, or counts nowhere), open.
    pub passed: String,
    pub planned: String,
    planned_rest: String,
    pub open: String,
    segments: Vec<Segment>,
    cards: Vec<Card>,
    /// The study's credits by where they stand, what counts towards no area included.
    tally: Tally,
}

/// „5.–6. FS": the Fachsemester an area's rows lie in.
pub(super) fn span_of(ready: &Ready, area: usize, t: &Texts) -> Option<String> {
    ready.named.get(area)?.span.map(|(from, to)| (t.study.fs_span)(from, to))
}

impl Info {
    pub fn of(ready: &Ready, t: &Texts) -> Self {
        let s = &t.study;
        let study = &ready.study;
        let program = format!("{} {}", ready.program.name, ready.program.degree());
        // The numbers of the whole as the start page says them too (`study::Totals`).
        let totals = study.totals();
        let headline = match totals.headline() {
            (passed, Some(total)) => (s.headline)(&n(passed, t), &n(total, t)),
            (passed, None) => (s.headline_alone)(&n(passed, t)),
        };
        let mut planned_rest = String::new();
        // „über Bedarf" is what the modules beyond need bring; what a module the area needs brings
        // past it (8 LP where 6 were left) is no more than planned (owner, 2026-11-09).
        if totals.spare.1 > 0.0 {
            planned_rest.push_str(&(s.over_part)(&n(totals.spare.1, t)));
        }
        if totals.outside.1 > 0.0 {
            planned_rest.push_str(&(s.outside_part)(&n(totals.outside.1, t)));
        }

        let (tallies, outside_tally) = Tally::of(study);
        let mut segments = Vec::new();
        let mut cards = Vec::new();
        for (i, progress) in study.progress.iter().enumerate() {
            let named = ready.named.get(i);
            let (name, tone) = named.map_or((String::new(), "var(--text-3)"), |named| (named.short.clone(), named.tone));
            let open = progress.open();
            if progress.area.required + progress.spare > 0.0 {
                segments.push(Segment {
                    area: Some(i),
                    tone,
                    passed: progress.passed,
                    planned: progress.planned,
                    open,
                    over: progress.spare,
                    label: (s.bar_area)(&name, &n(progress.passed, t), &n(progress.planned, t), &n(progress.area.required, t)),
                });
            }
            let stand = stand_of(progress);
            let tally = tallies.get(i).copied().unwrap_or_default();
            let state = if progress.spare > 0.0 {
                ((s.card_over)(&n(progress.spare, t)), "over")
            } else if open > 0.0 {
                ((s.card_open)(&n(open, t)), "open")
            } else {
                (s.all_planned.to_string(), "done")
            };
            // After the marks: what is beyond need; where nothing is planned yet, where the plan
            // puts the area.
            let after = if progress.spare > 0.0 {
                Some((state.0.clone(), "over"))
            } else if stand.taken() <= 0.0 {
                span_of(ready, i, t).map(|span| ((s.plan_says)(&span), ""))
            } else {
                None
            };
            let taken = n(stand.taken(), t);
            let required = n(progress.area.required, t);
            let figure = (format!("{taken}/{required}"), if state.1 == "open" { "" } else { state.1 });
            let title = (s.card_title)(&name, &taken, &required, &state.0);
            cards.push(Card { area: Some(i), name, tone, stand, tally, after, figure, title });
        }
        let (outside_passed, outside_planned) = study.outside;
        if outside_passed + outside_planned > 0.0 {
            segments.push(Segment {
                area: None,
                tone: "var(--text-3)",
                passed: 0.0,
                planned: 0.0,
                open: 0.0,
                over: outside_passed + outside_planned,
                label: format!("{}: {}", s.outside, (s.credits)(&n(outside_passed + outside_planned, t))),
            });
            let all = (s.credits)(&n(outside_passed + outside_planned, t));
            let title = format!("{}: {all}", s.outside);
            // It asks for nothing: all of it beyond need, hatched as on the bar of the areas.
            let stand = Stand { passed: outside_passed, planned: outside_planned, least: 0.0, most: 0.0, spare: outside_passed + outside_planned };
            cards.push(Card { area: None, name: s.outside.to_string(), tone: "var(--text-3)", stand, tally: outside_tally, after: None, figure: (all, ""), title });
        }
        let mut tally = Tally::sum(&tallies);
        tally.passed += outside_tally.passed;
        tally.running += outside_tally.running;
        tally.planned += outside_tally.planned;
        Info {
            program,
            headline,
            passed: (s.credits)(&n(totals.passed_all(), t)),
            planned: (s.credits)(&n(totals.planned_all(), t)),
            planned_rest,
            open: (s.credits)(&n(totals.open, t)),
            segments,
            cards,
            tally,
        }
    }

    /// The bar of the areas.
    pub fn segments(&self) -> Vec<Segment> {
        self.segments.clone()
    }

    /// Some part of the bar goes beyond what its area asks.
    pub fn has_over(&self) -> bool {
        self.segments.iter().any(|segment| segment.over > 0.0)
    }
}

/// The overview on a desktop.
#[component]
pub(super) fn Overview(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| ctx.with_ready(|ready| Info::of(ready, t)));
    move || {
        info.get().map(|info| {
            let kicker = format!("{} · {}", s.overview, info.program);
            view! {
                <section class="panel st-over" aria-label=s.overview>
                    <div class="st-over-top">
                        <div class="st-over-title">
                            <p class="st-kicker">{kicker}</p>
                            <p class="st-headline">{info.headline.clone()}</p>
                        </div>
                        <ul class="st-stats">
                            <li><span class="st-key passed"></span><b>{info.passed.clone()}</b>" "{s.passed_tail}</li>
                            <li><span class="st-key planned"></span><b>{info.planned.clone()}</b>" "{s.planned_tail}{info.planned_rest.clone()}</li>
                            <li><span class="st-key open"></span><b>{info.open.clone()}</b>" "{s.open_tail}</li>
                        </ul>
                    </div>
                    <Bar segments=info.segments.clone()/>
                    <div class="st-cards">
                        {info.cards.into_iter().map(|card| view! { <AreaCard ctx card/> }).collect_view()}
                    </div>
                </section>
            }
        })
    }
}

/// The bar of the areas, each as wide as it asks and what goes beyond it.
#[component]
pub(super) fn Bar(segments: Vec<Segment>) -> impl IntoView {
    let t = i18n::t();
    let label = segments.iter().map(|segment| segment.label.clone()).collect::<Vec<_>>().join("; ");
    view! {
        <div class="st-bar" role="img" aria-label=format!("{}: {label}", t.study.bar_label)>
            {segments.into_iter().map(|segment| {
                let weight = segment.passed + segment.planned + segment.open + segment.over;
                let part = |class: &'static str, value: f64| (value > 0.0).then(|| view! { <span class=class style=format!("flex-grow: {value}")></span> });
                view! {
                    <span class="st-seg" class:outside=segment.area.is_none() style=format!("flex-grow: {weight}; --c: {}", segment.tone)>
                        {part("passed", segment.passed)}
                        {part("planned", segment.planned)}
                        {part("open", segment.open)}
                        {part("over", segment.over)}
                        {(segment.over > 0.0 && segment.area.is_some()).then(|| view! { <b class="st-plus">{format!("+{}", super::n(segment.over, t))}</b> })}
                    </span>
                }
            }).collect_view()}
        </div>
    }
}

/// An area's card on a desktop, in one line: „● Informatik 44/66".
#[component]
fn AreaCard(ctx: StudyCtx, card: Card) -> impl IntoView {
    let area = card.area;
    let (figure, class) = card.figure;
    view! {
        <button class="st-card" type="button" style=format!("--c: {}", card.tone) title=card.title.clone() aria-label=card.title on:click=move |_| ctx.open(Dialog::Area(area))>
            <span class="st-dot"></span>
            <span class="st-card-name">{card.name}</span>
            <span class=format!("st-card-figure {class}")>{figure}{(class == "done").then(|| view! { <Icon name="check"/> })}</span>
        </button>
    }
}

/// „Übersicht nach Bereichen": the areas on a phone, each a way to its dialog. Under the head what
/// is passed of what the plan asks and the study's credits by where they stand, a strip of the
/// four marks with their words; then every area as the Wahlpflicht has a choice: its name and
/// what it holds of what it asks, its bar in its tone, and its marks with their credits. Not the
/// overview's bar and legend again, which stand right behind the sheet: half up it shows the
/// areas.
#[component]
pub(super) fn AreasSheet(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| ctx.with_ready(|ready| Info::of(ready, t)));
    move || {
        info.get().map(|info| view! {
            <DialogHead ctx title=s.areas_title sub=info.headline.clone()/>
            <div class="st-dlg-body st-areas">
                {tally_strip(&info.tally, t)}
                <ul class="st-area-list">
                    {info.cards.into_iter().map(|card| {
                        let area = card.area;
                        view! {
                            <li>
                                <button type="button" style=format!("--c: {}", card.tone) on:click=move |_| ctx.open(Dialog::Area(area))>
                                    <span class="st-area-t"><span class="st-dot"></span><b>{card.name}</b>{figure_of(&card.stand, t)}<Icon name="chevron-right"/></span>
                                    {stand_bar(&card.stand, None, "", t)}
                                    {tally_line(&card.tally, card.after, t)}
                                </button>
                            </li>
                        }
                    }).collect_view()}
                </ul>
            </div>
        })
    }
}

/// What an area's dialog lists of an item.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    id: Option<String>,
    name: String,
    credits: Option<String>,
    /// Where it stands: passed, under way in the current semester, planned for a later one.
    status: Status,
    /// Its semester, „WiSe 2026/27".
    when: String,
    over: bool,
    /// Counted here by the student where the plan does not put it (`study::Item::exception`).
    exception: bool,
}

/// A row of the plan an area's dialog lists as open.
#[derive(Clone, Debug, PartialEq)]
struct OpenRow {
    /// Its module, where the plan names one: the row opens it.
    id: Option<String>,
    name: String,
    credits: Option<String>,
    /// Not passed in a semester that is over, and not planned again.
    failed: bool,
    /// „laut Plan im 5. FS", „nicht bestanden im SoSe 2026".
    stands: Option<String>,
}

/// What an area's dialog shows.
#[derive(Clone, Debug, PartialEq)]
struct AreaInfo {
    name: String,
    tone: &'static str,
    /// „62 / 66 LP"; „6 LP" for what counts towards no area.
    asks: String,
    stand: Stand,
    tally: Tally,
    over_text: Option<String>,
    entries: Vec<Entry>,
    /// The plan's rows of the area that are open.
    rows: Vec<OpenRow>,
    catalog: Option<(String, &'static str)>,
    rules: Option<String>,
}

impl AreaInfo {
    fn of(ready: &Ready, area: Option<usize>, t: &Texts) -> Option<Self> {
        let s = &t.study;
        let study = &ready.study;
        let mut entries = Vec::new();
        for semester in &study.semesters {
            // A row that modules fill counts by them, and with what they leave of it: they stand
            // here in its place.
            for item in semester.items.iter().filter(|item| item.area == area && !item.failed && (item.fillers.is_empty() || item.counted().is_some())) {
                let credits = item.counted().map(|credits| (s.credits)(&n(credits, t)));
                let status = match (item.passed, semester.when) {
                    (true, _) => Status::Passed,
                    (false, When::Now) => Status::Running,
                    (false, _) => Status::Planned,
                };
                entries.push(Entry { id: item.module_id().map(str::to_string), name: item.name.clone(), credits, status, when: semester.key.label(t.locale), over: item.spare, exception: item.exception });
            }
        }
        let (tallies, outside_tally) = Tally::of(study);
        let Some(index) = area else {
            let total: f64 = study.outside.0 + study.outside.1;
            return Some(AreaInfo {
                name: s.outside.to_string(),
                tone: "var(--text-3)",
                asks: (s.credits)(&n(total, t)),
                stand: Stand { passed: study.outside.0, planned: study.outside.1, least: 0.0, most: 0.0, spare: total },
                tally: outside_tally,
                over_text: None,
                entries,
                rows: Vec::new(),
                catalog: None,
                rules: (total > 0.0).then(|| ProgramUrl::new(&ready.program.slug, ProgramTab::Areas).path()),
            });
        };
        let progress = study.progress.get(index)?;
        let named = ready.named.get(index)?;
        let stand = stand_of(progress);
        // The plan's rows of the area that are open, each with where the plan puts it; a
        // Wiederholer with the semester it was not passed in.
        let rows = ready
            .lines
            .iter()
            .filter(|line| line.suggestion.area == Some(index))
            .map(|line| {
                let stands = match (line.failed_in, line.plan_fs) {
                    (Some(failed), _) => Some((s.failed_in)(&failed.label(t.locale))),
                    (None, Some(fs)) => Some((s.plan_says)(&(s.fs_span)(fs, fs))),
                    (None, None) => None,
                };
                let id = match &line.suggestion.pick {
                    Pick::Module { id, .. } => Some(id.clone()),
                    Pick::Row { .. } => None,
                };
                let credits = line.suggestion.credits_text.clone().or_else(|| line.suggestion.credits.map(|credits| n(credits, t))).map(|credits| (s.credits)(&credits));
                OpenRow { id, name: line.suggestion.name.clone(), credits, failed: line.failed_in.is_some(), stands }
            })
            .collect();
        let scope = ProgramScope { program_slug: ready.program.slug.clone(), ..Default::default() };
        let catalog = match progress.area.kind {
            // The program's FÜS list, the BTU's where it has none.
            AreaKind::Fues => Some((CatalogUrl { query: variants::fues_list_for(&ready.program, t.locale).0, ..Default::default() }.path(), s.fues_catalog)),
            AreaKind::Plan | AreaKind::Other if !progress.area.tree.is_empty() => {
                let areas: Vec<i64> = progress.area.tree.iter().copied().collect();
                Some((CatalogUrl { query: CatalogQuery { program: Some(ProgramScope { areas, ..scope }), ..Default::default() }, ..Default::default() }.path(), s.area_catalog))
            }
            _ => None,
        };
        let shown = ready.setup.shown.map_or(1, |shown| shown + 1);
        Some(AreaInfo {
            name: named.full.clone(),
            tone: named.tone,
            asks: (t.study_views.figure)(&n(stand.taken(), t), &n(progress.area.required, t)),
            stand,
            tally: tallies.get(index).copied().unwrap_or_default(),
            // „Über Bedarf" is what the modules beyond need bring, as the bar hatches it; what a
            // module the area needs brings past it (8 LP where 6 were left) is no more than taken.
            over_text: (progress.spare > 0.0).then(|| (s.area_over_text)(&n(progress.spare, t))),
            entries,
            rows,
            catalog,
            rules: Some(ProgramUrl::new(&ready.program.slug, ProgramTab::Plan).with_variant(shown).path()),
        })
    }
}

/// An area: what it holds of what it asks under its name, the Wahlpflicht's bar in its tone, its
/// credits by where they stand; what counts there, each module with the mark of its status and its
/// semester; the plan's rows still open, with the dashed ring (a cross for one not passed and not
/// planned again), a row of a module a way to it; and the ways to its modules and its rules.
#[component]
pub(super) fn AreaDialog(ctx: StudyCtx, area: Option<usize>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| ctx.with_ready(|ready| AreaInfo::of(ready, area, t)).flatten());
    move || {
        let Some(info) = info.get() else { return ().into_any() };
        let tone = info.tone;
        // A row: its mark, its name over what is said of it, its credits; with a module a way to it
        // beside the page, which closes the dialog.
        let row = move |id: Option<String>, mark: AnyView, word: &'static str, name: String, sub: AnyView, credits: Option<String>| {
            let inner = view! {
                <span class="st-wmk" title=word>{mark}<span class="visually-hidden">{word}</span></span>
                <span class="st-area-nm"><b>{name}</b>{sub}</span>
                <span class="st-lp">{credits}</span>
            };
            match id {
                Some(id) => {
                    let href = super::module_href(ctx, &id, t);
                    view! { <li><a class="st-area-row" href=href data-noscroll="" on:click=move |_| ctx.dialog.set(None)>{inner}<Icon name="chevron-right"/></a></li> }.into_any()
                }
                None => view! { <li><div class="st-area-row">{inner}</div></li> }.into_any(),
            }
        };
        let area_name = info.name.clone();
        view! {
            <DialogHead ctx title=info.name.clone() sub=info.asks.clone() tone=tone/>
            <div class="st-dlg-body st-area-dlg" style=format!("--c: {tone}")>
                {stand_bar(&info.stand, None, "big", t)}
                {tally_strip(&info.tally, t)}
                {info.over_text.map(|text| view! { <p class="st-dlg-note">{text}</p> })}
                <section>
                    <h3>{s.your_modules}</h3>
                    {if info.entries.is_empty() {
                        view! { <p class="hint">{s.none_here}</p> }.into_any()
                    } else {
                        view! {
                            <ul class="st-area-items">
                                {info.entries.into_iter().map(|entry| {
                                    let sub = view! {
                                        <small>{entry.when}{entry.exception.then(|| view! { <span class="flag st-exception" title=(s.exception_title)(&area_name)>{s.exception}</span> })}{entry.over.then(|| view! { <span class="st-chip over">{s.over_need}</span> })}</small>
                                    };
                                    row(entry.id, tally_mark(Some(entry.status)), entry.status.word(t), entry.name, sub.into_any(), entry.credits)
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }}
                </section>
                {(!info.rows.is_empty()).then(|| view! {
                    <section>
                        <h3>{s.plan_rows}</h3>
                        <ul class="st-area-items">
                            {info.rows.into_iter().map(|open| {
                                let (mark, word) = if open.failed { (tally_mark(Some(Status::Failed)), Status::Failed.word(t)) } else { (tally_mark(None), s.status_open) };
                                let sub = open.stands.map(|stands| view! { <small>{stands}</small> });
                                row(open.id, mark, word, open.name, sub.into_any(), open.credits)
                            }).collect_view()}
                        </ul>
                    </section>
                })}
                <nav class="st-area-ways" aria-label=s.more_area>
                    {info.catalog.map(|(href, label)| view! { <a class="action" href=t.path(&href)><Icon name="search"/><span>{label}</span><Icon name="chevron-right"/></a> })}
                    {info.rules.map(|href| view! { <a class="action" href=t.path(&href)><Icon name="file-check-2"/><span>{s.area_rules}</span><Icon name="chevron-right"/></a> })}
                </nav>
            </div>
        }
        .into_any()
    }
}
