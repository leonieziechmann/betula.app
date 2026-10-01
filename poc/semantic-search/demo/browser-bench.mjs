// node browser-bench.mjs URL [CHROMIUM]
//
// Opens the demo (served from a folder holding index.html, e5.js, e5_mini.wasm, e5-de-en.bin,
// index.bin, index.json) in Chromium and times queries typed into it, with the CPU at full
// speed and slowed down 4× and 6× (DevTools' throttling, the usual stand-in for a mid-range
// and a low-end phone). Needs playwright-core.

import { chromium } from "playwright-core";

const [url, executablePath = "/opt/pw-browsers/chromium-1194/chrome-linux/chrome"] = process.argv.slice(2);
const queries = ["Statik", "maschinelles lernen", "wie berechne ich ob eine brücke hält",
  "I am looking for a course about the history of architecture and urban planning"];

const browser = await chromium.launch({ executablePath });
const page = await browser.newPage();
const cdp = await page.context().newCDPSession(page);
for (const rate of [1, 4, 6]) {
  await cdp.send("Emulation.setCPUThrottlingRate", { rate });
  await page.goto(url);
  await page.waitForFunction(() => document.getElementById("status").textContent.startsWith("Bereit"), null, { timeout: 120000 });
  const ready = await page.textContent("#status");
  console.log(`\nCPU ×${rate}: ${ready}`);
  for (const q of queries) {
    const times = [];
    for (let i = 0; i < 5; i++) {
      await page.fill("#q", "");
      await page.fill("#q", q + " ".repeat(i)); // a new value each time, trimmed away
      await page.waitForFunction(() => document.getElementById("status").textContent.includes("Embedding"));
      const status = await page.textContent("#status");
      times.push(Number(status.match(/Embedding (\d+) ms/)[1]));
      await page.evaluate(() => (document.getElementById("status").textContent = ""));
    }
    times.sort((a, b) => a - b);
    const top = await page.textContent("#hits li");
    console.log(`  ${String(times[2]).padStart(5)} ms  ${q}  →  ${top}`);
  }
}
await browser.close();
