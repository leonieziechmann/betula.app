//! „Profil" (owner, 2026-10-14: „eine seite, wo man seine ganzen informationen/einstellungen
//! verwalten kann"): the profiles of this browser and what the active one keeps, in one place.
//!
//! - The sidebar holds what belongs to the page as a whole (R17): the profiles — a click switches
//!   to one, in every tab of the browser (`folia_stores::profiles`) —, „Neues Profil", and a
//!   profile kept as a file or made of one („Sichern & übertragen").
//! - The page is the active profile: its name, renamed or deleted at its head; „Mein Studiengang";
//!   what it keeps, with the way to each and „Profil leeren"; and the settings of this browser,
//!   which every profile shares (light or dark, the language; R13). On a phone, which has no
//!   switch between light and dark, this is where they are.
//! - A profile deleted or emptied here comes back with „Rückgängig" until the page is left.
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
use folia_pages::ask::MetaAsk;
use folia_plans::study;
use folia_routes::url;
use folia_stores::bookmarks::Bookmarks;
use folia_stores::myprogram::{program_href, program_name, MineResolved, MyProgram};
use folia_stores::profiles::{Avatar, Backup, Cleared, Deleted, NoBackup, Profile, Profiles, Summary, MAX_NAME, MAX_PROFILES};
use folia_stores::studyplan::{Saved, Studyplan};
use leptos::prelude::*;
use folia_design::stage::Title;

use crate::i18n::{self, Texts};
use folia_data::{use_ask, use_data};
use folia_design::format;
use folia_design::nav;
use folia_design::ui::Icon;
use folia_shell::frame::Frame;
use folia_shell::pending::Shape;
use folia_shell::seo::Seo;
use folia_shell::skeleton::AppStandin;

/// Whether this build is the browser app: only there are there profiles.
const APP: bool = cfg!(feature = "csr");

/// Where this browser keeps light or dark (`enhance.js`, the script in `<head>`): `light`, `dark`,
/// else the system's.
const THEME_KEY: &str = "betula.theme";

/// The ids the focus is moved to.
const RENAME_ID: &str = "pf-rename";
const NAME_ID: &str = "pf-name";
const DELETE_ID: &str = "pf-delete";
const DELETE_YES_ID: &str = "pf-delete-yes";
const NEW_ID: &str = "pf-new";
const NEW_NAME_ID: &str = "pf-new-name";
const CLEAR_ID: &str = "pf-clear";
const CLEAR_YES_ID: &str = "pf-clear-yes";
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
    // JavaScript the page offers the other language, which lives in the profile (`Settings`).
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
            </AppStandin>
        }
        .into_any();
    }
    let note = RwSignal::new(None::<Note>);
    let sidebar = move || view! { <Sidebar note/> };
    view! {
        <Title text=p.title/>
        <Frame title=p.profiles sidebar sidebar_first=true>
            <div class="page-inner pf">
                <ProfileSeo/>
                <Notice note/>
                <Head note/>
                <Program/>
                <Kept note/>
                <Settings/>
            </div>
        </Frame>
    }
    .into_any()
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

/// The active profile: its badge and name, „Umbenennen" and „Löschen".
#[component]
fn Head(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let current = Memo::new(move |_| profiles.map(Profiles::current));
    // The only profile is emptied, not deleted (`Profiles::delete`).
    let only = Memo::new(move |_| profiles.map_or(1, Profiles::count) < 2);
    let renaming = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let asking = RwSignal::new(false);
    // Another profile (switched to in the sidebar or in another tab): what was begun for the one
    // before is closed.
    Effect::new(move |before: Option<Option<u16>>| {
        let id = current.with(|profile| profile.as_ref().map(|profile| profile.id));
        if before.is_some_and(|before| before != id) {
            renaming.set(false);
            asking.set(false);
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
    let ask = move |_| {
        if !only.get_untracked() {
            asking.set(true);
            request_animation_frame(|| nav::focus_by_id(DELETE_YES_ID));
        }
    };
    let delete = move |_| {
        asking.set(false);
        let (Some(profiles), Some(profile)) = (profiles, current.get_untracked()) else { return };
        if let Some(deleted) = profiles.delete(profile.id) {
            note.set(Some(Note::Deleted(deleted, profile.shown(t.locale))));
            request_animation_frame(|| nav::focus_by_id(UNDO_ID));
        }
    };
    let cancel = move |_| {
        asking.set(false);
        request_animation_frame(|| nav::focus_by_id(DELETE_ID));
    };
    view! {
        <section class="panel pf-head" aria-label=p.active>
            {move || current.get().map(|profile| {
                let shown = profile.shown(t.locale);
                let unnamed = (t.profiles.unnamed)(profile.id);
                let made = day_of(profile.at).map(|day| (p.made_on)(&day.date(t.locale)));
                let question = (p.delete_question)(&shown);
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
                                view! { <h2>{shown.clone()}</h2> }.into_any()
                            }}
                            <p class="pf-lead">{p.lead}</p>
                            {made.map(|made| view! { <p class="pf-made">{made}</p> })}
                        </div>
                    </div>
                    <div class="pf-head-actions">
                        <button class="btn secondary" type="button" id=RENAME_ID on:click=start_renaming><Icon name="pencil"/><span>{p.rename}</span></button>
                        <button
                            class="btn secondary"
                            type="button"
                            id=DELETE_ID
                            aria-disabled=move || only.get().then_some("true")
                            title=move || only.get().then_some(p.only_profile)
                            on:click=ask
                        >
                            <Icon name="trash-2"/><span>{p.delete}</span>
                        </button>
                    </div>
                    {move || asking.get().then(|| view! {
                        <p class="action note-action ask pf-ask">
                            <span>{question.clone()}</span>
                            <button class="mini danger hit" type="button" id=DELETE_YES_ID on:click=delete>{p.delete_yes}</button>
                            <button class="mini hit" type="button" on:click=cancel>{p.cancel}</button>
                        </p>
                    })}
                }
            })}
        </section>
    }
}

/// „Mein Studiengang" of the active profile: the program, its Studienrichtung, the Studienbeginn
/// with the Fachsemester, the Standort; changed where it is set, in „Mein Studium".
#[component]
fn Program() -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let mine = MyProgram::expect();
    let resolved = MineResolved::expect();
    let source = use_data().ok();
    // The current semester, for the Fachsemester.
    let meta = use_ask(|| Some(MetaAsk {}));
    let now = Memo::new(move |_| meta.with(|meta| meta.as_ref().and_then(|meta| meta.as_ref().ok()).and_then(|meta| meta.current_semester.as_deref().and_then(SemesterKey::parse))));
    let doc = Memo::new(move |_| mine.map(MyProgram::get).unwrap_or_default());
    let known = Memo::new(move |_| resolved.and_then(|resolved| resolved.0.get()));
    view! {
        <section class="panel pf-part" aria-labelledby="pf-program">
            <h2 id="pf-program"><Icon name="graduation-cap"/>{p.program_heading}</h2>
            {move || {
                let doc = doc.get();
                let Some(id) = doc.program.clone() else {
                    return view! {
                        <p class="pf-empty">{p.no_program_hint}</p>
                        <div class="pf-ways">
                            <a class="btn secondary" href=t.path(url::STUDY) rel="nofollow"><Icon name="graduation-cap"/><span>{p.set_up}</span></a>
                            <a class="btn secondary" href=t.path(url::PROGRAMS)><Icon name="layout-list"/><span>{p.all_programs}</span></a>
                        </div>
                    }
                    .into_any();
                };
                // The program as the catalog knows it, with the plan of the kept Studienrichtung; a
                // program gone from the snapshot is named as it was kept, and says so.
                let program = match known.get() {
                    Some(info) => {
                        let href = program_href(source.as_ref(), &info.program, doc.caption.as_deref(), doc.direction.as_deref());
                        let name = program_name(&info.program);
                        if info.exact {
                            view! { <a href=t.path(&href)>{name}</a> }.into_any()
                        } else {
                            view! { <span>{doc.name.clone().unwrap_or(id)}</span><small>{p.program_gone}</small> }.into_any()
                        }
                    }
                    None => view! { <span>{doc.name.clone().unwrap_or(id)}</span> }.into_any(),
                };
                let direction = doc.direction.clone().or_else(|| doc.caption.clone()).filter(|text| !text.trim().is_empty());
                let start = doc.start.map(|start| match now.get().and_then(|now| study::fs_of(start, now, &doc.leave)) {
                    Some(fs) => format!("{} · {}", start.label(t.locale), (p.fs)(fs)),
                    None => start.label(t.locale),
                });
                let leave = (!doc.leave.is_empty()).then(|| (p.leave)(doc.leave.len()));
                let town = match doc.town {
                    TownChoice::Derive => p.town_derived.to_string(),
                    TownChoice::Only(town) => town.label().to_string(),
                    TownChoice::Both => p.town_both.to_string(),
                };
                view! {
                    <dl class="pf-facts">
                        <div><dt>{p.program}</dt><dd>{program}</dd></div>
                        {direction.map(|direction| view! { <div><dt>{p.direction}</dt><dd>{direction}</dd></div> })}
                        {start.map(|start| view! { <div><dt>{p.start}</dt><dd>{start}</dd></div> })}
                        {leave.map(|leave| view! { <div><dt>{p.leave_heading}</dt><dd>{leave}</dd></div> })}
                        <div><dt>{p.town}</dt><dd>{town}</dd></div>
                    </dl>
                    <div class="pf-ways">
                        <a class="btn secondary" href=t.path(url::STUDY) rel="nofollow"><Icon name="pencil"/><span>{p.change_program}</span></a>
                    </div>
                }
                .into_any()
            }}
        </section>
    }
}

/// What the active profile keeps, each with the way to it, and „Profil leeren".
#[component]
fn Kept(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let bookmarks = Bookmarks::expect();
    let plan = Studyplan::expect();
    let mine = MyProgram::expect();
    let saved = Saved::open();
    // Each number a memo of its own (R5).
    let marked = Memo::new(move |_| bookmarks.map_or(0, Bookmarks::count));
    // The Merkliste's collections and notes (`folia_stores::collections`).
    let shelf = folia_stores::collections::Collections::expect();
    let kept = Memo::new(move |_| shelf.map_or((0, 0), |shelf| shelf.with(|shelf| (shelf.collections().len(), shelf.notes_count()))));
    let planned = Memo::new(move |_| plan.map_or((0, 0), |plan| (plan.count(), plan.with(|doc| doc.semesters().len()))));
    let passed = Memo::new(move |_| plan.map_or(0, |plan| plan.with(|doc| doc.passed.len())));
    let plans = Memo::new(move |_| saved.with(|saved| saved.plans.len()));
    let program = Memo::new(move |_| mine.is_some_and(|mine| mine.with(|doc| doc.program.is_some() || doc.start.is_some())));
    let empty = Memo::new(move |_| marked.get() == 0 && kept.get() == (0, 0) && planned.get().0 == 0 && passed.get() == 0 && plans.get() == 0 && !program.get());

    let confirming = RwSignal::new(false);
    // Another profile: the question was about the one before.
    Effect::new(move |before: Option<u16>| {
        let id = profiles.map_or(0, Profiles::active);
        if before.is_some_and(|before| before != id) {
            confirming.set(false);
        }
        id
    });
    let ask = move |_| {
        if !empty.get_untracked() {
            confirming.set(true);
            request_animation_frame(|| nav::focus_by_id(CLEAR_YES_ID));
        }
    };
    let clear = move |_| {
        confirming.set(false);
        let Some(profiles) = profiles else { return };
        let cleared = profiles.clear(profiles.active_untracked());
        note.set(Some(Note::Cleared(cleared)));
        request_animation_frame(|| nav::focus_by_id(UNDO_ID));
    };
    let cancel = move |_| {
        confirming.set(false);
        request_animation_frame(|| nav::focus_by_id(CLEAR_ID));
    };

    let row = move |icon: &'static str, label: &'static str, href: &'static str, value: Signal<String>| {
        view! {
            <a class="pf-row" href=t.path(href) rel="nofollow">
                <Icon name=icon/>
                <span class="pf-row-label">{label}</span>
                <span class="pf-row-value">{value}</span>
                <Icon name="chevron-right"/>
            </a>
        }
    };
    view! {
        <section class="panel pf-part" aria-labelledby="pf-kept">
            <h2 id="pf-kept"><Icon name="database"/>{p.kept_heading}</h2>
            <div class="pf-rows">
                {row("bookmark", p.bookmarks, url::BOOKMARKS, Signal::derive(move || format::modules(i64::try_from(marked.get()).unwrap_or(i64::MAX), t.locale)))}
                {row("tag", p.collections, url::BOOKMARKS, Signal::derive(move || match kept.get() {
                    (0, 0) => p.no_collections.to_string(),
                    (collections, 0) => (p.n_collections)(collections),
                    (0, notes) => (p.n_notes)(notes),
                    (collections, notes) => format!("{} · {}", (p.n_collections)(collections), (p.n_notes)(notes)),
                }))}
                {row("calendar-range", p.studyplan, url::STUDYPLAN, Signal::derive(move || {
                    let (modules, semesters) = planned.get();
                    (p.planned)(modules, semesters)
                }))}
                {row("graduation-cap", p.study, url::STUDY, Signal::derive(move || (p.passed)(passed.get())))}
                {row("copy", p.saved_plans, url::STUDYPLAN, Signal::derive(move || (p.plans)(plans.get())))}
            </div>
            <div class="pf-clear">
                {move || if confirming.get() {
                    view! {
                        <p class="action note-action ask">
                            <span>{p.clear_question}</span>
                            <button class="mini danger hit" type="button" id=CLEAR_YES_ID on:click=clear>{p.clear_yes}</button>
                            <button class="mini hit" type="button" on:click=cancel>{p.cancel}</button>
                        </p>
                    }
                    .into_any()
                } else {
                    view! {
                        <button
                            class="action"
                            type="button"
                            id=CLEAR_ID
                            aria-disabled=move || empty.get().then_some("true")
                            title=move || empty.get().then_some(p.nothing_to_clear)
                            on:click=ask
                        >
                            <Icon name="rotate-ccw"/><span>{p.clear}<small>{p.clear_hint}</small></span>
                        </button>
                    }
                    .into_any()
                }}
            </div>
        </section>
    }
}

/// Light or dark, as this browser keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Theme {
    /// The system's: nothing kept.
    System,
    Light,
    Dark,
}

impl Theme {
    const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    fn kept() -> Self {
        match nav::local_get(THEME_KEY).as_deref() {
            Some("light") => Theme::Light,
            Some("dark") => Theme::Dark,
            _ => Theme::System,
        }
    }

    /// What `enhance.js` is handed (`data-theme-value`).
    fn code(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Theme::System => "monitor",
            Theme::Light => "sun",
            Theme::Dark => "moon",
        }
    }

    fn label(self, t: &Texts) -> &'static str {
        match self {
            Theme::System => t.profile.theme_system,
            Theme::Light => t.profile.theme_light,
            Theme::Dark => t.profile.theme_dark,
        }
    }
}

/// The settings of this browser, the same in every profile (R13): light, dark or the system's,
/// and the language. Light and dark are set by `enhance.js` (`data-action="theme-set"`), which
/// keeps them as the switch in the rail does and says so (`betula:theme`); another language is
/// another address, and so a page load (`folia_shell::languages`).
#[component]
fn Settings() -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let theme = RwSignal::new(Theme::kept());
    // Effects run in the browser only.
    Effect::new(move |_| {
        let handle = window_event_listener_untyped("betula:theme", move |_| theme.set(Theme::kept()));
        on_cleanup(move || handle.remove());
    });
    let checked = move |on: bool| if on { "true" } else { "false" };
    view! {
        <section class="panel pf-part" aria-labelledby="pf-settings">
            <h2 id="pf-settings"><Icon name="sliders-horizontal"/>{p.settings_heading}</h2>
            <p class="pf-sub">{p.settings_hint}</p>
            <div class="pf-setting">
                <p class="label" id="pf-theme">{p.theme}</p>
                <div class="seg pf-seg" role="radiogroup" aria-labelledby="pf-theme">
                    {Theme::ALL.into_iter().map(|choice| view! {
                        <button type="button" role="radio" data-action="theme-set" data-theme-value=choice.code() aria-checked=move || checked(theme.get() == choice)>
                            <Icon name=choice.icon()/><span>{choice.label(t)}</span>
                        </button>
                    }).collect_view()}
                </div>
            </div>
            <div class="pf-setting">
                <p class="label" id="pf-language">{p.language}</p>
                <div class="seg pf-seg" role="radiogroup" aria-labelledby="pf-language">
                    {Locale::ALL.iter().copied().map(|locale| if locale == t.locale {
                        view! { <button type="button" role="radio" aria-checked="true" lang=locale.code()>{locale.name()}</button> }.into_any()
                    } else {
                        view! {
                            <a
                                role="radio"
                                aria-checked="false"
                                href=locale.path(url::PROFILE)
                                hreflang=locale.code()
                                lang=locale.code()
                                rel="alternate external nofollow"
                                data-language=locale.code()
                            >
                                {locale.name()}
                            </a>
                        }
                        .into_any()
                    }).collect_view()}
                </div>
            </div>
            // Betula's single keys (folia/design/tastatur/konzept.html, question 4), as in the
            // overview of the keys (`folia_shell::keys`).
            <div class="pf-setting">
                <p class="label" id="pf-letters">{t.ui.letters}</p>
                <folia_design::keys::LettersSwitch labelled_by="pf-letters"/>
                <p class="pf-sub">{t.ui.letters_hint}</p>
            </div>
        </section>
    }
}

/// The sidebar: the profiles, a new one, a backup; where all of it lives.
#[component]
fn Sidebar(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    view! {
        <ProfileList note/>
        <BackupGroup note/>
        <div class="fgroup">
            <p class="hint storage-hint"><Icon name="shield-check"/><span>{t.profile.storage_hint}</span></p>
        </div>
    }
}

/// The profiles of this browser: a click makes one the active profile; „Neues Profil".
#[component]
fn ProfileList(note: RwSignal<Option<Note>>) -> impl IntoView {
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
        <div class="fgroup first">
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
            <NewProfile note/>
        </div>
    }
}

/// „Neues Profil": a name (none: „Profil 3"), and the new profile, empty, is the active one.
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
                    <Icon name="user-round-plus"/><span>{p.new_profile}</span>
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

/// „Sichern & übertragen": the active profile as a file („Als Datei sichern", `Backup`), and a
/// profile made of one („Aus Datei laden"), which is offered before anything is made.
#[component]
fn BackupGroup(note: RwSignal<Option<Note>>) -> impl IntoView {
    let t = i18n::t();
    let p = &t.profile;
    let profiles = Profiles::expect();
    let input = NodeRef::<leptos::html::Input>::new();
    // What a file picked holds, until it is taken over or put aside.
    let offer = RwSignal::new(None::<Result<Backup, Refused>>);
    let save = move |_| {
        let Some(backup) = profiles.and_then(|profiles| profiles.backup(profiles.active_untracked(), t.locale)) else { return };
        download(&file_name(&backup.name, today()), &backup.file());
    };
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
        <div class="fgroup actions pf-backup">
            <p class="flabel label">{p.backup}</p>
            <button class="action" type="button" on:click=save>
                <Icon name="download"/><span>{p.save_file}<small>{p.save_file_hint}</small></span>
            </button>
            <button class="action" type="button" id=OPEN_ID on:click=pick>
                <Icon name="upload"/><span>{p.open_file}<small>{p.open_file_hint}</small></span>
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
        </div>
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
