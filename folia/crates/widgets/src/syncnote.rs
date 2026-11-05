//! The hints of Betula Sync (`folia_stores::sync::Conflict`) where the thing is: what two devices
//! of a profile did to it at the same time. Nothing is lost by it, and nothing is decided for the
//! person: the later version stands, the other one is offered beside it, and a button takes it over
//! or keeps what stands (research/sync/entwurf.html, „Notizen: die spätere Fassung und „andere
//! Fassung“").
//!
//! - A module's note (`NoteConflict`), at the note on the module's page and in the menu where it is
//!   written (`collect`).
//! - A Termin's note (`TerminNoteConflict`), at the note in the Termin's view (`termin`).
//! - A module moved on two devices (`MovedNotes`), on „Mein Studium": it stands in every semester
//!   it was moved to, and a button keeps it in one of them.
//!
//! Nothing on the server, which knows no profile (R9), and nothing where the profile does not sync.

use leptos::prelude::*;

use folia_calendar::rowkey::RowKey;
use folia_calendar::semester::SemesterKey;
use folia_design::ui::Icon;
use folia_stores::collections::Collections;
use folia_stores::studyplan::Studyplan;
use folia_stores::sync::{Conflict, Sync};

use crate::i18n;

/// The hint of a module's note typed on two devices at once, where there is one.
#[component]
pub fn NoteConflict(#[prop(into)] module: String) -> impl IntoView {
    let sync = Sync::expect();
    let conflict = Memo::new(move |_| {
        sync.and_then(|sync| sync.conflicts().into_iter().rev().find(|conflict| matches!(conflict, Conflict::ModuleNote { module: of, .. } if *of == module)))
    });
    move || {
        conflict.get().map(|conflict| {
            let Conflict::ModuleNote { module, other, .. } = conflict.clone() else { return ().into_any() };
            let take = Callback::new(move |()| {
                if let Some(collections) = Collections::expect() {
                    collections.update(|shelf| shelf.set_note(&module, &other));
                }
            });
            let other = note_of(&conflict);
            view! { <Hint conflict other take/> }.into_any()
        })
    }
}

/// The hint of a Termin's note typed on two devices at once, where there is one.
#[component]
pub fn TerminNoteConflict(semester: Signal<Option<SemesterKey>>, row: RowKey) -> impl IntoView {
    let sync = Sync::expect();
    let conflict = Memo::new(move |_| {
        let semester = semester.get()?;
        sync.and_then(|sync| sync.conflicts().into_iter().rev().find(|conflict| matches!(conflict, Conflict::TerminNote { semester: of, row: at, .. } if *of == semester && *at == row)))
    });
    move || {
        conflict.get().map(|conflict| {
            let Conflict::TerminNote { semester, row, other, .. } = conflict.clone() else { return ().into_any() };
            let take = Callback::new(move |()| {
                if let Some(plan) = Studyplan::expect() {
                    let other = other.clone();
                    plan.update_after_paint(move |doc| {
                        doc.set_note(semester, row, &other);
                    });
                }
            });
            let other = note_of(&conflict);
            view! { <Hint conflict other take/> }.into_any()
        })
    }
}

/// The other version of a note, as the hint shows it; `None` where it deleted the note.
fn note_of(conflict: &Conflict) -> Option<String> {
    match conflict {
        Conflict::ModuleNote { other, .. } | Conflict::TerminNote { other, .. } => Some(other.trim().to_string()).filter(|text| !text.is_empty()),
        Conflict::Moved { .. } => None,
    }
}

/// A note's hint: what happened, the other version, „Diese Fassung übernehmen" and „Behalten". Both
/// let the hint go; taking the other version over is a change of the store, made by `take` first.
#[component]
fn Hint(conflict: Conflict, other: Option<String>, take: Callback<()>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let sync = Sync::expect();
    let (to_take, to_keep) = (conflict.clone(), conflict);
    let take_it = move |_| {
        take.run(());
        if let Some(sync) = sync {
            sync.dismiss(&to_take);
        }
    };
    let keep = move |_| {
        if let Some(sync) = sync {
            sync.dismiss(&to_keep);
        }
    };
    let said = match other {
        Some(text) => view! { <p class="sync-hint-said">{s.note_other}</p><blockquote class="sync-hint-other">{text}</blockquote> }.into_any(),
        None => view! { <p class="sync-hint-said">{s.note_other_none}</p> }.into_any(),
    };
    view! {
        <div class="sync-hint" role="status">
            <Icon name="monitor-smartphone"/>
            <div class="sync-hint-body">
                <b>{s.note_heading}</b>
                {said}
                <div class="sync-hint-actions">
                    <button class="mini primary hit" type="button" on:click=take_it>{s.take_other}</button>
                    <button class="mini hit" type="button" on:click=keep>{s.keep}</button>
                </div>
            </div>
        </div>
    }
}

/// The modules moved on two devices at once, each with its semesters: „Nur in …" keeps it in that
/// semester alone (out of the others), „Alle behalten" lets it stand where it is. `title` names a
/// module by its id (its title where the page knows it).
#[component]
pub fn MovedNotes(title: Callback<String, String>) -> impl IntoView {
    let t = i18n::t();
    let s = &t.sync;
    let sync = Sync::expect();
    let plan = Studyplan::expect();
    // Only what still stands in more than one semester: a hint whose module was taken out or kept
    // in one semester since is done.
    let moved = Memo::new(move |_| {
        let conflicts = sync.map(Sync::conflicts).unwrap_or_default();
        conflicts
            .into_iter()
            .filter_map(|conflict| match &conflict {
                Conflict::Moved { module, .. } => {
                    let semesters = plan.map(|plan| plan.with(|doc| doc.planned_in(module))).unwrap_or_default();
                    (semesters.len() > 1).then(|| (conflict.clone(), module.clone(), semesters))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    move || {
        let all = moved.get();
        (!all.is_empty()).then(|| {
            view! {
                <div class="sync-moved" role="status">
                    {all.into_iter().map(|(conflict, module, semesters)| {
                        let named: Vec<String> = semesters.iter().map(|semester| semester.label(t.locale)).collect();
                        let said = (s.moved)(&title.run(module.clone()), &listed(&named, s.and));
                        let all_kept = conflict.clone();
                        let choices = semesters.iter().zip(named).map(|(semester, name)| {
                            let (keep, module, others, conflict) = (*semester, module.clone(), semesters.clone(), conflict.clone());
                            let only = move |_| {
                                if let Some(plan) = plan {
                                    let (module, others) = (module.clone(), others.clone());
                                    plan.update_after_paint(move |doc| {
                                        for other in others.into_iter().filter(|other| *other != keep) {
                                            doc.unplan(other, &module, &[]);
                                        }
                                    });
                                }
                                if let Some(sync) = sync {
                                    sync.dismiss(&conflict);
                                }
                            };
                            view! { <button class="mini hit" type="button" on:click=only>{(s.only_in)(&name)}</button> }
                        }).collect_view();
                        let keep_all = move |_| {
                            if let Some(sync) = sync {
                                sync.dismiss(&all_kept);
                            }
                        };
                        view! {
                            <div class="sync-hint">
                                <Icon name="monitor-smartphone"/>
                                <div class="sync-hint-body">
                                    <p class="sync-hint-said">{said}</p>
                                    <div class="sync-hint-actions">
                                        {choices}
                                        <button class="mini hit" type="button" on:click=keep_all>{s.keep_all}</button>
                                    </div>
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>
            }
        })
    }
}

/// „A, B und C".
fn listed(items: &[String], and: &str) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} {and} {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semesters_are_listed_as_a_sentence_says_them() {
        let names = |items: &[&str]| items.iter().map(|item| item.to_string()).collect::<Vec<_>>();
        assert_eq!(listed(&names(&[]), "und"), "");
        assert_eq!(listed(&names(&["WiSe 2026/27"]), "und"), "WiSe 2026/27");
        assert_eq!(listed(&names(&["WiSe 2026/27", "SoSe 2027"]), "und"), "WiSe 2026/27 und SoSe 2027");
        assert_eq!(listed(&names(&["A", "B", "C"]), "and"), "A, B and C");
    }

    #[test]
    fn the_other_version_of_a_note_is_what_it_says() {
        let note = |other: &str| Conflict::ModuleNote { module: "11787".into(), kept: Some("eins".into()), other: other.into(), at: 1 };
        assert_eq!(note_of(&note(" Altklausur mit Lena ")), Some("Altklausur mit Lena".to_string()));
        assert_eq!(note_of(&note("  ")), None);
        assert_eq!(note_of(&Conflict::Moved { module: "11787".into(), semesters: Vec::new(), at: 1 }), None);
    }
}
