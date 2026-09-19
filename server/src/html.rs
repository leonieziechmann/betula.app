use std::fs;
use std::path::Path;
use html_escape::encode_text;
use crate::db::{CoverageRow, ModuleDetail, ModuleSummary, ProgramDetail};
use crate::slug::{ProgramOption, program_slug};

#[derive(Clone)]
pub struct HtmlRenderer {
    head_prefix: String,
    body_suffix: String,
}

fn format_persons(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('[') {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(arr) = val.as_array() {
                let names: Vec<String> = arr
                    .iter()
                    .filter_map(|item| {
                        if let Some(r) = item.get("raw").and_then(|v| v.as_str()) {
                            Some(r.to_string())
                        } else if let Some(n) = item.get("name").and_then(|v| v.as_str()) {
                            let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                            if title.is_empty() {
                                Some(n.to_string())
                            } else {
                                Some(format!("{title} {n}"))
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                if !names.is_empty() {
                    return names.join(", ");
                }
            }
        }
    }
    trimmed.to_string()
}

impl HtmlRenderer {
    pub fn new(dist_dir: &str) -> Self {
        let index_path = Path::new(dist_dir).join("index.html");
        let content = fs::read_to_string(&index_path).unwrap_or_else(|_| {
            // Safe fallback if dist/index.html is not yet built
            r#"<!DOCTYPE html>
<html lang="de">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>BTU Smart Modulkatalog</title>
  <link rel="stylesheet" href="/static/app.css">
  <link rel="manifest" href="/manifest.json">
</head>
<body>
  <div id="root"></div>
</body>
</html>"#
                .to_string()
        });

        // Split template around `<div id="root"></div>`
        let marker = "<div id=\"root\"></div>";
        let (head_prefix, body_suffix) = if let Some(idx) = content.find(marker) {
            let prefix = content[..idx].to_string();
            let suffix = content[idx + marker.len()..].to_string();
            (prefix, suffix)
        } else {
            (
                "<!DOCTYPE html><html lang=\"de\"><head><meta charset=\"UTF-8\"><title>BTU Katalog</title><link rel=\"stylesheet\" href=\"/static/app.css\"></head><body><div id=\"root\">".to_string(),
                "</div></body></html>".to_string(),
            )
        };

        Self {
            head_prefix,
            body_suffix,
        }
    }

    fn wrap_page(&self, title: &str, description: &str, static_content: &str) -> String {
        // Adjust title and meta description in head_prefix
        let mut head = self.head_prefix.clone();

        // Replace <title>...</title>
        if let Some(start) = head.find("<title>") {
            if let Some(end) = head[start..].find("</title>") {
                head.replace_range(start..start + end + 8, &format!("<title>{}</title>", encode_text(title)));
            }
        }

        // Replace description if present
        if let Some(start) = head.find("name=\"description\" content=\"") {
            let content_start = start + "name=\"description\" content=\"".len();
            if let Some(end) = head[content_start..].find('"') {
                head.replace_range(content_start..content_start + end, &encode_text(description));
            }
        }

        format!(
            "{}<div id=\"root\"><noscript><div style=\"background:#003b5c;color:#fff;padding:12px 20px;text-align:center;font-size:14px;font-family:sans-serif;\">ℹ️ JavaScript ist deaktiviert. Sie sehen die vollständige, suchmaschinenoptimierte statische Version des BTU Modulkatalogs.</div></noscript>{}</div>{}",
            head, static_content, self.body_suffix
        )
    }

    pub fn render_catalog(&self, modules: &[ModuleSummary], programs: &[ProgramOption]) -> String {
        let title = "BTU Smart Modulkatalog - Übersicht & Suche";
        let desc = format!(
            "Offizieller Modulkatalog der BTU Cottbus-Senftenberg. {} Module und {} Studiengänge mit Voraussetzungen, Prüfungsformen und Studienplänen.",
            modules.len(),
            programs.len()
        );

        let mut body = String::with_capacity(128 * 1024);
        body.push_str(
            "<div class=\"static-container\" style=\"max-width:1200px;margin:0 auto;padding:24px 16px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1e293b;line-height:1.5;\">"
        );

        // Header / Hero
        body.push_str(
            "<header style=\"margin-bottom:32px;border-bottom:2px solid #e2e8f0;padding-bottom:20px;\">
                <div style=\"display:flex;justify-content:space-between;align-items:center;flex-wrap:wrap;gap:16px;\">
                    <div>
                        <h1 style=\"margin:0 0 8px 0;font-size:28px;color:#003b5c;font-weight:800;\">BTU Smart Modulkatalog</h1>
                        <p style=\"margin:0;color:#64748b;font-size:16px;\">Brandenburgische Technische Universität Cottbus-Senftenberg</p>
                    </div>
                    <div>
                        <a href=\"/programs\" style=\"display:inline-block;background:#003b5c;color:#ffffff;text-decoration:none;padding:10px 18px;border-radius:6px;font-weight:600;font-size:14px;\">🎓 Alle Studiengänge</a>
                    </div>
                </div>
            </header>"
        );

        // Study Programs Section
        body.push_str(
            "<section style=\"margin-bottom:40px;background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:20px;\">
                <h2 style=\"margin:0 0 16px 0;font-size:20px;color:#003b5c;\">🎓 Studiengänge & Prüfungsordnungen</h2>
                <div style=\"display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:12px;\">"
        );

        for p in programs.iter().take(24) {
            let slug = program_slug(&p.id, programs);
            let degree = p.degree.as_deref().unwrap_or("");
            let po = p.po_version.as_deref().unwrap_or("");
            body.push_str(&format!(
                "<a href=\"/study-programm/{}/plan\" style=\"display:block;background:#ffffff;border:1px solid #cbd5e1;border-radius:6px;padding:12px;text-decoration:none;color:#0f172a;transition:border-color 0.2s;\">
                    <strong style=\"display:block;color:#003b5c;font-size:15px;margin-bottom:4px;\">{}</strong>
                    <span style=\"font-size:12px;color:#64748b;\">{} &bull; {}</span>
                </a>",
                encode_text(&slug),
                encode_text(&p.program_name),
                encode_text(degree),
                encode_text(po)
            ));
        }

        if programs.len() > 24 {
            body.push_str(&format!(
                "<div style=\"grid-column:1/-1;text-align:center;padding-top:8px;\">
                    <a href=\"/programs\" style=\"color:#003b5c;font-weight:600;text-decoration:underline;\">&rarr; Alle {} Studiengänge anzeigen</a>
                </div>",
                programs.len()
            ));
        }
        body.push_str("</div></section>");

        // Modules Table / List
        body.push_str(&format!(
            "<section>
                <div style=\"display:flex;justify-content:space-between;align-items:baseline;margin-bottom:16px;\">
                    <h2 style=\"margin:0;font-size:22px;color:#003b5c;\">📚 Alle Module ({})</h2>
                    <span style=\"color:#64748b;font-size:14px;\">Statische Vollansicht für Suchmaschinen</span>
                </div>
                <div style=\"overflow-x:auto;background:#ffffff;border:1px solid #e2e8f0;border-radius:8px;\">
                    <table style=\"width:100%;border-collapse:collapse;text-align:left;font-size:14px;\">
                        <thead>
                            <tr style=\"background:#f1f5f9;border-bottom:2px solid #cbd5e1;color:#475569;\">
                                <th style=\"padding:12px 14px;white-space:nowrap;\">Code</th>
                                <th style=\"padding:12px 14px;\">Modulbezeichnung</th>
                                <th style=\"padding:12px 14px;white-space:nowrap;\">LP</th>
                                <th style=\"padding:12px 14px;white-space:nowrap;\">Turnus</th>
                                <th style=\"padding:12px 14px;\">Fachgebiet</th>
                                <th style=\"padding:12px 14px;white-space:nowrap;\">Aktion</th>
                            </tr>
                        </thead>
                        <tbody>",
            modules.len()
        ));

        for (i, m) in modules.iter().enumerate() {
            let bg = if i % 2 == 0 { "#ffffff" } else { "#f8fafc" };
            let credits = if m.credits > 0.0 {
                format!("{:.1}", m.credits).trim_end_matches(".0").to_string()
            } else {
                m.credits_raw.clone().unwrap_or_else(|| "-".to_string())
            };
            let turnus = m.turnus.as_deref().unwrap_or("-");
            let dept = m.department.as_deref().unwrap_or("-");

            body.push_str(&format!(
                "<tr style=\"background:{};border-bottom:1px solid #e2e8f0;\">
                    <td style=\"padding:10px 14px;font-family:monospace;font-weight:700;color:#003b5c;\">{}</td>
                    <td style=\"padding:10px 14px;\">
                        <a href=\"/catalog/module/{}\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">{}</a>",
                bg,
                encode_text(&m.code),
                encode_text(&m.id),
                encode_text(&m.title_de)
            ));

            if let Some(en) = &m.title_en {
                if !en.trim().is_empty() && en != &m.title_de {
                    body.push_str(&format!(
                        "<div style=\"font-size:12px;color:#64748b;margin-top:2px;\">{}</div>",
                        encode_text(en)
                    ));
                }
            }

            body.push_str(&format!(
                "</td>
                    <td style=\"padding:10px 14px;font-weight:600;\">{} LP</td>
                    <td style=\"padding:10px 14px;\">{}</td>
                    <td style=\"padding:10px 14px;color:#475569;\">{}</td>
                    <td style=\"padding:10px 14px;\">
                        <a href=\"/catalog/module/{}\" style=\"display:inline-block;padding:4px 10px;background:#e0f2fe;color:#0284c7;border-radius:4px;text-decoration:none;font-weight:600;font-size:12px;\">Details</a>
                    </td>
                </tr>",
                encode_text(&credits),
                encode_text(turnus),
                encode_text(dept),
                encode_text(&m.id)
            ));
        }

        body.push_str(
            "</tbody></table></div></section>
            <footer style=\"margin-top:40px;border-top:1px solid #e2e8f0;padding-top:20px;text-align:center;color:#94a3b8;font-size:13px;\">
                BTU Smart Modulkatalog &bull; Hybrid Progressive Web App mit statischem Server-Side Rendering
            </footer>
            </div>"
        );

        self.wrap_page(title, &desc, &body)
    }

    pub fn render_module(&self, m: &ModuleDetail) -> String {
        let title = format!("{} {} - BTU Modulkatalog", m.code, m.title_de);
        let desc = format!(
            "Modul {} ({}): {} LP, Fachgebiet: {}. Lehrformen, Voraussetzungen und zugeordnete Studiengänge an der BTU Cottbus-Senftenberg.",
            m.code,
            m.title_de,
            m.credits,
            m.department.as_deref().unwrap_or("BTU")
        );

        let mut body = String::with_capacity(32 * 1024);
        body.push_str(
            "<div class=\"static-container\" style=\"max-width:960px;margin:0 auto;padding:24px 16px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1e293b;line-height:1.6;\">"
        );

        // Breadcrumbs
        body.push_str(
            "<nav style=\"margin-bottom:20px;font-size:14px;color:#64748b;\">
                <a href=\"/catalog\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">&larr; Zurück zur Modulübersicht</a>
            </nav>"
        );

        // Module Header Card
        body.push_str(&format!(
            "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:28px;margin-bottom:24px;box-shadow:0 1px 3px rgba(0,0,0,0.05);\">
                <div style=\"display:flex;align-items:center;gap:12px;margin-bottom:8px;\">
                    <span style=\"background:#003b5c;color:#ffffff;padding:4px 10px;border-radius:4px;font-weight:700;font-size:14px;font-family:monospace;\">{}</span>
                    <span style=\"background:#e2e8f0;color:#334155;padding:4px 10px;border-radius:4px;font-weight:700;font-size:14px;\">{:.1} LP</span>",
            encode_text(&m.code),
            m.credits
        ));

        if let Some(t) = &m.turnus {
            body.push_str(&format!(
                "<span style=\"background:#f1f5f9;color:#475569;padding:4px 10px;border-radius:4px;font-size:13px;\">{}</span>",
                encode_text(t)
            ));
        }
        if let Some(lang) = &m.language {
            body.push_str(&format!(
                "<span style=\"background:#f1f5f9;color:#475569;padding:4px 10px;border-radius:4px;font-size:13px;\">Sprache: {}</span>",
                encode_text(lang)
            ));
        }

        body.push_str(&format!(
            "</div>
                <h1 style=\"margin:0 0 8px 0;font-size:26px;color:#003b5c;font-weight:800;\">{}</h1>",
            encode_text(&m.title_de)
        ));

        if let Some(en) = &m.title_en {
            if !en.trim().is_empty() && en != &m.title_de {
                body.push_str(&format!(
                    "<div style=\"font-size:16px;color:#64748b;margin-bottom:16px;\">{}</div>",
                    encode_text(en)
                ));
            }
        }

        // Meta grid
        body.push_str("<div style=\"display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:16px;margin-top:20px;padding-top:16px;border-top:1px solid #e2e8f0;font-size:14px;\">");
        if let Some(dept) = &m.department {
            body.push_str(&format!("<div><strong style=\"color:#64748b;display:block;\">Fachgebiet:</strong>{}</div>", encode_text(dept)));
        }
        if let Some(resp) = &m.responsible_persons {
            let clean_resp = format_persons(resp);
            body.push_str(&format!("<div><strong style=\"color:#64748b;display:block;\">Modulverantwortliche:</strong>{}</div>", encode_text(&clean_resp)));
        }
        if let Some(exam) = &m.exam_type {
            body.push_str(&format!("<div><strong style=\"color:#64748b;display:block;\">Prüfungsform:</strong>{}</div>", encode_text(exam)));
        }
        if let Some(dur) = &m.duration {
            body.push_str(&format!("<div><strong style=\"color:#64748b;display:block;\">Dauer:</strong>{}</div>", encode_text(dur)));
        }
        body.push_str("</div></div>");

        // Prerequisites
        if m.prerequisites_mandatory.as_deref().unwrap_or("").trim().len() > 0
            || m.prerequisites_recommended.as_deref().unwrap_or("").trim().len() > 0
        {
            body.push_str(
                "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:24px;margin-bottom:24px;\">
                    <h2 style=\"margin:0 0 16px 0;font-size:18px;color:#003b5c;\">⚠️ Voraussetzungen</h2>"
            );
            if let Some(mand) = &m.prerequisites_mandatory {
                if !mand.trim().is_empty() {
                    body.push_str(&format!(
                        "<div style=\"margin-bottom:12px;\"><strong style=\"color:#dc2626;display:block;\">Verpflichtend erforderlich:</strong><p style=\"margin:4px 0 0 0;\">{}</p></div>",
                        encode_text(mand)
                    ));
                }
            }
            if let Some(rec) = &m.prerequisites_recommended {
                if !rec.trim().is_empty() {
                    body.push_str(&format!(
                        "<div><strong style=\"color:#d97706;display:block;\">Empfohlen:</strong><p style=\"margin:4px 0 0 0;\">{}</p></div>",
                        encode_text(rec)
                    ));
                }
            }
            body.push_str("</div>");
        }

        // Learning Outcomes
        if let Some(lo) = &m.learning_outcomes {
            if !lo.trim().is_empty() {
                body.push_str(&format!(
                    "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:24px;margin-bottom:24px;\">
                        <h2 style=\"margin:0 0 12px 0;font-size:18px;color:#003b5c;\">🎯 Lernergebnisse &amp; Qualifikationsziele</h2>
                        <div style=\"white-space:pre-wrap;color:#334155;\">{}</div>
                    </div>",
                    encode_text(lo)
                ));
            }
        }

        // Contents
        if let Some(c) = &m.contents {
            if !c.trim().is_empty() {
                body.push_str(&format!(
                    "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:24px;margin-bottom:24px;\">
                        <h2 style=\"margin:0 0 12px 0;font-size:18px;color:#003b5c;\">📖 Lehrinhalte</h2>
                        <div style=\"white-space:pre-wrap;color:#334155;\">{}</div>
                    </div>",
                    encode_text(c)
                ));
            }
        }

        // Teaching Forms
        if let Some(tf) = &m.teaching_forms {
            if !tf.trim().is_empty() {
                body.push_str(&format!(
                    "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:24px;margin-bottom:24px;\">
                        <h2 style=\"margin:0 0 12px 0;font-size:18px;color:#003b5c;\">👥 Lehrformen &amp; Arbeitsaufwand</h2>
                        <div style=\"white-space:pre-wrap;color:#334155;\">{}</div>
                    </div>",
                    encode_text(tf)
                ));
            }
        }

        // Literature
        if let Some(lit) = &m.literature {
            if !lit.trim().is_empty() {
                body.push_str(&format!(
                    "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:24px;margin-bottom:24px;\">
                        <h2 style=\"margin:0 0 12px 0;font-size:18px;color:#003b5c;\">📚 Literatur &amp; Medien</h2>
                        <div style=\"white-space:pre-wrap;color:#334155;\">{}</div>
                    </div>",
                    encode_text(lit)
                ));
            }
        }

        // Linked Study Programs
        if !m.linked_programs.is_empty() {
            body.push_str(
                "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:24px;margin-bottom:24px;\">
                    <h2 style=\"margin:0 0 16px 0;font-size:18px;color:#003b5c;\">🎓 Zuordnung zu Studiengängen</h2>
                    <div style=\"display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:10px;\">"
            );
            for p in &m.linked_programs {
                body.push_str(&format!(
                    "<a href=\"/study-programm/{}/plan\" style=\"display:block;padding:10px;background:#f8fafc;border:1px solid #e2e8f0;border-radius:6px;text-decoration:none;color:#0f172a;\">
                        <strong style=\"color:#0369a1;display:block;font-size:14px;\">{}</strong>
                        <span style=\"font-size:12px;color:#64748b;\">{} &bull; {}</span>
                    </a>",
                    encode_text(&p.slug),
                    encode_text(&p.program_name),
                    encode_text(p.degree.as_deref().unwrap_or("")),
                    encode_text(p.po_version.as_deref().unwrap_or(""))
                ));
            }
            body.push_str("</div></div>");
        }

        body.push_str(
            "<div style=\"text-align:center;margin-top:32px;\">
                <a href=\"/catalog\" style=\"display:inline-block;padding:10px 24px;background:#003b5c;color:#ffffff;text-decoration:none;border-radius:6px;font-weight:600;\">&larr; Zurück zum Modulkatalog</a>
            </div>
            </div>"
        );

        self.wrap_page(&title, &desc, &body)
    }

    pub fn render_program(&self, p: &ProgramDetail, tab: &str) -> String {
        let tab_name = match tab {
            "electives" => "Wahlpflicht",
            "modules" => "Alle Module",
            _ => "Studienplan",
        };
        let title = format!("{} ({}) - {} - BTU Modulkatalog", p.program_name, p.degree.as_deref().unwrap_or(""), tab_name);
        let desc = format!(
            "Offizieller Studienplan und Modulliste für {} ({}, PO {}). Pflichtmodule, Wahlpflicht und Semesterzuordnung.",
            p.program_name,
            p.degree.as_deref().unwrap_or(""),
            p.po_version.as_deref().unwrap_or("")
        );

        let mut body = String::with_capacity(48 * 1024);
        body.push_str(
            "<div class=\"static-container\" style=\"max-width:1100px;margin:0 auto;padding:24px 16px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1e293b;line-height:1.5;\">"
        );

        // Breadcrumbs
        body.push_str(
            "<nav style=\"margin-bottom:20px;font-size:14px;color:#64748b;\">
                <a href=\"/catalog\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">&larr; Katalog</a> &bull;
                <a href=\"/programs\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">Studiengänge</a>
            </nav>"
        );

        // Program Header
        body.push_str(&format!(
            "<div style=\"background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:24px;margin-bottom:24px;\">
                <h1 style=\"margin:0 0 8px 0;font-size:26px;color:#003b5c;font-weight:800;\">{}</h1>
                <div style=\"font-size:15px;color:#64748b;\">
                    <span>{}</span> &bull; <span>Prüfungsordnung: {}</span>
                </div>
            </div>",
            encode_text(&p.program_name),
            encode_text(p.degree.as_deref().unwrap_or("")),
            encode_text(p.po_version.as_deref().unwrap_or(""))
        ));

        // Navigation Tabs
        let plan_active = if tab == "plan" || tab.is_empty() { "background:#003b5c;color:#ffffff;" } else { "background:#f1f5f9;color:#334155;" };
        let elec_active = if tab == "electives" { "background:#003b5c;color:#ffffff;" } else { "background:#f1f5f9;color:#334155;" };
        let mods_active = if tab == "modules" { "background:#003b5c;color:#ffffff;" } else { "background:#f1f5f9;color:#334155;" };

        body.push_str(&format!(
            "<div style=\"display:flex;gap:8px;margin-bottom:24px;\">
                <a href=\"/study-programm/{}/plan\" style=\"padding:10px 18px;border-radius:6px;text-decoration:none;font-weight:600;font-size:14px;{}\">📋 Studienplan</a>
                <a href=\"/study-programm/{}/electives\" style=\"padding:10px 18px;border-radius:6px;text-decoration:none;font-weight:600;font-size:14px;{}\">🎯 Wahlpflicht ({})</a>
                <a href=\"/study-programm/{}/modules\" style=\"padding:10px 18px;border-radius:6px;text-decoration:none;font-weight:600;font-size:14px;{}\">📚 Alle Module ({})</a>
            </div>",
            encode_text(&p.slug), plan_active,
            encode_text(&p.slug), elec_active, p.electives.len(),
            encode_text(&p.slug), mods_active, p.all_modules.len()
        ));

        if tab == "electives" {
            // Electives View
            body.push_str(
                "<div style=\"background:#ffffff;border:1px solid #e2e8f0;border-radius:8px;padding:20px;\">
                    <h2 style=\"margin:0 0 16px 0;font-size:20px;color:#003b5c;\">Wahlpflichtmodule</h2>"
            );
            if p.electives.is_empty() {
                body.push_str("<p style=\"color:#64748b;\">Keine gesonderten Wahlpflichtmodule hinterlegt.</p>");
            } else {
                body.push_str("<div style=\"display:grid;gap:8px;\">");
                for m in &p.electives {
                    body.push_str(&format!(
                        "<div style=\"display:flex;justify-content:space-between;align-items:center;padding:12px;background:#f8fafc;border:1px solid #e2e8f0;border-radius:6px;\">
                            <div>
                                <strong style=\"color:#003b5c;margin-right:8px;\">{}</strong>
                                <a href=\"/catalog/module/{}\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">{}</a>
                            </div>
                            <span style=\"font-weight:700;font-size:13px;background:#e2e8f0;padding:3px 8px;border-radius:4px;\">{:.1} LP</span>
                        </div>",
                        encode_text(&m.code),
                        encode_text(&m.id),
                        encode_text(&m.title_de),
                        m.credits
                    ));
                }
                body.push_str("</div>");
            }
            body.push_str("</div>");
        } else if tab == "modules" {
            // All Modules View
            body.push_str(
                "<div style=\"background:#ffffff;border:1px solid #e2e8f0;border-radius:8px;padding:20px;\">
                    <h2 style=\"margin:0 0 16px 0;font-size:20px;color:#003b5c;\">Alle zugeordneten Module</h2>
                    <div style=\"display:grid;gap:8px;\">"
            );
            for m in &p.all_modules {
                body.push_str(&format!(
                    "<div style=\"display:flex;justify-content:space-between;align-items:center;padding:12px;background:#f8fafc;border:1px solid #e2e8f0;border-radius:6px;\">
                        <div>
                            <span style=\"font-family:monospace;font-weight:700;color:#003b5c;margin-right:8px;\">{}</span>
                            <a href=\"/catalog/module/{}\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">{}</a>
                        </div>
                        <span style=\"font-weight:700;font-size:13px;background:#e2e8f0;padding:3px 8px;border-radius:4px;\">{:.1} LP</span>
                    </div>",
                    encode_text(&m.code),
                    encode_text(&m.id),
                    encode_text(&m.title_de),
                    m.credits
                ));
            }
            body.push_str("</div></div>");
        } else {
            // Plan View: Group by semester
            body.push_str("<div style=\"background:#ffffff;border:1px solid #e2e8f0;border-radius:8px;padding:20px;\">");
            if p.curriculum.is_empty() {
                body.push_str("<p style=\"color:#64748b;\">Für diesen Studiengang liegt noch kein validierter digitaler Semesterplan vor. Bitte nutzen Sie den Reiter 'Alle Module'.</p>");
            } else {
                let max_sem = p.curriculum.iter().map(|c| c.semester).max().unwrap_or(0);
                for sem in 1..=max_sem {
                    let sem_entries: Vec<_> = p.curriculum.iter().filter(|c| c.semester == sem).collect();
                    if sem_entries.is_empty() {
                        continue;
                    }
                    let sem_credits: f64 = sem_entries.iter().map(|c| c.credits).sum();
                    body.push_str(&format!(
                        "<div style=\"margin-bottom:28px;\">
                            <div style=\"display:flex;justify-content:space-between;align-items:center;border-bottom:2px solid #003b5c;padding-bottom:6px;margin-bottom:12px;\">
                                <h3 style=\"margin:0;font-size:18px;color:#003b5c;\">{} . Fachsemester</h3>
                                <span style=\"font-weight:700;color:#003b5c;font-size:14px;\">{:.1} LP</span>
                            </div>
                            <div style=\"display:grid;gap:8px;\">",
                        sem, sem_credits
                    ));

                    for entry in sem_entries {
                        body.push_str(&format!(
                            "<div style=\"display:flex;justify-content:space-between;align-items:center;padding:12px;background:#f8fafc;border:1px solid #e2e8f0;border-radius:6px;\">
                                <div>
                                    <span style=\"font-family:monospace;font-weight:700;color:#003b5c;margin-right:8px;\">{}</span>
                                    <a href=\"/catalog/module/{}\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">{}</a>
                                    <span style=\"margin-left:8px;font-size:12px;color:#64748b;\">({})</span>
                                </div>
                                <span style=\"font-weight:700;font-size:13px;background:#e2e8f0;padding:3px 8px;border-radius:4px;\">{:.1} LP</span>
                            </div>",
                            encode_text(&entry.module_code),
                            encode_text(&entry.module_id),
                            encode_text(&entry.module_title),
                            encode_text(&entry.module_type),
                            entry.credits
                        ));
                    }
                    body.push_str("</div></div>");
                }
            }
            body.push_str("</div>");
        }

        body.push_str("</div>");
        self.wrap_page(&title, &desc, &body)
    }

    pub fn render_programs_list(&self, programs: &[ProgramOption]) -> String {
        let title = "Alle Studiengänge der BTU Cottbus-Senftenberg";
        let desc = format!("Übersicht aller {} Studiengänge und Prüfungsordnungen der BTU Cottbus-Senftenberg mit Studienplänen und Modulen.", programs.len());

        let mut body = String::with_capacity(32 * 1024);
        body.push_str(
            "<div class=\"static-container\" style=\"max-width:1100px;margin:0 auto;padding:24px 16px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;color:#1e293b;line-height:1.5;\">
                <nav style=\"margin-bottom:20px;font-size:14px;\">
                    <a href=\"/catalog\" style=\"color:#0369a1;text-decoration:none;font-weight:600;\">&larr; Zurück zum Modulkatalog</a>
                </nav>
                <header style=\"margin-bottom:24px;\">
                    <h1 style=\"margin:0 0 8px 0;font-size:28px;color:#003b5c;font-weight:800;\">🎓 Alle Studiengänge</h1>
                    <p style=\"margin:0;color:#64748b;font-size:16px;\">Wählen Sie einen Studiengang für den vollständigen Studienplan und alle zugeordneten Module.</p>
                </header>
                <div style=\"display:grid;grid-template-columns:repeat(auto-fill,minmax(320px,1fr));gap:16px;\">"
        );

        for p in programs {
            let slug = program_slug(&p.id, programs);
            body.push_str(&format!(
                "<a href=\"/study-programm/{}/plan\" style=\"display:block;background:#ffffff;border:1px solid #cbd5e1;border-radius:8px;padding:16px;text-decoration:none;color:#0f172a;transition:transform 0.1s,box-shadow 0.1s;box-shadow:0 1px 2px rgba(0,0,0,0.05);\">
                    <strong style=\"display:block;color:#003b5c;font-size:16px;margin-bottom:6px;\">{}</strong>
                    <div style=\"font-size:13px;color:#64748b;\">
                        <span style=\"display:inline-block;background:#f1f5f9;padding:2px 6px;border-radius:4px;margin-right:6px;\">{}</span>
                        <span>PO: {}</span>
                    </div>
                </a>",
                encode_text(&slug),
                encode_text(&p.program_name),
                encode_text(p.degree.as_deref().unwrap_or("")),
                encode_text(p.po_version.as_deref().unwrap_or(""))
            ));
        }

        body.push_str("</div></div>");
        self.wrap_page(title, &desc, &body)
    }

    pub fn render_not_found(&self) -> String {
        let body = "<div style=\"max-width:600px;margin:80px auto;text-align:center;font-family:sans-serif;\">
            <h1 style=\"font-size:48px;color:#003b5c;margin:0 0 16px 0;\">404</h1>
            <p style=\"font-size:18px;color:#64748b;margin-bottom:24px;\">Die gesuchte Seite oder das Modul wurde nicht gefunden.</p>
            <a href=\"/catalog\" style=\"display:inline-block;padding:12px 24px;background:#003b5c;color:#ffffff;text-decoration:none;border-radius:6px;font-weight:600;\">Zurück zum Modulkatalog</a>
        </div>";
        self.wrap_page("404 Nicht gefunden - BTU Modulkatalog", "Seite nicht gefunden", body)
    }
}

const COVERAGE_CSS: &str = "\
*{box-sizing:border-box}body{margin:0;background:#f8fafc;color:#0f172a;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;line-height:1.5}\
.wrap{max-width:1180px;margin:0 auto;padding:24px 16px 64px}a{color:#0369a1;text-decoration:none}a:hover{text-decoration:underline}\
h1{margin:8px 0 6px;font-size:28px;color:#003b5c}p.lead{margin:0 0 20px;color:#475569}\
.stats{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px;margin-bottom:20px}\
.stat{background:#fff;border:1px solid #cbd5e1;border-radius:8px;padding:12px 14px}.stat b{display:block;font-size:26px;color:#003b5c}.stat span{font-size:13px;color:#475569}\
.tools{display:flex;flex-wrap:wrap;gap:8px;align-items:center;margin-bottom:12px}\
.tools input{flex:1 1 260px;padding:9px 12px;border:1px solid #94a3b8;border-radius:6px;font-size:15px}\
.tools button{padding:8px 12px;border:1px solid #94a3b8;background:#fff;border-radius:6px;cursor:pointer;font-size:14px}.tools button.on{background:#003b5c;color:#fff;border-color:#003b5c}\
table{width:100%;border-collapse:collapse;background:#fff;border:1px solid #cbd5e1;border-radius:8px;overflow:hidden}\
th,td{padding:10px 12px;text-align:left;vertical-align:top;border-bottom:1px solid #e2e8f0;font-size:14px}th{background:#f1f5f9;color:#334155;font-size:13px}\
.badge{display:inline-block;padding:2px 9px;border-radius:999px;font-size:12px;font-weight:600;white-space:nowrap}\
.plan{background:#dcfce7;color:#166534}.modules{background:#fef3c7;color:#92400e}.none{background:#e2e8f0;color:#475569}\
.sub{display:block;margin-top:2px;color:#64748b;font-size:12px}.legend{margin:16px 0;font-size:13px;color:#475569}\
@media(max-width:720px){th:nth-child(3),td:nth-child(3){display:none}}";

const COVERAGE_JS: &str = "\
(function(){var q=document.getElementById('q'),rows=[].slice.call(document.querySelectorAll('tbody tr')),btns=[].slice.call(document.querySelectorAll('[data-level]')),lvl='all';\
function apply(){var t=q.value.toLowerCase().trim();rows.forEach(function(r){var ok=(lvl==='all'||r.getAttribute('data-level')===lvl)&&(!t||r.getAttribute('data-text').indexOf(t)>=0);r.style.display=ok?'':'none';});}\
q.addEventListener('input',apply);btns.forEach(function(b){b.addEventListener('click',function(){lvl=b.getAttribute('data-level');btns.forEach(function(x){x.className=x===b?'on':'';});apply();});});})();";

/// Standalone overview of all study programs and how much is known about each.
/// It is deliberately independent of the SPA shell so it works without JavaScript.
pub fn render_coverage_page(rows: &[CoverageRow], programs: &[ProgramOption]) -> String {
    let total = rows.len();
    let plan = rows.iter().filter(|r| r.level == "plan").count();
    let modules = rows.iter().filter(|r| r.level == "modules").count();
    let none = total - plan - modules;

    let mut body = String::with_capacity(64 * 1024);
    body.push_str(&format!(
        "<!DOCTYPE html><html lang=\"de\"><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\
         <title>Studiengänge und Datenlage - BTU Modulkatalog</title>\
         <meta name=\"description\" content=\"Alle Studiengänge der BTU Cottbus-Senftenberg und wie vollständig ihre Studienpläne im Modulkatalog vorliegen.\">\
         <link rel=\"icon\" type=\"image/svg+xml\" href=\"/static/icon-192.svg\"><style>{}</style></head><body><div class=\"wrap\">\
         <nav><a href=\"/catalog\">&larr; Zurück zum Modulkatalog</a></nav>\
         <h1>Alle Studiengänge</h1>\
         <p class=\"lead\">Wie viel der Prüfungsordnung ist für den jeweiligen Studiengang im Modulkatalog verfügbar?</p>\
         <div class=\"stats\">\
         <div class=\"stat\"><b>{}</b><span>Studiengänge (Prüfungsordnungs-Versionen)</span></div>\
         <div class=\"stat\"><b>{}</b><span>Prüfungsordnung vollständig geparst</span></div>\
         <div class=\"stat\"><b>{}</b><span>Nur verknüpfte Module</span></div>\
         <div class=\"stat\"><b>{}</b><span>Keine Daten</span></div></div>\
         <div class=\"tools\"><input id=\"q\" type=\"search\" placeholder=\"Studiengang, Abschluss oder PO suchen ...\">\
         <button data-level=\"all\" class=\"on\">Alle</button><button data-level=\"plan\">Studienplan</button>\
         <button data-level=\"modules\">Nur Module</button><button data-level=\"none\">Keine Daten</button></div>\
         <table><thead><tr><th>Studiengang</th><th>Informationsgrad</th><th>Abschluss / PO</th><th>Details</th></tr></thead><tbody>",
        COVERAGE_CSS, total, plan, modules, none
    ));

    for r in rows {
        let slug = program_slug(&r.program_id, programs);
        let (badge_class, badge_text) = match r.level.as_str() {
            "plan" => ("plan", "Prüfungsordnung geparst"),
            "modules" => ("modules", "Nur verknüpfte Module"),
            _ => ("none", "Keine Daten"),
        };
        let mut detail = String::new();
        match r.level.as_str() {
            "plan" => {
                detail.push_str(&format!(
                    "{} Anforderungen im Studienplan, davon {} mit Katalogmodul verknüpft",
                    r.plan_requirements, r.plan_linked
                ));
                if r.plan_semesters > 0 {
                    detail.push_str(&format!(", {} Semester", r.plan_semesters));
                }
                if r.scan_status == "saved_with_warnings" {
                    detail.push_str("<span class=\"sub\">Mit Hinweisen aus der Prüfung; Details im Scan-Bericht.</span>");
                }
            }
            "modules" => {
                detail.push_str(&format!("{} Module dem Studiengang zugeordnet", r.linked_modules));
                let reason = match r.scan_status.as_str() {
                    "no_plan" => "Die Prüfungsordnung enthält keine Studienplan-Tabelle (z. B. eingestellter Studiengang).",
                    "missing_source" => "Keine Prüfungsordnung heruntergeladen.",
                    "needs_review" => "Der Studienplan konnte noch nicht sicher ausgelesen werden.",
                    _ => "Der Studienplan wurde noch nicht ausgewertet.",
                };
                detail.push_str(&format!("<span class=\"sub\">{}</span>", reason));
            }
            _ => detail.push_str("Bisher weder Studienplan noch zugeordnete Module."),
        }
        let text = format!("{} {} {}", r.program_name, r.degree, r.po_version).to_lowercase();
        body.push_str(&format!(
            "<tr data-level=\"{}\" data-text=\"{}\"><td><a href=\"/study-programm/{}/plan\"><strong>{}</strong></a></td>\
             <td><span class=\"badge {}\">{}</span></td><td>{}<span class=\"sub\">PO {}</span></td><td>{}</td></tr>",
            r.level,
            encode_text(&text),
            encode_text(&slug),
            encode_text(&r.program_name),
            badge_class,
            badge_text,
            encode_text(&r.degree),
            encode_text(&r.po_version),
            detail
        ));
    }

    body.push_str(&format!(
        "</tbody></table>\
         <p class=\"legend\"><b>Prüfungsordnung geparst:</b> der komplette Regelstudienplan wurde aus der Prüfungsordnung gelesen und gegen die dort gedruckten Semestersummen geprüft. \
         <b>Nur verknüpfte Module:</b> Module sind dem Studiengang zugeordnet, ein Semesterplan liegt nicht vor. \
         <b>Keine Daten:</b> weder Plan noch Module.</p></div><script>{}</script></body></html>",
        COVERAGE_JS
    ));
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_persons_json() {
        let json_str = r#"[{"title":"Prof. Dr. rer. nat. habil.","name":"Köhler, Ekkehard","raw":"Prof. Dr. rer. nat. habil. Köhler, Ekkehard"}]"#;
        assert_eq!(format_persons(json_str), "Prof. Dr. rer. nat. habil. Köhler, Ekkehard");

        let multiple = r#"[{"name":"Müller, Hans"},{"name":"Schmidt, Peter"}]"#;
        assert_eq!(format_persons(multiple), "Müller, Hans, Schmidt, Peter");
    }

    #[test]
    fn test_format_persons_plain() {
        assert_eq!(format_persons("Prof. Dr. Mustermann"), "Prof. Dr. Mustermann");
    }

    #[test]
    fn test_html_renderer_fallback() {
        let renderer = HtmlRenderer::new("non_existent_dir");
        let html = renderer.render_not_found();
        assert!(html.contains("404"));
        assert!(html.contains("<div id=\"root\">"));
        assert!(html.contains("</title>"));
    }

    #[test]
    fn test_coverage_page_levels() {
        let row = |id: &str, level: &str| CoverageRow {
            program_id: id.to_string(),
            program_name: format!("Programm {}", id),
            degree: "Bachelor".to_string(),
            po_version: "2020".to_string(),
            level: level.to_string(),
            linked_modules: 3,
            plan_requirements: 10,
            plan_linked: 8,
            plan_semesters: 6,
            scan_status: if level == "modules" { "no_plan".to_string() } else { String::new() },
            scan_message: String::new(),
        };
        let rows = vec![row("a", "plan"), row("b", "modules"), row("c", "none")];
        let html = render_coverage_page(&rows, &[]);
        assert!(html.contains("Prüfungsordnung geparst"));
        assert!(html.contains("Nur verknüpfte Module"));
        assert!(html.contains("10 Anforderungen im Studienplan, davon 8 mit Katalogmodul verknüpft, 6 Semester"));
        assert!(html.contains("keine Studienplan-Tabelle"));
        assert!(html.contains("data-level=\"none\""));
    }
}
