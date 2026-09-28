// The landing page in the browser app. A flat first panel (text; the figures beside it, about
// equally wide), then the carousel: the current picture in the middle, its neighbours at the
// sides, going round. It turns on by itself until the pause button stops it, and by the arrows,
// the tabs, the keys and a click on a neighbour; a click on the current screenshot opens its page,
// a click on the map opens the map large in a dialog (the legend beside it on a wide screen, the
// page behind it standing still). There the map is drawn from what the server laid out; a pointer
// over a dot shows its relatives in the caption, a click picks it (the outline of its faculty, the
// others step back, the caption links to it) instead of opening it, and that link opens the
// program without loading a page. Only the pictures that show are fetched. There is no sidebar
// (owner, 2026-09-28): under the first panel the way in for a first visit, three steps into the
// programs, the catalog and the Stundenplan that follow what this browser has done, with the way
// into the search at the top and, at its foot, the jumps to the sections. The questions open in
// place, „Betula im Detail" follows them with its chapters and every filter (on a phone the
// board's first groups, the others a tap away), the head describes the page shown.
// Needs radix serve-snapshot + folia running and a fresh `bash scripts/build-client.sh`.
//   node e2e/home.mjs [base-url]
import { chromium } from "playwright-core";

const base = process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080";
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
const errors = [];
page.on("pageerror", (error) => errors.push(String(error)));
const images = [];
page.on("request", (request) => { if (request.url().includes("/assets/shots/")) images.push(new URL(request.url()).pathname); });
const current = () => page.evaluate(() => document.querySelector(".cslide.is-current .cap-text b")?.textContent);
const waitFor = (title) => page.waitForFunction((t) => document.querySelector(".cslide.is-current .cap-text b")?.textContent === t, title, { timeout: 5000 });

// Without the app: the map is part of the server's HTML (the picture, and the dialog whose dots
// are links), every picture is a link, the screenshots are lazy (the carousel's and the two of
// „Geräte und Sprachen", each in both themes).
const html = await (await fetch(base + "/")).text();
check(/<svg[^>]*class="map map-wide"/.test(html) && /<svg[^>]*class="map map-tall"/.test(html), "server HTML has no map");
check(/<svg[^>]*class="map map-preview map-wide"/.test(html), "server HTML has no picture of the map");
check(/<a href="\/programs\/[^"]+\/plan"[^>]*class="map-dot /.test(html), "server HTML: dots are not links to programs");
check((html.match(/class="cslide /g) || []).length === 5, "server HTML: not four pictures and the spacer");
check((html.match(/loading="lazy"/g) || []).length === 10, "server HTML: the screenshots are not all lazy");
check(html.includes('href="/impressum"') && html.includes('href="/datenschutz"'), "server HTML: no legal links");
check(!html.includes("Was bedeutet FÜS?") && html.includes("Wie finde ich die Module für mein Studium?"), "server HTML: the questions are the old ones");
check(html.includes('"@type":"WebApplication"') && html.includes('id="im-detail"') && (html.match(/class="panel feature t-/g) || []).length === 8, "server HTML: no app in the structured data, or no „Betula im Detail\"");
check(html.includes('<dl class="birch">') && !html.includes('class="examples"'), "server HTML: the figures are not the stack, or the example searches are still there");

await page.goto(base + "/", { waitUntil: "networkidle" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
await page.waitForSelector(".home .map-preview");
// A page load would lose this (the method of spa.mjs).
await page.evaluate(() => { window.__marker = 1; });
check(await page.evaluate(() => typeof window.betulaMap === "string" && window.betulaMap.length > 1000), "boot.js did not hand the map to the app");
check(await page.evaluate(() => document.querySelectorAll("head meta[name=description]").length) === 1, "the head has not exactly one description after the takeover");
check(await page.evaluate(() => document.querySelectorAll("head link[rel=canonical]").length) === 1, "the head has not exactly one canonical address after the takeover");
// Two stylesheets could come from different places (the service worker, the network) and mix.
check(await page.evaluate(() => document.querySelectorAll("link[rel=stylesheet]").length) === 1, "the document has not exactly one stylesheet after the takeover");
check(await page.evaluate(() => document.querySelectorAll("link[rel=preload]").length) === 1, "the document has not exactly one preloaded font after the takeover");
check(await page.evaluate(() => /^\d+\.\d+\.\d+/.test(document.querySelector(".ground .ver")?.textContent || "")), "the ground does not name Folia's version");
// The first panel is flat: the figures stand beside the text, one under the other, and the ones
// with several digits are about equally wide (the single digit only grows as large as three).
const hero = await page.evaluate(() => {
  const text = document.querySelector(".home-hero-text").getBoundingClientRect();
  const side = document.querySelector(".birch").getBoundingClientRect();
  const widths = [...document.querySelectorAll(".birch dd")].filter((dd) => dd.textContent.length > 1).map((dd) => {
    const range = document.createRange();
    range.selectNodeContents(dd);
    return range.getBoundingClientRect().width;
  });
  const lefts = [...document.querySelectorAll(".birch dt")].map((dt) => Math.round(dt.getBoundingClientRect().left));
  return { height: document.querySelector(".home-hero").getBoundingClientRect().height, beside: side.left >= text.right - 1, spread: Math.max(...widths) / Math.min(...widths), names: new Set(lefts).size };
});
check(hero.beside && hero.height < 340, `the first panel is not flat: ${JSON.stringify(hero)}`);
check(hero.spread < 1.08 && hero.names === 1, `the figures are not equally wide, or their names not on one line: ${JSON.stringify(hero)}`);

// No sidebar, and under the first panel the way in: three steps side by side, the first the next
// one for a browser that has done nothing yet, each naming the item of the navigation that keeps it.
const way = await page.evaluate(() => ({
  sidebar: document.querySelector("#sidebar") !== null,
  after: document.querySelector(".home-hero").nextElementSibling?.id,
  steps: [...document.querySelectorAll(".start-step a")].map((a) => a.getAttribute("href")).join(),
  row: new Set([...document.querySelectorAll(".start-step")].map((step) => Math.round(step.getBoundingClientRect().top))).size,
  next: document.querySelector(".start-step.is-next a")?.getAttribute("href"),
  done: document.querySelectorAll(".start-step.is-done").length,
  places: [...document.querySelectorAll(".start-place")].map((place) => place.textContent).join(),
}));
check(!way.sidebar && way.after === "loslegen" && way.steps === "/programs,/catalog,/studyplan" && way.row === 1, `the way in is not under the first panel: ${JSON.stringify(way)}`);
check(way.next === "/programs" && way.done === 0 && way.places === "Studium,Module,Merkliste,Stundenplan", `the way in does not start at the program: ${JSON.stringify(way)}`);
// „Direkt suchen" goes into the search at the top, not to the catalog.
await page.click(".start-path a[data-action=search]");
check(await page.evaluate(() => document.activeElement?.id === "topsearch" && location.pathname === "/"), "„Direkt suchen\" does not go into the search");
await page.evaluate(() => document.activeElement?.blur());

// The carousel: the map in the middle, a neighbour on each side.
check((await current()) === "Die Karte", `the first picture is not the map: ${await current()}`);
const places = await page.evaluate(() => [...document.querySelectorAll(".cslide:not(.spacer)")].map((slide) => getComputedStyle(slide).getPropertyValue("--at").trim()));
check(JSON.stringify(places) === JSON.stringify(["0", "1", "-2", "-1"]), `the pictures are not placed around the current one: ${places}`);
const sides = await page.evaluate(() => {
  const frame = document.querySelector(".carousel").getBoundingClientRect();
  return [...document.querySelectorAll(".cslide.is-side")].map((slide) => { const r = slide.getBoundingClientRect(); return r.right > frame.left + 20 && r.left < frame.right - 20; });
});
check(sides.length === 2 && sides.every(Boolean), `the neighbours do not show at the sides: ${sides}`);
// Lazy pictures: what shows beside the map is fetched, never the hidden theme or the phone's.
check(images.every((path) => !path.includes("-dark") && !path.includes("-phone")), `pictures of the hidden theme or the phone were fetched: ${images}`);

// The arrows, a click on a neighbour, a tab, the keys; it goes round.
await page.click(".show-arrow.next");
await waitFor("Der Katalog");
// A neighbour lies half behind the current picture: a click on the half that shows.
const right = page.locator(".cslide.is-side >> nth=1");
const size = await right.boundingBox();
await right.click({ position: { x: size.width * 0.85, y: size.height * 0.4 } });
await waitFor("Der Regelstudienplan");
await page.click(".show-tabs button:has-text('Modul')");
await waitFor("Ein Modul");
await page.focus(".carousel");
await page.keyboard.press("ArrowRight");
await waitFor("Die Karte");
await page.keyboard.press("ArrowLeft");
await waitFor("Ein Modul");
check(await page.evaluate(() => document.querySelector(".show-tabs button[aria-checked='true']")?.textContent.startsWith("Modul")), "the tab does not follow");
check(await page.evaluate(() => location.pathname === "/" && location.hash === ""), "turning the pictures changed the address");

check(await page.evaluate(() => document.querySelector(".show-tabs").style.getPropertyValue("--i") === "3"), "the mark of the tabs does not stand at the current one");

// It turns on by itself, the pointer on it or not; the pause button stops it and starts it again.
await page.hover(".cslide.is-current");
await page.waitForFunction(() => document.querySelector(".cslide.is-current .cap-text b")?.textContent === "Die Karte", null, { timeout: 12000 }).catch(() => failures.push("the carousel does not turn by itself"));
check((await page.getAttribute(".show-play", "aria-label")) === "Bilder anhalten", "the pictures do not play from the start");
await page.click(".show-play");
check((await page.getAttribute(".show-play", "aria-label")) === "Bilder abspielen", "the pause button does not offer to play again");
check(await page.evaluate(() => localStorage.getItem("betula.showcase") === "paused"), "the browser does not remember the stop");
const held = await current();
await page.waitForTimeout(8000);
check((await current()) === held, "the carousel turns while paused");
await page.click(".show-play");
await page.waitForFunction((t) => document.querySelector(".cslide.is-current .cap-text b")?.textContent !== t, held, { timeout: 12000 }).catch(() => failures.push("the play button does not start it again"));
check(await page.evaluate(() => localStorage.getItem("betula.showcase") === null), "playing again leaves something in the browser");

// A click on the map opens it large; there a pointer shows a program, a click picks it.
if ((await current()) !== "Die Karte") { await page.click(".show-tabs button:has-text('Karte')"); await waitFor("Die Karte"); }
// Where the page stands when the click reaches it: Playwright now and then scrolls the page before
// it clicks (313 px, even with the map in full view), which is no doing of the page; measured
// before the click, that used to fail the check of the wheel below.
await page.evaluate(() => addEventListener("pointerdown", () => { window.__atClick = document.getElementById("page-scroll").scrollTop; }, { capture: true, once: true }));
await page.click(".cslide.is-current");
await page.waitForSelector(".map-dialog[open]");
const scrolled = await page.evaluate(() => document.getElementById("page-scroll").scrollTop);
check(await page.evaluate((top) => window.__atClick === top, scrolled), "opening the map moves the page behind it");
check(await page.evaluate(() => location.pathname === "/"), "a click on the map navigated");
check(await page.evaluate(() => document.querySelector(".showcase").classList.contains("paused")), "the carousel does not stand still while the map is open");
// A wide screen: the legend and the caption beside the map, the map 4:3 as high as it can be.
const layout = await page.evaluate(() => {
  const box = (s) => document.querySelector(s).getBoundingClientRect();
  const key = box(".map-dialog .live-key"), cap = box(".map-dialog .map-cap"), map = box(".map-dialog .map-live");
  return { beside: key.right <= map.left && cap.right <= map.left, ratio: map.width / map.height };
});
check(layout.beside && Math.abs(layout.ratio - 4 / 3) < 0.02, `the legend is not beside the map: ${JSON.stringify(layout)}`);
// The wheel over the dim backdrop does not move the page behind.
await page.mouse.move(8, 500);
await page.mouse.wheel(0, 800);
await page.waitForTimeout(300);
check(await page.evaluate((top) => document.getElementById("page-scroll").scrollTop === top, scrolled), "the page scrolls behind the open map");
const dot = page.locator(".map-dialog .map-wide .map-dot.bachelor").first();
const name = (await dot.getAttribute("aria-label")).split(" · ")[0];
await dot.hover();
await page.waitForSelector(".map-dialog .map-wide.has-hot .map-hot path");
check((await page.textContent(".map-dialog .map-cap")).includes(name), "the caption does not name the program under the pointer");
const href = await dot.getAttribute("href");
await dot.click();
await page.waitForSelector(".map-dialog .cap-link.picked");
check(await page.evaluate(() => document.querySelectorAll(".map-dialog .map-wide .map-region.shown").length) === 1, "not exactly one faculty outline is shown");
check(await page.evaluate(() => document.querySelectorAll(".map-dialog .map-wide .map-dot.outside").length) > 20, "the programs of the other faculties do not step back");
// The first Escape puts the pick away, the next one closes the dialog.
await page.keyboard.press("Escape");
await page.waitForFunction(() => !document.querySelector(".map-dialog .cap-link.picked"));
check(await page.evaluate(() => document.querySelector(".map-dialog").open), "the first Escape closed the dialog");
await page.keyboard.press("Escape");
await page.waitForFunction(() => !document.querySelector(".map-dialog").open);
// The ✕ closes it too; the caption's link opens the program in the app.
await page.click(".cslide.is-current");
await page.waitForSelector(".map-dialog[open]");
await page.click(".map-dialog-close");
await page.waitForFunction(() => !document.querySelector(".map-dialog").open);
await page.click(".cslide.is-current");
await page.waitForSelector(".map-dialog[open]");
await dot.click();
await page.click(".map-dialog .cap-link.picked");
await page.waitForFunction((path) => location.pathname === path, href);
await page.waitForSelector("[data-walk='program-page']");
check(await page.evaluate(() => window.__marker === 1), "the program was opened by a page load");
check(await page.evaluate((path) => document.querySelector("head link[rel=canonical]")?.href.endsWith(path), href), "the canonical address did not follow to the program");
check(await page.evaluate(() => document.querySelectorAll("head meta[name=description]").length) === 1, "two descriptions after a navigation");

// A click on the current screenshot opens its page in the app.
await page.goBack();
await page.waitForSelector(".home .map-preview");
await page.click(".show-arrow.next");
await waitFor("Der Katalog");
await page.click(".cslide.is-current");
await page.waitForFunction(() => location.pathname === "/catalog");
check(await page.evaluate(() => window.__marker === 1), "the catalog was opened by a page load");
await page.goBack();
await page.waitForSelector(".home .map-preview");

// The foot of the way in jumps to the questions, without a history entry; a question opens in place.
await page.click(".start-foot a[href='#fragen']");
await page.waitForFunction(() => { const top = document.getElementById("fragen").getBoundingClientRect().top; return top >= 0 && top < 160; });
check(await page.evaluate(() => location.pathname === "/" && location.hash === ""), "a jump of the way in changed the address");
await page.click(".faq summary >> nth=1");
check(await page.evaluate(() => document.querySelectorAll(".faq details[open]").length === 1), "a question does not open");
check(await page.evaluate(() => document.querySelectorAll(".faq-group").length === 3), "the questions are not in three groups");
// After the questions „Betula im Detail": its row of chapters, the chapters, and every group of
// the filter panel with its chips leading into the catalog.
const detail = await page.evaluate(() => ({
  nav: [...document.querySelectorAll("#im-detail .detail-nav a")].map((a) => a.getAttribute("href")).join(),
  chapters: [...document.querySelectorAll(".home .feature")].map((section) => "#" + section.id).join(),
  groups: document.querySelectorAll(".board .bgroup").length,
  chips: [...document.querySelectorAll(".board a.chip")].every((a) => a.getAttribute("href").startsWith("/catalog?")),
  example: document.querySelector(".board-foot a")?.getAttribute("href"),
  links: [...document.querySelectorAll(".feature-link")].map((a) => a.getAttribute("href")).join(),
}));
check(detail.nav === detail.chapters && detail.chapters.split(",").length === 8 && detail.groups === 12 && detail.chips, `„Betula im Detail" is not what it was: ${JSON.stringify(detail)}`);
check(/^\/catalog\?/.test(detail.example || "") && detail.links === "/catalog,/programs,/studyplan,/datenschutz", `„Betula im Detail" leads nowhere: ${JSON.stringify(detail)}`);
// Its last chapter shows the catalog and a module on a phone: in the theme shown only.
await page.evaluate(() => document.getElementById("geraete").scrollIntoView({ block: "center" }));
await page.waitForFunction(() => [...document.querySelectorAll(".devices img")].filter((img) => img.checkVisibility()).every((img) => img.complete && img.naturalWidth > 0));
check(images.every((path) => !path.includes("-dark") && (!path.includes("-phone") || path.endsWith("/module-phone.webp"))), `wrong pictures fetched: ${images}`);
check(await page.evaluate(() => [...document.querySelectorAll(".devices img")].filter((img) => img.checkVisibility()).length) === 2, "„Geräte und Sprachen\" does not show one theme's pictures");

// What this browser has done shows on the way in: with a program set as „Mein Studiengang" and a
// module marked those two steps are done and name it, and the Stundenplan is the next step.
await page.evaluate(() => {
  localStorage.setItem("betula.myprogram.v1", "program\t079-82-2008\nname\tInformatik B.Sc. · PO 2008\ncaption\t\n");
  localStorage.setItem("betula.bookmarks.v1", "12330\t1\n");
});
await page.goto(base + "/", { waitUntil: "networkidle" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
await page.waitForFunction(() => document.querySelectorAll(".start-step.is-done").length === 2, null, { timeout: 5000 }).catch(() => failures.push("the way in does not follow what this browser has done"));
const kept = await page.evaluate(() => ({
  program: document.querySelector(".start-step:nth-child(1) .start-state")?.textContent,
  href: document.querySelector(".start-step:nth-child(1) a")?.getAttribute("href"),
  marked: document.querySelector(".start-step:nth-child(2) .start-state")?.textContent,
  next: document.querySelector(".start-step.is-next a")?.getAttribute("href"),
}));
check(kept.program?.startsWith("Informatik B.Sc.") && kept.href?.startsWith("/programs/") && kept.marked === "1 Modul gemerkt" && kept.next === "/studyplan", `the way in does not show what was done: ${JSON.stringify(kept)}`);
await page.evaluate(() => { localStorage.removeItem("betula.myprogram.v1"); localStorage.removeItem("betula.bookmarks.v1"); });

// A phone: pictures upright, the tall sheet, a swipe turns them, nothing scrolls sideways. A stop
// is remembered over a page load.
await page.evaluate(() => localStorage.setItem("betula.showcase", "paused"));
await page.setViewportSize({ width: 390, height: 844 });
await page.goto(base + "/", { waitUntil: "networkidle" });
await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
await page.waitForSelector(".home .map-preview.map-tall");
check(await page.evaluate(() => document.querySelector(".showcase").classList.contains("paused")), "a remembered stop is forgotten over a page load");
await page.evaluate(() => localStorage.removeItem("betula.showcase"));
const shown = await page.evaluate(() => [".map-preview.map-wide", ".map-preview.map-tall"].map((s) => getComputedStyle(document.querySelector(s)).display));
check(shown[0] === "none" && shown[1] !== "none", `phone: the wrong sheet is shown (${shown})`);
const stage = await page.evaluate(() => { const r = document.querySelector(".cslide.is-current .stage").getBoundingClientRect(); return r.height / r.width; });
check(stage > 1.2, `phone: the pictures do not stand upright (${stage})`);
await page.locator(".carousel").scrollIntoViewIfNeeded();
const box = await page.locator(".carousel").boundingBox();
await page.mouse.move(box.x + box.width * 0.8, box.y + 150);
await page.mouse.down();
await page.mouse.move(box.x + box.width * 0.2, box.y + 160, { steps: 6 });
await page.mouse.up();
await waitFor("Der Katalog").catch(() => failures.push("phone: a swipe does not turn the pictures"));
check(await page.evaluate(() => location.pathname === "/"), "phone: a swipe opened a page");
check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), "phone: the page scrolls sideways");
// The name, not the button (its hit area reaches past it).
check(await page.evaluate(() => [...document.querySelectorAll(".show-tabs button")].every((b) => {
  const range = document.createRange();
  range.selectNodeContents(b);
  return range.getBoundingClientRect().width <= b.clientWidth;
})), "phone: a tab is cut off");
check(await page.isVisible(".show-play"), "phone: no pause button");
// The board of the filters: the groups of a first look, the others after „Alle Filter zeigen".
const groups = () => page.evaluate(() => [...document.querySelectorAll(".board .bgroup")].filter((group) => group.checkVisibility()).length);
check((await groups()) === 5, `phone: the board does not start with its first groups (${await groups()})`);
await page.locator(".board-more").scrollIntoViewIfNeeded();
await page.click(".board-more");
check((await groups()) === 12 && !(await page.isVisible(".board-more")), `phone: „Alle Filter zeigen" does not show all filters (${await groups()})`);
check(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), "phone: „Betula im Detail\" scrolls sideways");

check(errors.length === 0, `page errors: ${errors.join(" | ")}`);
await browser.close();
if (failures.length) {
  console.error(failures.map((f) => "FAIL " + f).join("\n"));
  process.exit(1);
}
console.log("home: ok");
