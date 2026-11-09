//! „Teilen" of the Merkliste (folia/design/teilen/teilen.html, decision 6: „Später derselbe
//! Dialog"; owner, 2026-11-09: „die Merkliste soll auch per dem neuen sync share dialog
//! funktionieren"): the dialog of a collection or of the whole list, as the calendar's
//! (`folia_planner`'s `share_dialog`). On top what is shared of it now, each with „Nicht mehr
//! teilen" (or „Zurückziehen" for a one-time link nobody opened); under it a new share: how (stays
//! current, or a fixed copy), who may open the link (one person once, within 7 days, or everybody),
//! whether the notes come along, and for whom (a name for the list alone).
//!
//! A fixed copy for everybody is the link of today (`/bookmarks#m=…`, `bookmarks::share_fragment`):
//! it needs no server, and its list is after the `#`, which never reaches one, so the notes may
//! come along in it too. What stays current and a one-time link need Betula Sync
//! (`Sync::share_list`); in a private profile those choices carry „mit Sync", taking one saves the
//! profile on the way, and under a fixed copy Betula offers what Betula Sync makes of it.
//!
//! Lena's side (owner, 2026-11-09: „Eigene Liste"): a link that names a list that stays current
//! (`/bookmarks#live=…`) or a one-time link (`/bookmarks#once=…`) is offered at the top of the list
//! (`ListOffer`): „Als Liste folgen" keeps it beside her collections, under „Geteilt mit dir",
//! where every change arrives; „Übernehmen" marks its modules, as a collection, with the notes
//! that came along; „Verwerfen" leaves everything as it is. A list followed is a view of the page
//! like a collection (`?collection=<n>`), to look at and take from: the marks of its rows are hers,
//! „Alle übernehmen" (`FollowedHead`) takes everything. When it is not shared any more, it keeps
//! its last state until she keeps it or deletes it.

use folia_calendar::day::Day;
use folia_design::modal::{kept, Modal, ModalHead, ModalSize};
use folia_design::nav;
use folia_design::ui::Icon;
use folia_routes::url::{self, BookmarksUrl, CollectionView};
use folia_stores::bookmarks::{share_fragment, Bookmarks, Brought, ListPart};
use folia_stores::collections::{Collections, Followed, ListShare, Refused};
use folia_stores::sync::share::{list_live_path, list_once_path, opening, Opening, ShareProblem, ONCE_TTL};
use folia_stores::sync::{self as sync, ListShareRequest, OpenedList, Sync};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;

use crate::bookmarks::{absolute, notes_of};
use crate::i18n::{self, use_location, Texts};

/// The name a list followed goes by: the one given here, else the list's own, or for a whole
/// Merkliste shared, which has none, the word for it.
pub(crate) fn followed_name(followed: &Followed, t: &'static Texts) -> String {
    followed.name_now().unwrap_or_else(|| t.bookmarks_share.unnamed.to_string())
}

/// What a part is called on Lena's side, before it is followed.
fn part_name(part: &ListPart, t: &'static Texts) -> String {
    if part.name.is_empty() {
        t.bookmarks_share.unnamed.to_string()
    } else {
        part.name.clone()
    }
}

/// „Übernehmen", „Alle übernehmen": the modules of a list shared marked (as many as there is room
/// for), as a collection of its name, or for a whole Merkliste with its collections, and the notes
/// that came along where a module has none. How many were marked anew.
pub(crate) fn take_part(bookmarks: Bookmarks, collections: Option<Collections>, part: &ListPart) -> usize {
    let added = bookmarks.add_all(&part.brought.ids);
    if let Some(collections) = collections {
        let named: Vec<(String, Vec<String>)> = if part.name.is_empty() { part.brought.collections.clone() } else { vec![(part.name.clone(), part.brought.ids.clone())] };
        collections.update(|shelf| shelf.adopt(&named, &part.brought.notes, |id| bookmarks.is_marked_untracked(id)));
    }
    added
}

// ---------- the dialog ----------

/// What the choices are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Choice {
    live: bool,
    once: bool,
    notes: bool,
}

impl Choice {
    /// The choices a dialog opens with: what stays current for one person in a synced profile, the
    /// fixed copy for everybody of today in a private one (as the calendar's, decision 1).
    fn first(synced: bool) -> Choice {
        Choice { live: synced, once: synced, notes: false }
    }

    /// Whether it needs Betula Sync: a part at Phloem, or a parcel.
    fn needs_sync(self) -> bool {
        self.live || self.once
    }
}

/// What the list shared holds now: its name (none for the whole Merkliste), its marked modules in
/// the Merkliste's order, how many of them have a note, and of the whole list its collections.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Facts {
    name: String,
    ids: Vec<String>,
    notes: usize,
    collections: usize,
}

/// What a share made shows: its address, what it is, until when it opens, what did not fit.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Made {
    url: String,
    choice: Choice,
    until: Option<u64>,
    left_out: usize,
    /// The share's id in the list, where it is one.
    id: Option<String>,
}

/// The texts of what went wrong.
fn problem_text(problem: ShareProblem, t: &'static Texts) -> &'static str {
    let d = &t.bookmarks_share;
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

/// The path of a share's link as the list copies it again: its waiting parcel's (one person), else
/// its part's (stays current).
fn path_of(share: &ListShare) -> Option<String> {
    if !share.parcel.is_empty() {
        return Some(list_once_path(&share.parcel));
    }
    share.link().map(|link| list_live_path(&link))
}

/// The dialog „Teilen" of the Merkliste (`open`: 0) or of one of its collections, while `open`
/// names one.
#[component]
pub(crate) fn ListShareDialog(open: RwSignal<Option<u32>>) -> impl IntoView {
    let t = i18n::t();
    let d = &t.bookmarks_share;
    let sync = Sync::expect();
    let bookmarks = Bookmarks::expect();
    let collections = Collections::expect();
    let offered = sync::offered().is_some();
    let shown = kept(move || open.get());
    let profile = move || bookmarks.map_or(0, Bookmarks::profile);
    let synced = Memo::new(move |_| sync.is_some_and(|sync| sync.state(profile()).synced()));

    let choice = RwSignal::new(Choice::first(false));
    let who = RwSignal::new(String::new());
    let made = RwSignal::new(None::<Made>);
    let working = RwSignal::new(false);
    let saving = RwSignal::new(false);
    let problem = RwSignal::new(None::<&'static str>);
    let stopping = RwSignal::new(None::<String>);
    let stop_failed = RwSignal::new(false);
    // A new opening starts anew: the choices the profile has, nothing made.
    Effect::new(move |before: Option<Option<u32>>| {
        let now = open.get();
        if now.is_some() && before.flatten() != now {
            choice.set(Choice::first(offered && synced.get_untracked()));
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
    let close = Callback::new(move |()| open.set(None));
    let collection = Memo::new(move |_| shown.get().unwrap_or(0));
    let facts = Memo::new(move |_| {
        let id = collection.get();
        let marks: Vec<String> = bookmarks.map(|bookmarks| bookmarks.marks().into_iter().map(|mark| mark.id).collect()).unwrap_or_default();
        collections
            .map(|collections| {
                collections.with(|shelf| {
                    let ids: Vec<String> = if id == 0 { marks.clone() } else { marks.iter().filter(|module| shelf.holds(id, module)).cloned().collect() };
                    Facts {
                        name: if id == 0 { String::new() } else { shelf.collection(id).map(|c| c.name.clone()).unwrap_or_default() },
                        notes: ids.iter().filter(|module| shelf.note(module).is_some()).count(),
                        collections: if id == 0 { shelf.collections().len() } else { 0 },
                        ids,
                    }
                })
            })
            .unwrap_or_default()
    });
    let shares = Memo::new(move |_| {
        let id = collection.get();
        collections.map(|collections| collections.with(|shelf| shelf.shares_of(id).into_iter().cloned().collect::<Vec<ListShare>>())).unwrap_or_default()
    });
    // The fixed link for everybody: the link of today, with the collection's name (of the whole
    // list every collection), and the notes where they come along.
    let fixed_link = move |notes: bool| -> Option<String> {
        let id = collection.get_untracked();
        let Facts { name, ids, .. } = facts.get_untracked();
        if ids.is_empty() {
            return None;
        }
        let brought = collections?.with_untracked(|shelf| {
            let held = if id == 0 { shelf.collections().iter().map(|c| (c.name.clone(), ids.iter().filter(|module| shelf.holds(c.id, module)).cloned().collect())).collect() } else { vec![(name.clone(), ids.clone())] };
            Brought { notes: if notes { notes_of(shelf, &ids) } else { Vec::new() }, ids: ids.clone(), collections: held }
        });
        share_fragment(&brought).map(|fragment| absolute(&t.path(&format!("{}#{fragment}", url::BOOKMARKS))))
    };

    // ---- making it
    let create = move || {
        let c = choice.get_untracked();
        problem.set(None);
        if !c.needs_sync() {
            match fixed_link(c.notes) {
                Some(url) => made.set(Some(Made { url, choice: c, until: None, left_out: 0, id: None })),
                None => problem.set(Some(d.note_nothing)),
            }
            return;
        }
        let Some(sync) = sync else { return };
        working.set(true);
        let request = ListShareRequest { collection: collection.get_untracked(), live: c.live, once: c.once, notes: c.notes, name: who.get_untracked() };
        sync.share_list(request, move |result| {
            let _ = working.try_set(false);
            match result {
                Ok(done) => {
                    let _ = made.try_set(Some(Made { url: absolute(&t.path(&done.path)), choice: c, until: done.until, left_out: done.left_out, id: Some(done.share.id) }));
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
    // „Profil speichern" of the offer: the choice it offered is taken.
    let save_offer = move |_| save_then(Box::new(move || choice.update(|c| c.live = true)));
    let stop = move |id: String| {
        let Some(sync) = sync else { return };
        stopping.set(Some(id.clone()));
        stop_failed.set(false);
        sync.unshare_list(&id, move |result| {
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
                } else if share.once {
                    d.row_once.to_string()
                } else {
                    d.row_everyone.to_string()
                };
                let avatar = match share.name.chars().next() {
                    Some(first) => first.to_uppercase().collect(),
                    None if share.once => "1×".to_string(),
                    None => "↗".to_string(),
                };
                let mut line: Vec<String> = Vec::new();
                if waiting {
                    line.push((d.waiting)(&day_of(share.made + ONCE_TTL).day_month(t.locale)));
                } else {
                    line.push((d.since)(&day_of(share.made).day_month(t.locale)));
                }
                if share.notes {
                    line.push(d.with_notes.to_string());
                }
                let copy = path_of(&share).map(|path| t.path(&path));
                let label = if share.live || !waiting { d.stop } else { d.withdraw };
                let is_fresh = fresh.as_deref() == Some(id.as_str());
                let action = id.clone();
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
        let Facts { notes: note_count, .. } = facts.get();
        let whole = collection.get() == 0;
        let live = Signal::derive(move || choice.get().live);
        let copy = Signal::derive(move || !choice.get().live);
        let once = Signal::derive(move || choice.get().once);
        let all = Signal::derive(move || !choice.get().once);
        // Without Betula Sync on the site there is the fixed link for everybody alone.
        let how = offered.then(|| {
            view! {
                <section class="sh-grp">
                    <h3 class="sh-label">{d.how}</h3>
                    <div class="sh-opts" role="radiogroup">
                        {option(d.stays, d.stays_hint, live, true, Callback::new(move |()| choice.update(|c| c.live = true)))}
                        {option(d.copy, d.copy_hint, copy, false, Callback::new(move |()| choice.update(|c| c.live = false)))}
                    </div>
                </section>
                <section class="sh-grp">
                    <h3 class="sh-label">{d.who_opens}</h3>
                    <div class="sh-opts" role="radiogroup">
                        {option(d.one, d.one_hint, once, true, Callback::new(move |()| choice.update(|c| c.once = true)))}
                        {option(d.everyone, d.everyone_hint, all, false, Callback::new(move |()| choice.update(|c| c.once = false)))}
                    </div>
                </section>
            }
        });
        // What the choice means, and what Betula Sync makes of it.
        let mut notes: Vec<AnyView> = Vec::new();
        if offered && c.needs_sync() && !synced.get() {
            notes.push(view! { <div class="sh-up need"><Icon name="monitor-smartphone"/><span class="t"><b>{d.needs_sync}</b><span>{d.needs_sync_hint}</span></span></div> }.into_any());
        }
        let note = |text: &'static str, warn: bool| view! { <p class="sh-note" class:warn=warn><Icon name="info"/><span>{text}</span></p> }.into_any();
        match (c.live, c.once) {
            (false, false) => notes.push(note(d.note_copy_everyone, false)),
            (false, true) => notes.push(note(d.note_copy_one, false)),
            (true, false) => notes.push(note(d.note_live_everyone, true)),
            (true, true) => {}
        }
        if offered && !synced.get() && !c.live {
            notes.push(
                view! {
                    <div class="sh-up">
                        <Icon name="monitor-smartphone"/>
                        <span class="t"><b>{d.offer}</b><span>{d.offer_hint}</span></span>
                        <button class="mini" type="button" aria-busy=move || saving.get().then_some("true") on:click=save_offer>{move || if saving.get() { d.saving } else { d.save_profile }}</button>
                    </div>
                }
                .into_any(),
            );
        }
        let notes_on = Signal::derive(move || choice.get().notes);
        let along = view! {
            <section class="sh-grp">
                <h3 class="sh-label">{d.along}</h3>
                <p class="sh-always"><b>{d.always}</b>" "{if whole { d.always_collections } else { d.always_modules }}</p>
                <button type="button" class="sh-sw" id="sh-notes" role="switch" aria-checked=move || if notes_on.get() { "true" } else { "false" } on:click=move |_| choice.update(|c| c.notes = !c.notes)>
                    <span class="track"></span>
                    <span class="t"><b>{d.notes}</b><span>{(d.notes_count)(note_count)}</span></span>
                </button>
            </section>
        };
        let for_whom = c.needs_sync().then(|| {
            view! {
                <section class="sh-grp">
                    <label class="sh-label" for="sh-who">{d.for_whom}</label>
                    <input id="sh-who" class="sh-input" type="text" autocomplete="off" maxlength=folia_stores::collections::MAX_SHARE_NAME placeholder=d.for_whom_placeholder prop:value=move || who.get() on:input=move |ev| who.set(event_target_value(&ev))/>
                    <small class="sh-hint">{d.for_whom_hint}</small>
                </section>
            }
        });
        view! { {how}{notes}{along}{for_whom} }.into_any()
    };
    let result = move || {
        let made = made.get()?;
        let c = made.choice;
        let qr = folia_design::qr::path(&made.url);
        let meta = match (c.live, c.once) {
            (_, true) => (d.meta_once)(&made.until.map(|ms| day_of(ms / 1_000).day_month(t.locale)).unwrap_or_default()),
            (true, false) => d.meta_live.to_string(),
            (false, false) => d.meta_copy.to_string(),
        };
        let count = facts.with_untracked(|facts| facts.ids.len());
        let mut summary: Vec<String> = vec![if c.live { d.summary_live.to_string() } else { d.summary_copy.to_string() }];
        summary.push(if c.once { d.summary_one.to_string() } else { d.summary_everyone.to_string() });
        summary.push(folia_design::format::modules(i64::try_from(count).unwrap_or(i64::MAX), t.locale));
        if c.notes {
            summary.push(d.summary_notes.to_string());
        }
        let url = made.url.clone();
        Some(view! {
            <section class="sh-grp">
                <h3 class="sh-label">{d.link}</h3>
                <div class="sh-result">
                    {qr.map(|(drawn, width)| view! {
                        <svg class="pf-qr sh-qr" viewBox=format!("0 0 {width} {width}") role="img" aria-label=d.qr_label shape-rendering="crispEdges"><path d=drawn/></svg>
                    })}
                    <div class="sh-result-text">
                        <p class="sh-link">{url.clone()}</p>
                        <p class="sh-meta">{meta}</p>
                        {(made.left_out > 0).then(|| view! { <p class="sh-meta warn">{(d.left_out)(made.left_out)}</p> })}
                        <div class="sh-acts">
                            <button class="mini primary" type="button" data-action="copy-text" data-text=url.clone()><Icon name="copy"/><span>{d.copy_link}</span></button>
                            <button class="mini" type="button" data-action="share-link" data-text=url.clone()><Icon name="share-2"/><span>{d.share_more}</span></button>
                        </div>
                    </div>
                </div>
                <p class="sh-summary">{summary.join(" · ")}</p>
            </section>
        })
    };

    let title = Memo::new(move |_| {
        let name = facts.with(|facts| facts.name.clone());
        if collection.get() == 0 { d.title_list.to_string() } else { (d.title_collection)(&name) }
    });
    let sub = Memo::new(move |_| facts.with(|facts| (d.sub)(facts.ids.len(), facts.collections)));
    let primary_label = move || {
        let c = choice.get();
        if working.get() || saving.get() {
            return d.creating;
        }
        match (offered && c.needs_sync() && !synced.get(), c.once) {
            (true, _) => d.save_and_link,
            (false, true) => d.create_once,
            (false, false) => d.create_link,
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

// ---------- a link that names a list (Lena's side) ----------

/// What the part of the address after `#` opened to (`/bookmarks#live=…`, `/bookmarks#once=…`).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Found {
    /// The part or the parcel is on its way.
    Opening,
    /// A list that stays current: its link (the part's id and key) and the code of what it holds.
    Live { link: String, code: String },
    /// A copy of then, from a one-time link: its code.
    Copy(String),
    /// It opens nothing (any more), and why.
    Problem(ShareProblem),
    /// The site does not offer Betula Sync, whose service keeps what is shared.
    NotOffered,
}

/// What a one-time link of a list opened to, kept for this tab (`sessionStorage`): its parcel opens
/// once, so a reload finds it here rather than at Phloem, which would say it was opened already.
/// Taken away once the offer is answered.
const OPENED_KEY: &str = "betula.share.list.opened";

thread_local! {
    /// The fragment being opened and what it opened to, for as long as the tab lives: the page built
    /// anew while the parcel is on its way waits for the same answer instead of asking again.
    static OPENED: ArcRwSignal<Option<(String, Found)>> = ArcRwSignal::new(None);
}

/// What `OPENED_KEY` keeps of a parcel: the link of a list that stays current, or a copy's code.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Kept {
    Live(String),
    Copy(String),
}

/// Its text: the parcel's link, then what it opened to.
fn kept_text(parcel: &str, kept: &Kept) -> String {
    match kept {
        Kept::Live(link) => format!("{parcel}\tlive\t{link}"),
        Kept::Copy(code) => format!("{parcel}\tcopy\t{code}"),
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
        ("copy", code) if ListPart::from_code(code).is_some() => Some(Kept::Copy(code.to_string())),
        _ => None,
    }
}

/// What the offer says where a link opens nothing.
fn link_problem(problem: ShareProblem, t: &'static Texts) -> &'static str {
    let s = &t.bookmarks_share;
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

/// Opens what the fragment names into `OPENED`, where it is still the fragment being opened.
fn open_fragment(what: Opening, fragment: String) {
    let state = OPENED.with(Clone::clone);
    let answer = move |found: Found| {
        if state.with_untracked(|now| now.as_ref().is_some_and(|(of, _)| *of == fragment)) {
            state.set(Some((fragment.clone(), found)));
        }
    };
    let Some(sync) = Sync::expect().filter(|_| sync::offered().is_some()) else {
        answer(Found::NotOffered);
        return;
    };
    match what {
        Opening::Live(link) => {
            let opened = link.clone();
            sync.open_list(&opened, move |result| {
                answer(match result {
                    Ok(code) => Found::Live { link, code },
                    Err(problem) => Found::Problem(problem),
                });
            });
        }
        Opening::Once(parcel) => {
            let asked = parcel.clone();
            sync.open_once_list(&asked, move |result| {
                answer(match result {
                    Ok(OpenedList::Live { link, code }) => {
                        nav::session_set(OPENED_KEY, &kept_text(&parcel, &Kept::Live(link.clone())));
                        Found::Live { link, code }
                    }
                    Ok(OpenedList::Copy(code)) => {
                        nav::session_set(OPENED_KEY, &kept_text(&parcel, &Kept::Copy(code.clone())));
                        Found::Copy(code)
                    }
                    Err(problem) => Found::Problem(problem),
                });
            });
        }
    }
}

/// The offer of a link that names a list that stays current, or of a one-time link of a list
/// (`/bookmarks#live=…`, `/bookmarks#once=…`): where the offer of a link of today stands, with the
/// same words for the same things. Each answer takes the part after `#` out of the address.
///
/// It also keeps the lists followed current where nothing else does: when the page is built and
/// when it comes into view again (`Sync::refresh_follows`).
#[component]
pub(crate) fn ListOffer() -> impl IntoView {
    let t = i18n::t();
    let s = &t.bookmarks_share;
    let bookmarks = Bookmarks::expect();
    let collections = Collections::expect();
    let location = use_location();
    let navigate = use_navigate();

    // What is followed is fetched anew when the page comes and when it is seen again.
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

    // The fragment as the browser has it, as the offer of a link of today reads it.
    let fragment = RwSignal::new(String::new());
    Effect::new(move |_| {
        location.hash.track();
        fragment.set(nav::fragment());
    });
    let in_front = folia_design::stage::front();
    Effect::new(move |_| {
        let handle = window_event_listener_untyped("hashchange", move |_| {
            if in_front.get_untracked() {
                fragment.set(nav::fragment());
            }
        });
        on_cleanup(move || handle.remove());
    });
    let state = RwSignal::from(OPENED.with(Clone::clone));
    Effect::new(move |_| {
        let now = fragment.get();
        let Some(what) = opening(&now) else { return };
        if state.with_untracked(|opened| opened.as_ref().is_some_and(|(of, _)| *of == now)) {
            return;
        }
        // A one-time link this tab opened before (a reload): what it opened to.
        if let Opening::Once(parcel) = &what {
            if let Some(kept) = nav::session_get(OPENED_KEY).and_then(|text| kept_of(&text, parcel)) {
                let found = match kept {
                    Kept::Live(link) => {
                        state.set(Some((now.clone(), Found::Opening)));
                        open_fragment(Opening::Live(link), now);
                        return;
                    }
                    Kept::Copy(code) => Found::Copy(code),
                };
                state.set(Some((now, found)));
                return;
            }
        }
        state.set(Some((now.clone(), Found::Opening)));
        open_fragment(what, now);
    });
    let taken = RwSignal::new(None::<String>);
    let full = RwSignal::new(false);

    // Answered: the fragment goes from the address (in the same history entry, or for the list
    // followed its view), what the tab kept of it too.
    let leave = move |to: Option<CollectionView>| {
        nav::session_set(OPENED_KEY, "");
        state.set(None);
        full.set(false);
        fragment.set(String::new());
        // The address as written carries the language's prefix; a view's path is the app's.
        let path = match to {
            Some(view) => t.path(&BookmarksUrl { collection: view, ..BookmarksUrl::parse(&location.search.get_untracked()) }.path()),
            None => folia_shell::tabs::location_of(&location.pathname.get_untracked(), &location.search.get_untracked()),
        };
        navigate(&path, NavigateOptions { replace: true, scroll: false, ..Default::default() });
    };
    let leave = StoredValue::new_local(leave);
    let follow = move |link: String, code: String| {
        let Some(collections) = collections else { return };
        match collections.update(|shelf| shelf.follow(&link, &code, now_secs())) {
            Ok(id) => leave.with_value(|leave| leave(Some(CollectionView::Of(id)))),
            Err(Refused::Full) => full.set(true),
            Err(_) => leave.with_value(|leave| leave(None)),
        }
    };
    let take = move |code: String| {
        let (Some(bookmarks), Some(part)) = (bookmarks, ListPart::from_code(&code)) else { return };
        let added = take_part(bookmarks, collections, &part);
        taken.set(Some((s.taken)(added)));
        leave.with_value(|leave| leave(None));
    };

    move || {
        let Some((_, now)) = state.get() else {
            return taken
                .get()
                .map(|note| {
                    view! {
                        <div class="offer" role="status">
                            <Icon name="check"/>
                            <p><b>{note}</b></p>
                            <button class="mini hit" type="button" id="list-taken-ok" on:click=move |_| taken.set(None)>{s.ok}</button>
                        </div>
                    }
                })
                .into_any();
        };
        let part = match &now {
            Found::Live { code, .. } | Found::Copy(code) => ListPart::from_code(code),
            _ => None,
        };
        let holds = part.as_ref().map(|part| {
            let new = part.brought.ids.iter().filter(|id| !bookmarks.is_some_and(|bookmarks| bookmarks.is_marked(id))).count();
            (s.holds)(part.brought.ids.len(), new)
        });
        match (now, part) {
            (Found::Opening, _) => view! {
                <div class="offer" role="status" aria-busy="true"><Icon name="share-2"/><p>{s.link_opening}</p></div>
            }
            .into_any(),
            (Found::Live { link, code }, Some(part)) => {
                let name = part_name(&part, t);
                // The same link followed already: that list.
                let already = collections.and_then(|collections| collections.with(|shelf| shelf.followed_by(&link).map(|f| (f.id, followed_name(f, t)))));
                if let Some((id, name)) = already {
                    return view! {
                        <div class="offer" role="status">
                            <Icon name="users-round"/>
                            <p><b>{(s.followed_already)(&name)}</b></p>
                            <button class="mini primary hit" type="button" id="list-open" on:click=move |_| leave.with_value(|leave| leave(Some(CollectionView::Of(id))))>{s.open_list}</button>
                            <button class="mini hit" type="button" id="list-dismiss" on:click=move |_| leave.with_value(|leave| leave(None))>{s.ok}</button>
                        </div>
                    }
                    .into_any();
                }
                let (follow_link, follow_code, take_code) = (link.clone(), code.clone(), code.clone());
                view! {
                    <div class="offer" role="status">
                        <Icon name="share-2"/>
                        <p><b>{(s.live_head)(&name)}</b>{holds}</p>
                        {move || if full.get() {
                            view! { <span class="hint">{s.follow_full}</span> }.into_any()
                        } else {
                            let (link, code) = (follow_link.clone(), follow_code.clone());
                            view! { <button class="mini primary hit" type="button" id="list-follow" title=s.follow_title on:click=move |_| follow(link.clone(), code.clone())>{s.follow}</button> }.into_any()
                        }}
                        <button class="mini hit" type="button" id="list-take" title=s.take_title on:click=move |_| take(take_code.clone())>{s.take}</button>
                        <button class="mini hit" type="button" id="list-dismiss" on:click=move |_| leave.with_value(|leave| leave(None))>{s.dismiss}</button>
                    </div>
                }
                .into_any()
            }
            (Found::Copy(code), Some(part)) => {
                let name = part_name(&part, t);
                view! {
                    <div class="offer" role="status">
                        <Icon name="share-2"/>
                        <p><b>{(s.copy_head)(&name)}</b>{holds}</p>
                        <button class="mini primary hit" type="button" id="list-take" title=s.take_title on:click=move |_| take(code.clone())>{s.take}</button>
                        <button class="mini hit" type="button" id="list-dismiss" on:click=move |_| leave.with_value(|leave| leave(None))>{s.dismiss}</button>
                    </div>
                }
                .into_any()
            }
            (found, _) => {
                let text = match found {
                    Found::Problem(problem) => link_problem(problem, t),
                    Found::NotOffered => s.link_not_offered,
                    _ => s.link_unknown,
                };
                view! {
                    <div class="offer" role="alert">
                        <Icon name="triangle-alert"/>
                        <p>{text}</p>
                        <button class="mini hit" type="button" id="list-ok" on:click=move |_| leave.with_value(|leave| leave(None))>{s.ok}</button>
                    </div>
                }
                .into_any()
            }
        }
    }
}

/// Now, in seconds since 1970.
fn now_secs() -> u64 {
    #[cfg(feature = "csr")]
    return (web_sys::js_sys::Date::now() / 1000.0).max(0.0) as u64;
    #[cfg(not(feature = "csr"))]
    0
}

// ---------- a list followed ----------

/// How the offer to save the profile with Betula Sync stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Upgrade {
    Offered,
    Saving,
    Saved,
    Failed,
}

/// What stands over a list followed while the page shows it: that it is shared with the visitor
/// and stays current (or was shared, with its last state), „Alle übernehmen", and once it is not
/// shared any more „Behalten" and „Löschen". In a private profile what Betula Sync makes better of
/// it: every change at once, on every device.
#[component]
pub(crate) fn FollowedHead(followed: Memo<Option<Followed>>, go: Callback<CollectionView>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.bookmarks_share;
    let bookmarks = Bookmarks::expect();
    let collections = Collections::expect();
    let sync = Sync::expect();
    let taken = RwSignal::new(None::<(u32, usize)>);
    let upgrade = RwSignal::new(Upgrade::Offered);
    let profile = move || bookmarks.map_or(0, Bookmarks::profile);
    let private = Memo::new(move |_| sync.is_some_and(|sync| sync::offered().is_some() && !sync.state(profile()).synced()));
    let save = move |_| {
        let Some(sync) = sync else { return };
        if upgrade.get_untracked() == Upgrade::Saving {
            return;
        }
        upgrade.set(Upgrade::Saving);
        sync.activate_then(profile(), move |result| {
            let _ = upgrade.try_set(if result.is_ok() { Upgrade::Saved } else { Upgrade::Failed });
        });
    };
    move || {
        let followed = followed.get()?;
        let part = followed.part()?;
        let (id, name) = (followed.id, followed_name(&followed, t));
        let ended = followed.ended != 0;
        let asking = ended && !followed.kept;
        let line = if !ended {
            s.head_live.to_string()
        } else if followed.kept {
            (s.head_kept)(&day_of(followed.ended).day_month(t.locale))
        } else {
            (s.head_ended)(&name)
        };
        let take = move |_| {
            if let Some(bookmarks) = bookmarks {
                taken.set(Some((id, take_part(bookmarks, collections, &part))));
            }
        };
        let note = move || taken.get().filter(|(of, _)| *of == id).map(|(_, added)| view! { <span class="fl-taken">" "{(s.taken)(added)}</span> });
        // In a private profile, after its words, what Betula Sync makes better of it, with
        // „Profil speichern" beside „Alle übernehmen": one box, not two.
        let offer = move || !ended && (private.get() || upgrade.get() == Upgrade::Saved);
        let offer_text = move || {
            offer().then(|| {
                let text = match upgrade.get() {
                    Upgrade::Saved => s.saved,
                    Upgrade::Failed => s.save_failed,
                    _ => s.private,
                };
                view! { <span class="fl-up">" "{text}</span> }
            })
        };
        Some(view! {
            <div class="offer fl-head" class:warn=asking role="status">
                <Icon name=if asking { "triangle-alert" } else { "users-round" }/>
                <p><b>{line}</b>{note}{offer_text}</p>
                <button class="mini hit" type="button" id="fl-take" title=s.take_all_title on:click=take>{s.take_all}</button>
                {move || (offer() && upgrade.get() != Upgrade::Saved).then(|| view! {
                    <button class="mini hit" type="button" id="fl-save" aria-busy=move || (upgrade.get() == Upgrade::Saving).then_some("true") on:click=save>
                        {move || if upgrade.get() == Upgrade::Saving { s.saving } else { s.save_profile }}
                    </button>
                })}
                {asking.then(|| view! {
                    <button class="mini hit" type="button" id="fl-keep" on:click=move |_| { if let Some(collections) = collections { collections.update(|shelf| shelf.keep_followed(id)); } }>{s.keep}</button>
                    <button class="mini danger hit" type="button" id="fl-delete" on:click=move |_| {
                        if let Some(collections) = collections {
                            collections.update(|shelf| shelf.unfollow(id));
                        }
                        go.run(CollectionView::All);
                    }>{s.delete}</button>
                })}
            </div>
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{DE, EN};

    #[test]
    fn a_dialog_opens_with_the_choices_the_profile_has() {
        assert_eq!(Choice::first(true), Choice { live: true, once: true, notes: false });
        assert_eq!(Choice::first(false), Choice { live: false, once: false, notes: false });
        assert!(!Choice::first(false).needs_sync());
        assert!(Choice { once: true, ..Choice::first(false) }.needs_sync(), "a copy for one person is a parcel");
    }

    #[test]
    fn a_share_is_copied_again_by_its_link() {
        use folia_plans::shares::encode;
        let share = ListShare { id: encode(&[1; 16]), collection: 0, made: 0, live: true, once: true, notes: false, key: encode(&[2; 32]), parcel: encode(&[3; 48]), name: String::new() };
        assert_eq!(path_of(&share), Some(list_once_path(&encode(&[3; 48]))), "while its parcel waits, the one-time link");
        let opened = ListShare { parcel: String::new(), ..share.clone() };
        assert_eq!(path_of(&opened), Some(list_live_path(&opened.link().unwrap())));
        assert!(path_of(&opened).unwrap().starts_with("/bookmarks#live="));
        assert_eq!(path_of(&ListShare { live: false, ..opened }), None, "a copy opened is nothing to copy again");
    }

    /// What a one-time link opened to is kept for its parcel alone, and read back as it was.
    #[test]
    fn what_a_one_time_link_opened_to_is_kept_for_the_tab() {
        let link = folia_plans::shares::encode(&[3u8; 48]);
        let parcel = folia_plans::shares::encode(&[5u8; 48]);
        assert_eq!(kept_of(&kept_text(&parcel, &Kept::Live(link.clone())), &parcel), Some(Kept::Live(link.clone())));
        assert_eq!(kept_of(&kept_text(&parcel, &Kept::Live(link)), &folia_plans::shares::encode(&[6u8; 48])), None, "another parcel's");
        let code = ListPart { name: "Für Lena".into(), brought: Brought::modules(&["14169".to_string()]) }.code().unwrap();
        assert_eq!(kept_of(&kept_text(&parcel, &Kept::Copy(code.clone())), &parcel), Some(Kept::Copy(code)));
        assert_eq!(kept_of(&format!("{parcel}\tcopy\tnot-a-code"), &parcel), None);
        assert_eq!(kept_of("", &parcel), None);
    }

    /// A link that opens nothing says why, each problem in its own words, in every language.
    #[test]
    fn a_link_that_opens_nothing_says_why() {
        assert_eq!(link_problem(ShareProblem::Ended, &DE), "Diese Liste wird nicht mehr geteilt.");
        let all = [ShareProblem::Taken, ShareProblem::Expired, ShareProblem::Withdrawn, ShareProblem::Ended, ShareProblem::Unknown, ShareProblem::Busy, ShareProblem::Unreachable, ShareProblem::Unsupported];
        for t in [&DE, &EN] {
            let texts: std::collections::BTreeSet<&str> = all.iter().map(|problem| link_problem(*problem, t)).collect();
            assert_eq!(texts.len(), all.len());
        }
    }

    #[test]
    fn a_whole_merkliste_shared_has_a_name_of_its_own() {
        let whole = ListPart::default();
        assert_eq!(part_name(&whole, &DE), "Geteilte Merkliste");
        assert_eq!(part_name(&ListPart { name: "Für Lena".into(), ..whole }, &EN), "Für Lena");
        assert_eq!((DE.bookmarks_share.holds)(3, 1), "3 Module, eins davon fehlt in deiner Merkliste.");
        assert_eq!((EN.bookmarks_share.holds)(1, 0), "1 module, all of them saved already.");
    }
}
