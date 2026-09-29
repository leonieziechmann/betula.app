//! The landing page. It has to answer three questions at a glance: what is this (the catalog of
//! one university, unofficial), what is in it (numbers, and pictures of the app), and where do I
//! start (the search, entry links with their counts, the faculties). Below that it says in plain
//! text what the app does and answers what people ask search engines about the modules of the
//! BTU and about studying there: that text is what the page is found by.
//!
//! Owner, 2026-09-21 („unaufgeräumt und anstrengend für die Augen"; the sidebar and the big map
//! did not work; a little more colour; then: save height in the head, a carousel that behaves like
//! one for pictures, more questions; then: the figures as a stack of bare numbers beside the text,
//! with something of the birch): a flat first panel — the text on the left, the figures on the
//! right between strokes of birch bark, each set so large that all of them are about equally
//! wide, in the colours of the theme — and under it a carousel of pictures: the map of the
//! programs, then screenshots of the catalog, a study plan and a module page
//! (`e2e/showcase-shots.mjs`). The current picture stands in front, half of each
//! neighbour shows behind it at its sides; it turns on by itself (the bar in the current tab shows
//! when) until the pause button stops it, and goes round endlessly. A click on the current picture
//! opens its page; the map opens large in a dialog (the page behind it stands still), where it is
//! the interactive map it was, with the legend and what is shown beside it on a wide screen.
//! Colour comes from the palette of the faculties. The questions open one at a time, in three
//! groups: about Betula, using it, and for the first semesters.
//!
//! Owner, 2026-09-28: the page is what search engines, and the assistants that answer with them,
//! know of Betula. Asked to compare it with other tools they took the missing account for missing
//! functions, and saw neither that it works without JavaScript nor that the study plans are read
//! from the regulations. So the page says all of it plainly: nine abilities (the account, the pages
//! without JavaScript and offline, the Stundenplan, the plans from the regulations among them),
//! „Betula im Detail" (`detail.rs`), a chapter per feature with a picture made of the app's own
//! parts, and at the end of the page questions on exactly these points (owner, 2026-09-29: the
//! FAQ at the end). The structured data names Betula as a web app with the same abilities
//! (`WebApplication`), free and running without JavaScript.
//!
//! Owner, the same day: the sidebar made the page confusing and odd to look at („sehr verwirrend",
//! „sorgt dafür, dass die Seite komisch aussieht"); a new visitor has to find their way, and the
//! onboarding matters most — organically, without a pop-up. So the start page is the one page
//! without the frame of the others (R17): no sidebar, the panels in one column across the width
//! (up to a measure that still reads well, in the middle). Under the first panel the way in for
//! a first visit (`start.rs`): three steps, each naming where the navigation keeps it, following
//! what the visitor has done. Of the sidebar's parts the jumps to the sections are the foot of
//! those steps now; the Datenstand is the ground's, at the end of every page, as before.
//! Then: one click a step („Studiengang wählen" picks the program in place, here in the first
//! panel too), a birch down the first panel's right edge with the figures on its branches
//! (`Hero`). The room beside the column on a wide screen is the wood's, which stands behind every
//! page (`ground::Wood`, owner, 2026-09-29); it took the place of the branches with small crowns
//! of leaves that grew out of the panels.
//!
//! The map (`catalog::graph`) is laid out by the web server once per snapshot, on a 4:3 sheet and
//! a tall one for phones; this page only draws it (`data::ProgramMapHandle`). The dialog is in the
//! server's HTML too (closed), so its dots are links that search engines follow; without
//! JavaScript the picture of the map leads to the program overview.
//!
//! The words of the page are in `i18n/home.rs` and `i18n/home_detail.rs`, in every language; the
//! names of the programs and the faculties are the BTU's, in every language (docs/i18n.md).

mod detail;
mod start;

use std::sync::Arc;

use catalog::filter::{ExamPart, Language, TurnusFilter};
use catalog::graph::{Cycle, Layout, ProgramMap};
use catalog::labels::Campus;
use catalog::pages::{self, HomeData};
use catalog::rows::Semester;
use catalog::timetable::semester::SemesterKey;
use catalog::url::{self, CatalogUrl, ProgramTab};
use catalog::{CatalogQuery, Locale};
use leptos::prelude::*;
use leptos_meta::Title;

use crate::combobox::ClosePopups;
use crate::data::{use_source, PageStatus, ProgramMapHandle};
use crate::format;
use crate::i18n::{self, Texts};
use crate::nav;
use crate::seo::{self, Seo};
use crate::ui::{ErrorState, Icon, Mark, Wordmark};

/// A way into the catalog: a filter people come for, with the number of modules behind it. The
/// tint colours its icon (`t-…` in app.css), after what it is about: winter cool, summer warm.
struct Entry {
    label: &'static str,
    hint: &'static str,
    icon: &'static str,
    tint: &'static str,
    query: CatalogQuery,
}

fn entries(t: &'static Texts) -> Vec<Entry> {
    let entry = |text: &'static i18n::home::Entry, icon, tint, query| Entry { label: text.label, hint: text.hint, icon, tint, query };
    let t = &t.home;
    vec![
        entry(&t.winter, "snowflake", "t-ice", CatalogQuery { turnus: TurnusFilter { winter: true, ..Default::default() }, ..Default::default() }),
        entry(&t.summer, "sun", "t-sun", CatalogQuery { turnus: TurnusFilter { summer: true, ..Default::default() }, ..Default::default() }),
        entry(&t.english, "languages", "t-violet", CatalogQuery { languages: vec![Language::English], ..Default::default() }),
        entry(&t.fues, "shuffle", "t-teal", CatalogQuery { fues: Some(true), ..Default::default() }),
        entry(&t.no_written_exam, "file-check-2", "t-green", CatalogQuery { exam_parts_exclude: vec![ExamPart::Written], ..Default::default() }),
        entry(&t.oral_exam, "users-round", "t-coral", CatalogQuery { exam_parts: vec![ExamPart::Oral], ..Default::default() }),
        entry(&t.senftenberg, "map-pin", "t-rose", CatalogQuery { campuses: vec![Campus::Senftenberg], ..Default::default() }),
        entry(&t.ungraded, "circle-check-big", "t-slate", CatalogQuery { graded: Some(false), ..Default::default() }),
    ]
}

/// Where the browser remembers that the pictures were stopped (`localStorage`, R20).
const PAUSED_KEY: &str = "betula.showcase";

/// What Betula does, each with its icon: three rows of three on a wide page.
fn abilities(t: &'static Texts) -> [(&'static str, &'static i18n::home::Ability); 9] {
    let t = &t.home;
    [
        ("search", &t.search),
        ("sliders-horizontal", &t.filters),
        ("file-check-2", &t.plans),
        ("calendar-range", &t.timetable),
        ("calendar-days", &t.dates),
        ("repeat", &t.prerequisites),
        ("shield-check", &t.account),
        ("database", &t.offline),
        ("eye", &t.gaps),
    ]
}

/// Questions and answers, in three groups: about Betula, using it, and about studying (for first
/// semesters).
fn questions(t: &'static Texts) -> [&'static i18n::home::Faq; 3] {
    [&t.home.about_betula, &t.home.using_betula, &t.home.for_studies]
}

/// The name of a semester in the page's language (the snapshot's `label` is German).
fn semester_name(semester: &Semester, locale: Locale) -> String {
    SemesterKey::parse(&semester.key).map_or_else(|| semester.label.clone(), |key| key.label(locale))
}

#[component]
pub fn HomePage() -> impl IntoView {
    let t = i18n::t();
    let source = use_source();
    let status = PageStatus::capture();
    let entries = entries(t);
    // The ways in, and after them the selection the filter board of „Betula im Detail" shows: its
    // count comes with theirs, in the same loader.
    let queries: Vec<CatalogQuery> = entries.iter().map(|entry| entry.query.clone()).chain([detail::example()]).collect();
    let loaded = source.and_then(|source| source.run(|db| pages::home(db, &queries)));
    let map = use_context::<ProgramMapHandle>().map(|handle| handle.0);
    // „Betula im Detail" says how many of the current programs have a checked plan, and how many
    // modules its example finds.
    let plans = loaded.as_ref().ok().map(|home| (home.overview.plans, home.overview.programs));
    let example = loaded.as_ref().ok().and_then(|home| home.entry_counts.get(entries.len()).copied());

    let data = vec![
        serde_json::json!({
            "@type": "WebSite",
            "@id": seo::absolute("/#website"),
            "url": seo::absolute(url::HOME),
            "name": seo::SITE_NAME,
            "alternateName": t.home.alternate_names,
            "description": t.home.description,
            "inLanguage": t.locale.code(),
            "about": { "@type": "CollegeOrUniversity", "name": seo::UNIVERSITY, "alternateName": "BTU Cottbus-Senftenberg", "url": seo::UNIVERSITY_URL },
            "potentialAction": {
                "@type": "SearchAction",
                "target": { "@type": "EntryPoint", "urlTemplate": format!("{}?q={{search_term_string}}", seo::absolute(url::CATALOG)) },
                "query-input": "required name=search_term_string",
            },
        }),
        // Betula as the app it is: what it does is what the page lists as its abilities, and it is
        // free and runs in any browser, with or without JavaScript.
        serde_json::json!({
            "@type": "WebApplication",
            "@id": seo::absolute("/#app"),
            "url": seo::absolute(url::HOME),
            "name": seo::SITE_NAME,
            "description": t.home.description,
            "applicationCategory": "EducationalApplication",
            "browserRequirements": t.home.browser_requirements,
            "featureList": abilities(t).into_iter().map(|(_, ability)| format!("{}. {}", ability.title, ability.text)).collect::<Vec<_>>(),
            "inLanguage": Locale::ALL.iter().map(|locale| locale.code()).collect::<Vec<_>>(),
            "isAccessibleForFree": true,
            "offers": { "@type": "Offer", "price": "0", "priceCurrency": "EUR" },
            "about": { "@type": "CollegeOrUniversity", "name": seo::UNIVERSITY, "url": seo::UNIVERSITY_URL },
        }),
        serde_json::json!({
            "@type": "FAQPage",
            "mainEntity": questions(t).into_iter().flat_map(|faq| faq.questions.iter()).map(|(question, answer)| serde_json::json!({
                "@type": "Question",
                "name": question,
                "acceptedAnswer": { "@type": "Answer", "text": answer },
            })).collect::<Vec<_>>(),
        }),
    ];

    // A picker's popup stands where its button was: scrolling the page takes it away, as the
    // filter panel of the catalog does.
    let close_popups = RwSignal::new(0u32);
    provide_context(ClosePopups(close_popups));

    // No frame and no sidebar (see above): the page scrolls on its own, as `#page-scroll` (the
    // ground and „Nach oben" follow it, `enhance.js`). The wood (`ground::Wood`) stands still
    // behind it.
    view! {
        <Title text=""/>
        <div class="page home-page" id="page-scroll" on:scroll=move |_| close_popups.update(|n| *n = n.wrapping_add(1))><div class="page-inner home">
        <Seo title=t.home.seo_title description=t.home.description path=url::HOME data/>
        {match loaded {
            Err(error) => {
                status.for_error(&error);
                view! { <Hero home=None/><start::StartPath/><Showcase map modules=None/><ErrorState error/> }.into_any()
            }
            Ok(home) => {
                let modules = Some(home.overview.modules);
                view! {
                    <Hero home=Some(home.clone())/>
                    <start::StartPath/>
                    <Showcase map modules/>
                    <Entries entries home=home.clone()/>
                }.into_any()
            }
        }}
        <section class="panel abilities" id="funktionen" aria-labelledby="funktionen-titel">
            <header class="block-head">
                <h2 id="funktionen-titel">{t.home.abilities}</h2>
                <p>{t.home.abilities_lead}</p>
            </header>
            <ul class="ability-grid">
                {abilities(t).into_iter().map(|(icon, ability)| view! {
                    <li class="ability">
                        <span class="ico"><Icon name=icon/></span>
                        <h3>{ability.title}</h3>
                        <p>{ability.text}</p>
                    </li>
                }).collect_view()}
            </ul>
            <p class="soon-line">
                <span class="label">{t.home.in_progress}</span>
                <span><Icon name="circle-check-big"/>{t.home.coming}</span>
            </p>
        </section>
        <detail::Details plans example/>
        // The questions are the list; an answer opens in place. The text is in the page either way
        // (and in the FAQPage data above).
        <section class="panel questions" id="fragen" aria-labelledby="fragen-titel">
            <header class="block-head">
                <h2 id="fragen-titel">{t.home.questions}</h2>
            </header>
            {questions(t).into_iter().map(|faq| view! {
                <div class="faq">
                    <h3 class="faq-group label">{faq.name}</h3>
                    {faq.questions.iter().map(|(question, answer)| view! {
                        <details>
                            <summary>{*question}<Icon name="plus"/></summary>
                            <p>{*answer}</p>
                        </details>
                    }).collect_view()}
                </div>
            }).collect_view()}
        </section>
        </div></div>
    }
}

/// The first panel, flat: what this is and the two ways in on the left, and a birch down its
/// right edge with the figures hanging to its left on branches of it (owner, 2026-09-28: „rechts am
/// Rand so ein dickerer Birkenstamm und dann nach links die Stats"; before, a small birch with a
/// crown and a ground of its own stood beside the text, and doubled the page's crown). The birch
/// and its figures stand beside the text from a notebook's width on; on a phone the panel is its
/// text alone (they took the height the way in needs there). At night the trunk is the moon's
/// grey, as the wood behind the page is dark (`--hero-bark`).
#[component]
fn Hero(home: Option<HomeData>) -> impl IntoView {
    let t = i18n::t();
    let figures = home.map(|home| {
        // The faculties are the numbered ones; centres that offer programs too are listed below.
        let faculties = home.faculties.iter().filter(|(department, _)| department.as_ref().is_some_and(|d| d.code.chars().all(|c| c.is_ascii_digit()))).count();
        let mut rows: Vec<(&str, Option<String>, u64)> = vec![(t.home.figure_modules, None, home.overview.modules), (t.home.figure_programs, None, home.overview.programs), (t.home.figure_faculties, None, faculties as u64)];
        if let Some(semester) = &home.overview.current_semester {
            rows.push((t.home.figure_dates, Some((t.home.in_semester)(&semester_name(semester, t.locale))), semester.teaching_events.max(0) as u64));
        }
        view! {
            <dl class="tree-figures">
                {rows.into_iter().map(|(label, detail, value)| view! {
                    <div>
                        <dt>{label}{detail.map(|detail| view! { <small>{detail}</small> })}</dt>
                        <dd class="num">{format::count(value, t.locale)}</dd>
                    </div>
                }).collect_view()}
            </dl>
        }
    });
    view! {
        <section class="panel home-hero" id="ueberblick">
            <div class="home-hero-text">
                // On a phone the start page carries the brand, and at the end of its line the switch
                // between the languages (`languages`): the phone has no rail to hold it.
                <div class="brand-phone"><span class="logo"><Mark/></span><span><Wordmark small=true/><small>{t.common.tagline}</small></span><crate::languages::Languages/></div>
                <p class="eyebrow-pill"><i></i>{t.home.eyebrow}</p>
                <h1>{t.home.title_before}<span class="nowrap">"BTU Cottbus-Senftenberg"</span>{t.home.title_after}</h1>
                <p class="lead">{t.home.lead}</p>
                // „Studiengang wählen" picks the program in place, or leads to all of them
                // (`start::ProgramPick`), as the first step of the way in below does.
                <div class="intro-actions">
                    <a class="btn primary" href=t.path(url::CATALOG)><Icon name="layout-list"/>{t.home.browse_modules}</a>
                    <start::ProgramPick id="home-program" class="btn secondary"/>
                </div>
            </div>
            {figures}
            // The trunk, its bark drawn by the stylesheet (`.hero-trunk`).
            <div class="hero-trunk" aria-hidden="true"></div>
        </section>
    }
}

/// What a picture of the carousel is: the map (it opens large) or a screenshot of a page (it
/// leads there; `app/assets/shots/<file>[-phone][-dark].webp`).
#[derive(Clone)]
enum Picture {
    Map(Arc<ProgramMap>),
    Shot { file: &'static str, alt: &'static str },
}

#[derive(Clone)]
struct Slide {
    tab: &'static str,
    title: &'static str,
    text: String,
    /// What a click on the current picture does, said on it.
    action: &'static str,
    href: String,
    /// The colours of the wash behind it (`wash-…` in app.css).
    wash: &'static str,
    picture: Picture,
}

/// Where a slide stands, counted from the current one: 0 in the middle, ±1 at the sides, ±2 out
/// of sight. `staged`: a move is about to happen in that direction, so what is out of sight waits
/// on the side it will come in from; else it lies on the side it left to (against `dir`).
fn place(i: usize, current: usize, count: usize, dir: i32, staged: Option<i32>) -> i32 {
    let d = (i + count - current % count.max(1)) % count.max(1);
    if d == 0 {
        0
    } else if d == 1 {
        1
    } else if d + 1 == count {
        -1
    } else {
        2 * staged.unwrap_or(-dir)
    }
}

/// The pictures: one in the middle, its neighbours at the sides, going round. It turns on by
/// itself when the bar in the current tab is full (a CSS animation: it stands still while paused
/// and while the map is open), by the arrows, the tabs, the arrow keys, a swipe, or a click on a
/// neighbour.
#[component]
fn Showcase(map: Option<Arc<ProgramMap>>, modules: Option<u64>) -> impl IntoView {
    let t = i18n::t();
    let home = &t.home;
    let mut slides: Vec<Slide> = Vec::new();
    if let Some(map) = map.clone() {
        slides.push(Slide {
            tab: home.map_slide.tab,
            title: home.map_slide.title,
            text: (home.map_text)(map.programs.len()),
            action: home.map_slide.action,
            href: t.path(url::PROGRAMS),
            wash: "wash-map",
            picture: Picture::Map(map),
        });
    }
    slides.push(Slide {
        tab: home.catalog_slide.tab,
        title: home.catalog_slide.title,
        text: match modules {
            Some(modules) => (home.catalog_text)(&format::count(modules, t.locale)),
            None => home.catalog_text_plain.to_string(),
        },
        action: home.catalog_slide.action,
        href: t.path(url::CATALOG),
        wash: "wash-catalog",
        picture: Picture::Shot { file: "catalog", alt: home.catalog_alt },
    });
    slides.push(Slide {
        tab: home.plan_slide.tab,
        title: home.plan_slide.title,
        text: home.plan_text.to_string(),
        action: home.plan_slide.action,
        href: t.path(url::PROGRAMS),
        wash: "wash-program",
        picture: Picture::Shot { file: "program", alt: home.plan_alt },
    });
    slides.push(Slide {
        tab: home.module_slide.tab,
        title: home.module_slide.title,
        text: home.module_text.to_string(),
        action: home.module_slide.action,
        href: t.path(&url::module_path("12330")),
        wash: "wash-module",
        picture: Picture::Shot { file: "module", alt: home.module_alt },
    });
    let count = slides.len();

    let current = RwSignal::new(0usize);
    let dir = RwSignal::new(1i32);
    let staged = RwSignal::new(None::<i32>);
    let map_open = RwSignal::new(false);
    // Turning by itself, until the pause button stops it. The browser remembers a stop (only
    // that: playing is the default and leaves nothing behind).
    let playing = RwSignal::new(nav::local_get(PAUSED_KEY).as_deref() != Some("paused"));
    let toggle_play = move |_| {
        let now = !playing.get_untracked();
        playing.set(now);
        nav::local_set(PAUSED_KEY, if now { "" } else { "paused" });
    };
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    // A swipe ends in a click on the picture under it; that click must not count.
    let swipe_start = RwSignal::new(None::<(i32, i32)>);
    let swiped = RwSignal::new(false);

    // A move: what comes in is put on its side first (without animation), and one frame later
    // everything moves (with animation). Without the first step a picture that left on one side
    // would cross the whole frame to come in on the other.
    let go = move |to: usize, towards: i32| {
        if to == current.get_untracked() || count < 2 {
            return;
        }
        staged.set(Some(towards));
        request_animation_frame(move || {
            request_animation_frame(move || {
                staged.set(None);
                dir.set(towards);
                current.set(to % count);
            })
        });
    };
    let next = move || go((current.get_untracked() + 1) % count, 1);
    let previous = move || go((current.get_untracked() + count - 1) % count, -1);
    // To a picture by its tab: the short way round.
    let show = move |to: usize| {
        let ahead = (to + count - current.get_untracked()) % count;
        go(to, if ahead * 2 <= count { 1 } else { -1 });
    };
    let open_map = move || {
        if let Some(dialog) = dialog.get_untracked() {
            if dialog.show_modal().is_ok() {
                map_open.set(true);
            }
        }
    };
    let close_map = move || {
        if let Some(dialog) = dialog.get_untracked() {
            dialog.close();
        }
    };

    let on_key = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "ArrowRight" => {
            ev.prevent_default();
            next();
        }
        "ArrowLeft" => {
            ev.prevent_default();
            previous();
        }
        _ => {}
    };
    let on_down = move |ev: leptos::ev::PointerEvent| {
        swiped.set(false);
        swipe_start.set(Some((ev.client_x(), ev.client_y())));
    };
    let on_up = move |ev: leptos::ev::PointerEvent| {
        if let Some((x, y)) = swipe_start.get_untracked() {
            let (dx, dy) = (ev.client_x() - x, ev.client_y() - y);
            if dx.abs() > 40 && dx.abs() > dy.abs() {
                swiped.set(true);
                if dx < 0 {
                    next();
                } else {
                    previous();
                }
            }
        }
        swipe_start.set(None);
    };

    let map_for_dialog = map.clone();
    view! {
        <section class="panel showcase" class:paused=move || map_open.get() || !playing.get() aria-roledescription=home.carousel aria-label=home.carousel_label>
            <div
                class="carousel"
                tabindex="0"
                aria-label=home.carousel_keys
                on:keydown=on_key
                on:pointerdown=on_down
                on:pointerup=on_up
                on:pointercancel=move |_| swipe_start.set(None)
            >
                // Gives the frame its height: a slide as it is, never seen.
                <div class="cslide spacer" aria-hidden="true"><div class="stage"></div><div class="cap"><span class="cap-text"><b>"–"</b><span>"–"</span></span></div></div>
                {slides.iter().cloned().enumerate().map(|(i, slide)| {
                    let at = move || place(i, current.get(), count, dir.get(), staged.get());
                    let is_map = matches!(slide.picture, Picture::Map(_));
                    let on_click = move |ev: leptos::ev::MouseEvent| {
                        if swiped.get_untracked() {
                            ev.prevent_default();
                            swiped.set(false);
                            return;
                        }
                        if ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
                            return;
                        }
                        if place(i, current.get_untracked(), count, dir.get_untracked(), None) != 0 {
                            // A neighbour comes to the middle.
                            ev.prevent_default();
                            show(i);
                        } else if is_map {
                            ev.prevent_default();
                            open_map();
                        }
                    };
                    let picture = match slide.picture {
                        Picture::Map(map) => view! { <MapPreview map/> }.into_any(),
                        // Every screenshot is lazy, and `loading` comes before the addresses: the
                        // app sets attributes in this order, and an image with an address and no
                        // `loading` yet is fetched at once. So only what shows is fetched: the
                        // pictures beside the current one, never those of the hidden theme.
                        Picture::Shot { file, alt } => [false, true].map(|dark| {
                            let suffix = if dark { "-dark" } else { "" };
                            let wide = format!("{}/{file}{suffix}.webp", crate::SHOTS);
                            let phone = format!("{}/{file}-phone{suffix}.webp", crate::SHOTS);
                            view! {
                                <picture class=if dark { "shot-dark" } else { "shot-light" }>
                                    <source media="(max-width: 600px)" width="720" height="960" srcset=phone/>
                                    <img loading="lazy" decoding="async" alt=alt width="1200" height="900" draggable="false" src=wide/>
                                </picture>
                            }
                        }).into_iter().collect_view().into_any(),
                    };
                    view! {
                        <a
                            class=format!("cslide {}", slide.wash)
                            class:is-current=move || at() == 0
                            class:is-side=move || at().abs() == 1
                            class:is-far=move || { at().abs() > 1 }
                            class:teleport=move || { at().abs() > 1 && staged.get().is_some() }
                            style=move || format!("--at:{}", at())
                            href=slide.href.clone()
                            draggable="false"
                            aria-roledescription=home.slide
                            aria-label=(home.slide_of)(i + 1, count, slide.title)
                            aria-current=move || (at() == 0).then_some("true")
                            tabindex=move || if at() == 0 { "0" } else { "-1" }
                            on:click=on_click
                        >
                            <div class=if is_map { "stage stage-map" } else { "stage stage-shot" }>
                                {picture}
                                <span class="stage-go" aria-hidden="true"><Icon name=if is_map { "maximize-2" } else { "arrow-up-right" }/></span>
                            </div>
                            <div class="cap">
                                <span class="cap-text"><b>{slide.title}</b><span>{slide.text.clone()}</span></span>
                                <span class="cap-go">{slide.action}<Icon name="chevron-right"/></span>
                            </div>
                        </a>
                    }
                }).collect_view()}
                <button type="button" class="show-arrow previous js-only" aria-label=home.previous on:click=move |_| previous()><Icon name="chevron-left"/></button>
                <button type="button" class="show-arrow next js-only" aria-label=home.next on:click=move |_| next()><Icon name="chevron-right"/></button>
            </div>
            <div class="show-bar js-only">
                // The mark of the current tab slides to it (`--i`).
                <nav class="seg show-tabs" role="radiogroup" aria-label=home.tabs style=move || format!("--i:{};--n:{count}", current.get())>
                    <i class="show-mark" aria-hidden="true"></i>
                    {slides.iter().enumerate().map(|(i, slide)| view! {
                        <button type="button" role="radio" aria-checked=move || (current.get() == i).to_string() on:click=move |_| show(i)>
                            {slide.tab}
                            // The time until the next picture; when it is full, the next one comes.
                            {move || (current.get() == i).then(|| view! { <i class="show-progress" on:animationend=move |_| next()></i> })}
                        </button>
                    }).collect_view()}
                </nav>
                <button
                    type="button"
                    class="show-play"
                    aria-label=move || if playing.get() { home.pause_pictures } else { home.play_pictures }
                    title=move || if playing.get() { home.pause } else { home.play }
                    on:click=toggle_play
                >
                    {move || if playing.get() { view! { <Icon name="pause"/> } } else { view! { <Icon name="play"/> } }}
                </button>
            </div>
            {map_for_dialog.map(|map| view! {
                <dialog
                    class="map-dialog"
                    node_ref=dialog
                    aria-labelledby="karte-titel"
                    on:close=move |_| map_open.set(false)
                    // A click on the dim backdrop is a click on the dialog itself.
                    on:click=move |ev: leptos::ev::MouseEvent| {
                        let on_backdrop = ev.target().and_then(|target| dialog.get_untracked().map(|dialog| {
                            let dialog: &leptos::web_sys::EventTarget = dialog.as_ref();
                            dialog == &target
                        }));
                        if on_backdrop == Some(true) {
                            close_map();
                        }
                    }
                >
                    <header class="map-dialog-head">
                        <div>
                            <h2 id="karte-titel">{home.map_heading}</h2>
                            <p>{home.map_hint}</p>
                        </div>
                        <button type="button" class="icon-btn map-dialog-close" aria-label=home.close_map on:click=move |_| close_map()><Icon name="x"/></button>
                    </header>
                    <MapStage map/>
                </dialog>
            })}
        </section>
    }
}

/// The map as a picture in the carousel: links, dots and names, nothing that reacts.
#[component]
fn MapPreview(map: Arc<ProgramMap>) -> impl IntoView {
    let t = i18n::t();
    let sheets = [("map-wide", map.wide.clone()), ("map-tall", map.tall.clone())].map(|(class, layout)| {
        let [weak, medium, strong] = link_paths(&map, &layout);
        view! {
            <svg class=format!("map map-preview {class}") viewBox=format!("0 0 {} {}", layout.width, layout.height) style=format!("--map-font:{}px", layout.font) aria-hidden="true">
                <path class="map-links" d=weak/>
                <path class="map-links medium" d=medium/>
                <path class="map-links strong" d=strong/>
                {map.programs.iter().zip(layout.dots.iter()).map(|(program, (x, y, r))| view! {
                    <g class=format!("map-dot {}", program.cycle.code())><circle cx=px(*x) cy=px(*y) r=px(*r)/></g>
                }).collect_view()}
                {layout.names.iter().filter_map(|name| map.programs.get(name.program).map(|program| view! {
                    <g><text class="map-name" x=px(name.x) y=px(name.y) text-anchor=name.anchor.code()>{program.name.clone()}</text></g>
                })).collect_view()}
            </svg>
        }
    });
    view! {
        <p class="map-key" aria-hidden="true">
            <span><i class="dot-key bachelor"></i>{t.home.bachelor}</span>
            <span><i class="dot-key master"></i>{t.home.master}</span>
            <span><i class="dot-key other"></i>{t.home.other}</span>
        </p>
        <div class="map-holder">{sheets}</div>
    }
}

/// The entry links with their counts, and the faculties: two lists on the same lines.
#[component]
fn Entries(entries: Vec<Entry>, home: HomeData) -> impl IntoView {
    let t = i18n::t();
    let counts = home.entry_counts.clone();
    view! {
        <div class="home-lists" id="einstiege">
            <section class="panel linklist-panel" aria-labelledby="einstiege-titel">
                <header class="block-head">
                    <h2 id="einstiege-titel">{t.home.ways_in}</h2>
                    <a class="ghost" href=t.path(url::CATALOG)>{(t.home.all_modules_count)(&format::count(home.overview.modules, t.locale))}<Icon name="chevron-right"/></a>
                </header>
                <ul class="rowlist">
                    {entries.into_iter().zip(counts).map(|(entry, count)| {
                        let href = t.path(&CatalogUrl { query: entry.query, ..Default::default() }.path());
                        view! {
                            <li><a class="rowlink" href=href rel="nofollow">
                                <span class=format!("ico {}", entry.tint)><Icon name=entry.icon/></span>
                                <span class="rowlink-text"><b>{entry.label}</b><small>{entry.hint}</small></span>
                                <span class="rowlink-count num">{format::count(count, t.locale)}</span>
                                <Icon name="chevron-right"/>
                            </a></li>
                        }
                    }).collect_view()}
                </ul>
            </section>
            <section class="panel linklist-panel" aria-labelledby="fakultaeten-titel">
                <header class="block-head">
                    <h2 id="fakultaeten-titel">{t.home.programs_by_faculty}</h2>
                    <a class="ghost" href=t.path(url::PROGRAMS)>{(t.home.all_programs_count)(&format::count(home.overview.programs, t.locale))}<Icon name="chevron-right"/></a>
                </header>
                <ul class="rowlist">
                    {home.faculties.iter().map(|(department, programs)| {
                        // A numbered faculty wears its colour, the same as on the map (`fac-1` … `fac-6`).
                        let (code, name, anchor, class) = match department {
                            Some(d) if d.code.chars().all(|c| c.is_ascii_digit()) => (format!("F{}", d.code), d.name_de.clone(), format!("fakultaet-{}", d.id), format!("ico code fac-{}", d.code)),
                            Some(d) => (d.code.clone(), d.name_de.clone(), format!("fakultaet-{}", d.id), "ico code".to_string()),
                            None => ("–".to_string(), t.home.no_faculty.to_string(), "ohne-fakultaet".to_string(), "ico code".to_string()),
                        };
                        let full_name = name.clone();
                        view! {
                            <li><a class="rowlink" href=t.path(&format!("{}#{anchor}", url::PROGRAMS)) title=full_name>
                                <span class=class>{code}</span>
                                <span class="rowlink-text"><b>{name}</b></span>
                                <span class="rowlink-count num">{*programs}</span>
                                <Icon name="chevron-right"/>
                            </a></li>
                        }
                    }).collect_view()}
                </ul>
            </section>
        </div>
    }
}

/// A coordinate as the SVG gets it: one decimal.
fn px(value: f64) -> String {
    ((value * 10.0).round() / 10.0).to_string()
}

/// The links of a layout as one path per weight: three elements instead of three hundred.
fn link_paths(map: &ProgramMap, layout: &Layout) -> [String; 3] {
    let mut paths = [String::new(), String::new(), String::new()];
    for link in &map.links {
        let (Some(a), Some(b)) = (layout.dots.get(link.a), layout.dots.get(link.b)) else { continue };
        let weight = if link.similarity >= 0.5 { 2 } else if link.similarity >= 0.2 { 1 } else { 0 };
        if let Some(path) = paths.get_mut(weight) {
            path.push_str(&format!("M{} {}L{} {}", a.0, a.1, b.0, b.1));
        }
    }
    paths
}

/// The class that colours a faculty: `fac-1` … `fac-6` (app.css).
fn faculty_class(map: &ProgramMap, faculty: usize) -> String {
    map.faculties.get(faculty).map(|f| format!("fac-{}", f.code)).unwrap_or_default()
}

/// The map of the programs as it reacts, in the dialog: a pointer over a dot shows its relatives,
/// a click picks it (its faculty's outline shows, the others step back, the caption links to it).
#[component]
fn MapStage(map: Arc<ProgramMap>) -> impl IntoView {
    let t = i18n::t();
    // The app only (server HTML has no state): the program under the pointer or with the focus,
    // and the one picked by a click. The map shows the first, else the second.
    let hover = RwSignal::new(None::<usize>);
    let picked = RwSignal::new(None::<usize>);
    let shown = Memo::new(move |_| hover.get().or(picked.get()));
    // The faculty of the picked program: its outline shows, the programs of the others step back.
    let picked_faculty = {
        let map = map.clone();
        Memo::new(move |_| picked.get().and_then(|i| map.programs.get(i)).and_then(|program| program.faculty))
    };
    // The colour of what is shown is its faculty's (`--pick`), not the accent of the app: the
    // highlight has to belong to the outline it stands in.
    let pick_colour = {
        let map = map.clone();
        Memo::new(move |_| {
            let code = shown.get().and_then(|i| map.programs.get(i)).and_then(|program| program.faculty).and_then(|f| map.faculties.get(f)).map(|f| f.code.clone());
            code.map_or("var(--accent)".to_string(), |code| format!("var(--fac-{code})"))
        })
    };
    // A click picks a program instead of opening it (owner wish 2026-09-21: too easily done by
    // accident); the caption links to it. A click beside the dots puts it away. With a modifier
    // the dot stays what it is without the app: a link.
    let on_click = move |ev: leptos::ev::MouseEvent| {
        if ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() {
            return;
        }
        match nav::index_under(ev.target()) {
            Some(i) => {
                ev.prevent_default();
                picked.update(|picked| *picked = if *picked == Some(i) { None } else { Some(i) });
            }
            None => picked.set(None),
        }
    };

    // Under the map, one line that never changes its height: what the map is, the program under
    // the pointer, or the picked one with the way to it.
    let caption = {
        let map = map.clone();
        move || {
            let Some((i, program)) = shown.get().and_then(|i| map.programs.get(i).map(|program| (i, program))) else {
                return view! {
                    <span class="cap-text"><b>{(t.home.program_count)(map.programs.len())}</b><span>{t.home.lines_show}</span></span>
                    <a class="cap-link" href=t.path(url::PROGRAMS)>{t.home.all_programs}<Icon name="chevron-right"/></a>
                }
                .into_any();
            };
            let faculty = program.faculty.and_then(|f| map.faculties.get(f)).map(|f| format!(" · {}", (t.home.faculty)(&f.code))).unwrap_or_default();
            let relatives = map.relatives(i);
            let named: Vec<String> = relatives.iter().take(2).filter_map(|(other, shared)| map.programs.get(*other).map(|other| (t.home.shared_with)(*shared, &other.name))).collect();
            // Beside the map on a wide screen there is room for the closest five, one per line.
            let closest: Vec<(String, usize)> = relatives.iter().take(5).filter_map(|(other, shared)| map.programs.get(*other).map(|other| (other.title(t.locale), *shared))).collect();
            let is_picked = picked.get() == Some(i);
            view! {
                <span class="cap-text">
                    <b>{program.title(t.locale)}</b>
                    <span class="quiet">{format!("{}{faculty}", (t.home.program_modules)(program.modules))}<span class="cap-shares">{(!named.is_empty()).then(|| format!(" · {}", (t.home.shares)(&named.join(", "))))}</span></span>
                </span>
                {(!closest.is_empty()).then(|| view! {
                    <div class="cap-relatives">
                        <p class="label">{t.home.shared_modules_with}</p>
                        <ol>{closest.into_iter().map(|(name, shared)| view! { <li><span>{name}</span><b class="num">{shared}</b></li> }).collect_view()}</ol>
                    </div>
                })}
                {is_picked.then(|| view! {
                    <a class="cap-link picked" href=t.path(&url::program_path(&program.slug, ProgramTab::Plan))>{t.home.to_program}<Icon name="chevron-right"/></a>
                    <button type="button" class="icon-btn cap-close" aria-label=t.home.clear_pick on:click=move |_| picked.set(None)><Icon name="x"/></button>
                })}
            }
            .into_any()
        }
    };

    let sheets = [("map-wide", map.wide.clone()), ("map-tall", map.tall.clone())].map(|(class, layout)| {
        let paths = link_paths(&map, &layout);
        let hot_links = {
            let (map, layout) = (map.clone(), layout.clone());
            move || {
                let i = shown.get()?;
                let from = *layout.dots.get(i)?;
                let mut path = String::new();
                let mut relatives = Vec::new();
                for (other, _) in map.relatives(i) {
                    if let (Some(to), Some(program)) = (layout.dots.get(other), map.programs.get(other)) {
                        path.push_str(&format!("M{} {}L{} {}", from.0, from.1, to.0, to.1));
                        relatives.push((*to, program.cycle.code()));
                    }
                }
                // Drawn over the map (which steps back): the links, the relatives as they are, the program itself.
                Some(view! {
                    <g class="map-hot">
                        <path d=path/>
                        {relatives.into_iter().map(|((x, y, r), cycle)| view! { <g><circle class=cycle cx=px(x) cy=px(y) r=px(r)/></g> }).collect_view()}
                        <circle class="self" cx=px(from.0) cy=px(from.1) r=px(from.2)/>
                    </g>
                })
            }
        };
        let [weak, medium, strong] = paths;
        view! {
            <svg class=format!("map {class}") class:has-hot=move || shown.get().is_some() class:has-pick=move || picked.get().is_some() viewBox=format!("0 0 {} {}", layout.width, layout.height) style=move || format!("--map-font:{}px;--pick:{}", layout.font, pick_colour.get())>
                // The outline of the picked program's faculty, under everything else.
                {layout.regions.iter().map(|region| {
                    let f = region.faculty;
                    view! {
                        <g><path class=format!("map-region {}", faculty_class(&map, f)) class:shown=move || picked_faculty.get() == Some(f) d=region.path.clone()/></g>
                    }
                }).collect_view()}
                <path class="map-links" d=weak/>
                <path class="map-links medium" d=medium/>
                <path class="map-links strong" d=strong/>
                // Virtual oversizing (R14): every dot takes the pointer 5 units around it. The halos lie
                // under all dots, so in a tight group a dot is never covered by its neighbour's halo.
                {map.programs.iter().zip(layout.dots.iter()).enumerate().map(|(i, (program, (x, y, r)))| view! {
                    <g>
                        <a class="map-halo" href=t.path(&url::program_path(&program.slug, ProgramTab::Plan)) data-i=i tabindex="-1" aria-hidden="true">
                            <circle cx=px(*x) cy=px(*y) r=px(*r + 5.0)/>
                        </a>
                    </g>
                }).collect_view()}
                {map.programs.iter().zip(layout.dots.iter()).enumerate().map(|(i, (program, (x, y, r)))| {
                    let program_faculty = program.faculty;
                    let faculty = program.faculty.and_then(|f| map.faculties.get(f)).map(|f| format!(" · {}", (t.home.faculty)(&f.code))).unwrap_or_default();
                    let label = format!("{} · {}{faculty}", program.title(t.locale), (t.home.program_modules)(program.modules));
                    let tooltip = label.clone();
                    let class = match program.cycle {
                        Cycle::Bachelor => "map-dot bachelor",
                        Cycle::Master => "map-dot master",
                        Cycle::Other => "map-dot other",
                    };
                    view! {
                        <g>
                            <a class=class class:picked=move || picked.get() == Some(i) class:outside=move || picked_faculty.get().is_some_and(|f| program_faculty != Some(f)) href=t.path(&url::program_path(&program.slug, ProgramTab::Plan)) data-i=i aria-label=label>
                                <title>{tooltip}</title>
                                <circle cx=px(*x) cy=px(*y) r=px(*r)/>
                            </a>
                        </g>
                    }
                }).collect_view()}
                {hot_links}
                {layout.names.iter().filter_map(|name| map.programs.get(name.program).map(|program| view! {
                    <g><text class="map-name" x=px(name.x) y=px(name.y) text-anchor=name.anchor.code()>{program.name.clone()}</text></g>
                })).collect_view()}
                {layout.regions.iter().map(|region| {
                    let f = region.faculty;
                    view! {
                        <g><text class=format!("map-faculty {}", faculty_class(&map, f)) class:shown=move || picked_faculty.get() == Some(f) x=px(region.x) y=px(region.y) text-anchor=region.anchor.code()>{region.label(&map, t.locale)}</text></g>
                    }
                }).collect_view()}
            </svg>
        }
    });

    view! {
        <div
            class="stage stage-map map-live wash-map"
            on:click=on_click
            on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Escape" && picked.get_untracked().is_some() {
                // The first Escape puts the pick away, the next one closes the dialog.
                ev.prevent_default();
                picked.set(None);
            }
            on:pointerover=move |ev| hover.set(nav::index_under(ev.target()))
            on:pointerleave=move |_| hover.set(None)
            on:focusin=move |ev| hover.set(nav::index_under(ev.target()))
            on:focusout=move |_| hover.set(None)
        >
            <div class="map-holder">{sheets}</div>
        </div>
        // On the map's upper left corner, or beside the map on a wide screen, with the caption
        // under it (the dialog's grid places them).
        <div class="map-key live-key" aria-hidden="true">
            <b class="label">{t.home.legend}</b>
            <span><i class="dot-key bachelor"></i>{t.home.bachelor}</span>
            <span><i class="dot-key master"></i>{t.home.master}</span>
            <span><i class="dot-key other"></i>{t.home.other}</span>
            <span class="key-links"><i class="link-key"></i>{t.home.shared_modules}</span>
        </div>
        <div class="cap map-cap" aria-live="polite">{caption}</div>
    }
}
