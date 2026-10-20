//! „Studiengang wählen" of „Über Betula"'s first panel (`ProgramPick`): until concept A it was the
//! first of three steps of a way in for a first visit under that panel (owner, 2026-09-28: one
//! click a step, organically, without a pop-up or a tour). The start page's setup took those steps
//! over (`super::setup`), and the button stayed: in the browser app, while this browser keeps no
//! program, it picks one in place, else it leads to all of them.

use folia_routes::url;
use folia_pages::ask::ProgramsAsk;
use leptos::prelude::*;

use folia_design::combobox::{ComboItem, Combobox};
use folia_data::use_data;
use crate::i18n;
use folia_stores::myprogram::{program_name, po_of, MyProgram};
use folia_design::nav;
use folia_design::ui::Icon;

/// The browser app (`csr`): only there is anything stored, and only there does a picker work.
const APP: bool = cfg!(feature = "csr");

/// „Studiengang wählen": in the browser app, while this browser keeps no program, a picker of all
/// programs (the catalog's `Combobox`), and a pick is „Mein Studiengang" at once, the page staying
/// where it is. Once a program is kept, and without JavaScript, the way to all programs,
/// „Alle Studiengänge" (owner, 2026-09-28). The server's link carries both words and the stylesheet
/// shows the ones the app will show (`html.js`, and `html.mine` where the head script found a
/// program), so they stay when the app takes over; where a picker comes, with the picker's chevron
/// and its room (without them the words moved by 10 px at the takeover on a phone).
#[component]
pub fn ProgramPick(
    /// The id of the button; the picker's popup derives the ids of its parts from it.
    id: &'static str,
    /// The classes of the link (`btn secondary`); a picker is styled by its place.
    class: &'static str,
) -> impl IntoView {
    let t = i18n::t();
    if !APP {
        return view! {
            <a id=id class=format!("{class} program-pick") href=t.path(url::PROGRAMS)>
                <Icon name="graduation-cap"/>
                <span class="if-choose">{t.home.choose_program}</span>
                <span class="if-all">{t.home.all_programs}</span>
                <Icon name="chevrons-up-down" class="combo-chevron if-choose"/>
            </a>
        }
        .into_any();
    }
    let mine = MyProgram::expect();
    let source = use_data().ok();
    let kept = Memo::new(move |_| mine.is_some_and(|mine| mine.with(|doc| doc.program.is_some())));
    // Asked of the catalog only once the picker opens: a memo computes when it is first read.
    let programs = Memo::new(move |before| match source.as_ref() {
        Some(source) => folia_pages::ask::unless_pending(source.now(&ProgramsAsk {}), before, Result::unwrap_or_default),
        None => Vec::new(),
    });
    let items = Signal::derive(move || {
        programs.with(|all| {
            all.iter()
                .map(|program| ComboItem::new(program.id.clone(), program.name.clone(), format!("{} · PO {}", program.degree(), po_of(program)), i64::from(program.is_latest_po)))
                .collect::<Vec<_>>()
        })
    });
    let pick = Callback::new(move |picked: Option<String>| {
        let Some(picked) = picked else { return };
        let Some(program) = programs.with_untracked(|all| all.iter().find(|program| program.id == picked).cloned()) else { return };
        if let Some(mine) = mine {
            mine.set_program(&program.id, &program_name(&program), "", None);
        }
        // The picker gives way to „Alle Studiengänge", under the same id: the focus goes there.
        request_animation_frame(move || nav::focus_by_id(id));
    });
    (move || {
        if kept.get() {
            view! { <a id=id class=class href=t.path(url::PROGRAMS)><Icon name="graduation-cap"/>{t.home.all_programs}</a> }.into_any()
        } else {
            view! {
                <div class="pick-combo">
                    <Combobox
                        id=id
                        label=t.catalog.program
                        placeholder=t.home.choose_program
                        search_placeholder=t.catalog.search_program
                        icon="graduation-cap"
                        min_width=420.0
                        items
                        selected=Signal::derive(|| None::<String>)
                        on_select=pick
                        clearable=false
                    />
                </div>
            }
            .into_any()
        }
    })
    .into_any()
}
