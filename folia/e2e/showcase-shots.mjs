// The screenshots of the pages about Betula (folia/assets/shots/*.webp, served under
// /assets/shots/ and embedded in the binary): the carousel of the overview — the start page of a
// study set up („Dein Tag"), the catalog, „Mein Studium" and the Stundenplan —, each light and
// dark, for wide screens (4:3) and phones (3:4), and a module's page on a phone for „Technik".
// Run it against a local build whenever those pages look different, then rebuild Folia:
//   node e2e/showcase-shots.mjs [base-url]      (needs radix serve-snapshot + folia, the browser app built)
// The pictures are taken after the browser app has taken over, at twice the pixels, scaled down to
// what the carousel shows on a sharp screen and written as WebP by the browser itself.
//
// The pages of a study are those of an example: the script walks the start page's setup once
// (Informatik B.Sc., begun a year ago, the Fachsemester's modules as the Regelstudienplan has
// them) and marks three modules, and every picture of a study opens with what that browser kept.
// The day stands still at SHOTS_NOW, a weekday of the lecture period of the snapshot's semester
// with something still to come: give another one when the snapshot is of a later semester.
import { writeFileSync, mkdirSync } from "node:fs";
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const out = new URL("../assets/shots/", import.meta.url);
mkdirSync(out, { recursive: true });
const now = new Date(process.env.SHOTS_NOW || "2026-11-16T12:20:00+01:00");

// clip in CSS pixels; width: the width of the file in pixels; study: with the example study.
const shots = [
  { name: "day", url: "/", study: true, w: 1280, h: 960, clip: { x: 52, y: 0, width: 1228, height: 921 }, width: 1200 },
  { name: "catalog", url: "/catalog?q=datenbank&open=12330", w: 1280, h: 900, clip: { x: 322, y: 0, width: 958, height: 718 }, width: 1200 },
  { name: "study", url: "/study", study: true, w: 1280, h: 900, clip: { x: 322, y: 0, width: 958, height: 718 }, width: 1200 },
  { name: "timetable", url: "/studyplan", study: true, w: 1280, h: 900, clip: { x: 322, y: 0, width: 958, height: 718 }, width: 1200 },
  { name: "day-phone", url: "/", study: true, w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
  { name: "catalog-phone", url: "/catalog?q=datenbank", w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
  { name: "study-phone", url: "/study", study: true, w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
  { name: "timetable-phone", url: "/studyplan", study: true, w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
  { name: "module-phone", url: "/catalog/module/12330", w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
];
// The modules the example marks, by their numbers.
const marked = ["12330", "11374", "11406"];

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const started = (page) => page.waitForFunction(() => window.__betulaApp === true && window.betulaAnswered === null, null, { timeout: 60000 });
const place = { locale: "de-DE", timezoneId: "Europe/Berlin" };

// The example study: the setup's three steps, then the marks. What the browser keeps of it
// (`localStorage`) is what every picture of a study starts with.
const study = await (async () => {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 }, ...place });
  const page = await context.newPage();
  await page.clock.setFixedTime(now);
  await page.goto(base + "/", { waitUntil: "load" });
  await started(page);
  await page.click("#ob-program >> visible=true");
  await page.waitForSelector("#ob-program-search >> visible=true");
  await page.keyboard.type("informatik bsc");
  await page.waitForFunction(() => /^Informatik/.test(document.querySelector("#ob-program-option-0")?.textContent.trim() ?? ""), null, { timeout: 10000 });
  await page.keyboard.press("Enter");
  // The second of the starts offered: a year ago, so the third Fachsemester with two behind it.
  await page.click('.ob-sem[role="radio"] >> visible=true >> nth=1');
  await page.click(".ob-next >> visible=true");
  await page.click(".ob-create >> visible=true");
  await page.waitForSelector(".wait-note.setup-note >> visible=true", { timeout: 10000 });
  for (const id of marked) {
    await page.goto(`${base}/catalog/module/${id}`, { waitUntil: "load" });
    await started(page);
    await page.waitForTimeout(600);
    await page.keyboard.press("m");
    await page.waitForTimeout(400);
  }
  const state = await context.storageState();
  await context.close();
  return state;
})();

const encoder = await browser.newPage();
for (const dark of [false, true]) {
  for (const shot of shots) {
    const context = await browser.newContext({
      viewport: { width: shot.w, height: shot.h },
      deviceScaleFactor: 2,
      colorScheme: dark ? "dark" : "light",
      isMobile: !!shot.phone,
      hasTouch: !!shot.phone,
      storageState: shot.study ? study : undefined,
      ...place,
    });
    const page = await context.newPage();
    await page.clock.setFixedTime(now);
    await page.goto(base + shot.url, { waitUntil: "networkidle" });
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
    await page.waitForTimeout(1500);
    const png = await page.screenshot({ clip: shot.clip, animations: "disabled", caret: "hide" });
    await context.close();
    const webp = await encoder.evaluate(async ({ data, width }) => {
      const image = new Image();
      image.src = "data:image/png;base64," + data;
      await image.decode();
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = Math.round((image.height * width) / image.width);
      const context = canvas.getContext("2d");
      context.imageSmoothingQuality = "high";
      context.drawImage(image, 0, 0, canvas.width, canvas.height);
      return canvas.toDataURL("image/webp", 0.82).split(",")[1];
    }, { data: png.toString("base64"), width: shot.width });
    const file = new URL(`${shot.name}${dark ? "-dark" : ""}.webp`, out);
    writeFileSync(file, Buffer.from(webp, "base64"));
    console.log(file.pathname, Math.round(Buffer.from(webp, "base64").length / 1024), "kB");
  }
}
await browser.close();
