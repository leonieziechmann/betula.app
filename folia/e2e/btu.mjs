// Checks the BTU's pages that „Ordnungen & Dokumente" of „Mein Studium" links for every program
// (folia/crates/planner/src/study/btu.rs, written by hand): its lists of regulations and forms and
// its page „Studienorganisation", German and English.
//   node btu.mjs            (no browser, no Betula: the BTU's site itself, through curl)
// Every page answers 200, one request a second. The regulations and the pages of „Rund ums Studium"
// are not written there any more: Radix reads the BTU's pages of Grundsatzordnungen and
// Studienorganisation every night and the snapshot has them (v_general_document, v_study_page);
// `radix validate` warns when a page yields nothing.
// Exits 1 on any page that does not answer, saying which: then btu.rs and docs/folia/frontend.md
// follow the BTU's site.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const source = readFileSync(path.join(here, "../crates/planner/src/study/btu.rs"), "utf8");

const curl = (url, ...args) => execFileSync("curl", ["-sS", "-L", "--max-time", "40", "-A", "Mozilla/5.0 (Betula's check of its links)", ...args, url], { encoding: "utf8", maxBuffer: 1 << 26 });
const pause = () => execFileSync(process.execPath, ["-e", "setTimeout(() => {}, 1000)"]);
const problems = [];

// What btu.rs links: the BTU's pages by their paths, and any whole address besides its site's.
const literals = [...source.matchAll(/"(https?:\/\/[^"]+)"/g)].map((m) => m[1]).filter((url) => url !== "https://www.b-tu.de");
const pages = [...source.matchAll(/(?:de|en): "(\/[^"]+)"/g)].map((m) => "https://www.b-tu.de" + m[1]);
const ours = [...new Set([...literals, ...pages])];
if (pages.length < 8) problems.push(`btu.rs links only ${pages.length} pages of the BTU: the check no longer reads it`);

for (const url of ours) {
  const status = curl(url, "-o", "/dev/null", "-w", "%{http_code}").trim();
  if (status !== "200") problems.push(`${url} answers ${status}`);
  pause();
}

console.log(JSON.stringify({ links: ours.length, problems }, null, 2));
if (problems.length) process.exit(1);
