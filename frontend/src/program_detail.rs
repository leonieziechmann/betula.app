use std::collections::{BTreeMap, HashSet};
use leptos::prelude::*;
use crate::db::*;
use crate::models::*;
use crate::components::study_program_selector::{format_program_title, format_po_labels, format_degree_short};

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

    // Share link toast
    let (share_copied, set_share_copied) = signal(false);

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
                    let prog_modules = modules.get_value();

                    // Calculate stats
                    let total_mods = prog_modules.len();
                    let total_credits: f64 = prog_modules.iter().filter_map(|m| m.credits).sum();

                    // Group modules by semester
                    let mut sem_groups: BTreeMap<i64, Vec<ModuleCardItem>> = BTreeMap::new();
                    for m in prog_modules {
                        let sem = m.recommended_semester.unwrap_or(0);
                        sem_groups.entry(sem).or_default().push(m);
                    }

                    let cur_id = detail.id.clone();
                    let prog_n = detail.program_name.clone();

                    let deg_badge = format_degree_short(Some(&detail.degree));

                    view! {
                        // Top navigation bar
                        <div style="display: flex; justify-content: space-between; align-items: center;">
                            <button type="button" class="btn btn-secondary" on:click=move |_| on_back() style="gap: 0.4rem;">
                                "← Zurück zum Katalog"
                            </button>
                            <div class="program-detail-actions">
                                <button type="button" class="btn btn-secondary" on:click=move |_| on_share.run(()) style="gap: 0.4rem;">
                                    "🔗 Teilen"
                                </button>
                                {move || if share_copied.get() {
                                    view! { <span class="badge" style="background: #10b981; color: white;">"Link kopiert!"</span> }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}
                            </div>
                        </div>

                        // Main Header Card
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
                                    </div>
                                    <h1 class="program-detail-title">{full_title.clone()}</h1>
                                    <div class="program-detail-subtitle">
                                        <span>"Offizielle Bezeichnung: " <strong>{detail.program_name.clone()}</strong></span>
                                        <span>"•"</span>
                                        <span>"Prüfungsordnung: " <em>{po_sub.clone()}</em></span>
                                    </div>
                                </div>
                            </div>
                        </div>

                        // Two column layout
                        <div class="program-detail-grid">
                            // Left Column: Curriculum Modules & Semesters
                            <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                                <div class="program-section-card">
                                    <div class="program-section-title">
                                        <span>"📚"</span>
                                        <span>"Studienplan & Module des Studiengangs (" {total_mods} ")"</span>
                                    </div>

                                    {if sem_groups.is_empty() {
                                        view! {
                                            <p style="color: #64748b; font-style: italic;">
                                                "Keine Modulzuordnungen für diesen Studiengang in der Datenbank gefunden."
                                            </p>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="curriculum-semesters-container">
                                                {sem_groups.into_iter().map(|(sem, mods)| {
                                                    let sem_title = if sem > 0 {
                                                        format!("{}. Fachsemester", sem)
                                                    } else {
                                                        "Wahlpflicht- & Ergänzungsmodule".to_string()
                                                    };

                                                    view! {
                                                        <div class="curriculum-semester-block">
                                                            <div class="curriculum-semester-header">
                                                                {sem_title} " (" {mods.len()} " Module)"
                                                            </div>
                                                            <div style="display: flex; flex-direction: column; gap: 0.5rem;">
                                                                {mods.into_iter().map(|m| {
                                                                    let m_id = m.id.clone();
                                                                    let m_id_click = m.id.clone();
                                                                    let is_comp = completed_modules.get().contains(&m_id);
                                                                    let is_bkmk = bookmarked_modules.get().contains(&m_id);
                                                                    let cr = m.credits.map(|c| format!("{} ECTS", c as i64)).unwrap_or_default();
                                                                    let turn = m.turnus.clone().unwrap_or_default();
                                                                    let code = m.code.clone().unwrap_or_default();

                                                                    view! {
                                                                        <div
                                                                            class="statute-doc-item"
                                                                            style="cursor: pointer;"
                                                                            on:click=move |_| on_open_module(m_id_click.clone())
                                                                        >
                                                                            <div class="statute-doc-info">
                                                                                <div style="display: flex; align-items: center; gap: 0.4rem;">
                                                                                    {if is_comp {
                                                                                        view! { <span style="color: #10b981; font-weight: bold;">"✓"</span> }.into_any()
                                                                                    } else if is_bkmk {
                                                                                        view! { <span style="color: #f59e0b;">"★"</span> }.into_any()
                                                                                    } else {
                                                                                        view! {}.into_any()
                                                                                    }}
                                                                                    <span class="statute-doc-title">{m.title_de}</span>
                                                                                </div>
                                                                                <div class="statute-doc-meta">
                                                                                    {if !code.is_empty() {
                                                                                        view! { <span>"Modul-Nr: " {code}</span> }.into_any()
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
                                                                            <div style="display: flex; align-items: center; gap: 0.5rem;">
                                                                                {if !cr.is_empty() {
                                                                                    view! { <span class="badge badge-ects">{cr}</span> }.into_any()
                                                                                } else {
                                                                                    view! {}.into_any()
                                                                                }}
                                                                                <span style="color: var(--primary); font-size: 0.8rem; font-weight: 600;">"Details →"</span>
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
                                    }}
                                </div>
                            </div>

                            // Right Column: Statutes, PO Switcher & QIS Links
                            <div style="display: flex; flex-direction: column; gap: 1.5rem;">
                                // Quick Stats
                                <div class="program-section-card">
                                    <div class="program-section-title">
                                        <span>"📊"</span>
                                        <span>"Übersicht"</span>
                                    </div>
                                    <div style="display: flex; flex-direction: column; gap: 0.5rem; font-size: 0.86rem;">
                                        <div style="display: flex; justify-content: space-between; border-bottom: 1px solid #f1f5f9; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Gesamte Module:"</span>
                                            <strong>{total_mods}</strong>
                                        </div>
                                        <div style="display: flex; justify-content: space-between; border-bottom: 1px solid #f1f5f9; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Summe ECTS:"</span>
                                            <strong>{total_credits as i64} " ECTS"</strong>
                                        </div>
                                        <div style="display: flex; justify-content: space-between; padding-bottom: 0.4rem;">
                                            <span style="color: #64748b;">"Abschlussgrad:"</span>
                                            <strong>{if !deg_badge.is_empty() { deg_badge.clone() } else { "–".to_string() }}</strong>
                                        </div>
                                    </div>
                                </div>

                                // Official Documents & Statutes
                                <div class="program-section-card">
                                    <div class="program-section-title">
                                        <span>"📄"</span>
                                        <span>"Satzungen & Amtsblätter"</span>
                                    </div>

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
                                </div>

                                // Available Regulations / PO Switcher
                                {if all_regs.len() > 1 {
                                    view! {
                                        <div class="program-section-card">
                                            <div class="program-section-title">
                                                <span>"🔄"</span>
                                                <span>"Verfügbare Satzungen"</span>
                                            </div>
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
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {}.into_any()
                                }}

                                // Official BTU Portals & Links
                                <div class="program-section-card">
                                    <div class="program-section-title">
                                        <span>"🌐"</span>
                                        <span>"Offizielle BTU Portale"</span>
                                    </div>
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
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
