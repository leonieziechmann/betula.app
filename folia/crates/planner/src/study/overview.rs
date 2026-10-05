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

use folia_plans::study::AreaKind;
use folia_routes::filter::{CatalogQuery, ProgramRelation, ProgramScope};
use folia_routes::url::{CatalogUrl, ProgramTab, ProgramUrl};
use leptos::prelude::*;

use folia_design::ui::Icon;

use super::dialog::DialogHead;
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

/// An area's card.
#[derive(Clone, Debug, PartialEq)]
struct Card {
    area: Option<usize>,
    name: String,
    tone: &'static str,
    required: Option<String>,
    line: String,
    /// What it still needs, all planned, or beyond need: the words and their class.
    state: (String, &'static str),
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
        let passed_all = study.passed + study.over_passed + study.outside.0;
        let over_planned = study.over - study.over_passed;
        let planned_all = study.planned + over_planned + study.outside.1;
        let headline = if study.total > 0.0 { (s.headline)(&n(study.passed, t), &n(study.total, t)) } else { (s.headline_alone)(&n(passed_all, t)) };
        let mut planned_rest = String::new();
        if over_planned > 0.0 {
            planned_rest.push_str(&(s.over_part)(&n(over_planned, t)));
        }
        if study.outside.1 > 0.0 {
            planned_rest.push_str(&(s.outside_part)(&n(study.outside.1, t)));
        }

        let mut segments = Vec::new();
        let mut cards = Vec::new();
        for (i, progress) in study.progress.iter().enumerate() {
            let named = ready.named.get(i);
            let (name, tone) = named.map_or((String::new(), "var(--text-3)"), |named| (named.short.clone(), named.tone));
            let open = progress.open();
            if progress.area.required + progress.over > 0.0 {
                segments.push(Segment {
                    area: Some(i),
                    tone,
                    passed: progress.passed,
                    planned: progress.planned,
                    open,
                    over: progress.over,
                    label: (s.bar_area)(&name, &n(progress.passed, t), &n(progress.planned, t), &n(progress.area.required, t)),
                });
            }
            let mut parts = Vec::new();
            if progress.passed + progress.over_passed > 0.0 {
                parts.push((s.card_passed)(&n(progress.passed + progress.over_passed, t)));
            }
            let planned = progress.planned + progress.over - progress.over_passed;
            if planned > 0.0 {
                parts.push((s.card_planned)(&n(planned, t)));
            }
            let line = match parts.is_empty() {
                false => parts.join(" · "),
                true => span_of(ready, i, t).map_or_else(|| s.nothing_yet.to_string(), |span| (s.plan_says)(&span)),
            };
            let state = if progress.over > 0.0 {
                ((s.card_over)(&n(progress.over, t)), "over")
            } else if open > 0.0 {
                ((s.card_open)(&n(open, t)), "open")
            } else {
                (s.all_planned.to_string(), "done")
            };
            let taken = n(progress.passed + progress.planned + progress.over, t);
            let required = n(progress.area.required, t);
            let figure = (format!("{taken}/{required}"), if state.1 == "open" { "" } else { state.1 });
            let title = (s.card_title)(&name, &taken, &required, &state.0);
            cards.push(Card { area: Some(i), name, tone, required: Some((s.credits)(&required)), line, state, figure, title });
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
            let mut parts = Vec::new();
            if outside_passed > 0.0 {
                parts.push((s.card_passed)(&n(outside_passed, t)));
            }
            if outside_planned > 0.0 {
                parts.push((s.card_planned)(&n(outside_planned, t)));
            }
            let all = (s.credits)(&n(outside_passed + outside_planned, t));
            let title = format!("{}: {all}", s.outside);
            cards.push(Card { area: None, name: s.outside.to_string(), tone: "var(--text-3)", required: None, line: parts.join(" · "), state: (String::new(), ""), figure: (all, ""), title });
        }
        Info {
            program,
            headline,
            passed: (s.credits)(&n(passed_all, t)),
            planned: (s.credits)(&n(planned_all, t)),
            planned_rest,
            open: (s.credits)(&n(study.open, t)),
            segments,
            cards,
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

/// „Übersicht nach Bereichen": the areas on a phone, each a way to its dialog.
#[component]
pub(super) fn AreasSheet(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| ctx.with_ready(|ready| Info::of(ready, t)));
    view! {
        <DialogHead ctx title=s.areas_title/>
        {move || info.get().map(|info| view! {
            <div class="st-dlg-body st-areas">
                <p class="st-headline small">{info.headline.clone()}</p>
                <p class="st-dlg-note">{format!("{} {}{} · {} {}", info.planned, s.planned_tail, info.planned_rest, info.open, s.open_tail)}</p>
                <Bar segments=info.segments.clone()/>
                <p class="st-stats-short">
                    <span><span class="st-key passed"></span>{s.passed_short}</span>
                    <span><span class="st-key planned"></span>{s.planned_short}</span>
                    <span><span class="st-key open"></span>{s.open_short}</span>
                    <span><span class="st-key over"></span>{s.over_need}</span>
                </p>
                <ul class="st-area-list">
                    {info.cards.into_iter().map(|card| {
                        let area = card.area;
                        view! {
                            <li>
                                <button type="button" style=format!("--c: {}", card.tone) on:click=move |_| ctx.open(Dialog::Area(area))>
                                    <span class="st-card-head"><span class="st-dot"></span><span class="st-card-name">{card.name}</span>{card.required.map(|required| view! { <span class="st-card-lp">{required}</span> })}<Icon name="chevron-right"/></span>
                                    <span class="st-card-line">{card.line}" · "<b class=card.state.1>{card.state.0}</b></span>
                                </button>
                            </li>
                        }
                    }).collect_view()}
                </ul>
            </div>
        })}
    }
}

/// What an area's dialog lists of an item.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    id: Option<String>,
    name: String,
    credits: Option<String>,
    passed: bool,
    when: String,
    over: bool,
}

/// What an area's dialog shows.
#[derive(Clone, Debug, PartialEq)]
struct AreaInfo {
    name: String,
    tone: &'static str,
    asks: Option<String>,
    required: f64,
    passed: f64,
    planned: f64,
    over: f64,
    over_text: Option<String>,
    entries: Vec<Entry>,
    /// The plan's rows of the area: name, credits, where it stands.
    rows: Vec<(String, Option<String>, String)>,
    catalog: Option<(String, &'static str)>,
    rules: Option<String>,
}

impl AreaInfo {
    fn of(ready: &Ready, area: Option<usize>, t: &Texts) -> Option<Self> {
        let s = &t.study;
        let study = &ready.study;
        let mut entries = Vec::new();
        for semester in &study.semesters {
            for item in semester.items.iter().filter(|item| item.area == area && !item.failed) {
                let credits = item.credits.map(|credits| (s.credits)(&n(credits, t)));
                let when = if item.passed { (s.passed_in)(&semester.key.label(t.locale)) } else { (s.planned_in)(&semester.key.label(t.locale)) };
                entries.push(Entry { id: item.module_id().map(str::to_string), name: item.name.clone(), credits, passed: item.passed, when, over: item.over > 0.0 });
            }
        }
        let Some(index) = area else {
            let total: f64 = study.outside.0 + study.outside.1;
            return Some(AreaInfo {
                name: s.outside.to_string(),
                tone: "var(--text-3)",
                asks: None,
                required: 0.0,
                passed: study.outside.0,
                planned: study.outside.1,
                over: 0.0,
                over_text: None,
                entries,
                rows: Vec::new(),
                catalog: None,
                rules: (total > 0.0).then(|| ProgramUrl::new(&ready.program.slug, ProgramTab::Areas).path()),
            });
        };
        let progress = study.progress.get(index)?;
        let named = ready.named.get(index)?;
        let taken = progress.passed + progress.planned + progress.over;
        // The plan's rows of the area, each with where it stands.
        let rows = ready
            .lines
            .iter()
            .filter(|line| line.suggestion.area == Some(index))
            .map(|line| {
                let stands = match (line.failed_in, line.plan_fs) {
                    (Some(failed), _) => (s.failed_in)(&failed.label(t.locale)),
                    (None, Some(fs)) => format!("{} · {}", s.row_open, (s.plan_says)(&(s.fs_span)(fs, fs))),
                    (None, None) => s.row_open.to_string(),
                };
                (line.suggestion.name.clone(), line.suggestion.credits_text.clone().or_else(|| line.suggestion.credits.map(|credits| n(credits, t))), stands)
            })
            .collect();
        let scope = ProgramScope { program_slug: ready.program.slug.clone(), ..Default::default() };
        let catalog = match progress.area.kind {
            AreaKind::Fues => Some((CatalogUrl { query: CatalogQuery { program: Some(ProgramScope { relation: ProgramRelation::Fues, ..scope }), ..Default::default() }, ..Default::default() }.path(), s.fues_catalog)),
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
            asks: Some((s.area_asks)(&n(progress.area.required, t), &n(taken, t))),
            required: progress.area.required,
            passed: progress.passed,
            planned: progress.planned,
            over: progress.over,
            over_text: (progress.over > 0.0).then(|| (s.area_over_text)(&n(progress.over, t))),
            entries,
            rows,
            catalog,
            rules: Some(ProgramUrl::new(&ready.program.slug, ProgramTab::Plan).with_variant(shown).path()),
        })
    }
}

/// An area: what it asks, what counts there, what goes beyond it, the plan's rows still open, and
/// the ways to its modules and its rules.
#[component]
pub(super) fn AreaDialog(ctx: StudyCtx, area: Option<usize>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.study;
    let info = Memo::new(move |_| ctx.with_ready(|ready| AreaInfo::of(ready, area, t)).flatten());
    move || {
        let Some(info) = info.get() else { return ().into_any() };
        let tone = info.tone;
        let whole = (info.required + info.over).max(info.passed + info.planned).max(1.0);
        let bar = (info.required > 0.0).then(|| {
            let at = |value: f64| format!("{:.3}%", value / whole * 100.0);
            view! {
                <div class="st-area-bar" style=format!("--c: {tone}") role="img" aria-label=info.asks.clone().unwrap_or_default()>
                    <span class="passed" style=format!("width: {}", at(info.passed))></span>
                    <span class="planned" style=format!("width: {}", at(info.planned))></span>
                    <span class="over" style=format!("width: {}", at(info.over))></span>
                    <i class="st-need" style=format!("left: {}", at(info.required))></i>
                </div>
                <p class="st-area-legend">
                    <span>{(s.card_passed)(&n(info.passed, t))}</span>
                    <span>{(s.card_planned)(&n(info.planned, t))}</span>
                    <span class="need">{(s.area_required)(&n(info.required, t))}</span>
                    {(info.over > 0.0).then(|| view! { <b class="over">{format!("+{}", n(info.over, t))}</b> })}
                </p>
            }
        });
        view! {
            <DialogHead ctx title=info.name.clone() sub=info.asks.clone().unwrap_or_default() tone=tone/>
            <div class="st-dlg-body st-area-dlg" style=format!("--c: {tone}")>
                {bar}
                {info.over_text.map(|text| view! { <p class="st-dlg-note">{text}</p> })}
                <section>
                    <h3>{s.your_modules}</h3>
                    {if info.entries.is_empty() {
                        view! { <p class="hint">{s.none_here}</p> }.into_any()
                    } else {
                        view! {
                            <ul class="st-area-items">
                                {info.entries.into_iter().map(|entry| {
                                    let name = match entry.id.clone() {
                                        Some(id) => {
                                            let href = super::module_href(ctx, &id, t);
                                            view! { <a href=href data-noscroll="" on:click=move |_| ctx.dialog.set(None)>{entry.name.clone()}</a> }.into_any()
                                        }
                                        None => view! { <span>{entry.name.clone()}</span> }.into_any(),
                                    };
                                    view! {
                                        <li class:passed=entry.passed>
                                            <span class="st-box" class:with=entry.passed aria-hidden="true">{entry.passed.then(|| view! { <Icon name="check"/> })}</span>
                                            <div>
                                                <div class="st-line">{name}<span class="st-lp">{entry.credits}</span></div>
                                                <div class="st-sub">{entry.when}{entry.over.then(|| view! { <span class="st-chip over">{s.over_need}</span> })}</div>
                                            </div>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }}
                </section>
                {(!info.rows.is_empty()).then(|| view! {
                    <section>
                        <h3>{s.plan_rows}</h3>
                        <ul class="st-area-rows">
                            {info.rows.into_iter().map(|(name, credits, stands)| view! {
                                <li><span>{name}</span><span class="st-sub">{stands}</span><span class="st-lp">{credits}</span></li>
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
