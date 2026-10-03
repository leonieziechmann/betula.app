// Crawl of the server-rendered site, without a browser: what a search engine or a visitor
// without JavaScript gets. Usage:
//
//   CRAWL_BASE_URL=http://127.0.0.1:8080 node crawl.mjs          (every program: plan, areas, my-plan; 300 modules)
//   CRAWL_MODULES=all node crawl.mjs                              (every module of the catalog)
//
// Fails (exit 1) on any status other than 200, a page without <title> or <h1>, an error
// state in the HTML, a catalog whose pages do not add up to its total, a view of a program
// hidden from search engines or offered to them against `ProgramTab::indexed`, or a wrong 404.
const base = (process.env.CRAWL_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const moduleLimit = process.env.CRAWL_MODULES === "all" ? Infinity : Number(process.env.CRAWL_MODULES || 300);

const failures = [];
let pages = 0;
let slowest = { ms: 0, path: "" };

async function get(path, expectStatus = 200) {
  const started = performance.now();
  const response = await fetch(base + path, { redirect: "manual" });
  const html = await response.text();
  const ms = performance.now() - started;
  pages++;
  if (ms > slowest.ms) slowest = { ms: Math.round(ms), path };
  if (response.status !== expectStatus) failures.push(`${path}: HTTP ${response.status}, expected ${expectStatus}`);
  if (!/<title>[^<]+<\/title>/.test(html)) failures.push(`${path}: no <title>`);
  if (!/<h1[\s>]/.test(html)) failures.push(`${path}: no <h1>`);
  if (expectStatus === 200 && html.includes("state-error")) failures.push(`${path}: renders an error state`);
  return html;
}

const links = (html, pattern) => [...new Set([...html.matchAll(pattern)].map((match) => match[1].replaceAll("&amp;", "&")))];

// Landing page and the two overviews.
await get("/");
const overview = await get("/programs");
const programs = links(overview, /href="(\/programs\/[^"/]+)\/plan"/g);
if (programs.length < 50) failures.push(`/programs lists only ${programs.length} programs`);

// Every program in every view its sidebar links (`ProgramTab` in folia/crates/routes/src/url.rs): the plan
// and the areas are for search engines, „Mein Plan" is the visitor's and says `noindex`.
const views = { plan: true, areas: true, "my-plan": false };
for (const program of programs) {
  for (const [tab, indexed] of Object.entries(views)) {
    const html = await get(`${program}/${tab}`);
    if (!html.includes('data-walk="program-page"')) failures.push(`${program}/${tab}: not a program page`);
    if (html.includes('content="noindex') === indexed) failures.push(`${program}/${tab}: ${indexed ? "hidden from" : "offered to"} search engines`);
    const linked = links(html, /data-walk="tab" href="\/programs\/[^"/]+\/([^"/?]+)"/g).join();
    if (linked !== Object.keys(views).join()) failures.push(`${program}/${tab}: links the views ${linked || "(none)"}, the crawl knows ${Object.keys(views).join()}`);
  }
}

// The catalog, page by page: the pages must add up to the exact total in the header.
const modules = new Set();
let next = "/catalog";
let total = null;
let catalogPages = 0;
while (next) {
  const html = await get(next);
  catalogPages++;
  const header = html.match(/class="count num"[^>]*>(?:<!>)?([\d.]+)</);
  if (total === null) total = header ? Number(header[1].replaceAll(".", "")) : NaN;
  // Every row of the server's list leads to the module's own page (nothing stands beside the list).
  for (const id of links(html, /<a href="\/catalog\/module\/([A-Za-z0-9_-]+)"[^>]*class="row"/g)) modules.add("/catalog/module/" + id);
  next = links(html, /rel="next" href="([^"]+)"/g)[0] || links(html, /href="([^"]+)" rel="next"/g)[0] || null;
}
if (!(total > 0) || modules.size !== total) failures.push(`/catalog: header says ${total} modules, its ${catalogPages} pages list ${modules.size}`);

// Module pages: evenly spread over the catalog.
const all = [...modules];
const step = Math.max(1, Math.floor(all.length / Math.min(moduleLimit, all.length)));
let modulePages = 0;
for (let i = 0; i < all.length && modulePages < moduleLimit; i += step) {
  await get(all[i]);
  modulePages++;
}

// The legal pages, a filter, a program scope with its FÜS list, and what must be a 404.
await get("/impressum");
await get("/datenschutz");
// The Studienplan lives in the browser: the server's page is one explanation, for no index.
const plan = await get("/studyplan");
if (!plan.includes('content="noindex')) failures.push("/studyplan: offered to search engines");
await get("/catalog?turnus=winter&form=exercise&lang=en");
if (programs[0]) await get(`/catalog?program=${programs[0].split("/").pop()}&list=fues`);
await get("/catalog/module/00000", 404);
await get("/programs/no-such-program", 404);
if (programs[0]) await get(`${programs[0]}/no-such-tab`, 404);
// „Alle Module" gave way to „Mein Plan" (2026-09-25; the program's modules are its catalog).
if (programs[0]) await get(`${programs[0]}/modules`, 404);
await get("/no-such-page", 404);

const summary = { base, pages, programs: programs.length, catalogTotal: total, catalogPages, modulePages, slowest, failures: failures.length };
console.log(JSON.stringify(summary, null, 2));
if (failures.length) {
  for (const failure of failures.slice(0, 40)) console.error("FAIL " + failure);
  process.exit(1);
}
console.log("crawl ok");
