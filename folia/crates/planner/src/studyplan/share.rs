//! „Stundenplan teilen" (owner, 2026-09-26): a link that hands the semester shown on
//! (`/studyplan?share=<code>`, `folia_calendar::share`), and on the page it opens, the offer to
//! take its modules over.
//!
//! The link is the visitor's own act, and the exception R20 makes for it is what it carries: the
//! semester, the planned modules in their order, the timetable's program — so that the preview a
//! messenger shows of it names the modules („MIT-1, AuP, EEG"), which the messenger can read from
//! the address alone — and the groups ticked, so that whoever opens it can put the plan beside
//! their own as a calendar with its owner's groups (`calendars.rs`). Nothing of what is hidden
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
//! The link copied is in the page's language (`/en/studyplan?share=…`), so that whoever opens it
//! reads the offer in the language it was handed on in.

use folia_calendar::share::SharedPlan;
use folia_pages::ask::SharedPlanAsk;
use folia_pages::SharedPlanData;
use leptos::prelude::*;
use leptos_router::NavigateOptions;

use crate::i18n;
use folia_shell::pending::Pending;
use folia_design::ui::Icon;
use super::import::{now_secs, taken_parts};
use super::PlanCtx;

/// „Link zum Teilen kopieren" in the sidebar's group „Plan": the address that hands the semester
/// shown on, in the page's language, copied whole (`data-absolute`). Greyed out while nothing is
/// planned that a code can carry.
#[component]
pub(super) fn ShareAction(ctx: PlanCtx) -> impl IntoView {
    let t = i18n::t();
    let s = &t.studyplan_share;
    // The groups ticked of the semester shown: the selection is a sibling of `wanted` (R16). The
    // link carries the planned modules, not the visits: whoever takes it over plans what it holds.
    let link = Memo::new(move |_| {
        let (chosen, visits) = ctx.selection.with(|(_, selection)| (selection.chosen_rows.clone(), selection.visits.clone()));
        ctx.wanted.with(|(key, ids, program)| {
            let planned: Vec<String> = ids.iter().filter(|id| !visits.contains(*id)).cloned().collect();
            share_path(*key, &planned, program.as_deref(), chosen)
        })
    });
    move || match link.get() {
        Some(link) => view! {
            <a class="action" href="#" data-action="copy-text" data-absolute="" data-text=t.path(&link) title=s.link_title>
                <Icon name="share-2"/>
                <span><span data-label="">{s.copy_link}</span><small>{s.with_modules}</small></span>
            </a>
        }
        .into_any(),
        None => view! {
            <button class="action" type="button" aria-disabled="true" title=s.nothing_planned><Icon name="share-2"/><span>{s.copy_link}</span></button>
        }
        .into_any(),
    }
}

/// The address that hands the modules `ids` of the semester `key` on, named as in `program`, with
/// the groups `chosen` ticked as far as the code carries them; `None` when no module can travel. A
/// path of the app: the link writes it as `t.path(…)`.
fn share_path(key: folia_calendar::semester::SemesterKey, ids: &[String], program: Option<&str>, chosen: impl IntoIterator<Item = folia_calendar::rowkey::RowKey>) -> Option<String> {
    let code = SharedPlan::of(key, ids, program)?.with_chosen(chosen).code().ok()?;
    Some(folia_routes::url::share_path(&code))
}

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
    let offer = Memo::new(move |_| {
        let page = ctx.key.get();
        shared.with(|shared| {
            let (plan, data) = shared.as_ref()?;
            let theirs = plan.key()?;
            let planned = ctx.plan.map(|plan| plan.modules_in(theirs)).unwrap_or_default();
            Some(offer_of(data, theirs == page, &planned))
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
                    (format!("{}{}", s.taken, taken_parts(added, 0, t)), before)
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
        let Some(offer) = offer.get() else {
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
        let label = shared.with(|shared| shared.as_ref().map(|(_, data)| data.label.clone())).unwrap_or_default();
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
            let notes = [t.studyplan_share.taken, t.studyplan_import.imported, t.studyplan_side.cleared, super::super::head::FROM_MARKED];
            for (i, a) in notes.iter().enumerate() {
                assert!(notes.iter().enumerate().all(|(j, b)| i == j || !a.starts_with(b)), "{a}");
            }
        }
    }

    #[test]
    fn only_a_plan_with_a_module_is_handed_on() {
        let key = SemesterKey::parse("2026W").unwrap();
        let tick = folia_calendar::rowkey::RowKey { event: 148_370, fp: 0xeb017 };
        let path = share_path(key, &["12104".to_string(), "11101".to_string()], Some("079-82-2008"), [tick]).unwrap();
        let code = path.strip_prefix("/studyplan?share=").unwrap();
        // The link of an English page opens the plan in English.
        assert_eq!(EN.path(&path), format!("/en/studyplan?share={code}"));
        let plan = SharedPlan::from_code(code).unwrap();
        assert_eq!((plan.module_ids(), plan.program.as_deref(), plan.chosen_rows()), (vec!["12104".to_string(), "11101".to_string()], Some("079-82-2008"), vec![tick]));
        assert_eq!(share_path(key, &[], None, []), None);
    }
}
