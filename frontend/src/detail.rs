use std::collections::HashSet;
use leptos::prelude::*;
use crate::db::*;
use crate::models::*;
use crate::components::{
    BreadcrumbItem, DetailNavBar, DisplayCard, ContentBox,
    study_program_selector::{format_degree_short, format_po_short},
};

#[component]
pub fn ModuleDetailPage<FBack, FOpen, FOpenProg, FToggleBkmk, FToggleComp>(
    module_id: String,
    completed_modules: Signal<HashSet<String>>,
    bookmarked_modules: Signal<HashSet<String>>,
    on_back: FBack,
    on_open_module: FOpen,
    on_open_program: FOpenProg,
    on_toggle_bookmark: FToggleBkmk,
    on_toggle_completed: FToggleComp,
) -> impl IntoView
where
    FBack: Fn() + Copy + Send + Sync + 'static,
    FOpen: Fn(String) + Copy + Send + Sync + 'static,
    FOpenProg: Fn(String) + Copy + Send + Sync + 'static,
    FToggleBkmk: Fn(String) + Copy + Send + Sync + 'static,
    FToggleComp: Fn(String) + Copy + Send + Sync + 'static,
{
    // Fetch module detail from client SQLite DB
    let mod_id_clone = module_id.clone();
    let detail_opt = StoredValue::new(get_module_detail(&mod_id_clone));
    let events = StoredValue::new(get_module_events(&mod_id_clone));
    let linked_programs = StoredValue::new(get_linked_programs(&mod_id_clone));
    let curriculum_entries = StoredValue::new(get_curriculum_entries(&mod_id_clone));

    // Calendar schedules computation
    let calendar_schedules = StoredValue::new(compute_calendar_schedules_from_events(&events.get_value()));

    // Expanded accordion states for events
    let (expanded_events, set_expanded_events) = signal(HashSet::<String>::new());

    // Share link copied toast feedback
    let (share_copied, set_share_copied) = signal(false);

    let on_share = {
        let m_id = module_id.clone();
        move |_| {
            if let Some(win) = web_sys::window() {
                let loc = win.location();
                let share_url = format!("{}{}", loc.origin().unwrap_or_default(), crate::query::module_url(&m_id));
                let nav = win.navigator();
                let clipboard = nav.clipboard();
                let _ = clipboard.write_text(&share_url);
                set_share_copied.set(true);
                gloo_timers::callback::Timeout::new(2000, move || {
                    set_share_copied.set(false);
                }).forget();
            }
        }
    };

    let toggle_accordion = move |id: String| {
        set_expanded_events.update(|set| {
            if set.contains(&id) {
                set.remove(&id);
            } else {
                set.insert(id);
            }
        });
    };

    let select_calendar_event = move |evt_id: String| {
        set_expanded_events.update(|set| {
            set.insert(evt_id.clone());
        });
        if let Some(win) = web_sys::window() {
            if let Some(doc) = win.document() {
                if let Some(el) = doc.get_element_by_id(&format!("accordion-{}", evt_id)) {
                    el.scroll_into_view();
                }
            }
        }
    };

    let on_scroll_top = move |_| {
        if let Some(win) = web_sys::window() {
            win.scroll_to_with_x_and_y(0.0, 0.0);
        }
    };

    if let Some(detail) = detail_opt.get_value() {
        let is_bookmarked = {
            let m_id = module_id.clone();
            Memo::new(move |_| bookmarked_modules.get().contains(&m_id))
        };
        let is_completed = {
            let m_id = module_id.clone();
            Memo::new(move |_| completed_modules.get().contains(&m_id))
        };

        // Reactive prerequisites calculation
        let prereq_info = {
            let mand = detail.prerequisites_mandatory.clone();
            let rec = detail.prerequisites_recommended.clone();
            Memo::new(move |_| {
                let comp = completed_modules.get();
                evaluate_prerequisites(mand.as_deref(), rec.as_deref(), &comp)
            })
        };

        let current_events = events.get_value();
        let has_events = !current_events.is_empty();
        let current_cal = calendar_schedules.get_value();
        let has_cal = !current_cal.is_empty();
        let programs = linked_programs.get_value();
        let cur_entries = curriculum_entries.get_value();
        let has_cur = !cur_entries.is_empty();
        let teaching_forms = detail.parse_teaching_forms();
        let literature = detail.parse_literature();
        let responsible_persons = detail.parse_responsible_persons();
        let successors = detail.successor_list();
        let mod_id_display = module_id.clone();
        let turnus_val = detail.turnus.clone().unwrap_or_else(|| "Nach Ankündigung".to_string());
        let duration_val = detail.duration.clone().unwrap_or_else(|| "1 Semester".to_string());
        let language_val = detail.language.clone().unwrap_or_else(|| "Deutsch".to_string());
        let exam_type_val = detail.exam_type.clone().unwrap_or_else(|| "Prüfungsleistung".to_string());
        let grading_val = detail.grading.clone().unwrap_or_else(|| "Benotet".to_string());
        let department_val = detail.department.clone().unwrap_or_else(|| "BTU Cottbus-Senftenberg".to_string());
        let clean_department_val = detail.clean_department();
        let formatted_credits_val = detail.formatted_credits();
        let module_id_attr = module_id.clone();
        let remarks_val = detail.remarks.clone();
        let raw_url_top = detail.raw_url.clone();
        let raw_url_bot = detail.raw_url.clone();

        view! {
            <div class="module-detail-page" id="module-detail-view" data-module-id=module_id_attr>

                // Unified Navigation Bar
                <DetailNavBar
                    back_text="Zurück zum Katalog"
                    shortcut_label="Esc"
                    on_back=Callback::new(move |_| on_back())
                    breadcrumbs=vec![
                        BreadcrumbItem::link("Modulkatalog", Callback::new(move |_| on_back())),
                        BreadcrumbItem::current(format!("Modul {}", mod_id_display)),
                    ]
                >
                    {
                        let m_id = module_id.clone();
                        view! {
                            <button
                                type="button"
                                class=move || format!("detail-action-btn btn-bookmark {}", if is_bookmarked.get() { "active" } else { "" })
                                id="detail-btn-bookmark"
                                on:click=move |_| on_toggle_bookmark(m_id.clone())
                                title="Modul merken / von Merkliste entfernen"
                            >
                                <span class="btn-icon">"⭐"</span>
                                <span class="btn-label" id="detail-bookmark-label">
                                    {move || if is_bookmarked.get() { "Gemerkt" } else { "Merken" }}
                                </span>
                            </button>
                        }
                    }

                    {
                        let m_id = module_id.clone();
                        view! {
                            <button
                                type="button"
                                class=move || format!("detail-action-btn btn-complete {}", if is_completed.get() { "active" } else { "" })
                                id="detail-btn-complete"
                                on:click=move |_| on_toggle_completed(m_id.clone())
                                title="Als bestanden markieren / Markierung aufheben"
                            >
                                <span class="btn-icon">"✓"</span>
                                <span class="btn-label" id="detail-complete-label">
                                    {move || if is_completed.get() { "Bestanden" } else { "+ Bestanden?" }}
                                </span>
                            </button>
                        }
                    }

                    <button
                        type="button"
                        class=move || format!("detail-action-btn btn-share {}", if share_copied.get() { "copied" } else { "" })
                        id="detail-btn-share"
                        on:click=on_share
                        title="Direktlink zu diesem Modul kopieren"
                    >
                        <span class="btn-icon">"🔗"</span>
                        <span class="btn-label" id="detail-share-label">
                            {move || if share_copied.get() { "Kopiert! ✓" } else { "Teilen" }}
                        </span>
                    </button>

                    {if let Some(ref raw_url) = raw_url_top {
                        view! {
                            <a
                                href=raw_url.clone()
                                target="_blank"
                                rel="noopener noreferrer"
                                class="detail-action-btn btn-portal"
                                title="Offizielle Modulbeschreibung auf b-tu.de öffnen"
                            >
                                <span class="btn-icon">"🌐"</span>
                                <span class="btn-label">"b-tu.de ↗"</span>
                            </a>
                        }.into_any()
                    } else {
                        ().into_any()
                    }}
                </DetailNavBar>

                //
                <header class="detail-hero-header">
                    <div class="detail-badges-row">
                        <span class="detail-badge badge-code">{format!("ID {}", detail.id)}</span>
                        <span class="detail-badge badge-credits">{format!("{} ECTS", detail.formatted_credits())}</span>

                        {if detail.is_not_offered.unwrap_or(0) == 1 {
                            view! { <span class="detail-badge badge-status-danger">"⛔ Nicht mehr im Angebot"</span> }.into_any()
                        } else if detail.is_phase_out.unwrap_or(0) == 1 {
                            view! { <span class="detail-badge badge-status-warning">"⚠️ Auslaufend"</span> }.into_any()
                        } else {
                            ().into_any()
                        }}

                        {if detail.is_fues.unwrap_or(0) == 1 || detail.cross_disciplinary.unwrap_or(0) == 1 {
                            view! { <span class="detail-badge badge-fues">"🌐 Fachübergreifendes Studium (FÜS)"</span> }.into_any()
                        } else {
                            ().into_any()
                        }}

                        {if let Some(ref lim) = detail.limitation {
                            let l = lim.trim().to_string();
                            let low = l.to_lowercase();
                            if !l.is_empty() && low != "keine" && low != "nein" && low != "ohne" && low != "k.a." {
                                view! {
                                    <span class="detail-badge badge-limitation" title=format!("Teilnehmerbeschränkung: {}", l)>
                                        {format!("👥 {}", l)}
                                    </span>
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        } else {
                            ().into_any()
                        }}

                        <span class="detail-badge badge-lang">
                            {format!("{} {}", detail.formatted_lang(), detail.language.as_deref().unwrap_or("Deutsch"))}
                        </span>
                    </div>

                    <h1 class="detail-title-de">{detail.title_de.clone()}</h1>
                    {if let Some(ref en) = detail.title_en {
                        view! { <div class="detail-title-en">{en.clone()}</div> }.into_any()
                    } else {
                        ().into_any()
                    }}
                </header>

                // KPI Display Cards
                <section class="detail-kpi-grid" aria-label="Wichtigste Kenndaten">
                    <DisplayCard
                        label="Leistungspunkte"
                        value=format!("{} ECTS", formatted_credits_val)
                        subtext=if !teaching_forms.is_empty() {
                            format!("{} Lehrform(en)", teaching_forms.len())
                        } else {
                            "Standardumfang".to_string()
                        }
                        emoji="🎓"
                        icon_class="kpi-blue"
                    />

                    <DisplayCard
                        label="Turnus & Dauer"
                        value=turnus_val.clone()
                        subtext=duration_val.clone()
                        emoji="🗓️"
                        icon_class="kpi-amber"
                    />

                    <DisplayCard
                        label="Unterrichtssprache"
                        value=language_val.clone()
                        subtext="BTU Cottbus-Senftenberg"
                        emoji="🗣️"
                        icon_class="kpi-emerald"
                    />

                    <DisplayCard
                        label="Prüfungsleistung"
                        value=exam_type_val.clone()
                        subtext=grading_val.clone()
                        emoji="📝"
                        icon_class="kpi-purple"
                        tooltip=exam_type_val.clone()
                    />

                    <DisplayCard
                        label="Modulverantwortung"
                        value=if !responsible_persons.is_empty() {
                            responsible_persons.join(", ")
                        } else {
                            "Keine Angabe".to_string()
                        }
                        subtext="Fachbereich & Lehrende"
                        emoji="👤"
                        icon_class="kpi-indigo"
                        wide=true
                    />

                    <DisplayCard
                        label="Fakultät / Fachbereich"
                        value=department_val.clone()
                        subtext=clean_department_val.clone()
                        emoji="🏛️"
                        icon_class="kpi-slate"
                        tooltip=department_val.clone()
                        wide=true
                    />
                </section>

                //
                {if !successors.is_empty() {
                    let succs = successors.clone();
                    view! {
                        <aside class="detail-banner-successor" aria-label="Nachfolgemodule">
                            <div class="banner-icon">"➡️"</div>
                            <div class="banner-body">
                                <strong>"Nachfolgemodul(e) vorhanden:"</strong>
                                <span>"Dieses Modul wurde durch ein Nachfolgemodul abgelöst:"</span>
                                <div class="banner-actions">
                                    {succs.into_iter().map(|succ_id| {
                                        let sid = succ_id.clone();
                                        view! {
                                            <button type="button" class="btn-successor-link" on:click=move |_| on_open_module(sid.clone())>
                                                {format!("Modul {} ansehen ↗", succ_id)}
                                            </button>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>
                        </aside>
                    }.into_any()
                } else {
                    ().into_any()
                }}

                //
                {move || {
                    let (status, missing_mand, missing_rec) = prereq_info.get();
                    match status {
                        PrereqStatus::Met => view! {
                            <div class="detail-prereq-alert alert-met" role="status">
                                <div class="alert-icon">"🟢"</div>
                                <div class="alert-content">
                                    <strong>"Voraussetzungen vollständig erfüllt"</strong>
                                    <p>"Du erfüllst alle formalen und empfohlenen Voraussetzungen für dieses Modul."</p>
                                </div>
                            </div>
                        }.into_any(),
                        PrereqStatus::RecommendedMissing(_) => view! {
                            <div class="detail-prereq-alert alert-recommended" role="status">
                                <div class="alert-icon">"🟡"</div>
                                <div class="alert-content">
                                    <strong>"Empfohlene Vorkenntnisse noch nicht absolviert"</strong>
                                    <p>
                                        "Für optimales Verständnis wird der vorherige Abschluss folgender Module empfohlen: "
                                        <strong>{missing_rec.join(", ")}</strong>
                                    </p>
                                </div>
                            </div>
                        }.into_any(),
                        PrereqStatus::Missing(_) => view! {
                            <div class="detail-prereq-alert alert-missing" role="alert">
                                <div class="alert-icon">"🔴"</div>
                                <div class="alert-content">
                                    <strong>"Pflicht-Voraussetzungen fehlen"</strong>
                                    <p>"Gemäß Prüfungsordnung musst du folgende Module vor Belegung erfolgreich bestanden haben:"</p>
                                    <div class="alert-missing-buttons">
                                        {missing_mand.into_iter().map(|mid| {
                                            let m_id_check = mid.clone();
                                            let m_id_inspect = mid.clone();
                                            let m_id_lbl = mid.clone();
                                            view! {
                                                <div class="missing-module-pill">
                                                    <span class="pill-id">{format!("Modul {}", m_id_lbl)}</span>
                                                    <button
                                                        type="button"
                                                        class="btn-pill-check"
                                                        on:click=move |_| on_toggle_completed(m_id_check.clone())
                                                        title="Als bestanden markieren"
                                                    >
                                                        "✓ jetzt abhaken"
                                                    </button>
                                                    <button
                                                        type="button"
                                                        class="btn-pill-inspect"
                                                        on:click=move |_| on_open_module(m_id_inspect.clone())
                                                        title="Moduldetails öffnen"
                                                    >
                                                        "ansehen ↗"
                                                    </button>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            </div>
                        }.into_any(),
                        PrereqStatus::None => ().into_any(),
                    }
                }}

                //
                <div class="detail-two-column-layout">

                    //
                    <div class="detail-column-primary">

                        //
                        // Qualifikationsziele & Kompetenzen
                        <ContentBox title="Qualifikationsziele & Kompetenzen" emoji="🎯">
                            {if let Some(ref outcomes) = detail.learning_outcomes {
                                view! { <div class="detail-formatted-text" style="white-space: pre-line;">{outcomes.clone()}</div> }.into_any()
                            } else {
                                view! { <p class="detail-text-empty">"Keine spezifischen Qualifikationsziele hinterlegt."</p> }.into_any()
                            }}
                        </ContentBox>

                        // Inhalte der Lehrveranstaltungen
                        <ContentBox title="Inhalte der Lehrveranstaltungen" emoji="📖">
                            {if let Some(ref contents) = detail.contents {
                                view! { <div class="detail-formatted-text" style="white-space: pre-line;">{contents.clone()}</div> }.into_any()
                            } else {
                                view! { <p class="detail-text-empty">"Keine Inhaltsbeschreibung verfügbar."</p> }.into_any()
                            }}
                        </ContentBox>

                        // Voraussetzungen
                        <ContentBox title="Voraussetzungen" emoji="🔒">
                            <div class="prereq-details-grid">
                                <div class="prereq-detail-block">
                                    <div class="prereq-type-badge prereq-type-mandatory">"Zwingend erforderlich"</div>
                                    {if let Some(ref mand) = detail.prerequisites_mandatory {
                                        let m = mand.trim();
                                        if !m.is_empty() && m != "-" && m != "keine" {
                                            view! { <div class="prereq-detail-text">{m.to_string()}</div> }.into_any()
                                        } else {
                                            view! { <div class="prereq-detail-text text-muted">"Keine zwingenden formalen Voraussetzungen vorgegeben."</div> }.into_any()
                                        }
                                    } else {
                                        view! { <div class="prereq-detail-text text-muted">"Keine zwingenden formalen Voraussetzungen vorgegeben."</div> }.into_any()
                                    }}
                                </div>

                                <div class="prereq-detail-block">
                                    <div class="prereq-type-badge prereq-type-recommended">"Empfohlene Vorkenntnisse"</div>
                                    {if let Some(ref rec) = detail.prerequisites_recommended {
                                        let r = rec.trim();
                                        if !r.is_empty() && r != "-" && r != "keine" {
                                            view! { <div class="prereq-detail-text">{r.to_string()}</div> }.into_any()
                                        } else {
                                            view! { <div class="prereq-detail-text text-muted">"Keine spezifischen Vorkenntnisse empfohlen."</div> }.into_any()
                                        }
                                    } else {
                                        view! { <div class="prereq-detail-text text-muted">"Keine spezifischen Vorkenntnisse empfohlen."</div> }.into_any()
                                    }}
                                </div>
                            </div>
                        </ContentBox>

                        // Lehrformen & Arbeitsaufwand
                        {if !teaching_forms.is_empty() {
                            let tfs = teaching_forms.clone();
                            view! {
                                <ContentBox title="Lehrformen & Arbeitsaufwand" emoji="⏱️">
                                    <div class="teaching-forms-grid">
                                        {tfs.into_iter().map(|tf| {
                                            view! {
                                                <div class="teaching-form-card">
                                                    <div class="teaching-form-type">{tf.form_type.unwrap_or_default()}</div>
                                                    <div class="teaching-form-workload">{tf.workload.unwrap_or_default()}</div>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </ContentBox>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}

                        // Prüfungsmodalitäten & Bewertung
                        <ContentBox title="Prüfungsmodalitäten & Bewertung" emoji="⚖️">
                            <div class="exam-info-table">
                                <div class="exam-info-row">
                                    <span class="exam-info-label">"Prüfungsform:"</span>
                                    <span class="exam-info-val">{exam_type_val.clone()}</span>
                                </div>
                                {if let Some(ref ed) = detail.exam_details {
                                    view! {
                                        <div class="exam-info-row">
                                            <span class="exam-info-label">"Prüfungsdetails & Dauer:"</span>
                                            <span class="exam-info-val">{ed.clone()}</span>
                                        </div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                                <div class="exam-info-row">
                                    <span class="exam-info-label">"Bewertung:"</span>
                                    <span class="exam-info-val">{grading_val.clone()}</span>
                                </div>
                                {if let Some(ref lim) = detail.limitation {
                                    view! {
                                        <div class="exam-info-row">
                                            <span class="exam-info-label">"Teilnehmer:innenbeschränkung:"</span>
                                            <span class="exam-info-val">{lim.clone()}</span>
                                        </div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            </div>
                        </ContentBox>

                        // Empfohlene Literatur & Quellen
                        {if !literature.is_empty() {
                            let lits = literature.clone();
                            view! {
                                <ContentBox title="Empfohlene Literatur & Quellen" emoji="📚">
                                    <ul class="detail-literature-list">
                                        {lits.into_iter().map(|lit| {
                                            view! { <li class="literature-item">{lit}</li> }
                                        }).collect::<Vec<_>>()}
                                    </ul>
                                </ContentBox>
                            }.into_any()
                        } else {
                            ().into_any()
                        }}

                        // Besondere Hinweise
                        {if let Some(ref rem) = remarks_val {
                            if !rem.trim().is_empty() {
                                let rem_text = rem.clone();
                                view! {
                                    <ContentBox title="Besondere Hinweise" emoji="💡">
                                        <div class="detail-formatted-text" style="white-space: pre-line;">{rem_text}</div>
                                    </ContentBox>
                                }.into_any()
                            } else {
                                ().into_any()
                            }
                        } else {
                            ().into_any()
                        }}

                    </div>

                    //
                    <div class="detail-column-secondary">

                        //
                        // Stundenplan & Termine
                        <ContentBox
                            title="Stundenplan & Termine"
                            subtitle=if has_events {
                                format!("{} Lehrveranstaltung(en) im aktuellen Semester", current_events.len())
                            } else {
                                "Aktuelles Semester".to_string()
                            }
                            emoji="📅"
                            sticky=true
                        >
                            // Wochenübersicht / Stundenplan-Grid
                            {if has_cal {
                                view! {
                                    <div class="schedule-calendar-wrapper">
                                        <div class="schedule-calendar-label">"Wochenübersicht (Klick fokussiert Einzeltermin):"</div>
                                        <div class="calendar-container">
                                            <div class="calendar-header">
                                                <div>"Zeit"</div>
                                                <div>"Mo"</div>
                                                <div>"Di"</div>
                                                <div>"Mi"</div>
                                                <div>"Do"</div>
                                                <div>"Fr"</div>
                                            </div>
                                            <div class="calendar-grid">
                                                <div class="calendar-time-col">
                                                    <span>"08:00"</span>
                                                    <span>"10:00"</span>
                                                    <span>"12:00"</span>
                                                    <span>"14:00"</span>
                                                    <span>"16:00"</span>
                                                    <span>"18:00"</span>
                                                    <span>"20:00"</span>
                                                </div>

                                                {(1..=5).map(|day| {
                                                    let day_schedules: Vec<_> = current_cal.iter().filter(|s| s.day_index == day).cloned().collect();
                                                    view! {
                                                        <div class="calendar-day-col" data-day=day>
                                                            {day_schedules.into_iter().map(|s| {
                                                                let evt_id = s.event_id.clone();
                                                                let evt_type = s.event_type.clone();
                                                                let extra_class = if evt_type.contains("Übung") {
                                                                    "uebung"
                                                                } else if evt_type.contains("Praktikum") {
                                                                    "praktikum"
                                                                } else {
                                                                    ""
                                                                };
                                                                view! {
                                                                    <div
                                                                        class=format!("cal-event-block {}", extra_class)
                                                                        style=format!("top: {:.1}%; height: {:.1}%;", s.top_percent, s.height_percent)
                                                                        on:click=move |_| select_calendar_event(evt_id.clone())
                                                                        title=format!("{} ({} - {} Uhr) [{}]", s.title, s.start_time, s.end_time, s.room)
                                                                    >
                                                                        <div class="cal-event-title">{s.event_type}</div>
                                                                        <div>{format!("{}–{}", s.start_time, s.end_time)}</div>
                                                                        {if !s.room.is_empty() {
                                                                            view! { <div class="cal-event-room">{truncate(&s.room, 15)}</div> }.into_any()
                                                                        } else {
                                                                            ().into_any()
                                                                        }}
                                                                    </div>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </div>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>
                                        </div>
                                    </div>
                                }.into_any()
                            } else if has_events {
                                view! {
                                    <div class="schedule-empty-note">
                                        <span>"📅"</span>
                                        <div>
                                            <strong>"Keine festen wöchentlichen Zeitfenster hinterlegt"</strong>
                                            <p>"Möglicherweise Blockveranstaltung, asynchrone Termine oder individuelle Absprache."</p>
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}

                            // Veranstaltungsliste
                            <div class="detail-events-list">
                                {if has_events {
                                    view! {
                                        <div>
                                            {current_events.into_iter().map(|evt| {
                                                let e_id = evt.id.clone();
                                                let id_chk1 = e_id.clone();
                                                let id_chk2 = e_id.clone();
                                                let toggle_fn = {
                                                    let id_tog = e_id.clone();
                                                    move |_| toggle_accordion(id_tog.clone())
                                                };
                                                view! {
                                                    <div class="detail-event-item" id=format!("accordion-{}", e_id)>
                                                        <div class="event-item-header" on:click=toggle_fn>
                                                            <div class="event-title-line">
                                                                {if let Some(ref nr) = evt.event_number {
                                                                    view! { <span class="badge badge-id">{nr.clone()}</span> }.into_any()
                                                                } else {
                                                                    ().into_any()
                                                                }}
                                                                <strong class="event-title">{evt.title}</strong>
                                                            </div>
                                                            <div class="event-meta-line">
                                                                {if let Some(ref t) = evt.event_type {
                                                                    view! { <span class="badge-event-type">{t.clone()}</span> }.into_any()
                                                                } else {
                                                                    ().into_any()
                                                                }}
                                                                {if let Some(ref sws) = evt.sws {
                                                                    view! { <span class="badge-sws">{sws.clone()}</span> }.into_any()
                                                                } else {
                                                                    ().into_any()
                                                                }}
                                                                <span class="accordion-caret">{move || if expanded_events.get().contains(&id_chk1) { "▲" } else { "▼" }}</span>
                                                            </div>
                                                        </div>

                                                        <div class="event-item-body" style=move || if expanded_events.get().contains(&id_chk2) { "display: block;" } else { "display: none;" }>
                                                            {if !evt.schedules.is_empty() {
                                                                view! {
                                                                    <div class="event-schedules-list">
                                                                        {evt.schedules.into_iter().map(|sc| {
                                                                            view! {
                                                                                <div class="schedule-entry">
                                                                                    <div class="schedule-time">
                                                                                        <strong>{sc.day_of_week.unwrap_or_default()}</strong>
                                                                                        " "
                                                                                        {if let (Some(s), Some(e)) = (sc.start_time, sc.end_time) {
                                                                                            format!("{} – {} Uhr", s, e)
                                                                                        } else {
                                                                                            sc.time_slot.unwrap_or_default()
                                                                                        }}
                                                                                        {if let Some(r) = sc.rhythm {
                                                                                            view! { <span class="schedule-rhythm">{format!(" ({})", r)}</span> }.into_any()
                                                                                        } else {
                                                                                            ().into_any()
                                                                                        }}
                                                                                    </div>
                                                                                    {if let Some(room) = sc.room {
                                                                                        view! {
                                                                                            <div class="schedule-detail-line">
                                                                                                "📍 " <strong>"Raum:"</strong> " " {room}
                                                                                            </div>
                                                                                        }.into_any()
                                                                                    } else {
                                                                                        ().into_any()
                                                                                    }}
                                                                                    {if let Some(inst) = sc.instructor {
                                                                                        view! {
                                                                                            <div class="schedule-detail-line">
                                                                                                "👤 " <strong>"Dozent:"</strong> " " {inst}
                                                                                            </div>
                                                                                        }.into_any()
                                                                                    } else {
                                                                                        ().into_any()
                                                                                    }}
                                                                                </div>
                                                                            }
                                                                        }).collect::<Vec<_>>()}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                ().into_any()
                                                            }}

                                                            <div class="event-footer-line">
                                                                {if let Some(ref sem) = evt.semester {
                                                                    view! { <span class="event-semester-tag">{format!("Semester: {}", sem)}</span> }.into_any()
                                                                } else {
                                                                    ().into_any()
                                                                }}
                                                                {if let Some(ref raw_url) = evt.raw_url {
                                                                    view! {
                                                                        <a href=raw_url.clone() target="_blank" rel="noopener noreferrer" class="btn-qis-link">
                                                                            "Im QIS-Vorlesungsverzeichnis ↗"
                                                                        </a>
                                                                    }.into_any()
                                                                } else {
                                                                    ().into_any()
                                                                }}
                                                            </div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="schedule-none-message">
                                            "Keine Lehrveranstaltungen für das aktuelle Semester hinterlegt."
                                        </div>
                                    }.into_any()
                                }}
                            </div>
                        </ContentBox>

                        // Zugeordnete Studiengänge
                        <ContentBox
                            title="Zugeordnete Studiengänge"
                            subtitle=format!("In {} Studiengängen anerkannt", programs.len())
                            emoji="🎓"
                        >
                            {if has_cur {
                                view! {
                                    <div style="margin-bottom: 1.25rem; padding-bottom: 1rem; border-bottom: 1px solid var(--border-color, #e2e8f0);">
                                        <div style="font-size: 0.85rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.04em; color: var(--color-primary, #005a9c); margin-bottom: 0.75rem; display: flex; align-items: center; gap: 0.4rem;">
                                            <span>"📋"</span> <span>"Offizielle Studienplan-Semesterempfehlung (Satzungsprüfung)"</span>
                                        </div>
                                        <div class="studyprograms-tags-list">
                                            {cur_entries.into_iter().map(|c| {
                                                let p_id = c.program_id.clone();
                                                let deg_short = format_degree_short(c.degree.as_deref());
                                                let po_short = c.po_version.as_deref().map(format_po_short).unwrap_or_default();
						let evidence=c.source_evidence.as_deref().and_then(|s|serde_json::from_str::<crate::study_plan::PlanCell>(s).ok());
						let sem_str=crate::study_plan::semester_label(c.start_semester.filter(|s|*s>0).or(c.recommended_semester).unwrap_or(0),c.end_semester.filter(|s|*s>0).or(c.recommended_semester).unwrap_or(0));
						let credit_hint=evidence.map(|cell|if cell.is_workload(){format!("{} insgesamt · Arbeitsaufwand {} ECTS · Anrechnung im {}. Semester",cell.credits(),cell.workload.iter().map(|v|crate::study_plan::number(*v)).collect::<Vec<_>>().join(" + "),cell.credit_semester)}else{format!("{}{}",cell.credits(),if cell.is_window(){" im gesamten Zeitraum"}else{""})});
                                                view! {
                                                    <div
                                                        class="studyprogram-item-card interactive"
                                                        style="border-left: 3px solid var(--primary);"
                                                        on:click=move |_| on_open_program(p_id.clone())
                                                    >
                                                        {if !deg_short.is_empty() {
                                                            view! { <span class="prog-degree-badge">{deg_short}</span> }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                        <span class="prog-name">{c.program_name}</span>
                                                        <div class="prog-meta-badges">
                                                            <span class="badge-semester">{sem_str}</span>
						    {credit_hint.map(|hint|view!{<span class="badge-module-type">{hint}</span>})}
                                                            {if let Some(ref mt) = c.module_type {
                                                                if !mt.is_empty() {
                                                                    view! { <span class="badge-module-type">{mt.clone()}</span> }.into_any()
                                                                } else {
                                                                    ().into_any()
                                                                }
                                                            } else {
                                                                ().into_any()
                                                            }}
                                                            {if !po_short.is_empty() {
                                                                view! { <span class="prog-po-badge">{po_short}</span> }.into_any()
                                                            } else {
                                                                ().into_any()
                                                            }}
                                                            <span class="prog-link-arrow">"→"</span>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}

                            {if !programs.is_empty() {
                                view! {
                                    <div class="studyprograms-tags-list">
                                        {programs.into_iter().map(|p| {
                                            let p_id = p.id.clone();
                                            let deg_short = format_degree_short(p.degree.as_deref());
                                            let po_short = p.po_version.as_deref().map(format_po_short).unwrap_or_default();
                                            view! {
                                                <div
                                                    class="studyprogram-item-card interactive"
                                                    on:click=move |_| on_open_program(p_id.clone())
                                                >
                                                    {if !deg_short.is_empty() {
                                                        view! { <span class="prog-degree-badge">{deg_short}</span> }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                    <span class="prog-name">{p.program_name}</span>
                                                    <div class="prog-meta-badges">
                                                        {if !po_short.is_empty() {
                                                            view! { <span class="prog-po-badge">{po_short}</span> }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                        <span class="prog-link-arrow">"→"</span>
                                                    </div>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <p class="detail-text-empty">"Keine spezifische Studiengangszuordnung hinterlegt."</p>
                                }.into_any()
                            }}
                        </ContentBox>

                    </div>

                </div>

                //
                <footer class="detail-bottom-bar">
                    <button type="button" class="btn-detail-back" on:click=move |_| on_back()>
                        <span class="back-arrow">"←"</span>
                        <span>"Zurück zum Modulkatalog"</span>
                    </button>

                    <button type="button" class="btn-scroll-top" on:click=on_scroll_top>
                        <span>"Nach oben ↑"</span>
                    </button>

                    {if let Some(ref raw_url) = raw_url_bot {
                        let r_url = raw_url.clone();
                        view! {
                            <a href=r_url target="_blank" rel="noopener noreferrer" class="btn-portal-secondary">
                                <span>"Offizielle Modulbeschreibung auf b-tu.de ↗"</span>
                            </a>
                        }.into_any()
                    } else {
                        ().into_any()
                    }}
                </footer>

            </div>
        }.into_any()
    } else {
        view! {
            <div class="module-detail-page">
                <div class="detail-error-box">
                    <h3>"Modul konnte nicht gefunden werden"</h3>
                    <p>{format!("Die Modul-ID '{}' existiert nicht in der Datenbank.", module_id)}</p>
                    <button type="button" class="btn-detail-back" on:click=move |_| on_back()>
                        "← Zurück zum Katalog"
                    </button>
                </div>
            </div>
        }.into_any()
    }
}
