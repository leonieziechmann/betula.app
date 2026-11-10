// Checks the BTU's documents and pages that „Ordnungen & Dokumente" of „Mein Studium" lists for every
// program (folia/crates/planner/src/study/btu.rs, written by hand: the data has none of them).
//   node btu.mjs            (no browser, no Betula: the BTU's site itself, through curl)
// 1 every page of the BTU's site, every English translation and the law answer 200, one request a
//   second; the documents on OPUS are not fetched (its bot check answers anything that is not a
//   browser), they are compared with the BTU's own list instead:
// 2 on the BTU's page of Grundsatzordnungen, the sections btu.rs takes documents from list nothing
//   btu.rs lacks (an amendment come anew), and every document of btu.rs is still listed there.
// Exits 1 on any difference, saying which: then btu.rs, the date in its panel's head
// (i18n/study_btu.rs, `statutes_lead`) and docs/folia/frontend.md follow the BTU's pages.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const source = readFileSync(path.join(here, "../crates/planner/src/study/btu.rs"), "utf8");
const LIST = "https://www.b-tu.de/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen";
// The sections of that page btu.rs takes its documents from, by their headings.
const SECTIONS = ["Rahmenordnung für Bachelor-Studiengänge", "Rahmenordnung für Master-Studiengänge", "Immatrikulationsordnung"];

const curl = (url, ...args) => execFileSync("curl", ["-sS", "-L", "--max-time", "40", "-A", "Mozilla/5.0 (Betula's check of its links)", ...args, url], { encoding: "utf8", maxBuffer: 1 << 26 });
const pause = () => execFileSync(process.execPath, ["-e", "setTimeout(() => {}, 1000)"]);
const problems = [];

// What btu.rs links: whole addresses, and the BTU's pages by their paths.
const literals = [...source.matchAll(/"(https?:\/\/[^"]+)"/g)].map((m) => m[1]);
const pages = [...source.matchAll(/(?:de|en): "(\/[^"]+)"/g)].map((m) => "https://www.b-tu.de" + m[1]);
const ours = [...new Set([...literals.filter((url) => url !== "https://www.b-tu.de"), ...pages])];
const opus = ours.filter((url) => /opus(4)?\.kobv\.de/.test(url));

// 1: what answers without a bot check.
for (const url of ours.filter((url) => !opus.includes(url))) {
  const status = curl(url, "-o", "/dev/null", "-w", "%{http_code}").trim();
  if (status !== "200") problems.push(`${url} answers ${status}`);
  pause();
}

// 2: the BTU's list.
const page = curl(LIST).replace(/<(script|style)[^>]*>[\s\S]*?<\/\1>/g, "");
const decode = (text) => text.replace(/&amp;/g, "&").replace(/&#39;/g, "'").replace(/&quot;/g, '"');
let section = "";
const listed = new Map();
for (const m of page.matchAll(/<h[1-6][^>]*>([\s\S]*?)<\/h[1-6]>|<a\s[^>]*href="([^"]+)"/g)) {
  if (m[1] !== undefined) section = decode(m[1].replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim());
  else listed.set(decode(m[2]), section);
}
if (listed.size < 20) problems.push(`the BTU's list of Grundsatzordnungen has ${listed.size} links: its page changed (${LIST})`);
for (const [url, heading] of listed) {
  if (SECTIONS.includes(heading) && /\.pdf$/i.test(url) && !ours.includes(url)) problems.push(`the BTU lists anew under „${heading}": ${url}`);
}
for (const url of opus) {
  if (!listed.has(url)) problems.push(`the BTU no longer lists ${url}`);
}

console.log(JSON.stringify({ links: ours.length, fetched: ours.length - opus.length, compared: opus.length, listed: listed.size, problems }, null, 2));
if (problems.length) process.exit(1);
