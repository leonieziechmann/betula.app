const CACHE_NAME = 'btu-catalog-pwa-v1';
const PRECACHE_ASSETS = [
  '/',
  '/catalog',
  '/manifest.json',
  '/static/app.css',
  '/static/sql-wasm.js',
  '/static/sql-wasm.wasm',
  '/static/sqlite_bridge.js',
  '/static/icon-192.svg',
  '/static/icon-512.svg'
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      console.log('[ServiceWorker] Pre-caching offline shell');
      return cache.addAll(PRECACHE_ASSETS).catch((err) => {
        console.warn('[ServiceWorker] Precache failed for some assets:', err);
      });
    })
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((cacheNames) => {
      return Promise.all(
        cacheNames
          .filter((name) => name !== CACHE_NAME)
          .map((name) => {
            console.log('[ServiceWorker] Removing old cache:', name);
            return caches.delete(name);
          })
      );
    })
  );
  self.clients.claim();
});

self.addEventListener('fetch', (event) => {
  const req = event.request;
  const url = new URL(req.url);

  // Only handle GET requests
  if (req.method !== 'GET') {
    return;
  }

  // Bypass the service worker for the huge raw SQLite DB download or Range requests
  if (url.pathname === '/api/db' || req.headers.get('range')) {
    return;
  }

  // HTML page navigations: Network-first, fallback to cache
  if (req.mode === 'navigate') {
    event.respondWith(
      fetch(req)
        .then((networkRes) => {
          if (networkRes && networkRes.status === 200) {
            const clone = networkRes.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(req, clone));
          }
          return networkRes;
        })
        .catch(async () => {
          const cached = await caches.match(req);
          if (cached) return cached;
          const catalogFallback = await caches.match('/catalog');
          if (catalogFallback) return catalogFallback;
          return caches.match('/');
        })
    );
    return;
  }

  // Static assets (WASM, JS, CSS, fonts, icons): Cache-first with network fallback
  if (
    url.pathname.startsWith('/static/') ||
    url.pathname.endsWith('.wasm') ||
    url.pathname.endsWith('.js') ||
    url.pathname.endsWith('.css') ||
    url.pathname.endsWith('.svg') ||
    url.pathname.endsWith('.png') ||
    url.pathname === '/manifest.json'
  ) {
    event.respondWith(
      caches.match(req).then((cachedRes) => {
        if (cachedRes) {
          // Return cached, update in background if needed
          fetch(req).then((netRes) => {
            if (netRes && netRes.status === 200) {
              caches.open(CACHE_NAME).then((cache) => cache.put(req, netRes));
            }
          }).catch(() => {});
          return cachedRes;
        }
        return fetch(req).then((netRes) => {
          if (netRes && netRes.status === 200) {
            const clone = netRes.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(req, clone));
          }
          return netRes;
        });
      })
    );
    return;
  }

  // Default: Network-first
  event.respondWith(
    fetch(req)
      .then((netRes) => {
        if (netRes && netRes.status === 200) {
          const clone = netRes.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(req, clone));
        }
        return netRes;
      })
      .catch(() => caches.match(req))
  );
});
