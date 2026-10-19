// Builds konzept.html from konzept.src.html: the catalog of a snapshot (the offered modules and the
// phase-out ones, what the catalog lists without filters), engine.mjs, and the table „Wo das Gemeinte
// landet“ computed with it.
//
//   node --no-warnings folia/design/suche/build.mjs <catalog.db>
//
// The snapshot is the one the browser loads (`/api/db` of canary or a local Folia). Node 22's
// built-in SQLite reads it; nothing to install.

import { readFileSync, writeFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildIndex, searchNew, searchOld } from "./engine.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const snapshot = process.argv[2];
if (!snapshot) {
  console.error("usage: node --no-warnings build.mjs <catalog.db>");
  process.exit(2);
}
const db = new DatabaseSync(snapshot, { readOnly: true });
const all = (sql) => db.prepare(sql).all();

// ---------------------------------------------------------------- the data
const STATUS = { active: 0, phase_out: 1, not_offered: 2 };
const programs = all(`SELECT p.id, p.slug, p.name, p.degree_display FROM v_program p WHERE p.is_latest_po = 1
  AND EXISTS (SELECT 1 FROM v_program_module pm WHERE pm.program_id = p.id AND pm.relation = 'curricular')
  ORDER BY p.name, p.degree_display`);
const programIndex = new Map(programs.map((p, i) => [p.id, i]));
const member = new Map();
for (const { program_id, module_id } of all("SELECT DISTINCT program_id, module_id FROM v_program_module WHERE relation = 'curricular'")) {
  if (!programIndex.has(program_id)) continue;
  if (!member.has(module_id)) member.set(module_id, new Set());
  member.get(module_id).add(programIndex.get(program_id));
}
const programAbbrevs = new Map();
for (const { module_id, abbrev } of all("SELECT DISTINCT module_id, abbrev FROM v_program_module WHERE abbrev IS NOT NULL")) {
  if (!programAbbrevs.has(module_id)) programAbbrevs.set(module_id, new Set());
  programAbbrevs.get(module_id).add(abbrev);
}
// the lecturers: a name each, responsible (0) or teaching (1)
const lecturerNames = [];
const lecturerIndex = new Map();
const taught = new Map();
for (const { module_id, name, role } of all("SELECT DISTINCT module_id, name, role FROM v_module_lecturer ORDER BY module_id, role DESC, name")) {
  if (!lecturerIndex.has(name)) { lecturerIndex.set(name, lecturerNames.length); lecturerNames.push(name); }
  if (!taught.has(module_id)) taught.set(module_id, new Map());
  const roles = taught.get(module_id);
  const r = role === "responsible" ? 0 : 1;
  const i = lecturerIndex.get(name);
  roles.set(i, Math.min(roles.get(i) ?? 1, r));
}
const modules = [];
for (const m of all(`SELECT m.id, m.title, m.title_de, m.title_en, m.offer_status, m.abbrev, f.abbrevs FROM v_module m
    JOIN v_module_folded f ON f.module_id = m.id WHERE m.offer_status != 'not_offered' ORDER BY m.id`)) {
  let other = "", shownEn = 0;
  if (m.title === m.title_de || m.title_de == null) {
    other = m.title_en && m.title_en !== m.title ? m.title_en : "";
    shownEn = m.title_de == null ? 1 : 0;
  } else {
    other = m.title_de && m.title_de !== m.title ? m.title_de : "";
    shownEn = 1;
  }
  const own = [...new Set([m.abbrev, ...(programAbbrevs.get(m.id) ?? [])].filter(Boolean))].sort();
  const progs = [...(member.get(m.id) ?? [])].sort((a, b) => a - b);
  const lecturers = [...(taught.get(m.id) ?? new Map())].sort((a, b) => a[1] - b[1] || a[0] - b[0]);
  modules.push([m.id, m.title, other, shownEn, STATUS[m.offer_status], m.abbrevs ?? "", own, progs.length, progs, lecturers]);
}
const meta = Object.fromEntries(all("SELECT key, value FROM v_meta").map((r) => [r.key, r.value]));
const data = {
  snapshot: { built_at: meta.built_at, current_semester: meta.current_semester, radix_version: meta.radix_version },
  programs: programs.map((p) => [p.slug, p.name, p.degree_display]),
  lecturers: lecturerNames,
  modules,
};

if (process.env.DATA_OUT) writeFileSync(process.env.DATA_OUT, JSON.stringify(data));

// ---------------------------------------------------------------- where what is meant lands
// A search, what a student most likely means by it, and how its title is recognised.
const CASES = [
  ["TI", "Theoretische Informatik", /^Theoretische Informatik$/],
  ["KI", "ein Modul zur Künstlichen Intelligenz", /Künstliche Intelligenz/],
  ["theo inf", "Theoretische Informatik", /^Theoretische Informatik$/],
  ["TheoInf", "Theoretische Informatik", /^Theoretische Informatik$/],
  ["LinA", "Lineare Algebra und analytische Geometrie", /^Lineare Algebra/],
  ["mathe 1", "Mathematik 1", /^Mathematik 1$/],
  ["Mathe", "ein Modul Mathematik", /^Mathematik/],
  ["Analysis", "Analysis I", /^Analysis I$/],
  ["Ana 1", "Analysis I", /^Analysis I$/],
  ["AuD", "Algorithmen und Datenstrukturen", /^Algorithmen und Datenstrukturen$/],
  ["DB", "Datenbanken", /^Datenbanken$/],
  ["BS", "Betriebssysteme I", /^Betriebssysteme I$/],
  ["SWT", "Softwaretechnik", /^Softwaretechnik$/],
  ["PP", "Programmierpraktikum", /^Programmierpraktikum$/],
  ["ML", "Machine Learning oder Maschinelles Lernen", /Machine Learning|Maschinelles Lernen/],
  ["machine learning", "Machine Learning oder Maschinelles Lernen", /^(Machine Learning|Maschinelles Lernen)/],
  ["TM", "Technische Mechanik 1", /^Technische Mechanik 1/],
  ["Mechanik", "Technische Mechanik", /^Technische Mechanik/],
  ["ET", "Elektrotechnik 1", /^Elektrotechnik 1$/],
  ["MfI", "Mathematik für Ingenieure", /Mathematik für Ingenieure|Mathematics of Engineering/],
  ["BWL", "ein Modul Betriebswirtschaftslehre", /Betriebswirtschaftslehre/],
  ["VWL", "ein Modul Volkswirtschaftslehre", /Volkswirtschaftslehre/],
  ["Physik", "Physik", /^Physik$/],
  ["Statistik", "Statistik", /^Statistik$/],
  ["Thermodynamik", "Technische Thermodynamik", /^Technische Thermodynamik$/],
  ["Regelungstechnik", "Regelungstechnik 1", /^Regelungstechnik 1$/],
  ["Strömung", "Strömungslehre", /^Strömungslehre$/],
  ["Projekt", "Projekt I", /^Projekt I$/],
  ["Masterarbeit", "Master-Arbeit", /^Master-Arbeit$/],
  ["algoritmen", "ein Modul zu Algorithmen", /Algorithmen/],
  ["Hofstedt", "ein Modul von Prof. Hofstedt", { lecturer: /Hofstedt/ }],
  ["Hoffstedt", "ein Modul von Prof. Hofstedt", { lecturer: /Hofstedt/ }],
  ["P. Hofstedt", "ein Modul von Prof. Hofstedt", { lecturer: /Hofstedt/ }],
  ["Doering", "ein Modul von Prof. Döring", { lecturer: /Döring/ }],
];
const index = buildIndex(data);
const meant = (m, re) => (re instanceof RegExp ? re.test(m.title) : m.lecturers.some((l) => re.lecturer.test(l.raw)));
const at = (rows, re) => { const i = rows.findIndex((r) => meant(r.m, re)); return i < 0 ? null : i + 1; };
const ranks = CASES.map(([query, meant, re]) => {
  const o = searchOld(index, query).rows, n = searchNew(index, query).rows;
  return [query, meant, at(o, re), o.length, at(n, re), n.length];
});
const missed = ranks.filter((r) => r[4] === null);
if (missed.length) throw new Error(`the new search misses what is meant: ${missed.map((r) => r[0]).join(", ")}`);

// ---------------------------------------------------------------- the page
const json = (value) => JSON.stringify(value).replace(/</g, "\\u003c");
const engine = readFileSync(join(here, "engine.mjs"), "utf8").replace(/^export /gm, "");
const page = readFileSync(join(here, "konzept.src.html"), "utf8")
  .replace("/*DATA*/", () => json(data))
  .replace("/*RANKS*/", () => json(ranks))
  .replace("/*ENGINE*/", () => engine);
writeFileSync(join(here, "konzept.html"), page);
console.log(`konzept.html: ${modules.length} modules, ${programs.length} programs, ${Math.round(page.length / 1024)} kB`);
for (const r of ranks) console.log(`  ${r[0].padEnd(18)} heute ${String(r[2] ?? "–").padStart(3)}/${String(r[3]).padEnd(4)} neu ${String(r[4]).padStart(2)}/${r[5]}`);
