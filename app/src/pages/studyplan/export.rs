//! The group „Kalender" (design A.8, D.9): the semester as an .ics file, and as an address a
//! calendar service fetches by itself. It stands in the sidebar, on a phone under the Termine
//! (`mod.rs`: the sidebar is the sheet „Anpassen" there).
//!
//! The file is made in the browser, from the page's own data and timetable, by the function the
//! feed uses (`StudyplanData::calendar`), so a download and a subscription of the same plan carry
//! the same bytes, and nothing of it reaches the server. It is made ahead, as a Blob URL: the link
//! has it before it is clicked, so the click has nothing left to do (R21). Writing a semester's
//! file takes up to 20 ms (100 ms on a slow phone), so it is not written on every change: a
//! semester's first file follows the frame that shows the semester (`nav::after_paint`), and a
//! change of the plan writes it anew once the plan has been left alone for a moment (`QUIET`), or
//! at once when the link is about to be used (a pointer on it, the focus on it). A burst of chips
//! and eye buttons thus writes it once, after they have answered. The link offers only a file of
//! the semester shown; a click that finds the file behind the plan (a pointer that rested on the
//! link through a change) makes it before the browser follows.
//!
//! The subscription is an address, `/calendar/<code>.ics`, whose code carries the semester, the
//! planned modules and what is hidden or chosen (`Subscription`; owner decision 2026-09-24), and the
//! program whose abbreviations the entries name the modules by (2026-09-25). The
//! page never asks for it (no preview, no prefetch); the calendar service does, from its own
//! servers, and the server makes the feed anew each time. Each way of handing the address out
//! keeps its code in the plan (`PlanDoc::remember`), a middle click and the context menu („Link
//! kopieren") too, so the page can say when the plan has moved on from what a calendar shows
//! („Abo veraltet").
//!
//! R16: the subscription is worked out from the timetable, from a memo of what the semester hides
//! (`hides`) and from one of the program the data is loaded for (`program`), which are the
//! timetable's siblings (derived from `selection` and from `data` as it is), never from `selection`
//! or `data` beside the timetable made of them. The file follows the timetable and the data's
//! other parts the same way (`parts`, a sibling of the timetable under `data`).

use std::collections::BTreeMap;

use catalog::pages::StudyplanData;
use catalog::timetable::export::{self, same_subscription};
use catalog::timetable::ics;
use catalog::timetable::model::Timetable;
use catalog::timetable::select::Selection;
use catalog::timetable::semester::SemesterKey;
use catalog::timetable::subscription::{self, Subscription};
use leptos::prelude::*;

use super::PlanCtx;
use crate::data::DataError;
use crate::nav;
use crate::ui::Icon;

/// The id of the ways to subscribe, which „Abonnieren" opens.
const WAYS_ID: &str = "sp-sub";

/// The id of „Abonnieren", which takes the focus from a note that goes once it is answered.
const ABO_ID: &str = "sp-abo";

/// How long the plan is left alone before its file is written anew: longer than the gap between
/// two clicks of a burst.
const QUIET: std::time::Duration = std::time::Duration::from_millis(300);

/// What the semester can be subscribed as.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Offer {
    /// The code of the address.
    Code(String),
    /// More is hidden or chosen than one address carries (`subscription::MAX_CODE`).
    TooLong,
    /// Nothing a feed could serve: no planned module has an id a code can carry.
    Nothing,
}

/// The semester's subscription as the group shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Abo {
    key: SemesterKey,
    offer: Offer,
    /// An address of the semester was handed out, and the plan has moved on from it: the
    /// calendar shows another plan than the page, whatever the page can offer instead.
    stale: bool,
}

/// The subscription of the semester `table` shows, as `selection` hides and chooses and with the
/// modules named as in `program` (`StudyplanData::program`), and whether the address last handed
/// out for it (`subscribed`) still says the same. `None` for a semester without a planned module:
/// it has nothing to export.
///
/// Only a code the server would serve is offered (`Subscription::code`): too long a code is
/// „Zu viel ausgeblendet", and one no reader takes (no module with a numeric id) offers nothing.
/// Stale compares only what the timetable still has (`export::same_subscription`): a Termin QIS
/// removed does not make an address stale, since its feed shows what the page shows. Whether a
/// new address can be offered does not matter: with too much hidden for one, the calendar still
/// shows an older plan.
fn abo_of(table: &Timetable, selection: &Selection, program: Option<&str>, subscribed: &BTreeMap<SemesterKey, String>) -> Option<Abo> {
    if table.modules.is_empty() {
        return None;
    }
    let (current, _) = Subscription::of(table.key, &table.modules, program, selection, Some(table));
    let offer = match current.code() {
        Ok(code) => Offer::Code(code),
        Err(pack::Error::TooLong) => Offer::TooLong,
        Err(_) => Offer::Nothing,
    };
    let stale = subscribed.get(&table.key).is_some_and(|stored| !same_subscription(stored, &current, table));
    Some(Abo { key: table.key, offer, stale })
}

/// The ways to subscribe to one address.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Ways {
    /// `webcal://`: Apple's calendars (and others) subscribe to what this scheme names.
    apple: String,
    google: String,
    outlook: String,
    /// The address as a path of this site, for „Adresse kopieren" (`data-absolute`).
    path: String,
}

/// The ways to subscribe to the code's address on the site at `origin` (`https://betula.app`,
/// whose `host` is `betula.app`), for the calendar named after `label` („WiSe 2026/27").
fn ways(origin: &str, host: &str, code: &str, label: &str) -> Ways {
    let path = subscription::path(code);
    let webcal = format!("webcal://{host}{path}");
    Ways {
        google: format!("https://calendar.google.com/calendar/r?cid={}", component(&webcal)),
        outlook: format!(
            "https://outlook.office.com/calendar/0/addfromweb?url={}&name={}",
            component(&format!("{origin}{path}")),
            component(&format!("Studienplan {label}"))
        ),
        apple: webcal,
        path,
    }
}

/// A text as one component of an address, as JavaScript's `encodeURIComponent` writes it: letters,
/// digits and `-_.!~*'()` stay, every other byte of its UTF-8 is `%HH`.
fn component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The site's origin and host as the browser has them („https://betula.app", „betula.app"). The
/// server renders no plan, so it has none.
fn site() -> Option<(String, String)> {
    #[cfg(feature = "csr")]
    {
        let location = web_sys::window()?.location();
        Some((location.origin().ok()?, location.host().ok()?))
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The file „.ics herunterladen" offers, of the semester `key`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum File {
    /// Not made yet, or there is nothing to make it of (the data failed).
    Waiting,
    /// The semester's calendar as a Blob URL.
    Ready { key: SemesterKey, url: String },
    /// The semester's calendar has no entry: its dates are not published yet (`unpublished`), or
    /// nothing of the plan has a date that is shown.
    Empty { key: SemesterKey, unpublished: bool },
}

/// What the link offers of `file` while the page shows the semester `shown`: the file's address,
/// or the line saying why there is none. Nothing of another semester's file, which the link still
/// holds for a moment after ‹ ›.
fn link_of(file: &File, shown: SemesterKey) -> (Option<String>, Option<&'static str>) {
    match file {
        File::Ready { key, url } if *key == shown => (Some(url.clone()), None),
        File::Empty { key, unpublished: true } if *key == shown => (None, Some("noch keine Termine")),
        File::Empty { key, unpublished: false } if *key == shown => (None, Some("keine Termine")),
        _ => (None, None),
    }
}

/// Whether nothing of the semester is published yet: not one dated teaching row in the whole
/// semester (the head says so as well), and the semester is not a past one, whose dates the data
/// has dropped.
fn unpublished(data: &StudyplanData) -> bool {
    let past = data.meta.current_semester.as_deref().and_then(SemesterKey::parse).is_some_and(|current| data.key < current);
    data.counts.is_empty() && !past
}

/// The calendar text of the semester (`Ok`), or whether its dates are unpublished when it has no
/// entry (`Err`). `None` while the data failed or the timetable is not of its semester.
fn calendar_text(data: &Result<StudyplanData, DataError>, table: &Option<Timetable>) -> Option<(SemesterKey, Result<String, bool>)> {
    let (data, table) = (data.as_ref().ok()?, table.as_ref()?);
    if data.key != table.key {
        return None;
    }
    let calendar = data.calendar(table);
    if calendar.entries.is_empty() {
        return Some((data.key, Err(unpublished(data))));
    }
    Some((data.key, Ok(ics::write(&calendar))))
}

/// The text as a Blob URL of an iCalendar file. Only in the browser.
#[allow(unused_variables)]
fn object_url(text: &str) -> Option<String> {
    #[cfg(feature = "csr")]
    {
        let parts = web_sys::js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("text/calendar;charset=utf-8");
        let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options).ok()?;
        web_sys::Url::create_object_url_with_blob(&blob).ok()
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// Lets the browser forget a Blob URL (and the file behind it).
#[allow(unused_variables)]
fn revoke(url: &str) {
    #[cfg(feature = "csr")]
    {
        let _ = web_sys::Url::revoke_object_url(url);
    }
}

/// „Kalender": „.ics herunterladen", and „Abonnieren" with Apple, Google, Outlook and „Adresse
/// kopieren"; a note when the plan has moved on from the address last handed out, or when more is
/// hidden than an address carries. For a semester without a planned module both are greyed out:
/// the group stays where it is (owner, 2026-09-25: the sidebar holds still).
#[component]
pub(super) fn CalendarGroup(ctx: PlanCtx) -> impl IntoView {
    // ---- the subscription
    let hides = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.clone()));
    let program = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().and_then(|data| data.program.clone())));
    let subscribed = Memo::new(move |_| ctx.plan.map(|plan| plan.with(|doc| doc.subscribed.clone())).unwrap_or_default());
    let abo = Memo::new(move |_| {
        hides.with(|selection| {
            program.with(|program| {
                subscribed.with(|subscribed| ctx.table.with(|table| table.as_ref().and_then(|table| abo_of(table, selection, program.as_deref(), subscribed))))
            })
        })
    });
    let offered = Memo::new(move |_| abo.with(|abo| abo.as_ref().is_some_and(|abo| abo.offer != Offer::Nothing)));
    let too_long = Memo::new(move |_| abo.with(|abo| abo.as_ref().is_some_and(|abo| abo.offer == Offer::TooLong)));
    let stale = Memo::new(move |_| abo.with(|abo| abo.as_ref().is_some_and(|abo| abo.stale)));
    let code = Memo::new(move |_| {
        abo.with(|abo| match abo {
            Some(Abo { offer: Offer::Code(code), .. }) => Some(code.clone()),
            _ => None,
        })
    });
    let ways = Memo::new(move |_| {
        abo.with(|abo| match abo {
            Some(Abo { key, offer: Offer::Code(code), .. }) => site().map(|(origin, host)| ways(&origin, &host, code, &key.label(crate::i18n::locale()))),
            _ => None,
        })
    });
    let path = Memo::new(move |_| code.get().map(|code| subscription::path(&code)));
    // Whether there is a new address to copy, apart from which one: the stale note's button stays
    // while the code changes under it.
    let copyable = Memo::new(move |_| path.with(Option::is_some));
    let open = RwSignal::new(false);
    // The code „Neue Adresse kopieren" copied, which the line after it names while it is the one.
    // The line answers that click: the next step to another semester, view or module lets it go,
    // and so does a change of the plan, which a way back does not bring it back from. Two effects:
    // the code is derived from the address, so one closure must not read both (R16).
    let renewed = RwSignal::new(None::<String>);
    let renewed_shown = Memo::new(move |_| !stale.get() && renewed.with(|renewed| renewed.is_some() && *renewed == code.get()));
    Effect::new(move |ran: Option<()>| {
        ctx.url.track();
        if ran.is_some() && renewed.with_untracked(Option::is_some) {
            renewed.set(None);
        }
    });
    Effect::new(move |_| {
        let now = code.get();
        if renewed.with_untracked(|renewed| renewed.is_some() && *renewed != now) {
            renewed.set(None);
        }
    });

    // Handing the address out keeps its code, before the browser follows the link (no query). A
    // middle click (a new tab) and the context menu („Link kopieren", a long press on a phone)
    // hand it out as well.
    let remember = move || {
        let (Some(plan), Some(code)) = (ctx.plan, code.get_untracked()) else { return };
        if let Some(key) = abo.with_untracked(|abo| abo.as_ref().map(|abo| abo.key)) {
            plan.update(|doc| doc.remember(key, &code));
        }
    };
    let renew = move |_| {
        renewed.set(code.get_untracked());
        remember();
        // The note makes way for a line without a control; „Abonnieren" takes the focus it had.
        request_animation_frame(|| nav::focus_by_id(ABO_ID));
    };
    let show_all = move |_| {
        let (Some(plan), Some(key)) = (ctx.plan, abo.with_untracked(|abo| abo.as_ref().map(|abo| abo.key))) else { return };
        plan.update(|doc| doc.show_all(key));
        request_animation_frame(|| nav::focus_by_id(ABO_ID));
    };

    // ---- the file, made ahead (see the module's head for when). The parts of the data the
    // calendar reads besides the timetable say when to make it; what it is made of is read when
    // it is made.
    let parts = Memo::new(move |_| {
        ctx.data.with(|data| {
            data.as_ref().ok().map(|data| (data.key, data.label.clone(), data.titles(), data.slot_names(), export::snapshot_stamp(&data.meta), unpublished(data)))
        })
    });
    let made = RwSignal::new(File::Waiting);
    // Each change of what the file is made of counts one `turn`; `done` is the turn it was made
    // at. A timer finds its turn still standing only for the last change of a burst.
    let turn = StoredValue::new(0_u64);
    let done = StoredValue::new(0_u64);
    let behind = move || turn.try_get_value() != done.try_get_value();
    // Makes the file of the plan as it is now, unless it is made already.
    let make = move || {
        let Some(mine) = turn.try_get_value() else { return };
        if done.try_get_value() == Some(mine) {
            return;
        }
        let text = ctx.data.with_untracked(|data| ctx.table.with_untracked(|table| calendar_text(data, table)));
        let next = match text {
            Some((key, Ok(text))) => object_url(&text).map_or(File::Waiting, |url| File::Ready { key, url }),
            Some((key, Err(unpublished))) => File::Empty { key, unpublished },
            None => File::Waiting,
        };
        if let Some(File::Ready { url, .. }) = made.try_get_untracked() {
            revoke(&url);
        }
        made.try_set(next);
        done.try_set_value(mine);
    };
    // A semester's first file (the group comes, ‹ ›) follows the frame that shows the semester, so
    // the page is not kept from its first paint; a change within the semester waits for `QUIET`.
    Effect::new(move |seen: Option<Option<SemesterKey>>| {
        let key = parts.with(|parts| parts.as_ref().map(|parts| parts.0));
        ctx.table.track();
        let Some(mine) = turn.try_update_value(|turn| {
            *turn = turn.wrapping_add(1);
            *turn
        }) else {
            return key;
        };
        let later = move || {
            if turn.try_get_value() == Some(mine) {
                make();
            }
        };
        if seen == Some(key) {
            set_timeout(later, QUIET);
        } else {
            nav::after_paint(later);
        }
        key
    });
    on_cleanup(move || {
        if let Some(File::Ready { url, .. }) = made.try_get_untracked() {
            revoke(&url);
        }
    });
    let href = Memo::new(move |_| made.with(|made| link_of(made, ctx.key.get()).0));
    let none = Memo::new(move |_| made.with(|made| link_of(made, ctx.key.get()).1));
    // Named after the semester shown, with a file or without: the link is the same link.
    let name = Memo::new(move |_| format!("studienplan-{}.ics", ctx.key.get().key()));
    let anchor = NodeRef::<leptos::html::A>::new();
    // A click that finds the file behind the plan (a pointer that rested on the link through a
    // change, a click from a script) makes it now and gives the link its address itself: the
    // view sets it only after this handler, and the browser follows the link right after it.
    let follow = move |ev: leptos::ev::MouseEvent| {
        if !behind() {
            return;
        }
        make();
        match (href.get_untracked(), anchor.get_untracked()) {
            (Some(url), Some(anchor)) => {
                let _ = anchor.set_attribute("href", &url);
            }
            _ => ev.prevent_default(),
        }
    };

    view! {
        <div class="fgroup actions">
            <p class="flabel label">"Kalender"</p>
            // Without a file the link has no address: it downloads nothing, rather than
            // the page.
            <a
                class="action"
                node_ref=anchor
                href=move || href.get()
                download=move || name.get()
                aria-disabled=move || none.with(Option::is_some).then_some("true")
                on:pointerenter=move |_| make()
                on:focus=move |_| make()
                on:click=follow
            >
                <Icon name="download"/>
                <span>".ics herunterladen"{move || none.get().map(|text| view! { <small>{text}</small> })}</span>
            </a>
            <button
                class="action"
                type="button"
                id=ABO_ID
                aria-disabled=move || (!offered.get()).then_some("true")
                aria-expanded=move || if open.get() && offered.get() { "true" } else { "false" }
                aria-controls=WAYS_ID
                on:click=move |_| {
                    if offered.get_untracked() {
                        open.update(|open| *open = !*open);
                    }
                }
            >
                <Icon name="calendar-plus"/>
                <span>"Abonnieren"</span>
            </button>
            {move || {
                if !open.get() || !offered.get() {
                    return ().into_any();
                }
                if too_long.get() {
                    return view! {
                        <div class="sp-sub" id=WAYS_ID>
                            <p class="note note-action ask">
                                <span>"Zu viel ausgeblendet für ein Abo."</span>
                                // A stale address's note below has the button already.
                                {move || (!stale.get()).then(|| view! { <button class="mini hit" type="button" on:click=show_all>"Alle einblenden"</button> })}
                            </p>
                        </div>
                    }
                    .into_any();
                }
                let Some(ways) = ways.get() else { return ().into_any() };
                view! {
                    <div class="sp-sub" id=WAYS_ID>
                        <a class="action" href=ways.apple rel="external" on:click=move |_| remember() on:auxclick=move |_| remember() on:contextmenu=move |_| remember()>
                            "Apple Kalender"
                        </a>
                        <a
                            class="action"
                            href=ways.google
                            target="_blank"
                            rel="external noopener"
                            on:click=move |_| remember()
                            on:auxclick=move |_| remember()
                            on:contextmenu=move |_| remember()
                        >
                            "Google Kalender"
                        </a>
                        <a
                            class="action"
                            href=ways.outlook
                            target="_blank"
                            rel="external noopener"
                            on:click=move |_| remember()
                            on:auxclick=move |_| remember()
                            on:contextmenu=move |_| remember()
                        >
                            "Outlook"
                        </a>
                        <button class="action" type="button" data-action="copy-text" data-absolute="" data-text=ways.path on:click=move |_| remember()>
                            <Icon name="copy"/>
                            <span>"Adresse kopieren"</span>
                        </button>
                        <p class="hint">"Die Adresse enthält Semester, Module und Ausgeblendetes; dein Kalender holt Änderungen selbst (Google etwa täglich)."</p>
                    </div>
                }
                .into_any()
            }}
            {move || {
                stale.get().then(|| {
                    view! {
                        <p class="note note-action ask">
                            <span>"Abo veraltet: Plan seitdem geändert"</span>
                            // The way to an address the calendar can follow: a new one, or,
                            // with too much hidden for one, back to all Termine.
                            {move || {
                                copyable.get().then(|| {
                                    view! {
                                        <button class="mini hit" type="button" data-action="copy-text" data-absolute="" data-text=move || path.get() on:click=renew>
                                            "Neue Adresse kopieren"
                                        </button>
                                    }
                                })
                            }}
                            {move || too_long.get().then(|| view! { <button class="mini hit" type="button" on:click=show_all>"Alle einblenden"</button> })}
                        </p>
                    }
                })
            }}
            {move || renewed_shown.get().then(|| view! { <p class="action note-action"><Icon name="check"/><span>"Neue Adresse kopiert"</span></p> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use catalog::labels::Code;
    use catalog::rows::Meta;
    use catalog::rows_detail::EventDate;
    use catalog::timetable::facts::SemesterFacts;
    use catalog::timetable::kind::{kinds_of, Class};
    use catalog::timetable::model::{Attendance, Event, Row};
    use catalog::timetable::occur::Occurrences;
    use catalog::timetable::rowkey::RowKey;

    use super::*;

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    /// An Übung of `module` with one weekly Termin, keyed `id`-`fp`.
    fn event(id: u32, fp: u32, module: &str) -> Event {
        let row = Row {
            key: Some(RowKey { event: id, fp }),
            ord: Some(1),
            date: EventDate {
                semester_key: "2026W".into(),
                semester_label: "WiSe 2026/27".into(),
                event_id: id.to_string(),
                event_number: None,
                event_title: format!("Übung {id}"),
                event_type: Some("Übung".into()),
                group_name: None,
                weekday: Some(1),
                start_time: Some("09:15".into()),
                end_time: Some("10:45".into()),
                rhythm: Some(Code::parse("weekly")),
                rhythm_raw: None,
                first_date: Some("2026-10-12".into()),
                last_date: Some("2027-01-25".into()),
                room: None,
                campus: None,
                instructor: None,
                comment: None,
                source_url: None,
                room_short: None,
            },
            cancelled_dates: None,
            occ: Occurrences::default(),
            from: Some(555),
            to: Some(645),
            option: None,
            hidden: None,
        };
        Event {
            id: id.to_string(),
            number: None,
            title: format!("Übung {id}"),
            type_raw: Some("Übung".into()),
            kinds: kinds_of(Some("Übung")),
            class: Class::Other,
            modules: vec![module.into()],
            tone: 1,
            attendance: Attendance::All,
            chosen: None,
            rows: vec![row],
            hidden: None,
            source_url: None,
        }
    }

    fn table(modules: &[&str], events: Vec<Event>) -> Timetable {
        Timetable {
            key: key("2026W"),
            facts: SemesterFacts::derive(key("2026W"), None, &[]),
            modules: modules.iter().map(|id| id.to_string()).collect(),
            events,
            exams: Vec::new(),
            tracks: BTreeSet::new(),
            town: None,
            town_derived: false,
            clashes: Vec::new(),
            blocked: Vec::new(),
            exam_warnings: Vec::new(),
            place_unknown: Vec::new(),
            without_dates: Vec::new(),
        }
    }

    fn code_of(abo: &Option<Abo>) -> Option<String> {
        match abo {
            Some(Abo { offer: Offer::Code(code), .. }) => Some(code.clone()),
            _ => None,
        }
    }

    #[test]
    fn the_abo_is_stale_once_the_plan_moved_on_from_it() {
        let table = table(&["12104", "12107"], vec![event(148369, 0xa4d12, "12104"), event(149408, 7, "12107")]);
        let nothing = BTreeMap::new();
        let informatik = Some("079-82-2008");
        let first = abo_of(&table, &Selection::default(), informatik, &nothing);
        // Never handed out: nothing to be stale.
        assert!(first.as_ref().is_some_and(|abo| !abo.stale && abo.key == key("2026W")));
        let handed: BTreeMap<SemesterKey, String> = [(key("2026W"), code_of(&first).unwrap())].into_iter().collect();
        assert!(abo_of(&table, &Selection::default(), informatik, &handed).is_some_and(|abo| !abo.stale));
        // An event hidden since: the calendar still shows it.
        let hiding = Selection { hidden_events: [149408].into_iter().collect(), ..Default::default() };
        let now = abo_of(&table, &hiding, informatik, &handed);
        assert!(now.as_ref().is_some_and(|abo| abo.stale));
        assert!(code_of(&now).is_some() && code_of(&now) != code_of(&first));
        // Another program, or none: the calendar names the modules by other abbreviations.
        for program in [Some("C38-82-2024"), None] {
            assert!(abo_of(&table, &Selection::default(), program, &handed).is_some_and(|abo| abo.stale), "{program:?}");
        }
        // Hiding what the timetable does not have changes nothing a calendar shows.
        let elsewhere = Selection { hidden_events: [150001].into_iter().collect(), ..Default::default() };
        assert!(abo_of(&table, &elsewhere, informatik, &handed).is_some_and(|abo| !abo.stale));
        // An address of another semester says nothing about this one.
        let other: BTreeMap<SemesterKey, String> = [(key("2027S"), code_of(&first).unwrap())].into_iter().collect();
        assert!(abo_of(&table, &hiding, informatik, &other).is_some_and(|abo| !abo.stale));
    }

    #[test]
    fn too_much_hidden_is_no_address_and_nothing_planned_no_group() {
        // 300 Termine of events far apart: several characters each in the code.
        let events: Vec<Event> = (0..300).map(|n| event(100_000 + 7 * n, n * 3301 % 0xfffff, "12104")).collect();
        let rows: BTreeSet<RowKey> = events.iter().filter_map(|event| event.rows.first().and_then(|row| row.key)).collect();
        let many = table(&["12104"], events);
        let hiding = Selection { hidden_rows: rows, ..Default::default() };
        assert_eq!(abo_of(&many, &hiding, None, &BTreeMap::new()), Some(Abo { key: key("2026W"), offer: Offer::TooLong, stale: false }));
        // „Alle einblenden": an address again.
        let all = code_of(&abo_of(&many, &Selection::default(), None, &BTreeMap::new()));
        assert!(all.is_some());
        // An address handed out before so much was hidden: the calendar shows another plan, though
        // no new address can be offered.
        let handed: BTreeMap<SemesterKey, String> = all.into_iter().map(|code| (key("2026W"), code)).collect();
        assert_eq!(abo_of(&many, &hiding, None, &handed), Some(Abo { key: key("2026W"), offer: Offer::TooLong, stale: true }));
        // A module no code can carry offers no address; no module, no group.
        assert_eq!(abo_of(&table(&["B-12"], Vec::new()), &Selection::default(), None, &BTreeMap::new()).map(|abo| abo.offer), Some(Offer::Nothing));
        assert_eq!(abo_of(&table(&[], Vec::new()), &Selection::default(), None, &BTreeMap::new()), None);
    }

    #[test]
    fn each_way_carries_the_same_address() {
        let ways = ways("http://127.0.0.1:8181", "127.0.0.1:8181", "b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0", "WiSe 2026/27");
        assert_eq!(ways.path, "/calendar/b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0.ics");
        assert_eq!(ways.apple, "webcal://127.0.0.1:8181/calendar/b3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0.ics");
        assert_eq!(ways.google, "https://calendar.google.com/calendar/r?cid=webcal%3A%2F%2F127.0.0.1%3A8181%2Fcalendar%2Fb3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0.ics");
        assert_eq!(
            ways.outlook,
            "https://outlook.office.com/calendar/0/addfromweb?url=http%3A%2F%2F127.0.0.1%3A8181%2Fcalendar%2Fb3MOclrbw-CLbf8P0dlhmewCCVwqAX4Y41XPC~Uv0.ics&name=Studienplan%20WiSe%202026%2F27"
        );
        // Every character of a code stays as it is, as encodeURIComponent leaves it.
        assert_eq!(component("Az09-_.~"), "Az09-_.~");
        assert_eq!(component("SoSe 2027 · ä&?=#+"), "SoSe%202027%20%C2%B7%20%C3%A4%26%3F%3D%23%2B");
    }

    #[test]
    fn the_link_offers_only_a_file_of_the_semester_shown() {
        let winter = File::Ready { key: key("2026W"), url: "blob:http://127.0.0.1:8181/1".into() };
        assert_eq!(link_of(&winter, key("2026W")), (Some("blob:http://127.0.0.1:8181/1".into()), None));
        // Right after ‹ ›, the winter's file is no file of the summer, nor is the summer's line.
        assert_eq!(link_of(&winter, key("2027S")), (None, None));
        let summer = File::Empty { key: key("2027S"), unpublished: true };
        assert_eq!(link_of(&summer, key("2027S")), (None, Some("noch keine Termine")));
        assert_eq!(link_of(&summer, key("2026W")), (None, None));
        assert_eq!(link_of(&File::Empty { key: key("2026W"), unpublished: false }, key("2026W")), (None, Some("keine Termine")));
        assert_eq!(link_of(&File::Waiting, key("2026W")), (None, None));
    }

    #[test]
    fn a_calendar_without_entries_says_why() {
        let data = |semester: &str| StudyplanData {
            key: key(semester),
            label: key(semester).label(crate::i18n::locale()),
            semester: None,
            meta: Meta { current_semester: Some("2026W".into()), data_changed_at: Some("2026-09-23T12:35:16Z".into()), ..Default::default() },
            ids: vec!["12104".into()],
            modules: Vec::new(),
            missing: vec!["12104".into()],
            schedule: Vec::new(),
            exams: Vec::new(),
            sws: Vec::new(),
            counts: Vec::new(),
            abbrevs: Default::default(),
            program: None,
        };
        // Nothing published for the summer to come; a past summer's dates are gone.
        let summer = data("2027S");
        let table = Some(summer.timetable(&Selection::default()));
        assert_eq!(calendar_text(&Ok(summer), &table), Some((key("2027S"), Err(true))));
        let past = data("2026S");
        let table = Some(past.timetable(&Selection::default()));
        assert_eq!(calendar_text(&Ok(past), &table), Some((key("2026S"), Err(false))));
        // A timetable of another semester, or none: nothing yet.
        assert_eq!(calendar_text(&Ok(data("2026W")), &table), None);
        assert_eq!(calendar_text(&Ok(data("2026W")), &None), None);
    }
}
