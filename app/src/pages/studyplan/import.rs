//! „Importieren" in the sidebar of the Stundenplan (owner's redesign of 2026-09-25): one
//! Fachsemester of the timetable's program's Regelstudienplan into the semester shown, on top of
//! what is planned (`studyplan::import_fs`; „Plan leeren" empties). A program with several plans
//! has a select for the plan; the Fachsemester starts at the first one of the current semester's
//! half of the year. „Mein Plan" (the whole-study plan of the program's page) is the second source
//! to come, shown disabled.
//!
//! „Übernehmen" answers at once and writes after the next frame (R21): the modules and
//! placeholders, the program with them, and „Mein Studiengang" where none is set yet. „Übernommen:
//! 4 Module, 1 Platzhalter · Rückgängig" stands under it until the next change of the kind
//! (`PlanCtx::undo`, which „Plan leeren" shares).

use catalog::labels::Season;
use catalog::pages::{self, PlanSource};
use catalog::rows::Program;
use catalog::studyplan::{self, MineDoc, PlanDoc};
use catalog::timetable::semester::SemesterKey;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use super::{key_of, PlanCtx};
use crate::format;
use crate::i18n;
use crate::myprogram::program_name;
use crate::nav;
use crate::pending::Pending;
use crate::ui::Icon;

/// The id of „Übernehmen".
const GO_ID: &str = "sp-import-go";

/// The import as the sidebar shows it, for a program with a plan.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Form {
    /// The plans to choose from (index and label), when there are two or more.
    pub cores: Vec<(usize, String)>,
    pub core: usize,
    /// The page that fills the core plan's direction row, and that row's `ord`.
    pub page: Option<(usize, i64)>,
    /// The plan's last Fachsemester, and the one chosen.
    pub most: u8,
    pub fs: u8,
    /// „4 Module · 1 Platzhalter", or what the timetable holds of it already.
    pub adds: String,
    pub any: bool,
}

/// The plans a Regelstudienplan is taken over as: every plan that fills no row of another.
fn cores(source: &PlanSource) -> Vec<usize> {
    (0..source.variants.len()).filter(|index| !source.supplements.iter().any(|s| s.page == *index)).collect()
}

/// The plan to start with: the one „Mein Studiengang" stored (by its caption), else the one the
/// address names (`variant=`, 1-based as on the program's page, the core of a page it names),
/// else the first.
fn default_core(source: &PlanSource, caption: Option<&str>, variant: Option<usize>) -> Option<usize> {
    let cores = cores(source);
    let stored = caption
        .and_then(|caption| catalog::variants::variant_for(&source.variants, caption))
        .and_then(|found| source.variants.iter().position(|variant| std::ptr::eq(variant, found)));
    let named = variant.and_then(|variant| variant.checked_sub(1)).map(|index| source.supplements.iter().find(|s| s.page == index).map_or(index, |s| s.core));
    stored.into_iter().chain(named).find(|index| cores.contains(index)).or_else(|| cores.first().copied())
}

/// The page that fills a row of `core`: the direction „Mein Studiengang" stored, else the page the
/// address names; with the `ord` of the row it fills. None: the row stays a placeholder.
fn default_page(source: &PlanSource, core: usize, direction: Option<&str>, variant: Option<usize>) -> Option<(usize, i64)> {
    let pages: Vec<_> = source.supplements.iter().filter(|s| s.core == core).collect();
    let stored = direction.and_then(|direction| pages.iter().find(|s| source.variants.get(s.page).is_some_and(|page| page.full.trim() == direction.trim())));
    let named = variant.and_then(|variant| variant.checked_sub(1)).and_then(|index| pages.iter().find(|s| s.page == index));
    stored.or(named).map(|s| (s.page, s.ord))
}

/// The Fachsemester to start with: the first of the half of the year of `semester`, the plan's
/// odd ones being those of its intake (a winter intake where the plan cannot tell).
fn default_fs(intake: Option<Season>, semester: SemesterKey, most: u8) -> u8 {
    let intake = intake.unwrap_or(Season::Winter);
    let first = if semester.season() == intake { 1 } else { 2 };
    first.min(most.max(1))
}

/// The form for `source`: the defaults where nothing was picked, and what the import adds to the
/// timetable of `semester`. `variant` is the plan the address names for this program.
#[allow(clippy::too_many_arguments)]
pub(super) fn form_of(source: &PlanSource, doc: &PlanDoc, mine: &MineDoc, semester: SemesterKey, variant: Option<usize>, core: Option<usize>, fs: Option<u8>, t: &i18n::Texts) -> Option<Form> {
    let own = mine.program.as_deref() == Some(source.program.id.as_str());
    let (caption, direction) = if own && variant.is_none() { (mine.caption.as_deref(), mine.direction.as_deref()) } else { (None, None) };
    let listed = cores(source);
    let core = core.filter(|core| listed.contains(core)).or_else(|| default_core(source, caption, variant))?;
    let plan = source.variants.get(core)?;
    let page = default_page(source, core, direction, variant);
    let most = u8::try_from(plan.semesters.clamp(1, 30)).unwrap_or(1);
    let fs = fs.filter(|fs| (1..=most).contains(fs)).unwrap_or_else(|| default_fs(studyplan::intake_season(plan, &source.linked), semester, most));
    let page_plan = page.and_then(|(page, ord)| Some((source.variants.get(page)?, ord)));
    let import = studyplan::import_fs(doc, &source.program.id, plan, page_plan, semester, fs);
    let mut adds = Vec::new();
    if !import.modules.is_empty() {
        adds.push(format::modules(i64::try_from(import.modules.len()).unwrap_or(i64::MAX), t.locale));
    }
    if !import.placeholders.is_empty() {
        adds.push((t.studyplan_head.placeholders)(import.placeholders.len()));
    }
    let any = !adds.is_empty();
    let adds = match (any, import.skipped) {
        (true, _) => adds.join(" · "),
        (false, 0) => t.studyplan_import.nothing_to_take.to_string(),
        (false, _) => t.studyplan_import.already.to_string(),
    };
    let cores = match listed.len() {
        0 | 1 => Vec::new(),
        _ => listed.iter().filter_map(|index| Some((*index, source.variants.get(*index)?.label.clone()))).collect(),
    };
    Some(Form { cores, core, page, most, fs, adds, any })
}

/// „Übernommen: 4 Module, 1 Platzhalter": the note of an import (`PlanCtx::undo`), which begins
/// with `studyplan_import::imported`; the sidebar's „Plan geleert" and the note of a plan taken
/// over from a link are the others.
fn imported_note(modules: usize, placeholders: usize, t: &i18n::Texts) -> String {
    format!("{}{}", t.studyplan_import.imported, taken_parts(modules, placeholders, t))
}

/// „4 Module, 1 Platzhalter", „nichts": what a note says was taken over.
pub(super) fn taken_parts(modules: usize, placeholders: usize, t: &i18n::Texts) -> String {
    let mut parts = Vec::new();
    if modules > 0 {
        parts.push(format::modules(i64::try_from(modules).unwrap_or(i64::MAX), t.locale));
    }
    if placeholders > 0 {
        parts.push((t.studyplan_head.placeholders)(placeholders));
    }
    if parts.is_empty() {
        parts.push(t.studyplan_import.nothing.to_string());
    }
    parts.join(", ")
}

/// Seconds since 1970, for when the modules were planned; 0 outside the browser.
pub(super) fn now_secs() -> u64 {
    #[cfg(feature = "csr")]
    {
        let millis = web_sys::js_sys::Date::now();
        if millis.is_finite() && millis > 0.0 {
            return (millis / 1000.0) as u64;
        }
    }
    0
}

/// What the plans of the timetable's program say.
#[derive(Clone, Debug, PartialEq)]
enum Plans {
    NoProgram,
    NoPlan,
    Found(Box<PlanSource>),
}

/// „Importieren": the source, the plan where there are several, the Fachsemester and
/// „Übernehmen". `program` is the timetable's (`side::Programs::shown`); `imported` keeps the
/// Fachsemester last taken over, for the name „Plan speichern" suggests.
#[component]
pub(super) fn ImportGroup(ctx: PlanCtx, program: Memo<Option<Program>>, imported: RwSignal<Option<u8>>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.studyplan_import;
    let id = Memo::new(move |_| program.with(|program| program.as_ref().map(|program| program.id.clone())));
    let plans = Memo::new(move |_| {
        let Some(id) = id.get() else { return Plans::NoProgram };
        match ctx.source.with_value(|source| source.as_ref().and_then(|source| source.run(|db| pages::plan_source(db, &id, t.locale)).ok().flatten())) {
            Some(plans) if !plans.variants.is_empty() => Plans::Found(Box::new(plans)),
            _ => Plans::NoPlan,
        }
    });
    // The picks belong to the program they were made for.
    let core_pick = RwSignal::new(None::<(String, usize)>);
    let fs_pick = RwSignal::new(None::<(String, u8)>);
    let busy = RwSignal::new(false);
    // The address as a memo of this group's own: the program's comes from `ctx.url` too, and no
    // closure reads a memo together with one derived from it (R16).
    let address = Memo::new(move |_| ctx.url.get());
    let form = Memo::new(move |_| {
        let (url, current) = (address.get(), ctx.current.get());
        let mine = ctx.mine.map(|mine| mine.with(Clone::clone)).unwrap_or_default();
        let (core, fs) = (core_pick.get(), fs_pick.get());
        plans.with(|plans| {
            let Plans::Found(source) = plans else { return None };
            let id = source.program.id.as_str();
            let core = core.filter(|(of, _)| of == id).map(|(_, core)| core);
            let fs = fs.filter(|(of, _)| of == id).map(|(_, fs)| fs);
            // The plan the address names belongs to the program it names.
            let variant = url.import.as_deref().filter(|slug| *slug == source.program.slug).map(|_| url.variant);
            let doc = ctx.plan.map(|plan| plan.with(Clone::clone)).unwrap_or_default();
            let semester = key_of(&url, current, &doc, ctx.today);
            form_of(source, &doc, &mine, semester, variant, core, fs, t)
        })
    });
    let cores = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.cores.clone()).unwrap_or_default()));
    let core = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.core)));
    let semesters = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| (1..=form.most).collect::<Vec<u8>>()).unwrap_or_default()));
    let fs = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| form.fs)));
    let adds = Memo::new(move |_| form.with(|form| form.as_ref().map(|form| (form.adds.clone(), form.any))));
    let state = Memo::new(move |_| {
        plans.with(|plans| match plans {
            Plans::NoProgram => 0,
            Plans::NoPlan => 1,
            Plans::Found(_) => 2,
        })
    });
    let note = Memo::new(move |_| ctx.undo.with(|undo| undo.as_ref().map(|(note, _)| note.clone()).filter(|note| note.starts_with(s.imported))));

    let pick_core = move |ev: leptos::ev::Event| {
        let (Some(id), Ok(index)) = (id.get_untracked(), event_target_value(&ev).parse::<usize>()) else { return };
        core_pick.set(Some((id, index)));
    };
    let pick_fs = move |ev: leptos::ev::Event| {
        let (Some(id), Ok(fs)) = (id.get_untracked(), event_target_value(&ev).parse::<u8>()) else { return };
        fs_pick.set(Some((id, fs)));
    };
    let take = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(form) = form.get_untracked().filter(|form| form.any) else { return };
        let taken = plans.with_untracked(|plans| match plans {
            Plans::Found(source) => {
                let core = source.variants.get(form.core)?.clone();
                let page = form.page.and_then(|(page, ord)| Some((source.variants.get(page)?.clone(), ord)));
                Some((source.program.clone(), core, page))
            }
            _ => None,
        });
        let Some((program, core, page)) = taken else { return };
        busy.set(true);
        let semester = ctx.key.get_untracked();
        let (plan, mine, undo, going) = (ctx.plan, ctx.mine, ctx.undo, Pending::expect());
        let away = ctx.url.with_untracked(|url| url.import.is_some().then(|| url.without_import().path()));
        nav::after_paint(move || {
            if let Some(plan) = plan {
                let note = plan.update(|doc| {
                    let before = doc.clone();
                    doc.program = Some(program.id.clone());
                    let import = studyplan::import_fs(doc, &program.id, &core, page.as_ref().map(|(page, ord)| (page, *ord)), semester, form.fs);
                    let (modules, placeholders) = doc.apply(&import, now_secs());
                    (imported_note(modules, placeholders, t), before)
                });
                let _ = undo.try_set(Some(note));
            }
            let _ = imported.try_set(Some(form.fs));
            // The first program taken is „Mein Studiengang" too, as a pick of it would be.
            if let Some(mine) = mine.filter(|mine| untrack(|| mine.with(|doc| doc.program.is_none()))) {
                mine.set_program(&program.id, &program_name(&program), &core.full, page.as_ref().map(|(page, _)| page.full.as_str()));
            }
            if let (Some(going), Some(away)) = (going, away) {
                going.go(&away, NavigateOptions { replace: true, ..Default::default() });
            }
            let _ = busy.try_set(false);
        });
    };
    let restoring = RwSignal::new(false);
    let restore = move |_| {
        let (Some(plan), Some((_, before))) = (ctx.plan, ctx.undo.get_untracked()) else { return };
        if restoring.get_untracked() {
            return;
        }
        restoring.set(true);
        let undo = ctx.undo;
        plan.update_after_paint(move |doc| {
            *doc = before;
            let _ = undo.try_set(None);
            let _ = restoring.try_set(false);
        });
    };

    view! {
        <div class="fgroup sp-import" id="sp-import">
            <p class="flabel label">{s.import}</p>
            <div class="seg" role="radiogroup" aria-label=s.source>
                <button type="button" role="radio" aria-checked="true">{s.standard_plan}</button>
                <button type="button" role="radio" aria-checked="false" aria-disabled="true" title=s.my_plan_soon>
                    {s.my_plan}<small>{s.soon}</small>
                </button>
            </div>
            {move || match state.get() {
                0 => view! { <p class="hint">{s.choose_program_first}</p> }.into_any(),
                1 => view! { <p class="hint">{s.no_plan}</p> }.into_any(),
                _ => view! {
                    {move || {
                        (!cores.with(Vec::is_empty)).then(|| view! {
                            <span class="select-wrap plain">
                                <select aria-label=s.plan prop:value=move || core.get().map(|core| core.to_string()).unwrap_or_default() on:change=pick_core>
                                    <For
                                        each=move || cores.get()
                                        key=|entry| entry.clone()
                                        children=move |(index, label): (usize, String)| view! { <option value=index.to_string() selected=move || core.get() == Some(index)>{label}</option> }
                                    />
                                </select>
                                <Icon name="chevrons-up-down"/>
                            </span>
                        })
                    }}
                    <div class="sp-import-go">
                        <span class="select-wrap plain">
                            <select aria-label=s.fachsemester prop:value=move || fs.get().map(|fs| fs.to_string()).unwrap_or_default() on:change=pick_fs>
                                <For
                                    each=move || semesters.get()
                                    key=|fs| *fs
                                    children=move |n: u8| view! { <option value=n.to_string() selected=move || fs.get() == Some(n)>{(s.fs)(n)}</option> }
                                />
                            </select>
                            <Icon name="chevrons-up-down"/>
                        </span>
                        <button
                            class="btn primary"
                            type="button"
                            id=GO_ID
                            disabled=move || !adds.with(|adds| adds.as_ref().is_some_and(|(_, any)| *any))
                            aria-busy=move || busy.get().then_some("true")
                            on:click=take
                        >
                            {s.take}
                        </button>
                    </div>
                    {move || match note.get() {
                        Some(note) => view! {
                            <p class="action note-action">
                                <Icon name="check"/>
                                <span>{note}</span>
                                <button class="mini hit" type="button" aria-busy=move || restoring.get().then_some("true") on:click=restore>{t.common.undo}</button>
                            </p>
                        }
                        .into_any(),
                        None => view! { <p class="hint num">{move || adds.get().map(|(adds, _)| adds)}</p> }.into_any(),
                    }}
                }
                .into_any(),
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{DE, EN};

    fn key(text: &str) -> SemesterKey {
        SemesterKey::parse(text).unwrap()
    }

    #[test]
    fn the_fachsemester_starts_in_the_half_of_the_year_shown() {
        // A winter intake: 1. FS in a winter, 2. FS in a summer.
        assert_eq!(default_fs(Some(Season::Winter), key("2026W"), 6), 1);
        assert_eq!(default_fs(Some(Season::Winter), key("2027S"), 6), 2);
        // A summer intake the other way round; unknown counts as a winter intake.
        assert_eq!(default_fs(Some(Season::Summer), key("2026W"), 6), 2);
        assert_eq!(default_fs(None, key("2027S"), 6), 2);
        // A plan of one semester has only the first.
        assert_eq!(default_fs(None, key("2027S"), 1), 1);
        assert_eq!(imported_note(4, 1, &DE), "Übernommen: 4 Module, 1 Platzhalter");
        assert_eq!(imported_note(0, 0, &DE), "Übernommen: nichts");
        assert_eq!((imported_note(4, 1, &EN), imported_note(1, 2, &EN)), ("Imported: 4 modules, 1 placeholder".to_string(), "Imported: 1 module, 2 placeholders".to_string()));
    }
}
