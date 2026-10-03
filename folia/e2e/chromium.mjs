// The checks launch an installed Edge (SMOKE_BROWSER_CHANNEL=msedge by default). Where there is
// none, this stands in for it: run any check with `node --import ./chromium.mjs <check>.mjs`, and
// it launches Playwright's own Chromium (`npx playwright-core install chromium`), or the browser
// SMOKE_BROWSER_PATH names (a Chromium or Chrome executable).
import { chromium } from "playwright-core";
const launch = chromium.launch.bind(chromium);
chromium.launch = (options = {}) => launch({ ...options, channel: undefined, executablePath: process.env.SMOKE_BROWSER_PATH || undefined });
