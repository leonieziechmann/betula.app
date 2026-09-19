use leptos::prelude::*;
use leptos::ev;
use crate::models::{FilterOptions, ProgramOption};
use crate::components::study_program_selector::StudyProgramSelection;
use crate::components::segmented_control::{SegmentedControl, SegmentedOption};
use crate::components::range_slider::DualRangeSlider;
use crate::components::turnus_matrix::TurnusMatrix;
use crate::db::get_all_departments;

/// Sidebar containing all catalog filters grouped into flat full-width accordions
#[component]
pub fn FilterSidebar(
    on_close_sidebar: Callback<()>,
    all_programs: Signal<Vec<ProgramOption>>,
    total_modules_count: Signal<i64>,
    selected_program_id: Signal<String>,
    selected_program_name: Signal<String>,
    selected_po_version: Signal<String>,
    filters: RwSignal<FilterOptions>,
    program_combobox_open: RwSignal<bool>,
    on_select_program: Callback<(String, String, String)>,
    on_clear_program: Callback<()>,
    on_reset_filters: Callback<()>,
    active_filter_count: Signal<usize>,
    on_open_program_page: Callback<String>,
) -> impl IntoView {
    // Local input for Dozent search
    let prof_input = RwSignal::new(String::new());

    // Departments list
    let departments = Memo::new(move |_| get_all_departments());

    // Limitation options
    let limitation_options = vec![
        SegmentedOption::new("ja".to_string(), "Alle"),
        SegmentedOption::new("nein".to_string(), "Ohne Limit"),
        SegmentedOption::new("nur".to_string(), "Nur Beschränkt"),
    ];

    // FÜS options
    let fues_options = vec![
        SegmentedOption::new("inkl".to_string(), "Alle"),
        SegmentedOption::new("exkl".to_string(), "Ohne FÜS"),
        SegmentedOption::new("nur".to_string(), "Nur FÜS"),
    ];

    // Modulart options
    let module_type_options = vec![
        SegmentedOption::new("alle".to_string(), "Alle"),
        SegmentedOption::new("pflicht".to_string(), "Pflicht"),
        SegmentedOption::new("wahlpflicht".to_string(), "Wahlpfl."),
        SegmentedOption::new("fues".to_string(), "FÜS"),
    ];

    // Benotung options
    let grading_options = vec![
        SegmentedOption::new("alle".to_string(), "Alle"),
        SegmentedOption::new("benotet".to_string(), "Nur Benotet"),
        SegmentedOption::new("unbenotet".to_string(), "Unbenotet"),
    ];

    // Moduldauer options
    let duration_options = vec![
        SegmentedOption::new("alle".to_string(), "Alle"),
        SegmentedOption::new("1".to_string(), "1 Semester"),
        SegmentedOption::new("2".to_string(), "2 Semester"),
    ];

    // Toggle all accordions
    let toggle_all_accordions = move |_| {
        filters.update(|f| {
            let any_open = f.acc_stg_open || f.acc_prof_open || f.acc_turnus_open || f.acc_filter_open || f.acc_ort_open;
            let new_state = !any_open;
            f.acc_stg_open = new_state;
            f.acc_prof_open = new_state;
            f.acc_turnus_open = new_state;
            f.acc_filter_open = new_state;
            f.acc_ort_open = new_state;
        });
    };

    // Add Dozent chip
    let add_prof_include = move || {
        let val = prof_input.get().trim().to_string();
        if !val.is_empty() {
            filters.update(|f| {
                if !f.prof_includes.contains(&val) {
                    f.prof_includes.push(val);
                }
            });
            prof_input.set(String::new());
        }
    };

    let add_prof_exclude = move || {
        let val = prof_input.get().trim().to_string();
        if !val.is_empty() {
            filters.update(|f| {
                if !f.prof_excludes.contains(&val) {
                    f.prof_excludes.push(val);
                }
            });
            prof_input.set(String::new());
        }
    };

    view! {
        <div class="sidebar-header">
            <a href="/catalogue" class="sidebar-brand">
                <span class="brand-badge">"BTU"</span>
                <span class="brand-title">"Smart Modulkatalog"</span>
            </a>
            <div style="display: flex; align-items: center; gap: 0.35rem;">
                <button
                    type="button"
                    class="btn-toggle-all-subtle"
                    on:click=toggle_all_accordions
                    title="Alle Akkordeons auf-/zuklappen"
                >
                    "↕ Alle"
                </button>
                <button
                    type="button"
                    class="btn-sidebar-close"
                    on:click=move |_| on_close_sidebar.run(())
                    aria-label="Filter schließen"
                >
                    "×"
                </button>
            </div>
        </div>

        <div class="sidebar-body">
            <form on:submit=move |ev| ev.prevent_default()>

                // =================================================================
                // 1. GROUP: Studiengang & Semester
                // =================================================================
                <div class=move || if filters.get().acc_stg_open { "flat-accordion is-open" } else { "flat-accordion" }>
                    <button
                        type="button"
                        class="flat-accordion-header"
                        on:click=move |_| filters.update(|f| f.acc_stg_open = !f.acc_stg_open)
                    >
                        <div class="header-title-row">
                            <span class="header-title">"Studiengang & Semester"</span>
                            {move || {
                                let prog = selected_program_name.get();
                                if !selected_program_id.get().is_empty() {
                                    view! { <span class="header-count-dezent">{format!("({})", prog)}</span> }.into_any()
                                } else {
                                    view! {}.into_any()
                                }
                            }}
                        </div>
                        <span class="header-arrow">
                            {move || if filters.get().acc_stg_open { "▴" } else { "▾" }}
                        </span>
                    </button>

                    <div class="flat-accordion-content">
                        <div>
                            <span class="filter-label">"Studiengang:"</span>
                            <StudyProgramSelection
                                all_programs=all_programs
                                total_modules_count=total_modules_count
                                selected_program_id=selected_program_id
                                selected_program_name=selected_program_name
                                selected_po_version=selected_po_version
                                program_combobox_open=program_combobox_open
                                on_select_program=on_select_program
                                on_clear_program=on_clear_program
                                on_open_program_page=on_open_program_page
                            />
                        </div>

                        // Fachsemester Quick Buttons
                        <div>
                            <div class="filter-label-row">
                                <span class="filter-label" style="margin-bottom:0;">"Fachsemester:"</span>
                                <button
                                    type="button"
                                    style="background:none; border:none; color:var(--primary); font-size:0.75rem; cursor:pointer; font-weight:600;"
                                    on:click=move |_| filters.update(|f| f.semester = None)
                                >
                                    "Alle"
                                </button>
                            </div>
                            <div class="semester-grid" style="margin-top: 0.35rem;">
                                {vec![1i64, 2, 3, 4, 5, 6].into_iter().map(|sem| {
                                    view! {
                                        <button
                                            type="button"
                                            class=move || if filters.get().semester == Some(sem) { "btn-sem active" } else { "btn-sem" }
                                            on:click=move |_| {
                                                filters.update(|f| {
                                                    if f.semester == Some(sem) {
                                                        f.semester = None;
                                                    } else {
                                                        f.semester = Some(sem);
                                                    }
                                                });
                                            }
                                        >
                                            {if sem == 6 { "6+".to_string() } else { sem.to_string() }}
                                        </button>
                                    }
                                }).collect_view()}
                                <button
                                    type="button"
                                    class=move || if filters.get().semester == Some(0) { "btn-sem active" } else { "btn-sem" }
                                    on:click=move |_| {
                                        filters.update(|f| {
                                            if f.semester == Some(0) {
                                                f.semester = None;
                                            } else {
                                                f.semester = Some(0);
                                            }
                                        });
                                    }
                                    title="Ohne Semesterzuordnung"
                                >
                                    "?"
                                </button>
                            </div>
                        </div>

                        // Modulart
                        <div>
                            <span class="filter-label">"Modulart:"</span>
                            <SegmentedControl
                                options=module_type_options
                                selected=Signal::derive(move || filters.get().module_type)
                                on_select=Callback::new(move |val: String| {
                                    filters.update(|f| f.module_type = val);
                                })
                            />
                        </div>
                    </div>
                </div>

                // =================================================================
                // 2. GROUP: Dozierende & Lehrstühle
                // =================================================================
                <div class=move || if filters.get().acc_prof_open { "flat-accordion is-open" } else { "flat-accordion" }>
                    <button
                        type="button"
                        class="flat-accordion-header"
                        on:click=move |_| filters.update(|f| f.acc_prof_open = !f.acc_prof_open)
                    >
                        <div class="header-title-row">
                            <span class="header-title">"Dozierende & Lehrstühle"</span>
                            {move || {
                                let inc = filters.get().prof_includes.len();
                                let exc = filters.get().prof_excludes.len();
                                let total = inc + exc;
                                if total > 0 {
                                    view! { <span class="header-count-dezent">{format!("({} aktiv)", total)}</span> }.into_any()
                                } else {
                                    view! {}.into_any()
                                }
                            }}
                        </div>
                        <span class="header-arrow">
                            {move || if filters.get().acc_prof_open { "▴" } else { "▾" }}
                        </span>
                    </button>

                    <div class="flat-accordion-content">
                        <div>
                            <span class="filter-label">"Dozent:in suchen & filtern:"</span>
                            <div class="prof-input-row">
                                <input
                                    type="text"
                                    class="prof-text-input"
                                    placeholder="Name eingeben..."
                                    prop:value=move || prof_input.get()
                                    on:input=move |ev| prof_input.set(event_target_value(&ev))
                                    on:keydown=move |ev: ev::KeyboardEvent| {
                                        if ev.key() == "Enter" {
                                            ev.prevent_default();
                                            add_prof_include();
                                        }
                                    }
                                />
                                <button
                                    type="button"
                                    class="btn-prof-action inc"
                                    title="Als Whitelist (Einschließen) hinzufügen"
                                    on:click=move |_| add_prof_include()
                                >
                                    "+ Include"
                                </button>
                                <button
                                    type="button"
                                    class="btn-prof-action exc"
                                    title="Als Blacklist (Ausschließen) hinzufügen"
                                    on:click=move |_| add_prof_exclude()
                                >
                                    "− Exclude"
                                </button>
                            </div>

                            // Tag chips list
                            <div class="prof-tags-wrap">
                                {move || {
                                    let includes = filters.get().prof_includes;
                                    includes.into_iter().map(|name| {
                                        let name_clone = name.clone();
                                        view! {
                                            <div class="prof-tag-item inc">
                                                <span>{format!("+ {}", name)}</span>
                                                <button
                                                    type="button"
                                                    class="prof-tag-remove"
                                                    on:click=move |_| {
                                                        let n = name_clone.clone();
                                                        filters.update(|f| f.prof_includes.retain(|x| x != &n));
                                                    }
                                                >
                                                    "×"
                                                </button>
                                            </div>
                                        }
                                    }).collect_view()
                                }}

                                {move || {
                                    let excludes = filters.get().prof_excludes;
                                    excludes.into_iter().map(|name| {
                                        let name_clone = name.clone();
                                        view! {
                                            <div class="prof-tag-item exc">
                                                <span>{format!("− {}", name)}</span>
                                                <button
                                                    type="button"
                                                    class="prof-tag-remove"
                                                    on:click=move |_| {
                                                        let n = name_clone.clone();
                                                        filters.update(|f| f.prof_excludes.retain(|x| x != &n));
                                                    }
                                                >
                                                    "×"
                                                </button>
                                            </div>
                                        }
                                    }).collect_view()
                                }}
                            </div>
                        </div>

                        // Department Dropdown
                        <div>
                            <span class="filter-label">"Fachgebiet / Institut:"</span>
                            <select
                                class="combobox-display"
                                style="height: 38px;"
                                on:change=move |ev| {
                                    let val = event_target_value(&ev);
                                    filters.update(|f| f.department = val);
                                }
                            >
                                <option value="" selected=move || filters.get().department.is_empty()>"Alle Institute & Fachgebiete"</option>
                                {move || {
                                    let current_dept = filters.get().department;
                                    departments.get().into_iter().map(|d| {
                                        let is_sel = d == current_dept;
                                        let d_val = d.clone();
                                        view! {
                                            <option value=d_val selected=is_sel>{d}</option>
                                        }
                                    }).collect_view()
                                }}
                            </select>
                        </div>
                    </div>
                </div>

                // =================================================================
                // 3. GROUP: Semester Turnus & Lehrformen
                // =================================================================
                <div class=move || if filters.get().acc_turnus_open { "flat-accordion is-open" } else { "flat-accordion" }>
                    <button
                        type="button"
                        class="flat-accordion-header"
                        on:click=move |_| filters.update(|f| f.acc_turnus_open = !f.acc_turnus_open)
                    >
                        <div class="header-title-row">
                            <span class="header-title">"Semester Turnus & Lehrformen"</span>
                            {move || {
                                if !filters.get().turnus_all {
                                    view! { <span class="header-count-dezent">"(Aktiv)"</span> }.into_any()
                                } else {
                                    view! {}.into_any()
                                }
                            }}
                        </div>
                        <span class="header-arrow">
                            {move || if filters.get().acc_turnus_open { "▴" } else { "▾" }}
                        </span>
                    </button>

                    <div class="flat-accordion-content">
                        <div>
                            <span class="filter-label">"Semester Turnus:"</span>
                            <div class="turnus-quick-row">
                                <button
                                    type="button"
                                    class=move || if filters.get().turnus_all { "btn-turnus-quick active" } else { "btn-turnus-quick" }
                                    on:click=move |_| {
                                        filters.update(|f| {
                                            f.turnus_all = true;
                                            f.turnus_next = false;
                                            f.turnus_wise_even = false;
                                            f.turnus_wise_odd = false;
                                            f.turnus_sose_even = false;
                                            f.turnus_sose_odd = false;
                                            f.turnus_sporadic = false;
                                        });
                                    }
                                >
                                    "Alle"
                                </button>
                                <button
                                    type="button"
                                    class=move || if filters.get().turnus_next { "btn-turnus-quick active" } else { "btn-turnus-quick" }
                                    on:click=move |_| {
                                        filters.update(|f| {
                                            f.turnus_all = false;
                                            f.turnus_next = true;
                                            f.turnus_wise_even = false;
                                            f.turnus_wise_odd = false;
                                            f.turnus_sose_even = false;
                                            f.turnus_sose_odd = false;
                                            f.turnus_sporadic = false;
                                        });
                                    }
                                >
                                    "WiSe 26"
                                </button>
                            </div>

                            // Accordion Andere (1:1 from original)
                            <div class="turnus-accordion" style="margin-top: 0.35rem;">
                                <button
                                    type="button"
                                    class="turnus-accordion-toggle"
                                    on:click=move |_| filters.update(|f| f.turnus_accordion_open = !f.turnus_accordion_open)
                                >
                                    <span class="turnus-accordion-title">"Andere"</span>
                                    <span class="turnus-accordion-arrow">
                                        {move || if filters.get().turnus_accordion_open { "▴" } else { "▾" }}
                                    </span>
                                </button>

                                {move || if filters.get().turnus_accordion_open {
                                    view! {
                                        <div class="turnus-accordion-content" style="display: block;">
                                            <TurnusMatrix filters=filters />
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}
                            </div>
                        </div>

                        // Lehrformen Chips
                        <div>
                            <span class="filter-label">"Enthaltene Lehrformen:"</span>
                            <div class="campus-chips">
                                <label
                                    class=move || if filters.get().teaching_vorlesung { "campus-chip active" } else { "campus-chip" }
                                    on:click=move |_| filters.update(|f| f.teaching_vorlesung = !f.teaching_vorlesung)
                                >
                                    <span class="chip-content"><span class="chip-icon">"📖"</span> " Vorlesung"</span>
                                </label>
                                <label
                                    class=move || if filters.get().teaching_uebung { "campus-chip active" } else { "campus-chip" }
                                    on:click=move |_| filters.update(|f| f.teaching_uebung = !f.teaching_uebung)
                                >
                                    <span class="chip-content"><span class="chip-icon">"✏️"</span> " Übung"</span>
                                </label>
                                <label
                                    class=move || if filters.get().teaching_praktikum { "campus-chip active" } else { "campus-chip" }
                                    on:click=move |_| filters.update(|f| f.teaching_praktikum = !f.teaching_praktikum)
                                >
                                    <span class="chip-content"><span class="chip-icon">"🧪"</span> " Praktikum"</span>
                                </label>
                            </div>
                        </div>

                        // Moduldauer
                        <div>
                            <span class="filter-label">"Moduldauer:"</span>
                            <SegmentedControl
                                options=duration_options
                                selected=Signal::derive(move || filters.get().duration)
                                on_select=Callback::new(move |val: String| {
                                    filters.update(|f| f.duration = val);
                                })
                            />
                        </div>
                    </div>
                </div>

                // =================================================================
                // 4. GROUP: Filter & Kriterien
                // =================================================================
                <div class=move || if filters.get().acc_filter_open { "flat-accordion is-open" } else { "flat-accordion" }>
                    <button
                        type="button"
                        class="flat-accordion-header"
                        on:click=move |_| filters.update(|f| f.acc_filter_open = !f.acc_filter_open)
                    >
                        <div class="header-title-row">
                            <span class="header-title">"Filter & Kriterien"</span>
                        </div>
                        <span class="header-arrow">
                            {move || if filters.get().acc_filter_open { "▴" } else { "▾" }}
                        </span>
                    </button>

                    <div class="flat-accordion-content">
                        // Limitation Segmented Control
                        <div>
                            <span class="filter-label">"Teilnehmer:innenbeschränkung:"</span>
                            <SegmentedControl
                                options=limitation_options
                                selected=Signal::derive(move || filters.get().limitation)
                                on_select=Callback::new(move |val: String| {
                                    filters.update(|f| f.limitation = val);
                                })
                            />
                        </div>

                        // FÜS Segmented Control
                        <div>
                            <span class="filter-label">"Fachübergreifendes Studium:"</span>
                            <SegmentedControl
                                options=fues_options
                                selected=Signal::derive(move || filters.get().fues)
                                on_select=Callback::new(move |val: String| {
                                    filters.update(|f| f.fues = val);
                                })
                            />
                        </div>

                        // Prüfungsformen Chips
                        <div>
                            <span class="filter-label">"Prüfungsform:"</span>
                            <div class="campus-chips">
                                <label
                                    class=move || if filters.get().exam_klausur { "campus-chip active" } else { "campus-chip" }
                                    on:click=move |_| filters.update(|f| f.exam_klausur = !f.exam_klausur)
                                >
                                    <span class="chip-content">"📝 Klausur"</span>
                                </label>
                                <label
                                    class=move || if filters.get().exam_muendlich { "campus-chip active" } else { "campus-chip" }
                                    on:click=move |_| filters.update(|f| f.exam_muendlich = !f.exam_muendlich)
                                >
                                    <span class="chip-content">"🗣️ Mündlich"</span>
                                </label>
                                <label
                                    class=move || if filters.get().exam_beleg { "campus-chip active" } else { "campus-chip" }
                                    on:click=move |_| filters.update(|f| f.exam_beleg = !f.exam_beleg)
                                >
                                    <span class="chip-content">"📄 Beleg"</span>
                                </label>
                            </div>
                        </div>

                        // Benotung Segmented Control
                        <div>
                            <span class="filter-label">"Benotung:"</span>
                            <SegmentedControl
                                options=grading_options
                                selected=Signal::derive(move || filters.get().grading)
                                on_select=Callback::new(move |val: String| {
                                    filters.update(|f| f.grading = val);
                                })
                            />
                        </div>

                        // Toggles
                        <div class="filter-toggles-list">
                            <label class="filter-toggle-row">
                                <span>"Voraussetzung Erfüllt:"</span>
                                <div class="switch">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || filters.get().only_prereqs_met
                                        on:change=move |ev| {
                                            let chk = event_target_checked(&ev);
                                            filters.update(|f| f.only_prereqs_met = chk);
                                        }
                                    />
                                    <span class="slider"></span>
                                </div>
                            </label>

                            <label class="filter-toggle-row">
                                <span>"Auslaufmodule Ausblenden:"</span>
                                <div class="switch">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || filters.get().hide_phase_out
                                        on:change=move |ev| {
                                            let chk = event_target_checked(&ev);
                                            filters.update(|f| f.hide_phase_out = chk);
                                        }
                                    />
                                    <span class="slider"></span>
                                </div>
                            </label>
                        </div>

                        // Dual ECTS Range Slider
                        <div>
                            <div class="filter-label-row">
                                <span class="filter-label" style="margin-bottom:0;">"ECTS Range:"</span>
                                <span id="credits-val-badge" class="slider-val-badge">
                                    {move || {
                                        let min = filters.get().min_credits;
                                        let max = filters.get().max_credits;
                                        if min == 0.0 && max >= 30.0 {
                                            "0 – 30 ECTS (Alle)".to_string()
                                        } else {
                                            format!("{} – {} ECTS", min as i64, max as i64)
                                        }
                                    }}
                                </span>
                            </div>
                            <DualRangeSlider
                                min_bound=0.0
                                max_bound=30.0
                                step=1.0
                                min_val=Signal::derive(move || filters.get().min_credits)
                                max_val=Signal::derive(move || filters.get().max_credits)
                                on_change=Callback::new(move |(min, max)| {
                                    filters.update(|f| {
                                        f.min_credits = min;
                                        f.max_credits = max;
                                    });
                                })
                            />
                        </div>
                    </div>
                </div>

                // =================================================================
                // 5. GROUP: Standort & Sprache
                // =================================================================
                <div class=move || if filters.get().acc_ort_open { "flat-accordion is-open" } else { "flat-accordion" }>
                    <button
                        type="button"
                        class="flat-accordion-header"
                        on:click=move |_| filters.update(|f| f.acc_ort_open = !f.acc_ort_open)
                    >
                        <div class="header-title-row">
                            <span class="header-title">"Standort & Sprache"</span>
                        </div>
                        <span class="header-arrow">
                            {move || if filters.get().acc_ort_open { "▴" } else { "▾" }}
                        </span>
                    </button>

                    <div class="flat-accordion-content">
                        <div>
                            <span class="filter-label">"Veranstaltungsort:"</span>
                            <div class="campus-chips">
                                <label class=move || if filters.get().campus_hauptcampus { "campus-chip active" } else { "campus-chip" }>
                                    <input
                                        type="checkbox"
                                        prop:checked=move || filters.get().campus_hauptcampus
                                        on:change=move |ev| {
                                            let chk = event_target_checked(&ev);
                                            filters.update(|f| f.campus_hauptcampus = chk);
                                        }
                                    />
                                    <span class="chip-content"><span class="chip-icon">"🏛️"</span> " Hauptcampus"</span>
                                </label>
                                <label class=move || if filters.get().campus_sachsendorf { "campus-chip active" } else { "campus-chip" }>
                                    <input
                                        type="checkbox"
                                        prop:checked=move || filters.get().campus_sachsendorf
                                        on:change=move |ev| {
                                            let chk = event_target_checked(&ev);
                                            filters.update(|f| f.campus_sachsendorf = chk);
                                        }
                                    />
                                    <span class="chip-content"><span class="chip-icon">"🌲"</span> " Sachsendorf"</span>
                                </label>
                                <label class=move || if filters.get().campus_senftenberg { "campus-chip active" } else { "campus-chip" }>
                                    <input
                                        type="checkbox"
                                        prop:checked=move || filters.get().campus_senftenberg
                                        on:change=move |ev| {
                                            let chk = event_target_checked(&ev);
                                            filters.update(|f| f.campus_senftenberg = chk);
                                        }
                                    />
                                    <span class="chip-content"><span class="chip-icon">"🧪"</span> " Senftenberg"</span>
                                </label>
                            </div>
                        </div>

                        <div>
                            <span class="filter-label">"Unterrichtssprachen:"</span>
                            <div class="lang-chips">
                                <label class=move || if filters.get().lang_de { "lang-chip active" } else { "lang-chip" }>
                                    <input
                                        type="checkbox"
                                        prop:checked=move || filters.get().lang_de
                                        on:change=move |ev| {
                                            let chk = event_target_checked(&ev);
                                            filters.update(|f| f.lang_de = chk);
                                        }
                                    />
                                    <span class="chip-content"><span class="chip-icon">"🇩🇪"</span> " Deutsch"</span>
                                </label>
                                <label class=move || if filters.get().lang_en { "lang-chip active" } else { "lang-chip" }>
                                    <input
                                        type="checkbox"
                                        prop:checked=move || filters.get().lang_en
                                        on:change=move |ev| {
                                            let chk = event_target_checked(&ev);
                                            filters.update(|f| f.lang_en = chk);
                                        }
                                    />
                                    <span class="chip-content"><span class="chip-icon">"🇬🇧"</span> " English"</span>
                                </label>
                            </div>
                        </div>
                    </div>
                </div>

                // =================================================================
                // 6. Reset Section
                // =================================================================
                <div class="sidebar-section-reset">
                    <button
                        type="button"
                        class="btn-reset-filters"
                        on:click=move |_| on_reset_filters.run(())
                        title="Alle Filter auf Standardwerte zurücksetzen"
                    >
                        <span>"🗑 Filter zurücksetzen"</span>
                        {move || {
                            let cnt = active_filter_count.get();
                            if cnt > 0 {
                                view! {
                                    <span class="active-badge">{format!("{} aktiv", cnt)}</span>
                                }.into_any()
                            } else {
                                view! {}.into_any()
                            }
                        }}
                    </button>
                </div>
            </form>
        </div>
    }
}
