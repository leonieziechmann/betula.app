//! The sidebar group „Kalender" (design A.8, D.9): the semester as an .ics file, and as an address
//! a calendar service fetches by itself.
//!
//! The file is made in the browser, from the page's own data and timetable, by the function the
//! feed uses (`StudyplanData::calendar`), so a download and a subscription of the same plan carry
//! the same bytes, and nothing of it reaches the server. It is made ahead, as a Blob URL: the link
//! has it before it is clicked, so the click has nothing left to do (R21). A change of the plan
//! makes it anew after the frame that shows the change (`nav::after_paint`), so a chip or an eye
//! button answers first, whatever a semester's file costs.
//!
//! The subscription is an address, `/calendar/<code>.ics`, whose code carries the semester, the
//! planned modules and what is hidden or chosen (`Subscription`; owner decision 2026-09-24). The
//! page never asks for it (no preview, no prefetch); the calendar service does, from its own
//! servers, and the server makes the feed anew each time. Each way of handing the address out keeps
//! its code in the plan (`PlanDoc::remember`), so the page can say when the plan has moved on from
//! what a calendar shows („Abo veraltet").
//!
//! R16: the subscription is worked out from the timetable and from a memo of what the semester
//! hides (`hides`), which is the timetable's sibling (both are derived from `selection`), never
//! from `selection` or `data` beside the timetable made of them. The file follows the timetable and
//! the data's other parts the same way (`parts`, a sibling of the timetable under `data`).

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
    /// An address of the semester was handed out, and the plan has moved on from it.
    stale: bool,
}

/// The subscription of the semester `table` shows, as `selection` hides and chooses, and whether
/// the address last handed out for it (`subscribed`) still says the same. `None` for a semester
/// without a planned module: it has nothing to export.
///
/// Only a code the server would serve is offered (`Subscription::code`): too long a code is
/// „Zu viel ausgeblendet", and one no reader takes (no module with a numeric id) offers nothing.
/// Stale compares only what the timetable still has (`export::same_subscription`): a Termin QIS
/// removed does not make an address stale, since its feed shows what the page shows.
fn abo_of(table: &Timetable, selection: &Selection, subscribed: &BTreeMap<SemesterKey, String>) -> Option<Abo> {
    if table.modules.is_empty() {
        return None;
    }
    let (current, _) = Subscription::of(table.key, &table.modules, selection, Some(table));
    let offer = match current.code() {
        Ok(code) => Offer::Code(code),
        Err(pack::Error::TooLong) => Offer::TooLong,
        Err(_) => Offer::Nothing,
    };
    let stale = matches!(offer, Offer::Code(_)) && subscribed.get(&table.key).is_some_and(|stored| !same_subscription(stored, &current, table));
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

/// The file „.ics herunterladen" offers.
#[derive(Clone, Debug, PartialEq, Eq)]
enum File {
    /// Not made yet, or there is nothing to make it of (the data failed).
    Waiting,
    /// The semester's calendar as a Blob URL.
    Ready { key: SemesterKey, url: String },
    /// The calendar has no entry: the semester's dates are not published yet (`unpublished`), or
    /// nothing of the plan has a date that is shown.
    Empty { unpublished: bool },
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
/// hidden than an address carries. Nothing for a semester without a planned module.
#[component]
pub(super) fn CalendarGroup(ctx: PlanCtx) -> impl IntoView {
    // ---- the subscription
    let hides = Memo::new(move |_| ctx.selection.with(|(_, selection)| selection.clone()));
    let subscribed = Memo::new(move |_| ctx.plan.map(|plan| plan.with(|doc| doc.subscribed.clone())).unwrap_or_default());
    let abo = Memo::new(move |_| {
        hides.with(|selection| subscribed.with(|subscribed| ctx.table.with(|table| table.as_ref().and_then(|table| abo_of(table, selection, subscribed)))))
    });
    let shown = Memo::new(move |_| abo.with(Option::is_some));
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
            Some(Abo { key, offer: Offer::Code(code), .. }) => site().map(|(origin, host)| ways(&origin, &host, code, &key.label())),
            _ => None,
        })
    });
    let path = Memo::new(move |_| code.get().map(|code| subscription::path(&code)));
    let open = RwSignal::new(false);
    // The code „Neue Adresse kopieren" copied, which the line after it names while it is the one.
    let renewed = RwSignal::new(None::<String>);
    let renewed_shown = Memo::new(move |_| !stale.get() && renewed.with(|renewed| renewed.is_some() && *renewed == code.get()));

    // Handing the address out keeps its code, before the browser follows the link (no query).
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

    // ---- the file, made ahead: at once when the group comes, after the next frame on a change
    // (only the last change of a burst is made). The parts of the data the calendar reads besides
    // the timetable say when to make it; what it is made of is read when it is made.
    let parts = Memo::new(move |_| {
        ctx.data.with(|data| data.as_ref().ok().map(|data| (data.key, data.label.clone(), data.titles(), export::snapshot_stamp(&data.meta), unpublished(data))))
    });
    let made = RwSignal::new(File::Waiting);
    let turn = StoredValue::new(0_u64);
    Effect::new(move |ran: Option<()>| {
        parts.track();
        ctx.table.track();
        let mine = turn.get_value().wrapping_add(1);
        turn.set_value(mine);
        let make = move || {
            if turn.try_get_value() != Some(mine) {
                return;
            }
            let text = ctx.data.with_untracked(|data| ctx.table.with_untracked(|table| calendar_text(data, table)));
            let next = match text {
                Some((key, Ok(text))) => object_url(&text).map_or(File::Waiting, |url| File::Ready { key, url }),
                Some((_, Err(unpublished))) => File::Empty { unpublished },
                None => File::Waiting,
            };
            if let Some(File::Ready { url, .. }) = made.try_get_untracked() {
                revoke(&url);
            }
            made.try_set(next);
        };
        if ran.is_none() {
            make();
        } else {
            nav::after_paint(make);
        }
    });
    on_cleanup(move || {
        if let Some(File::Ready { url, .. }) = made.try_get_untracked() {
            revoke(&url);
        }
    });
    let href = Memo::new(move |_| {
        made.with(|made| match made {
            File::Ready { url, .. } => Some(url.clone()),
            _ => None,
        })
    });
    let name = Memo::new(move |_| {
        made.with(|made| match made {
            File::Ready { key, .. } => Some(format!("studienplan-{}.ics", key.key())),
            _ => None,
        })
    });
    let none = Memo::new(move |_| {
        made.with(|made| match made {
            File::Empty { unpublished: true } => Some("noch keine Termine"),
            File::Empty { unpublished: false } => Some("keine Termine"),
            _ => None,
        })
    });

    move || {
        shown.get().then(|| {
            view! {
                <div class="fgroup actions">
                    <p class="flabel label">"Kalender"</p>
                    // Without a file the link has no address: it downloads nothing, rather than
                    // the page.
                    <a class="action" href=move || href.get() download=move || name.get() aria-disabled=move || none.with(Option::is_some).then_some("true")>
                        <Icon name="download"/>
                        <span>".ics herunterladen"{move || none.get().map(|text| view! { <small>{text}</small> })}</span>
                    </a>
                    {move || {
                        offered.get().then(|| {
                            view! {
                                <button
                                    class="action"
                                    type="button"
                                    id=ABO_ID
                                    aria-expanded=move || if open.get() { "true" } else { "false" }
                                    aria-controls=WAYS_ID
                                    on:click=move |_| open.update(|open| *open = !*open)
                                >
                                    <Icon name="calendar-plus"/>
                                    <span>"Abonnieren"</span>
                                </button>
                            }
                        })
                    }}
                    {move || {
                        if !open.get() || !offered.get() {
                            return ().into_any();
                        }
                        if too_long.get() {
                            return view! {
                                <div class="sp-sub" id=WAYS_ID>
                                    <p class="note note-action ask">
                                        <span>"Zu viel ausgeblendet für ein Abo."</span>
                                        <button class="mini hit" type="button" on:click=show_all>"Alle einblenden"</button>
                                    </p>
                                </div>
                            }
                            .into_any();
                        }
                        let Some(ways) = ways.get() else { return ().into_any() };
                        view! {
                            <div class="sp-sub" id=WAYS_ID>
                                <a class="action" href=ways.apple rel="external" on:click=move |_| remember()>"Apple Kalender"</a>
                                <a class="action" href=ways.google target="_blank" rel="external noopener" on:click=move |_| remember()>"Google Kalender"</a>
                                <a class="action" href=ways.outlook target="_blank" rel="external noopener" on:click=move |_| remember()>"Outlook"</a>
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
                                    <button class="mini hit" type="button" data-action="copy-text" data-absolute="" data-text=move || path.get() on:click=renew>
                                        "Neue Adresse kopieren"
                                    </button>
                                </p>
                            }
                        })
                    }}
                    {move || renewed_shown.get().then(|| view! { <p class="action note-action"><Icon name="check"/><span>"Neue Adresse kopiert"</span></p> })}
                </div>
            }
        })
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
        let first = abo_of(&table, &Selection::default(), &nothing);
        // Never handed out: nothing to be stale.
        assert!(first.as_ref().is_some_and(|abo| !abo.stale && abo.key == key("2026W")));
        let handed: BTreeMap<SemesterKey, String> = [(key("2026W"), code_of(&first).unwrap())].into_iter().collect();
        assert!(abo_of(&table, &Selection::default(), &handed).is_some_and(|abo| !abo.stale));
        // An event hidden since: the calendar still shows it.
        let hiding = Selection { hidden_events: [149408].into_iter().collect(), ..Default::default() };
        let now = abo_of(&table, &hiding, &handed);
        assert!(now.as_ref().is_some_and(|abo| abo.stale));
        assert!(code_of(&now).is_some() && code_of(&now) != code_of(&first));
        // Hiding what the timetable does not have changes nothing a calendar shows.
        let elsewhere = Selection { hidden_events: [150001].into_iter().collect(), ..Default::default() };
        assert!(abo_of(&table, &elsewhere, &handed).is_some_and(|abo| !abo.stale));
        // An address of another semester says nothing about this one.
        let other: BTreeMap<SemesterKey, String> = [(key("2027S"), code_of(&first).unwrap())].into_iter().collect();
        assert!(abo_of(&table, &hiding, &other).is_some_and(|abo| !abo.stale));
    }

    #[test]
    fn too_much_hidden_is_no_address_and_nothing_planned_no_group() {
        // 300 Termine of events far apart: several characters each in the code.
        let events: Vec<Event> = (0..300).map(|n| event(100_000 + 7 * n, n * 3301 % 0xfffff, "12104")).collect();
        let rows: BTreeSet<RowKey> = events.iter().filter_map(|event| event.rows.first().and_then(|row| row.key)).collect();
        let many = table(&["12104"], events);
        let hiding = Selection { hidden_rows: rows, ..Default::default() };
        assert_eq!(abo_of(&many, &hiding, &BTreeMap::new()).map(|abo| abo.offer), Some(Offer::TooLong));
        // „Alle einblenden": an address again.
        assert!(code_of(&abo_of(&many, &Selection::default(), &BTreeMap::new())).is_some());
        // A module no code can carry offers no address; no module, no group.
        assert_eq!(abo_of(&table(&["B-12"], Vec::new()), &Selection::default(), &BTreeMap::new()).map(|abo| abo.offer), Some(Offer::Nothing));
        assert_eq!(abo_of(&table(&[], Vec::new()), &Selection::default(), &BTreeMap::new()), None);
    }

    #[test]
    fn each_way_carries_the_same_address() {
        let ways = ways("http://127.0.0.1:8181", "127.0.0.1:8181", "CQpJeFAKchJKBgdlgf0e7Hwl_4S", "WiSe 2026/27");
        assert_eq!(ways.path, "/calendar/CQpJeFAKchJKBgdlgf0e7Hwl_4S.ics");
        assert_eq!(ways.apple, "webcal://127.0.0.1:8181/calendar/CQpJeFAKchJKBgdlgf0e7Hwl_4S.ics");
        assert_eq!(ways.google, "https://calendar.google.com/calendar/r?cid=webcal%3A%2F%2F127.0.0.1%3A8181%2Fcalendar%2FCQpJeFAKchJKBgdlgf0e7Hwl_4S.ics");
        assert_eq!(
            ways.outlook,
            "https://outlook.office.com/calendar/0/addfromweb?url=http%3A%2F%2F127.0.0.1%3A8181%2Fcalendar%2FCQpJeFAKchJKBgdlgf0e7Hwl_4S.ics&name=Studienplan%20WiSe%202026%2F27"
        );
        // Every character of a code stays as it is, as encodeURIComponent leaves it.
        assert_eq!(component("Az09-_.~"), "Az09-_.~");
        assert_eq!(component("SoSe 2027 · ä&?=#+"), "SoSe%202027%20%C2%B7%20%C3%A4%26%3F%3D%23%2B");
    }

    #[test]
    fn a_calendar_without_entries_says_why() {
        let data = |semester: &str| StudyplanData {
            key: key(semester),
            label: key(semester).label(),
            semester: None,
            meta: Meta { current_semester: Some("2026W".into()), data_changed_at: Some("2026-09-23T12:35:16Z".into()), ..Default::default() },
            ids: vec!["12104".into()],
            modules: Vec::new(),
            missing: vec!["12104".into()],
            schedule: Vec::new(),
            exams: Vec::new(),
            sws: Vec::new(),
            counts: Vec::new(),
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
