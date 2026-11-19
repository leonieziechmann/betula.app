//! The Stundenplan has two states (owner, 2026-11-17: „Ich glaube, das was hier die richtige Lösung
//! wäre ist bei dem Kalender einen edit modus hinzuzufügen, weil man dadurch unnötiges ui ausblenden
//! kann, und dann im edit modus auch Termine leicht anzeigen kann, die man ausgeblendet hat"):
//! looking, the default, where nothing on the page changes the plan, and editing (`edit=1` in the
//! address), where the page has its own controls and shows what the plan leaves out, faint, in its
//! place. What one opens on purpose — a Termin's card, the module's panel — changes what it is about
//! in both states.
//!
//! The mode is an address (`StudyplanUrl::edit`): „Bearbeiten" in the head enters it, and „Fertig",
//! Esc, Back and the pressed „Bearbeiten" leave it; a choice in focus (`wahl=`) is the mode's too, so
//! an old link `?wahl=…` opens editing. The head holds the same controls in both states, and under
//! it one row of a fixed height holds what the state says (`StatusRow`): the sum and „3 Wahlen offen
//! ›" while looking, the mode's bar while editing, the dark bar of a choice in focus — one bar at a
//! time, so that no slot of the week moves a pixel when the mode comes or goes (the owner,
//! 2026-09-25: „Da sollte sich das Layout nicht viel shiften"). On a phone the mode is a dock in the
//! tab bar's place (owner, 2026-11-04: „wenn unten die navbar mit ausgetauscht wird mit einer
//! interaction bar"), and nothing over the week comes with it.
//!
//! Every action is a button (CLAUDE.md, „No action is a key's alone"): Esc is „Fertig"'s key, named
//! beside it. „Bearbeiten" has a name as a command (R25). What shows the mode reads `PlanCtx::edit`,
//! which follows a click in its frame (R21); a link is built from the address, never from `edit`
//! beside it (R16).

use folia_routes::url::StudyplanUrl;
use folia_shell::pending::{same_page, Pending};
use folia_shell::tabs::Tabs;
use leptos::prelude::*;

use crate::i18n;
use folia_design::ui::{Icon, Shortcut};
use super::head::ClashFigure;
use super::open::{open_said, FocusBar, OpenUnder, OpenWay, OPEN_ROWS_ID};
use super::PlanCtx;

/// Whether an address is the plan being edited: it says so, or it holds a choice in focus, which is
/// the mode's (an old link `?wahl=…` too).
pub(super) fn on(url: &StudyplanUrl) -> bool {
    url.edit || url.wahl.is_some()
}

/// „Fertig": the plan looked at again (`with_edit(false)`, which lets a choice in focus go too), and
/// whether that is a step back (`data-back`, as `open::back_of`).
fn leave_link(ctx: PlanCtx) -> (Signal<String>, Signal<Option<&'static str>>) {
    let t = i18n::t();
    let tabs = Tabs::expect();
    let href = Signal::derive(move || t.path(&ctx.url.with(|url| url.with_edit(false).path())));
    let back = Signal::derive(move || {
        let (page, now) = ctx.url.with(|url| (url.with_edit(false).path(), url.path()));
        tabs.is_some_and(|tabs| same_page(&tabs.before(&now), &page)).then_some("history")
    });
    (href, back)
}

/// „Bearbeiten" at the head's right end, in both states, pressed while editing: a link to the plan
/// in the mode, so that the mode is an address (a new tab, Back), with the role of a button, which
/// it acts as (`aria-pressed`; Space acts as Enter does). Pressed, it leaves the mode as „Fertig"
/// does, in the frame of the click (R21) — the way out of the mode and of a choice in focus at once
/// (two layers, one click) — and is named so as a command („Bearbeiten beenden"). It carries no
/// `data-action="close-focus"`: Esc clicks the first such link of the page, and with a choice in
/// focus one Esc would then leave two layers. Where the mode was left by a control that goes with it
/// („Fertig"), the keyboard comes to this button, never to the page's body.
#[component]
pub(super) fn EditToggle(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let e = &t.studyplan_edit;
    let (going, tabs) = (Pending::expect(), Tabs::expect());
    let href = move || ctx.url.with(|url| t.path(&url.with_edit(!on(url)).path()));
    let leave = move |ev: leptos::ev::MouseEvent| {
        let plain = !ev.default_prevented() && ev.button() == 0 && !ev.ctrl_key() && !ev.meta_key() && !ev.shift_key() && !ev.alt_key();
        if !plain || !ctx.url.with_untracked(on) {
            // Entering, or a new tab: the link's own step.
            return;
        }
        ev.prevent_default();
        let (page, now) = ctx.url.with_untracked(|url| (url.with_edit(false).path(), url.path()));
        folia_shell::nav::close_to(going, &page, || tabs.is_some_and(|tabs| same_page(&tabs.before(&now), &page)));
    };
    // A button is pressed by Space too; a link is not.
    let space = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == " " && !ev.ctrl_key() && !ev.meta_key() && !ev.alt_key() {
            ev.prevent_default();
            #[cfg(feature = "csr")]
            {
                use leptos::wasm_bindgen::JsCast;
                if let Some(element) = ev.current_target().and_then(|target| target.dyn_into::<leptos::web_sys::HtmlElement>().ok()) {
                    element.click();
                }
            }
        }
    };
    let name = move || if ctx.edit.get() { e.edit_end } else { e.edit_title };
    // The mode left by „Fertig" or Esc, whose bar goes with it: the keyboard to this button. Entered
    // by „3 Wahlen offen ›", which goes with the looking state: to the first of „Offen"'s rows (on
    // a narrow page the bar's figure that unfolds them, on a phone the line), else „Fertig".
    #[cfg(feature = "csr")]
    Effect::new(move |was: Option<bool>| {
        let now = ctx.edit.get();
        if was == Some(true) && !now {
            ctx.left.again();
        }
        match (was, now) {
            (Some(true), false) => request_animation_frame(|| {
                if lost_focus() {
                    folia_design::nav::focus_by_id(EDIT_TOGGLE_ID);
                }
            }),
            (Some(false), true) => request_animation_frame(|| {
                if lost_focus() {
                    focus_first_shown(&[".sp-open .sp-open-rows a", ".sp-editbar .eb-open", "details.sp-open-line > summary", ".sp-editbar .sp-done", ".sp-editdock .sp-editdock-go"]);
                }
            }),
            _ => {}
        }
        now
    });
    view! {
        <a
            class="ghost sp-edit-toggle"
            class:on=move || ctx.edit.get()
            id=EDIT_TOGGLE_ID
            href=href
            role="button"
            data-noscroll=""
            aria-pressed=move || if ctx.edit.get() { "true" } else { "false" }
            title=name
            data-command=name
            on:click=leave
            on:keydown=space
        >
            <Icon name="pencil"/>
            {e.edit}
        </a>
    }
}

/// The id of „Bearbeiten", which the keyboard comes back to when the mode is left.
const EDIT_TOGGLE_ID: &str = "sp-edit-toggle";

/// The keyboard to the first element of the page in front that one of `selectors` finds and that is
/// shown (the stylesheet shows some of them on some screens alone), in the order of `selectors`.
#[cfg(feature = "csr")]
fn focus_first_shown(selectors: &[&str]) {
    use leptos::wasm_bindgen::JsCast;
    let shown = selectors.iter().find_map(|selector| folia_design::stage::select_all(selector).into_iter().find(|element| { let rect = element.get_bounding_client_rect(); rect.width() > 0.0 && rect.height() > 0.0 }));
    if let Some(element) = shown.and_then(|element| element.dyn_into::<leptos::web_sys::HtmlElement>().ok()) {
        let options = leptos::web_sys::FocusOptions::new();
        options.set_prevent_scroll(true);
        let _ = element.focus_with_options(&options);
    }
}

/// Whether the keyboard is nowhere: on the page's body, or on an element no longer in the page (a
/// bar that went with the mode).
#[cfg(feature = "csr")]
pub(super) fn lost_focus() -> bool {
    let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) else { return false };
    match document.active_element() {
        None => true,
        Some(active) => active.is_same_node(document.body().map(Into::into).as_ref()) || !active.is_connected(),
    }
}

/// How many things the student hid (`folia_timetable::views::hidden_count`): events, Termine,
/// dates and kinds, never a choice made.
fn hidden(ctx: PlanCtx) -> Memo<usize> {
    Memo::new(move |_| ctx.table.with(|table| table.as_ref().map_or(0, |table| folia_timetable::views::hidden_count(table).total())))
}

/// „3 Wahlen offen · 2 ausgeblendet": what is open (`open`, „Offen"'s phrase) and how many things
/// are hidden, joined by `sep`; empty with neither.
fn figures_text(open: Option<&str>, hidden: usize, sep: &str, t: &i18n::Texts) -> String {
    let hid = (hidden > 0).then(|| (t.studyplan_head.hidden)(hidden));
    open.map(str::to_string).into_iter().chain(hid).collect::<Vec<_>>().join(sep)
}

/// What the mode says to a screen reader: its name and figures while editing („Stundenplan
/// bearbeiten: 3 Wahlen offen, 2 ausgeblendet"), „Bearbeiten beendet" once it was left, nothing
/// before. It reads `edit` (of the address) and the two figures (of the timetable): siblings (R16).
pub(super) fn said(ctx: PlanCtx) -> Memo<String> {
    let t = i18n::t();
    // Every choice open, the one in focus too while none of its groups is ticked.
    let open = super::open::open_said_all(ctx);
    let hidden = hidden(ctx);
    Memo::new(move |before: Option<&String>| {
        if ctx.edit.get() {
            let figures = open.with(|open| figures_text(open.as_ref().map(|(said, _)| said.as_str()), hidden.get(), ", ", t));
            return (t.studyplan_edit.said)(&figures);
        }
        match before.is_some_and(|before| !before.is_empty()) {
            true => t.studyplan_edit.ended.to_string(),
            false => String::new(),
        }
    })
}

/// The status row: one row under the head, on every screen wider than a phone, as
/// tall in each of its three states — the sum and „3 Wahlen offen ›" while looking, the mode's bar
/// while editing, the bar of a choice in focus. One bar at a time. It stands above the week and
/// changes nothing above itself („what picks stands above what it changes").
#[component]
pub(super) fn StatusRow(ctx: PlanCtx, sum: Memo<String>) -> impl IntoView {
    let focused = super::open::focused(ctx);
    view! {
        <div class="sp-status">
            // The choice in focus: its bar, and compared on a narrow page the groups under it;
            // nothing without one.
            <FocusBar ctx/>
            {move || (!focused.get()).then(|| match ctx.edit.get() {
                true => view! { <EditBar ctx/><OpenUnder ctx/> }.into_any(),
                false => view! {
                    <div class="sp-look">
                        <p class="sp-sum">{move || sum.get()}</p>
                        <ClashFigure ctx/>
                        <OpenWay ctx/>
                    </div>
                }
                .into_any(),
            })}
        </div>
    }
}

/// The module list's head to the top of the window, under the bar (the window's `scroll-padding`,
/// app.css „phone and tablet").
fn to_list() {
    #[cfg(feature = "csr")]
    if let Some(list) = folia_design::stage::select(".sp-list") {
        let options = leptos::web_sys::ScrollIntoViewOptions::new();
        options.set_block(leptos::web_sys::ScrollLogicalPosition::Start);
        list.scroll_into_view_with_scroll_into_view_options(&options);
    }
}

/// The bar's figure „2 ausgeblendet": the keyboard on „Alle einblenden" under the list, which
/// brings it all back — the figure says what is hidden, the foot is where it comes back. The page
/// scrolls only as far as brings the foot into sight, and not at all where it is in sight: the
/// bar under the pointer stays where it is.
fn to_show_all() {
    #[cfg(feature = "csr")]
    if let Some(foot) = folia_design::stage::select(&format!("#{}", super::modules::SHOW_ALL_ID)) {
        let options = leptos::web_sys::ScrollIntoViewOptions::new();
        options.set_block(leptos::web_sys::ScrollLogicalPosition::Nearest);
        foot.scroll_into_view_with_scroll_into_view_options(&options);
    }
    folia_design::nav::focus_by_id(super::modules::SHOW_ALL_ID);
}

/// The mode's bar (accent-soft, as the workbench's bar over the catalog; the dark bar stays the
/// focus's): the pencil, „Stundenplan bearbeiten" and the figures under it, „+ Termin" and „Fertig"
/// with Esc. Where the page has no column beside the week, the open figure is a button that unfolds
/// „Offen"'s rows under the bar (`PlanCtx::unfolded`); beside the week the panel stands, and the
/// figure is words (app.css shows one of the two).
#[component]
fn EditBar(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let e = &t.studyplan_edit;
    let open = open_said(ctx);
    let hidden = hidden(ctx);
    let (done, back) = leave_link(ctx);
    // „Fertig" stands where „3 Wahlen offen ›" was clicked: the second click of a slow double click
    // was meant for that, and left the mode again (folia_design::nav::Newborn).
    let born = folia_design::nav::Newborn::now();
    let phrase = move || open.with(|open| open.as_ref().map(|(said, _)| said.clone()).unwrap_or_default());
    let figures = move || {
        let any = open.with(Option::is_some);
        let hid = hidden.get();
        view! {
            {any.then(|| view! {
                <button
                    class="eb-open"
                    type="button"
                    aria-expanded=move || if ctx.unfolded.get() { "true" } else { "false" }
                    aria-controls=OPEN_ROWS_ID
                    on:click=move |_| ctx.unfolded.update(|unfolded| *unfolded = !*unfolded)
                >
                    {phrase}
                    <Icon name="chevron-down" class="chev"/>
                </button>
                <span class="eb-open-fig">{phrase}</span>
            })}
            {(hid > 0).then(|| {
                let said = (t.studyplan_head.hidden)(hid);
                let name = format!("{said} · {}", t.studyplan_head.to_unhide_all);
                view! {
                    // The figures stand in a row of boxes, which drops the spaces at a text's ends:
                    // the separator keeps them in a box of its own („offen · 1", not „offen·1").
                    {any.then(|| view! { <span class="eb-sep">" · "</span> })}
                    <button class="eb-hid" type="button" title=t.studyplan_head.to_unhide_all aria-label=name on:click=move |_| to_show_all()>{said}</button>
                }
            })}
        }
    };
    view! {
        <div class="sp-editbar" role="region" aria-label=e.editing>
            <i class="eb-ic" aria-hidden="true"><Icon name="pencil"/></i>
            <span class="eb-text">
                <b>{e.editing}</b>
                <span class="figs">{figures}</span>
            </span>
            <ClashFigure ctx/>
            <button class="btn" type="button" data-action="own-add" title=t.studyplan_own.add_title data-command=t.studyplan_own.form_new data-command-group=t.studyplan.title on:click=move |_| super::own::ask_today(ctx, 0, None)>
                <Icon name="plus"/>{t.studyplan_own.add}
            </button>
            <a class="btn sp-done" href=done data-action="close-focus" data-back=move || back.get() data-noscroll="" data-command=t.studyplan_open.done data-command-group=t.studyplan.title data-command-keys="Esc" on:click=move |ev| { born.swallows(&ev); }>
                {t.studyplan_open.done}
                <Shortcut keys="Esc"/>
            </a>
        </div>
    }
}

/// The mode on a phone, in the tab bar's place (owner, 2026-11-04: „wenn unten die navbar mit
/// ausgetauscht wird mit einer interaction bar"): „Bearbeiten" with the figures small under it,
/// „+ Termin" and „Fertig". Not while a choice is in focus: its sheet stands at the bottom then and
/// has the way out (one bar at a time). Its box is the workbench's dock's, its class its own (a
/// check looks for `.bench-dock` in the whole document).
#[component]
pub(super) fn EditDock(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let e = &t.studyplan_edit;
    let focused = super::open::focused(ctx);
    let open = open_said(ctx);
    let hidden = hidden(ctx);
    // The two figures apart: where the dock is short the second gives way whole (app.css), never a
    // word cut in its middle („3 Wahlen offen · 2 ausgebl…").
    let figures = Memo::new(move |_| {
        let open = open.with(|open| open.as_ref().map(|(said, _)| said.clone()));
        let hid = hidden.get();
        let hid = (hid > 0).then(|| (t.studyplan_head.hidden)(hid));
        match (open, hid) {
            (Some(open), Some(hid)) => (Some(open), Some(format!(" · {hid}"))),
            (open, hid) => (open.or(hid), None),
        }
    });
    move || {
        (ctx.edit.get() && !focused.get()).then(|| {
            let (done, back) = leave_link(ctx);
            view! {
                <div class="sp-editdock" role="toolbar" aria-label=e.editing>
                    // Its left part brings the module list into view, where the eyes are: the mode's
                    // workbench on a phone stands under the week.
                    <button class="sp-editdock-w" type="button" data-action="to-list" title=e.to_list on:click=move |_| to_list()>
                        <b>{e.dock}</b>
                        {move || figures.with(|(first, second)| first.clone().map(|first| view! {
                            <span class="figs"><span>{first}</span>{second.clone().map(|second| view! { <span>{second}</span> })}</span>
                        }))}
                    </button>
                    <button class="btn secondary sp-editdock-add" type="button" data-action="own-add" title=t.studyplan_own.add_title on:click=move |_| super::own::ask_today(ctx, 0, None)>
                        <Icon name="plus"/>{t.studyplan_own.add}
                    </button>
                    <a class="btn primary sp-editdock-go" href=done data-action="close-focus" data-back=move || back.get() data-noscroll="">{t.studyplan_open.done}</a>
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_says_what_is_open_and_what_is_hidden() {
        let de = &i18n::DE;
        assert_eq!(figures_text(Some("3 Wahlen offen"), 2, " · ", de), "3 Wahlen offen · 2 ausgeblendet");
        assert_eq!(figures_text(Some("3 Wahlen offen"), 2, ", ", de), "3 Wahlen offen, 2 ausgeblendet");
        assert_eq!(figures_text(Some("1 Änderung offen"), 0, " · ", de), "1 Änderung offen");
        assert_eq!(figures_text(None, 1, " · ", de), "1 ausgeblendet");
        assert_eq!(figures_text(None, 0, " · ", de), "");
        assert_eq!(figures_text(None, 2, ", ", &i18n::EN), "2 hidden");
    }

    #[test]
    fn a_choice_in_focus_is_the_mode() {
        assert!(!on(&StudyplanUrl::parse("sem=2026W")));
        assert!(on(&StudyplanUrl::parse("edit=1")));
        assert!(on(&StudyplanUrl::parse("wahl=e148370")));
    }
}
