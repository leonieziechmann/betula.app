// The browser's side of Xylem's live check (internal/server/live_test.go; docs/xylem/xylem.md,
// „Checks"): it subscribes at Mozilla's push service as a Firefox would, with the VAPID key the
// check wrote, and hands back what the service delivers. It opens nothing itself: the check
// holds the subscription's keys.
//
//   node live/autopush.mjs <dir>
//
// <dir>/vapid         read: Xylem's public VAPID key (the check writes it)
// <dir>/endpoint      written: the subscription's endpoint
// <dir>/notification  written: the push message as the service delivered it (JSON: data, headers)
//
// The WebSocket is a browser's (Playwright's Chromium), so that it goes the way a browser goes,
// through whatever proxy the machine has.
import { readFile, writeFile } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import { createRequire } from "node:module";
import { randomUUID } from "node:crypto";

const { chromium } = createRequire(import.meta.url)("playwright");
const dir = process.argv[2];
if (!dir) {
  console.error("usage: node live/autopush.mjs <dir>");
  process.exit(2);
}

let key = "";
for (let i = 0; i < 300 && !key; i++) {
  key = await readFile(`${dir}/vapid`, "utf8").then((t) => t.trim(), () => "");
  if (!key) await sleep(200);
}
if (!key) throw new Error(`no ${dir}/vapid: the check did not start`);

const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.goto("about:blank");
  await page.exposeFunction("hand", (name, text) => writeFile(`${dir}/${name}`, text));
  const end = await page.evaluate(({ key, channelID }) => new Promise((resolve) => {
    const ws = new WebSocket("wss://push.services.mozilla.com/");
    const done = (why) => { try { ws.close(); } catch {} resolve(why); };
    setTimeout(() => done("no push message within two minutes"), 120000);
    ws.onopen = () => ws.send(JSON.stringify({ messageType: "hello", use_webpush: true, broadcasts: {} }));
    ws.onerror = () => done("the WebSocket failed");
    ws.onclose = (e) => done(`the WebSocket closed (${e.code})`);
    ws.onmessage = async (e) => {
      const m = JSON.parse(e.data);
      if (m.messageType === "hello") {
        ws.send(JSON.stringify({ messageType: "register", channelID, key }));
      } else if (m.messageType === "register") {
        if (m.status !== 200) return done(`register answered ${m.status}`);
        await window.hand("endpoint", m.pushEndpoint);
      } else if (m.messageType === "notification") {
        ws.send(JSON.stringify({ messageType: "ack", updates: [{ channelID: m.channelID, version: m.version, code: 100 }] }));
        await window.hand("notification", JSON.stringify({ data: m.data, headers: m.headers }));
        ws.send(JSON.stringify({ messageType: "unregister", channelID }));
        setTimeout(() => done("delivered"), 500);
      }
    };
  }), { key, channelID: randomUUID() });
  console.log(end);
  process.exitCode = end === "delivered" ? 0 : 1;
} finally {
  await browser.close();
}
