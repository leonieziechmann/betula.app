//! A module as a row of a list (`Row`), the keys of a list (`ListKeys`), and whether the phone
//! layout is in use (`phone_layout`): the catalog's list, the marked modules, a program's page.


use folia_model::labels::TurnusSeason;
use folia_model::rows::CatalogRow;
use folia_routes::url::{self};
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
    /// Where the row leads on the desktop: in the app its list with this module previewed next
    /// to it, on the server's page the module's own page. On a phone it leads to the module's
    /// own page either way, unless the list shows its modules `in_place`.
    #[prop(into)] preview: Signal<String>,
    /// This module is the one previewed.
    #[prop(into)] current: Signal<bool>,
    phone: RwSignal<bool>,
    with_program: bool,
    /// In the list of marked modules a module whose mark was taken away stays where it is,
    /// dimmed, so that a slip is one click to undo.
    #[prop(optional)] dim_unmarked: bool,
    /// The list shows its modules in place (a local view, `crate::local`): on a phone as well
    /// the row leads to `preview`, where the module is the page, not to the module's own page.
    #[prop(optional)] in_place: bool,
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
    let target = row.id.clone();
    let finder = use_context::<Finder>();
    // The preview next to the list; on a phone the module's own page, or where the list shows it
    // in place, the module filling the list's page. The module's page takes along what „Einplanen"
    // aims at from the catalog (`?plan=…&fill=…`), as the preview's „Vollbild" does. `preview` is a
    // path of the app; the link carries the language's prefix.
    let href = move || {
        t.path(&if phone.get() && !in_place {
            let hint = finder.and_then(|finder| finder.hint.get()).map(|hint| hint.query()).unwrap_or_default();
            format!("{}{hint}", url::module_path(&target))
        } else {
            preview.get()
        })
    };
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
    let swipe = (APP && swipe).then(|| RowSwipe::new(row.id.clone(), row.turnus_season.as_ref().and_then(|turnus| turnus.known()), finder.map(|finder| finder.hint), phone));
    let unmarked = move || unmarked.is_some_and(|unmarked| unmarked.get());
    let link = view! {
        <a class="row" href=href data-noscroll="" data-id=row.id.clone() aria-current=move || current.get().then_some("true")>
            <div class="t">
                <b>{row.title.clone()}</b>
                <small>
                    <span class="mono">{row.id.clone()}</span>
                    {with_program.then(|| view! { <KindBadge kind=row.kind.clone()/> })}
                    // Inside a program the study plan's semester stands at the row (the
                    // list is in plan order, without headings between the semesters).
                    {with_program.then(|| row.plan_semester.map(|n| view! { <span class="plan-sem">{(t.format.semesters)(&(t.format.semester_one)(n))}</span> }))}
                    <OfferBadge status=row.offer_status.clone()/>
                    {(row.is_fues && !with_program).then(|| view! { <span class="flag neutral">"FÜS"</span> })}
                    {(row.is_limited == Some(true)).then(|| view! { <span class="flag neutral">{t.catalog.limited_places}</span> })}
                    {fit_note}
                    <span class="narrow-only">{language.map(|l| format!("{l} · "))}{events.clone()}</span>
                </small>
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
