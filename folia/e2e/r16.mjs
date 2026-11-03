// The misses of R16 (docs/folia/frontend.md) while a check runs, against the app scripts/r16.sh
// builds: every console line „BETULA-MISSED …" of the check's pages on stderr, with the check and
// the page, and appended to $R16_LOG where it is set. It launches as chromium.mjs does
// (Playwright's Chromium, or the browser SMOKE_BROWSER_PATH names).
//   cd e2e && node --import ./r16.mjs study.mjs [base-url]
import { appendFileSync } from "node:fs";
import { basename } from "node:path";
import { chromium } from "playwright-core";

const check = basename(process.argv[1] ?? "?", ".mjs");
const watched = new WeakSet();
const watch = (page) => {
  // Once a page: `browser.newPage` may go through `newContext` as well.
  if (watched.has(page)) return;
  watched.add(page);
  page.on("console", (message) => {
    const text = message.text();
    if (!text.includes("BETULA-MISSED")) return;
    const line = `${check}\t${new URL(page.url()).pathname}\t${text}`;
    console.error(line);
    if (process.env.R16_LOG) appendFileSync(process.env.R16_LOG, line + "\n");
  });
};
const launch = chromium.launch.bind(chromium);
chromium.launch = async (options = {}) => {
  const browser = await launch({ ...options, channel: undefined, executablePath: process.env.SMOKE_BROWSER_PATH || undefined });
  const newContext = browser.newContext.bind(browser);
  browser.newContext = async (...args) => {
    const context = await newContext(...args);
    context.on("page", watch);
    return context;
  };
  const newPage = browser.newPage.bind(browser);
  browser.newPage = async (...args) => {
    const page = await newPage(...args);
    watch(page);
    return page;
  };
  return browser;
};
