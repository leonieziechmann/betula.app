// Betula as an app (owner, 2026-11-17: „Mach mal bitte a und b. Also schon bei der einrichtung
// fragen und dann, wenn es noch nicht passiert ist dann anzeigen. Bei der einrichtung soll es nur
// später als option geben und wenn man es dann bei b nicht mehr anzeigen wählt, dann wird es auch
// nicht mehr angezeigt"; folia/design/installieren/konzepte.html, A and B; home/install.rs):
// - A browser that has not offered its dialog (Chromium before `beforeinstallprompt`, or with Betula
//   installed): the setup ends as before, no chip „App".
// - Once it offers it (an event put in by the check, whose `prompt()` it counts): after „Studium
//   anlegen" the step „App" with its chip, „Installieren" and „Später" alone; „Installieren" opens the
//   browser's dialog and, accepted, ends the setup („Eingerichtet"), installed, no card on the day.
// - „Später": the setup ends, no card that day; three more days with Betula and a week later the
//   card on the day; its × puts it off again; „Nicht mehr anzeigen" ends it, also after a reload.
// - A study set up before (never asked): the card at once once the browser offers its dialog.
// - An iPhone (its user agent, `navigator.standalone`): „Auf den Home-Bildschirm" opens the guide
//   with its three steps and puts the profile on the clipboard („Kopiert"); „Verstanden" ends the
//   setup. The app on its home screen (standalone, its own storage, empty): „Studium einfügen" over
//   step 1 takes the profile from the clipboard, and the day is there; something else on the
//   clipboard is said and taken nowhere.
// - Running installed (`display-mode: standalone`): neither the step nor the card, and counted as
//   installed.
// - Taken off since (owner, 2026-11-18: „ich hatte es … chrome explizit deinstalliert zum testen"):
//   „installed" kept from the app, and the browser's offer as the page loads, before the start page
//   listens: the offer outweighs what was kept, and the step comes.
// Nothing of it in a request. Needs radix serve-snapshot + folia running and a fresh
// `bash scripts/build-client.sh`; a server without `--sync` (three steps before „App").
//   node e2e/install.mjs [base-url]
import { fileURLToPath } from "node:url";
import { chromium, devices } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };
const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const ON_PAGE = { path: fileURLToPath(new URL("on-page.js", import.meta.url)) };
const IPHONE_UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 26_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.1 Mobile/15E148 Safari/604.1";
const KEY = "betula.install.v1";
const errors = [];
const asked = [];

// Today as the app counts it: days since 1970-01-01 of the local date.
const today = () => { const d = new Date(); return Math.round(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) / 86400000); };

// A browser: `ios` an iPhone's Safari (`installed`: on its home screen), `standalone` an app window
// of Chromium; `seed` what it keeps before the first page; `offerAtLoad` the browser offering its
// dialog as the page loads (after enhance.js, before the app).
const open = async ({ ios = false, installed = false, standalone = false, seed = {}, phone = false, offerAtLoad = false } = {}) => {
  const device = phone ? { ...devices["Pixel 7"] } : { viewport: { width: 1280, height: 860 } };
  const context = await browser.newContext({ ...device, ...(ios ? { userAgent: IPHONE_UA } : {}), timezoneId: "Europe/Berlin", locale: "de-DE" });
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin: base });
  await context.addInitScript(ON_PAGE);
  await context.addInitScript(({ ios, installed, standalone, seed, offerAtLoad }) => {
    if (ios) Object.defineProperty(Navigator.prototype, "standalone", { get: () => installed, configurable: true });
    if (offerAtLoad) {
      document.addEventListener("DOMContentLoaded", () => {
        window.__prompted = 0;
        const event = new Event("beforeinstallprompt", { cancelable: true });
        event.prompt = () => { window.__prompted += 1; return Promise.resolve(); };
        event.userChoice = Promise.resolve({ outcome: "dismissed", platform: "web" });
        dispatchEvent(event);
      });
    }
    const media = window.matchMedia.bind(window);
    window.matchMedia = (query) => (query === "(display-mode: standalone)" ? { matches: standalone, media: query, addEventListener() {}, removeEventListener() {} } : media(query));
    if (!sessionStorage.getItem("seeded")) {
      for (const [key, value] of Object.entries(seed)) localStorage.setItem(key, value);
      sessionStorage.setItem("seeded", "1");
    }
  }, { ios, installed, standalone, seed, offerAtLoad });
  const page = await context.newPage();
  page.on("pageerror", (error) => errors.push(String(error).slice(0, 300)));
  page.on("request", (request) => { if (/install|clipboard/i.test(request.url()) || /betula\.install/.test(request.postData() || "")) asked.push(request.url()); });
  await page.goto(base + "/", { waitUntil: "load" });
  await page.waitForFunction(() => window.__betulaApp === true && window.betulaAnswered === null, null, { timeout: 60000 });
  return { context, page };
};

// The browser offers its own dialog, as Chromium does once Betula is installable: an event whose
// `prompt()` is counted and answered `outcome`.
const offer = (page, outcome = "accepted") => page.evaluate((outcome) => {
  window.__prompted = 0;
  const event = new Event("beforeinstallprompt", { cancelable: true });
  event.prompt = () => { window.__prompted += 1; return Promise.resolve(); };
  event.userChoice = Promise.resolve({ outcome, platform: "web" });
  dispatchEvent(event);
  return event.defaultPrevented;
}, outcome);

// The setup up to „Studium anlegen": Informatik, the latest start, the plan's modules.
const setUp = async (page) => {
  await page.click("#ob-program >> visible=true");
  await page.waitForTimeout(600);
  await page.keyboard.type("Informatik");
  await page.waitForFunction(() => [...document.querySelectorAll("[role=option]")].some((option) => option.checkVisibility() && /^Informatik/.test(option.textContent.trim())), null, { timeout: 15000 });
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => onPage(".ob-sems") && !onPage(".ob-sems").classList.contains("wait"), null, { timeout: 10000 });
  await page.click(".ob-next >> visible=true");
  await page.waitForSelector(".ob-create >> visible=true");
  await page.click(".ob-create >> visible=true");
};
const card = (page) => page.evaluate(() => {
  const card = onPage(".day-install");
  return card && card.checkVisibility() ? { title: card.querySelector("h2")?.textContent, buttons: [...card.querySelectorAll("button")].map((b) => b.textContent.trim() || b.getAttribute("aria-label")) } : null;
});
const kept = (page) => page.evaluate((key) => localStorage.getItem(key) || "", KEY);
const chips = (page) => page.evaluate(() => allOnPage(".ob-chip").map((chip) => chip.textContent.replace(/^\d/, "").trim()));
const noteSaid = (page) => page.waitForSelector(".wait-note.setup-note >> visible=true", { timeout: 8000 }).then(() => true, () => false);

// ---------- A browser that has not offered its dialog: the setup as before ----------
{
  const { context, page } = await open();
  await setUp(page);
  check(await noteSaid(page), "without the browser's offer „Studium anlegen\" does not end the setup");
  check(!(await page.evaluate(() => Boolean(onPage(".ob-install")))), "a step „App\" without the browser's offer");
  check((await card(page)) === null, "a card on the day without the browser's offer");
  // A study set up before the step was there, never asked: the card at once once the browser offers.
  check(await offer(page), "the browser's own bar is not held back (preventDefault)");
  await page.waitForSelector(".day-install >> visible=true", { timeout: 5000 }).catch(() => failures.push("never asked: no card once the browser offers its dialog"));
  const shown = await card(page);
  check(shown?.title === "Betula als App" && shown.buttons.includes("Installieren") && shown.buttons.includes("Nicht mehr anzeigen") && shown.buttons.includes("Später"), `the card: ${JSON.stringify(shown)}`);
  // On a wide screen over the side's panels.
  check(await page.evaluate(() => onPage(".day-install")?.parentElement?.firstElementChild === onPage(".day-install") && !onPage(".day-install").closest(".day-main")), "the card does not stand first in the side's column");
  // „Nicht mehr anzeigen": gone, and after a reload with the browser's offer again as well.
  await page.click(".day-install >> text=Nicht mehr anzeigen");
  await page.waitForTimeout(200);
  check((await card(page)) === null, "„Nicht mehr anzeigen\" leaves the card");
  check(/^never$/m.test(await kept(page)), `„Nicht mehr anzeigen\" is not kept: ${JSON.stringify(await kept(page))}`);
  await page.reload({ waitUntil: "load" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
  await offer(page);
  await page.waitForTimeout(500);
  check((await card(page)) === null, "the card comes back after „Nicht mehr anzeigen\"");
  await context.close();
}

// ---------- A: the step „App", installed ----------
{
  const { context, page } = await open();
  await offer(page, "accepted");
  await setUp(page);
  await page.waitForSelector(".ob-install >> visible=true", { timeout: 8000 }).catch(() => failures.push("no step „App\" after „Studium anlegen\" where the browser offers its dialog"));
  const step = await page.evaluate(() => ({
    title: onPage(".ob-title")?.textContent,
    buttons: allOnPage(".ob-actions button").map((b) => b.textContent.trim()),
    points: allOnPage(".ob-points li").length,
    on: onPage(".ob-chip.on")?.textContent.replace(/^\d/, "").trim(),
  }));
  check(step.title === "Betula als App?" && step.buttons.join() === "Später,Installieren" && step.points === 3 && step.on === "App", `the step „App": ${JSON.stringify(step)}`);
  check((await chips(page)).length === 4, `the chips with „App": ${JSON.stringify(await chips(page))}`);
  await page.click(".ob-install >> visible=true");
  check(await noteSaid(page), "„Installieren\", accepted, does not end the setup");
  check(await page.evaluate(() => window.__prompted) === 1, "„Installieren\" does not open the browser's dialog once");
  check(/^installed$/m.test(await kept(page)), "installed is not kept");
  await page.waitForTimeout(300);
  check((await card(page)) === null, "a card on the day after installing");
  await context.close();
}

// ---------- A: „Später", and the card after a while ----------
{
  const { context, page } = await open();
  await offer(page, "dismissed");
  await setUp(page);
  await page.waitForSelector(".ob-later >> visible=true", { timeout: 8000 });
  await page.click(".ob-later >> visible=true");
  check(await noteSaid(page), "„Später\" does not end the setup");
  check(await page.evaluate(() => window.__prompted) === 0, "„Später\" opened the browser's dialog");
  check(new RegExp(`^later\\t${today()}$`, "m").test(await kept(page)), `„Später\" is not kept for today: ${JSON.stringify(await kept(page))}`);
  check((await card(page)) === null, "the card on the day of „Später\"");
  // A week and three more days with Betula later.
  const t = today();
  await page.evaluate(({ key, text }) => localStorage.setItem(key, text), { key: KEY, text: `days\t${t - 9},${t - 4},${t - 2}\nlater\t${t - 9}\n` });
  await page.reload({ waitUntil: "load" });
  await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
  await offer(page, "dismissed");
  await page.waitForSelector(".day-install >> visible=true", { timeout: 5000 }).catch(() => failures.push("no card three days with Betula and a week after „Später\""));
  // Its ×: later again, from today.
  await page.click(".day-install-x");
  await page.waitForTimeout(200);
  check((await card(page)) === null && new RegExp(`^later\\t${t}$`, "m").test(await kept(page)), `the card's × does not put it off: ${JSON.stringify(await kept(page))}`);
  await context.close();
}

// ---------- An iPhone: the guide, the profile on the clipboard, the app takes it over ----------
let copied = "";
{
  const { context, page } = await open({ ios: true, phone: true });
  await setUp(page);
  await page.waitForSelector(".ob-install >> visible=true", { timeout: 8000 }).catch(() => failures.push("no step „App\" on an iPhone"));
  const label = await page.evaluate(() => onPage(".ob-install")?.textContent.trim());
  check(label === "Auf den Home-Bildschirm", `the button on an iPhone: ${label}`);
  await page.click(".ob-install >> visible=true");
  await page.waitForSelector(".inst-dlg[open]", { timeout: 5000 }).catch(() => failures.push("no guide on an iPhone"));
  await page.waitForSelector(".inst-got", { timeout: 5000 }).catch(() => failures.push("the guide does not say the profile was copied"));
  const guide = await page.evaluate(() => ({ title: document.querySelector(".inst-dlg #inst-title")?.textContent, steps: [...document.querySelectorAll(".inst-dlg .inst-steps li")].map((li) => li.textContent) }));
  check(guide.title === "Betula auf den Home-Bildschirm" && guide.steps.length === 3 && /Teilen/.test(guide.steps[0]) && /Zum Home-Bildschirm/.test(guide.steps[1]) && /Studium einfügen/.test(guide.steps[2]), `the guide: ${JSON.stringify(guide)}`);
  copied = await page.evaluate(() => navigator.clipboard.readText());
  check(/"betula": "profile"/.test(copied) && /betula\.myprogram\.v1/.test(copied), "the clipboard does not hold the profile");
  await page.click(".inst-dlg .st-dlg-foot .btn.primary");
  check(await noteSaid(page), "„Verstanden\" does not end the setup");
  check(new RegExp(`^later\\t${today()}$`, "m").test(await kept(page)), "the guide's „Verstanden\" is not „Später\"");
  await context.close();
}
{
  // The app on the home screen: storage of its own, empty.
  const { context, page } = await open({ ios: true, installed: true, phone: true });
  check(/^installed$/m.test(await kept(page)), "the app on the home screen is not counted as installed");
  await page.waitForSelector(".ob-take >> visible=true", { timeout: 8000 }).catch(() => failures.push("no „Studium einfügen\" in the app's first start"));
  // Something else on the clipboard: said, nothing taken.
  await page.evaluate(() => navigator.clipboard.writeText("Einkaufsliste"));
  await page.click(".ob-take-btn");
  await page.waitForSelector(".ob-take .ob-note", { timeout: 5000 }).catch(() => failures.push("something else on the clipboard is not said"));
  check(!(await page.evaluate(() => localStorage.getItem("betula.myprogram.v1"))), "something else on the clipboard was taken");
  // The profile Safari put there: the day.
  await page.evaluate((text) => navigator.clipboard.writeText(text), copied);
  await page.click(".ob-take-btn");
  await page.waitForSelector(".day-hero:not(.day-first) >> visible=true", { timeout: 10000 }).catch(() => failures.push("„Studium einfügen\" does not lead to the day"));
  check(/Informatik/.test(await page.evaluate(() => localStorage.getItem("betula.myprogram.v1") || "")), "the study did not come over");
  check((await card(page)) === null && !(await page.evaluate(() => Boolean(onPage(".ob-install")))), "the app offers itself");
  await context.close();
}

// ---------- Running installed: nothing offered ----------
{
  const { context, page } = await open({ standalone: true });
  await offer(page);
  await setUp(page);
  check(await noteSaid(page), "running installed: „Studium anlegen\" does not end the setup");
  await page.waitForTimeout(300);
  check((await card(page)) === null, "running installed: a card");
  check(/^installed$/m.test(await kept(page)), "running installed is not counted as installed");
  await context.close();
}

// ---------- Taken off since: the browser's offer at load outweighs „installed" kept ----------
{
  const { context, page } = await open({ seed: { [KEY]: "installed\n" }, offerAtLoad: true });
  check(await page.evaluate(() => window.betulaInstall?.can() === true), "the offer at load is not held");
  check(!/^installed$/m.test(await kept(page)), `„installed\" stays after the browser offers its dialog: ${JSON.stringify(await kept(page))}`);
  await setUp(page);
  await page.waitForSelector(".ob-install >> visible=true", { timeout: 8000 }).catch(() => failures.push("taken off since: no step „App\" though the browser offers its dialog"));
  await context.close();
}

await browser.close();
check(asked.length === 0, `requests about installing: ${asked.join(", ")}`);
check(errors.length === 0, `errors on the page: ${errors.join(" | ")}`);
if (failures.length) {
  console.error(`install: ${failures.length} problem(s)`);
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("install: ok");
