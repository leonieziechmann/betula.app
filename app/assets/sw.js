// The service worker of Betula: the app starts without a network as well.
//
// What it keeps is the shell of the app — a page of the site (any one: the browser app renders
// the page the address names from the local catalog), the scripts, the styles, the sprite of the
// icons, the WASM bundle, the font, the site's icons and the manifest. The catalog itself (`/api/db`) lives in IndexedDB,
// where `boot.js` keeps it, and the worker never touches it or the other `/api/*` answers. Nor a
// Studienplan's calendar feed (`/calendar/…`): a calendar service fetches it from its own servers,
// and a feed kept here would be somebody's plan in Cache Storage, or the shell offline.
//
// Versions: the server writes its build into this file, so a new build installs a new worker,
// which caches the new shell and drops the old one. A page names the build that wrote it in the
// addresses of its stylesheet and scripts (`/assets/app.css?v=<build>`, `app::BuildId`; `boot.js`
// does the same for the bundle and sql.js), and the worker keeps the files under exactly these
// addresses. So a page always gets the files of its own build: on the first load after a deploy
// the old worker still answers, the page comes from the new server and asks for
// `?v=<new build>`, the old shell has nothing under that address, and the network answers.
// And the worker keeps only what its own build answered (the server names it in `x-build`): a
// page of the new build is not kept by the old worker, so offline it never shows new markup with
// its old files, and a server that moved on during the install fails the install, which leaves
// the worker in charge that has a whole shell.
//
// Pages: the network first, and what it answered kept for the way back; offline the kept page,
// else the cached shell. Assets: the cache first, the network for what is not there yet.
const VERSION = "__BUILD__";
const SHELL = "betula-shell-" + VERSION;
const PAGES = "betula-pages-" + VERSION;
const KEPT_PAGES = 60;
// How a page of this build asks for the files that change with a build.
const TAG = new URL("?v=" + VERSION, self.location.href).search;
const BUILT = [
  "/assets/app.css",
  "/assets/icons.svg",
  "/assets/enhance.js",
  "/assets/boot.js",
  "/assets/sql-wasm.js",
  "/assets/sql-wasm.wasm",
  "/pkg/folia_client.js",
  "/pkg/folia_client_bg.wasm",
];
const PRECACHE = [
  "/",
  ...BUILT.map((path) => path + TAG),
  "/assets/inter-latin.woff2",
  "/assets/favicon.svg",
  "/favicon.ico",
  "/apple-touch-icon.png",
  "/assets/icon-192.png",
  "/assets/icon-512.png",
  "/assets/icon-maskable-512.png",
  "/manifest.webmanifest",
];
const ASSET = /^\/(assets\/|pkg\/|favicon\.ico$|apple-touch-icon(-precomposed)?\.png$|manifest\.webmanifest$)/;
const NEVER = /^\/(api\/|access|sw\.js$|cards\/|calendar\/)/;

// Did this build answer? Only such answers are kept.
const ours = (response) => response.ok && response.headers.get("x-build") === VERSION;

self.addEventListener("install", (event) => {
  event.waitUntil((async () => {
    // Asked of the server, not of the browser's HTTP cache, which may hold a page of an older
    // build. A file the server does not have (a bundle not built yet) must not keep the rest out.
    // Any other failure — no network, an error, a server of another build (it moved on since this
    // file was fetched) — fails the install: the worker in charge keeps its whole shell, and the
    // next page load tries again.
    const answers = await Promise.all(PRECACHE.map((path) => fetch(path, { credentials: "same-origin", cache: "no-cache" })));
    const wrong = answers.findIndex((response) => response.status !== 404 && !ours(response));
    if (wrong >= 0) throw new Error(`${PRECACHE[wrong]}: ${answers[wrong].status} from build ${answers[wrong].headers.get("x-build")}, not ${VERSION}`);
    const cache = await caches.open(SHELL);
    await Promise.all(answers.map((response, i) => (response.ok ? cache.put(PRECACHE[i], response) : null)));
    await self.skipWaiting();
  })());
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

// A page: the network's answer (kept for later if this build wrote it), else the page as it was
// kept, else the shell. Always asked of the server (a 304 when it is unchanged): the browser's
// HTTP cache may still hold the page of an older build, whose files the server no longer has.
async function page(request) {
  const key = new URL(request.url);
  key.hash = "";
  // (A browser that cannot copy a navigation with another cache mode fetches it as it came.)
  let fresh = request;
  try {
    fresh = new Request(request, { cache: "no-cache" });
  } catch {}
  try {
    const response = await fetch(fresh);
    if (ours(response)) {
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

// An asset of the shell: what the worker installed with, else the network (and then kept, if it
// is a file of this build under an address of this build).
async function asset(request) {
  const cached = await caches.match(request, { cacheName: SHELL });
  if (cached) return cached;
  const response = await fetch(request);
  const search = new URL(request.url).search;
  if (ours(response) && (search === "" || search === TAG)) {
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
