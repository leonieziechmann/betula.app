//! „Stundenplan teilen" (owner, 2026-09-26): on the page a link opens, the offer to take over the
//! semester it hands on. The link is made by the dialog „Teilen" (`share_dialog.rs`): for everybody
//! with it as a copy of now, `/studyplan?share=<code>` (`folia_calendar::share`), or through Betula
//! Sync (below).
//!
//! The link is the visitor's own act, and the exception R20 makes for it is what it carries: the
//! semester, the planned modules in their order, the timetable's program — so that the preview a
//! messenger shows of it names the modules („MIT-1, AuP, EEG"), which the messenger can read from
//! the address alone — the groups ticked and the changes of Termine of QIS, so that whoever opens
//! it can put the plan beside their own as a calendar with its owner's groups and Termine as they
//! changed them (`calendars.rs`). Nothing of what is hidden, no note and no Termin of one's own
//! travels.
//!
//! The offer stands at the top of the plan while the address carries a code, as the Merkliste
//! offers a list from a link: „Übernehmen" plans the modules the plan does not have yet into the
//! semester, after the next frame (R21), with the program where the plan has none; „Als Kalender"
//! puts the plan beside one's own as a calendar, with its groups (`calendars.rs`; a plan of another
//! semester too, which then stands in the week once the page shows that semester); „Verwerfen"
//! leaves the plan as it is. Each takes the code out of the address, in the same history entry.
//! Where the offer stood, „Aus dem Link übernommen: 6 Module" or „Als Kalender angelegt: Kalender
//! 2" then answers with „Rückgängig" (`PlanCtx::undo`, which the import and „Plan leeren" share)
//! until the next change of the kind. A code of another semester than the one the page shows is
//! otherwise only named: the page has no other.
//!
//! The link handed on is in the page's language (`/en/studyplan?share=…`), so that whoever opens it
//! reads the offer in the language it was handed on in.
//!
//! A calendar shared to stay current (Betula Sync, step 3; folia/design/teilen/teilen.html) comes as
//! `/studyplan#live=<part>` or, for one person, `/studyplan#once=<parcel>`: what follows `#` never
//! reaches a server. Its offer (`LiveOffer`) stands where this one does and says the same things,
//! with „Als Kalender folgen"; a link that opens nothing any more says why.

use folia_calendar::live::LivePart;
use folia_calendar::share::SharedPlan;
use folia_pages::ask::SharedPlanAsk;
use folia_pages::SharedPlanData;
use folia_stores::sync::share::{opening, Opening, ShareProblem};
use folia_stores::sync::{Opened, Sync};
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n;
use folia_shell::pending::Pending;
use folia_design::ui::Icon;
use super::{now_secs, PlanCtx};

/// What the offer says of a shared plan.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Offer {
    /// Its modules, `new` of them not planned yet.
    Take { names: String, count: usize, new: usize },
    /// Every module of it is planned already.
    Held { names: String },
    /// A plan of another semester than the one the page shows.
    Elsewhere { names: String },
    /// The catalog knows none of its modules (any more).
    Unknown,
}

/// What the offer says of `data`: on a page of its semester (`same_semester`), whose plan holds
/// `planned` there, or of another.
fn offer_of(data: &SharedPlanData, same_semester: bool, planned: &[String]) -> Offer {
    if data.modules.is_empty() {
        return Offer::Unknown;
    }
    let names = data.modules.iter().map(|module| module.name.as_str()).collect::<Vec<_>>().join(", ");
    if !same_semester {
        return Offer::Elsewhere { names };
    }
    let new = data.modules.iter().filter(|module| !planned.contains(&module.id)).count();
    match new {
        0 => Offer::Held { names },
        new => Offer::Take { names, count: data.modules.len(), new },
    }
}

/// The offer above the plan while the address carries a shared plan (`share=`).
#[component]
pub(super) fn ShareOffer(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.studyplan_share;
    let code = Memo::new(move |_| ctx.url.with(|url| url.share.clone()));
    // What the code names, from the local copy of the catalog.
    let shared = Memo::new(move |before| {
        let plan = code.with(|code| code.as_deref().and_then(SharedPlan::from_code))?;
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&SharedPlanAsk { plan: plan.clone(), locale: t.locale })))?;
        folia_pages::ask::unless_pending(now, before, |now| Some((plan, now.ok()??)))
    });
    // The offer and the semester it names, together: the view read `shared` for the label after
    // `offer` and kept the label before when a new plan came with the same offer (R16).
    let offer = Memo::new(move |_| {
        let page = ctx.key.get();
        shared.with(|shared| {
            let (plan, data) = shared.as_ref()?;
            let theirs = plan.key()?;
            let planned = ctx.plan.map(|plan| plan.modules_in(theirs)).unwrap_or_default();
            Some((offer_of(data, theirs == page, &planned), data.label.clone()))
        })
    });
    let busy = RwSignal::new(false);

    // The same page without the code, in the same history entry. The context is taken here: the
    // write after the next frame runs outside the page's owner.
    let going = Pending::expect();
    let leave = move || {
        let away = ctx.url.with_untracked(|url| url.without_share().path());
        if let Some(going) = going {
            going.go(&away, NavigateOptions { replace: true, scroll: false, ..Default::default() });
        }
    };
    let take = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some((plan, data)) = shared.get_untracked() else { return };
        let Some(key) = plan.key() else { return };
        busy.set(true);
        let ids: Vec<String> = data.modules.iter().map(|module| module.id.clone()).collect();
        let program = plan.program.clone();
        let (store, undo) = (ctx.plan, ctx.undo);
        folia_design::nav::after_paint(move || {
            if let Some(store) = store {
                let note = store.update(|doc| {
                    let before = doc.clone();
                    let at = now_secs();
                    let added = ids.iter().filter(|id| doc.plan(key, id, at, None)).count();
                    if doc.program.is_none() {
                        doc.program = program;
                    }
                    (format!("{}{}", s.taken, taken_count(added, t)), before)
                });
                let _ = undo.try_set(Some(note));
            }
            leave();
            let _ = busy.try_set(false);
        });
    };
    let dismiss = move |_| leave();
    // „Als Kalender": the plan of the link beside one's own, not in it (`calendars.rs`), with the
    // groups it ticked; „Rückgängig" puts the plan back as it was. The eighth calendar is the last.
    let made = RwSignal::new(None::<String>);
    let room = Memo::new(move |_| ctx.calendars.with(Vec::len) < folia_plans::studyplan::MAX_CALENDARS);
    let as_calendar = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some((plan, _)) = shared.get_untracked() else { return };
        busy.set(true);
        let (store, undo) = (ctx.plan, ctx.undo);
        folia_design::nav::after_paint(move || {
            if let Some(store) = store {
                let before = store.with(Clone::clone);
                if let Some(id) = super::calendars::create(ctx, &plan, "") {
                    let note = (t.studyplan_calendars.created)(&(t.studyplan_calendars.unnamed)(id));
                    let _ = made.try_set(Some(note.clone()));
                    let _ = undo.try_set(Some((note, before)));
                }
            }
            leave();
            let _ = busy.try_set(false);
        });
    };

    // Once taken over or made a calendar: the note where the offer stood, until the next change of
    // the kind.
    let taken = Memo::new(move |_| {
        ctx.undo.with(|undo| undo.as_ref().map(|(note, _)| note.clone()).filter(|note| note.starts_with(s.taken) || made.with(|made| made.as_ref() == Some(note))))
    });
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        let (Some(store), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        if restoring.get_untracked() {
            return;
        }
        restoring.set(true);
        let undo = ctx.undo;
        store.update_after_paint(move |doc| {
            *doc = before;
            let _ = undo.try_set(None);
            let _ = restoring.try_set(false);
        });
    };

    move || {
        let Some((offer, label)) = offer.get() else {
            return taken.get().map(|note| {
                view! {
                    <div class="offer" role="status">
                        <Icon name="check"/>
                        <p><b>{note}</b></p>
                        <button class="mini hit" type="button" id="sp-share-undo" aria-busy=move || restoring.get().then_some("true") on:click=restore>{t.common.undo}</button>
                    </div>
                }
                .into_any()
            });
        };
        // A calendar of a plan whose modules the catalog knows.
        let known = !matches!(offer, Offer::Unknown);
        let (text, can_take) = offer_text(offer, t);
        Some(view! {
            <div class="offer" role="status">
                <Icon name="share-2"/>
                <p><b>{(s.shared)(&label)}</b>{text}</p>
                {can_take.then(|| view! {
                    <button class="mini primary hit" type="button" id="sp-share-take" aria-busy=move || busy.get().then_some("true") on:click=take>{s.take}</button>
                })}
                {move || (known && room.get()).then(|| view! {
                    <button class="mini hit" type="button" id="sp-share-calendar" title=t.studyplan_calendars.as_calendar_title aria-busy=move || busy.get().then_some("true") on:click=as_calendar>
                        {t.studyplan_calendars.as_calendar}
                    </button>
                })}
                <button class="mini hit" type="button" id="sp-share-dismiss" on:click=dismiss>{if can_take { s.dismiss } else { s.ok }}</button>
            </div>
        }
        .into_any())
    }
}

/// What the offer says after its first words, and whether it offers „Übernehmen".
fn offer_text(offer: Offer, t: &i18n::Texts) -> (String, bool) {
    let s = &t.studyplan_share;
    match offer {
        Offer::Take { names, count, new } => {
            let question = if new == count { s.take_all.to_string() } else { (s.take_missing)(new) };
            (format!("{}: {names}. {question}", folia_design::format::modules(i64::try_from(count).unwrap_or(i64::MAX), t.locale)), true)
        }
        Offer::Held { names } => ((s.held)(&names), false),
        Offer::Elsewhere { names } => ((s.elsewhere)(&names), false),
        Offer::Unknown => (s.unknown.to_string(), false),
    }
}

/// „6 Module", „nichts": what the note of a plan taken over from a link says was added.
fn taken_count(modules: usize, t: &i18n::Texts) -> String {
    match modules {
        0 => t.studyplan_share.nothing.to_string(),
        n => folia_design::format::modules(i64::try_from(n).unwrap_or(i64::MAX), t.locale),
    }
}

// ---------- a calendar that stays current, and a one-time link ----------

/// What the part of the address after `#` opened to (`/studyplan#live=…`, `/studyplan#once=…`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Found {
    /// The part or the parcel is on its way.
    Opening,
    /// A calendar that stays current: its link (the part's id and key) and what it shows now.
    Live { link: String, part: LivePart },
    /// A copy of then, from a one-time link.
    Copy(LivePart),
    /// It opens nothing (any more), and why.
    Problem(ShareProblem),
    /// The site does not offer Betula Sync, whose service keeps what is shared.
    NotOffered,
}

/// What a one-time link opened to, kept for this tab (`sessionStorage`): its parcel opens once, so
/// a reload, or the page built anew, finds it here rather than at Phloem, which would say it was
/// opened already. Taken away once the offer is answered.
const OPENED_KEY: &str = "betula.share.opened";

thread_local! {
    /// The fragment being opened and what it opened to, for as long as the tab lives: the page built
    /// anew while the parcel is on its way waits for the same answer instead of asking again.
    static OPENED: std::cell::RefCell<Option<(String, ArcRwSignal<Option<Found>>)>> = const { std::cell::RefCell::new(None) };
}

/// What `OPENED_KEY` keeps of a parcel: the link of a calendar that stays current, or a copy.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Kept {
    Live(String),
    Copy(Box<LivePart>),
}

/// Its text: the parcel's link, then what it opened to.
fn kept_text(parcel: &str, kept: &Kept) -> String {
    match kept {
        Kept::Live(link) => format!("{parcel}\tlive\t{link}"),
        Kept::Copy(part) => format!("{parcel}\tcopy\t{}", part.code().unwrap_or_default()),
    }
}

/// What `kept_text` wrote of parcel `parcel`; `None` for another parcel or a text that is none.
fn kept_of(text: &str, parcel: &str) -> Option<Kept> {
    let mut fields = text.splitn(3, '\t');
    if fields.next()? != parcel {
        return None;
    }
    match (fields.next()?, fields.next()?) {
        ("live", link) if folia_plans::shares::split_link(link).is_some() => Some(Kept::Live(link.to_string())),
        ("copy", code) => LivePart::from_code(code).map(|part| Kept::Copy(Box::new(part))),
        _ => None,
    }
}

/// The address without its part after `#`, in place (the history entry keeps its state): what was
/// opened is not opened again by a reload.
fn drop_fragment() {
    #[cfg(feature = "csr")]
    if let Some(window) = web_sys::window() {
        let location = window.location();
        if location.hash().is_ok_and(|hash| !hash.is_empty()) {
            let rest = format!("{}{}", location.pathname().unwrap_or_default(), location.search().unwrap_or_default());
            if let Ok(history) = window.history() {
                let state = history.state().unwrap_or(wasm_bindgen::JsValue::NULL);
                let _ = history.replace_state_with_url(&state, "", Some(&rest));
            }
        }
    }
}

/// What the offer says where a link opens nothing (owner, 2026-11-08: „man soll nicht an die daten
/// kommen danach. Aber es sollte schon eine fehlermeldung geben mit link abgelaufen oder so").
fn problem_text(problem: ShareProblem, t: &i18n::Texts) -> &'static str {
    let s = &t.studyplan_share;
    match problem {
        ShareProblem::Taken => s.link_taken,
        ShareProblem::Expired => s.link_expired,
        ShareProblem::Withdrawn => s.link_withdrawn,
        ShareProblem::Ended => s.link_ended,
        ShareProblem::Unknown | ShareProblem::NotSynced => s.link_unknown,
        ShareProblem::Busy | ShareProblem::TooMany => s.link_busy,
        ShareProblem::Unreachable => s.link_unreachable,
        ShareProblem::Unsupported => s.link_unsupported,
    }
}

/// Opens what the fragment names, once per tab, into the signal the offer reads.
fn open_fragment(opening: Opening, found: ArcRwSignal<Option<Found>>) {
    let Some(sync) = Sync::expect().filter(|_| folia_stores::sync::offered().is_some()) else {
        found.set(Some(Found::NotOffered));
        return;
    };
    let live = |sync: Sync, link: String, found: ArcRwSignal<Option<Found>>| {
        let opened = link.clone();
        sync.open_live(&opened, move |result| {
            found.set(Some(match result {
                Ok((part, _)) => Found::Live { link, part },
                Err(problem) => Found::Problem(problem),
            }));
        });
    };
    match opening {
        Opening::Live(link) => live(sync, link, found),
        Opening::Once(parcel) => match folia_design::nav::session_get(OPENED_KEY).and_then(|text| kept_of(&text, &parcel)) {
            Some(Kept::Live(link)) => live(sync, link, found),
            Some(Kept::Copy(part)) => found.set(Some(Found::Copy(*part))),
            None => {
                let link = parcel.clone();
                sync.open_once(&link, move |result| {
                    found.set(Some(match result {
                        Ok(Opened::Live { link, part, .. }) => {
                            folia_design::nav::session_set(OPENED_KEY, &kept_text(&parcel, &Kept::Live(link.clone())));
                            Found::Live { link, part }
                        }
                        Ok(Opened::Copy(part)) => {
                            folia_design::nav::session_set(OPENED_KEY, &kept_text(&parcel, &Kept::Copy(Box::new(part.clone()))));
                            Found::Copy(part)
                        }
                        Err(problem) => Found::Problem(problem),
                    }));
                });
            }
        },
    }
}

/// How the offer to save the profile with Betula Sync stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Upgrade {
    Offered,
    Saving,
    Saved,
    Failed,
}

/// The offer of a link that names a calendar that stays current, or of a one-time link
/// (`/studyplan#live=…`, `/studyplan#once=…`; folia/design/teilen/teilen.html, „Was Lena sieht").
///
/// It stands where the offer of a fixed plan stands (`ShareOffer`), with the same words for the
/// same things: „Als Kalender folgen" puts the plan beside one's own as a calendar that follows
/// the share (`PlanDoc::follow`), so that every change of it arrives until it is not shared any
/// more; „Übernehmen" plans its modules; „Verwerfen" leaves the plan as it is. A one-time link of a
/// copy offers „Als Kalender" without following. What opens nothing says why: opened already,
/// expired, taken back, not shared any more, mistyped. Each answer takes the part after `#` out of
/// the address. Where the profile is private, the calendar follows from this browser alone, and
/// beside it stands what Betula Sync makes better, with „Profil speichern" (owner, 2026-11-08).
///
/// It also keeps the calendars that follow a share current where nothing else does: when the page
/// is built and when it comes into view again (`Sync::refresh_follows`).
#[component]
pub(super) fn LiveOffer(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.studyplan_share;
    let c = &t.studyplan_calendars;

    // What follows a share is fetched anew when the page comes and when it is seen again.
    #[cfg(feature = "csr")]
    if let Some(sync) = Sync::expect() {
        sync.refresh_follows();
        let seen = window_event_listener_untyped("visibilitychange", move |_| {
            if web_sys::window().and_then(|w| w.document()).is_some_and(|d| !d.hidden()) {
                sync.refresh_follows();
            }
        });
        on_cleanup(move || seen.remove());
    }

    // What the fragment names, opened once per tab.
    let fragment = folia_design::nav::fragment();
    let arc = match opening(&fragment) {
        None => ArcRwSignal::new(None),
        Some(what) => {
            let kept = OPENED.with(|opened| opened.borrow().as_ref().filter(|(of, _)| *of == fragment).map(|(_, found)| found.clone()));
            match kept {
                Some(found) => found,
                None => {
                    let found = ArcRwSignal::new(Some(Found::Opening));
                    OPENED.with(|opened| *opened.borrow_mut() = Some((fragment.clone(), found.clone())));
                    open_fragment(what, found.clone());
                    found
                }
            }
        }
    };
    let found = RwSignal::from(arc);

    // What the part names, from the local copy of the catalog: the semester's name and the
    // modules' short names, as for a fixed plan.
    let shared = Memo::new(move |before| {
        let part = found.with(|found| match found {
            Some(Found::Live { part, .. } | Found::Copy(part)) => Some(part.clone()),
            _ => None,
        })?;
        let plan = SharedPlan { semester: part.semester, modules: part.modules.clone(), program: part.program.clone(), chosen: Vec::new(), changes: Vec::new() };
        let now = ctx.source.with_value(|source| source.as_ref().map(|source| source.now(&SharedPlanAsk { plan, locale: t.locale })))?;
        folia_pages::ask::unless_pending(now, before, |now| Some((part, now.ok()??)))
    });
    let offer = Memo::new(move |_| {
        let page = ctx.key.get();
        shared.with(|shared| {
            let (part, data) = shared.as_ref()?;
            let theirs = part.key()?;
            let planned = ctx.plan.map(|plan| plan.modules_in(theirs)).unwrap_or_default();
            // A plan emptied is a calendar to follow too: Lena sees it again once it holds modules.
            let offer = if part.modules.is_empty() { None } else { Some(offer_of(data, theirs == page, &planned)) };
            Some((offer, data.label.clone()))
        })
    });
    let room = Memo::new(move |_| ctx.calendars.with(Vec::len) < folia_plans::studyplan::MAX_CALENDARS);
    let busy = RwSignal::new(false);
    let made = RwSignal::new(None::<String>);
    // Whether the calendar made follows the share (a copy does not: Betula Sync adds nothing to it).
    let follows = RwSignal::new(false);
    let upgrade = RwSignal::new(Upgrade::Offered);

    // Answered: the fragment goes from the address, what the tab kept of it too.
    let done = move || {
        drop_fragment();
        folia_design::nav::session_set(OPENED_KEY, "");
        OPENED.with(|opened| *opened.borrow_mut() = None);
        found.set(None);
    };
    // „Als Kalender folgen" (with the link) and „Als Kalender" (a copy): a calendar of one's own
    // with what the part holds, after the next frame (R21); „Rückgängig" puts the plan back.
    let as_calendar = move |link: Option<String>| {
        if busy.get_untracked() {
            return;
        }
        let Some(part) = found.with_untracked(|found| match found {
            Some(Found::Live { part, .. } | Found::Copy(part)) => Some(part.clone()),
            _ => None,
        }) else {
            return;
        };
        let Some(key) = part.key() else { return };
        busy.set(true);
        let (store, undo) = (ctx.plan, ctx.undo);
        folia_design::nav::after_paint(move || {
            if let Some(store) = store {
                let before = store.with_untracked(Clone::clone);
                let id = store.update(|doc| {
                    let id = doc.add_calendar("", part.program.as_deref(), key, &[], &[], &[])?;
                    if let Some(link) = &link {
                        doc.follow(id, link, now_secs());
                    }
                    doc.take_part(id, &part);
                    Some(id)
                });
                if let Some(id) = id {
                    let note = (c.created)(&(c.unnamed)(id));
                    let _ = follows.try_set(link.is_some());
                    let _ = made.try_set(Some(note.clone()));
                    let _ = undo.try_set(Some((note, before)));
                }
            }
            done();
            let _ = busy.try_set(false);
        });
    };
    // „Übernehmen": the modules the catalog knows, planned into the part's semester.
    let take = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some((part, data)) = shared.get_untracked() else { return };
        let Some(key) = part.key() else { return };
        busy.set(true);
        let ids: Vec<String> = data.modules.iter().map(|module| module.id.clone()).collect();
        let program = part.program.clone();
        let (store, undo) = (ctx.plan, ctx.undo);
        folia_design::nav::after_paint(move || {
            if let Some(store) = store {
                let note = store.update(|doc| {
                    let before = doc.clone();
                    let at = now_secs();
                    let added = ids.iter().filter(|id| doc.plan(key, id, at, None)).count();
                    if doc.program.is_none() {
                        doc.program = program;
                    }
                    (format!("{}{}", s.live_taken, taken_count(added, t)), before)
                });
                let _ = undo.try_set(Some(note));
            }
            done();
            let _ = busy.try_set(false);
        });
    };

    // The note after „Übernehmen" or „Als Kalender", until the next change of the kind.
    let taken = Memo::new(move |_| {
        ctx.undo.with(|undo| undo.as_ref().map(|(note, _)| note.clone()).filter(|note| note.starts_with(s.live_taken) || made.with(|made| made.as_ref() == Some(note))))
    });
    let followed = Memo::new(move |_| follows.get() && taken.with(|taken| taken.is_some() && *taken == made.get()));
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        let (Some(store), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        if restoring.get_untracked() {
            return;
        }
        restoring.set(true);
        let undo = ctx.undo;
        store.update_after_paint(move |doc| {
            *doc = before;
            let _ = undo.try_set(None);
            let _ = restoring.try_set(false);
        });
    };
    // A private profile follows from this browser alone: Betula Sync brings every change at once,
    // and the calendar to every device (owner, 2026-11-08: „schnell sein profil speichern").
    let profile = ctx.plan.map(folia_stores::studyplan::Studyplan::profile);
    let private = Memo::new(move |_| {
        let (Some(sync), Some(profile)) = (Sync::expect(), profile) else { return false };
        folia_stores::sync::offered().is_some() && !sync.state(profile).synced()
    });
    let save = move |_| {
        let (Some(sync), Some(profile)) = (Sync::expect(), profile) else { return };
        if upgrade.get_untracked() == Upgrade::Saving {
            return;
        }
        upgrade.set(Upgrade::Saving);
        sync.activate_then(profile, move |result| {
            let _ = upgrade.try_set(if result.is_ok() { Upgrade::Saved } else { Upgrade::Failed });
        });
    };
    let upgrade_view = move || {
        let state = upgrade.get();
        (followed.get() && (private.get() || state == Upgrade::Saved)).then(|| match state {
            Upgrade::Saved => view! {
                <div class="offer" role="status"><Icon name="check"/><p>{s.upgrade_saved}</p></div>
            }
            .into_any(),
            _ => view! {
                <div class="offer">
                    <Icon name="monitor-smartphone"/>
                    <p><b>{s.upgrade}</b>" "{if state == Upgrade::Failed { s.upgrade_failed } else { s.upgrade_small }}</p>
                    <button class="mini hit" type="button" id="sp-live-save" aria-busy=move || (upgrade.get() == Upgrade::Saving).then_some("true") on:click=save>
                        {move || if upgrade.get() == Upgrade::Saving { s.upgrade_saving } else { s.upgrade_save }}
                    </button>
                </div>
            }
            .into_any(),
        })
    };

    move || {
        let Some(now) = found.get() else {
            return taken
                .get()
                .map(|note| {
                    view! {
                        <div class="offer" role="status">
                            <Icon name="check"/>
                            <p><b>{note}</b></p>
                            <button class="mini hit" type="button" id="sp-live-undo" aria-busy=move || restoring.get().then_some("true") on:click=restore>{t.common.undo}</button>
                        </div>
                        {upgrade_view}
                    }
                })
                .into_any();
        };
        match now {
            Found::Opening => view! {
                <div class="offer" role="status" aria-busy="true"><Icon name="share-2"/><p>{s.link_opening}</p></div>
            }
            .into_any(),
            Found::NotOffered | Found::Problem(_) => {
                let text = match now {
                    Found::Problem(problem) => problem_text(problem, t),
                    _ => s.link_not_offered,
                };
                view! {
                    <div class="offer" role="alert">
                        <Icon name="triangle-alert"/>
                        <p>{text}</p>
                        <button class="mini hit" type="button" id="sp-live-ok" on:click=move |_| done()>{s.ok}</button>
                    </div>
                }
                .into_any()
            }
            Found::Live { link, .. } => {
                let live = link.clone();
                let follow = move |_| as_calendar(Some(live.clone()));
                view! {
                    <div class="offer" role="status">
                        <Icon name="share-2"/>
                        {move || offer_line(offer.get(), true, t)}
                        {move || match room.get() {
                            true => {
                                let follow = follow.clone();
                                view! {
                                    <button class="mini primary hit" type="button" id="sp-live-follow" title=s.follow_title aria-busy=move || busy.get().then_some("true") on:click=follow>{s.follow}</button>
                                }
                                .into_any()
                            }
                            false => view! { <span class="hint">{c.full_up}</span> }.into_any(),
                        }}
                        {move || can_take(offer.get()).then(|| view! {
                            <button class="mini hit" type="button" id="sp-live-take" aria-busy=move || busy.get().then_some("true") on:click=take>{s.take}</button>
                        })}
                        <button class="mini hit" type="button" id="sp-live-dismiss" on:click=move |_| done()>{s.dismiss}</button>
                    </div>
                }
                .into_any()
            }
            Found::Copy(_) => view! {
                <div class="offer" role="status">
                    <Icon name="share-2"/>
                    {move || offer_line(offer.get(), false, t)}
                    {move || can_take(offer.get()).then(|| view! {
                        <button class="mini primary hit" type="button" id="sp-live-take" aria-busy=move || busy.get().then_some("true") on:click=take>{s.take}</button>
                    })}
                    {move || match room.get() {
                        true => view! {
                            <button class="mini hit" type="button" id="sp-live-calendar" title=c.as_calendar_title aria-busy=move || busy.get().then_some("true") on:click=move |_| as_calendar(None)>
                                {c.as_calendar}
                            </button>
                        }
                        .into_any(),
                        false => view! { <span class="hint">{c.full_up}</span> }.into_any(),
                    }}
                    <button class="mini hit" type="button" id="sp-live-dismiss" on:click=move |_| done()>{s.dismiss}</button>
                </div>
            }
            .into_any(),
        }
    }
}

/// The offer's words: „Geteilter Stundenplan, bleibt aktuell · WiSe 2026/27." (`live`) or the one
/// of a copy, then what it holds; while the catalog answers, the first words alone.
fn offer_line(offer: Option<(Option<Offer>, String)>, live: bool, t: &'static i18n::Texts) -> impl IntoView {
    let s = &t.studyplan_share;
    let Some((offer, label)) = offer else {
        return view! { <p><b>{if live { s.live_opened } else { s.copy_opened }}</b></p> }.into_any();
    };
    let head = if live { (s.live_shared)(&label) } else { (s.shared)(&label) };
    let text = match offer {
        Some(offer) => offer_text(offer, t).0,
        None => s.live_empty.to_string(),
    };
    view! { <p><b>{head}</b>{text}</p> }.into_any()
}

/// Whether the offer can take the modules over: some are not planned yet, in the semester shown.
fn can_take(offer: Option<(Option<Offer>, String)>) -> bool {
    matches!(offer, Some((Some(Offer::Take { .. }), _)))
}

#[cfg(test)]
mod tests {
    use folia_calendar::semester::SemesterKey;
    use folia_pages::SharedModule;

    use crate::i18n::{DE, EN};
    use super::*;

    fn data(modules: &[(&str, &str)]) -> SharedPlanData {
        SharedPlanData {
            key: SemesterKey::parse("2026W").unwrap(),
            label: "WiSe 2026/27".into(),
            modules: modules.iter().map(|(id, name)| SharedModule { id: id.to_string(), name: name.to_string(), title: format!("Modul {name}"), credits: Some(6.0) }).collect(),
            missing: Vec::new(),
            program: None,
        }
    }

    #[test]
    fn the_offer_counts_what_the_plan_does_not_have() {
        let shared = data(&[("12104", "MIT-1"), ("11101", "AuP"), ("12107", "EEG")]);
        let names = "MIT-1, AuP, EEG".to_string();
        assert_eq!(offer_of(&shared, true, &[]), Offer::Take { names: names.clone(), count: 3, new: 3 });
        assert_eq!(offer_of(&shared, true, &["11101".to_string()]), Offer::Take { names: names.clone(), count: 3, new: 2 });
        assert_eq!(offer_of(&shared, true, &["12104".into(), "11101".into(), "12107".into()]), Offer::Held { names: names.clone() });
        assert_eq!(offer_of(&shared, false, &[]), Offer::Elsewhere { names });
        assert_eq!(offer_of(&data(&[]), true, &[]), Offer::Unknown);
    }

    #[test]
    fn the_offer_asks_in_the_pages_language() {
        let take = |count, new| Offer::Take { names: "MIT-1, AuP, EEG".into(), count, new };
        assert_eq!(offer_text(take(3, 3), &DE), ("3 Module: MIT-1, AuP, EEG. In deinen Stundenplan übernehmen?".to_string(), true));
        assert_eq!(offer_text(take(3, 1), &DE).0, "3 Module: MIT-1, AuP, EEG. Eins davon fehlen in deinem Stundenplan. Übernehmen?");
        assert_eq!(offer_text(take(3, 2), &EN).0, "3 modules: MIT-1, AuP, EEG. 2 of them are missing from your timetable. Add them?");
        assert_eq!(offer_text(Offer::Held { names: "AuP".into() }, &EN), ("AuP. All of them are already in your timetable.".to_string(), false));
        // The notes of the plan each begin their own way, in every language: none is where another
        // is looked for (`PlanCtx::undo`).
        for t in [&DE, &EN] {
            let notes = [t.studyplan_share.taken, t.studyplan_share.live_taken, t.studyplan_side.cleared, super::super::head::FROM_MARKED];
            for (i, a) in notes.iter().enumerate() {
                assert!(notes.iter().enumerate().all(|(j, b)| i == j || !a.starts_with(b)), "{a}");
            }
        }
    }

    /// A one-time link that opens nothing says why, and never shows a piece of the plan (owner,
    /// 2026-11-08: „Aber es sollte schon eine fehlermeldung geben mit link abgelaufen oder so").
    #[test]
    fn a_link_that_opens_nothing_says_why() {
        assert!(problem_text(ShareProblem::Taken, &DE).starts_with("Dieser Link wurde schon geöffnet."));
        assert!(problem_text(ShareProblem::Expired, &DE).starts_with("Dieser Link ist abgelaufen."));
        assert!(problem_text(ShareProblem::Withdrawn, &DE).starts_with("Dieser Link wurde zurückgezogen"));
        assert_eq!(problem_text(ShareProblem::Ended, &DE), "Dieser Kalender wird nicht mehr geteilt.");
        assert!(problem_text(ShareProblem::Unknown, &EN).starts_with("This link opens nothing"));
        // Each problem its own words, in every language.
        let all = [ShareProblem::Taken, ShareProblem::Expired, ShareProblem::Withdrawn, ShareProblem::Ended, ShareProblem::Unknown, ShareProblem::Busy, ShareProblem::Unreachable, ShareProblem::Unsupported];
        for t in [&DE, &EN] {
            let texts: std::collections::BTreeSet<&str> = all.iter().map(|problem| problem_text(*problem, t)).collect();
            assert_eq!(texts.len(), all.len());
        }
    }

    /// What a one-time link opened to is kept for its parcel alone, and read back as it was.
    #[test]
    fn what_a_one_time_link_opened_to_is_kept_for_the_tab() {
        let link = folia_plans::shares::encode(&[3u8; 48]);
        let parcel = folia_plans::shares::encode(&[5u8; 48]);
        let kept = Kept::Live(link.clone());
        assert_eq!(kept_of(&kept_text(&parcel, &kept), &parcel), Some(kept));
        assert_eq!(kept_of(&kept_text(&parcel, &Kept::Live(link)), &folia_plans::shares::encode(&[6u8; 48])), None, "another parcel's");
        let part = LivePart { semester: SemesterKey::parse("2026W").unwrap().index(), modules: vec![12104], ..LivePart::default() };
        let copy = Kept::Copy(Box::new(part));
        assert_eq!(kept_of(&kept_text(&parcel, &copy), &parcel), Some(copy));
        assert_eq!(kept_of(&format!("{parcel}\tlive\tnot-a-link"), &parcel), None);
        assert_eq!(kept_of("", &parcel), None);
    }
}
