//! „Wahlpflicht", a view of „Mein Studium" (`/study?view=areas`; owner, 2026-10-14, concept A
//! „Ansichten"): what the plan leaves to choose (`folia_plans::choices`), by the areas of the
//! progress — the same areas, colours and figures as the bar — each choice with its lists and what
//! of them the semester offers, one click from being planned (owner, the same day: „Wahlpflicht
//! wird planbar: jedes Modul mit „In SoSe 27""). Betula blocks nothing: whatever is planned here
//! counts where the tree places it, and the list says what the student has of it.
//!
//! On a desktop under the overview: the choices on the left by area, the one picked on the right —
//! what the plan says of it, the semester to plan into (the one the plan puts it into and the one
//! after; „Alle" shows what neither offers as well), its lists as tabs where it has several
//! („Anwendungsfach": Mathematik, Physik …), and the list's modules. On a phone a page of its own
//! behind the overview: the areas as chips, the semester, and the area's choices one under the
//! other, their modules as cards.

use std::collections::BTreeMap;

use folia_calendar::semester::SemesterKey;
use folia_model::labels::{Labelled, OfferStatus, TurnusSeason};
use folia_model::rows::CatalogRow;
use folia_pages::ask::StudyModulesAsk;
use folia_plans::choices::{self, Choice, PlanView};
use folia_plans::study::{self, Input, Offer, Pick};
use folia_routes::filter::{CatalogQuery, ProgramRelation, ProgramScope};
use folia_routes::url::CatalogUrl;
use leptos::prelude::*;

use folia_design::format;
use folia_design::ui::{EmptyState, Icon};

use super::regular::view_back;
use super::{n, now_secs, Ready, StudyCtx};
use crate::i18n::{self, Texts};

/// Where a module of a list stands for the student.
#[derive(Clone, Debug, PartialEq)]
enum Has {
    No,
    Planned(SemesterKey),
    Passed,
}

/// A choice as the view shows it.
#[derive(Clone, Debug, PartialEq)]
struct Item {
    index: usize,
    name: String,
    area: Option<usize>,
    area_name: String,
    tone: &'static str,
    /// „12 LP", „10–24 LP".
    credits: String,
    /// „laut Plan im 3. und 4. FS".
    span: String,
    /// What the plan says of it, in words; what it says beyond its credits and Fachsemester.
    rule: String,
    rule_extra: Option<String>,
    fues: bool,
    /// Its lists: name and modules.
    lists: Vec<(String, Vec<String>)>,
    /// What is taken of it: name, credits, the stand.
    taken: Vec<(String, String)>,
    /// „8 von 12 LP gewählt", „Erledigt im SoSe 2026".
    taken_line: Option<String>,
    complete: bool,
    /// The semester the plan puts it into, not before the current one.
    due: SemesterKey,
    /// Where else to look: the FÜS list, the catalog of its row.
    catalog: Option<(String, &'static str)>,
}

/// An area of the progress with its choices.
#[derive(Clone, Debug, PartialEq)]
struct Area {
    area: Option<usize>,
    name: String,
    tone: &'static str,
    /// „0/18".
    figure: String,
    complete: bool,
    choices: Vec<usize>,
}

/// What the view shows.
#[derive(Clone, Debug, PartialEq)]
struct Pane {
    lead: String,
    areas: Vec<Area>,
    items: Vec<Item>,
    /// Every module of the lists, and where each stands for the student.
    has: BTreeMap<String, Has>,
    /// The first choice not complete.
    first: Option<usize>,
    now: SemesterKey,
}

impl Pane {
    fn of(input: &Input, ready: &Ready, ctx: StudyCtx, t: &'static Texts) -> Self {
        let v = &t.study_views;
        let study = &ready.study;
        let view: PlanView = choices::plan_view(input, study);
        let now = study.now;
        let due_of = |choice: &Choice| study::semester_of_fs(study.start, choice.first, &ready.setup.leave).map_or(now, |s| s.max(now));
        let items: Vec<Item> = view
            .choices
            .iter()
            .enumerate()
            .map(|(index, choice)| {
                let credits = (t.study.credits)(&folia_plans::plan::credits_in(&choice.credits_text, t.locale));
                let span = (v.plan_says)(&(t.study.fs_span)(choice.first, choice.last));
                let mut extra: Vec<String> = Vec::new();
                if choice.fues {
                    extra.push(v.fues_rule.to_string());
                }
                if choice.lists.len() > 1 {
                    extra.push((v.one_of)(choice.lists.len()));
                }
                if let Some((together, others)) = &choice.together {
                    let names = match others.split_last() {
                        Some((last, [])) => last.clone(),
                        Some((last, rest)) => format!("{} {} {last}", rest.join(", "), v.and),
                        None => String::new(),
                    };
                    if !names.is_empty() {
                        extra.push((v.together)(&names, &n(*together, t)));
                    }
                }
                let rule_extra = (!extra.is_empty()).then(|| extra.join(" "));
                let rule = std::iter::once((v.rule)(&folia_plans::plan::credits_in(&choice.credits_text, t.locale), &(t.study.fs_span)(choice.first, choice.last))).chain(rule_extra.clone()).collect::<Vec<_>>().join(" ");
                let taken = choice
                    .taken
                    .iter()
                    .map(|taken| {
                        let stand = if taken.passed { (v.state_passed)(&taken.semester.short(t.locale)) } else { (v.state_planned)(&taken.semester.short(t.locale)) };
                        let credits = taken.credits.map(|credits| format!(" · {}", (t.study.credits)(&n(credits, t)))).unwrap_or_default();
                        (taken.name.clone(), format!("{stand}{credits}"))
                    })
                    .collect();
                let taken_line = match choice.done {
                    Some(done) => Some((v.done_in)(&done.label(t.locale))),
                    None if choice.taken.is_empty() => None,
                    None => Some((v.taken)(&n(choice.taken_credits(), t), &folia_plans::plan::credits_in(&choice.credits_text, t.locale))),
                };
                let scope = ProgramScope { program_slug: ready.program.slug.clone(), ..Default::default() };
                let catalog = if choice.fues {
                    Some((CatalogUrl { query: CatalogQuery { program: Some(ProgramScope { relation: ProgramRelation::Fues, ..scope }), ..Default::default() }, ..Default::default() }.path(), v.fues_catalog))
                } else if choice.lists.is_empty() {
                    choice.lines.first().and_then(|at| view.lines.get(*at)).and_then(|line| super::focus::find_href(ctx, &line.caption, line.ord, t)).map(|href| (href, v.search_catalog))
                } else {
                    None
                };
                Item {
                    index,
                    name: choice.name.clone(),
                    area: choice.area,
                    area_name: ready.area_name(choice.area, t),
                    tone: ready.tone(choice.area),
                    credits,
                    span,
                    rule,
                    rule_extra,
                    fues: choice.fues,
                    lists: choice.lists.iter().map(|list| (list.name.clone(), list.modules.clone())).collect(),
                    taken,
                    taken_line,
                    complete: choice.complete(),
                    due: due_of(choice),
                    catalog,
                }
            })
            .collect();

        // The areas with choices, those with something open first.
        let mut areas: Vec<Area> = Vec::new();
        for item in &items {
            match areas.iter_mut().find(|area| area.area == item.area) {
                Some(area) => area.choices.push(item.index),
                None => {
                    let figure = item
                        .area
                        .and_then(|at| study.progress.get(at))
                        .map(|progress| format!("{}/{}", n(progress.passed + progress.planned + progress.over, t), n(progress.area.required, t)))
                        .unwrap_or_default();
                    areas.push(Area { area: item.area, name: item.area_name.clone(), tone: item.tone, figure, complete: false, choices: vec![item.index] });
                }
            }
        }
        for area in &mut areas {
            area.complete = area.choices.iter().all(|at| items.get(*at).is_some_and(|item| item.complete));
        }
        areas.sort_by_key(|area| area.complete);

        let doc = input.doc;
        let has = view
            .choices
            .iter()
            .flat_map(|choice| choice.modules())
            .map(|id| {
                let has = match doc.passed_in(id) {
                    Some(_) => Has::Passed,
                    None => doc.planned_in(id).into_iter().find(|s| *s >= now).map_or(Has::No, Has::Planned),
                };
                (id.to_string(), has)
            })
            .collect();
        let first = areas.iter().flat_map(|area| area.choices.iter()).copied().find(|at| items.get(*at).is_some_and(|item| !item.complete));
        let lead = if items.is_empty() {
            v.choices_none.to_string()
        } else if view.to_choose > 0.0 {
            (v.choices_lead)(&n(view.to_choose, t))
        } else {
            v.choices_done_lead.to_string()
        };
        Pane { lead, areas, items, has, first, now }
    }
}

/// The semester the list shows what is offered in, or all of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    In(SemesterKey),
    All,
}

/// A module of a list as the view draws it.
#[derive(Clone, Debug, PartialEq)]
struct Module {
    id: String,
    title: String,
    credits: Option<String>,
    turnus: String,
    /// Offered in the semester shown.
    fits: bool,
    /// Its turnus names one half of the year: what fits, fits for sure.
    seasonal: bool,
    phase_out: bool,
    has: Has,
    /// Where „In SoSe 27" plans it.
    into: SemesterKey,
}

/// The modules of a list for `target`: what the student has first, then what the semester offers,
/// then the rest; and how many of them the semester does not offer.
fn modules_of(ids: &[String], rows: &BTreeMap<String, CatalogRow>, has: &BTreeMap<String, Has>, target: Target, due: SemesterKey, t: &Texts) -> Vec<Module> {
    let v = &t.study_views;
    let mut out: Vec<Module> = ids
        .iter()
        .filter_map(|id| {
            let row = rows.get(id)?;
            let has = has.get(id).cloned().unwrap_or(Has::No);
            let gone = row.offer_status.known() == Some(OfferStatus::NotOffered);
            if gone && has == Has::No {
                return None;
            }
            let offer = Offer::of(row);
            let (fits, into) = match target {
                Target::In(s) => (offer.offered(s), s),
                Target::All => (true, offer.next(due)),
            };
            let turnus = match offer.season {
                Some(TurnusSeason::Winter) => t.format.winter_short.to_string(),
                Some(TurnusSeason::Summer) => t.format.summer_short.to_string(),
                _ if row.turnus_season.is_some() => format::turnus(row.turnus_season.as_ref(), None, t.locale),
                _ => v.turnus_unknown.to_string(),
            };
            Some(Module {
                id: id.clone(),
                title: row.title.clone(),
                credits: row.credits.map(|credits| folia_design::format::number(credits, t.locale)),
                turnus,
                fits,
                seasonal: offer.only().is_some(),
                phase_out: row.offer_status.known() == Some(OfferStatus::PhaseOut),
                has,
                into,
            })
        })
        .collect();
    out.sort_by_cached_key(|module| (module.has == Has::No, !module.fits, !module.seasonal, module.title.to_lowercase()));
    out
}

/// Plans the module `id` into `s`, with a note „Rückgängig" takes it back by.
fn plan(ctx: StudyCtx, id: String, title: String, s: SemesterKey, t: &'static Texts) {
    let note = (t.study_views.planned_note)(&title, &s.label(t.locale));
    ctx.change_noted(
        Some(note),
        move |doc, input| {
            study::add(doc, input, s, &[Pick::Module { id, from_plan: false }], now_secs());
        },
        || {},
    );
}

/// What a module of a list offers: its stand, or „In SoSe 27".
fn module_act(ctx: StudyCtx, module: &Module, t: &'static Texts) -> AnyView {
    let v = &t.study_views;
    match module.has {
        Has::Passed => view! { <span class="st-chip ok"><Icon name="check"/>{v.chip_passed}</span> }.into_any(),
        Has::Planned(s) => view! { <span class="st-chip plan">{(v.chip_planned)(&s.short(t.locale))}</span> }.into_any(),
        Has::No => {
            let (id, title, s) = (module.id.clone(), module.title.clone(), module.into);
            let label = (v.into)(&s.short(t.locale));
            let hint = label.clone();
            let aria = format!("{} · {}", module.title, (v.plan_into)(&s.label(t.locale)));
            view! {
                <button class="st-addb" type="button" aria-label=aria title=hint on:click=move |_| plan(ctx, id.clone(), title.clone(), s, t)>
                    <Icon name="plus"/><span class="st-addb-t">{label}</span>
                </button>
            }
            .into_any()
        }
    }
}

/// The view.
#[component]
pub(super) fn Choose(ctx: StudyCtx) -> impl IntoView {
    let t = i18n::t();
    view_back(ctx);
    let pane = Memo::new(move |_| ctx.with_input(|input, ready| Pane::of(input, ready, ctx, t)));
    // The catalog's rows of every module of the lists.
    let ids = Memo::new(move |_| {
        pane.with(|pane| {
            let mut ids: Vec<String> = pane.as_ref().map(|pane| pane.has.keys().cloned().collect()).unwrap_or_default();
            ids.sort();
            ids
        })
    });
    let rows = Memo::new(move |before: Option<&BTreeMap<String, CatalogRow>>| {
        let ids = ids.get();
        if ids.is_empty() {
            return BTreeMap::new();
        }
        let Some(now) = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&StudyModulesAsk { ids }))) else { return BTreeMap::new() };
        let before = before.cloned();
        match now {
            Ok(modules) => modules.rows.into_iter().map(|row| (row.id.clone(), row)).collect(),
            Err(_) => before.unwrap_or_default(),
        }
    });
    // The choice picked: the one asked for (a row's „Wählen"), else the first not complete.
    let picked = Memo::new(move |_| {
        let asked = ctx.choice.get();
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            asked.filter(|at| *at < pane.items.len()).or(pane.first).or((!pane.items.is_empty()).then_some(0))
        })
    });
    move || {
        let phone = ctx.phone.get();
        match pane.with(|pane| pane.as_ref().map(|pane| pane.items.is_empty())) {
            None => ().into_any(),
            Some(true) => view! {
                <section class="panel">
                    <EmptyState title=t.study_views.choices_none hint=t.study_views.choices_none_hint/>
                </section>
            }
            .into_any(),
            Some(false) if phone => view! { <ChoosePhone ctx pane rows picked/> }.into_any(),
            Some(false) => view! { <ChooseDesk ctx pane rows picked/> }.into_any(),
        }
    }
}

/// The semesters to plan into: the one the plan puts it into, the one after it, and all.
fn targets(due: SemesterKey) -> Vec<Target> {
    std::iter::once(Target::In(due)).chain(due.plus(1).map(Target::In)).chain(std::iter::once(Target::All)).collect()
}

/// „SoSe 2027 | WiSe 2027/28 | Alle".
fn target_switch(target: RwSignal<Target>, due: SemesterKey, small: bool, t: &'static Texts) -> impl IntoView {
    let v = &t.study_views;
    view! {
        <div class="st-views st-season-seg" class:sm=small role="group" aria-label=v.offered_in>
            {targets(due).into_iter().map(|option| {
                let label = match option {
                    Target::In(s) if small => s.short(t.locale),
                    Target::In(s) => s.label(t.locale),
                    Target::All => v.all.to_string(),
                };
                view! { <button type="button" aria-pressed=move || if target.get() == option { "true" } else { "false" } on:click=move |_| target.set(option)>{label}</button> }
            }).collect_view()}
        </div>
    }
}

/// The list of a choice shown: the one the student has most of, else the first.
fn list_at(item: &Item, has: &BTreeMap<String, Has>) -> usize {
    item.lists
        .iter()
        .enumerate()
        .max_by_key(|(at, (_, modules))| (modules.iter().filter(|id| has.get(*id).is_some_and(|has| *has != Has::No)).count(), std::cmp::Reverse(*at)))
        .map_or(0, |(at, _)| at)
}

/// On a desktop: the choices on the left, the one picked on the right.
#[component]
fn ChooseDesk(ctx: StudyCtx, pane: Memo<Option<Pane>>, rows: Memo<BTreeMap<String, CatalogRow>>, picked: Memo<Option<usize>>) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let item = Memo::new(move |_| {
        let at = picked.get()?;
        pane.with(|pane| pane.as_ref().and_then(|pane| pane.items.get(at).cloned()))
    });
    let target = RwSignal::new(Target::All);
    let list = RwSignal::new(0usize);
    // Another choice: its semester and its list.
    Effect::new(move |before: Option<Option<usize>>| {
        let now = item.with(|item| item.as_ref().map(|item| item.index));
        if before != Some(now) {
            if let Some((due, at)) = item.with_untracked(|item| item.as_ref().map(|item| (item.due, pane.with_untracked(|pane| pane.as_ref().map_or(0, |pane| list_at(item, &pane.has)))))) {
                target.set(Target::In(due));
                list.set(at);
            }
        }
        now
    });
    let left = move || {
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            let now = picked.get();
            Some(
                pane.areas
                    .iter()
                    .map(|area| {
                        view! {
                            <p class="st-sl-h" style=format!("--c: {}", area.tone)><span class="st-dot"></span><span>{area.name.clone()}</span><small class:ok=area.complete>{area.figure.clone()}</small></p>
                            {area.choices.iter().filter_map(|at| pane.items.get(*at)).map(|item| {
                                let at = item.index;
                                let sub = match (&item.taken_line, item.complete) {
                                    (Some(line), true) => view! { <small class="ok"><Icon name="check"/>{line.clone()}</small> }.into_any(),
                                    (Some(line), false) => view! { <small>{item.credits.clone()}" · "{line.clone()}</small> }.into_any(),
                                    (None, _) => view! { <small>{item.credits.clone()}" · "{item.span.clone()}</small> }.into_any(),
                                };
                                view! {
                                    <button class="st-sl" type="button" style=format!("--c: {}", item.tone) title=item.name.clone() aria-current=(now == Some(at)).then_some("true") on:click=move |_| ctx.choice.set(Some(at))>
                                        <b>{item.name.clone()}</b>{sub}
                                    </button>
                                }
                            }).collect_view()}
                        }
                    })
                    .collect_view(),
            )
        })
    };
    let right = move || {
        let item = item.get()?;
        let has = pane.with(|pane| pane.as_ref().map(|pane| pane.has.clone()))?;
        let shown_list = list.get().min(item.lists.len().saturating_sub(1));
        let lists = (item.lists.len() > 1).then(|| {
            view! {
                <div class="st-views st-lists" role="group" aria-label=v.lists>
                    {item.lists.iter().enumerate().map(|(at, (name, modules))| view! {
                        <button type="button" aria-pressed=move || if list.get() == at { "true" } else { "false" } on:click=move |_| list.set(at)>
                            {name.clone()}<small>{modules.len()}</small>
                        </button>
                    }).collect_view()}
                </div>
            }
        });
        let ids = item.lists.get(shown_list).map(|(_, modules)| modules.clone()).unwrap_or_default();
        let table = (!item.lists.is_empty()).then(|| {
            let target_now = target.get();
            let modules = rows.with(|rows| modules_of(&ids, rows, &has, target_now, item.due, t));
            let (fit, rest): (Vec<Module>, Vec<Module>) = modules.into_iter().partition(|module| module.fits || module.has != Has::No);
            let season = match target_now {
                Target::In(s) => if s.winter { t.format.winter_short } else { t.format.summer_short },
                Target::All => "",
            };
            let hidden = rest.len();
            view! {
                <table class="st-wtable">
                    <thead><tr><th scope="col" class="id">{v.col_id}</th><th scope="col">{v.col_module}</th><th scope="col" class="r">{v.col_credits}</th><th scope="col">{v.col_turnus}</th><th scope="col" class="r"><span class="visually-hidden">{v.plan_it}</span></th></tr></thead>
                    <tbody>
                        {fit.into_iter().map(|module| {
                            let href = super::module_href(ctx, &module.id, t);
                            view! {
                                <tr class:taken=module.has != Has::No>
                                    <td class="id mono">{module.id.clone()}</td>
                                    <td class="nm"><a href=href data-noscroll="">{module.title.clone()}</a>{module.phase_out.then(|| view! { <small>{OfferStatus::PhaseOut.label(t.locale)}</small> })}</td>
                                    <td class="r num">{module.credits.clone()}</td>
                                    <td><span class="st-turnus" class:match=module.fits && module.seasonal && target_now != Target::All>{module.turnus.clone()}</span></td>
                                    <td class="r">{module_act(ctx, &module, t)}</td>
                                </tr>
                            }
                        }).collect_view()}
                        {(hidden > 0).then(|| view! {
                            <tr class="st-more-row">
                                <td></td>
                                <td colspan="4">{(v.more_unoffered)(hidden, season)}" · "<button class="st-link" type="button" on:click=move |_| target.set(Target::All)>{v.show_all}</button></td>
                            </tr>
                        })}
                    </tbody>
                </table>
            }
        });
        Some(view! {
            <div class="st-pane-h" style=format!("--c: {}", item.tone)>
                <div class="st-pane-title">
                    <h3>{item.name.clone()}</h3>
                    <span class="st-chip tone"><span class="st-dot"></span>{format!("{} · {}", item.area_name, item.credits)}</span>
                </div>
                {(!item.lists.is_empty()).then(|| view! { <div class="st-season"><span>{v.offered_in}</span>{target_switch(target, item.due, false, t)}</div> })}
                <p class="st-rule">{item.rule.clone()}</p>
                {lists}
            </div>
            {(!item.taken.is_empty() || item.taken_line.is_some()).then(|| view! {
                <div class="st-done-box" class:complete=item.complete>
                    <Icon name=if item.complete { "circle-check-big" } else { "list-checks" }/>
                    <div>
                        <b>{item.taken_line.clone()}</b>
                        <ul>{item.taken.iter().map(|(name, stand)| view! { <li>{name.clone()}<small>{stand.clone()}</small></li> }).collect_view()}</ul>
                    </div>
                </div>
            })}
            {item.catalog.clone().map(|(href, label)| view! {
                <div class="st-pane-note">
                    {item.lists.is_empty().then(|| (!item.fues).then(|| view! { <p class="hint">{v.no_list}</p> }))}
                    <a class="action" href=t.path(&href)><Icon name="search"/><span>{label}</span><Icon name="chevron-right"/></a>
                </div>
            })}
            {table}
        })
    };
    view! {
        <section class="panel st-wp" aria-labelledby="st-wp-title">
            <header class="st-view-head">
                <div>
                    <h2 id="st-wp-title">{v.choices}</h2>
                    <p>{move || pane.with(|pane| pane.as_ref().map(|pane| pane.lead.clone()))}</p>
                </div>
            </header>
            <div class="st-split">
                <nav class="st-split-l" aria-label=v.areas>{left}</nav>
                <div class="st-split-r">{right}</div>
            </div>
        </section>
    }
}

/// On a phone: the areas as chips, the semester, the area's choices with their modules.
#[component]
fn ChoosePhone(ctx: StudyCtx, pane: Memo<Option<Pane>>, rows: Memo<BTreeMap<String, CatalogRow>>, picked: Memo<Option<usize>>) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    // The area shown: the picked choice's.
    let area = RwSignal::new(picked.get_untracked().and_then(|at| pane.with_untracked(|pane| pane.as_ref().and_then(|pane| pane.items.get(at).map(|item| item.area)))).flatten());
    let shown = Memo::new(move |_| {
        let wanted = area.get();
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            pane.areas.iter().find(|candidate| candidate.area == wanted).or(pane.areas.first()).cloned()
        })
    });
    // The semester: the first one an open choice of the area is due in.
    let due = Memo::new(move |_| {
        let area = shown.get()?;
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            let items: Vec<&Item> = area.choices.iter().filter_map(|at| pane.items.get(*at)).collect();
            items.iter().filter(|item| !item.complete).map(|item| item.due).min().or(items.iter().map(|item| item.due).min())
        })
    });
    let target = RwSignal::new(Target::All);
    Effect::new(move |_| {
        if let Some(due) = due.get() {
            target.set(Target::In(due));
        }
    });
    let chips = move || {
        pane.with(|pane| {
            let pane = pane.as_ref()?;
            let now = shown.with(|shown| shown.as_ref().map(|shown| shown.area));
            Some(
                pane.areas
                    .iter()
                    .map(|candidate| {
                        let at = candidate.area;
                        view! {
                            <button class="st-fchip" type="button" style=format!("--c: {}", candidate.tone) aria-pressed=if now == Some(at) { "true" } else { "false" } on:click=move |_| area.set(at)>
                                <span class="st-dot"></span>{candidate.name.clone()}
                                <small>{if candidate.complete { "✓".to_string() } else { candidate.figure.clone() }}</small>
                            </button>
                        }
                    })
                    .collect_view(),
            )
        })
    };
    let groups = move || {
        let area = shown.get()?;
        let (items, has) = pane.with(|pane| pane.as_ref().map(|pane| (area.choices.iter().filter_map(|at| pane.items.get(*at).cloned()).collect::<Vec<_>>(), pane.has.clone())))?;
        let target_now = target.get();
        Some(items.into_iter().map(|item| view! { <ChoiceGroup ctx item has=has.clone() rows target=target_now/> }).collect_view())
    };
    view! {
        <div class="st-phone-page st-wp-phone">
            <header class="st-vhead">
                <h1>{v.choices}</h1>
                <p>{move || pane.with(|pane| pane.as_ref().map(|pane| pane.lead.clone()))}</p>
            </header>
            <div class="st-chiprow" role="group" aria-label=v.areas>{chips}</div>
            {move || due.get().map(|due| view! { <div class="st-season"><span>{v.offered_in}</span>{target_switch(target, due, true, t)}</div> })}
            {groups}
        </div>
    }
}

/// How many modules a phone's group shows before „alle zeigen".
const SHOWN: usize = 6;

/// A choice on a phone: its head, what is taken, its lists, its modules as cards.
#[component]
fn ChoiceGroup(ctx: StudyCtx, item: Item, has: BTreeMap<String, Has>, rows: Memo<BTreeMap<String, CatalogRow>>, target: Target) -> impl IntoView {
    let t = i18n::t();
    let v = &t.study_views;
    let list = RwSignal::new(list_at(&item, &has));
    let all = RwSignal::new(false);
    let count: usize = item.lists.iter().map(|(_, modules)| modules.len()).sum();
    let head_sub = if item.lists.is_empty() { item.span.clone() } else { format!("{} · {}", item.span, (v.modules)(count)) };
    let lists = (item.lists.len() > 1).then(|| {
        view! {
            <div class="st-chiprow small" role="group" aria-label=v.lists>
                {item.lists.iter().enumerate().map(|(at, (name, modules))| view! {
                    <button class="st-fchip" type="button" aria-pressed=move || if list.get() == at { "true" } else { "false" } on:click=move |_| { list.set(at); all.set(false); }>
                        {name.clone()}<small>{modules.len()}</small>
                    </button>
                }).collect_view()}
            </div>
        }
    });
    let item_lists = item.lists.clone();
    let due = item.due;
    let cards = move || {
        let ids = item_lists.get(list.get().min(item_lists.len().saturating_sub(1))).map(|(_, modules)| modules.clone()).unwrap_or_default();
        if ids.is_empty() {
            return None;
        }
        let modules = rows.with(|rows| modules_of(&ids, rows, &has, target, due, t));
        let (fit, rest): (Vec<Module>, Vec<Module>) = modules.into_iter().partition(|module| module.fits || module.has != Has::No);
        let total = fit.len();
        let open = all.get();
        let shown: Vec<Module> = if open { fit } else { fit.into_iter().take(SHOWN).collect() };
        let more = (!open && total > SHOWN).then(|| view! { <button class="st-more-btn" type="button" on:click=move |_| all.set(true)>{format!("{} ({total})", v.show_all)}<Icon name="chevron-down"/></button> });
        let season = match target {
            Target::In(s) => if s.winter { t.format.winter_short } else { t.format.summer_short },
            Target::All => "",
        };
        let hidden = (!rest.is_empty()).then(|| view! { <p class="hint st-hidden-n">{(v.more_unoffered)(rest.len(), season)}</p> });
        Some(view! {
            <ul class="st-mcards">
                {shown.into_iter().map(|module| {
                    let href = super::module_href(ctx, &module.id, t);
                    view! {
                        <li class="st-mcard" class:taken=module.has != Has::No>
                            <div class="st-mcard-t">
                                <a href=href data-noscroll="">{module.title.clone()}</a>
                                <span class="st-meta"><span class="st-code mono">{module.id.clone()}</span><span class="st-turnus" class:match=module.fits && module.seasonal && target != Target::All>{module.turnus.clone()}</span></span>
                            </div>
                            <span class="st-lp">{module.credits.clone()}<small>{v.col_credits}</small></span>
                            <span class="st-mcard-act">{module_act(ctx, &module, t)}</span>
                        </li>
                    }
                }).collect_view()}
            </ul>
            {more}
            {hidden}
        })
    };
    view! {
        <section class="st-grp" style=format!("--c: {}", item.tone) aria-label=item.name.clone()>
            <div class="st-grp-h">
                <b>{item.name.clone()}<span class="st-chip">{item.credits.clone()}</span></b>
                <small>{head_sub}</small>
            </div>
            {item.rule_extra.clone().map(|rule| view! { <p class="st-rule"><Icon name="info"/><span>{rule}</span></p> })}
            {(!item.taken.is_empty() || item.taken_line.is_some()).then(|| view! {
                <div class="st-done-box" class:complete=item.complete>
                    <Icon name=if item.complete { "circle-check-big" } else { "list-checks" }/>
                    <div>
                        <b>{item.taken_line.clone()}</b>
                        <ul>{item.taken.iter().map(|(name, stand)| view! { <li>{name.clone()}<small>{stand.clone()}</small></li> }).collect_view()}</ul>
                    </div>
                </div>
            })}
            {item.catalog.clone().map(|(href, label)| view! {
                {(item.lists.is_empty() && !item.fues).then(|| view! { <p class="hint">{v.no_list}</p> })}
                <a class="action" href=t.path(&href)><Icon name="search"/><span>{label}</span><Icon name="chevron-right"/></a>
            })}
            {lists}
            {cards}
        </section>
    }
}
