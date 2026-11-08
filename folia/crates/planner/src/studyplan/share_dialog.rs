//! „Teilen" (folia/design/teilen/teilen.html; owner, 2026-11-08: „wenn man auf teilen drückt ein
//! modal bekommt, wo man ähnlich wie bei google docs so dann einstellen kann, ist das ein einmal
//! link. Wir der kallender statisch geteilt ohne sync, wird er mit sync geteilt, sollen extra
//! termine angezeigt werden etc."): one dialog for handing a calendar on, to persons or into a
//! calendar app, as a copy of now or one that stays current (Betula Sync, step 3).
//!
//! On top what is shared of the calendar now, each with „Nicht mehr teilen" (or „Zurückziehen" for
//! a one-time link nobody opened), as Google Docs lists the people with access. Under it a new
//! share: with persons or into a calendar app (the segment changes only what stands below it), how
//! (stays current or a fixed copy), who may open the link (one person once, within 7 days, or
//! everybody), what comes along beside the modules, groups and changes (the Termine of one's own,
//! the notes), and for whom (a name for the list alone). Every choice says what it means next to
//! it.
//!
//! What stays current and a one-time link need Betula Sync. In a private profile those choices
//! carry „mit Sync"; taking one saves the profile with Betula Sync on the way („Profil speichern
//! und Link erstellen"), and under a fixed copy Betula offers what Betula Sync makes of it, one
//! click away (owner, 2026-11-08: „bei jeder option, wo die nutzererfahrung besser wird ein
//! "upgrade" von betula sync angeboten wird. Also dass man da schnell sein profil speichern
//! kann"). A fixed copy for everybody is the link of today (`share.rs`), and a fixed calendar
//! subscription the address of today (`export.rs`): neither needs a server.
//!
//! A calendar of one's own (Lena's in the plan) is shared into a calendar app only (owner,
//! decision 5: whether it goes on to others is Lena's to decide).

use folia_calendar::day::Day;
use folia_calendar::live::feed_path;
use folia_calendar::share::SharedPlan;
use folia_design::modal::{kept, Modal, ModalHead, ModalSize};
use folia_design::ui::Icon;
use folia_plans::shares::{Share, ShareTo};
use folia_stores::sync::share::{live_path, once_path, ShareProblem, ONCE_TTL};
use folia_stores::sync::{self as sync, ShareRequest, Sync};
use leptos::prelude::*;

use crate::i18n;
use super::export::{abo_of, site, ways_of, Offer};
use super::PlanCtx;

/// Which side of the dialog is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShareTab {
    /// „Mit Personen": a link someone opens in Betula.
    People,
    /// „In Kalender-App": an address a calendar app subscribes to.
    App,
}

/// The dialog open: for which calendar (0 the plan's, „Mein Studiengang"), on which side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShareOpen {
    pub calendar: u8,
    pub tab: ShareTab,
}

/// What the choices are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Choice {
    tab: ShareTab,
    live: bool,
    once: bool,
    own: bool,
    notes: bool,
}

impl Choice {
    /// The choices a dialog opens with: what stays current for one person in a synced profile,
    /// the fixed copy for everybody of today in a private one (decision 1), and the side asked for.
    fn first(tab: ShareTab, synced: bool) -> Choice {
        Choice { tab, live: synced, once: synced && tab == ShareTab::People, own: false, notes: false }
    }

    /// One person once: on the side of persons only.
    fn once(self) -> bool {
        self.once && self.tab == ShareTab::People
    }

    /// Whether it needs Betula Sync: a part at Phloem, or a parcel.
    fn needs_sync(self) -> bool {
        self.live || self.once()
    }

    /// Whether the Termine of one's own and the notes can come along: in what Phloem keeps, not in
    /// an address of today.
    fn carries_along(self) -> bool {
        self.needs_sync()
    }
}

/// What a share made shows: its address, what it is, until when it opens, what did not fit.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Made {
    url: String,
    choice: Choice,
    until: Option<u64>,
    left_out: usize,
    /// The ways into a calendar app: Apple (`webcal://`), Google, Outlook.
    ways: Option<(String, String, String)>,
    /// The share's id in the list, where it is one.
    id: Option<String>,
}

/// The texts of what went wrong.
fn problem_text(problem: ShareProblem, t: &'static i18n::Texts) -> &'static str {
    let d = &t.studyplan_share_dialog;
    match problem {
        ShareProblem::NotSynced => d.problem_not_synced,
        ShareProblem::TooMany => d.problem_too_many,
        ShareProblem::Busy => d.problem_busy,
        ShareProblem::Unsupported => d.problem_unsupported,
        _ => d.problem_unreachable,
    }
}

/// The day of a time in seconds, for the list's words.
fn day_of(secs: u64) -> Day {
    Day(i32::try_from(secs / 86_400).unwrap_or(i32::MAX))
}

/// The path of a share's link as the list copies it again: its part's (stays current), its
/// waiting parcel's (one person), a calendar app's address.
fn path_of(share: &Share) -> Option<String> {
    if !share.parcel.is_empty() {
        return Some(once_path(&share.parcel));
    }
    let link = share.link()?;
    Some(match share.to {
        ShareTo::App => feed_path(&link),
        ShareTo::Person => live_path(&link),
    })
}

/// The address of a path of this site, in the page's language.
fn absolute(path: &str, t: &'static i18n::Texts) -> String {
    let origin = site().map(|(origin, _)| origin).unwrap_or_default();
    format!("{origin}{}", t.path(path))
}

/// The dialog, while `ctx.sharing` names a calendar.
#[component]
pub(super) fn ShareDialog(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let d = &t.studyplan_share_dialog;
    let sync = Sync::expect();
    let offered = sync::offered().is_some();
    let open = Memo::new(move |_| ctx.sharing.get());
    let shown = kept(move || open.get());
    let profile = move || ctx.plan.map_or(0, |plan| plan.profile());
    let synced = Memo::new(move |_| sync.is_some_and(|sync| sync.state(profile()).synced()));

    let choice = RwSignal::new(Choice::first(ShareTab::People, false));
    let who = RwSignal::new(String::new());
    let made = RwSignal::new(None::<Made>);
    let working = RwSignal::new(false);
    let saving = RwSignal::new(false);
    let problem = RwSignal::new(None::<&'static str>);
    let stopping = RwSignal::new(None::<String>);
    let stop_failed = RwSignal::new(false);
    // A new opening starts anew: its side, the choices the profile has, nothing made.
    Effect::new(move |before: Option<Option<ShareOpen>>| {
        let now = open.get();
        if let Some(asked) = now.filter(|asked| before.flatten() != Some(*asked)) {
            let tab = if asked.calendar == 0 { asked.tab } else { ShareTab::App };
            choice.set(Choice::first(tab, offered && synced.get_untracked()));
            who.set(String::new());
            made.set(None);
            problem.set(None);
            stop_failed.set(false);
            if let Some(sync) = sync {
                sync.check_parcels();
            }
        }
        now
    });
    let close = Callback::new(move |()| ctx.sharing.set(None));
    let calendar = Memo::new(move |_| shown.get().map_or(0, |open| open.calendar));

    // ---- what is shared
    let shares = Memo::new(move |_| {
        let id = calendar.get();
        ctx.plan.map(|plan| plan.with(|doc| doc.shares_of(id).into_iter().cloned().collect::<Vec<Share>>())).unwrap_or_default()
    });
    // The calendar's name, its modules and what comes along of the semester shown.
    let key = ctx.key;
    let facts = Memo::new(move |_| {
        let id = calendar.get();
        let s = key.get();
        ctx.plan.map(|plan| {
            plan.with(|doc| {
                if id == 0 {
                    let notes = doc.hidden.get(&s).map_or(0, |hides| hides.notes.len());
                    (String::new(), doc.week_in(s).len(), doc.own_in(0, s).len(), notes)
                } else {
                    let calendar = doc.calendar(id);
                    let name = calendar.map(|c| if c.name.is_empty() { (t.studyplan_calendars.unnamed)(c.id) } else { c.name.clone() }).unwrap_or_default();
                    let modules = calendar.map_or(0, |c| c.modules_in(s).len());
                    let notes = calendar.map_or(0, |c| c.notes.keys().filter(|(semester, _)| *semester == s).count());
                    (name, modules, doc.own_in(id, s).len(), notes)
                }
            })
        })
        .unwrap_or_default()
    });

    // ---- the fixed links of today: the plan's link, and the subscription's address.
    let fixed_link = Memo::new(move |_| {
        if calendar.get() != 0 {
            return None;
        }
        let (chosen, changes, visits) = ctx.selection.with(|(_, selection)| (selection.chosen_rows.clone(), selection.changes.clone(), selection.visits.clone()));
        ctx.wanted.with(|(key, ids, program)| {
            let planned: Vec<String> = ids.iter().filter(|id| !visits.contains(*id)).cloned().collect();
            let code = SharedPlan::of(*key, &planned, program.as_deref())?.with_chosen(chosen).with_changes(&changes).code().ok()?;
            Some(folia_routes::url::share_path(&code))
        })
    });
    let program = Memo::new(move |_| ctx.data.with(|data| data.as_ref().ok().and_then(|data| data.program.clone())));
    let fixed_abo = Memo::new(move |_| {
        let id = calendar.get();
        if id == 0 {
            let subscribed = ctx.plan.map(|plan| plan.with(|doc| doc.subscribed.clone())).unwrap_or_default();
            let abo = ctx.selection.with(|(_, selection)| program.with(|program| ctx.table.with(|table| table.as_ref().and_then(|table| abo_of(table, selection, program.as_deref(), &subscribed)))));
            return abo.and_then(|abo| match abo.offer {
                Offer::Code(code) => Some(code),
                _ => None,
            });
        }
        let s = key.get();
        let (modules, selection, program) = ctx.plan.and_then(|plan| plan.with(|doc| doc.calendar(id).map(|c| (c.modules_in(s), c.selection(s), c.program.clone()))))?;
        folia_timetable::export::subscription_of(s, &modules, program.as_deref(), &selection, None).0.code().ok()
    });
    // The program whose abbreviations name the modules of a share.
    let share_program = move || {
        let id = calendar.get_untracked();
        if id == 0 {
            ctx.wanted.with_untracked(|(_, _, program)| program.clone())
        } else {
            ctx.plan.and_then(|plan| plan.with_untracked(|doc| doc.calendar(id).and_then(|c| c.program.clone())))
        }
    };

    // ---- making it
    let create = move || {
        let c = choice.get_untracked();
        let s = key.get_untracked();
        problem.set(None);
        if !c.needs_sync() {
            // A fixed copy for everybody: the link or the address of today.
            let result = match c.tab {
                ShareTab::People => fixed_link.get_untracked().map(|path| Made { url: absolute(&path, t), choice: c, until: None, left_out: 0, ways: None, id: None }),
                ShareTab::App => fixed_abo.get_untracked().map(|code| {
                    let path = folia_calendar::subscription::path(&code);
                    if calendar.get_untracked() == 0 {
                        if let Some(plan) = ctx.plan {
                            plan.update(|doc| doc.remember(s, &code));
                        }
                    }
                    let label = s.label(t.locale);
                    let ways = site().map(|(origin, host)| ways_of(&origin, &host, &path, &label, t.locale));
                    Made { url: absolute(&path, t), choice: c, until: None, left_out: 0, ways: ways.map(|w| (w.apple, w.google, w.outlook)), id: None }
                }),
            };
            match result {
                Some(result) => made.set(Some(result)),
                None => problem.set(Some(d.note_nothing)),
            }
            return;
        }
        let Some(sync) = sync else { return };
        working.set(true);
        let request = ShareRequest {
            calendar: calendar.get_untracked(),
            to: if c.tab == ShareTab::App { ShareTo::App } else { ShareTo::Person },
            semester: s,
            live: c.live,
            once: c.once(),
            own: c.own,
            notes: c.notes,
            program: share_program(),
            name: who.get_untracked(),
        };
        sync.share(request, move |result| {
            let _ = working.try_set(false);
            match result {
                Ok(done) => {
                    let url = absolute(&done.path, t);
                    let ways = (c.tab == ShareTab::App).then(|| site().map(|(origin, host)| ways_of(&origin, &host, &done.path, &s.label(t.locale), t.locale))).flatten();
                    let _ = made.try_set(Some(Made { url, choice: c, until: done.until, left_out: done.left_out, ways: ways.map(|w| (w.apple, w.google, w.outlook)), id: Some(done.share.id) }));
                }
                Err(why) => {
                    let _ = problem.try_set(Some(problem_text(why, t)));
                }
            }
        });
    };
    // Saves the profile with Betula Sync, then `then`.
    let save_then = move |then: Box<dyn FnOnce() + 'static>| {
        let Some(sync) = sync else { return };
        if saving.get_untracked() {
            return;
        }
        saving.set(true);
        problem.set(None);
        sync.activate_then(profile(), move |result| {
            let _ = saving.try_set(false);
            match result {
                Ok(()) => then(),
                Err(_) => {
                    let _ = problem.try_set(Some(d.save_failed));
                }
            }
        });
    };
    let primary = move |_| {
        if working.get_untracked() || saving.get_untracked() {
            return;
        }
        if choice.get_untracked().needs_sync() && !synced.get_untracked() {
            save_then(Box::new(create));
        } else {
            create();
        }
    };
    // „Profil speichern" of an offer: the choice it offered is taken.
    let save_offer = move |_| save_then(Box::new(move || choice.update(|c| c.live = true)));
    let stop = move |id: String| {
        let Some(sync) = sync else { return };
        stopping.set(Some(id.clone()));
        stop_failed.set(false);
        sync.unshare(&id, move |result| {
            let _ = stopping.try_set(None);
            if result.is_err() {
                let _ = stop_failed.try_set(true);
            }
        });
    };

    // ---- the parts of the view
    let list = move || {
        let all = shares.get();
        if all.is_empty() {
            return ().into_any();
        }
        let fresh = made.with(|made| made.as_ref().and_then(|made| made.id.clone()));
        let rows = all
            .into_iter()
            .map(|share| {
                let id = share.id.clone();
                let busy = {
                    let id = id.clone();
                    move || stopping.with(|stopping| stopping.as_deref() == Some(id.as_str()))
                };
                let waiting = !share.parcel.is_empty();
                let title = if !share.name.is_empty() {
                    share.name.clone()
                } else if share.to == ShareTo::App {
                    d.row_app.to_string()
                } else if share.once {
                    d.row_once.to_string()
                } else {
                    d.row_everyone.to_string()
                };
                let avatar = match (share.to, share.name.chars().next()) {
                    (ShareTo::App, _) => "K".to_string(),
                    (_, Some(first)) => first.to_uppercase().collect(),
                    (_, None) if share.once => "1×".to_string(),
                    (_, None) => "↗".to_string(),
                };
                let mut line: Vec<String> = Vec::new();
                if waiting {
                    line.push((d.waiting)(&day_of(share.made + ONCE_TTL).day_month(t.locale)));
                } else {
                    line.push((d.since)(&day_of(share.made).day_month(t.locale)));
                }
                if share.own {
                    line.push(d.with_own.to_string());
                }
                let copy = path_of(&share).map(|path| t.path(&path));
                let (action, label) = if share.live || !waiting { (id.clone(), d.stop) } else { (id.clone(), d.withdraw) };
                let is_fresh = fresh.as_deref() == Some(id.as_str());
                view! {
                    <div class="sh-row" class:fresh=is_fresh>
                        <span class="sh-av">{avatar}</span>
                        <span class="t">
                            <b>{title}{is_fresh.then(|| view! { <span class="sh-new">{d.new}</span> })}</b>
                            <span>
                                {if share.live { view! { <span class="sh-live">{d.live}</span> }.into_any() } else { view! { <span>{d.fixed}</span> }.into_any() }}
                                {format!(" · {}", line.join(" · "))}
                            </span>
                        </span>
                        {copy.map(|path| view! {
                            <button class="mini" type="button" data-action="copy-text" data-absolute="" data-text=path><Icon name="copy"/><span>{d.copy_again}</span></button>
                        })}
                        <button class="mini danger" type="button" aria-busy=move || busy().then_some("true") on:click=move |_| stop(action.clone())>{label}</button>
                    </div>
                }
            })
            .collect_view();
        view! {
            <section class="sh-grp">
                <h3 class="sh-label">{d.shared}</h3>
                <div class="sh-list">{rows}</div>
                {move || stop_failed.get().then(|| view! { <p class="sh-note warn" role="status"><Icon name="triangle-alert"/><span>{d.stop_failed}</span></p> })}
            </section>
        }
        .into_any()
    };
    let option = move |title: &'static str, text: &'static str, on: Signal<bool>, sync_mark: bool, pick: Callback<()>| {
        view! {
            <button type="button" class="sh-opt" role="radio" aria-checked=move || if on.get() { "true" } else { "false" } on:click=move |_| pick.run(())>
                <b>{title}{move || (sync_mark && offered && !synced.get()).then(|| view! { <i class="sh-tag">{d.with_sync}</i> })}</b>
                <span>{text}</span>
            </button>
        }
    };
    let settings = move || {
        let c = choice.get();
        let own_calendar = calendar.get() != 0;
        let (_, _, own_count, note_count) = facts.get();
        let tabs = (!own_calendar).then(|| {
            view! {
                <div class="seg sh-seg" role="radiogroup">
                    <button type="button" role="radio" aria-checked=move || if choice.get().tab == ShareTab::People { "true" } else { "false" } on:click=move |_| choice.update(|c| { c.tab = ShareTab::People; })>{d.people}</button>
                    <button type="button" role="radio" aria-checked=move || if choice.get().tab == ShareTab::App { "true" } else { "false" } on:click=move |_| choice.update(|c| { c.tab = ShareTab::App; })>{d.app}</button>
                </div>
            }
        });
        let live = Signal::derive(move || choice.get().live);
        let copy = Signal::derive(move || !choice.get().live);
        let once = Signal::derive(move || choice.get().once);
        let all = Signal::derive(move || !choice.get().once);
        let how = offered.then(|| {
            let (stays_text, copy_text) = if c.tab == ShareTab::People { (d.stays_people, d.copy_people) } else { (d.stays_app, d.copy_app) };
            view! {
                <section class="sh-grp">
                    <h3 class="sh-label">{d.how}</h3>
                    <div class="sh-opts" role="radiogroup">
                        {option(d.stays, stays_text, live, true, Callback::new(move |()| choice.update(|c| c.live = true)))}
                        {option(d.copy, copy_text, copy, false, Callback::new(move |()| choice.update(|c| c.live = false)))}
                    </div>
                </section>
                {(c.tab == ShareTab::People).then(|| view! {
                    <section class="sh-grp">
                        <h3 class="sh-label">{d.who_opens}</h3>
                        <div class="sh-opts" role="radiogroup">
                            {option(d.one, d.one_hint, once, true, Callback::new(move |()| choice.update(|c| c.once = true)))}
                            {option(d.everyone, d.everyone_hint, all, false, Callback::new(move |()| choice.update(|c| c.once = false)))}
                        </div>
                    </section>
                })}
            }
        });
        // What the choice means, and what Betula Sync makes of it.
        let mut notes: Vec<AnyView> = Vec::new();
        if offered && c.needs_sync() && !synced.get() {
            notes.push(view! { <div class="sh-up need"><Icon name="monitor-smartphone"/><span class="t"><b>{d.needs_sync}</b><span>{d.needs_sync_hint}</span></span></div> }.into_any());
        }
        let note = |text: &'static str, warn: bool| view! { <p class="sh-note" class:warn=warn><Icon name="info"/><span>{text}</span></p> }.into_any();
        match (c.tab, c.live, c.once()) {
            (ShareTab::People, false, false) => notes.push(note(d.note_copy_everyone, false)),
            (ShareTab::People, false, true) => notes.push(note(d.note_copy_one, false)),
            (ShareTab::People, true, false) => notes.push(note(d.note_live_everyone, true)),
            (ShareTab::App, true, _) => notes.push(note(d.note_app_live, false)),
            (ShareTab::App, false, _) => notes.push(note(d.note_app_copy, false)),
            _ => {}
        }
        if offered && !synced.get() && !c.live {
            let (title, hint) = if c.tab == ShareTab::People { (d.offer_people, d.offer_people_hint) } else { (d.offer_app, d.offer_app_hint) };
            notes.push(
                view! {
                    <div class="sh-up">
                        <Icon name="monitor-smartphone"/>
                        <span class="t"><b>{title}</b><span>{hint}</span></span>
                        <button class="mini" type="button" aria-busy=move || saving.get().then_some("true") on:click=save_offer>{move || if saving.get() { d.saving } else { d.save_profile }}</button>
                    </div>
                }
                .into_any(),
            );
        }
        let along_note = (!c.carries_along() && (c.own || c.notes)).then(|| note(d.note_along_fixed, false));
        let switch = move |title: &'static str, text: String, on: Signal<bool>, flip: Callback<()>| {
            view! {
                <button type="button" class="sh-sw" role="switch" aria-checked=move || if on.get() { "true" } else { "false" } on:click=move |_| flip.run(())>
                    <span class="track"></span>
                    <span class="t"><b>{title}</b><span>{text}</span></span>
                </button>
            }
        };
        let own_on = Signal::derive(move || choice.get().own);
        let notes_on = Signal::derive(move || choice.get().notes);
        let along = view! {
            <section class="sh-grp">
                <h3 class="sh-label">{d.along}</h3>
                <p class="sh-always"><b>{d.always}</b>" "{d.always_list}</p>
                {switch(d.own, (d.own_count)(own_count), own_on, Callback::new(move |()| choice.update(|c| c.own = !c.own)))}
                {switch(if c.tab == ShareTab::People { d.notes } else { d.notes_app }, (d.notes_count)(note_count), notes_on, Callback::new(move |()| choice.update(|c| c.notes = !c.notes)))}
                {along_note}
            </section>
        };
        let for_whom = (c.tab == ShareTab::People && c.needs_sync()).then(|| {
            view! {
                <section class="sh-grp">
                    <label class="sh-label" for="sh-who">{d.for_whom}</label>
                    <input id="sh-who" class="sh-input" type="text" autocomplete="off" maxlength=folia_plans::shares::MAX_SHARE_NAME placeholder=d.for_whom_placeholder prop:value=move || who.get() on:input=move |ev| who.set(event_target_value(&ev))/>
                    <small class="sh-hint">{d.for_whom_hint}</small>
                </section>
            }
        });
        view! { {tabs}{how}{notes}{along}{for_whom} }.into_any()
    };
    let result = move || {
        let made = made.get()?;
        let c = made.choice;
        let qr = folia_design::qr::path(&made.url);
        let meta = match (c.tab, c.live, c.once()) {
            (ShareTab::App, true, _) => d.meta_app_live.to_string(),
            (ShareTab::App, false, _) => d.meta_app_copy.to_string(),
            (_, _, true) => (d.meta_once)(&made.until.map(|ms| day_of(ms / 1_000).day_month(t.locale)).unwrap_or_default()),
            (_, true, false) => d.meta_live.to_string(),
            (_, false, false) => d.meta_copy.to_string(),
        };
        let mut summary: Vec<String> = vec![if c.live { d.summary_live.to_string() } else { d.summary_copy.to_string() }];
        if c.tab == ShareTab::People {
            summary.push(if c.once() { d.summary_one.to_string() } else { d.summary_everyone.to_string() });
        }
        let s = key.get_untracked();
        summary.push(if c.live { (d.summary_next)(&s.label(t.locale)) } else { s.label(t.locale) });
        if c.carries_along() {
            summary.push(if c.own { d.summary_own.to_string() } else { d.summary_no_own.to_string() });
            if c.notes {
                summary.push(d.summary_notes.to_string());
            }
        }
        let url = made.url.clone();
        let buttons = match made.ways.clone() {
            Some((apple, google, outlook)) => view! {
                <button class="mini primary" type="button" data-action="copy-text" data-text=url.clone()><Icon name="copy"/><span>{d.copy_address}</span></button>
                <a class="mini" href=google target="_blank" rel="external noopener">"Google"</a>
                <a class="mini" href=apple rel="external">"Apple"</a>
                <a class="mini" href=outlook target="_blank" rel="external noopener">"Outlook"</a>
            }
            .into_any(),
            None => view! {
                <button class="mini primary" type="button" data-action="copy-text" data-text=url.clone()><Icon name="copy"/><span>{d.copy_link}</span></button>
                <button class="mini" type="button" data-action="share-link" data-text=url.clone()><Icon name="share-2"/><span>{d.share_more}</span></button>
            }
            .into_any(),
        };
        Some(view! {
            <section class="sh-grp">
                <h3 class="sh-label">{if c.tab == ShareTab::App { d.address } else { d.link }}</h3>
                <div class="sh-result">
                    {qr.map(|(drawn, width)| view! {
                        <svg class="pf-qr sh-qr" viewBox=format!("0 0 {width} {width}") role="img" aria-label=d.qr_label shape-rendering="crispEdges"><path d=drawn/></svg>
                    })}
                    <div class="sh-result-text">
                        <p class="sh-link">{url.clone()}</p>
                        <p class="sh-meta">{meta}</p>
                        {(made.left_out > 0).then(|| view! { <p class="sh-meta warn">{(d.left_out)(made.left_out)}</p> })}
                        <div class="sh-acts">{buttons}</div>
                    </div>
                </div>
                <p class="sh-summary">{summary.join(" · ")}</p>
            </section>
        })
    };

    let title = Memo::new(move |_| {
        let (name, ..) = facts.get();
        if calendar.get() == 0 { d.title.to_string() } else { (d.title_calendar)(&name) }
    });
    let sub = Memo::new(move |_| {
        let (_, modules, ..) = facts.get();
        (d.sub)(&key.get().label(t.locale), modules)
    });
    let primary_label = move || {
        let c = choice.get();
        if working.get() || saving.get() {
            return d.creating;
        }
        let save = offered && c.needs_sync() && !synced.get();
        match (c.tab, save, c.once()) {
            (ShareTab::App, true, _) => d.save_and_address,
            (ShareTab::App, false, _) => d.create_address,
            (ShareTab::People, true, _) => d.save_and_link,
            (ShareTab::People, false, true) => d.create_once,
            (ShareTab::People, false, false) => d.create_link,
        }
    };
    let open_now = Signal::derive(move || open.get().is_some());
    view! {
        <Modal open=open_now on_close=close size=ModalSize::M class="sh-dialog" labelled_by="sh-title">
            <ModalHead id="sh-title" title=title.get_untracked() on_close=close>
                <p>{move || sub.get()}</p>
            </ModalHead>
            <div class="st-dlg-body sh-body">
                {list}
                {move || if made.with(Option::is_some) { result().map(IntoAny::into_any).unwrap_or_else(|| ().into_any()) } else { settings() }}
                {move || problem.get().map(|text| view! { <p class="sh-note warn" role="alert"><Icon name="triangle-alert"/><span>{text}</span></p> })}
            </div>
            <footer class="st-dlg-foot">
                {move || if made.with(Option::is_some) {
                    view! {
                        <button class="mini" type="button" on:click=move |_| made.set(None)>{d.another}</button>
                        <button class="mini primary" type="button" on:click=move |_| close.run(())>{d.done}</button>
                    }
                    .into_any()
                } else {
                    view! {
                        <button class="mini" type="button" on:click=move |_| close.run(())>{d.cancel}</button>
                        <button class="mini primary" type="button" id="sh-make" aria-busy=move || (working.get() || saving.get()).then_some("true") on:click=primary>{primary_label}</button>
                    }
                    .into_any()
                }}
            </footer>
        </Modal>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use folia_calendar::semester::SemesterKey;

    #[test]
    fn a_dialog_opens_with_the_choices_the_profile_has() {
        // Synced: what stays current, for one person; private: the fixed link for everybody.
        assert_eq!(Choice::first(ShareTab::People, true), Choice { tab: ShareTab::People, live: true, once: true, own: false, notes: false });
        assert_eq!(Choice::first(ShareTab::People, false), Choice { tab: ShareTab::People, live: false, once: false, own: false, notes: false });
        // A calendar app has no one-time link.
        let app = Choice::first(ShareTab::App, true);
        assert!(app.live && !app.once());
        assert!(!Choice { once: true, ..Choice::first(ShareTab::App, false) }.needs_sync());
        // The own Termine come along only in what Phloem keeps.
        assert!(!Choice::first(ShareTab::People, false).carries_along());
        assert!(Choice { once: true, ..Choice::first(ShareTab::People, false) }.carries_along());
    }

    #[test]
    fn a_share_is_copied_again_by_its_link() {
        use folia_plans::shares::encode;
        let share = Share {
            id: encode(&[1; 16]),
            calendar: 0,
            to: ShareTo::Person,
            semester: SemesterKey::parse("2026W").unwrap(),
            made: 0,
            live: true,
            once: true,
            own: false,
            notes: false,
            key: encode(&[2; 32]),
            parcel: encode(&[3; 48]),
            program: None,
            name: String::new(),
        };
        assert_eq!(path_of(&share), Some(once_path(&encode(&[3; 48]))), "while its parcel waits, the one-time link");
        let opened = Share { parcel: String::new(), ..share.clone() };
        assert_eq!(path_of(&opened), Some(live_path(&opened.link().unwrap())));
        let app = Share { to: ShareTo::App, once: false, ..opened.clone() };
        assert_eq!(path_of(&app), Some(feed_path(&app.link().unwrap())));
        assert!(folia_plans::shares::split_link(&app.link().unwrap()).is_some());
    }
}
