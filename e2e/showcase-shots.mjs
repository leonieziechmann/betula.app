// The screenshots of the start page's carousel (app/assets/shots/*.webp, served under
// /assets/shots/ and embedded in the binary): the catalog, a study plan and a module page, each
// light and dark, for wide screens (4:3) and phones (3:4). Run it against a local build whenever
// those pages look different, then rebuild Folia:
//   node e2e/showcase-shots.mjs [base-url]      (needs radix serve-snapshot + folia, the browser app built)
// The pictures are taken after the browser app has taken over, at twice the pixels, scaled down to
// what the carousel shows on a sharp screen and written as WebP by the browser itself.
import { writeFileSync, mkdirSync } from "node:fs";
import { chromium } from "playwright-core";

const base = (process.argv[2] || process.env.SMOKE_BASE_URL || "http://127.0.0.1:8080").replace(/\/$/, "");
const out = new URL("../app/assets/shots/", import.meta.url);
mkdirSync(out, { recursive: true });

// clip in CSS pixels; width: the width of the file in pixels.
const shots = [
  { name: "catalog", url: "/catalog?q=datenbank&open=12330", w: 1280, h: 900, clip: { x: 322, y: 0, width: 958, height: 718 }, width: 1200 },
  { name: "program", url: "/programs/bachelor-informatik-2008/plan", w: 1600, h: 1000, clip: { x: 326, y: 50, width: 800, height: 600 }, width: 1200 },
  { name: "module", url: "/catalog/module/12330", w: 1280, h: 900, clip: { x: 322, y: 0, width: 958, height: 718 }, width: 1200 },
  { name: "catalog-phone", url: "/catalog?q=datenbank", w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
  { name: "program-phone", url: "/programs/bachelor-informatik-2008/plan", w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
  { name: "module-phone", url: "/catalog/module/12330", w: 390, h: 780, phone: true, clip: { x: 0, y: 0, width: 390, height: 520 }, width: 720 },
];

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const encoder = await browser.newPage();
for (const dark of [false, true]) {
  for (const shot of shots) {
    const page = await browser.newPage({
      viewport: { width: shot.w, height: shot.h },
      deviceScaleFactor: 2,
      colorScheme: dark ? "dark" : "light",
      isMobile: !!shot.phone,
      hasTouch: !!shot.phone,
    });
    await page.goto(base + shot.url, { waitUntil: "networkidle" });
    await page.waitForFunction(() => window.__betulaApp === true, null, { timeout: 60000 });
    await page.waitForTimeout(1200);
    const png = await page.screenshot({ clip: shot.clip, animations: "disabled", caret: "hide" });
    await page.close();
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
