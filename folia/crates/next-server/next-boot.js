// The minimal version's boot (docs/folia-refactor.md §6.7). The page has painted already (the
// site's HTML or the app document); now the data worker and the UI bundle start in parallel, and
// the app takes the page over once it has the page's data.
const BUILD = new URL(import.meta.url).search;
const worker = new Worker("/assets/next-worker.js" + BUILD);
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") worker.postMessage({ type: "visible" });
});
window.__foliaWorker = worker;
// Until the app listens, what the worker says is kept for it.
window.__foliaEarly = [];
worker.onmessage = ({ data }) => window.__foliaEarly.push(data);
const app = await import("/pkg/folia_app.js" + BUILD);
await app.default({ module_or_path: "/pkg/folia_app_bg.wasm" + BUILD });
app.start(worker);
