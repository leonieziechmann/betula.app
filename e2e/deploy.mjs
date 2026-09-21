// Checks that a page gets the stylesheet, the scripts and the bundle of its own build, also on the
// first load after a deploy, which the service worker of the old build still answers (the page
// comes from the new server, network first, while the old worker keeps the old files). It starts
// two builds of Folia one after the other on the address of SMOKE_BASE_URL:
//   1. build A: a visit installs the worker, which keeps A's shell;
//   2. A stops and B starts (the deploy); ONE reload, and the page must apply B's stylesheet and
//      ask for B's scripts and bundle, although A's worker answered the load;
//   3. without a network at once, before B's worker is installed: the page that loads must apply
//      the stylesheet of the build its markup names (the old worker must not show B's page with
//      A's files);
//   4. once B's worker has taken over, a reload without a network shows B's stylesheet as well.
// The two builds must differ in their stylesheet (any rule; the server embeds `app/assets/app.css`,
// so change it, build again and copy the binary away before building the other one):
//   DEPLOY_A=/tmp/folia-a.exe DEPLOY_B=/tmp/folia-b.exe DEPLOY_ARGS="--data-dir web-data --site-root site" \
//     SMOKE_BASE_URL=http://127.0.0.1:8193 node deploy.mjs      (SMOKE_BROWSER_CHANNEL=msedge by default)
// The servers get `--addr` from SMOKE_BASE_URL and DEPLOY_ARGS as they are (split at spaces).
import { spawn } from "node:child_process";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8193").replace(/\/$/, "");
const builds = { A: process.env.DEPLOY_A, B: process.env.DEPLOY_B };
if (!builds.A || !builds.B) {
  console.error("DEPLOY_A and DEPLOY_B name the two builds of folia (see the head of this file)");
  process.exit(2);
}
const args = ["--addr", new URL(base).host, ...(process.env.DEPLOY_ARGS || "").split(/\s+/).filter(Boolean)];
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
// Playwright's waitForFunction takes the promise of an async function for a truthy answer, so a
// check that has to wait for something (the caches, the worker) is polled here.
async function until(check, arg, ms) {
  for (const end = Date.now() + ms; Date.now() < end; await pause(250)) {
    if (await page.evaluate(check, arg).catch(() => false)) return true;
  }
  return false;
}

// A server of one build: started, and ready once it serves a snapshot. Returns its build id.
// Nothing else may answer on the address: the check would test that server instead.
let server = null;
const answers = () => fetch(base + "/livez").then(() => true, () => false);
async function start(name) {
  if (await answers()) throw new Error(`something else already answers on ${base}: choose another SMOKE_BASE_URL`);
  const child = spawn(builds[name], args, { stdio: "ignore" });
  let exited = null;
  child.once("exit", (code) => { exited = code; });
  child.once("error", (error) => { exited = String(error); });
  server = child;
  for (let i = 0; i < 600; i++) {
    if (exited !== null) throw new Error(`build ${name} stopped at once (${exited})`);
    try {
      const status = await (await fetch(base + "/api/status")).json();
      if (status.snapshot) return status.build;
    } catch {}
    await pause(200);
  }
  throw new Error(`build ${name} served no snapshot within two minutes`);
}
async function stop() {
  if (!server || server.exitCode !== null) return;
  const gone = new Promise((resolve) => server.once("exit", resolve));
  server.kill();
  await gone;
  for (let i = 0; i < 50 && (await answers()); i++) await pause(100);
}

// Does the page apply exactly this stylesheet? Both sides are read by the browser's own CSS
// parser, so formatting does not matter, every rule does. Every linked sheet must be it.
const applies = (css) => page.evaluate((css) => {
  const rules = (sheet) => [...sheet.cssRules].map((rule) => rule.cssText).join("\n");
  const want = new CSSStyleSheet();
  want.replaceSync(css);
  const links = [...document.querySelectorAll("link[rel=stylesheet]")];
  return links.length > 0 && links.every((link) => link.sheet && rules(link.sheet) === rules(want));
}, css);
// What the page applies instead, for the message: each sheet, and its first rule unlike `css`.
const differs = (css) => page.evaluate((css) => {
  const want = new CSSStyleSheet();
  want.replaceSync(css);
  return [...document.querySelectorAll("link[rel=stylesheet]")].map((link) => {
    const rules = link.sheet ? [...link.sheet.cssRules] : [];
    let at = 0;
    while (at < Math.max(rules.length, want.cssRules.length) && rules[at]?.cssText === want.cssRules[at]?.cssText) at++;
    const place = new URL(link.href);
    return `${place.pathname + place.search}: ${rules.length} rules (want ${want.cssRules.length})` + (at < rules.length ? `, rule ${at} is ${JSON.stringify(rules[at].cssText.slice(0, 80))}` : "");
  }).join("; ") || "no stylesheet";
}, css);
// The files that change with a build, as this page asked for them.
const asked = () => page.evaluate(() => performance.getEntriesByType("resource").map((entry) => new URL(entry.name))
  .filter((url) => /^\/(assets\/(app\.css|enhance\.js|boot\.js|sql-wasm\.(js|wasm))|pkg\/)/.test(url.pathname))
  .map((url) => url.pathname + url.search));
const shells = () => page.evaluate(async () => (await caches.keys()).filter((name) => name.startsWith("betula-shell-")));
const stylesheet = async () => (await fetch(base + "/assets/app.css")).text();

let browser = null;
let page = null;
try {
  // 1. Build A, with a worker that keeps its shell and answers the page.
  const buildA = await start("A");
  const cssA = await stylesheet();
  browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
  const context = await browser.newContext({ viewport: { width: 1300, height: 900 } });
  page = await context.newPage();
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + "/catalog", { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("A: the browser app never took over"));
  const installed = await until(async (shell) => {
    const registration = await navigator.serviceWorker.getRegistration();
    return Boolean(registration && registration.active && navigator.serviceWorker.controller && (await caches.keys()).includes(shell));
  }, "betula-shell-" + buildA, 120000);
  check(installed, "A: the service worker did not take over with its shell");
  await page.reload({ waitUntil: "load" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 }).catch(() => problems.push("A: the app did not take over the reloaded page"));
  check(await applies(cssA), "A: the page does not apply A's stylesheet (the comparison itself is wrong): " + (await differs(cssA)));

  // 2. The deploy: B replaces A; the reload is answered by A's worker, the page by B's server.
  await stop();
  const buildB = await start("B");
  const cssB = await stylesheet();
  check(buildA !== buildB, `the two servers report the same build (${buildA})`);
  check(cssA !== cssB, "the two builds have the same stylesheet: this check would prove nothing");
  await page.reload({ waitUntil: "load" });
  const answeredByWorker = await page.evaluate(() => performance.getEntriesByType("navigation")[0].workerStart > 0);
  check(answeredByWorker, "B, first load: no service worker answered the load, so the old worker was not tested");
  check(await applies(cssB), "B, first load: the page applies " + ((await applies(cssA)) ? "A's (the old build's) stylesheet" : "neither B's nor A's stylesheet: " + (await differs(cssB))));
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 }).catch(() => problems.push("B, first load: the app did not take over"));
  const files = await asked();
  const foreign = files.filter((file) => !file.endsWith("?v=" + buildB));
  check(files.length >= 5 && foreign.length === 0, `B, first load: files not of build B: ${JSON.stringify(foreign)} (of ${files.length})`);

  // 3. The network is gone before B's worker could install: whichever worker answers, the page
  // and its stylesheet are of one build. (A worker that loses the network while it installs
  // gives up, so the old one stays in charge until the next load with a network.)
  await context.setOffline(true);
  const offlineShells = await shells();
  await page.reload({ waitUntil: "load" }).catch((error) => problems.push("offline after the deploy: the page did not load (" + String(error).slice(0, 120) + ")"));
  const named = await page.evaluate(() => new URL(document.querySelector("link[rel=stylesheet]")?.href || location.href).searchParams.get("v"));
  const offlineAfterDeploy = named === buildA ? "A" : named === buildB ? "B" : `neither (${named})`;
  check(named === buildA || named === buildB, `offline after the deploy: the page links the stylesheet of ${offlineAfterDeploy}`);
  if (named === buildA || named === buildB) {
    const css = named === buildA ? cssA : cssB;
    check(await applies(css), `offline after the deploy: the page of build ${offlineAfterDeploy} does not apply its build's stylesheet: ${await differs(css)}`);
  }
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 }).catch(() => problems.push("offline after the deploy: the app did not take over"));
  await context.setOffline(false);

  // 4. With the network back, B's worker installs and takes over (and drops A's shell); then B's
  // page and files, with the network and without.
  await page.reload({ waitUntil: "load" });
  const replaced = await until(async ([a, b]) => {
    const names = await caches.keys();
    return names.includes("betula-shell-" + b) && !names.includes("betula-shell-" + a);
  }, [buildA, buildB], 120000);
  check(replaced, `B: its worker did not replace A's shell (${await shells()})`);
  await page.reload({ waitUntil: "load" });
  check(await applies(cssB), "B, second load: the page does not apply B's stylesheet: " + (await differs(cssB)));
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 }).catch(() => problems.push("B, second load: the app did not take over"));
  await context.setOffline(true);
  await page.reload({ waitUntil: "load" }).catch((error) => problems.push("B, offline: the page did not load (" + String(error).slice(0, 120) + ")"));
  check(await applies(cssB), "B, offline: the page does not apply B's stylesheet: " + (await differs(cssB)));
  await page.waitForFunction(() => window.__betulaApp === true && document.querySelectorAll(".rows a.row").length > 5, null, { timeout: 60000 }).catch(() => problems.push("B, offline: the app did not take over"));
  await context.setOffline(false);
  console.log(JSON.stringify({ buildA, buildB, firstLoadAfterDeploy: files, offlineAfterDeploy: { shells: offlineShells, answered: offlineAfterDeploy } }, null, 2));
} catch (error) {
  problems.push(String(error).slice(0, 300));
} finally {
  if (browser) await browser.close();
  await stop();
}
console.log(JSON.stringify({ problems }, null, 2));
process.exit(problems.length ? 1 : 0);
