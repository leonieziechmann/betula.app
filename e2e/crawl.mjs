// Crawl of the server-rendered site, without a browser: what a search engine or a visitor
// without JavaScript gets. Usage:
//
//   CRAWL_BASE_URL=http://127.0.0.1:8080 node crawl.mjs          (every program × tab, 300 modules)
//   CRAWL_MODULES=all node crawl.mjs                              (every module of the catalog)
//
// Fails (exit 1) on any status other than 200, a page without <title> or <h1>, an error
// state in the HTML, a catalog whose pages do not add up to its total, or a wrong 404.
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

// Every program with every tab.
for (const program of programs) {
  for (const tab of ["plan", "areas", "modules"]) {
    const html = await get(`${program}/${tab}`);
    if (!html.includes('data-walk="program-page"')) failures.push(`${program}/${tab}: not a program page`);
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
  // Rows open a preview (`open=<id>`); the module's own page is /catalog/module/<id>.
  for (const id of links(html, /href="\/catalog\?[^"]*?open=([A-Za-z0-9_-]+)/g)) modules.add("/catalog/module/" + id);
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

// A filter, a program scope with its FÜS list, and what must be a 404.
await get("/catalog?turnus=winter&form=exercise&lang=en");
if (programs[0]) await get(`/catalog?program=${programs[0].split("/").pop()}&list=fues`);
await get("/catalog/module/00000", 404);
await get("/programs/no-such-program", 404);
if (programs[0]) await get(`${programs[0]}/no-such-tab`, 404);
await get("/no-such-page", 404);

const summary = { base, pages, programs: programs.length, catalogTotal: total, catalogPages, modulePages, slowest, failures: failures.length };
console.log(JSON.stringify(summary, null, 2));
if (failures.length) {
  for (const failure of failures.slice(0, 40)) console.error("FAIL " + failure);
  process.exit(1);
}
console.log("crawl ok");
