// Screenshots for design reviews. Usage:
//   node shot.mjs <url> <out.png> [width] [height] [--dark] [--full] [--click=<selector>] [--scale=2]
// Uses an installed Edge/Chrome through SMOKE_BROWSER_CHANNEL (default msedge).
import { chromium } from "playwright-core";

const [url, out, width = "1440", height = "900", ...flags] = process.argv.slice(2);
if (!url || !out) {
  console.error("usage: node shot.mjs <url> <out.png> [width] [height] [--dark] [--full] [--click=<selector>]");
  process.exit(2);
}
const flag = (name) => flags.find((f) => f === `--${name}` || f.startsWith(`--${name}=`));
const value = (name) => flag(name)?.split("=").slice(1).join("=");

const browser = await chromium.launch({ channel: process.env.SMOKE_BROWSER_CHANNEL || "msedge" });
const page = await browser.newPage({
  viewport: { width: Number(width), height: Number(height) },
  deviceScaleFactor: Number(value("scale") || 1),
  colorScheme: flag("dark") ? "dark" : "light",
  isMobile: Number(width) < 600,
  hasTouch: Number(width) < 600,
});
await page.goto(url, { waitUntil: "networkidle" });
if (flag("dark")) await page.evaluate(() => (document.documentElement.dataset.theme = "dark"));
for (const f of flags.filter((f) => f.startsWith("--click="))) {
  await page.click(f.slice("--click=".length));
  await page.waitForTimeout(450);
}
for (const f of flags.filter((f) => f.startsWith("--attr="))) {
  const [name, val] = f.slice("--attr=".length).split(":");
  await page.evaluate(([n, v]) => (document.documentElement.dataset[n] = v), [name, val]);
}
await page.waitForTimeout(300);
await page.screenshot({ path: out, fullPage: Boolean(flag("full")) });
await browser.close();
console.log(out);
