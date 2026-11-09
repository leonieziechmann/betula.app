//! „Profil" (owner, 2026-10-14: „eine seite, wo man seine ganzen informationen/einstellungen
//! verwalten kann"): the profiles of this browser and the active one, in one place. Since 2026-11-08
//! (folia/design/profil/konzepte.html, the second round; owner: „die profil unabhängigen
//! einstellungen wo anders hin … ich brauche nicht noch ein link mehr zu mein studium … betula sync,
//! was ich persönlich als sehr essentiell einstufen würde") the page is what a profile is: who, what
//! it holds, on which devices; what every profile shares (light or dark, the language) is in the menu
//! of the profiles (`folia_shell::chrome::Preferences`), and the page links no tab.
//!
//! - The sidebar holds what belongs to the page as a whole (R17): the profiles — a click switches
//!   to one, in every tab of the browser (`folia_stores::profiles`) — and „Profil hinzufügen": an
//!   empty one, one from another device (the recovery code, `sync::FromOther`), one from a file. On
//!   a phone the sidebar stands first as a row: the profiles as chips, „+ Profil" at its end.
//! - The head is the active profile: its badge, its name with the pencil to rename it in place,
//!   „Mein Studiengang" as a line of text (changed where it is set, in „Mein Studium"), since when it
//!   is, and its numbers: saved, planned, passed with the credits, collections and notes, saved
//!   plans. None of them is a link.
//! - Betula Sync, where the site offers it (`sync::SyncPart`), `#devices`.
//! - „Daten": the active profile as a file („Als Datei sichern", `Backup`).
//! - „Gefahrenzone": „Profil leeren" and „Profil löschen", each a row with its icon on a tile and a
//!   button of one width with the same icon, the question under the row; „Rückgängig" after, until
//!   the page is left. The only profile is emptied instead of deleted, and says so before.
//! - At its end a line: the Impressum and the Datenschutz (§ 5 DDG: every page leads to the profile
//!   page, the rail and the head of a phone's page; the menu of the profiles names them too), the
//!   versions and how fresh the data is, where it comes from.
//! - A switch, a profile made, a file taken over answer in the next frame (R21): the sidebar marks
//!   the profile at once, every store reads its texts after.
//!
//! The page exists in the browser app only. Which profiles there are and what they keep is known
//! to this browser and to nobody else (R20): the server renders the same explanation for everybody
//! (R9), `noindex`, and the address is the page and says nothing more. A backup is made and read
//! in the browser and reaches no server.

use folia_calendar::day::Day;
use folia_calendar::select::TownChoice;
use folia_calendar::semester::SemesterKey;
use folia_locale::Locale;
use folia_pages::ask::{GroundAsk, MetaAsk, StudyplanModulesAsk};
use folia_plans::study;
use folia_routes::url;
use folia_stores::bookmarks::Bookmarks;
use folia_stores::myprogram::{program_name, MineResolved, MyProgram};
use folia_stores::profiles::{Avatar, Backup, Cleared, Deleted, NoBackup, Profile, Profiles, Summary, MAX_NAME, MAX_PROFILES};
use folia_stores::studyplan::{Saved, Studyplan};
use leptos::prelude::*;
use folia_design::stage::Title;

use crate::confirm::{Confirm, Question};
use crate::i18n::{self, Texts};
use folia_data::use_ask;
use folia_design::format;
use folia_design::nav;
use folia_design::ui::Icon;
use folia_shell::frame::Frame;
use folia_shell::pending::Shape;
use folia_shell::seo::Seo;
use folia_shell::skeleton::AppStandin;

/// Whether this build is the browser app: only there are there profiles.
const APP: bool = cfg!(feature = "csr");

/// The ids the focus is moved to.
const RENAME_ID: &str = "pf-rename";
const NAME_ID: &str = "pf-name";
const ADD_ID: &str = "pf-add";
const NEW_ID: &str = "pf-new";
const NEW_NAME_ID: &str = "pf-new-name";
const OTHER_ID: &str = "pf-other";
const CLEAR_ID: &str = "pf-clear";
const CLEAR_YES_ID: &str = "pf-clear-yes";
const DELETE_ID: &str = "pf-delete";
const DELETE_YES_ID: &str = "pf-delete-yes";
const OPEN_ID: &str = "pf-open";
const UNDO_ID: &str = "pf-undo";

/// What the page says of the last thing done on it, at its head; a profile deleted or emptied
/// comes back from there with „Rückgängig" until the page is left.
#[derive(Clone, Debug, PartialEq)]
enum Note {
    /// An empty profile was made, and is the active one: its name.
    Created(String),
    /// A profile was made of a file, and is the active one: its name.
    Opened(String),
    /// A profile was deleted: what puts it back, and its name.
    Deleted(Deleted, String),
    /// The active profile was emptied: what puts it back.
    Cleared(Cleared),
}

/// The tags of the profile page: a page of one visitor, the same address for everybody and nothing
/// to list.
#[component]
fn ProfileSeo() -> impl IntoView {
    let t = i18n::t();
    view! { <Seo title=t.profile.title description=t.profile.description path=url::PROFILE noindex=true/> }
}

#[component]
pub fn ProfilePage() -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    // A route of the app: the server writes its document, the app the page (§4.2). Without
    // JavaScript the page offers the other language, which the menu of the profiles holds.
    if !APP {
        return view! {
            <Title text=p.title/>
            <ProfileSeo/>
            <AppStandin shape=Shape::Profile title=p.server_title hint=p.server_hint>
                {Locale::ALL.iter().copied().filter(|locale| *locale != t.locale).map(|locale| view! {
                    <a class="btn secondary" href=locale.path(url::PROFILE) hreflang=locale.code() lang=locale.code() rel="alternate external nofollow" data-language=locale.code()>
                        <Icon name="languages"/>{locale.name()}
                    </a>
                }).collect_view()}
                // The legal pages, which the page leads to without JavaScript as well.
                <a class="btn secondary" href=t.path(url::IMPRINT)>{t.ground.imprint}</a>
                <a class="btn secondary" href=t.path(url::PRIVACY)>{t.ground.privacy}</a>
            </AppStandin>
        }
        .into_any();
    }
    let note = RwSignal::new(None::<Note>);
    let kept = Kept::new();
    // „Gerät verbinden", which Betula Sync and „Profil hinzufügen" open, either way.
    provide_context(crate::sync::ConnectDialog(RwSignal::new(None)));
    let sidebar = move || view! { <Sidebar note/> };
    // „Geräte" in the menu of the profiles leads to `#devices`: the page scrolls there, and the part
    // lights up once.
    let location = folia_design::i18n::use_location();
    Effect::new(move |_| {
        let hash = location.hash.get();
        if hash.trim_start_matches('#') == url::DEVICES {
            request_animation_frame(to_devices);
        }
    });
    view! {
        <Title text=p.title/>
        <Frame title=p.profiles sidebar sidebar_first=true>
            <div class="page-inner pf">
                <ProfileSeo/>
                <Notice note/>
                <Head kept/>
                <crate::sync::SyncPart empty=kept.empty/>
                <DataPart/>
                <DangerPart note kept/>
                <Foot/>
            </div>
        </Frame>
    }
    .into_any()
}

/// Scrolls the page to Betula Sync and lets it light up once (`#devices`).
fn to_devices() {
    #[cfg(feature = "csr")]
    {
        let Some(window) = web_sys::window() else { return };
        let Some(part) = window.document().and_then(|document| document.get_element_by_id(url::DEVICES)) else { return };
        let reduced = window.match_media("(prefers-reduced-motion: reduce)").ok().flatten().is_some_and(|query| query.matches());
        let options = web_sys::ScrollIntoViewOptions::new();
        options.set_block(web_sys::ScrollLogicalPosition::Start);
        options.set_behavior(if reduced { web_sys::ScrollBehavior::Instant } else { web_sys::ScrollBehavior::Smooth });
        part.scroll_into_view_with_scroll_into_view_options(&options);
        let list = part.class_list();
        let _ = list.remove_1("pf-flash");
        let _ = part.get_bounding_client_rect();
        let _ = list.add_1("pf-flash");
    }
}

/// What the active profile keeps, counted, each a memo of its own (R5): the head's numbers and
/// whether it is empty (the Gefahrenzone, Betula Sync of a new device).
#[derive(Clone, Copy)]
struct Kept {
    marked: Memo<usize>,
    /// The Merkliste's collections and notes (`folia_stores::collections`).
    shelf: Memo<(usize, usize)>,
    /// The modules planned, each once, and the semesters they are in.
    planned: Memo<(usize, usize)>,
    /// The modules passed.
    passed: Memo<Vec<String>>,
    plans: Memo<usize>,
    empty: Memo<bool>,
}

impl Kept {
    fn new() -> Self {
        let bookmarks = Bookmarks::expect();
        let plan = Studyplan::expect();
        let mine = MyProgram::expect();
        let saved = Saved::open();
        let shelf = folia_stores::collections::Collections::expect();
        let marked = Memo::new(move |_| bookmarks.map_or(0, Bookmarks::count));
        let kept = Memo::new(move |_| shelf.map_or((0, 0), |shelf| shelf.with(|shelf| (shelf.collections().len(), shelf.notes_count()))));
        let planned = Memo::new(move |_| plan.map_or((0, 0), |plan| (plan.count(), plan.with(|doc| doc.semesters().len()))));
        let passed = Memo::new(move |_| plan.map(|plan| plan.with(|doc| doc.passed.iter().map(|passed| passed.module_id.clone()).collect())).unwrap_or_default());
        let plans = Memo::new(move |_| saved.with(|saved| saved.plans.len()));
        let program = Memo::new(move |_| mine.is_some_and(|mine| mine.with(|doc| doc.program.is_some() || doc.start.is_some())));
        let empty = Memo::new(move |_| marked.get() == 0 && kept.get() == (0, 0) && planned.get().0 == 0 && passed.with(Vec::is_empty) && plans.get() == 0 && !program.get());
        Kept { marked, shelf: kept, planned, passed, plans, empty }
    }
}

/// The note of the last thing done, with „Rückgängig" where it can be taken back.
#[component]
fn Notice(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let undo = move |_| {
        if let Some(profiles) = profiles {
            match note.get_untracked() {
                Some(Note::Deleted(deleted, _)) => {
                    profiles.undo_delete(&deleted);
                }
                Some(Note::Cleared(cleared)) => profiles.undo_clear(&cleared),
                _ => {}
            }
        }
        note.set(None);
    };
    move || {
        note.get().map(|now| {
            let (icon, said, undoable) = match &now {
                Note::Created(name) => ("user-round-plus", (p.created)(name), false),
                Note::Opened(name) => ("upload", (p.opened)(name), false),
                Note::Deleted(_, name) => ("trash-2", (p.deleted)(name), true),
                Note::Cleared(_) => ("check", p.cleared.to_string(), true),
            };
            view! {
                <div class="offer pf-note" role="status">
                    <Icon name=icon/>
                    <p>{said}</p>
                    {undoable.then(|| view! { <button class="mini hit" type="button" id=UNDO_ID on:click=undo>{t.common.undo}</button> })}
                    <button class="mini hit" type="button" on:click=move |_| note.set(None)>{p.ok}</button>
                </div>
            }
        })
    }
}

/// The active profile: its badge, its name with the pencil, „Mein Studiengang" as a line, since
/// when, and its numbers.
#[component]
fn Head(kept: Kept) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let current = Memo::new(move |_| profiles.map(Profiles::current));
    let renaming = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    // Another profile (switched to in the sidebar or in another tab): what was begun for the one
    // before is closed.
    Effect::new(move |before: Option<Option<u16>>| {
        let id = current.with(|profile| profile.as_ref().map(|profile| profile.id));
        if before.is_some_and(|before| before != id) {
            renaming.set(false);
        }
        id
    });
    let start_renaming = move |_| {
        name.set(current.with_untracked(|profile| profile.as_ref().map(|profile| profile.name.clone()).unwrap_or_default()));
        renaming.set(true);
        request_animation_frame(|| nav::focus_by_id(NAME_ID));
    };
    let stop_renaming = move || {
        renaming.set(false);
        request_animation_frame(|| nav::focus_by_id(RENAME_ID));
    };
    let rename = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if let (Some(profiles), Some(profile)) = (profiles, current.get_untracked()) {
            profiles.rename(profile.id, &name.get_untracked());
        }
        stop_renaming();
    };
    view! {
        <section class="panel pf-head" aria-label=p.active>
            {move || current.get().map(|profile| {
                let shown = profile.shown(t.locale);
                let unnamed = (t.profiles.unnamed)(profile.id);
                let made = day_of(profile.at).map(|day| (p.made_on)(&day.date(t.locale)));
                view! {
                    <div class="pf-head-main">
                        <Avatar profile=profile.clone() large=true/>
                        <div class="pf-head-text">
                            {move || if renaming.get() {
                                let unnamed = unnamed.clone();
                                view! {
                                    <form class="pf-rename" on:submit=rename>
                                        <input
                                            id=NAME_ID
                                            type="text"
                                            maxlength=MAX_NAME.to_string()
                                            aria-label=p.name
                                            placeholder=unnamed
                                            autocomplete="off"
                                            prop:value=move || name.get()
                                            on:input=move |ev| name.set(event_target_value(&ev))
                                            on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                                if ev.key() == "Escape" {
                                                    stop_renaming();
                                                }
                                            }
                                        />
                                        <button class="mini primary hit" type="submit">{p.save}</button>
                                        <button class="mini hit" type="button" on:click=move |_| stop_renaming()>{p.cancel}</button>
                                    </form>
                                }
                                .into_any()
                            } else {
                                view! {
                                    <div class="pf-name">
                                        <h2>{shown.clone()}</h2>
                                        <button class="pf-pencil" type="button" id=RENAME_ID aria-label=p.rename title=p.rename on:click=start_renaming><Icon name="pencil"/></button>
                                    </div>
                                }
                                .into_any()
                            }}
                            <p class="pf-line"><ProgramLine/></p>
                            {made.map(|made| view! { <p class="pf-made">{made}</p> })}
                        </div>
                    </div>
                }
            })}
            <Stats kept/>
        </section>
    }
}

/// „Mein Studiengang" of the active profile as a line: the program, its Studienrichtung, the
/// Fachsemester (else the Studienbeginn), the Standort where one is chosen. Changed where it is set,
/// in „Mein Studium"; the page links it no more.
#[component]
fn ProgramLine() -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let mine = MyProgram::expect();
    let resolved = MineResolved::expect();
    // The current semester, for the Fachsemester.
    let meta = use_ask(|| Some(MetaAsk {}));
    let now = Memo::new(move |_| meta.with(|meta| meta.as_ref().and_then(|meta| meta.as_ref().ok()).and_then(|meta| meta.current_semester.as_deref().and_then(SemesterKey::parse))));
    let doc = Memo::new(move |_| mine.map(MyProgram::get).unwrap_or_default());
    let known = Memo::new(move |_| resolved.and_then(|resolved| resolved.0.get()));
    move || {
        let doc = doc.get();
        let Some(id) = doc.program.clone() else { return p.no_program.to_string() };
        // The program as the catalog knows it; one gone from the snapshot as it was kept.
        let name = match known.get() {
            Some(info) if info.exact => program_name(&info.program),
            _ => doc.name.clone().unwrap_or(id),
        };
        let mut parts = vec![name];
        parts.extend(doc.direction.clone().or_else(|| doc.caption.clone()).filter(|text| !text.trim().is_empty()));
        if let Some(start) = doc.start {
            parts.push(match now.get().and_then(|now| study::fs_of(start, now, &doc.leave)) {
                Some(fs) => (p.fs)(fs),
                None => start.label(t.locale),
            });
        }
        match doc.town {
            TownChoice::Only(town) => parts.push(town.label().to_string()),
            TownChoice::Both => parts.push(p.town_both.to_string()),
            TownChoice::Derive => {}
        }
        parts.join(" · ")
    }
}

/// The numbers of the active profile, each a value over its label; none is a link. The credits of
/// what is passed come from the catalog (the count stands in until they are there).
#[component]
fn Stats(kept: Kept) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let rows = use_ask(move || {
        let ids = kept.passed.get();
        (!ids.is_empty()).then_some(StudyplanModulesAsk { ids })
    });
    let credits = Memo::new(move |_| rows.with(|rows| rows.as_ref().and_then(|rows| rows.as_ref().ok()).map(|(rows, _)| rows.iter().filter_map(|row| row.credits).sum::<f64>())));
    let stat = |value: Signal<String>, label: Signal<String>| view! { <div class="pf-stat"><b>{value}</b><span>{label}</span></div> };
    move || {
        if kept.empty.get() {
            return view! { <p class="pf-stats-empty">{p.stats_empty}</p> }.into_any();
        }
        view! {
            <div class="pf-stats">
                {stat(Signal::derive(move || kept.marked.get().to_string()), Signal::derive(move || p.stat_marked.to_string()))}
                {stat(Signal::derive(move || kept.planned.get().0.to_string()), Signal::derive(move || (p.stat_planned)(kept.planned.get().1)))}
                {stat(
                    Signal::derive(move || {
                        let passed = kept.passed.with(Vec::len);
                        match credits.get() {
                            Some(credits) if passed > 0 => format::credits(Some(credits), t.locale),
                            _ => passed.to_string(),
                        }
                    }),
                    Signal::derive(move || (p.stat_passed)(kept.passed.with(Vec::len))),
                )}
                {stat(Signal::derive(move || { let (collections, notes) = kept.shelf.get(); format!("{collections} · {notes}") }), Signal::derive(move || p.stat_kept.to_string()))}
                {stat(Signal::derive(move || kept.plans.get().to_string()), Signal::derive(move || p.stat_plans.to_string()))}
            </div>
        }
        .into_any()
    }
}

/// A row of „Daten" and of the Gefahrenzone: its icon on a tile, what it does and a sentence, a
/// button of one width with the same icon.
#[component]
fn ActionRow(
    icon: &'static str,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] line: Signal<String>,
    #[prop(into)] label: Signal<String>,
    id: &'static str,
    #[prop(optional)] danger: bool,
    #[prop(optional, into)] disabled: Signal<Option<&'static str>>,
    #[prop(optional, into)] expanded: Option<Signal<bool>>,
    on_press: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="pf-arow">
            <span class="pf-tile" class:danger=danger aria-hidden="true"><Icon name=icon/></span>
            <span class="pf-arow-text"><b>{title}</b><span>{line}</span></span>
            <button
                class=if danger { "btn secondary pf-abtn danger" } else { "btn secondary pf-abtn" }
                type="button"
                id=id
                aria-disabled=move || disabled.get().map(|_| "true")
                title=move || disabled.get()
                aria-haspopup=expanded.map(|_| "dialog")
                aria-expanded=move || expanded.map(|open| if open.get() { "true" } else { "false" })
                on:click=move |_| on_press.run(())
            >
                <Icon name=icon/><span>{label}</span>
            </button>
        </div>
    }
}

/// „Daten": the active profile as a file, which reaches no server.
#[component]
fn DataPart() -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let save = Callback::new(move |()| {
        let Some(backup) = profiles.and_then(|profiles| profiles.backup(profiles.active_untracked(), t.locale)) else { return };
        download(&file_name(&backup.name, today()), &backup.file());
    });
    view! {
        <section class="panel pf-part" aria-labelledby="pf-data">
            <h2 id="pf-data"><Icon name="database"/>{p.data_heading}</h2>
            <div class="pf-arows">
                <ActionRow icon="download" title=p.save_file.to_string() line=p.save_file_hint.to_string() label=p.save_button.to_string() id="pf-save" on_press=save/>
            </div>
        </section>
    }
}

/// „Gefahrenzone": „Profil leeren" and „Profil löschen", the question under the row, „Rückgängig"
/// after in the note at the head. The only profile is emptied instead, and says so before.
#[component]
fn DangerPart(note: RwSignal<Option<Note>>, kept: Kept) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let current = Memo::new(move |_| profiles.map(Profiles::current));
    let only = Memo::new(move |_| profiles.map_or(1, Profiles::count) < 2);
    // The question, in a menu at the button that asked (`confirm`).
    let asking = RwSignal::new(None::<Question>);
    // Another profile: the question was about the one before.
    Effect::new(move |before: Option<u16>| {
        let id = profiles.map_or(0, Profiles::active);
        if before.is_some_and(|before| before != id) {
            asking.set(None);
        }
        id
    });
    let shown = move || current.with(|profile| profile.as_ref().map(|profile| profile.shown(t.locale)).unwrap_or_default());
    let clear = move || {
        let Some(profiles) = profiles else { return };
        let cleared = profiles.clear(profiles.active_untracked());
        note.set(Some(Note::Cleared(cleared)));
        request_animation_frame(|| nav::focus_by_id(UNDO_ID));
    };
    let delete = move || {
        let (Some(profiles), Some(profile)) = (profiles, current.get_untracked()) else { return };
        if untrack(|| only.get()) {
            clear();
            return;
        }
        if let Some(deleted) = profiles.delete(profile.id) {
            note.set(Some(Note::Deleted(deleted, profile.shown(t.locale))));
            request_animation_frame(|| nav::focus_by_id(UNDO_ID));
        }
    };
    let ask = move |kind: &'static str| {
        Callback::new(move |()| {
            if kind == CLEAR_ID && kept.empty.get_untracked() {
                return;
            }
            // The only profile is emptied instead of deleted, and asked about so.
            let deleting = kind == DELETE_ID && !only.get_untracked();
            let name = untrack(shown);
            let question = if deleting {
                Question { at: kind.to_string(), title: (p.delete_question)(&name), hint: p.delete_after.to_string(), yes: p.delete_yes.to_string(), icon: "trash-2", yes_id: DELETE_YES_ID }
            } else {
                Question { at: kind.to_string(), title: (p.clear_question)(&name), hint: p.clear_after.to_string(), yes: p.clear_yes.to_string(), icon: "rotate-ccw", yes_id: CLEAR_YES_ID }
            };
            asking.set(Some(question));
        })
    };
    let asked = move |kind: &'static str| Signal::derive(move || asking.with(|asking| asking.as_ref().is_some_and(|q| q.at == kind)));
    view! {
        <section class="panel pf-part pf-danger" aria-labelledby="pf-danger">
            <h2 id="pf-danger"><Icon name="triangle-alert"/>{p.danger_heading}</h2>
            <p class="pf-sub">{p.danger_hint}</p>
            <div class="pf-arows">
                <ActionRow
                    icon="rotate-ccw"
                    title=p.clear.to_string()
                    line=Signal::derive(move || (p.clear_hint)(&shown()))
                    label=p.clear_button.to_string()
                    id=CLEAR_ID
                    danger=true
                    disabled=Signal::derive(move || kept.empty.get().then_some(p.nothing_to_clear))
                    expanded=asked(CLEAR_ID)
                    on_press=ask(CLEAR_ID)
                />
                <ActionRow
                    icon="trash-2"
                    title=p.delete.to_string()
                    line=Signal::derive(move || if only.get() { p.delete_only.to_string() } else { (p.delete_hint)(&shown()) })
                    label=Signal::derive(move || if only.get() { p.clear_button } else { p.delete_button }.to_string())
                    id=DELETE_ID
                    danger=true
                    expanded=asked(DELETE_ID)
                    on_press=ask(DELETE_ID)
                />
            </div>
            <Confirm asking on_yes=Callback::new(move |yes: &'static str| if yes == DELETE_YES_ID { delete() } else { clear() })/>
        </section>
    }
}

/// The page's last line: what the ground at the end of every page held until 2026-11-03, now that
/// only the start page and the pages about Betula have one (`folia_shell::ground`) — that Betula is
/// not the BTU's, the Impressum and the Datenschutz, the versions of Folia and Radix, how fresh the data is and where it comes
/// from. The data's facts once they are there; the rest stands from the first frame.
#[component]
fn Foot() -> impl IntoView {
    let t = i18n::t();
    let facts = use_ask(|| Some(GroundAsk {}));
    let radix = move || facts.get().and_then(Result::ok).and_then(|ground| ground.meta.radix_version.clone());
    let changed = move || facts.get().and_then(Result::ok).and_then(|ground| ground.meta.data_changed_at.as_deref().map(|date| format::date(date, t.locale)));
    view! {
        <footer class="pf-foot" aria-label=t.ground.legal>
            <span class="pf-foot-note">{t.ground.note}</span>
            <a href=t.path(url::IMPRINT)>{t.ground.imprint}</a>
            <a href=t.path(url::PRIVACY)>{t.ground.privacy}</a>
            <span>"Folia "<span class="ver">{folia_shell::document::VERSION}</span></span>
            {move || radix().map(|version| view! { <span>"Radix "<span class="ver">{version}</span></span> })}
            {move || changed().map(|date| view! { <span>{(t.ground.data_of)(&date)}</span> })}
            <a href=folia_shell::seo::UNIVERSITY_URL rel="noopener">{t.ground.source}<Icon name="arrow-up-right"/></a>
        </footer>
    }
}

/// The sidebar: the profiles and „Profil hinzufügen"; where all of it lives. On a phone a row.
#[component]
fn Sidebar(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    // Where the site offers Betula Sync, a profile may reach the server, sealed.
    let hint = if folia_stores::sync::offered().is_some() { p.storage_hint_sync } else { p.storage_hint };
    let adding = RwSignal::new(false);
    view! {
        <div class="pf-side">
            <div class="pf-pick">
                <ProfileList/>
                <button class="pf-item pf-add-way" type="button" id=ADD_ID aria-expanded=move || if adding.get() { "true" } else { "false" } on:click=move |_| adding.update(|open| *open = !*open)>
                    <Icon name="user-round-plus"/>
                    <span class="pf-item-text"><b>{p.add_profile}</b></span>
                    <Icon name="chevron-down"/>
                </button>
            </div>
            {move || adding.get().then(|| view! { <AddProfile note/> })}
            <p class="hint storage-hint"><Icon name="shield-check"/><span>{hint}</span></p>
        </div>
    }
}

/// The profiles of this browser: a click makes one the active profile.
#[component]
fn ProfileList() -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let all = Memo::new(move |_| profiles.map(Profiles::all).unwrap_or_default());
    let active = Memo::new(move |_| profiles.map_or(0, Profiles::active));
    // A click marks the profile at once; the switch follows after the next frame (R21): every
    // store reads the profile's texts then, and what the pages show follows from them.
    let going = RwSignal::new(None::<u16>);
    let checked = Memo::new(move |_| going.get().unwrap_or_else(|| active.get()));
    let switch = move |id: u16| {
        if checked.get_untracked() == id {
            return;
        }
        going.set(Some(id));
        nav::after_paint(move || {
            if let Some(profiles) = profiles {
                profiles.switch(id);
            }
            going.try_set(None);
        });
    };
    // The line under each profile: its „Mein Studiengang", the active one's as its store has it.
    let programs = Memo::new(move |_| profiles.map(|profiles| profiles.program_lines(t.locale)).unwrap_or_default());
    view! {
        <div class="pf-list" role="radiogroup" aria-label=p.profiles>
            <For
                each=move || all.get()
                key=|profile| (profile.id, profile.name.clone())
                children=move |profile: Profile| {
                    let id = profile.id;
                    let shown = profile.shown(t.locale);
                    let line = Memo::new(move |_| programs.with(|programs| programs.iter().find(|(of, _)| *of == id).map(|(_, line)| line.clone()).unwrap_or_default()));
                    let on = Memo::new(move |_| checked.get() == id);
                    view! {
                        <button class="pf-item" type="button" role="radio" aria-checked=move || if on.get() { "true" } else { "false" } on:click=move |_| switch(id)>
                            <Avatar profile/>
                            <span class="pf-item-text">
                                <b>{shown}</b>
                                <small>{line}</small>
                            </span>
                            <Icon name="check"/>
                        </button>
                    }
                }
            />
        </div>
    }
}

/// „Profil hinzufügen": an empty one (a name; none: „Profil 3"), one from another device (where
/// the site offers Betula Sync: on a wide screen the dialog „Gerät verbinden" the other way, a code
/// the phone scans; on a phone the scanner), one from a file.
#[component]
fn AddProfile(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let offered = folia_stores::sync::Sync::expect().is_some() && folia_stores::sync::offered().is_some();
    let dialog = use_context::<crate::sync::ConnectDialog>().filter(|_| offered);
    view! {
        <div class="pf-add">
            <NewProfile note/>
            {dialog.map(|dialog| view! {
                <button class="action only-desk" type="button" id=OTHER_ID on:click=move |_| dialog.0.set(Some(crate::sync::Direction::Get))>
                    <Icon name="monitor-smartphone"/><span>{p.add_other}</span>
                </button>
                <a class="action only-phone" href=t.path(url::CONNECT) rel="nofollow">
                    <Icon name="scan-line"/><span>{p.add_other}</span>
                </a>
            })}
            <OpenFile note/>
        </div>
    }
}

/// „Leer anlegen": a name (none: „Profil 3"), and the new profile, empty, is the active one.
#[component]
fn NewProfile(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let room = Memo::new(move |_| profiles.is_some_and(Profiles::room));
    let next = Memo::new(move |_| profiles.and_then(Profiles::next_id));
    let naming = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let start = move |_| {
        if room.get_untracked() {
            name.set(String::new());
            naming.set(true);
            request_animation_frame(|| nav::focus_by_id(NEW_NAME_ID));
        }
    };
    let stop = move || {
        naming.set(false);
        request_animation_frame(|| nav::focus_by_id(NEW_ID));
    };
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let typed = name.get_untracked();
        naming.set(false);
        let Some(profiles) = profiles else { return };
        // The field goes at once; the profile, and every store reading its empty texts, after the
        // next frame (R21).
        nav::after_paint(move || {
            if profiles.create(&typed).is_some() {
                note.try_set(Some(Note::Created(untrack(|| profiles.current()).shown(t.locale))));
            }
            request_animation_frame(|| nav::focus_by_id(NEW_ID));
        });
    };
    move || {
        if naming.get() {
            view! {
                <form class="sp-save pf-new" on:submit=create>
                    <input
                        id=NEW_NAME_ID
                        type="text"
                        maxlength=MAX_NAME.to_string()
                        aria-label=p.new_name
                        placeholder=move || next.get().map(|id| (t.profiles.unnamed)(id)).unwrap_or_default()
                        autocomplete="off"
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                            if ev.key() == "Escape" {
                                stop();
                            }
                        }
                    />
                    <button class="mini primary hit" type="submit">{p.create}</button>
                    <button class="mini hit" type="button" on:click=move |_| stop()>{p.cancel}</button>
                </form>
            }
            .into_any()
        } else {
            view! {
                <button
                    class="action"
                    type="button"
                    id=NEW_ID
                    aria-disabled=move || (!room.get()).then_some("true")
                    title=move || (!room.get()).then(|| (p.full)(MAX_PROFILES))
                    on:click=start
                >
                    <Icon name="plus"/><span>{p.add_empty}</span>
                </button>
            }
            .into_any()
        }
    }
}

/// Why a file picked makes no profile. (Only the browser reads a file: the server's build makes no
/// `File`.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
enum Refused {
    File(NoBackup),
    /// There are `MAX_PROFILES` already.
    Full,
}

/// „Aus Datei laden": a profile made of a file (`Backup`), which is offered before anything is made.
#[component]
fn OpenFile(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let input = NodeRef::<leptos::html::Input>::new();
    // What a file picked holds, until it is taken over or put aside.
    let offer = RwSignal::new(None::<Result<Backup, Refused>>);
    let pick = move |_| {
        offer.set(None);
        #[cfg(feature = "csr")]
        if let Some(input) = input.get_untracked() {
            input.click();
        }
    };
    let picked = move |_: leptos::ev::Event| {
        #[cfg(feature = "csr")]
        read_file(input, offer);
    };
    let take = move |_| {
        let (Some(profiles), Some(Ok(backup))) = (profiles, offer.get_untracked()) else { return };
        if !untrack(|| profiles.room()) {
            offer.set(Some(Err(Refused::Full)));
            return;
        }
        offer.set(None);
        // The offer goes at once; the profile, and every store reading its texts, after the next
        // frame (R21).
        nav::after_paint(move || {
            if profiles.create_from(&backup).is_some() {
                note.try_set(Some(Note::Opened(untrack(|| profiles.current()).shown(t.locale))));
            }
            request_animation_frame(|| nav::focus_by_id(OPEN_ID));
        });
    };
    let put_aside = move |_| {
        offer.set(None);
        request_animation_frame(|| nav::focus_by_id(OPEN_ID));
    };
    view! {
        <button class="action" type="button" id=OPEN_ID on:click=pick>
            <Icon name="upload"/><span>{p.open_file}</span>
        </button>
        <input class="visually-hidden" type="file" accept=".json,application/json" tabindex="-1" aria-hidden="true" node_ref=input on:change=picked/>
        {move || offer.get().map(|offer| match offer {
            Ok(backup) => {
                let holds = summary_line(&backup.summary(), t);
                view! {
                    <div class="pf-offer" role="status">
                        <p><b>{(p.file_named)(&backup.name)}</b>" "{holds}</p>
                        <p>{p.file_question}</p>
                        <div>
                            <button class="mini primary hit" type="button" id="pf-take" on:click=take>{p.file_take}</button>
                            <button class="mini hit" type="button" on:click=put_aside>{p.cancel}</button>
                        </div>
                    </div>
                }
                .into_any()
            }
            Err(refused) => {
                let why = match refused {
                    Refused::File(NoBackup::Unreadable) => p.file_unreadable.to_string(),
                    Refused::File(NoBackup::Newer) => p.file_newer.to_string(),
                    Refused::File(NoBackup::TooLarge) => p.file_too_large.to_string(),
                    Refused::Full => (p.file_full)(MAX_PROFILES),
                };
                view! {
                    <p class="action note-action pf-refused" role="status">
                        <Icon name="triangle-alert"/><span>{why}</span>
                        <button class="mini hit" type="button" on:click=put_aside>{p.ok}</button>
                    </p>
                }
                .into_any()
            }
        })}
    }
}

/// Reads the file picked into `offer`: the backup it holds, or why it holds none. Only in the
/// browser.
#[cfg(feature = "csr")]
fn read_file(input: NodeRef<leptos::html::Input>, offer: RwSignal<Option<Result<Backup, Refused>>>) {
    use folia_stores::profiles::MAX_BACKUP;
    let Some(input) = input.get_untracked() else { return };
    let Some(file) = input.files().and_then(|files| files.get(0)) else { return };
    // The same file picked once more is a change again.
    input.set_value("");
    if file.size() > MAX_BACKUP as f64 {
        offer.set(Some(Err(Refused::File(NoBackup::TooLarge))));
        return;
    }
    let text = file.text();
    leptos::task::spawn_local(async move {
        let read = wasm_bindgen_futures::JsFuture::from(text).await.ok().and_then(|text| text.as_string());
        offer.try_set(Some(read.map_or(Err(Refused::File(NoBackup::Unreadable)), |text| Backup::read(&text).map_err(Refused::File))));
    });
}

/// „12 gemerkte Module, 3 Sammlungen, 8 Module geplant, Informatik B.Sc. · PO 2008.": what a backup
/// holds, in a line.
fn summary_line(summary: &Summary, t: &Texts) -> String {
    let p = &t.profile;
    if summary.is_empty() {
        return p.file_empty.to_string();
    }
    let mut parts = Vec::new();
    if summary.marked > 0 {
        parts.push((p.n_marked)(summary.marked));
    }
    if summary.collections > 0 {
        parts.push((p.n_collections)(summary.collections));
    }
    if summary.notes > 0 {
        parts.push((p.n_notes)(summary.notes));
    }
    if summary.planned > 0 {
        parts.push((p.n_planned)(summary.planned));
    }
    if summary.passed > 0 {
        parts.push((p.passed)(summary.passed));
    }
    if summary.saved > 0 {
        parts.push((p.n_plans)(summary.saved));
    }
    parts.extend(summary.program.clone());
    format!("{}.", parts.join(", "))
}

/// „betula-zweitstudium-2026-10-14.json": the file a profile is kept in, by its name and the day.
fn file_name(name: &str, today: Option<Day>) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug.chars().take(40).collect();
    let mut parts = vec!["betula".to_string()];
    parts.extend(Some(slug.trim_end_matches('-').to_string()).filter(|slug| !slug.is_empty()));
    parts.extend(today.map(Day::iso));
    format!("{}.json", parts.join("-"))
}

/// Hands the visitor a file of `text` named `name`, made in the browser: it reaches no server.
#[allow(unused_variables)]
fn download(name: &str, text: &str) {
    #[cfg(feature = "csr")]
    {
        use wasm_bindgen::JsCast;
        let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
        let parts = web_sys::js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/json;charset=utf-8");
        let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) else { return };
        let Ok(address) = web_sys::Url::create_object_url_with_blob(&blob) else { return };
        let Some(link) = document.create_element("a").ok().and_then(|element| element.dyn_into::<web_sys::HtmlAnchorElement>().ok()) else { return };
        link.set_href(&address);
        link.set_download(name);
        link.set_hidden(true);
        // In the document while it is clicked, as a browser may want it.
        if let Some(body) = document.body() {
            let _ = body.append_child(&link);
            link.click();
            link.remove();
        }
        // The browser has the file once the click is handled; its address is let go a little later.
        set_timeout(
            move || {
                let _ = web_sys::Url::revoke_object_url(&address);
            },
            std::time::Duration::from_secs(10),
        );
    }
}

/// The browser's date. `None` on the server, which reads no clock.
fn today() -> Option<Day> {
    #[cfg(feature = "csr")]
    {
        let now = web_sys::js_sys::Date::new_0();
        Day::from_ymd(i32::try_from(now.get_full_year()).ok()?, now.get_month() + 1, now.get_date())
    }
    #[cfg(not(feature = "csr"))]
    None
}

/// The day a profile was made, in the browser's time zone; `None` where that is not known (0) and
/// on the server.
#[allow(unused_variables)]
fn day_of(secs: u64) -> Option<Day> {
    if secs == 0 {
        return None;
    }
    #[cfg(feature = "csr")]
    {
        let then = web_sys::js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(secs as f64 * 1000.0));
        Day::from_ymd(i32::try_from(then.get_full_year()).ok()?, then.get_month() + 1, then.get_date())
    }
    #[cfg(not(feature = "csr"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backup_is_named_by_its_profile_and_its_day() {
        let day = Day::from_ymd(2026, 10, 14);
        assert_eq!(file_name("Zweitstudium", day), "betula-zweitstudium-2026-10-14.json");
        assert_eq!(file_name("  Wirtschafts-Ing. (2) ", day), "betula-wirtschafts-ing-2-2026-10-14.json");
        assert_eq!(file_name("Ölberg Übung", day), "betula-ölberg-übung-2026-10-14.json");
        assert_eq!(file_name("…", day), "betula-2026-10-14.json");
        assert_eq!(file_name("Profil 1", None), "betula-profil-1.json");
        assert!(file_name(&"x".repeat(100), None).len() < 60);
    }

    #[test]
    fn what_a_backup_holds_is_said_in_a_line() {
        for t in [&i18n::DE, &i18n::EN] {
            assert_eq!(summary_line(&Summary::default(), t), t.profile.file_empty);
        }
        let summary = Summary { marked: 12, planned: 8, semesters: 2, passed: 1, saved: 0, program: Some("Informatik B.Sc. · PO 2008".to_string()), collections: 3, notes: 1 };
        assert_eq!(summary_line(&summary, &i18n::DE), "12 gemerkte Module, 3 Sammlungen, 1 Notiz, 8 Module geplant, 1 Modul bestanden, Informatik B.Sc. · PO 2008.");
        assert_eq!(summary_line(&summary, &i18n::EN), "12 saved modules, 3 collections, 1 note, 8 modules planned, 1 module passed, Informatik B.Sc. · PO 2008.");
    }
}
