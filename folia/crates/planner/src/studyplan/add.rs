//! „+ Modul" in every calendar (folia/design/kalender/konzepte.html, „+ Modul in einem Kalender";
//! owner, 2026-10-26: „Besuchmodul kann pro kallender gemacht werden also eher bei jedem kallender
//! kann man auch andere module hinyufügen. Aber beim Studien Plan zählt es dann als besucher
//! modul").
//!
//! A module taken into „Mein Studiengang" from here is only visited (`PlanDoc::visit`): in the
//! week, hatched, giving way where it meets the plan, and in „Mein Studium" on „nur besuchen",
//! without credits. The modules of the study come from „Mein Studium" alone (owner, 2026-10-26:
//! „Nein aus dem Kallender soll man nicht module in den Studienplan aufnehmen können. Es soll nur
//! möglich sein besuchsmodule zu planen. Damit es einen weg gibt, wie man sein studium plant und
//! nicht 200 verschiedene."). One taken into a calendar of one's own counts nowhere
//! (`PlanDoc::add_to_calendar`).
//!
//! The panel opens under the head of the calendar's part of the list beside the week (`AddPanel`):
//! what a module taken in is, the Merkliste and the catalog as two ways, each module with how it
//! sits in that calendar's week (`pages::fit_lines`: „passt", „überschneidet sich mit …"; one that
//! does not fit is taken in all the same, Betula says what is unusual), „Aufnehmen", and after it
//! a note with „Rückgängig". The Merkliste is as „Aus der Merkliste" had it until it planned into
//! the study (folia/design/merken/konzept.html, „Von überall": „nach Sammlung und Suche; zuerst,
//! was das Semester anbietet, mit „passt“ oder „überschneidet sich mit …“; danach, was es nicht
//! anbietet, mit dem Grund; „3 gemerkte übernehmen“ auch für eine Sammlung"): the collections as
//! chips, a search, what the semester offers, all of it at once, and folded what it does not, each
//! with why. The catalog is a search. A click answers at once, the plan follows after the next
//! frame (R21), and the module then leaves the list, being in the calendar.

use std::collections::{BTreeMap, BTreeSet};

use folia_calendar::select::Selection;
use folia_calendar::semester::SemesterKey;
use folia_locale::Locale;
use folia_model::rows::CatalogRow;
use folia_pages::ask::{CatalogAsk, FitLinesAsk};
use folia_pages::Sits;
use folia_routes::filter::CatalogQuery;
use folia_routes::url::CatalogUrl;
use folia_stores::collections::Collections;
use folia_stores::myprogram::MineResolved;
use leptos::prelude::*;

use folia_design::nav;
use folia_design::ui::Icon;

use super::head::{add_module_href, MarkedModule};
use super::{now_secs, PlanCtx};
use crate::i18n;

/// Which calendar „+ Modul" takes a module into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AddTo {
    /// „Mein Studiengang": a visit.
    Mine,
    /// A calendar of one's own, by its number.
    Calendar(u8),
}

/// How the note of a module taken in begins (`PlanCtx::undo`, which the import, „Plan leeren" and
/// the list's × share): a mark of its own, never shown.
const ADDED: &str = "Aufgenommen: ";

/// The most modules the catalog's search lists here; the whole catalog has the rest.
const FOUND: usize = 12;

/// The two ways to a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    Marked,
    Catalog,
}

/// What a calendar holds of the semester shown, for the panel: its name, the semester, its
/// modules, and how a module is held against its week (`pages::fit_lines`).
#[derive(Clone, Debug, PartialEq)]
struct Target {
    name: String,
    key: SemesterKey,
    held: Vec<String>,
    program: Option<String>,
    selection: Selection,
}

/// A module the panel offers: its number, its title, why the semester does not offer it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Offered {
    id: String,
    title: String,
    unoffered: Option<String>,
}

/// What the panel shows, worked out in one memo from what it reads (the calendar, the marked
/// modules, the way chosen, what the catalog's search found), so that no closure reads a memo
/// with what it is derived from (R16): the calendar's name, the marked modules it does not hold,
/// the way shown (the Merkliste while it offers anything, until one is chosen), what the search
/// found that it does not hold (`None` before two letters), and how they would sit in its week.
#[derive(Clone, Debug, PartialEq)]
struct Panel {
    name: String,
    offered: Vec<Offered>,
    way: Way,
    found: Option<Vec<Offered>>,
    ask: Option<FitLinesAsk>,
}

impl Panel {
    fn of(target: Option<Target>, marked: &[MarkedModule], to: AddTo, way: Option<Way>, rows: Option<&[CatalogRow]>, locale: Locale) -> Panel {
        let held = target.as_ref().map_or(&[][..], |target| target.held.as_slice());
        let offered = marked_for(marked, to, held);
        let way = way.unwrap_or(if offered.is_empty() { Way::Catalog } else { Way::Marked });
        let found = rows.map(|rows| found_for(rows, held));
        let ids: Vec<String> = match way {
            Way::Marked => offered.iter().filter(|module| module.unoffered.is_none()).map(|module| module.id.clone()).collect(),
            Way::Catalog => found.iter().flatten().map(|module| module.id.clone()).collect(),
        };
        let ask = target.as_ref().filter(|_| !ids.is_empty()).map(|target| FitLinesAsk {
            key: target.key,
            others: target.held.clone(),
            program: target.program.clone(),
            ids,
            selection: target.selection.clone(),
            locale,
        });
        Panel { name: target.map(|target| target.name).unwrap_or_default(), offered, way, found, ask }
    }
}

/// The marked modules a calendar does not hold yet: of „Mein Studiengang" what its week holds
/// neither planned nor visited, of one of one's own what it holds not.
fn marked_for(marked: &[MarkedModule], to: AddTo, held: &[String]) -> Vec<Offered> {
    marked
        .iter()
        .filter(|module| match to {
            AddTo::Mine => !module.in_week,
            AddTo::Calendar(_) => !held.contains(&module.id),
        })
        .map(|module| Offered { id: module.id.clone(), title: module.title.clone(), unoffered: module.unoffered.clone() })
        .collect()
}

/// What the catalog's search finds that the calendar does not hold, the first `FOUND`.
fn found_for(rows: &[CatalogRow], held: &[String]) -> Vec<Offered> {
    rows.iter().filter(|row| !held.contains(&row.id)).take(FOUND).map(|row| Offered { id: row.id.clone(), title: row.title.clone(), unoffered: None }).collect()
}

/// „+ Modul" of the calendar `to`, under the head of its part of the list (`modules.rs`,
/// `calendars.rs`). Closed by its ×, by Esc, and by „+ Modul" again.
#[component]
pub(super) fn AddPanel(ctx: PlanCtx, to: AddTo, marked: Memo<Vec<MarkedModule>>) -> impl IntoView {
    let t = i18n::t();
    let a = &t.studyplan_add;
    let resolved = MineResolved::expect();
    // The calendar and what it holds: „Mein Studiengang"'s week is the plan's (`wanted`, its
    // semester too), held against every Termin of it, visits too (a visit gives way only to what
    // counts); a calendar of one's own as it stands (`calendars.rs`). `wanted` and `selection` are
    // siblings (R16).
    let target = Memo::new(move |_| match to {
        AddTo::Mine => {
            let (key, held, program) = ctx.wanted.get();
            let selection = ctx.selection.with(|(_, selection)| Selection { visits: BTreeSet::new(), ..selection.clone() });
            Some(Target { name: t.studyplan_calendars.mine.to_string(), key, held, program, selection })
        }
        AddTo::Calendar(id) => ctx.calendars.with(|calendars| {
            calendars.iter().find(|calendar| calendar.id == id).map(|calendar| Target {
                name: calendar.name.clone(),
                key: calendar.key,
                held: calendar.modules.clone(),
                program: calendar.program.clone(),
                selection: calendar.selection.clone(),
            })
        }),
    });
    // The catalog's search, from two letters: asked whichever way is shown (what was typed stays
    // when the Merkliste is looked at), kept while the next answer is on its way.
    let search = RwSignal::new(String::new());
    let found_rows = Memo::new(move |before: Option<&Option<Vec<CatalogRow>>>| {
        let text = search.get();
        let text = text.trim();
        if text.chars().count() < 2 {
            return None;
        }
        let ask = CatalogAsk { url: CatalogUrl { query: CatalogQuery { text: text.to_string(), ..Default::default() }, ..Default::default() }, locale: t.locale };
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&ask)))?;
        folia_pages::ask::unless_pending(now, before, |answer| answer.ok().map(|data| data.page.rows))
    });
    let way = RwSignal::new(None::<Way>);
    let panel = Memo::new(move |_| {
        let chosen = way.get();
        let target = target.get();
        marked.with(|marked| found_rows.with(|rows| Panel::of(target, marked, to, chosen, rows.as_deref(), t.locale)))
    });
    let shown_way = Memo::new(move |_| panel.with(|panel| panel.way));
    let listed = Memo::new(move |_| panel.with(|panel| panel.found.clone().unwrap_or_default()));

    // The Merkliste by collection and search: what the visitor keeps of the modules, the page's
    // alone, never an address's or a request's (R20).
    let collections = Collections::expect();
    let shelf = Memo::new(move |_| collections.map(|collections| collections.with(Clone::clone)).unwrap_or_default());
    let collection = RwSignal::new(None::<u32>);
    let marked_search = RwSignal::new(String::new());
    let unoffered_open = RwSignal::new(false);
    // A collection that is gone, or holds none of these any more, shows all of them again.
    Effect::new(move |_| {
        if let Some(chosen) = collection.get() {
            let held = shelf.with(|shelf| panel.with(|panel| panel.offered.iter().any(|module| shelf.holds(chosen, &module.id))));
            if !held {
                collection.set(None);
            }
        }
    });
    // What the collection and the search leave of the marked modules, and whether they leave
    // none of some.
    let shown_marked = Memo::new(move |_| {
        let (chosen, text) = (collection.get(), marked_search.get());
        shelf.with(|shelf| {
            panel.with(|panel| {
                let shown: Vec<Offered> = panel
                    .offered
                    .iter()
                    .filter(|module| chosen.is_none_or(|chosen| shelf.holds(chosen, &module.id)))
                    .filter(|module| text.trim().is_empty() || folia_search::matches(&format!("{} {} {}", module.title, module.id, shelf.kept_text(&module.id).unwrap_or_default()), &text))
                    .cloned()
                    .collect();
                let none_left = !panel.offered.is_empty() && shown.is_empty();
                (shown, none_left)
            })
        })
    });
    let marked_here = Memo::new(move |_| shown_marked.with(|(shown, _)| shown.iter().filter(|module| module.unoffered.is_none()).cloned().collect::<Vec<_>>()));
    let marked_not = Memo::new(move |_| shown_marked.with(|(shown, _)| shown.iter().filter(|module| module.unoffered.is_some()).cloned().collect::<Vec<_>>()));

    // How each module the way shown offers would sit in the calendar's week: one question for all
    // of them, kept while the next is on its way.
    let fits = Memo::new(move |before: Option<&BTreeMap<String, Option<Sits>>>| {
        let Some(ask) = panel.with(|panel| panel.ask.clone()) else { return BTreeMap::new() };
        let Some(now) = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&ask))) else { return BTreeMap::new() };
        folia_pages::ask::unless_pending(now, before, |answer| answer.map(|lines| lines.into_iter().collect()).unwrap_or_default())
    });

    // The modules whose „Aufnehmen" was clicked and whose write has not landed yet.
    let taking = RwSignal::new(BTreeSet::<String>::new());
    let take = move |modules: Vec<Offered>| {
        let Some(plan) = ctx.plan else { return };
        let modules: Vec<Offered> = modules.into_iter().filter(|module| !taking.with_untracked(|taking| taking.contains(&module.id))).collect();
        if modules.is_empty() {
            return;
        }
        taking.update(|taking| taking.extend(modules.iter().map(|module| module.id.clone())));
        let key = ctx.key.get_untracked();
        let name = panel.with_untracked(|panel| panel.name.clone());
        let undo = ctx.undo;
        nav::after_paint(move || {
            let (done, before) = plan.update(|doc| {
                let before = doc.clone();
                let at = now_secs();
                let done: Vec<&Offered> = modules
                    .iter()
                    .filter(|module| match to {
                        AddTo::Mine => doc.visit(key, &module.id, at),
                        AddTo::Calendar(id) => doc.add_to_calendar(id, key, &module.id),
                    })
                    .collect();
                let said = match done.as_slice() {
                    [] => None,
                    [one] => Some((a.added)(&one.title, &name)),
                    many => Some((a.all_added)(many.len(), &name)),
                };
                (said, before)
            });
            if let Some(said) = done {
                let _ = undo.try_set(Some((format!("{ADDED}{said}"), before)));
            }
            let _ = taking.try_update(|taking| {
                for module in &modules {
                    taking.remove(&module.id);
                }
            });
        });
    };
    let note = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().and_then(|(note, _)| note.strip_prefix(ADDED).map(str::to_string))));
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        if restoring.get_untracked() {
            return;
        }
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        restoring.set(true);
        let undo = ctx.undo;
        plan.update_after_paint(move |doc| {
            *doc = before;
            let _ = undo.try_set(None);
            let _ = restoring.try_set(false);
        });
    };
    let close = move || ctx.adding.set(None);
    // Opened from a calendar's menu the panel may lie out of view (on a phone under the week).
    nav::after_paint(|| {
        nav::reveal_selector(".sp-addp");
    });

    let semester = Memo::new(move |_| ctx.key.get().label(t.locale));
    let row = move |module: Offered| {
        let busy = Memo::new({
            let id = module.id.clone();
            move |_| taking.with(|taking| taking.contains(&id))
        });
        let sits = {
            let (id, unoffered) = (module.id.clone(), module.unoffered.clone());
            move || {
                if let Some(why) = unoffered.clone() {
                    return view! { <small class="sp-fit">{why}</small> }.into_any();
                }
                match fits.with(|fits| fits.get(&id).cloned()) {
                    Some(Some(Sits::Fits)) => view! { <small class="sp-fit ok"><Icon name="check"/>{t.studyplan_head.fits}</small> }.into_any(),
                    Some(Some(Sits::FitsWith(text))) => view! { <small class="sp-fit">{text}</small> }.into_any(),
                    Some(Some(Sits::Clashes(text))) => view! { <small class="sp-fit warn"><Icon name="triangle-alert"/><span>{text}</span></small> }.into_any(),
                    // Asked, and without a Termin to hold against the week.
                    Some(None) => view! { <small class="sp-fit">{(a.no_dates)(&semester.get())}</small> }.into_any(),
                    None => ().into_any(),
                }
            }
        };
        let label = (a.take_named)(&module.title);
        let (title, id) = (module.title.clone(), module.id.clone());
        // Its number beside its name: the catalog has modules of one name (two „Denkmalpflege").
        let number = id.clone();
        let on_take = move |_| take(vec![module.clone()]);
        view! {
            <li data-id=id>
                <span class="sp-marked-t"><b>{title}" "<small class="mono sp-addp-id">{number}</small></b>{sits}</span>
                <button class="mini" type="button" aria-label=label aria-busy=move || busy.get().then_some("true") on:click=on_take>
                    {move || if busy.get() { a.taken } else { a.take }}
                </button>
            </li>
        }
    };
    let tab = move |which: Way, words: Signal<String>| {
        view! {
            <button type="button" role="tab" aria-selected=move || if shown_way.get() == which { "true" } else { "false" } on:click=move |_| way.set(Some(which))>
                <span class="seg-label">{move || words.get()}</span>
            </button>
        }
    };
    let h = &t.studyplan_head;
    let chips = move || {
        let ids: Vec<String> = panel.with(|panel| panel.offered.iter().map(|module| module.id.clone()).collect());
        shelf.with(|shelf| {
            // The collections that hold any of these, each with how many.
            let counted: Vec<(u32, String, usize)> = shelf.collections().iter().map(|kept| (kept.id, kept.name.clone(), ids.iter().filter(|id| shelf.holds(kept.id, id)).count())).filter(|(.., count)| *count > 0).collect();
            (!counted.is_empty()).then(|| {
                let chip = move |which: Option<u32>, words: String, count: usize| view! {
                    <button type="button" aria-pressed=move || if collection.get() == which { "true" } else { "false" } on:click=move |_| collection.set(which)>{words}<span class="n">{count}</span></button>
                };
                view! {
                    <div class="st-filters sp-marked-chips" role="group" aria-label=h.collections>
                        {chip(None, h.all.to_string(), ids.len())}
                        {counted.into_iter().map(|(id, name, count)| chip(Some(id), name, count)).collect_view()}
                    </div>
                }
            })
        })
    };
    let body = move || match shown_way.get() {
        Way::Marked => view! {
            {chips}
            <label class="sp-marked-search">
                <Icon name="search"/>
                <input type="search" placeholder=h.search_marked aria-label=h.search_marked prop:value=move || marked_search.get() on:input=move |ev| marked_search.set(event_target_value(&ev))/>
            </label>
            {move || panel.with(|panel| panel.offered.is_empty()).then(|| view! { <p class="sp-marked-none">{a.nothing_marked}</p> })}
            <ul class="sp-addp-list"><For each=move || marked_here.get() key=|module| module.id.clone() children=row/></ul>
            // All of them at once, the collection's or what the search found too.
            {move || {
                let all = marked_here.get();
                (all.len() > 1).then(|| {
                    let count = all.len();
                    let busy = Memo::new(move |_| taking.with(|taking| !taking.is_empty()));
                    view! { <button class="mini sp-take" type="button" aria-busy=move || busy.get().then_some("true") on:click=move |_| take(all.clone())>{(a.take_all)(count)}</button> }
                })
            }}
            {move || {
                let count = marked_not.with(Vec::len);
                (count > 0).then(|| view! {
                    <button class="sp-marked-fold" type="button" aria-expanded=move || if unoffered_open.get() { "true" } else { "false" } on:click=move |_| unoffered_open.update(|open| *open = !*open)>
                        <Icon name="chevron-right"/>{(h.unoffered_in)(&semester.get(), count)}
                    </button>
                    {move || unoffered_open.get().then(|| view! {
                        <ul class="sp-addp-list"><For each=move || marked_not.get() key=|module| module.id.clone() children=row/></ul>
                    })}
                })
            }}
            {move || shown_marked.with(|(_, none_left)| *none_left).then(|| view! { <p class="sp-marked-none">{h.nothing_found}</p> })}
        }
        .into_any(),
        Way::Catalog => view! {
            <label class="sp-marked-search">
                <Icon name="search"/>
                <input type="search" placeholder=a.search aria-label=a.search prop:value=move || search.get() on:input=move |ev| search.set(event_target_value(&ev))/>
            </label>
            {move || match panel.with(|panel| panel.found.as_ref().map(Vec::is_empty)) {
                None => view! { <p class="sp-marked-none">{a.search_hint}</p> }.into_any(),
                Some(true) => view! { <p class="sp-marked-none">{a.nothing_found}</p> }.into_any(),
                Some(false) => ().into_any(),
            }}
            <ul class="sp-addp-list"><For each=move || listed.get() key=|module| module.id.clone() children=row/></ul>
        }
        .into_any(),
    };
    let name = move || panel.with(|panel| panel.name.clone());
    let says = move || match to {
        AddTo::Mine => a.mine_says.to_string(),
        AddTo::Calendar(_) => (a.calendar_says)(&name()),
    };
    let marked_words = Signal::derive(move || (a.marked)(panel.with(|panel| panel.offered.len())));
    let catalog_words = Signal::derive(move || a.catalog.to_string());
    let catalog = move || t.path(&add_module_href(resolved, ctx.key.get()));
    view! {
        <section
            class="sp-addp"
            aria-label=move || (a.title)(&name())
            on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                if ev.key() == "Escape" {
                    ev.stop_propagation();
                    close();
                }
            }
        >
            <div class="sp-addp-head">
                <b>{move || (a.title)(&name())}</b>
                <button class="icon-btn" type="button" aria-label=a.close title=a.close on:click=move |_| close()><Icon name="x"/></button>
            </div>
            <p class="sp-addp-says">{says}</p>
            {move || note.get().map(|note| view! {
                <p class="action note-action sp-marked-note">
                    <Icon name="check"/>
                    <span>{note}</span>
                    <button class="mini hit" type="button" aria-busy=move || restoring.get().then_some("true") on:click=restore>{t.common.undo}</button>
                </p>
            })}
            <div class="seg sp-addp-ways" role="tablist" aria-label=a.ways>
                {tab(Way::Marked, marked_words)}
                {tab(Way::Catalog, catalog_words)}
            </div>
            {body}
            // The whole catalog, with what fits the week: of „Mein Studiengang" alone, whose week
            // the catalog's finder knows.
            {(to == AddTo::Mine).then(|| view! {
                <a class="sp-way" href=catalog><span>{a.to_catalog}</span><Icon name="chevron-right"/></a>
            })}
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(id: &str, in_week: bool, unoffered: Option<&str>) -> MarkedModule {
        MarkedModule { id: id.into(), title: format!("Modul {id}"), unoffered: unoffered.map(str::to_string), in_week }
    }

    #[test]
    fn the_merkliste_offers_what_the_calendar_does_not_hold() {
        let list = [marked("13676", false, None), marked("12104", true, None), marked("41103", false, Some("nur SoSe"))];
        // „Mein Studiengang": what its week holds neither planned nor visited, the one the semester
        // does not offer too, with why.
        let mine: Vec<(String, Option<String>)> = marked_for(&list, AddTo::Mine, &[]).into_iter().map(|module| (module.id, module.unoffered)).collect();
        assert_eq!(mine, [("13676".to_string(), None), ("41103".to_string(), Some("nur SoSe".to_string()))]);
        // A calendar of one's own: what it holds not, whatever the week holds.
        let lena: Vec<String> = marked_for(&list, AddTo::Calendar(1), &["13676".to_string()]).into_iter().map(|module| module.id).collect();
        assert_eq!(lena, ["12104", "41103"]);
    }
}
