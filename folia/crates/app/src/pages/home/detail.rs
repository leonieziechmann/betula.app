//! „Betula im Detail", the end of the start page. Owner, 2026-09-28: the first version was one long
//! text — „das ist so langweilig und einfach nur eine wall of text. Mach das mal so, dass es für
//! normale User auch interessant zu betrachten ist, während das Betula-Theme nicht gebrochen
//! wird." So a panel names the chapters, and each chapter is a panel of its own: its name in its
//! tint over a headline, a line under it and four points, and beside them a picture made of the
//! app's own parts, not a drawing of them:
//!
//! - the way from four questions to the one module page,
//! - the filter panel laid out as a board, its chips in the panel's look, with an example chosen
//!   and the number of modules the catalog has for it; in the browser app the chips switch in
//!   place and the number follows,
//! - a PDF of a regulation becoming the plan's matrix, and the number of programs with a plan,
//! - a week in the Stundenplan's own grid (`week::WeekGrid`), with a clash and a choice,
//! - where a visitor's things live, and four figures (no account, no tracker …),
//! - the site with and without JavaScript,
//! - the birch from its root (Radix) to its leaves (Folia), in the season's crown and the ground's
//!   sand of the site's frame,
//! - screenshots on a computer and a phone.
//!
//! On a wide page the picture stands beside the words, every other chapter on the left; the board
//! of the filters takes the whole width under them; on a narrow page everything stacks. What a
//! picture shows that the app has words for is said in the app's words (`catalog::Texts`,
//! `studyplan_export` …), the rest is `i18n/home_detail.rs`. Everything looks the same with and
//! without JavaScript; the only numbers are the snapshot's, and a picture without them leaves them
//! out.

use folia_calendar::day::clock;
use folia_calendar::kind::EventKind;
use folia_model::labels::{Campus, Code, Labelled, ModuleKind, TeachingForm};
use folia_routes::CatalogQuery;
use folia_routes::filter::{ExamPart, Language, TurnusFilter};
use folia_routes::url::{self, CatalogUrl, ProgramTab};
use folia_pages::ask::CatalogCountAsk;
use leptos::prelude::*;

use crate::data::use_data;
use crate::format;
use crate::i18n::{self, home_detail::Chapter as Words, Texts};
use crate::pages::catalog::{duration_choices, years_choices, Choice, Toggle, Tri};
use crate::seo;
use crate::ui::{Icon, KindBadge, Mark};
use crate::week::{GridSlot, WeekGrid};

/// The browser app (`csr`): only there do the board's chips switch, and count what they choose.
const APP: bool = cfg!(feature = "csr");

/// The selection the filter board shows chosen: offered in winter, taught in English, without a
/// written exam. The start page counts it with its ways in (`HomePage`).
pub fn example() -> CatalogQuery {
    CatalogQuery {
        turnus: TurnusFilter { winter: true, ..Default::default() },
        languages: vec![Language::English],
        exam_parts_exclude: vec![ExamPart::Written],
        ..Default::default()
    }
}

/// How a chapter looks: where it stands (its id, which the row of chapters links), its tint
/// (`t-…`), the icon beside its name, and the icons of its four points.
struct Look {
    id: &'static str,
    tint: &'static str,
    icon: &'static str,
    points: [&'static str; 4],
}

const FLOW: Look = Look { id: "weg", tint: "t-ice", icon: "route", points: ["search", "panel-right-open", "link", "keyboard"] };
const FILTERS: Look = Look { id: "filter", tint: "t-violet", icon: "sliders-horizontal", points: ["circle-check-big", "eye", "graduation-cap", "smartphone"] };
const PLANS: Look = Look { id: "regelstudienplaene", tint: "t-sun", icon: "file-check-2", points: ["scan-line", "sigma", "sparkles", "waypoints"] };
const TIMETABLE: Look = Look { id: "stundenplan", tint: "t-coral", icon: "calendar-range", points: ["calendar-clock", "check-check", "triangle-alert", "calendar-plus"] };
const ACCOUNT: Look = Look { id: "ohne-konto", tint: "t-teal", icon: "shield-check", points: ["smartphone", "share-2", "calendar-plus", "server"] };
const TECHNOLOGY: Look = Look { id: "technik", tint: "t-slate", icon: "code", points: ["file-code-2", "zap", "wifi-off", "download"] };
const DATA: Look = Look { id: "daten", tint: "t-green", icon: "tree-deciduous", points: ["clock-3", "badge-check", "eye", "arrow-up-right"] };
const DEVICES: Look = Look { id: "geraete", tint: "t-rose", icon: "monitor-smartphone", points: ["languages", "moon", "keyboard", "share-2"] };

/// The chapters in their order, with their words.
fn chapters(t: &'static Texts) -> [(&'static Look, &'static Words); 8] {
    let d = &t.home_detail;
    [(&FLOW, &d.flow), (&FILTERS, &d.filters), (&PLANS, &d.plans), (&TIMETABLE, &d.timetable), (&ACCOUNT, &d.account), (&TECHNOLOGY, &d.javascript), (&DATA, &d.data), (&DEVICES, &d.devices)]
}

/// The panel that names the chapters, then the chapters. `plans`: the current programs with a
/// checked plan and all current programs; `example`: what the board's selection finds. Both are
/// the snapshot's, and missing where it could not be read.
#[component]
pub fn Details(plans: Option<(u64, u64)>, example: Option<u64>) -> impl IntoView {
    let t = i18n::t();
    let d = &t.home_detail;
    let count = |n: u64| format::count(n, t.locale);
    // The number of programs with a checked plan, in the look of the start page's figures.
    let plan_figure = plans.filter(|(plans, _)| *plans > 0).map(|(plans, programs)| {
        view! {
            <dl class="figures one">
                <div><dt>{d.plans_figure}<small>{(d.plans_of)(&count(programs))}</small></dt><dd class="num">{count(plans)}</dd></div>
            </dl>
        }
        .into_any()
    });
    let zeros = view! {
        <dl class="figures">
            {d.zeros.iter().map(|(figure, what)| view! { <div><dt>{*what}</dt><dd class="num">{*figure}</dd></div> }).collect_view()}
        </dl>
    }
    .into_any();
    view! {
        <section class="panel detail-intro" id="im-detail" aria-labelledby="im-detail-titel">
            <header class="block-head">
                <h2 id="im-detail-titel">{d.heading}</h2>
                <p>{d.lead}</p>
            </header>
            <nav class="detail-nav" aria-label=d.chapters>
                {chapters(t).into_iter().map(|(look, words)| view! {
                    <a class=look.tint href=format!("#{}", look.id) data-action="jump"><Icon name=look.icon/>{words.name}</a>
                }).collect_view()}
            </nav>
        </section>
        <Feature look=&FLOW words=&d.flow link=(d.to_catalog, url::CATALOG) picture=flow(t).into_any()/>
        <Feature look=&FILTERS words=&d.filters wide=true figure=steps(t).into_any() picture=board(t, example).into_any()/>
        <Feature look=&PLANS words=&d.plans flip=true figure=plan_figure.into_any() link=(d.to_programs, url::PROGRAMS) picture=plan(t).into_any()/>
        <Feature look=&TIMETABLE words=&d.timetable link=(d.to_studyplan, url::STUDYPLAN) picture=week(t).into_any()/>
        <Feature look=&ACCOUNT words=&d.account flip=true figure=zeros link=(d.to_privacy, url::PRIVACY) picture=places(t).into_any()/>
        <Feature look=&TECHNOLOGY words=&d.javascript picture=modes(t).into_any()/>
        <Feature look=&DATA words=&d.data flip=true bare=true picture=tree(t).into_any()/>
        <Feature look=&DEVICES words=&d.devices picture=devices(t).into_any()/>
    }
}

/// A chapter as a panel: its words (name, headline, the line under it, what stands in their middle,
/// the four points, the way on) and its picture — beside them on a wide page, on the left where
/// `flip`, and under them across the whole width where `wide`. `bare`: the picture brings its own
/// ground instead of the wash of the tint.
#[component]
fn Feature(
    look: &'static Look,
    words: &'static Words,
    picture: AnyView,
    #[prop(optional)] figure: Option<AnyView>,
    #[prop(optional)] link: Option<(&'static str, &'static str)>,
    #[prop(optional)] flip: bool,
    #[prop(optional)] wide: bool,
    #[prop(optional)] bare: bool,
) -> impl IntoView {
    let t = i18n::t();
    let title = format!("{}-titel", look.id);
    view! {
        <section class=format!("panel feature {}", look.tint) class:flip=flip class:wide=wide id=look.id aria-labelledby=format!("{}-titel", look.id)>
            <div class="feature-text">
                <p class="feature-name"><span class="ico"><Icon name=look.icon/></span>{words.name}</p>
                <h3 id=title>{words.title}</h3>
                <p class="feature-lead">{words.lead}</p>
                {figure}
                <ul class="points">
                    {words.points.iter().zip(look.points).map(|(point, icon)| view! {
                        <li><Icon name=icon/><div><b>{point.title}</b><p>{point.text}</p></div></li>
                    }).collect_view()}
                </ul>
                {link.map(|(words, path)| view! { <a class="ghost feature-link" href=t.path(path) rel=crate::seo::nofollow(path)>{words}<Icon name="chevron-right"/></a> })}
            </div>
            <div class="feature-picture" class:bare=bare>{picture}</div>
        </section>
    }
}

/// The address of the catalog with a filter, in the page's language.
fn catalog_link(t: &'static Texts, query: CatalogQuery) -> String {
    t.path(&CatalogUrl { query, ..Default::default() }.path())
}

/// What a chip shows: the box with its state (empty, ticked, crossed), an icon, the label.
fn face(label: String, icon: Option<&'static str>) -> impl IntoView {
    view! {
        <span class="box"><Icon name="check"/><Icon name="x"/></span>
        {icon.map(|name| view! { <Icon name=name/> })}
        <span class="chip-label">{label}</span>
    }
}

/// A chip in the look of the filter panel (`.chip` with its box, `data-state` off, with or
/// without), and nothing to click: the steps of „Klausur", the chips of the board that need more
/// than a click (a program, the Merkliste, the Stundenplan), and every chip of the board where the
/// app does not run.
fn look(label: String, icon: Option<&'static str>, state: Tri) -> AnyView {
    view! { <span class="chip" data-state=state.code() aria-disabled="true">{face(label, icon)}</span> }.into_any()
}

/// A chip of the board that filters by itself. In the browser app it switches the board's
/// selection (`chosen`) the way its chip in the catalog's panel switches the list — off, with,
/// without, the same toggle — and the page stays: the number under the board follows. On the
/// server's page (without JavaScript, and until the app runs) it is its look in the example's
/// state; a link there would leave the page as well.
fn switch(t: &'static Texts, chosen: RwSignal<CatalogQuery>, label: String, icon: Option<&'static str>, toggle: Toggle) -> AnyView {
    if !APP {
        let state = chosen.with_untracked(|q| (toggle.read)(q));
        return look(label, icon, state);
    }
    let read = toggle.read.clone();
    let state = Memo::new(move |_| chosen.with(|q| read(q)));
    let excludes = toggle.excludes;
    let name = label.clone();
    view! {
        <button
            type="button"
            class="chip"
            role="checkbox"
            data-state=move || state.get().code()
            aria-checked=move || state.get().checked()
            aria-label=move || match state.get() {
                Tri::Without => (t.catalog.excluded)(&name),
                _ => name.clone(),
            }
            title=move || match (state.get(), excludes) {
                (Tri::Off, true) => Some(t.catalog.chip_off),
                (Tri::With, true) => Some(t.catalog.chip_with),
                (Tri::Without, _) => Some(t.catalog.chip_without),
                _ => None,
            }
            on:click=move |_| chosen.update(|q| (toggle.write)(q, toggle.after((toggle.read)(q))))
        >
            {face(label, icon)}
        </button>
    }
    .into_any()
}

/// A row of choices in the look of the panel's segmented rows, the `on`-th one raised.
fn seg(items: Vec<String>, on: usize) -> AnyView {
    view! {
        <span class="seg mock-seg">
            {items.into_iter().enumerate().map(|(i, item)| view! { <span aria-checked=(i == on).then_some("true")><span class="seg-label">{item}</span></span> }).collect_view()}
        </span>
    }
    .into_any()
}

/// A row of choices of the filter itself (the duration, the years): in the browser app one of them
/// is chosen in the board's selection by a click, as the panel's row chooses in the list's; else
/// its look, the example's choice raised.
fn choose(chosen: RwSignal<CatalogQuery>, label: &'static str, choices: Vec<Choice>) -> AnyView {
    if !APP {
        let on = chosen.with_untracked(|q| choices.iter().position(|choice| (choice.is_on)(q))).unwrap_or(0);
        return seg(choices.into_iter().map(|choice| choice.label).collect(), on);
    }
    view! {
        <span class="seg mock-seg" role="radiogroup" aria-label=label>
            {choices.into_iter().map(|Choice { label, is_on, choose, .. }| view! {
                <button type="button" role="radio" aria-checked=move || if chosen.with(|q| is_on(q)) { "true" } else { "false" } on:click=move |_| chosen.update(|q| choose(q))>
                    <span class="seg-label">{label}</span>
                </button>
            }).collect_view()}
        </span>
    }
    .into_any()
}

/// A field of the panel (a search, a picker), with what is typed into it or what it offers.
fn field(icon: &'static str, text: String, typed: bool) -> AnyView {
    view! { <span class="mock-field" class:typed=typed><Icon name=icon/><span>{text}</span></span> }.into_any()
}

/// „Klausur" in its three steps: off, only with, all but — a picture of them; the board under it
/// is where they switch.
fn steps(t: &'static Texts) -> impl IntoView {
    let d = &t.home_detail;
    let written = ExamPart::Written.short_label(t.locale).to_string();
    let [off, only, but] = d.steps;
    view! {
        <div class="tri">
            <figure>{look(written.clone(), None, Tri::Off)}<figcaption>{off}</figcaption></figure>
            <Icon name="arrow-right"/>
            <figure>{look(written.clone(), None, Tri::With)}<figcaption>{only}</figcaption></figure>
            <Icon name="arrow-right"/>
            <figure>{look(written, None, Tri::Without)}<figcaption>{but}</figcaption></figure>
        </div>
    }
}

/// Four questions, each a way through the app, and the module page they all end on.
fn flow(t: &'static Texts) -> impl IntoView {
    let d = &t.home_detail;
    let c = &t.catalog;
    let english = format!("{}: {}", c.language, Language::English.label(t.locale));
    let ways: [(&'static str, Vec<String>); 4] = [
        ("search", vec![c.tag_search.to_string(), d.as_you_type.to_string()]),
        ("graduation-cap", vec![t.home.choose_program.to_string(), ProgramTab::Plan.label(t.locale).to_string(), d.third_semester.to_string()]),
        ("languages", vec![t.program.in_catalog.to_string(), c.area.to_string(), english]),
        ("calendar-range", vec![t.app.studyplan.to_string(), c.fits.to_string()]),
    ];
    let parts = ["layout-list", "file-check-2", "calendar-days", "repeat", "graduation-cap"];
    view! {
        <div class="flow">
            <ol class="lanes">
                {ways.into_iter().zip(d.questions).map(|((icon, steps), question)| {
                    let last = steps.len().saturating_sub(1);
                    view! {
                        <li class="lane">
                            <p class="lane-ask"><Icon name=icon/>{question}</p>
                            <p class="steps">
                                {steps.into_iter().enumerate().map(|(i, step)| view! {
                                    <span class="step">{step}</span>
                                    {(i < last).then(|| view! { <Icon name="chevron-right"/> })}
                                }).collect_view()}
                            </p>
                        </li>
                    }
                }).collect_view()}
            </ol>
            <div class="goal">
                <p class="goal-head"><span class="ico"><Icon name="file-text"/></span>{d.module_page}</p>
                <p class="goal-badges" aria-hidden="true"><i></i><i></i><i></i></p>
                <ul>{d.module_parts.iter().zip(parts).map(|(part, icon)| view! { <li><Icon name=icon/>{*part}</li> }).collect_view()}</ul>
            </div>
        </div>
    }
}

/// The filter panel laid out as a board, in its order and its look, with the example chosen
/// (winter, English, no written exam), and under it how many modules the catalog has for what is
/// chosen and the way there. On a phone it shows the groups a first look needs (`key`: the search,
/// the program and the example's) and the others after „Alle Filter zeigen" — a checkbox, so that
/// it opens with and without JavaScript alike.
///
/// Owner, 2026-09-29: a click on one of its filters led straight into the catalog („mach das so,
/// dass die filter tatsächlich funktionieren und man dann unten sieht, wie viele module das
/// selected hat und sich die angucken kann"). So in the browser app every chip that filters by
/// itself, and the rows of the duration and the years, switch the board's selection in place, as
/// they do in the catalog's panel; the number under the board follows (from the local copy of the
/// catalog), and its button opens the catalog with that selection. What needs more than a click —
/// a program and what comes with it, the Merkliste, the Stundenplan, a name typed, the slider —
/// stays its picture. Without the app the board is a picture as a whole, its button the example's.
fn board(t: &'static Texts, example_count: Option<u64>) -> impl IntoView {
    let d = &t.home_detail;
    let h = &d.hints;
    let c = &t.catalog;
    let locale = t.locale;
    let chosen = RwSignal::new(example());
    // The server's number is the example's, which its loader counted already; the app counts what
    // is chosen (the local catalog keeps an answer, so choosing again asks nothing).
    let source = use_data().ok().filter(|_| APP);
    let count = Memo::new(move |_| match &source {
        Some(source) => chosen.with(|q| source.now(&CatalogCountAsk { query: q.clone() })).ok(),
        None => example_count,
    });
    let group = |label: String, body: AnyView, hint: Option<&'static str>, key: bool| {
        view! {
            <div class="bgroup" class:key=key>
                <p class="flabel label">{label}</p>
                {body}
                {hint.map(|hint| view! { <p class="bhint">{hint}</p> })}
            </div>
        }
    };
    let chips = |items: Vec<AnyView>| view! { <span class="chips">{items}</span> }.into_any();
    let chip = move |label: &str, icon: Option<&'static str>, toggle: Toggle| switch(t, chosen, label.to_string(), icon, toggle);
    let turnus = vec![
        chip(c.winter_chip, Some("snowflake"), Toggle::turnus(|f| (f.winter, f.not_winter), |f| (&mut f.winter, &mut f.not_winter))),
        chip(c.summer_chip, Some("sun"), Toggle::turnus(|f| (f.summer, f.not_summer), |f| (&mut f.summer, &mut f.not_summer))),
        chip(c.irregular_chip, Some("shuffle"), Toggle::turnus(|f| (f.irregular, f.not_irregular), |f| (&mut f.irregular, &mut f.not_irregular))),
    ];
    let forms = [TeachingForm::Lecture, TeachingForm::Exercise, TeachingForm::Seminar, TeachingForm::Practical, TeachingForm::Project, TeachingForm::Excursion]
        .into_iter()
        .map(|form| chip(form.label(locale), None, Toggle::teaching_form(form)))
        .collect();
    let exams = ExamPart::ALL.iter().map(|part| chip(part.short_label(locale), None, Toggle::exam_part(*part))).collect();
    let languages = Language::ALL.iter().map(|language| chip(language.label(locale), None, Toggle::language(*language))).collect();
    let kinds = [ModuleKind::Compulsory, ModuleKind::Elective, ModuleKind::Thesis, ModuleKind::Internship]
        .into_iter()
        .map(|kind| look(kind.label(locale).to_string(), None, if kind == ModuleKind::Elective { Tri::With } else { Tri::Off }))
        .chain([look(c.not_stated.to_string(), None, Tri::Off)])
        .collect();
    let properties = vec![
        chip(c.graded_chip, None, Toggle::flag(|q| q.graded, |q, value| q.graded = value)),
        chip(c.limited_chip, None, Toggle::flag(|q| q.limited, |q, value| q.limited = value)),
        chip(c.fues_list, None, Toggle::flag(|q| q.fues, |q, value| q.fues = value)),
        look(c.saved_chip.to_string(), Some("bookmark"), Tri::Off),
    ];
    let campuses = [Campus::Zentralcampus, Campus::Sachsendorf, Campus::Senftenberg].into_iter().map(|campus| chip(campus.label(locale), None, Toggle::campus(campus))).collect();
    let dates = vec![chip(c.confirmed, Some("calendar-check-2"), Toggle::flag(|q| q.scheduled, |q, value| q.scheduled = value)), look(c.fits.to_string(), None, Tri::Off)];
    let semesters = std::iter::once(c.all.to_string()).chain((1..=6).map(|n| n.to_string())).chain(["?".to_string()]).collect();
    let credits = (t.format.credits)(&format!("{}–{}", format::number(6.0, locale), format::number(12.0, locale)));
    view! {
        <input class="board-all" type="checkbox" id="alle-filter"/>
        <div class="board">
            {group(c.tag_search.to_string(), view! { <span class="mock-field typed"><Icon name="search"/><span>{h.search_example}</span><kbd>{t.common.search_shortcut}</kbd></span> }.into_any(), Some(h.search), true)}
            {group(c.program.to_string(), view! {
                {field("graduation-cap", h.program_typed.to_string(), true)}
                <span class="mock-hit"><Icon name="check"/>{h.program_found}</span>
            }.into_any(), Some(h.program), true)}
            {group(h.with_program.to_string(), view! {
                {seg(vec![c.curriculum.to_string(), "FÜS".to_string()], 0)}
                {chips(kinds)}
                {field("layout-list", h.area_example.to_string(), false)}
                {seg(semesters, 3)}
            }.into_any(), Some(h.with_program_hint), false)}
            {group(c.dates.to_string(), chips(dates), Some(h.dates), false)}
            {group(c.offered_in.to_string(), chips(turnus), None, true)}
            {group(c.teaching_form.to_string(), chips(forms), None, false)}
            {group(c.exam.to_string(), chips(exams), Some(h.exam), true)}
            {group(c.credit_points.to_string(), view! {
                <span class="mock-slider" aria-hidden="true"><i></i><i></i></span>
                <span class="mock-range num">{credits}</span>
            }.into_any(), Some(h.credits), false)}
            {group(c.language.to_string(), chips(languages), None, true)}
            {group(c.properties.to_string(), chips(properties), None, false)}
            {group(c.more_filters.to_string(), view! {
                <p class="flabel label sub">{c.lecturers}</p>
                {field("users-round", c.search_name.to_string(), false)}
                <p class="bhint people"><span><b>"+"</b>{c.people_any}</span><span><b>"×"</b>{c.people_none}</span></p>
                <p class="flabel label sub">{c.department}</p>
                {field("building-2", c.all_departments.to_string(), false)}
                <p class="flabel label sub">{c.duration}</p>
                {choose(chosen, c.duration, duration_choices(t))}
                <p class="flabel label sub">{c.years_only}</p>
                {choose(chosen, c.years, years_choices(t))}
                <p class="flabel label sub">{c.location}</p>
                {chips(campuses)}
                {chips(vec![chip(c.show_not_offered, None, Toggle::show_not_offered())])}
            }.into_any(), Some(c.location_hint), false)}
            {group(h.sort.to_string(), seg(vec![c.col_module.to_string(), t.common.credits_unit.to_string(), c.dates.to_string()], 0), Some(h.sort_hint), false)}
        </div>
        <label class="board-more btn secondary" for="alle-filter"><Icon name="chevron-down"/>{h.all}</label>
        // What is chosen, counted, and the way to it: said again while the chips switch.
        <p class="board-foot" aria-live="polite">
            {move || count.get().map(|n| view! {
                <a class="btn secondary" href=catalog_link(t, chosen.get()) rel="nofollow"><Icon name="sliders-horizontal"/>{(d.example)(n, &format::count(n, locale))}<Icon name="chevron-right"/></a>
            })}
        </p>
    }
}

/// A page of a regulation (a faint table, a line of light reading it) becoming the plan in
/// Betula: modules by semester, a module over two semesters, a range of credits, and the sums of
/// the regulation, each checked.
fn plan(t: &'static Texts) -> impl IntoView {
    let d = &t.home_detail;
    // The table of the PDF, as a sheet shows it from a distance: the head, then rows whose cells
    // hold a number or not.
    const PAGE: [[bool; 6]; 6] = [
        [true, false, false, false, false, false],
        [true, true, false, false, false, false],
        [false, false, true, true, false, false],
        [false, false, false, false, true, false],
        [false, true, false, true, false, false],
        [false, false, false, false, false, true],
    ];
    // The rows of the matrix: kind (Pflicht, Wahlpflicht, Abschlussarbeit) and the row's cell: its
    // first column (2 = the first semester), how many semesters it spans, and what it says.
    let rows: [(ModuleKind, u8, u8, &str); 5] = [
        (ModuleKind::Compulsory, 2, 1, "8"),
        (ModuleKind::Compulsory, 2, 2, "12"),
        (ModuleKind::Elective, 4, 2, "10–24"),
        (ModuleKind::Compulsory, 6, 1, "12"),
        (ModuleKind::Thesis, 7, 1, "12"),
    ];
    view! {
        <div class="planscan">
            <figure class="plansheet paper">
                <figcaption><Icon name="file-text"/>{d.pdf}</figcaption>
                <div class="paper-table" aria-hidden="true">
                    <i class="head"></i>{(1..=6).map(|_| view! { <i class="head"></i> }).collect_view()}
                    {PAGE.iter().map(|row| view! {
                        <i class="name"></i>
                        {row.iter().map(|on| view! { <i class:on=*on></i> }).collect_view()}
                    }).collect_view()}
                </div>
                <i class="beam" aria-hidden="true"></i>
            </figure>
            <span class="planscan-arrow" aria-hidden="true"><Icon name="arrow-right"/></span>
            <figure class="plansheet planmx">
                <figcaption><span class="mini-mark"><Mark/></span>{d.in_betula}</figcaption>
                <div class="mx">
                    <div class="mx-row mx-head"><span>{t.catalog.col_module}</span>{(1..=6).map(|n| view! { <span>{n.to_string()}</span> }).collect_view()}</div>
                    {rows.into_iter().zip(d.plan_rows).map(|((kind, column, span, credits), name)| view! {
                        <div class="mx-row">
                            <span class=format!("mx-name kind k-{}", kind.code())><i title=kind.label(t.locale)></i>{name}</span>
                            <span class="mx-cell" style=format!("--c:{column};--n:{span}")>{credits}</span>
                        </div>
                    }).collect_view()}
                    <div class="mx-row mx-sums">
                        <span class="mx-name">{d.plan_sum}</span>
                        {(1..=6).map(|_| view! { <span><Icon name="check"/>"30"</span> }).collect_view()}
                    </div>
                </div>
                <p class="mx-key">
                    {[ModuleKind::Compulsory, ModuleKind::Elective, ModuleKind::Thesis].into_iter().map(|kind| view! { <KindBadge kind=Some(Code::Known(kind))/> }).collect_view()}
                </p>
            </figure>
        </div>
    }
}

/// A week of the Stundenplan in its own grid: a lecture, an Übung with two groups to choose from,
/// two lectures at one time (the clash, framed in red), and the ways out into a calendar.
fn week(t: &'static Texts) -> impl IntoView {
    let d = &t.home_detail;
    let [maths, programming, physics, writing] = d.week_modules;
    let slot = |day: u8, from: u16, to: u16, kind: EventKind, name: &str, hue: &'static str| GridSlot {
        day,
        from,
        to,
        label: format!("{} {name}", kind.short(t.locale)),
        small: clock(from),
        class: "tinted",
        hue: Some(hue),
        ..Default::default()
    };
    let slots = vec![
        slot(1, 555, 645, EventKind::Lecture, maths, "t-ice"),
        GridSlot { alt: true, ..slot(1, 825, 915, EventKind::Exercise, maths, "t-ice") },
        GridSlot { alt: true, ..slot(3, 690, 780, EventKind::Exercise, maths, "t-ice") },
        GridSlot { clash: true, ..slot(2, 555, 645, EventKind::Lecture, programming, "t-sun") },
        GridSlot { clash: true, ..slot(2, 555, 645, EventKind::Lecture, physics, "t-violet") },
        slot(4, 690, 780, EventKind::Lecture, physics, "t-violet"),
        slot(4, 930, 1020, EventKind::Seminar, writing, "t-teal"),
        GridSlot { small: t.studyplan_head.week_a.to_string(), ..slot(5, 555, 750, EventKind::Practical, physics, "t-violet") },
    ];
    let e = &t.studyplan_export;
    let calendars = [("download", e.download), ("calendar-plus", e.apple), ("calendar-plus", e.google), ("calendar-plus", "Outlook"), ("share-2", t.studyplan_share.copy_link)];
    view! {
        <div class="demo-week">
            <p class="demo-week-head"><span class="label">{d.example_week}</span><span class="clashline"><i></i>{(t.studyplan_head.clashes_per_week)(1)}</span></p>
            <WeekGrid slots=Signal::derive(move || slots.clone()) fit=true/>
        </div>
        <p class="calendars">
            {calendars.into_iter().map(|(icon, label)| view! { <span class="badge"><Icon name=icon/>{label}</span> }).collect_view()}
        </p>
    }
}

/// Where a visitor's things live: in the browser, which holds all of them; on the server, which
/// holds the same for everybody; and between them the link that carries the Merkliste.
fn places(t: &'static Texts) -> impl IntoView {
    let d = &t.home_detail;
    let mine = [("bookmark", t.app.bookmarks), ("calendar-range", t.app.studyplan), ("copy", d.saved_plans), ("star", t.myprogram.mine), ("moon", d.settings), ("database", d.catalog_copy)];
    let host = seo::site_url();
    let host = host.split("://").nth(1).unwrap_or(&host).to_string();
    let transfer = format!("{host}{}#m=…", t.path(url::BOOKMARKS));
    view! {
        <div class="places">
            <div class="place mine">
                <p class="place-head"><Icon name="monitor"/>{d.in_browser}</p>
                <ul>{mine.into_iter().map(|(icon, label)| view! { <li><Icon name=icon/>{label}</li> }).collect_view()}</ul>
            </div>
            <div class="place-link">
                <Icon name="link"/>
                <code>{transfer}</code>
                <small>{d.after_hash}</small>
            </div>
            <div class="place server">
                <p class="place-head"><Icon name="server"/>{d.on_server}</p>
                <p>{d.for_everyone}</p>
                <p class="nothing"><Icon name="eye-off"/>{d.nothing_of_yours}</p>
            </div>
        </div>
    }
}

/// The site without JavaScript and with it, side by side, and what it is made of.
fn modes(t: &'static Texts) -> impl IntoView {
    let d = &t.home_detail;
    let column = |mode: &'static i18n::home_detail::Mode, icon: &'static str, on: bool| {
        view! {
            <div class="mode" class:on=on>
                <p class="mode-head"><Icon name=icon/>{mode.title}</p>
                <ul>{mode.items.iter().map(|item| view! { <li><Icon name="check"/>{*item}</li> }).collect_view()}</ul>
            </div>
        }
    };
    view! {
        <div class="modes">
            {column(&d.without_js, "file-code-2", false)}
            {column(&d.with_js, "zap", true)}
        </div>
        <p class="stack">{["Rust", "WebAssembly", "SQLite", "Service Worker"].into_iter().map(|name| view! { <span class="badge">{name}</span> }).collect_view()}</p>
    }
}

/// The birch Betula is named after, drawn as the site draws its frame: the season's crown on top
/// (Folia, the leaves), a trunk of birch bark (the Datenstand), and the ground with its roots
/// (Radix) holding the sources, each with how often it is read.
fn tree(t: &'static Texts) -> impl IntoView {
    let d = &t.home_detail;
    let tag = |icon: &'static str, (name, text): (&'static str, &'static str)| {
        view! { <p class="tree-tag"><Icon name=icon/><b>{name}</b><span>{text}</span></p> }
    };
    view! {
        <div class="tree">
            // As high as the rows beside it: `slice` cuts off what the drawing has more.
            <svg class="trunk" viewBox="0 0 24 320" preserveAspectRatio="xMidYMin slice" aria-hidden="true">
                <rect width="24" height="320" rx="4"/>
                <path d="M0 58h9v3H0zM15 76h9v3h-9zM0 93h5v3H0zM11 110h13v3H11zM0 128h10v3H0zM17 145h7v3h-7zM0 162h7v3H0zM13 180h11v3H13zM0 197h12v3H0zM19 214h5v3h-5zM0 232h6v3H0zM12 249h12v3H12zM0 266h9v3H0zM16 284h8v3h-8zM0 301h11v3H0z"/>
            </svg>
            {tag("leaf", d.folia)}
            {tag("database", d.trunk)}
            <div class="tree-ground">
                {tag("sprout", d.radix)}
                <ul class="sources">{d.sources.iter().map(|(source, when)| view! { <li>{*source}<em>{*when}</em></li> }).collect_view()}</ul>
            </div>
        </div>
    }
}

/// The catalog on a computer and a module on a phone (the start page's screenshots, in the theme
/// shown), and the keys and switches of every page.
fn devices(t: &'static Texts) -> impl IntoView {
    // Lazy, and `loading` before the address, as in the carousel: only what shows is fetched,
    // never the hidden theme's pictures.
    let shots = |file: &'static str, alt: &'static str, width: u32, height: u32| {
        [false, true]
            .map(|dark| {
                let suffix = if dark { "-dark" } else { "" };
                view! { <img class=if dark { "shot-dark" } else { "shot-light" } loading="lazy" decoding="async" alt=alt width=width height=height src=format!("{}/{file}{suffix}.webp", crate::SHOTS)/> }
            })
            .into_iter()
            .collect_view()
    };
    view! {
        <div class="devices">
            <figure class="screen desktop">{shots("catalog", t.home.catalog_alt, 1200, 900)}</figure>
            <figure class="screen phone">{shots("module-phone", t.home_detail.phone_alt, 720, 960)}</figure>
        </div>
        <p class="device-keys">
            <kbd>{t.common.search_shortcut}</kbd><kbd>"↑"</kbd><kbd>"↓"</kbd><kbd>"Enter"</kbd><kbd>"M"</kbd><kbd>"F"</kbd><kbd>"Esc"</kbd>
            <span class="badge"><Icon name="languages"/>"DE · EN"</span>
            <span class="badge"><Icon name="sun"/><Icon name="moon"/></span>
        </p>
    }
}
