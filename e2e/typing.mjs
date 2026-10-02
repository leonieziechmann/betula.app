// Checks typing in the search of the top bar (docs/frontend.md, „The search of the catalog",
// „Typing"): once the catalog's search worker answers (`window.betulaSearch`), the page's own copy
// of the catalog runs none of a search's queries while the visitor types, the list follows what
// was typed, and „Ähnliche Module" hold none of its rows. And how long the keys wait: a word typed
// a key every 200 ms, as it is and with the CPU slowed down four times (a phone), the keys' wait
// for the page's thread (Event Timing) and its long tasks; without the slowdown no key may wait
// longer than KEY_WAIT_MS.
//   SMOKE_BASE_URL=http://127.0.0.1:8080 node typing.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// Fails on a page load after takeover, a console error, or a step that does not show up.
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const measured = {};
// The longest a key may wait for the page's thread while a word is typed, as it is.
const KEY_WAIT_MS = 100;
const GAP_MS = 200;

const page = await browser.newPage({ viewport: { width: 1500, height: 900 } });
page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 300)); });
page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
await page.waitForFunction(() => window.betulaSearch?.ready === true, null, { timeout: 60000 }).catch(() => problems.push("the search worker never answered"));
await page.evaluate(() => {
  window.__marker = 1;
  // The semantic search as boot.js offers it, answering as its worker does: the modules in a fixed
  // order of the text, the list's own first (as the real one finds them first).
  const ids = window.betulaDb.query("SELECT module_id FROM v_module_folded ORDER BY module_id", []).rows.map((row) => row[0]);
  window.betulaSemantic = {
    ready: Promise.resolve({ rows: ids.length }),
    search: async (query, k) => {
      const listed = [...document.querySelectorAll(".vlist a.row")].map((row) => row.dataset.id);
      let h = 0;
      for (const c of query) h = (h * 31 + c.charCodeAt(0)) >>> 0;
      const rest = ids.filter((_, i) => (i + h) % 7 === 0);
      return { hits: [...new Set([...listed, ...rest])].slice(0, k).map((id, i) => ({ id, score: 0.9 - i / 1000 })) };
    },
  };
  // What the page's own copy is asked: the search's queries read the folded names.
  const query = window.betulaDb.query.bind(window.betulaDb);
  window.__searched = 0;
  window.betulaDb.query = (sql, params) => {
    if (sql.includes("v_module_folded")) window.__searched++;
    return query(sql, params);
  };
  window.__keys = [];
  window.__long = [];
  new PerformanceObserver((list) => {
    for (const e of list.getEntries()) if (e.name === "keydown") window.__keys.push(e.processingStart - e.startTime);
  }).observe({ type: "event", durationThreshold: 16 });
  new PerformanceObserver((list) => { for (const e of list.getEntries()) window.__long.push(e.duration); }).observe({ type: "longtask" });
});

const cdp = await page.context().newCDPSession(page);
await page.click("#topsearch");
for (const [rate, word] of [[1, "datenbanksysteme"], [1, "maschinelles lernen"], [4, "regelungstechnik"], [4, "algoritmen und daten"]]) {
  await cdp.send("Emulation.setCPUThrottlingRate", { rate });
  await page.fill("#topsearch", "");
  await page.waitForTimeout(800);
  await page.evaluate(() => { window.__keys = []; window.__long = []; window.__searched = 0; });
  for (const key of word) {
    await page.keyboard.type(key);
    await page.waitForTimeout(GAP_MS);
  }
  const shown = await page.waitForFunction((word) => new URLSearchParams(location.search).get("q") === word, word, { timeout: 15000 }).then(() => true, () => false);
  if (!shown) problems.push(`${word}: the list never followed the typing (${page.url()})`);
  // „Ähnliche Module" come once the typing has stopped.
  await page.waitForTimeout(1500);
  const seen = await page.evaluate(() => ({
    keys: window.__keys,
    long: window.__long,
    searched: window.__searched,
    listed: [...document.querySelectorAll(".vlist a.row")].map((row) => row.dataset.id),
    similar: [...document.querySelectorAll(".similar a.row")].map((row) => row.dataset.id),
  }));
  const worst = Math.round(Math.max(0, ...seen.keys));
  measured[`${word} (CPU ×${rate})`] = { worstKeyWaitMs: worst, longTasks: seen.long.length, longestTaskMs: Math.round(Math.max(0, ...seen.long)), longTasksMs: Math.round(seen.long.reduce((sum, ms) => sum + ms, 0)) };
  if (seen.searched) problems.push(`${word}: the page's own copy ran ${seen.searched} queries of the search while it was typed`);
  const twice = seen.similar.filter((id) => seen.listed.includes(id));
  if (twice.length) problems.push(`${word}: „Ähnliche Module“ repeat rows of the list: ${twice.join(", ")}`);
  if (shown && seen.listed.length && !seen.similar.length) problems.push(`${word}: no „Ähnliche Module“ under the results`);
  if (rate === 1 && worst > KEY_WAIT_MS) problems.push(`${word}: a key waited ${worst} ms for the page's thread`);
}
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });

// A module searched by its number: another module of the same title is no „Ähnliches Modul“
// (14851 and 14508, „Anti-Gewalt-Arbeit").
await page.fill("#topsearch", "14851");
await page.waitForFunction(() => new URLSearchParams(location.search).get("q") === "14851" && document.querySelector('.vlist a.row[data-id="14851"]'), null, { timeout: 15000 })
  .catch(() => problems.push("14851: not found by its number"));
await page.evaluate(() => {
  const search = window.betulaSemantic.search;
  window.betulaSemantic.search = async (query, k) => ({ hits: [{ id: "14508", score: 0.99 }, ...(await search(query, k)).hits.filter((hit) => hit.id !== "14508")] });
});
await page.fill("#topsearch", "");
await page.waitForTimeout(400);
await page.fill("#topsearch", "14851");
await page.waitForFunction(() => new URLSearchParams(location.search).get("q") === "14851" && document.querySelectorAll(".similar a.row").length > 0, null, { timeout: 15000 })
  .catch(() => problems.push("14851: no „Ähnliche Module“"));
const twin = await page.evaluate(() => Boolean(document.querySelector('.similar a.row[data-id="14508"]')));
if (twin) problems.push("14851: its namesake 14508 stands among „Ähnliche Module“");
if (!(await page.evaluate(() => window.__marker === 1))) problems.push("the page was loaded again");

await browser.close();
console.log(JSON.stringify({ measured, problems }, null, 2));
process.exit(problems.length ? 1 : 0);
