//! The menu at the arrow beside „Eingeplant" (owner, 2026-10-20: „bei merken haben wir ja nachdem es
//! selected ist so einen more context pfeil ran gemacht, damit man das nicht löschen und dann wieder
//! merken muss, um was daran zu ändern. Mach das mal bei planen auch so. Dass man da das semester
//! ändern kann aber zum beispiel auch das modul als bestanden markieren kann"): what is planned
//! changes in place, worded as the rows' menus of „Mein Studium" word it —
//!
//! - „Als bestanden markieren" in the semester it is planned in, or „Bestanden zurücknehmen";
//! - „Verschieben nach ›", every semester from the first Fachsemester to a few ahead
//!   (`studyplan::move_targets`), each with its Fachsemester, „jetzt" at the current one and whether
//!   it offers the module; one behind the student as well (Betula does not say no: who passed it
//!   then puts it there);
//! - „Im Stundenplan öffnen", the semester's week with the module beside it;
//! - „Entfernen", what a click on the switch does.
//!
//! Each holds at once and says so in the note of „Einplanen" (`plannote`), with „Rückgängig"; a
//! move as after a press, with ‹ ›. On a wide screen the menu stands under the switch and its arrow,
//! the semesters beside it; on a phone it is a sheet from below and the semesters take its place
//! (app.css `.st-pop`). The arrow keys go through it, → and ← into the semesters and back, Esc and
//! a click beside it close it, and the focus goes back to the arrow.

use std::time::Duration;

use folia_calendar::semester::SemesterKey;
use folia_plans::study::fs_of;
use folia_routes::url::StudyplanUrl;
use leptos::prelude::*;

use folia_data::use_data;
use folia_design::nav::{self, room, Room};
use folia_design::ui::Icon;
use folia_stores::collections::Anchor;
use folia_stores::myprogram::MyProgram;
use folia_stores::studyplan::{move_targets, MenuAsk, Planning, Studyplan};

use crate::collect::FocusBack;
use crate::i18n::{self, Texts};
use crate::list::phone_layout;

/// The sizes of the menu on a wide screen (app.css `.st-pop`), to place it before it is drawn.
const WIDTH: f64 = 264.0;
const SUB_WIDTH: f64 = 280.0;
const PAD: f64 = 6.0;
const ITEM: f64 = 36.0;
const LINE: f64 = 9.0;
/// How far it stays from the window's edges.
const EDGE: f64 = 8.0;

/// What an entry does.
#[derive(Clone, Debug, PartialEq)]
enum Does {
    /// Marks the module as passed there, or takes that back.
    Passed(bool),
    /// Moves it to the semester.
    Move(SemesterKey),
    /// The semesters to move it to.
    Sub,
    /// A way to a page.
    Link(String),
    Remove,
    Line,
}

/// An entry of the menu or of its semesters.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    /// What the checks find it by.
    id: Option<&'static str>,
    icon: Option<&'static str>,
    label: String,
    /// The current semester, among those to move to.
    now: bool,
    /// Small, at its end: „angeboten", „nicht angeboten", with its class.
    note: Option<(&'static str, &'static str)>,
    danger: bool,
    does: Does,
}

impl Entry {
    fn new(id: Option<&'static str>, icon: Option<&'static str>, label: impl Into<String>, does: Does) -> Self {
        Entry { id, icon, label: label.into(), now: false, note: None, danger: false, does }
    }

    fn height(&self) -> f64 {
        if self.does == Does::Line {
            LINE
        } else {
            ITEM
        }
    }
}

/// The menu's entries for what was asked.
fn entries(ask: &MenuAsk, t: &'static Texts) -> Vec<Entry> {
    let p = &t.planner;
    let (id, at) = (ask.what.id.as_str(), ask.at);
    let timetable = t.path(&StudyplanUrl { sem: Some(at.key()), open: Some(id.to_string()), ..Default::default() }.path());
    vec![
        if ask.passed {
            Entry::new(Some("plan-menu-passed"), Some("rotate-ccw"), p.unmark_passed, Does::Passed(false))
        } else {
            Entry::new(Some("plan-menu-passed"), Some("circle-check-big"), p.mark_passed, Does::Passed(true))
        },
        Entry::new(Some("plan-menu-move"), Some("arrow-right-left"), p.move_to, Does::Sub),
        Entry::new(Some("plan-menu-timetable"), Some("calendar-range"), p.in_timetable, Does::Link(timetable)),
        Entry::new(None, None, "", Does::Line),
        Entry { danger: true, ..Entry::new(Some("plan-menu-remove"), Some("trash-2"), p.remove, Does::Remove) },
    ]
}

/// The semesters to move the module to, each with its Fachsemester where the Studienbeginn is
/// known („SoSe 27 · 10. FS", „Urlaubssemester" for one of leave), „jetzt" at the current one and
/// whether it offers the module.
fn semesters(ask: &MenuAsk, floor: SemesterKey, start: Option<SemesterKey>, leave: &std::collections::BTreeSet<SemesterKey>, until: Option<SemesterKey>, t: &'static Texts) -> Vec<Entry> {
    let p = &t.planner;
    move_targets(ask.at, ask.what.current, floor, until)
        .into_iter()
        .map(|s| {
            let short = s.short(t.locale);
            let label = match start.filter(|start| s >= *start) {
                Some(start) => match fs_of(start, s, leave) {
                    Some(fs) => format!("{short} · {}", (p.fs_short)(fs)),
                    None => format!("{short} · {}", p.leave),
                },
                None => short,
            };
            let note = if ask.what.offer.offered(s) { (p.offered, "ok") } else { (p.not_offered, "warn") };
            Entry { now: s == ask.what.current, note: Some(note), ..Entry::new(None, None, label, Does::Move(s)) }
        })
        .collect()
}

/// Where a menu of `size` stands under the switch and its arrow (`at`): its right edge at theirs,
/// above them where the window has no room below; never past the window's edges, nor under the
/// rail (`Room`). In the middle, near the top, without them.
fn place(at: Option<Anchor>, size: (f64, f64), room: Room) -> (f64, f64) {
    let ((width, height), Room { left: rail, width: view_width, height: view_height }) = (size, room);
    let least = rail + EDGE;
    let (left, top) = match at {
        Some(at) => {
            let below = at.bottom + 4.0;
            let above = at.top - 4.0 - height;
            (at.right - width, if below + height <= view_height - EDGE || above < EDGE { below } else { above })
        }
        None => ((view_width - width) / 2.0, view_height * 0.12),
    };
    (left.clamp(least, (view_width - width - EDGE).max(least)), top.clamp(EDGE, (view_height - height - EDGE).max(EDGE)))
}

/// Where the semesters stand beside the menu at `menu`, whose entry begins `entry_top` px from the
/// window's top: right of it, left of it where there is no room; their first entry level with it;
/// never under the rail.
fn place_beside(menu: (f64, f64), entry_top: f64, size: (f64, f64), room: Room) -> (f64, f64) {
    let ((left, _), (width, height), Room { left: rail, width: view_width, height: view_height }) = (menu, size, room);
    let least = rail + EDGE;
    let right = left + WIDTH;
    let x = if right - 2.0 + width <= view_width - EDGE { right - 2.0 } else { left + 2.0 - width };
    (x.clamp(least, (view_width - width - EDGE).max(least)), (entry_top - PAD).clamp(EDGE, (view_height - height - EDGE).max(EDGE)))
}

fn height_of(entries: &[Entry]) -> f64 {
    2.0 * PAD + entries.iter().map(Entry::height).sum::<f64>()
}

/// The menu while it is asked for, once in the app (`folia_app::App`).
#[component]
pub fn PlanMenuHost() -> impl IntoView {
    let Some(planning) = Planning::expect() else { return ().into_any() };
    let asked = Memo::new(move |_| planning.menu());
    (move || asked.get().map(|ask| view! { <PlanMenu ask planning/> })).into_any()
}

/// The focus goes from entry to entry of `menu` (`.plan-menu.root`, `.plan-menu.sub`): `by` on from
/// the one that has it, round; 0 the first.
fn step_focus(menu: &str, by: i32) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
        let Ok(list) = document.query_selector_all(&format!("{menu} [role=menuitem]")) else { return };
        let items: Vec<web_sys::HtmlElement> = (0..list.length()).filter_map(|i| list.item(i)).filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok()).collect();
        if items.is_empty() {
            return;
        }
        let active = document.active_element();
        let at = items.iter().position(|item| active.as_ref().is_some_and(|active| active.is_same_node(Some(item))));
        let count = items.len() as i32;
        let to = match at {
            Some(at) if by != 0 => (at as i32 + by).rem_euclid(count),
            _ if by < 0 => count - 1,
            _ => 0,
        };
        if let Some(item) = usize::try_from(to).ok().and_then(|to| items.get(to)) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = item.focus_with_options(&options);
        }
    }
    #[cfg(not(feature = "csr"))]
    let _ = (menu, by);
}

/// The focus from the arrow to its switch, beside it.
fn focus_switch() {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let arrow = web_sys::window().and_then(|window| window.document()).and_then(|document| document.active_element()).filter(|active| active.class_list().contains("plan-more"));
        if let Some(switch) = arrow.and_then(|arrow| arrow.previous_element_sibling()).and_then(|switch| switch.dyn_into::<web_sys::HtmlElement>().ok()) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = switch.focus_with_options(&options);
        }
    }
}

/// The id of what has the focus.
fn focused_id() -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let id = web_sys::window()?.document()?.active_element()?.id();
        (!id.is_empty()).then_some(id)
    }
    #[cfg(not(feature = "csr"))]
    None
}

#[component]
fn PlanMenu(ask: MenuAsk, planning: Planning) -> impl IntoView {
    let t = i18n::t();
    let Some(plan) = Studyplan::expect() else { return ().into_any() };
    let mine = MyProgram::expect();
    let source = use_data().ok();
    let phone = phone_layout();
    let back = FocusBack::take();
    let close = move || {
        planning.close_menu();
        back.give();
    };
    folia_shell::nav::layer_while(move || planning.menu().is_some(), move || planning.close_menu(), folia_shell::nav::Kind::Over);

    let root = entries(&ask, t);
    let start = mine.and_then(MyProgram::start_untracked);
    let (leave, until) = mine.map(|mine| mine.with(|doc| (doc.leave.clone(), doc.until))).unwrap_or_default();
    let subs = semesters(&ask, planning.floor(ask.what.current), start, &leave, until, t);

    // The semesters open; whether the keys opened them (their first entry takes the focus).
    let sub = RwSignal::new(false);
    // They close a while after the pointer goes to another entry: time to cross to them.
    let closing = StoredValue::new(None::<TimeoutHandle>);
    let stay = move || {
        if let Some(handle) = closing.get_value() {
            handle.clear();
            closing.set_value(None);
        }
    };
    let leave_sub = move || {
        if sub.get_untracked() && closing.get_value().is_none() {
            let handle = set_timeout_with_handle(
                move || {
                    closing.set_value(None);
                    sub.try_set(false);
                },
                Duration::from_millis(300),
            );
            closing.set_value(handle.ok());
        }
    };
    let open_sub = move |keys: bool| {
        stay();
        sub.set(true);
        if keys {
            request_animation_frame(|| step_focus(".plan-menu.sub", 0));
        }
    };
    let close_sub = move || {
        stay();
        if sub.get_untracked() {
            sub.set(false);
            nav::focus_selector("#plan-menu-move");
        }
    };
    request_animation_frame(|| step_focus(".plan-menu.root", 0));

    let run = {
        let what = ask.what.clone();
        let at = ask.at;
        move |does: Does| {
            let what = what.clone();
            match does {
                Does::Passed(passed) => {
                    close();
                    planning.set_passed(plan, what, at, passed);
                }
                Does::Move(to) => {
                    close();
                    planning.move_planned(plan, what, at, to);
                }
                Does::Remove => {
                    close();
                    // The arrow goes with the module: the focus goes to its switch.
                    focus_switch();
                    planning.remove(plan, source.clone(), what, at);
                }
                Does::Sub => open_sub(true),
                Does::Link(_) | Does::Line => {}
            }
        }
    };
    let root_keys = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "ArrowDown" => {
            ev.prevent_default();
            step_focus(".plan-menu.root", 1);
        }
        "ArrowUp" => {
            ev.prevent_default();
            step_focus(".plan-menu.root", -1);
        }
        "ArrowRight" if focused_id().as_deref() == Some("plan-menu-move") => {
            ev.prevent_default();
            open_sub(true);
        }
        "Escape" | "Tab" => {
            ev.prevent_default();
            ev.stop_propagation();
            close();
        }
        _ => {}
    };
    let sub_keys = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "ArrowDown" => {
            ev.prevent_default();
            step_focus(".plan-menu.sub", 1);
        }
        "ArrowUp" => {
            ev.prevent_default();
            step_focus(".plan-menu.sub", -1);
        }
        "ArrowLeft" | "Escape" => {
            ev.prevent_default();
            ev.stop_propagation();
            close_sub();
        }
        "Tab" => {
            ev.prevent_default();
            close();
        }
        _ => {}
    };

    // A mouse moved onto another entry leaves the semesters; a phone opens them by a tap alone.
    let hovers = move |ev: &leptos::ev::PointerEvent| ev.pointer_type() == "mouse" && !phone.get_untracked();
    let entry = {
        let run = run.clone();
        move |in_sub: bool, entry: Entry| -> AnyView {
            let Entry { id, icon, label, now, note, danger, does } = entry;
            let icon = icon.map(|icon| view! { <Icon name=icon/> });
            let words = view! {
                <span class="st-pop-label"><span>{label}{now.then(|| view! { " " <span class="st-now">{t.planner.now}</span> })}</span></span>
            };
            let note = note.map(|(text, class)| view! { <span class=format!("st-pop-note {class}")>{(class == "warn").then(|| view! { <Icon name="triangle-alert"/> })}{text}</span> });
            let others = move |ev: leptos::ev::PointerEvent| {
                if !in_sub && hovers(&ev) {
                    leave_sub();
                }
            };
            match does {
                Does::Line => view! { <hr/> }.into_any(),
                Does::Link(href) => view! {
                    <a class="st-pop-item" role="menuitem" id=id href=href on:click=move |_| planning.close_menu() on:pointermove=others>{icon}{words}</a>
                }
                .into_any(),
                Does::Sub => view! {
                    <button
                        class="st-pop-item"
                        type="button"
                        role="menuitem"
                        aria-haspopup="menu"
                        aria-expanded=move || if sub.get() { "true" } else { "false" }
                        id=id
                        // A click opens them (and keeps them open where the pointer opened them
                        // already); from the keys (Enter, Space) their first entry takes the focus.
                        on:click=move |ev: leptos::ev::MouseEvent| open_sub(ev.detail() == 0)
                        on:pointermove=move |ev: leptos::ev::PointerEvent| if hovers(&ev) && !sub.get_untracked() { open_sub(false) }
                    >
                        {icon}{words}<Icon name="chevron-right" class="st-pop-more"/>
                    </button>
                }
                .into_any(),
                does => {
                    let run = run.clone();
                    view! {
                        <button class="st-pop-item" class:danger=danger type="button" role="menuitem" id=id on:click=move |_| run(does.clone()) on:pointermove=others>
                            {icon}{words}{note}
                        </button>
                    }
                    .into_any()
                }
            }
        }
    };

    let view_size = room();
    let height = height_of(&root).min(view_size.height - 2.0 * EDGE);
    let (left, top) = place(ask.anchor, (WIDTH, height), view_size);
    let move_top = top + PAD + root.iter().take_while(|entry| entry.does != Does::Sub).map(Entry::height).sum::<f64>();
    let sub_size = (SUB_WIDTH, height_of(&subs).min(view_size.height - 2.0 * EDGE));
    let (x, y) = place_beside((left, top), move_top, sub_size, view_size);
    let move_title = (t.planner.move_title)(&ask.what.title);
    let title = ask.what.title.clone();
    let sub_panel = {
        let entry = entry.clone();
        move || {
            sub.get().then(|| {
                let entry = entry.clone();
                let (label, back_label) = (move_title.clone(), move_title.clone());
                view! {
                    <div data-sheet="" class="st-pop sub plan-menu" role="menu" aria-label=label style=format!("--x:{x:.0}px;--y:{y:.0}px;--w:{SUB_WIDTH:.0}px") on:keydown=sub_keys on:pointerenter=move |_| stay()>
                        <button class="st-pop-back st-phone" type="button" on:click=move |_| close_sub()><Icon name="chevron-left"/><span>{back_label}</span></button>
                        {subs.clone().into_iter().map(|semester| entry(true, semester)).collect_view()}
                    </div>
                }
            })
        }
    };
    view! {
        <div class="st-pop-shade" on:mousedown=|ev: leptos::ev::MouseEvent| ev.prevent_default() on:click=move |_| close() on:contextmenu=move |ev: leptos::ev::MouseEvent| { ev.prevent_default(); close(); } on:wheel=move |_| planning.close_menu()></div>
        <div data-sheet="" class="st-pop root plan-menu" class:has-sub=move || sub.get() role="menu" aria-label=format!("{} · {}", t.planner.more, title) style=format!("--x:{left:.0}px;--y:{top:.0}px;--w:{WIDTH:.0}px") on:keydown=root_keys>
            <p class="st-pop-title st-phone">{title.clone()}</p>
            {root.into_iter().map(|root_entry| entry(false, root_entry)).collect_view()}
        </div>
        {sub_panel}
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_plans::study::Offer;
    use folia_model::labels::TurnusSeason;
    use folia_stores::studyplan::Pressed;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    fn ask(at: &str, passed: bool) -> MenuAsk {
        let what = Pressed { id: "12330".into(), title: "Datenbanken".into(), current: key("2026W"), offer: Offer { season: Some(TurnusSeason::Winter), parity: None }, newest: None, hint: None };
        MenuAsk { what, at: key(at), passed, anchor: None }
    }

    #[test]
    fn the_menu_says_what_it_does_as_mein_studium_says_it() {
        let t = &crate::i18n::DE;
        let labels = |entries: Vec<Entry>| entries.into_iter().map(|entry| entry.label).collect::<Vec<_>>();
        assert_eq!(labels(entries(&ask("2026W", false), t)), ["Als bestanden markieren", "Verschieben nach", "Im Stundenplan öffnen", "", "Entfernen"]);
        assert_eq!(labels(entries(&ask("2026W", true), t)).first().map(String::as_str), Some("Bestanden zurücknehmen"));
        // The Stundenplan of the semester, with the module beside it.
        let link = entries(&ask("2027S", false), t).into_iter().find_map(|entry| match entry.does {
            Does::Link(href) => Some(href),
            _ => None,
        });
        assert_eq!(link.as_deref(), Some("/studyplan?sem=2027S&open=12330"));
    }

    #[test]
    fn the_semesters_say_their_fachsemester_now_and_the_offer() {
        let t = &crate::i18n::DE;
        let leave = [key("2025S")].into_iter().collect();
        let list = semesters(&ask("2027W", false), key("2024W"), Some(key("2024W")), &leave, None, t);
        let shown: Vec<(String, bool, &str)> = list.iter().map(|entry| (entry.label.clone(), entry.now, entry.note.map_or("", |note| note.0))).collect();
        assert_eq!(shown.first(), Some(&("WiSe 24/25 · 1. FS".to_string(), false, "angeboten")));
        assert_eq!(shown.get(1), Some(&("SoSe 25 · Urlaubssemester".to_string(), false, "nicht angeboten")));
        assert_eq!(shown.get(2), Some(&("WiSe 25/26 · 2. FS".to_string(), false, "angeboten")));
        assert!(shown.iter().any(|(label, now, _)| *now && label == "WiSe 26/27 · 4. FS"));
        assert!(!shown.iter().any(|(label, _, _)| label.starts_with("WiSe 27/28")), "the semester it is in is left out");
        assert_eq!(shown.last().map(|entry| entry.0.as_str()), Some("SoSe 29 · 9. FS"), "three after the semester it is in");
        // Without a Studienbeginn the semesters alone.
        let plain = semesters(&ask("2026W", false), key("2026S"), None, &Default::default(), None, t);
        assert_eq!(plain.first().map(|entry| entry.label.as_str()), Some("SoSe 26"));
    }

    #[test]
    fn the_menu_stands_under_the_switch_and_the_semesters_beside_it() {
        let view = Room::window(1440.0, 900.0);
        let pair = Anchor { left: 900.0, top: 300.0, right: 1200.0, bottom: 332.0 };
        assert_eq!(place(Some(pair), (264.0, 200.0), view), (936.0, 336.0));
        assert_eq!(place(Some(Anchor { top: 800.0, bottom: 832.0, ..pair }), (264.0, 200.0), view), (936.0, 596.0), "above it at the window's bottom");
        assert_eq!(place_beside((936.0, 336.0), 378.0, (280.0, 300.0), view), (658.0, 372.0), "left of it at the right edge");
        assert_eq!(place_beside((300.0, 336.0), 378.0, (280.0, 300.0), view), (562.0, 372.0));
        // Never under the rail.
        let railed = Room { left: 52.0, ..view };
        assert_eq!(place(Some(Anchor { left: 40.0, top: 300.0, right: 120.0, bottom: 332.0 }), (264.0, 200.0), railed).0, 60.0);
        assert_eq!(place_beside((1150.0, 336.0), 378.0, (280.0, 300.0), Room { left: 52.0, width: 1300.0, height: 900.0 }).0, 872.0);
    }
}
