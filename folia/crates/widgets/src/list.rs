//! A module as a row of a list (`Row`), the keys of a list (`ListKeys`), and whether the phone
//! layout is in use (`phone_layout`): the catalog's list, the marked modules, a program's page.


use folia_model::labels::TurnusSeason;
use folia_model::rows::{CatalogRow, MarkedText};
use leptos::prelude::*;

use folia_stores::bookmarks::{Bookmarks, MarkButton, MarkLook};
use folia_design::format;
use crate::i18n::{self};
use folia_design::nav;
use crate::finder::Finder;
use crate::swipe::RowSwipe;
use folia_design::ui::{Icon, KindBadge, OfferBadge};


/// A module as a row of a list: the catalog's, and the list of marked modules. The whole row is
/// a link; the mark at its end is a button next to the link, not inside it.
/// Whether this build is the browser app: only there does a row swipe and mark.
const APP: bool = cfg!(feature = "csr");

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
) -> impl IntoView {
    let t = i18n::t();
    let language = format::languages(row.teaches_german, row.teaches_english);
    let (turnus_icon, turnus_text) = match row.turnus_season.as_ref().and_then(|s| s.known()) {
        Some(TurnusSeason::Winter) => ("snowflake", t.catalog.row_winter.to_string()),
        Some(TurnusSeason::Summer) => ("sun", t.catalog.row_summer.to_string()),
        Some(TurnusSeason::Both) => ("repeat", t.catalog.row_every.to_string()),
        Some(TurnusSeason::Irregular) => ("shuffle", t.catalog.row_irregular.to_string()),
        None => ("minus", row.turnus_season.as_ref().map(|s| s.label(t.locale).to_string()).unwrap_or_else(|| t.catalog.row_unknown.to_string())),
    };
    let events = (t.catalog.events)(row.teaching_events);
    let has_events = row.teaching_events > 0;
    // In the list of a search: the letters it found, and a line under the title with what it was
    // found by where that is not the title one sees (folia/design/suche/konzept.html, „Die Zeile
    // zeigt den Grund").
    let marks = row.marks.clone().unwrap_or_default();
    let found_by = |label: &'static str, found: &MarkedText| view! { <small class="found-by"><i>{label}</i>" "{marked(&found.text, &found.spans)}</small> };
    let other = marks.other.as_ref().map(|other| {
        let english = row.title_en.as_deref() == Some(other.text.as_str());
        found_by(if english { t.catalog.row_title_en } else { t.catalog.row_title_de }, other)
    });
    let lecturer = marks.lecturer.as_ref().map(|name| found_by(if marks.teaching { t.catalog.row_teaches } else { t.catalog.row_responsible }, name));
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
    let swipe = (APP && swipe).then(|| RowSwipe::new(row.id.clone(), row.title.clone(), offer, finder.map(|finder| finder.hint), phone));
    let unmarked = move || unmarked.is_some_and(|unmarked| unmarked.get());
    let link = view! {
        <a class="row" href=href data-noscroll="" data-id=row.id.clone() aria-current=move || current.get().then_some("true")>
            <div class="t">
                <b>{marked(&row.title, &marks.title)}</b>
                <small>
                    <span class="mono">{marked(&row.id, &marks.id)}</span>
                    {marks.abbrev.as_ref().map(|abbrev| view! { <span class="flag neutral">{(t.catalog.row_abbrev)(abbrev)}</span> })}
                    {with_program.then(|| view! { <KindBadge kind=row.kind.clone() basis=row.kind_basis.clone() source=row.kind_source.clone() area=row.area.clone()/> })}
                    // Inside a program the study plan's semester stands at the row (the
                    // list is in plan order, without headings between the semesters).
                    {with_program.then(|| row.plan_semester.map(|n| view! { <span class="plan-sem">{(t.format.semesters)(&(t.format.semester_one)(n))}</span> }))}
                    <OfferBadge status=row.offer_status.clone()/>
                    {(row.is_fues && !with_program).then(|| view! { <span class="flag neutral">"FÜS"</span> })}
                    {(row.is_limited == Some(true)).then(|| view! { <span class="flag neutral">{t.catalog.limited_places}</span> })}
                    {fit_note}
                    <span class="narrow-only">{language.map(|l| format!("{l} · "))}{events.clone()}</span>
                </small>
                {other}
                {lecturer}
            </div>
            <span class="resp">{row.responsible.clone()}</span>
            <span class="exam">{row.exam_form.as_ref().map(|form| format::exam_short(form, t.locale))}</span>
            <span class="lp num">{row.credits.map(|value| format::number(value, t.locale))}<small>{t.common.credits_unit}</small></span>
            <span class="turnus" title=turnus_text.clone()><Icon name=turnus_icon/><span class="txt">{turnus_text.clone()}</span></span>
            <span class="lang" class:unknown=language.is_none()>{language.unwrap_or(t.catalog.row_unknown)}</span>
            <span class="events" class:none=!has_events>
                {has_events.then(|| view! { <Icon name="calendar-check-2"/> })}
                {if has_events { events } else { t.catalog.events_none.to_string() }}
            </span>
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
            <div class="row-wrap" class:shaded=shaded class:unmarked=unmarked>
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
/// and the list of marked modules.
#[component]
pub fn ListKeys() -> impl IntoView {
    let t = i18n::t();
    view! {
        <span class="keys" title=t.catalog.keys_title><kbd>"↑"</kbd><kbd>"↓"</kbd>" "{t.catalog.key_move}" "<kbd>"Enter"</kbd>" "{t.catalog.key_open}" "<kbd>"M"</kbd>" "{t.catalog.key_save}</span>
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
