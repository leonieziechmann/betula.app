use leptos::prelude::*;
use crate::models::ProgramOption;
use crate::components::combobox::{Combobox, ComboboxItem};

/// Helper to extract clean short academic degree (e.g. "B.Sc.", "M.Sc.", "B.A.", "M.A.", "B.Eng.", "M.Eng.")
pub fn format_degree_short(degree: Option<&str>) -> String {
    match degree {
        Some(d) => {
            let d_trim = d.trim();
            let d_lower = d_trim.to_lowercase();
            if d_lower.contains("keine abschlussprüfung")
                || d_lower.contains("ohne abschluss")
                || d_lower.contains("kein abschluss")
                || d_lower.contains("abschluss im ausland")
            {
                String::new()
            } else if d_trim.contains("Bachelor of Science") || d_trim.eq_ignore_ascii_case("B.Sc.") || d_trim.eq_ignore_ascii_case("B.Sc") {
                "B.Sc.".to_string()
            } else if d_trim.contains("Master of Science") || d_trim.eq_ignore_ascii_case("M.Sc.") || d_trim.eq_ignore_ascii_case("M.Sc") {
                "M.Sc.".to_string()
            } else if d_trim.contains("Bachelor of Arts") || d_trim.eq_ignore_ascii_case("B.A.") || d_trim.eq_ignore_ascii_case("B.A") {
                "B.A.".to_string()
            } else if d_trim.contains("Master of Arts") || d_trim.eq_ignore_ascii_case("M.A.") || d_trim.eq_ignore_ascii_case("M.A") {
                "M.A.".to_string()
            } else if d_trim.contains("Bachelor of Engineering") || d_trim.eq_ignore_ascii_case("B.Eng.") || d_trim.eq_ignore_ascii_case("B.Eng") {
                "B.Eng.".to_string()
            } else if d_trim.contains("Master of Engineering") || d_trim.eq_ignore_ascii_case("M.Eng.") || d_trim.eq_ignore_ascii_case("M.Eng") {
                "M.Eng.".to_string()
            } else if d_trim.contains("Bachelor of Education") || d_trim.eq_ignore_ascii_case("B.Ed.") {
                "B.Ed.".to_string()
            } else if d_trim.contains("Master of Education") || d_trim.eq_ignore_ascii_case("M.Ed.") {
                "M.Ed.".to_string()
            } else if d_lower.contains("bachelor") {
                "B.Sc.".to_string()
            } else if d_lower.contains("master") {
                "M.Sc.".to_string()
            } else if d_lower.contains("diplom") {
                "Dipl.".to_string()
            } else if d_lower.contains("staatsexamen") {
                "StEx".to_string()
            } else if d_lower.contains("zertifikat") {
                "Zertifikat".to_string()
            } else if !d_trim.is_empty() && d_trim.chars().count() <= 12 {
                d_trim.to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

/// Extracts base regulation year string (e.g. "PO 2008" from "PO 2008 - 2. SÄ 2024" or "2008")
pub fn format_po_short(raw_po: &str) -> String {
    let trimmed = raw_po.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Extract first 4-digit contiguous numeric sequence
    let mut current_year = String::new();
    for c in trimmed.chars() {
        if c.is_ascii_digit() {
            current_year.push(c);
            if current_year.len() == 4 {
                return format!("PO {}", current_year);
            }
        } else {
            current_year.clear();
        }
    }
    if !trimmed.to_lowercase().starts_with("po") {
        format!("PO {}", trimmed)
    } else {
        trimmed.to_string()
    }
}

/// Helper to format clean academic title + study program name (e.g., "B.Sc. Informatik")
pub fn format_program_title(degree: Option<&str>, name: &str) -> String {
    let name_trimmed = name.trim();
    let deg_prefix = format_degree_short(degree);
    if deg_prefix.is_empty() {
        name_trimmed.to_string()
    } else {
        format!("{} {}", deg_prefix, name_trimmed)
    }
}

/// Extracts a user-friendly start period (e.g. "Seit 2024") and official subtitle (e.g. "PO 2008 - 2. SÄ 2024")
pub fn format_po_labels(raw_po: &str) -> (String, String) {
    let trimmed = raw_po.trim();
    if trimmed.is_empty() {
        return ("Satzung".to_string(), "Prüfungsordnung".to_string());
    }

    // Extract all contiguous 4-digit numeric chunks (e.g. "2008", "2024")
    let mut years: Vec<String> = Vec::new();
    let mut current_year = String::new();

    for c in trimmed.chars() {
        if c.is_ascii_digit() {
            current_year.push(c);
        } else {
            if current_year.len() == 4 {
                years.push(current_year.clone());
            }
            current_year.clear();
        }
    }
    if current_year.len() == 4 {
        years.push(current_year);
    }

    let primary_year = years.first();

    let primary = match primary_year {
        Some(y) => format!("Seit {}", y),
        None => format!("PO {}", trimmed),
    };

    let subtitle = format!("PO {}", trimmed);

    (primary, subtitle)
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupedProgram {
    pub display_title: String,
    pub raw_name: String,
    pub degree: Option<String>,
    pub po_options: Vec<ProgramOption>,
}

fn scroll_po_index_into_view(options_ref: NodeRef<leptos::html::Div>, index: usize) {
    if let Some(container) = options_ref.get_untracked() {
        if let Ok(Some(target)) = container.query_selector(&format!("[data-po-index=\"{}\"]", index)) {
            let obj = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&obj, &"block".into(), &"nearest".into());
            if let Ok(scroll_fn) = js_sys::Reflect::get(&target, &"scrollIntoView".into()) {
                if scroll_fn.is_function() {
                    let _ = js_sys::Reflect::apply(
                        &scroll_fn.into(),
                        &target,
                        &js_sys::Array::of1(&obj),
                    );
                }
            }
        }
    }
}

/// Integrated Study Program & Examination Regulations (PO) Selector
#[component]
pub fn StudyProgramSelection(
    all_programs: Signal<Vec<ProgramOption>>,
    total_modules_count: Signal<i64>,
    selected_program_id: Signal<String>,
    selected_program_name: Signal<String>,
    selected_po_version: Signal<String>,
    program_combobox_open: RwSignal<bool>,
    on_select_program: Callback<(String, String, String)>,
    on_clear_program: Callback<()>,
    on_open_program_page: Callback<String>,
) -> impl IntoView {
    let po_dropdown_open = RwSignal::new(false);
    let po_highlighted_index = RwSignal::new(0usize);
    let po_dropdown_ref = NodeRef::<leptos::html::Div>::new();
    let po_options_ref = NodeRef::<leptos::html::Div>::new();

    // Group study programs by clean title with PO deduplication
    let grouped_programs = Memo::new(move |_| {
        let list = all_programs.get();
        let mut map: Vec<GroupedProgram> = Vec::new();

        for p in list {
            let title = format_program_title(p.degree.as_deref(), &p.program_name);
            let po_str = p.po_version.as_deref().unwrap_or("").trim().to_string();

            if let Some(group) = map.iter_mut().find(|g| g.display_title == title) {
                let is_dup = group.po_options.iter().any(|existing| {
                    existing.po_version.as_deref().unwrap_or("").trim() == po_str
                });
                if !is_dup {
                    group.po_options.push(p);
                }
            } else {
                map.push(GroupedProgram {
                    display_title: title,
                    raw_name: p.program_name.clone(),
                    degree: p.degree.clone(),
                    po_options: vec![p],
                });
            }
        }

        // Sort PO versions within each group (newest / highest version first)
        for group in &mut map {
            group.po_options.sort_by(|a, b| {
                let po_a = a.po_version.as_deref().unwrap_or("");
                let po_b = b.po_version.as_deref().unwrap_or("");
                po_b.cmp(po_a)
            });
        }

        // Sort groups alphabetically by display title
        map.sort_by(|a, b| a.display_title.cmp(&b.display_title));
        map
    });

    // Combobox items: one item per distinct study program title
    let combobox_items = Memo::new(move |_| {
        grouped_programs.get().into_iter().map(|g| {
            let default_id = g.po_options.first().map(|p| p.id.clone()).unwrap_or_default();
            ComboboxItem::new(default_id, g.display_title, None)
        }).collect::<Vec<_>>()
    });

    let has_program_selected = Signal::derive(move || !selected_program_id.get().is_empty());
    let default_badge = Signal::derive(move || format!("{}", total_modules_count.get()));

    // Find available PO options for the currently selected study program
    let current_po_options = Memo::new(move |_| {
        let cur_name = selected_program_name.get();
        if cur_name.is_empty() || cur_name == "Alle Studiengänge" {
            return Vec::<ProgramOption>::new();
        }

        let groups = grouped_programs.get();
        groups.into_iter()
            .find(|g| g.display_title == cur_name || g.raw_name == cur_name)
            .map(|g| g.po_options)
            .unwrap_or_default()
    });

    // Reset PO highlight index when opening PO dropdown
    Effect::new(move |_| {
        if po_dropdown_open.get() {
            untrack(move || {
                let cur_po = selected_po_version.get_untracked();
                let cur_id = selected_program_id.get_untracked();
                let initial_idx = current_po_options.with_untracked(|list| {
                    list.iter().position(|p| {
                        p.id == cur_id || p.po_version.as_deref().unwrap_or("") == cur_po
                    }).unwrap_or(0)
                });

                po_highlighted_index.set(initial_idx);

                request_animation_frame(move || {
                    if let Some(dropdown) = po_dropdown_ref.get_untracked() {
                        let _ = dropdown.focus();
                    }
                    scroll_po_index_into_view(po_options_ref, initial_idx);
                });
            });
        }
    });

    // Close PO dropdown if program combobox opens
    Effect::new(move |_| {
        if program_combobox_open.get() {
            po_dropdown_open.set(false);
        }
    });

    view! {
        <div class="study-program-selection">
            <div class="study-program-dual-row">
                <Combobox
                    selected_label=selected_program_name
                    has_selection=has_program_selected
                items=combobox_items.into()
                placeholder="Alle Studiengänge"
                search_placeholder="Studiengang suchen..."
                default_item_label="Alle Studiengänge (Gesamtkatalog)"
                default_item_badge=default_badge
                is_open=program_combobox_open
                container_class="program-combobox"
                anchor_to_parent=true
                on_select=Callback::new(move |item: ComboboxItem| {
                    let title = item.label.clone();
                    let groups = grouped_programs.get_untracked();
                    if let Some(group) = groups.iter().find(|g| g.display_title == title) {
                        if let Some(latest) = group.po_options.first() {
                            let po = latest.po_version.clone().unwrap_or_default();
                            on_select_program.run((latest.id.clone(), title, po));
                        }
                    } else {
                        on_select_program.run((item.id, title, String::new()));
                    }
                })
                on_clear=Callback::new(move |_| {
                    po_dropdown_open.set(false);
                    on_clear_program.run(());
                })
            />

            // Right: PO Indicator / Interactive Dropdown
            <div class="po-container" id="po-container">
                {move || {
                    let po = selected_po_version.get();
                    let po_list = current_po_options.get();
                    let has_prog = has_program_selected.get();
                    let has_multi = has_prog && po_list.len() > 1;
                    let is_open = po_dropdown_open.get();

                    let (display_text, po_sub) = if !has_prog || po.is_empty() {
                        ("Satzung".to_string(), "Prüfungsordnung".to_string())
                    } else {
                        format_po_labels(&po)
                    };

                    let tooltip_text = if !has_prog || po.is_empty() {
                        "Satzung / Prüfungsordnung".to_string()
                    } else if has_multi {
                        format!("{} ({}) – Klicken zum Wechseln", display_text, po_sub)
                    } else {
                        format!("{} ({})", display_text, po_sub)
                    };

                    view! {
                        <div
                            class="po-badge-display"
                            class:disabled=move || !has_prog
                            class:interactive=move || has_multi
                            class:active=move || is_open
                            tabindex=if has_multi { "0" } else { "-1" }
                            title=tooltip_text
                            on:click=move |_| {
                                if has_multi {
                                    program_combobox_open.set(false);
                                    po_dropdown_open.update(|o| *o = !*o);
                                }
                            }
                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                if !has_multi { return; }
                                match ev.key().as_str() {
                                    "ArrowDown" | "Enter" | " " if !po_dropdown_open.get_untracked() => {
                                        ev.prevent_default();
                                        program_combobox_open.set(false);
                                        po_dropdown_open.set(true);
                                    }
                                    _ => {}
                                }
                            }
                        >
                            <span id="po-display-text">{display_text}</span>
                            {if has_multi {
                                view! { <span class="po-arrow">"▼"</span> }.into_any()
                            } else {
                                view! {}.into_any()
                            }}
                        </div>

                        // PO Dropdown
                        {if is_open && has_multi {
                            let prog_title = selected_program_name.get_untracked();
                            view! {
                                <div
                                    node_ref=po_dropdown_ref
                                    class="po-dropdown"
                                    tabindex="0"
                                    on:click=move |ev| ev.stop_propagation()
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        match ev.key().as_str() {
                                            "ArrowDown" => {
                                                ev.prevent_default();
                                                let len = current_po_options.get_untracked().len();
                                                if len > 0 {
                                                    let next = (po_highlighted_index.get_untracked() + 1) % len;
                                                    po_highlighted_index.set(next);
                                                    scroll_po_index_into_view(po_options_ref, next);
                                                }
                                            }
                                            "ArrowUp" => {
                                                ev.prevent_default();
                                                let len = current_po_options.get_untracked().len();
                                                if len > 0 {
                                                    let current = po_highlighted_index.get_untracked();
                                                    let next = if current == 0 { len - 1 } else { current - 1 };
                                                    po_highlighted_index.set(next);
                                                    scroll_po_index_into_view(po_options_ref, next);
                                                }
                                            }
                                            "Enter" => {
                                                ev.prevent_default();
                                                let list = current_po_options.get_untracked();
                                                let idx = po_highlighted_index.get_untracked();
                                                if let Some(target_po) = list.get(idx).or_else(|| list.first()) {
                                                    let po_str = target_po.po_version.clone().unwrap_or_default();
                                                    on_select_program.run((target_po.id.clone(), prog_title.clone(), po_str));
                                                    po_dropdown_open.set(false);
                                                }
                                            }
                                            "Escape" => {
                                                ev.prevent_default();
                                                po_dropdown_open.set(false);
                                            }
                                            _ => {}
                                        }
                                    }
                                >
                                    <div class="po-dropdown-header">"Studienbeginn / Satzung:"</div>
                                    <div node_ref=po_options_ref class="po-options">
                                        {move || {
                                            let list = current_po_options.get();
                                            let curr_highlight = po_highlighted_index.get();
                                            let active_po = selected_po_version.get();
                                            let active_id = selected_program_id.get();
                                            let prog_name = selected_program_name.get();

                                            list.into_iter().enumerate().map(|(idx, opt)| {
                                                let is_highlighted = curr_highlight == idx;
                                                let opt_po = opt.po_version.clone().unwrap_or_default();
                                                let opt_id = opt.id.clone();
                                                let is_selected = opt.id == active_id || opt_po == active_po;
                                                let (year_label, official_sub) = format_po_labels(&opt_po);
                                                let is_latest = idx == 0;
                                                let prog_n = prog_name.clone();

                                                view! {
                                                    <div
                                                        class="po-option"
                                                        class:selected=is_selected
                                                        class:highlighted=is_highlighted
                                                        data-po-index=idx.to_string()
                                                        on:mouseenter=move |_| po_highlighted_index.set(idx)
                                                        on:click=move |_| {
                                                            on_select_program.run((opt_id.clone(), prog_n.clone(), opt_po.clone()));
                                                            po_dropdown_open.set(false);
                                                        }
                                                    >
                                                        <div class="po-option-primary-row">
                                                            <span class="po-option-year">{year_label}</span>
                                                            {if is_latest {
                                                                view! { <span class="badge badge-po-latest">"Aktuell"</span> }.into_any()
                                                            } else {
                                                                view! {}.into_any()
                                                            }}
                                                        </div>
                                                        <div class="po-option-subtitle">{official_sub}</div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()
                                        }}
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! {}.into_any()
                        }}
                    }
                }}
            </div>
            </div>

            {move || {
                if has_program_selected.get() {
                    let cur_id = selected_program_id.get();
                    if !cur_id.is_empty() {
                        return view! {
                            <button
                                type="button"
                                class="btn-open-program-page"
                                on:click=move |_| {
                                    let id = selected_program_id.get();
                                    if !id.is_empty() {
                                        on_open_program_page.run(id);
                                    }
                                }
                            >
                                "📖 Studiengangsseite & Satzungen öffnen"
                            </button>
                        }.into_any();
                    }
                }
                view! {}.into_any()
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_program_title() {
        assert_eq!(format_program_title(Some("Bachelor of Science"), "Informatik"), "B.Sc. Informatik");
        assert_eq!(format_program_title(Some("Master of Science"), "Informatik"), "M.Sc. Informatik");
        assert_eq!(format_program_title(Some("B.Sc."), "Künstliche Intelligenz"), "B.Sc. Künstliche Intelligenz");
        assert_eq!(format_program_title(Some("Master of Arts"), "Architektur"), "M.A. Architektur");
        assert_eq!(format_program_title(None, "Informatik"), "Informatik");
    }

    #[test]
    fn test_format_po_labels() {
        let (y1, sub1) = format_po_labels("2008 - 2. SÄ 2024");
        assert_eq!(y1, "Seit 2008");
        assert_eq!(sub1, "PO 2008 - 2. SÄ 2024");

        let (y2, sub2) = format_po_labels("2024");
        assert_eq!(y2, "Seit 2024");
        assert_eq!(sub2, "PO 2024");

        let (y3, sub3) = format_po_labels("2016 - 1. SÄ 2024");
        assert_eq!(y3, "Seit 2016");
        assert_eq!(sub3, "PO 2016 - 1. SÄ 2024");

        let (y4, sub4) = format_po_labels("");
        assert_eq!(y4, "Satzung");
        assert_eq!(sub4, "Prüfungsordnung");
    }

    #[test]
    fn test_format_degree_short() {
        assert_eq!(format_degree_short(Some("Bachelor (universitär)")), "B.Sc.");
        assert_eq!(format_degree_short(Some("Bachelor (universitär) - Duales Studium, praxisintegrierend")), "B.Sc.");
        assert_eq!(format_degree_short(Some("Master (universitär)")), "M.Sc.");
        assert_eq!(format_degree_short(Some("Bachelor of Science")), "B.Sc.");
        assert_eq!(format_degree_short(Some("Master of Arts")), "M.A.");
        assert_eq!(format_degree_short(Some("keine Abschlussprüfung möglich")), "");
        assert_eq!(format_degree_short(None), "");
    }

    #[test]
    fn test_format_po_short() {
        assert_eq!(format_po_short("PO 2008 - 2. SÄ 2024"), "PO 2008");
        assert_eq!(format_po_short("2008 - 2. SÄ 2024"), "PO 2008");
        assert_eq!(format_po_short("PO 2022"), "PO 2022");
        assert_eq!(format_po_short("2023"), "PO 2023");
        assert_eq!(format_po_short("PO 2019 - 1. SÄ 2021"), "PO 2019");
        assert_eq!(format_po_short(""), "");
    }
}
