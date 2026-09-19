use std::collections::{BTreeMap,HashSet};
use leptos::prelude::*;
use crate::{db::*,models::ModuleCardItem,study_plan::*};

#[component]
pub fn StudyPlanView(
    program_id:String,
    modules:Vec<ModuleCardItem>,
    on_open:Callback<String>,
    completed:Signal<HashSet<String>>,
    bookmarked:Signal<HashSet<String>>,
) -> impl IntoView {
    let plan=get_verified_study_plan(&program_id).unwrap_or_default();
    let variants=plan.variants();
    let (selected,set_selected)=signal(variants.keys().next().cloned().unwrap_or_default());
    let mut linked=BTreeMap::<String,ModuleCardItem>::new();
    for m in &modules {if let Some(c)=m.source_evidence.as_deref().and_then(|s|serde_json::from_str::<PlanCell>(s).ok()){linked.insert(c.id,m.clone());}}
    let normalize=|s:&str|s.replace('\\',"/").replace("//","/");
    let source_file=linked.values().find_map(|m|m.source_file.clone()).unwrap_or_default();
    let source_url=get_study_program_detail(&program_id).into_iter().flat_map(|d|d.parse_documents()).find(|d|d.local_path.as_deref().is_some_and(|p|normalize(p)==normalize(&source_file))).and_then(|d|d.url).unwrap_or_default();
    let state=StoredValue::new((plan,linked,source_url));
    view! {
        <section class="study-plan" aria-label="Regelstudienplan">
            <div class="plan-intro">
                <span class="plan-eyebrow">"DEIN STUDIENVERLAUF"</span>
                <h2>"Semester für Semester"</h2>
                <p>"Module, Wahlpflichtbereiche und Zeiträume aus deinem Regelstudienplan. ECTS werden nur dort einem Semester zugeordnet, wo die Ordnung es festlegt."</p>
            </div>
            {if variants.is_empty(){view!{
                <div class="plan-empty"><h3>"Studienplan noch nicht bestätigt"</h3><p>"Für diese Prüfungsordnung liegt noch keine vollständig geprüfte Semesteraufteilung vor. Die verfügbaren Module und Originaldokumente findest du im Katalog und unter Satzungen."</p></div>
            }.into_any()}else{view!{
                {if variants.len()>1{view!{
                    <div class="plan-variants"><strong>"Studienrichtung wählen"</strong><p>"Jede Variante ist ein eigener Studienplan."</p>
                        <div role="group" aria-label="Studienvarianten">{variants.into_iter().map(|(id,name)|{
                            let click=id.clone();let active=id.clone();
                            view!{<button type="button" class:active=move ||selected.get()==active aria-pressed=move ||selected.get()==id on:click=move |_|set_selected.set(click.clone())>{name.replace("Regelstudienplan der Studienrichtungen ","")}</button>}
                        }).collect_view()}</div>
                    </div>
                }.into_any()}else{().into_any()}}
                {move || {
                    let (plan,linked,source_url)=state.get_value();
                    let table=selected.get();
                    let cells:Vec<_>=plan.cells.iter().filter(|c|c.table==table).cloned().collect();
                    let semesters=cells.iter().flat_map(|c|&c.semesters).copied().max().unwrap_or(0);
                    let totals=plan.totals_for(&table);
                    let total:f64=totals.iter().map(|t|t.min).sum();
                    let mut windows=BTreeMap::<(i64,i64),Vec<PlanCell>>::new();
                    for c in &cells{if c.is_window(){windows.entry((*c.semesters.first().unwrap(),*c.semesters.last().unwrap())).or_default().push(c.clone());}}
                    let pages:std::collections::BTreeSet<_>=cells.iter().map(|c|c.page).collect();
                    let page_label=pages.iter().map(|p|p.to_string()).collect::<Vec<_>>().join(", ");
                    view!{
                        <div class="plan-summary"><div><strong>{semesters}</strong><span>"Fachsemester"</span></div><div><strong>{number(total)}</strong><span>"ECTS insgesamt"</span></div><div><strong>{cells.len()}</strong><span>"Module & Wahlpflichtvorgaben"</span></div></div>
                        <div class="plan-legend"><span><i class="plan-dot fixed"></i>"Festes Semester"</span><span><i class="plan-dot ongoing"></i>"Mehrsemestriges Modul"</span><span><i class="plan-dot window"></i>"Gemeinsamer Zeitraum"</span></div>
                        <div class="plan-semester-grid">
                            {(1..=semesters).map(|sem|{
                                let current:Vec<_>=cells.iter().filter(|c|c.semesters.contains(&sem)&&!c.is_window()).cloned().collect();
                                let fixed_total=totals.iter().find(|t|t.semesters==vec![sem]).map(|t|t.credits());
                                let workload=plan.totals.iter().find(|t|t.table==table&&t.row.to_lowercase()=="summe aufwand"&&t.semesters==vec![sem]).map(|t|t.credits());
                                let involved:Vec<_>=windows.keys().filter(|(a,b)|sem>=*a&&sem<=*b).copied().collect();
                                let shown_total=fixed_total.clone().unwrap_or_else(||"Gemeinsames ECTS-Budget".into());
                                view!{
                                    <section class="plan-semester" aria-label=format!("{sem}. Fachsemester")>
                                        <header><div class="plan-semester-number">{format!("{sem:02}")}</div><div><h3>{format!("{sem}. Semester")}</h3><p>{shown_total}</p>{workload.filter(|w|Some(w)!=fixed_total.as_ref()).map(|w|view!{<small>{format!("{w} Arbeitsaufwand")}</small>})}</div></header>
                                        <div class="plan-rows">{current.into_iter().map(|cell|{
                                            let module=linked.get(&cell.id).cloned();
                                            view!{<PlanRequirement cell=cell module=module semester=sem on_open=on_open completed=completed bookmarked=bookmarked/>}
                                        }).collect_view()}</div>
                                        {involved.into_iter().map(|(a,b)|view!{<a class="plan-window-reference" href=format!("#plan-window-{a}-{b}")><span>"↔"</span><span>{format!("Gemeinsam mit Semester {}",if sem==a{b}else{a})}<small>"Vorgaben für den gesamten Zeitraum ansehen ↓"</small></span></a>}).collect_view()}
                                    </section>
                                }
                            }).collect_view()}
                        </div>
                        {windows.into_iter().map(|((a,b),entries)|{
                            let joint=totals.iter().find(|t|t.semesters.first()==Some(&a)&&t.semesters.last()==Some(&b)).map(|t|t.credits());
                            view!{
                                <section class="plan-window" id=format!("plan-window-{a}-{b}")>
                                    <header><div><span class="plan-eyebrow">"GEMEINSAMER ZEITRAUM"</span><h3>{semester_label(a,b)}</h3></div>{joint.map(|s|view!{<strong>{format!("{s} insgesamt")}</strong>})}</header>
                                    <p class="plan-window-explanation">"Diese Vorgaben gelten zusammen über den gesamten Zeitraum. Die Ordnung legt keine Verteilung auf die einzelnen Semester fest. Die Bereichsgrenzen müssen innerhalb des gemeinsamen Gesamtumfangs eingehalten werden."</p>
                                    <div class="plan-rows">{entries.into_iter().map(|cell|{let module=linked.get(&cell.id).cloned();view!{<PlanRequirement cell=cell module=module semester=0 on_open=on_open completed=completed bookmarked=bookmarked/>}}).collect_view()}</div>
                                </section>
                            }
                        }).collect_view()}
                        <footer class="plan-source"><span>{format!("Quelle: Regelstudienplan · PDF-Seite {page_label}")}</span>{if !source_url.is_empty(){view!{<a href=format!("{source_url}#page={}",pages.first().unwrap_or(&1)) target="_blank" rel="noopener noreferrer">"Original öffnen ↗"</a>}.into_any()}else{().into_any()}}</footer>
                    }
                }}
            }.into_any()}}
        </section>
    }
}

#[component]
fn PlanRequirement(cell:PlanCell,module:Option<ModuleCardItem>,semester:i64,on_open:Callback<String>,completed:Signal<HashSet<String>>,bookmarked:Signal<HashSet<String>>) -> impl IntoView {
    let module_id=module.as_ref().map(|m|m.id.clone()).filter(|id|!id.starts_with("curriculum_"));
    let click=module_id.clone();let comp=module_id.clone();let mark=module_id.clone();
    let is_workload=cell.is_workload();let is_window=cell.is_window();
    let credit=if is_workload {format!("{} ECTS Aufwand",number(cell.workload_in(semester).unwrap_or(0.)))} else {cell.credits()};
    let hint=if is_workload {
        let phase=if semester==*cell.semesters.first().unwrap(){"Beginn"}else{"Fortsetzung / Abschluss"};
        format!("{phase} · {} · {} gesamt, Anrechnung im {}. Semester",cell.period(),cell.credits(),cell.credit_semester)
    }else if is_window{format!("{} · insgesamt im Zeitraum",cell.period())}else{
        let kind=module.as_ref().and_then(|m|m.module_type.clone()).unwrap_or_default();
        if kind=="Modul"{"".into()}else{kind}
    };
    let requirement=module_id.is_none();
    let title=cell.row.clone();
    view!{
        <div class="plan-requirement" class:plan-ongoing=is_workload class:plan-flexible=is_window>
            <div class="plan-requirement-text">
                {if let Some(id)=click {view!{<button type="button" class="plan-module-link" on:click=move |_|on_open.run(id.clone())>{title}</button>}.into_any()}else{view!{<strong>{title}</strong>}.into_any()}}
                {if !hint.is_empty(){view!{<small>{hint}</small>}.into_any()}else{().into_any()}}
                {move ||comp.as_ref().filter(|id|completed.get().contains(*id)).map(|_|view!{<span class="plan-completed">"✓ Abgeschlossen"</span>})}
                {move ||mark.as_ref().filter(|id|bookmarked.get().contains(*id)).map(|_|view!{<span class="plan-bookmarked">"★ Gemerkt"</span>})}
                {if requirement&&!is_window{view!{<span class="plan-requirement-label">"Studienplanvorgabe"</span>}.into_any()}else{().into_any()}}
            </div>
            <span class="plan-credits">{credit}</span>
        </div>
    }
}
