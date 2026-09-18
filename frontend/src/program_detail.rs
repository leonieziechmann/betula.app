use std::collections::{BTreeMap, HashSet};
use leptos::prelude::*;
use crate::db::*;
use crate::models::*;
use crate::components::{
    BreadcrumbItem, DetailNavBar, ContentBox,
    study_program_selector::{format_program_title, format_po_labels, format_degree_short},
};

#[derive(Clone, Copy, PartialEq)]
enum ProgramViewTab {
    Plan,
    Electives,
    All,
}

#[component]
pub fn StudyProgramDetailPage<FBack, FOpen, FSelectProg>(
    program_id: String,
    on_back: FBack,
    on_open_module: FOpen,
    on_select_program: FSelectProg,
    completed_modules: Signal<HashSet<String>>,
    bookmarked_modules: Signal<HashSet<String>>,
) -> impl IntoView
where
    FBack: Fn() + Copy + Send + Sync + 'static,
    FOpen: Fn(String) + Copy + Send + Sync + 'static,
    FSelectProg: Fn(String, String, String) + Copy + Send + Sync + 'static,
{
    let prog_id_clone = program_id.clone();
    let detail_opt = StoredValue::new(get_study_program_detail(&prog_id_clone));
    let modules = StoredValue::new(get_study_program_curriculum_modules(&prog_id_clone));

    // Active view tab (Plan vs Electives vs All)
    let (active_tab, set_active_tab) = signal(ProgramViewTab::Plan);

    // Share link toast
    let (share_copied, set_share_copied) = signal(false);

    let (selected_area_filter, set_selected_area_filter) = signal(Option::<String>::None);

    let on_share = {
        let p_id = program_id.clone();
        Callback::new(move |_| {
            if let Some(win) = web_sys::window() {
                let loc = win.location();
                let share_url = format!("{}{}?program={}", loc.origin().unwrap_or_default(), loc.pathname().unwrap_or_default(), p_id);
                let nav = win.navigator();
                let clipboard = nav.clipboard();
                let _ = clipboard.write_text(&share_url);
                set_share_copied.set(true);
                gloo_timers::callback::Timeout::new(2000, move || {
                    set_share_copied.set(false);
                }).forget();
            }
        })
    };

    view! {
        <div class="program-detail-page">
            {move || match detail_opt.get_value() {
                None => view! {
                    <div class="program-section-card" style="text-align: center; padding: 3rem 1rem;">
                        <div style="font-size: 2.5rem; margin-bottom: 0.5rem;">"🔍"</div>
                        <h2 style="font-size: 1.3rem; margin-bottom: 0.5rem;">"Studiengang nicht gefunden"</h2>
                        <p style="color: #64748b; margin-bottom: 1.5rem;">"Der angeforderte Studiengang existiert nicht oder wurde noch nicht geladen."</p>
                        <button type="button" class="btn btn-primary" on:click=move |_| on_back()>
                            "← Zurück zum Katalog"
                        </button>
                    </div>
                }.into_any(),
                Some(detail) => {
                    let full_title = format_program_title(Some(&detail.degree), &detail.program_name);
                    let (po_year, po_sub) = format_po_labels(&detail.po_version);
                    let docs = detail.parse_documents();
                    let all_regs = get_study_program_all_regulations(&detail.program_name, &detail.degree);
                    let counterpart_opt = get_study_program_counterpart(&detail.program_name, &detail.degree);
                    let prog_modules = modules.get_value();

                    // Calculate stats
                    let total_mods = prog_modules.len();
                    let total_credits: f64 = prog_modules.iter().filter_map(|m| m.credits).sum();

                    // 1. Mandatory Modules grouped by recommended semester (1, 2, 3, ...)
                    let mut mandatory_semesters: BTreeMap<i64, Vec<ModuleCardItem>> = BTreeMap::new();

                    // 2. Elective / Wahlpflicht Modules grouped by Subject Area / Category
                    let mut subject_areas: BTreeMap<String, Vec<ModuleCardItem>> = BTreeMap::new();

                    // 3. FÜS / Abschlussarbeit
                    let mut final_thesis_modules: Vec<ModuleCardItem> = Vec::new();

                    for m in &prog_modules {
                        let m_type_low = m.module_type.as_deref().unwrap_or("").to_lowercase();
                        let title_low = m.title_de.to_lowercase();
                        let sem = m.recommended_semester.unwrap_or(0);

                        let is_thesis = title_low.contains("bachelorarbeit") || title_low.contains("masterarbeit")
                            || title_low.contains("abschlussarbeit") || title_low.contains("kolloquium")
                            || m_type_low.contains("abschluss");

                        if is_thesis {
                            final_thesis_modules.push(m.clone());
                            if sem > 0 {
                                mandatory_semesters.entry(sem).or_default().push(m.clone());
                            }
                        } else if sem > 0 {
                            // Any module or slot scheduled for a specific semester belongs in that semester's plan
                            mandatory_semesters.entry(sem).or_default().push(m.clone());

                            if m_type_low.contains("wahl") {
                                let area = if let Some(ref sa) = m.subject_area {
                                    if !sa.trim().is_empty() { sa.trim().to_string() } else { "Wahlpflichtbereich".to_string() }
                                } else if let Some(ref sp) = m.specialization {
                                    if !sp.trim().is_empty() { sp.trim().to_string() } else { "Wahlpflichtbereich".to_string() }
                                } else {
                                    format!("{}. Fachsemester (Wahlpflicht)", sem)
                                };
                                subject_areas.entry(area).or_default().push(m.clone());
                            }
                        } else {
                            // Group by SubjectArea or Specialization or Fallback
                            let area = if let Some(ref sa) = m.subject_area {
                                if !sa.trim().is_empty() { sa.trim().to_string() } else { "Wahlpflichtbereich".to_string() }
                            } else if let Some(ref sp) = m.specialization {
                                if !sp.trim().is_empty() { sp.trim().to_string() } else { "Wahlpflichtbereich".to_string() }
                            } else {
                                "Wahlpflicht- & Ergänzungsmodule".to_string()
                            };
                            subject_areas.entry(area).or_default().push(m.clone());
                        }
                    }

                    let cur_id = detail.id.clone();
                    let prog_n = detail.program_name.clone();
                    let deg_badge = format_degree_short(Some(&detail.degree));
                    let p_name_display = detail.program_name.clone();

                    let mut study_sections_set = std::collections::BTreeSet::new();
                    for m in &prog_modules {
                        if let Some(ref sec) = m.study_section {
                            let trimmed = sec.trim();
                            if !trimmed.is_empty() {
                                study_sections_set.insert(trimmed.to_string());
                            }
                        }
                    }
                    let study_sections_display = if !study_sections_set.is_empty() {
                        study_sections_set.into_iter().collect::<Vec<_>>().join(" & ")
                    } else {
                        "Grund- & Fachstudium".to_string()
                    };
                    let subject_areas_count = subject_areas.len();

                    view! {
                        // Top Breadcrumb & Navbar
                        <DetailNavBar
                            back_text="Zurück zum Katalog"
                            shortcut_label="Esc"
                            on_back=Callback::new(move |_| on_back())
                            breadcrumbs=vec![
                                BreadcrumbItem::link("Modulkatalog", Callback::new(move |_| on_back())),
                                BreadcrumbItem::static_text("Studiengänge"),
                                BreadcrumbItem::current(p_name_display),
                            ]
                        >
                            <button
                                type="button"
                                class=move || format!("detail-action-btn btn-share {}", if share_copied.get() { "copied" } else { "" })
                                on:click=move |_| on_share.run(())
                                title="Direktlink zu diesem Studiengang kopieren"
                            >
                                <span class="btn-icon">"🔗"</span>
                                <span class="btn-label">
                                    {move || if share_copied.get() { "Kopiert! ✓" } else { "Teilen" }}
                                </span>
                            </button>
                        </DetailNavBar>

                        // Header Card
                        <div class="program-detail-header">
                            <div class="program-detail-header-top">
                                <div class="program-detail-title-wrap">
                                    <div class="program-detail-badges">
                                        {if !deg_badge.is_empty() {
                                            view! { <span class="badge badge-dept">{deg_badge.clone()}</span> }.into_any()
                                        } else {
                                            view! {}.into_any()
                                        }}
                                        <span class="badge badge-po">{po_year.clone()}</span>
                                        {if let Some(ref code) = detail.program_code {
                                            view! { <span class="badge badge-id">{format!("Code: {}", code)}</span> }.into_any()
                                        } else {
                                            view! {}.into_any()
                                        }}
                                        <span class="badge badge-ects">{format!("{} ECTS gesamt", total_credits as i64)}</span>
                                    </div>
                                    <h1 class="program-detail-title">{full_title.clone()}</h1>
                                    <div class="program-detail-subtitle">
                                        <span>"Offizielle Bezeichnung: " <strong>{detail.program_name.clone()}</strong></span>
                                        <span>"•"</span>
                                        <span>"Prüfungsordnung: " <em>{po_sub.clone()}</em></span>
                                    </div>
                                </div>
                            </div>

                            // Bachelor / Master Counterpart Link Banner
                            {if let Some(cp) = counterpart_opt {
                                let cp_id = cp.id.clone();
                                let cp_name = cp.program_name.clone();
                                let cp_po = cp.po_version.clone().unwrap_or_default();
                                let cp_deg = cp.degree.clone().unwrap_or_default();
                                let cp_deg_short = format_degree_short(cp.degree.as_deref());
                                let is_to_master = cp_deg.to_lowercase().contains("master") || cp_deg.to_lowercase().contains("m.sc");
                                let label = if is_to_master {
                                    format!("🎓 Konsekutiver Master-Studiengang: {} ({})", cp_name, cp_deg_short)
                                } else {
                                    format!("🎓 Zugehöriger Bachelor-Studiengang: {} ({})", cp_name, cp_deg_short)
                                };

                                view! {
                                    <div class="counterpart-banner">
                                        <div class="counterpart-info">
                                            <span>{label}</span>
                                        </div>
                                        <button
                                            type="button"
                                            class="btn-counterpart-link"
                                            on:click=move |_| {
                                                on_select_program(cp_id.clone(), cp_name.clone(), cp_po.clone());
                                            }
                                        >
                                            "Studiengang anzeigen →"
                                        </button>
                                    </div>
                                }.into_any()
                            } else {
                                view! {}.into_any()
                            }}
                        </div>

                        // Main Content Grid (Two Columns: Left = Curriculum, Right = Meta & Statutes)
                        <div class="program-detail-grid">
                            // Left Main Column: Structured Curriculum Plan & Electives
                            <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                                <ContentBox
                                    title=format!("Studienplan & Module ({})", total_mods)
                                    emoji="📚"
                                >
                                    // View Tabs
                                    <div class="curriculum-view-tabs">
                                        <button
                                            type="button"
                                            class=move || format!("btn-curriculum-tab {}", if active_tab.get() == ProgramViewTab::Plan { "active" } else { "" })
                                            on:click=move |_| set_active_tab.set(ProgramViewTab::Plan)
                                        >
                                            "📅 Studienablaufplan (Pflicht)"
                                        </button>
                                        <button
                                            type="button"
                                            class=move || format!("btn-curriculum-tab {}", if active_tab.get() == ProgramViewTab::Electives { "active" } else { "" })
                                            on:click=move |_| set_active_tab.set(ProgramViewTab::Electives)
                                        >
                                            "📂 Wahlpflicht & Fachbereiche"
                                        </button>
                                        <button
                                            type="button"
                                            class=move || format!("btn-curriculum-tab {}", if active_tab.get() == ProgramViewTab::All { "active" } else { "" })
                                            on:click=move |_| set_active_tab.set(ProgramViewTab::All)
                                        >
                                            "Alle Module"
                                        </button>
                                    </div>

                                    {if prog_modules.is_empty() {
                                        view! {
                                            <p style="color: #64748b; font-style: italic; padding: 1rem 0;">
                                                "Keine Modulzuordnungen für diesen Studiengang in der Datenbank gefunden."
                                            </p>
                                        }.into_any()
                                    } else {
                                        let mand_groups = mandatory_semesters.clone();
                                        let sub_groups = subject_areas.clone();
                                        let thesis_mods = final_thesis_modules.clone();

                                        view! {
                                            <div style="display: flex; flex-direction: column; gap: 1.75rem;">
                                                // 1. SECTION: Studienablaufplan (Pflichtmodule nach Semester 1..6)
                                                {move || {
                                                    let tab = active_tab.get();
                                                    if (tab == ProgramViewTab::Plan || tab == ProgramViewTab::All) && !mand_groups.is_empty() {
                                                        view! {
                                                            <div class="curriculum-semesters-container">
                                                                <h3 style="font-size: 1.05rem; font-weight: 700; color: #0f172a; margin-bottom: -0.5rem; display: flex; align-items: center; gap: 0.5rem;">
                                                                    <span>"📅"</span> "Regelstudienplan (Studienablaufplan nach Fachsemester)"
                                                                </h3>
                                                                {mand_groups.clone().into_iter().map(|(sem, s_mods)| {
                                                                    let sem_credits: f64 = s_mods.iter().filter_map(|m| m.credits).sum();
                                                                    let explicit_section = s_mods.iter().find_map(|m| m.study_section.as_ref().filter(|s| !s.trim().is_empty()));
                                                                    let study_section_label = explicit_section.cloned().unwrap_or_else(|| {
                                                                        if sem <= 2 {
                                                                            "Grundstudium".to_string()
                                                                        } else if sem <= 4 {
                                                                            "Fachstudium".to_string()
                                                                        } else {
                                                                            "Vertiefungsstudium".to_string()
                                                                        }
                                                                    });

                                                                    view! {
                                                                        <div class="curriculum-semester-block">
                                                                            <div class="curriculum-semester-header">
                                                                                <div class="curriculum-semester-header-left">
                                                                                    <span class="curriculum-semester-badge-num">{format!("{}. Fachsemester", sem)}</span>
                                                                                    <span class="badge-study-section">{study_section_label}</span>
                                                                                    <span style="font-size: 0.85rem; color: #64748b;">"(" {s_mods.len()} " Module / Fächer)"</span>
                                                                                </div>
                                                                                <span class="curriculum-semester-ects-badge">{format!("{} ECTS", sem_credits as i64)}</span>
                                                                            </div>
                                                                            <div class="curriculum-module-list">
                                                                                {s_mods.into_iter().map(|m| {
                                                                                    let m_id = m.id.clone();
                                                                                    let m_id_click = m.id.clone();
                                                                                    let is_comp = completed_modules.get().contains(&m_id);
                                                                                    let is_bkmk = bookmarked_modules.get().contains(&m_id);
                                                                                    let cr = if let (Some(min), Some(max)) = (m.min_credits, m.max_credits) {
                                                                                        if min > 0.0 && max > min {
                                                                                            format!("{}–{} ECTS", min as i64, max as i64)
                                                                                        } else if let Some(c) = m.credits {
                                                                                            format!("{} ECTS", c as i64)
                                                                                        } else {
                                                                                            String::new()
                                                                                        }
                                                                                    } else if let Some(c) = m.credits {
                                                                                        format!("{} ECTS", c as i64)
                                                                                    } else {
                                                                                        String::new()
                                                                                    };
                                                                                    let turn = m.turnus.clone().unwrap_or_default();
                                                                                    let code = m.code.clone().unwrap_or_default();
                                                                                    let area = m.subject_area.clone().unwrap_or_default();
                                                                                    let rules = m.area_rules.clone().unwrap_or_default();

                                                                                    view! {
                                                                                        <div
                                                                                            class=move || format!("curriculum-module-item {} {}", if is_comp { "is-completed" } else { "" }, if is_bkmk { "is-bookmarked" } else { "" })
                                                                                            on:click=move |_| on_open_module(m_id_click.clone())
                                                                                        >
                                                                                            <div class="curriculum-module-left">
                                                                                                <span class=move || format!("curriculum-module-status-btn {}", if is_comp { "completed" } else { "" })>
                                                                                                    {if is_comp { "✓" } else if is_bkmk { "★" } else { "○" }}
                                                                                                </span>
                                                                                                <div class="curriculum-module-info">
                                                                                                    <div class="curriculum-module-title-row">
                                                                                                        <span class="curriculum-module-title">{m.title_de}</span>
                                                                                                    </div>
                                                                                                    <div class="curriculum-module-meta">
                                                                                                        {if !code.is_empty() {
                                                                                                            view! { <span class="badge badge-id">{code}</span> }.into_any()
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                        {if let (Some(start), Some(end)) = (m.start_semester, m.end_semester) {
                                                                                                            if start > 0 && end > start {
                                                                                                                view! { <span class="badge" style="background: #fef3c7; color: #b45309; font-weight: 600;">{format!("{}-semestrig (Sem. {}–{})", end - start + 1, start, end)}</span> }.into_any()
                                                                                                            } else {
                                                                                                                view! {}.into_any()
                                                                                                            }
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                        {if let Some(ref span) = m.semester_span {
                                                                                                            if !span.trim().is_empty() && m.start_semester.unwrap_or(0) == 0 {
                                                                                                                view! { <span class="badge" style="background: #e0f2fe; color: #0369a1; font-weight: 600;">{format!("Sem. {}", span)}</span> }.into_any()
                                                                                                            } else {
                                                                                                                view! {}.into_any()
                                                                                                            }
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                        {if !area.is_empty() {
                                                                                                            view! { <span class="badge-subject-area">{area}</span> }.into_any()
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                        {if !rules.is_empty() {
                                                                                                            view! { <span class="badge" style="background: #f1f5f9; color: #475569; font-size: 0.75rem;">{rules}</span> }.into_any()
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                        {if !turn.is_empty() {
                                                                                                            view! { <span>"• " {turn}</span> }.into_any()
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                    </div>
                                                                                                </div>
                                                                                            </div>
                                                                                            <div class="curriculum-module-right">
                                                                                                {if !cr.is_empty() {
                                                                                                    view! { <span class="badge badge-ects">{cr}</span> }.into_any()
                                                                                                } else {
                                                                                                    view! {}.into_any()
                                                                                                }}
                                                                                                <span style="color: var(--primary); font-size: 0.82rem; font-weight: 600;">"Details →"</span>
                                                                                            </div>
                                                                                        </div>
                                                                                    }
                                                                                }).collect::<Vec<_>>()}
                                                                            </div>
                                                                        </div>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </div>
                                                        }.into_any()
                                                    } else {
                                                        view! {}.into_any()
                                                    }
                                                }}

                                                // 2. SECTION: Wahlpflichtbereiche & Fachbereiche
                                                {move || {
                                                    let tab = active_tab.get();
                                                    if (tab == ProgramViewTab::Electives || tab == ProgramViewTab::All) && !sub_groups.is_empty() {
                                                        view! {
                                                            <div style="display: flex; flex-direction: column; gap: 1.25rem;">
                                                                <h3 style="font-size: 1.05rem; font-weight: 700; color: #0f172a; margin-bottom: -0.25rem; display: flex; align-items: center; gap: 0.5rem;">
                                                                    <span>"📂"</span> "Wahlpflichtkataloge, Vertiefungen & Fachbereiche"
                                                                </h3>

                                                                // Interactive Subject Area Filter Chips
                                                                <div class="subject-area-filter-bar">
                                                                    <button
                                                                        type="button"
                                                                        class=move || format!("btn-area-filter-chip {}", if selected_area_filter.get().is_none() { "active" } else { "" })
                                                                        on:click=move |_| set_selected_area_filter.set(None)
                                                                    >
                                                                        <span>"Alle Bereiche"</span>
                                                                        <span class="chip-count">{sub_groups.len()}</span>
                                                                    </button>
                                                                    {sub_groups.iter().map(|(area_name, a_mods)| {
                                                                        let a_name_click = area_name.clone();
                                                                        let a_name_active = area_name.clone();
                                                                        let count = a_mods.len();
                                                                        view! {
                                                                            <button
                                                                                type="button"
                                                                                class=move || format!("btn-area-filter-chip {}", if selected_area_filter.get().as_deref() == Some(a_name_active.as_str()) { "active" } else { "" })
                                                                                on:click=move |_| {
                                                                                    if selected_area_filter.get().as_deref() == Some(a_name_click.as_str()) {
                                                                                        set_selected_area_filter.set(None);
                                                                                    } else {
                                                                                        set_selected_area_filter.set(Some(a_name_click.clone()));
                                                                                    }
                                                                                }
                                                                            >
                                                                                <span>{area_name.clone()}</span>
                                                                                <span class="chip-count">{count}</span>
                                                                            </button>
                                                                        }
                                                                    }).collect::<Vec<_>>()}
                                                                </div>

                                                                {sub_groups.clone().into_iter().filter(|(area_title, _)| {
                                                                    let sel = selected_area_filter.get();
                                                                    sel.is_none() || sel.as_deref() == Some(area_title.as_str())
                                                                }).map(|(area_title, a_mods)| {
                                                                    // Extract any rule hint from the first module having one
                                                                    let area_rule_opt = a_mods.iter().find_map(|m| m.area_rules.as_ref()).cloned();
                                                                    let total_area_credits: f64 = a_mods.iter().filter_map(|m| m.credits).sum();

                                                                    view! {
                                                                        <div class="subject-area-block">
                                                                            <div class="subject-area-header">
                                                                                <div style="display: flex; align-items: center; gap: 0.5rem;">
                                                                                    <span style="font-weight: 700; color: #1e293b;">{area_title}</span>
                                                                                    <span style="font-size: 0.8rem; color: #64748b;">"(" {a_mods.len()} " Module)"</span>
                                                                                </div>
                                                                                <span class="badge badge-ects">{format!("{} ECTS Pool", total_area_credits as i64)}</span>
                                                                            </div>

                                                                            {if let Some(rule) = area_rule_opt {
                                                                                if !rule.trim().is_empty() {
                                                                                    view! {
                                                                                        <div class="area-rules-banner">
                                                                                            <span>"ℹ️"</span>
                                                                                            <div><strong>"Regelung: "</strong> {rule}</div>
                                                                                        </div>
                                                                                    }.into_any()
                                                                                } else {
                                                                                    view! {}.into_any()
                                                                                }
                                                                            } else {
                                                                                view! {}.into_any()
                                                                            }}

                                                                            <div class="curriculum-module-list">
                                                                                {a_mods.into_iter().map(|m| {
                                                                                    let m_id = m.id.clone();
                                                                                    let m_id_click = m.id.clone();
                                                                                    let is_comp = completed_modules.get().contains(&m_id);
                                                                                    let is_bkmk = bookmarked_modules.get().contains(&m_id);
                                                                                    let cr = m.credits.map(|c| format!("{} ECTS", c as i64)).unwrap_or_default();
                                                                                    let turn = m.turnus.clone().unwrap_or_default();
                                                                                    let code = m.code.clone().unwrap_or_default();
                                                                                    let spec = m.specialization.clone().unwrap_or_default();

                                                                                    view! {
                                                                                        <div
                                                                                            class=move || format!("curriculum-module-item {} {}", if is_comp { "is-completed" } else { "" }, if is_bkmk { "is-bookmarked" } else { "" })
                                                                                            on:click=move |_| on_open_module(m_id_click.clone())
                                                                                        >
                                                                                            <div class="curriculum-module-left">
                                                                                                <span class=move || format!("curriculum-module-status-btn {}", if is_comp { "completed" } else { "" })>
                                                                                                    {if is_comp { "✓" } else if is_bkmk { "★" } else { "○" }}
                                                                                                </span>
                                                                                                <div class="curriculum-module-info">
                                                                                                    <div class="curriculum-module-title-row">
                                                                                                        <span class="curriculum-module-title">{m.title_de}</span>
                                                                                                    </div>
                                                                                                    <div class="curriculum-module-meta">
                                                                                                        {if !code.is_empty() {
                                                                                                            view! { <span class="badge badge-id">{code}</span> }.into_any()
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                        {if !spec.is_empty() {
                                                                                                            view! { <span class="badge badge-po">{spec}</span> }.into_any()
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                        {if !turn.is_empty() {
                                                                                                            view! { <span>"• " {turn}</span> }.into_any()
                                                                                                        } else {
                                                                                                            view! {}.into_any()
                                                                                                        }}
                                                                                                    </div>
                                                                                                </div>
                                                                                            </div>
                                                                                            <div class="curriculum-module-right">
                                                                                                {if !cr.is_empty() {
                                                                                                    view! { <span class="badge badge-ects">{cr}</span> }.into_any()
                                                                                                } else {
                                                                                                    view! {}.into_any()
                                                                                                }}
                                                                                                <span style="color: var(--primary); font-size: 0.82rem; font-weight: 600;">"Details →"</span>
                                                                                            </div>
                                                                                        </div>
                                                                                    }
                                                                                }).collect::<Vec<_>>()}
                                                                            </div>
                                                                        </div>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </div>
                                                        }.into_any()
                                                    } else {
                                                        view! {}.into_any()
                                                    }
                                                }}

                                                // 3. SECTION: Abschlussarbeiten
                                                {if !thesis_mods.is_empty() {
                                                    view! {
                                                        <div class="subject-area-block">
                                                            <div class="subject-area-header">
                                                                <div style="display: flex; align-items: center; gap: 0.5rem;">
                                                                    <span style="font-weight: 700; color: #1e293b;">"🎓 Abschlussarbeit & Kolloquium"</span>
                                                                </div>
                                                            </div>
                                                            <div class="curriculum-module-list">
                                                                {thesis_mods.into_iter().map(|m| {
                                                                    let m_id = m.id.clone();
                                                                    let m_id_click = m.id.clone();
                                                                    let is_comp = completed_modules.get().contains(&m_id);
                                                                    let is_bkmk = bookmarked_modules.get().contains(&m_id);
                                                                    let cr = m.credits.map(|c| format!("{} ECTS", c as i64)).unwrap_or_default();
                                                                    let code = m.code.clone().unwrap_or_default();

                                                                    view! {
                                                                        <div
                                                                            class=move || format!("curriculum-module-item {} {}", if is_comp { "is-completed" } else { "" }, if is_bkmk { "is-bookmarked" } else { "" })
                                                                            on:click=move |_| on_open_module(m_id_click.clone())
                                                                        >
                                                                            <div class="curriculum-module-left">
                                                                                <span class=move || format!("curriculum-module-status-btn {}", if is_comp { "completed" } else { "" })>
                                                                                    {if is_comp { "✓" } else if is_bkmk { "★" } else { "○" }}
                                                                                </span>
                                                                                <div class="curriculum-module-info">
                                                                                    <span class="curriculum-module-title">{m.title_de}</span>
                                                                                    <div class="curriculum-module-meta">
                                                                                        {if !code.is_empty() {
                                                                                            view! { <span class="badge badge-id">{code}</span> }.into_any()
                                                                                        } else {
                                                                                            view! {}.into_any()
                                                                                        }}
                                                                                        <span class="badge-module-type-tag pflicht">"Pflicht-Abschluss"</span>
                                                                                    </div>
                                                                                </div>
                                                                            </div>
                                                                            <div class="curriculum-module-right">
                                                                                {if !cr.is_empty() {
                                                                                    view! { <span class="badge badge-ects">{cr}</span> }.into_any()
                                                                                } else {
                                                                                    view! {}.into_any()
                                                                                }}
                                                                                <span style="color: var(--primary); font-size: 0.82rem; font-weight: 600;">"Details →"</span>
                                                                            </div>
                                                                        </div>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </div>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! {}.into_any()
                                                }}
                                            </div>
                                        }.into_any()
                                    }}
                                </ContentBox>
                            </div>

                            // Right Column: Overview, Statutes & PO Switcher
                            <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                                // Quick Stats
                                <ContentBox title="Übersicht" emoji="📊">
                                    <div style="display: flex; flex-direction: column; gap: 0.6rem; font-size: 0.88rem;">
                                        <div style="display: flex; justify-content: space-between; border-bottom: 1px solid #f1f5f9; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Gesamte Module:"</span>
                                            <strong>{total_mods}</strong>
                                        </div>
                                        <div style="display: flex; justify-content: space-between; border-bottom: 1px solid #f1f5f9; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Fachbereiche / Kataloge:"</span>
                                            <strong style="color: var(--primary);">{subject_areas_count}</strong>
                                        </div>
                                        <div style="display: flex; justify-content: space-between; border-bottom: 1px solid #f1f5f9; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Studienabschnitte:"</span>
                                            <span style="font-weight: 600; text-align: right; max-width: 60%;">{study_sections_display}</span>
                                        </div>
                                        <div style="display: flex; justify-content: space-between; border-bottom: 1px solid #f1f5f9; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Summe ECTS:"</span>
                                            <strong>{total_credits as i64} " ECTS"</strong>
                                        </div>
                                        <div style="display: flex; justify-content: space-between; border-bottom: 1px solid #f1f5f9; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Abschlussgrad:"</span>
                                            <strong>{if !deg_badge.is_empty() { deg_badge.clone() } else { "–".to_string() }}</strong>
                                        </div>
                                        <div style="display: flex; justify-content: space-between; padding-bottom: 0.2rem;">
                                            <span style="color: #64748b;">"Prüfungsordnung:"</span>
                                            <strong>{po_year.clone()}</strong>
                                        </div>
                                    </div>
                                </ContentBox>

                                // Official Documents & Statutes
                                <ContentBox title="Satzungen & Amtsblätter" emoji="📄">
                                    {if docs.is_empty() {
                                        view! {
                                            <p style="color: #64748b; font-size: 0.82rem; font-style: italic;">
                                                "Keine gesonderten Satzungsdokumente im Archiv hinterlegt."
                                            </p>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="statute-documents-list">
                                                {docs.into_iter().map(|doc| {
                                                    let doc_title = doc.title.unwrap_or_else(|| "Prüfungsordnung".to_string());
                                                    let doc_type = doc.doc_type.unwrap_or_else(|| "Satzung".to_string());
                                                    let abl = doc.abl_number.clone().unwrap_or_default();
                                                    let url = doc.url.clone().unwrap_or_default();

                                                    view! {
                                                        <div class="statute-doc-item">
                                                            <div class="statute-doc-info">
                                                                <span class="statute-doc-title">{doc_title}</span>
                                                                <div class="statute-doc-meta">
                                                                    <span class="badge badge-id">{doc_type}</span>
                                                                    {if !abl.is_empty() {
                                                                        view! { <span>"ABl. " {abl}</span> }.into_any()
                                                                    } else {
                                                                        view! {}.into_any()
                                                                    }}
                                                                </div>
                                                            </div>
                                                            {if !url.is_empty() {
                                                                view! {
                                                                    <a
                                                                        href=url
                                                                        target="_blank"
                                                                        rel="noopener noreferrer"
                                                                        class="btn-statute-download"
                                                                    >
                                                                        "PDF ↗"
                                                                    </a>
                                                                }.into_any()
                                                            } else {
                                                                view! {}.into_any()
                                                            }}
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        }.into_any()
                                    }}
                                </ContentBox>

                                // Available Regulations / PO Switcher
                                {if all_regs.len() > 1 {
                                    view! {
                                        <ContentBox title="Verfügbare Satzungen" emoji="🔄">
                                            <div class="po-switcher-list">
                                                {all_regs.into_iter().map(|reg| {
                                                    let reg_id = reg.id.clone();
                                                    let reg_po = reg.po_version.clone();
                                                    let is_active = reg.id == cur_id;
                                                    let (y_label, sub) = format_po_labels(&reg.po_version);
                                                    let p_name = prog_n.clone();

                                                    view! {
                                                        <div
                                                            class="po-switcher-item"
                                                            class:active=is_active
                                                            on:click=move |_| {
                                                                on_select_program(reg_id.clone(), p_name.clone(), reg_po.clone());
                                                            }
                                                        >
                                                            <div style="display: flex; flex-direction: column;">
                                                                <span style="font-weight: 700; font-size: 0.84rem;">{y_label}</span>
                                                                <span style="font-size: 0.72rem; color: #64748b; font-style: italic;">{sub}</span>
                                                            </div>
                                                            {if is_active {
                                                                view! { <span style="color: var(--primary); font-weight: 800;">"Aktiv"</span> }.into_any()
                                                            } else {
                                                                view! { <span style="color: #94a3b8; font-size: 0.75rem;">"Wechseln →"</span> }.into_any()
                                                            }}
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </ContentBox>
                                    }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}

                                // Official BTU Portals & Links
                                <ContentBox title="Offizielle BTU Portale" emoji="🌐">
                                    <div style="display: flex; flex-direction: column; gap: 0.6rem;">
                                        {if let Some(ref q_url) = detail.qis_url {
                                            if !q_url.is_empty() {
                                                view! {
                                                    <a
                                                        href=q_url.clone()
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        class="btn btn-secondary"
                                                        style="justify-content: center; font-size: 0.82rem;"
                                                    >
                                                        "BTU QIS Modulbaum öffnen ↗"
                                                    </a>
                                                }.into_any()
                                            } else {
                                                view! {}.into_any()
                                            }
                                        } else {
                                            view! {}.into_any()
                                        }}
                                    </div>
                                </ContentBox>
                            </div>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
