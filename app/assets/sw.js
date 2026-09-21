// The service worker of Betula: the app starts without a network as well.
//
// What it keeps is the shell of the app — a page of the site (any one: the browser app renders
// the page the address names from the local catalog), the scripts, the styles, the WASM bundle,
// the font, the icons and the manifest. The catalog itself (`/api/db`) lives in IndexedDB,
// where `boot.js` keeps it, and the worker never touches it or the other `/api/*` answers.
//
// Versions: the server writes its build into this file, so a new build installs a new worker,
// which caches the new shell and drops the old one. Pages: the network first, and what it
// answered kept for the way back; offline the kept page, else the cached shell. Assets: the cache
// first, the network for what is not there yet.
const VERSION = "__BUILD__";
const SHELL = "betula-shell-" + VERSION;
const PAGES = "betula-pages";
const KEPT_PAGES = 60;
const PRECACHE = [
  "/",
  "/assets/app.css",
  "/assets/enhance.js",
  "/assets/boot.js",
  "/assets/sql-wasm.js",
  "/assets/sql-wasm.wasm",
  "/assets/inter-latin.woff2",
  "/assets/favicon.svg",
  "/favicon.ico",
  "/apple-touch-icon.png",
  "/assets/icon-192.png",
  "/assets/icon-512.png",
  "/assets/icon-maskable-512.png",
  "/manifest.webmanifest",
  "/pkg/folia_client.js",
  "/pkg/folia_client_bg.wasm",
];
const ASSET = /^\/(assets\/|pkg\/|favicon\.ico$|apple-touch-icon(-precomposed)?\.png$|manifest\.webmanifest$)/;
const NEVER = /^\/(api\/|access|sw\.js$|cards\/)/;

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(SHELL).then(async (cache) => {
      // Each file on its own: a bundle that is not built yet must not keep the rest out.
      await Promise.all(PRECACHE.map(async (path) => {
        try {
          const response = await fetch(path, { credentials: "same-origin" });
          if (response.ok) await cache.put(path, response);
        } catch {}
      }));
      await self.skipWaiting();
    }),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys()
      .then((names) => Promise.all(names.filter((name) => name !== SHELL && name !== PAGES).map((name) => caches.delete(name))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin || NEVER.test(url.pathname)) return;

  if (request.mode === "navigate") {
    event.respondWith(page(request));
  } else if (ASSET.test(url.pathname)) {
    event.respondWith(asset(request));
  }
});

// A page: the network's answer (kept for later), else the page as it was kept, else the shell.
async function page(request) {
  const key = new URL(request.url);
  key.hash = "";
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(PAGES);
      await cache.put(key.href, response.clone());
      forget(cache);
    }
    return response;
  } catch (error) {
    const kept = await caches.match(key.href, { cacheName: PAGES });
    if (kept) return kept;
    const shell = await caches.match("/", { cacheName: SHELL });
    if (shell) return shell;
    throw error;
  }
}

// An asset of the shell: what the worker installed with, else the network (and then kept).
async function asset(request) {
  const cached = await caches.match(request, { cacheName: SHELL });
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) {
    const cache = await caches.open(SHELL);
    await cache.put(request, response.clone());
  }
  return response;
}

// The kept pages stay a short list: the oldest go first.
async function forget(cache) {
  const keys = await cache.keys();
  if (keys.length <= KEPT_PAGES) return;
  await Promise.all(keys.slice(0, keys.length - KEPT_PAGES).map((key) => cache.delete(key)));
}
