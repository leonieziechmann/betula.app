use leptos::prelude::*;
use crate::models::{FilterOptions, ProgramOption};
use crate::components::study_program_selector::StudyProgramSelection;
use crate::components::segmented_control::{SegmentedControl, SegmentedOption};
use crate::components::range_slider::DualRangeSlider;
use crate::components::turnus_matrix::TurnusMatrix;

/// Sidebar containing all catalog filters: study program, semester turnus, limits, ECTS, campus, and language
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

    view! {
        <div class="sidebar-header">
            <a href="/" class="sidebar-brand">
                <span class="brand-badge">"BTU"</span>
                <span class="brand-title">"Smart Modulkatalog"</span>
            </a>
            <button
                type="button"
                class="btn-sidebar-close"
                on:click=move |_| on_close_sidebar.run(())
                aria-label="Filter schließen"
            >
                "×"
            </button>
        </div>

        <div class="sidebar-body">
            <form on:submit=move |ev| ev.prevent_default()>

                // Section 1: Studiengang
                <div class="sidebar-section">
                    <div class="sidebar-section-title">"Studiengang:"</div>
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
                <hr class="sidebar-divider" />

                // Section 2: Semester Turnus
                <div class="sidebar-section">
                    <div class="sidebar-section-title">"Semester Turnus:"</div>
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

                    // Accordion Andere
                    <div class="turnus-accordion">
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
                <hr class="sidebar-divider" />

                // Section 3: Filter (Limitation, FÜS, Toggles, ECTS Range)
                <div class="sidebar-section">
                    <div class="sidebar-section-title">"Filter:"</div>

                    // Limitation Segmented Control
                    <div class="filter-item">
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
                    <div class="filter-item">
                        <span class="filter-label">"Fachübergreifendes Studium:"</span>
                        <SegmentedControl
                            options=fues_options
                            selected=Signal::derive(move || filters.get().fues)
                            on_select=Callback::new(move |val: String| {
                                filters.update(|f| f.fues = val);
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
                    <div class="filter-item">
                        <div class="filter-label-row">
                            <span class="filter-label">"ECTS Range:"</span>
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
                <hr class="sidebar-divider" />

                // Section 4: Veranstaltungsort
                <div class="sidebar-section">
                    <div class="sidebar-section-header">
                        <div class="sidebar-section-title">"Veranstaltungsort:"</div>
                    </div>
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
                <hr class="sidebar-divider" />

                // Section 5: Unterrichtssprachen
                <div class="sidebar-section">
                    <div class="sidebar-section-title">"Unterrichtssprachen:"</div>
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
                <hr class="sidebar-divider" />

                // Section 6: Filter Zurücksetzen
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
