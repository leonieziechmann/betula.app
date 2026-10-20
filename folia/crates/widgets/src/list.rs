//! A module as a row of a list (`Row`) and what it says of the module beside its title (`Facts`),
//! the keys of a list (`ListKeys`), and whether the phone layout is in use (`phone_layout`): the
//! catalog's list, the marked modules, a program's page, the semesters of „Mein Studium".


use folia_model::labels::TurnusSeason;
use folia_model::rows::{CatalogRow, MarkedText};
use leptos::prelude::*;

use folia_stores::bookmarks::{Bookmarks, MarkButton, MarkLook};
use folia_design::format;
use crate::i18n::{self, Locale};
use folia_design::nav;
use crate::finder::Finder;
use crate::swipe::RowSwipe;
use folia_design::ui::{Icon, KindBadge, OfferBadge};


/// Whether this build is the browser app: only there does a row swipe and mark.
const APP: bool = cfg!(feature = "csr");

/// What a row of a list says of a module besides its title, its number and its marks: under them
/// its language and its events where the list is narrow, and beside them its columns — who is
/// responsible, the exam, then (after the credits, which each list writes itself) the turnus, the
/// language, the events. The catalog's list and the Merkliste (`Row`), and the semesters of „Mein
/// Studium", which show a module as the catalog does (owner, 2026-10-19: „1 zu 1 der Style vom
/// Modulkatalog … so dass man einfach versteht, dass es sich um ein Modul handelt"). For a row
/// that is no module of the catalog (a row of a plan that names none) the columns stand empty, so
/// that the next ones keep their places.
#[derive(Clone, Debug, PartialEq)]
pub struct Facts {
    known: bool,
    /// „DE", „EN", „DE/EN"; what the column says without one.
    language: Option<&'static str>,
    unknown: &'static str,
    turnus: (&'static str, String),
    /// „2 Termine" under the title; what the column says, „noch keine" for none.
    events: String,
    events_cell: String,
    has_events: bool,
    responsible: Option<String>,
    exam: Option<String>,
}

impl Facts {
    /// The facts of `row` in the language of `locale`; `None` for a row that is no module.
    pub fn of(row: Option<&CatalogRow>, locale: Locale) -> Self {
        let t = i18n::texts(locale);
        let Some(row) = row else {
            return Facts {
                known: false,
                language: None,
                unknown: t.catalog.row_unknown,
                turnus: ("minus", String::new()),
                events: String::new(),
                events_cell: String::new(),
                has_events: false,
                responsible: None,
                exam: None,
            };
        };
        let turnus = match row.turnus_season.as_ref().and_then(|s| s.known()) {
            Some(TurnusSeason::Winter) => ("snowflake", t.catalog.row_winter.to_string()),
            Some(TurnusSeason::Summer) => ("sun", t.catalog.row_summer.to_string()),
            Some(TurnusSeason::Both) => ("repeat", t.catalog.row_every.to_string()),
            Some(TurnusSeason::Irregular) => ("shuffle", t.catalog.row_irregular.to_string()),
            None => ("minus", row.turnus_season.as_ref().map(|s| s.label(t.locale).to_string()).unwrap_or_else(|| t.catalog.row_unknown.to_string())),
        };
        let has_events = row.teaching_events > 0;
        let events = (t.catalog.events)(row.teaching_events);
        Facts {
            known: true,
            language: format::languages(row.teaches_german, row.teaches_english),
            unknown: t.catalog.row_unknown,
            turnus,
            events_cell: if has_events { events.clone() } else { t.catalog.events_none.to_string() },
            events,
            has_events,
            responsible: row.responsible.clone(),
            exam: row.exam_form.as_ref().map(|form| format::exam_short(form, t.locale)),
        }
    }

    /// Whether the module has teaching events in its newest semester.
    pub fn has_events(&self) -> bool {
        self.has_events
    }

    /// Under the title where the list is narrow: „DE · 2 Termine".
    pub fn narrow(&self) -> impl IntoView + use<> {
        let (language, events) = (self.language, self.events.clone());
        self.known.then(|| view! { <span class="narrow-only">{language.map(|l| format!("{l} · "))}{events}</span> })
    }

    /// The columns before the credits.
    pub fn lead(&self) -> impl IntoView + use<> {
        view! {
            <span class="resp">{self.responsible.clone()}</span>
            <span class="exam">{self.exam.clone()}</span>
        }
    }

    /// The columns after the credits.
    pub fn trail(&self) -> impl IntoView + use<> {
        if !self.known {
            return view! { <span class="turnus"></span><span class="lang"></span><span class="events none"></span> }.into_any();
        }
        let (icon, text) = self.turnus.clone();
        let title = text.clone();
        let has_events = self.has_events;
        view! {
            <span class="turnus" title=title><Icon name=icon/><span class="txt">{text}</span></span>
            <span class="lang" class:unknown=self.language.is_none()>{self.language.unwrap_or(self.unknown)}</span>
            <span class="events" class:none=!has_events>
                {has_events.then(|| view! { <Icon name="calendar-check-2"/> })}
                {self.events_cell.clone()}
            </span>
        }
        .into_any()
    }
}

/// What „Mein Studium" says of a module, where a row of the Merkliste shows it (owner, 2026-10-19:
/// „Ja mein stadium"): its mark as „Mein Studium" draws it (app.css `.st-glyph`), the word that mark
/// stands for, and the semester it is about.
#[derive(Clone, Debug, PartialEq)]
pub struct RowStatus {
    /// `planned`, `running`, `passed` or `failed`, the class of the mark.
    pub class: &'static str,
    /// „Geplant", „Läuft", „Bestanden", „Nicht bestanden": for screen readers and the tooltip.
    pub word: &'static str,
    /// „WiSe 26/27".
    pub semester: String,
}

/// What the visitor keeps of a module, where a row of the Merkliste shows it (oberflaeche.html, „Die
/// Zeile"): the collections it is in, outlined after what Betula says of it (at most two by name,
/// then „+1"; the one the list shows is left out), and the first line of its note, under the row
/// as wide as the row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowMine {
    pub collections: Vec<String>,
    /// The note, whole: the row shows its first line, its Markdown drawn.
    pub note: Option<String>,
}

/// How many of a module's collections a row names; the rest is „+1".
const NAMED: usize = 2;

/// A module as a row of a list: the catalog's, and the list of marked modules. The whole row is
/// a link; the mark at its end is a button next to the link, not inside it.
#[component]
pub fn Row(
    row: CatalogRow,
    /// Where the row leads: in the app its list with this module previewed next to it (on a phone
    /// a sheet over the list), on the server's page the module's own page.
    #[prop(into)] preview: Signal<String>,
    /// This module is the one previewed.
    #[prop(into)] current: Signal<bool>,
    phone: RwSignal<bool>,
    with_program: bool,
    /// In the list of marked modules a module whose mark was taken away stays where it is,
    /// dimmed, so that a slip is one click to undo.
    #[prop(optional)] dim_unmarked: bool,
    /// Every other row of the list is shaded. The list says which, from the row's place in the
    /// whole list: the virtual list renders only the rows on screen, so the stylesheet cannot count.
    #[prop(optional)] shaded: bool,
    /// On a phone the row is swiped to mark the module (to the left) and to plan it (to the right,
    /// `crate::swipe`): the lists of the catalog and of the marked modules in the browser app.
    #[prop(optional)] swipe: bool,
    /// What „Mein Studium" says of the module, after what the catalog says of it (the Merkliste).
    #[prop(optional, into)] status: Option<Signal<Option<RowStatus>>>,
    /// What the visitor keeps of it, after that (the Merkliste).
    #[prop(optional, into)] mine: Option<Signal<RowMine>>,
    /// Where the list selects (the Merkliste, as „Mein Studium"): whether the row is selected,
    /// hatched in the accent …
    #[prop(optional, into)] selected: Option<Signal<bool>>,
    /// … and what a finger held still on it does (`crate::swipe`): the list selects it.
    #[prop(optional)] on_hold: Option<Callback<String>>,
) -> impl IntoView {
    let t = i18n::t();
    let facts = Facts::of(Some(&row), t.locale);
    let has_events = facts.has_events();
    // In the list of a search: the letters it found, and a line under the row with what it was
    // found by where that is not the title one sees (folia/design/suche/konzept.html, „Die Zeile
    // zeigt den Grund"), as wide as the row (owner, 2026-10-19: „über die ganze Länge (oder
    // zumindest 75%) von der Zeile").
    let marks = row.marks.clone().unwrap_or_default();
    let found_by = |label: &'static str, found: &MarkedText| view! { <small class="found-by"><i>{label}</i>" "{marked(&found.text, &found.spans)}</small> };
    let other = marks.other.as_ref().map(|other| {
        let english = row.title_en.as_deref() == Some(other.text.as_str());
        found_by(if english { t.catalog.row_title_en } else { t.catalog.row_title_de }, other)
    });
    let lecturer = marks.lecturer.as_ref().map(|name| found_by(if marks.teaching { t.catalog.row_teaches } else { t.catalog.row_responsible }, name));
    // Its courses and choices (folia/design/wahlangebote/konzept.html): „14 Projekte zur Wahl",
    // „2 Standorte · Übung je Studiengang".
    let offered = crate::options::row_offer(row.courses, &row.choices, t);
    let finder = use_context::<Finder>();
    // The preview next to the list, on a phone a sheet over it (owner, 2026-10-06). `preview` is a
    // path of the app; the link carries the language's prefix.
    let href = move || t.path(&preview.get());
    let unmarked = dim_unmarked.then(|| {
        let (bookmarks, id) = (Bookmarks::expect(), row.id.clone());
        Memo::new(move |_| !bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(&id)))
    });
    // With „Passt in meinen Stundenplan" on: how the module fits, where it fits only in part
    // („Übung 1 von 3 frei") or could not be checked. One memo a row (R5); the catalog's list
    // alone has it.
    let fit_note = finder.map(|finder| {
        let id = row.id.clone();
        Memo::new(move |_| finder.view.with(|view| view.note_of(&id, !has_events)))
    });
    let fit_note = move || fit_note.and_then(|note| note.get()).map(|(text, quiet)| view! { <span class="flag fit-note" class:neutral=quiet>{text}</span> });
    let offer = folia_plans::study::Offer::of(&row);
    let swipe = (APP && swipe).then(|| RowSwipe::new(row.id.clone(), row.title.clone(), offer, finder.map(|finder| finder.hint), phone).on_hold(on_hold));
    let is_selected = move || selected.is_some_and(|selected| selected.get());
    let unmarked = move || unmarked.is_some_and(|unmarked| unmarked.get());
    let link = view! {
        <a class="row" href=href data-noscroll="" data-id=row.id.clone() aria-current=move || current.get().then_some("true")>
            <div class="t">
                <b>{marked(&row.title, &marks.title)}{selected.map(|_| move || is_selected().then(|| view! { <span class="visually-hidden">{format!(" ({})", t.catalog.row_selected)}</span> }))}</b>
                <small>
                    <span class="mono">{marked(&row.id, &marks.id)}</span>
                    // What the query names of it marked as in the title: one sign for what the
                    // search found (owner, 2026-10-19: „einheitliches zeichen für die suche", „nur
                    // so in den badges, dass da der Matching text makiert wird").
                    {marks.abbrev.as_ref().map(|abbrev| view! { <span class="flag neutral">{t.catalog.row_abbrev}" "{marked(&abbrev.text, &abbrev.spans)}</span> })}
                    {with_program.then(|| view! { <KindBadge kind=row.kind.clone() basis=row.kind_basis.clone() source=row.kind_source.clone() area=row.area.clone()/> })}
                    // Inside a program the study plan's semester stands at the row (the
                    // list is in plan order, without headings between the semesters).
                    {with_program.then(|| row.plan_semester.map(|n| view! { <span class="plan-sem">{(t.format.semesters)(&(t.format.semester_one)(n))}</span> }))}
                    <OfferBadge status=row.offer_status.clone()/>
                    {(row.is_fues && !with_program).then(|| view! { <span class="flag neutral">"FÜS"</span> })}
                    {(row.is_limited == Some(true)).then(|| view! { <span class="flag neutral">{t.catalog.limited_places}</span> })}
                    {fit_note}
                    {offered.map(|offered| view! { <span class="row-offer">{offered}</span> })}
                    {status.map(|status| move || status.get().map(|status| {
                        let title = format!("{} · {}", status.word, status.semester);
                        view! {
                            <span class="row-status" title=title>
                                <span class=format!("st-glyph {}", status.class) aria-hidden="true"><Icon name="check" class="g-passed"/><Icon name="x" class="g-failed"/></span>
                                <span class="visually-hidden">{status.word}" "</span>
                                {status.semester}
                            </span>
                        }
                    }))}
                    {mine.map(|mine| move || {
                        let names = mine.with(|mine| mine.collections.clone());
                        let rest: Vec<String> = names.iter().skip(NAMED).cloned().collect();
                        view! {
                            {names.into_iter().take(NAMED).map(|name| view! { <span class="row-tag">{name}</span> }).collect_view()}
                            {(!rest.is_empty()).then(|| view! { <span class="row-tag" title=rest.join(", ")>{format!("+{}", rest.len())}</span> })}
                        }
                    })}
                    {facts.narrow()}
                </small>
            </div>
            {facts.lead()}
            <span class="lp num">{row.credits.map(|value| format::number(value, t.locale))}<small>{t.common.credits_unit}</small></span>
            {facts.trail()}
            {other}
            {lecturer}
            {mine.map(|mine| move || mine.with(|mine| mine.note.clone()).map(|note| view! {
                <small class="row-note" title=folia_model::text::note_plain(&note)><Icon name="pencil"/><span class="md">{folia_design::ui::note_line(&note)}</span></small>
            }))}
        </a>
    };
    // Marking belongs to the browser app (R9, R15). The server leaves the button out and the
    // stylesheet keeps its place at the end of the row, so nothing moves at the takeover.
    let mark = APP.then(|| view! { <MarkButton id=row.id.clone() title=row.title.clone() look=MarkLook::Row/> });
    match swipe {
        // What lies under the card comes while the row is swiped, before the card in the order of
        // the page, so the card covers it.
        Some(swipe) => view! {
            <div
                class="row-wrap swipes"
                class:shaded=shaded
                class:unmarked=unmarked
                class:is-selected=is_selected
                node_ref=swipe.wrap()
                data-swipe=move || swipe.phase_attr()
                data-side=move || swipe.side_attr()
                data-armed=move || swipe.armed_attr()
                data-done=move || swipe.done_attr()
                style=move || swipe.style()
                on:pointerdown=move |ev| swipe.down(ev)
                on:pointermove=move |ev| swipe.moving(ev)
                on:pointerup=move |ev| swipe.up(ev)
                on:pointercancel=move |ev| swipe.cancel(ev)
                on:touchmove=move |ev| swipe.touch_move(ev)
                on:click:capture=move |ev| swipe.click(ev)
                on:dragstart=move |ev| swipe.drag_start(ev)
            >
                {swipe.ground_view()}
                {link}
                {mark}
            </div>
        }
        .into_any(),
        None => view! {
            <div class="row-wrap" class:shaded=shaded class:unmarked=unmarked class:is-selected=is_selected>
                {link}
                {mark}
            </div>
        }
        .into_any(),
    }
}

/// `text` with the letters a search found in it marked (`<mark>`): `spans` are byte ranges of it,
/// those that overlap or touch made one; one that is no range of the text is left out.
fn marked(text: &str, spans: &[(u32, u32)]) -> Vec<AnyView> {
    let mut ranges: Vec<(usize, usize)> = spans.iter().map(|&(a, b)| (a as usize, b as usize)).filter(|(a, b)| a < b).collect();
    ranges.sort_unstable();
    let mut joined: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (a, b) in ranges {
        match joined.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => joined.push((a, b)),
        }
    }
    let mut parts = Vec::with_capacity(joined.len() * 2 + 1);
    let mut at = 0;
    for (a, b) in joined {
        let (Some(before), Some(found)) = (text.get(at..a), text.get(a..b)) else { continue };
        if !before.is_empty() {
            parts.push(before.to_string().into_any());
        }
        parts.push(view! { <mark>{found.to_string()}</mark> }.into_any());
        at = b;
    }
    if let Some(rest) = text.get(at..).filter(|rest| !rest.is_empty()) {
        parts.push(rest.to_string().into_any());
    }
    parts
}

/// The keys of a list as its head names them (`enhance.js` does what they say): the catalog's,
/// and the list of marked modules, whose tooltip says how it selects (`select`).
#[component]
pub fn ListKeys(#[prop(optional)] select: bool) -> impl IntoView {
    let t = i18n::t();
    let title = if select { format!("{}. {}", t.catalog.keys_title, t.catalog.keys_select) } else { t.catalog.keys_title.to_string() };
    view! {
        <span class="keys" title=title><kbd>"↑"</kbd><kbd>"↓"</kbd>" "{t.catalog.key_move}" "<kbd>"Enter"</kbd>" "{t.catalog.key_open}" "<kbd>"M"</kbd>" "{t.catalog.key_save}" "<kbd>"S"</kbd>" "{t.catalog.key_collect}</span>
    }
}
/// Whether the phone layout is in use, kept up to date while the window changes its size. A list
/// needs it because its rows lead to the module's page there and to a preview elsewhere.
pub fn phone_layout() -> RwSignal<bool> {
    let phone = RwSignal::new(nav::is_phone());
    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::resize, move |_| {
            if phone.get_untracked() != nav::is_phone() {
                phone.set(nav::is_phone());
            }
        });
        on_cleanup(move || handle.remove());
    });
    phone
}
