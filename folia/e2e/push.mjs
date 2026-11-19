// Checks the notifications of the app (owner, 2026-11-19: „mach beides und dann Folias Teil";
// enhance.js „notifications", sw.js, folia/crates/shell/src/chrome.rs `PushRow`,
// docs/xylem/protocol.md section 5), against a Xylem that runs: the row of the menu of the
// profiles where the site offers them; switched on from a tap, the browser asks for the
// permission, subscribes with Xylem's key and puts its subscription with the page's language and
// every topic; a topic unticked is put again, the last one unticked switches them off; the choice
// is this browser's and there after a reload, when nothing is put again within half a day and at
// once after the language changed; a subscription made with another key (a Xylem that lost its
// database) is given up for a new one; the permission refused, or not answered, is said under the
// row, and so is an iPhone outside the app on the home screen; switching off deletes the
// subscription at Xylem and gives it up in the browser. The service worker shows what Xylem
// sends, and something for a message it cannot read; a tap opens the address the message names,
// never one of another site.
//
// The browser's own push service is not part of it (a browser under automation has none): the
// page's `PushManager` is the check's own, with keys a browser would make and an endpoint at a
// push service Xylem writes to. That Xylem's message reaches a real one and opens there is
// xylem/live (docs/xylem/xylem.md, „Checks").
//
//   SMOKE_BASE_URL=http://127.0.0.1:8080 XYLEM_BIN=../../xylem/xylem node push.mjs
//
// Needs Folia with --push (FOLIA_PUSH=on; FOLIA_PUSH_URL=http://127.0.0.1:8300/push/v1/ where
// Xylem has a port of its own, which then takes the site as an origin: XYLEM_ORIGINS) and without
// --live-assets (the worker of live assets shows nothing). XYLEM_BIN is the binary that asks the
// running instance what it holds (`xylem status`; XYLEM_INSTANCE where it is not 127.0.0.1:8300);
// without it, what Xylem holds is not looked at. SMOKE_BROWSER_CHANNEL=msedge by default.
// Fails on a console error or a step that does not show up.
import { execFileSync } from "node:child_process";
import { chromium } from "playwright-core";

const base = (process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const xylem = process.env.XYLEM_BIN || "";
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge", headless: !process.env.SMOKE_HEADED });
const problems = [];
const check = (ok, message) => { if (!ok) problems.push(message); };
const phone = { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true };

// What Xylem holds, by its own word; null without the binary.
const held = () => {
  if (!xylem) return null;
  try {
    return JSON.parse(execFileSync(xylem, ["status"], { encoding: "utf8", env: { ...process.env, XYLEM_LOG_FORMAT: "text" } }));
  } catch (error) {
    problems.push("xylem status: " + String(error).slice(0, 200));
    return null;
  }
};
const topicsHeld = (status) => Object.fromEntries(Object.entries((status && status.topics) || {}).filter(([, n]) => n > 0));

// The browser's side of the Push API, the check's own: a subscription with keys as a browser
// makes them, kept across reloads as a browser keeps its own, and a permission the check answers.
// `e2e.permission` is what the page sees (default, granted, denied), `e2e.answer` what the browser's
// question is answered with.
const pushStub = () => {
  const KEPT = "e2e.subscription";
  const b64 = (bytes) => btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  const bytes = (text) => Uint8Array.from(atob(text.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));
  const log = () => JSON.parse(localStorage.getItem("e2e.log") || "[]");
  const note = (what) => localStorage.setItem("e2e.log", JSON.stringify([...log(), what]));
  const wrap = (kept) => ({
    endpoint: kept.endpoint,
    options: { userVisibleOnly: true, applicationServerKey: bytes(kept.key).buffer },
    toJSON: () => ({ endpoint: kept.endpoint, expirationTime: null, keys: { p256dh: kept.p256dh, auth: kept.auth } }),
    unsubscribe: async () => { localStorage.removeItem(KEPT); note("unsubscribe"); return true; },
  });
  if (!("PushManager" in window)) return;
  PushManager.prototype.getSubscription = async function () {
    const kept = localStorage.getItem(KEPT);
    return kept ? wrap(JSON.parse(kept)) : null;
  };
  PushManager.prototype.subscribe = async function (options) {
    const pair = await crypto.subtle.generateKey({ name: "ECDH", namedCurve: "P-256" }, true, ["deriveBits"]);
    const kept = {
      endpoint: "https://updates.push.services.mozilla.com/wpush/v2/e2e-" + crypto.randomUUID(),
      key: b64(options.applicationServerKey),
      p256dh: b64(await crypto.subtle.exportKey("raw", pair.publicKey)),
      auth: b64(crypto.getRandomValues(new Uint8Array(16))),
    };
    localStorage.setItem(KEPT, JSON.stringify(kept));
    note("subscribe");
    return wrap(kept);
  };
  Object.defineProperty(Notification, "permission", { configurable: true, get: () => localStorage.getItem("e2e.permission") || "default" });
  Notification.requestPermission = async () => {
    note("asked");
    const answer = localStorage.getItem("e2e.answer") || "granted";
    if (answer !== "default") localStorage.setItem("e2e.permission", answer);
    return answer;
  };
};

async function open(context, path) {
  const page = await context.newPage();
  page.on("console", (m) => { if (m.type() === "error") problems.push("console: " + m.text().slice(0, 600)); });
  page.on("pageerror", (e) => problems.push("pageerror: " + String(e).slice(0, 300)));
  await page.goto(base + path, { waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => problems.push("the browser app never took over"));
  return page;
}
const ROW = ".profile-menu button.pm-push";
const openMenu = async (page) => {
  await page.tap(".topbar .top-profile").catch(() => problems.push("the phone's profile box cannot be tapped"));
  await page.waitForSelector(`${ROW} >> visible=true`, { timeout: 5000 }).catch(() => problems.push("the menu of the profiles has no row for notifications"));
};
// The row as the page shows it, once nothing is on its way any more.
const row = async (page) => {
  await page.waitForFunction((sel) => { const row = document.querySelector(sel); return row && row.getAttribute("aria-busy") !== "true"; }, ROW, { timeout: 15000 }).catch(() => problems.push("the row stays busy"));
  await page.waitForTimeout(150);
  return page.evaluate((sel) => {
    const row = document.querySelector(sel);
    const menu = row.closest(".profile-menu");
    return {
      on: row.getAttribute("aria-checked") === "true",
      hint: row.querySelector("small")?.textContent || "",
      icon: row.querySelector("svg use")?.getAttribute("href")?.split("#")[1] || "",
      topics: [...menu.querySelectorAll(".pm-topic")].map((topic) => [topic.dataset.pushTopic, topic.getAttribute("aria-checked") === "true", topic.textContent.trim()]),
      note: menu.querySelector(".pm-push-note")?.textContent || "",
      state: window.betulaPush.state(),
      kept: localStorage.getItem("betula.push.v1"),
      log: JSON.parse(localStorage.getItem("e2e.log") || "[]"),
    };
  }, ROW);
};
const forgetLog = (page) => page.evaluate(() => localStorage.removeItem("e2e.log"));

// ---- a site that offers them: the row, switched on from a tap ----
{
  const context = await browser.newContext({ ...phone, locale: "de-DE", permissions: ["notifications"] });
  await context.addInitScript(pushStub);
  const page = await open(context, "/catalog");
  const offered = await page.evaluate(() => document.querySelector('meta[name="betula-push"]')?.content);
  check(offered !== undefined, "the page does not say that the site offers notifications (Folia without --push?)");
  await openMenu(page);
  let now = await row(page);
  check(!now.on && now.icon === "bell-off" && now.hint.includes("Prüfungen") && now.topics.length === 0 && now.note === "", `off at first: ${JSON.stringify(now)}`);
  check(now.state === "yes;default;;0;", `the script's word at first: ${now.state}`);
  const before = held();

  await page.tap(ROW);
  now = await row(page);
  check(now.on && now.icon === "bell" && now.hint === "", `switched on: ${JSON.stringify(now)}`);
  check(JSON.stringify(now.topics) === JSON.stringify([["exams", true, "Prüfungen"], ["semester", true, "Semester"], ["breaks", true, "Pausen"]]), `every topic ticked: ${JSON.stringify(now.topics)}`);
  check(now.kept === "exams,semester,breaks" && now.state === "yes;granted;exams,semester,breaks;0;", `kept: ${now.kept}, ${now.state}`);
  check(JSON.stringify(now.log) === JSON.stringify(["asked", "subscribe"]), `asked for the permission, then subscribed: ${JSON.stringify(now.log)}`);
  let status = held();
  if (status && before) {
    check(status.subscriptions === before.subscriptions + 1, `Xylem holds ${status.subscriptions} subscriptions, ${before.subscriptions} before`);
    const topics = topicsHeld(status), was = topicsHeld(before);
    check(["exams", "semester", "breaks"].every((topic) => (topics[topic] || 0) === (was[topic] || 0) + 1), `Xylem's topics: ${JSON.stringify(topics)}, before ${JSON.stringify(was)}`);
  }
  // What the worker reads when the browser replaces the subscription by itself.
  const settings = await page.evaluate(async () => (await (await caches.open("betula-push")).match("/push-settings"))?.json());
  check(settings && settings.lang === "de" && settings.topics.length === 3 && settings.base.endsWith("/push/v1/"), `the choice kept for the worker: ${JSON.stringify(settings)}`);

  // A topic unticked is put again; it stays on.
  await forgetLog(page);
  await page.tap('.profile-menu .pm-topic[data-push-topic="breaks"]');
  now = await row(page);
  check(now.on && now.kept === "exams,semester" && now.topics.find(([topic]) => topic === "breaks")?.[1] === false, `„Pausen“ unticked: ${JSON.stringify(now)}`);
  check(now.log.length === 0, `no new subscription for a topic: ${JSON.stringify(now.log)}`);
  status = held();
  if (status && before) {
    const topics = topicsHeld(status), was = topicsHeld(before);
    check((topics.breaks || 0) === (was.breaks || 0) && (topics.exams || 0) === (was.exams || 0) + 1 && status.subscriptions === before.subscriptions + 1, `Xylem after „Pausen“ went: ${JSON.stringify(topics)}`);
  }

  // The choice is this browser's: there after a reload, and nothing is put again so soon.
  let puts = 0;
  page.on("request", (request) => { if (request.url().includes("/push/v1/subscription")) puts++; });
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await page.waitForTimeout(6500);
  check(puts === 0, `put again ${puts} times right after it was put`);
  await openMenu(page);
  now = await row(page);
  check(now.on && now.kept === "exams,semester" && now.topics.filter(([, ticked]) => ticked).length === 2, `after a reload: ${JSON.stringify(now)}`);
  // … but at once in another language (Xylem tells it in that one from now on).
  puts = 0;
  const english = await open(context, "/en/catalog");
  english.on("request", (request) => { if (request.url().includes("/push/v1/subscription") && request.method() === "PUT") puts++; });
  await english.waitForTimeout(7000);
  check(puts === 1, `the subscription put ${puts} times after the language changed`);
  const seen = await english.evaluate(() => JSON.parse(localStorage.getItem("betula.push.seen.v1") || "null"));
  check(seen && seen.lang === "en", `the language it was last put in: ${JSON.stringify(seen)}`);
  await english.tap(".topbar .top-profile").catch(() => {});
  await english.waitForSelector(`${ROW} >> visible=true`, { timeout: 5000 }).catch(() => problems.push("the English menu has no row for notifications"));
  const en = await row(english);
  check(en.on && JSON.stringify(en.topics.map(([, , name]) => name)) === JSON.stringify(["Exams", "Semester", "Breaks"]), `the English row: ${JSON.stringify(en)}`);
  await english.close();

  // A Xylem that lost its database has another key: the old subscription is given up for a new
  // one, without anybody doing anything, at the next start that puts.
  await page.evaluate(() => {
    const kept = JSON.parse(localStorage.getItem("e2e.subscription"));
    kept.key = kept.key.slice(0, -4) + (kept.key.endsWith("AAAA") ? "BBBB" : "AAAA");
    localStorage.setItem("e2e.subscription", JSON.stringify(kept));
    localStorage.removeItem("betula.push.seen.v1");
    localStorage.removeItem("e2e.log");
  });
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 120000 }).catch(() => {});
  await page.waitForFunction(() => localStorage.getItem("betula.push.seen.v1") !== null, null, { timeout: 15000 }).catch(() => problems.push("nothing was put at the start after the key changed"));
  const again = await page.evaluate(() => JSON.parse(localStorage.getItem("e2e.log") || "[]"));
  check(JSON.stringify(again) === JSON.stringify(["unsubscribe", "subscribe"]), `a subscription of another key: ${JSON.stringify(again)}`);

  // ---- the worker shows what Xylem sends ----
  const cdp = await context.newCDPSession(page);
  await cdp.send("ServiceWorker.enable");
  const registrationId = await new Promise((resolve) => {
    const timer = setTimeout(() => resolve(null), 15000);
    cdp.on("ServiceWorker.workerRegistrationUpdated", ({ registrations }) => {
      const mine = registrations.find((registration) => registration.scopeURL.startsWith(base) && !registration.isDeleted);
      if (mine) { clearTimeout(timer); resolve(mine.registrationId); }
    });
  });
  check(registrationId, "no service worker registered (Folia with --live-assets?)");
  const shown = () => page.evaluate(async () => (await (await navigator.serviceWorker.ready).getNotifications()).map((n) => ({ title: n.title, body: n.body, tag: n.tag, lang: n.lang, data: n.data, icon: n.icon })));
  const deliver = async (data) => {
    await cdp.send("ServiceWorker.deliverPushMessage", { origin: base, registrationId, data });
    await page.waitForTimeout(600);
  };
  if (registrationId) {
    await page.evaluate(() => navigator.serviceWorker.ready);
    await deliver(JSON.stringify({
      web_push: 8030,
      notification: { title: "Die Prüfungsphase beginnt", body: "Bis 20.02. stehen viele Prüfungen des WiSe 2026/27 an. Viel Erfolg!", navigate: base + "/studyplan", lang: "de", tag: "exams-2026w-1", timestamp: 1801461600000, data: { topic: "exams" } },
    }));
    let notes = await shown();
    const told = notes.find((n) => n.tag === "exams-2026w-1");
    check(told && told.title === "Die Prüfungsphase beginnt" && told.body.startsWith("Bis 20.02.") && told.lang === "de" && told.data.topic === "exams" && told.data.navigate === base + "/studyplan" && told.icon.endsWith("/assets/icon-192.png"),
      `the notification of Xylem's message: ${JSON.stringify(notes)}`);
    // The same announcement again takes its own place (its tag); a message nobody can read is
    // still a notification.
    await deliver(JSON.stringify({ web_push: 8030, notification: { title: "Die Prüfungsphase beginnt", navigate: base + "/", tag: "exams-2026w-1" } }));
    await deliver("not a message of Xylem's");
    notes = await shown();
    check(notes.filter((n) => n.tag === "exams-2026w-1").length === 1 && notes.some((n) => n.title === "Betula" && n.data.navigate === "/"), `one per announcement, and one for what cannot be read: ${JSON.stringify(notes)}`);
  }
  // Nothing of this browser stays at Xylem.
  await openMenu(page);
  await page.tap(ROW);
  now = await row(page);
  check(!now.on && now.kept === null, `switched off at the end: ${JSON.stringify(now)}`);
  await context.close();
}

// ---- switching off: deleted at Xylem, given up in the browser ----
{
  const context = await browser.newContext({ ...phone, locale: "de-DE" });
  await context.addInitScript(pushStub);
  const page = await open(context, "/catalog");
  await openMenu(page);
  const before = held();
  await page.tap(ROW);
  let now = await row(page);
  check(now.on, `switched on in a new browser: ${JSON.stringify(now)}`);
  const during = held();
  if (before && during) check(during.subscriptions === before.subscriptions + 1, `Xylem holds ${during.subscriptions}, ${before.subscriptions} before`);
  await forgetLog(page);
  // The last topic unticked switches them off as the switch does.
  await page.tap('.profile-menu .pm-topic[data-push-topic="exams"]');
  await row(page);
  await page.tap('.profile-menu .pm-topic[data-push-topic="semester"]');
  await row(page);
  await page.tap('.profile-menu .pm-topic[data-push-topic="breaks"]');
  now = await row(page);
  check(!now.on && now.kept === null && now.topics.length === 0 && now.icon === "bell-off", `the last topic unticked: ${JSON.stringify(now)}`);
  check(now.log.includes("unsubscribe"), `the subscription given up: ${JSON.stringify(now.log)}`);
  const after = held();
  if (before && after) check(after.subscriptions === before.subscriptions, `Xylem holds ${after.subscriptions} after it was switched off, ${before.subscriptions} before`);
  const settings = await page.evaluate(async () => Boolean(await (await caches.open("betula-push")).match("/push-settings")));
  check(!settings, "the choice is still kept for the worker after it was switched off");
  // On again with the switch, off with the switch.
  await page.tap(ROW);
  now = await row(page);
  check(now.on && now.kept === "exams,semester,breaks", `on again: ${JSON.stringify(now)}`);
  await page.tap(ROW);
  now = await row(page);
  check(!now.on && now.kept === null, `off with the switch: ${JSON.stringify(now)}`);
  const end = held();
  if (before && end) check(end.subscriptions === before.subscriptions, `Xylem holds ${end.subscriptions} at the end, ${before.subscriptions} before`);
  await context.close();
}

// ---- what is in the way is said, and the switch is never dead ----
{
  // The permission refused.
  const context = await browser.newContext({ ...phone, locale: "de-DE" });
  await context.addInitScript(pushStub);
  const page = await open(context, "/catalog");
  await page.evaluate(() => localStorage.setItem("e2e.answer", "denied"));
  await openMenu(page);
  await page.tap(ROW);
  let now = await row(page);
  check(!now.on && now.kept === null && now.note.includes("blockiert") && now.state.endsWith(";denied"), `the permission refused: ${JSON.stringify(now)}`);
  check(await page.evaluate((sel) => document.querySelector(sel).getAttribute("aria-describedby") === "pm-push-note", ROW), "the switch is not described by its note");
  // Not answered: asked again with the next tap, and then it goes.
  await page.evaluate(() => { localStorage.setItem("e2e.permission", "default"); localStorage.setItem("e2e.answer", "default"); });
  await page.tap(ROW);
  now = await row(page);
  check(!now.on && now.note.includes("nicht beantwortet"), `the question not answered: ${JSON.stringify(now)}`);
  await page.evaluate(() => localStorage.setItem("e2e.answer", "granted"));
  await page.tap(ROW);
  now = await row(page);
  check(now.on && now.note === "", `allowed with the next tap: ${JSON.stringify(now)}`);
  // Blocked in the browser's settings afterwards: said without a tap, and what was chosen stays.
  await page.evaluate(() => { localStorage.setItem("e2e.permission", "denied"); dispatchEvent(new Event("betula:push")); });
  now = await row(page);
  check(!now.on && now.kept === "exams,semester,breaks" && now.note.includes("blockiert"), `blocked afterwards: ${JSON.stringify(now)}`);
  await page.evaluate(() => { localStorage.setItem("e2e.permission", "granted"); dispatchEvent(new Event("betula:push")); });
  await page.tap(ROW);
  await row(page);
  await context.close();
}
{
  // An iPhone in Safari's own tab: no Push API there, `navigator.standalone` false.
  const context = await browser.newContext({ ...phone, locale: "de-DE" });
  await context.addInitScript(() => {
    delete window.PushManager;
    Object.defineProperty(Navigator.prototype, "standalone", { configurable: true, get: () => false });
  });
  const page = await open(context, "/catalog");
  await openMenu(page);
  let now = await row(page);
  check(!now.on && now.note === "" && now.state === "install;default;;0;", `an iPhone's tab before a tap: ${JSON.stringify(now)}`);
  await page.tap(ROW);
  now = await row(page);
  check(!now.on && now.note.includes("Home-Bildschirm") && now.kept === null, `an iPhone's tab says where it works: ${JSON.stringify(now)}`);
  await context.close();
}
{
  // A browser without push messages at all.
  const context = await browser.newContext({ ...phone, locale: "en-GB" });
  await context.addInitScript(() => { delete window.PushManager; });
  const page = await open(context, "/en/catalog");
  await page.tap(".topbar .top-profile").catch(() => {});
  await page.waitForSelector(`${ROW} >> visible=true`, { timeout: 5000 }).catch(() => problems.push("no row in a browser without push messages"));
  await page.tap(ROW);
  const now = await row(page);
  check(!now.on && now.note.includes("cannot receive notifications"), `a browser without push messages: ${JSON.stringify(now)}`);
  await context.close();
}

// ---- the worker's handlers, run with events as a browser makes them ----
// (No automation taps a notification or replaces a subscription: the worker's own code is run
// here with what it reads of each event, and what it asks of the browser is written down.)
{
  const source = await (await fetch(base + "/sw.js")).text();
  const listeners = {};
  const opened = [], focused = [], sent = [], shown = [];
  let windows = [];
  let settings = null;
  const self = {
    location: new URL(base + "/sw.js"),
    addEventListener: (type, listener) => { listeners[type] = listener; },
    skipWaiting: async () => {},
    clients: {
      claim: async () => {},
      matchAll: async () => windows,
      openWindow: async (url) => { opened.push(url); },
    },
    registration: {
      showNotification: async (title, options) => { shown.push({ title, ...options }); },
      pushManager: { getSubscription: async () => null, subscribe: async () => null },
    },
  };
  const caches = {
    open: async () => ({ match: async () => (settings ? { json: async () => settings } : undefined), put: async () => {}, delete: async () => {}, keys: async () => [] }),
    match: async () => undefined,
    keys: async () => [],
    delete: async () => true,
  };
  const fetchStub = async (url, init) => { sent.push({ url: String(url), method: init?.method || "GET", body: init?.body ? JSON.parse(init.body) : null }); return { ok: true, status: 204 }; };
  const vm = await import("node:vm");
  vm.runInNewContext(source, { self, caches, fetch: fetchStub, URL, Request: class {}, Response: class {}, Promise, console });
  const run = async (type, event) => {
    let waited = Promise.resolve();
    listeners[type]({ ...event, waitUntil: (promise) => { waited = promise; } });
    await waited;
  };
  check(["push", "notificationclick", "pushsubscriptionchange"].every((type) => typeof listeners[type] === "function"), `the worker listens to: ${Object.keys(listeners)}`);

  // A tap: the address the notification names, in a new window while none shows it …
  const note = (navigate) => ({ notification: { close: () => {}, data: { navigate } } });
  await run("notificationclick", note(base + "/studyplan"));
  check(opened.length === 1 && opened[0] === base + "/studyplan", `a tap opened ${JSON.stringify(opened)}`);
  // … the window that shows it already, where there is one …
  windows = [{ url: base + "/catalog", focus: async () => { focused.push("catalog"); } }, { url: base + "/studyplan", focus: async () => { focused.push("studyplan"); } }];
  await run("notificationclick", note(base + "/studyplan"));
  check(opened.length === 1 && focused.join() === "studyplan", `a tap with the page open: opened ${JSON.stringify(opened)}, focused ${focused}`);
  // … and never an address of another site, nor one that is none.
  windows = [];
  for (const navigate of ["https://example.org/x", "javascript:alert(1)", undefined, "http://["]) {
    await run("notificationclick", note(navigate));
  }
  check(opened.slice(1).every((url) => url === base + "/") && opened.length === 5, `what is not the site's opens the start page: ${JSON.stringify(opened.slice(1))}`);

  // A push without anything to read still ends in a notification.
  await run("push", { data: null });
  await run("push", { data: { json: () => { throw new Error("no JSON"); } } });
  await run("push", { data: { json: () => ({ web_push: 8030, notification: { title: "Weihnachtspause", navigate: base + "/", data: { topic: "breaks" } } }) } });
  check(shown.length === 3 && shown[0].title === "Betula" && shown[1].title === "Betula" && shown[2].title === "Weihnachtspause" && shown[2].data.navigate === base + "/" && shown[2].data.topic === "breaks" && shown[2].body === undefined,
    `what the worker shows: ${JSON.stringify(shown)}`);

  // The browser replaced the subscription by itself: the new one put with what the page chose,
  // the old one deleted; without a choice, nothing.
  const sub = (name) => ({ endpoint: "https://updates.push.services.mozilla.com/wpush/v2/" + name, toJSON: () => ({ endpoint: "https://updates.push.services.mozilla.com/wpush/v2/" + name, keys: { p256dh: "p", auth: "a" } }) });
  await run("pushsubscriptionchange", { oldSubscription: sub("old"), newSubscription: sub("new") });
  check(sent.length === 0, `a subscription replaced where nothing was chosen: ${JSON.stringify(sent)}`);
  settings = { base: base + "/push/v1/", lang: "en", topics: ["exams"] };
  await run("pushsubscriptionchange", { oldSubscription: sub("old"), newSubscription: sub("new") });
  check(sent.length === 2 && sent[0].method === "PUT" && sent[0].url === base + "/push/v1/subscription" && sent[0].body.endpoint.endsWith("/new") && sent[0].body.lang === "en" && sent[0].body.topics.join() === "exams"
    && sent[1].method === "DELETE" && sent[1].body.endpoint.endsWith("/old"), `a subscription replaced: ${JSON.stringify(sent)}`);
}

await browser.close();
if (problems.length) {
  console.error("PUSH: " + problems.length + " problem(s)");
  for (const problem of problems) console.error("  - " + problem);
  process.exit(1);
}
console.log("PUSH: the notifications' row, the subscription at Xylem, the worker's notification: as they should be");
