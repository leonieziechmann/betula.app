// The Web Worker of the catalog's search (app::data::CatalogWorker; owner, 2026-10-02: typing a
// search lagged): a copy of the local catalog of its own, in sql.js, and the app's bundle, whose
// `worker_catalog` and `worker_similar` (client/src/worker.rs) run the loaders of the catalog's
// list and of its „Ähnliche Module" on it. So the queries of a search the visitor types run here,
// beside the page's thread, which only builds what they found. `boot.js` starts it once the app
// runs and hands it the catalog the page opened; scripts/build-client.sh puts it into site/pkg.
//
// Messages (each answer carries the `id` of its request):
//   {id, type: "open", bytes, bundle}   the bytes of the page's copy of the catalog (handed over),
//                                       and the bundle as the page compiled it (a
//                                       WebAssembly.Module; without it the worker compiles it) → {}
//   {id, type: "catalog", ask}          JSON → {answer}: JSON of a Result<CatalogData, DataError>
//   {id, type: "similar", ask}          JSON → {answer}: JSON of a Result<Vec<CatalogRow>, DataError>
//   any failure                         → {error}
//
// A classic worker: sql.js is a classic script (`importScripts`), the bundle a module, which comes
// by `import()`. Both of the build of this worker's address (`?v=<build>`), as the page has them.
const BUILD = self.location.search;
let app = null;

const handlers = {
  async open({ bytes, bundle: compiled }) {
    importScripts("/assets/sql-wasm.js" + BUILD);
    const SQL = await self.initSqlJs({ locateFile: (file) => "/assets/" + file + BUILD });
    const db = new SQL.Database(bytes);
    // What the bundle's `LocalDatabase` asks, as on the page (boot.js).
    self.betulaDb = {
      query(sql, params) {
        const statement = db.prepare(sql);
        try {
          statement.bind(params);
          const columns = statement.getColumnNames();
          const rows = [];
          while (statement.step()) rows.push(statement.get());
          return { columns, rows };
        } finally {
          statement.free();
        }
      },
    };
    const bundle = await import("/pkg/folia_client.js" + BUILD);
    await bundle.default({ module_or_path: compiled ?? "/pkg/folia_client_bg.wasm" + BUILD });
    app = bundle;
    return {};
  },
  catalog({ ask }) {
    return { answer: app.worker_catalog(ask) };
  },
  similar({ ask }) {
    return { answer: app.worker_similar(ask) };
  },
};

// One message at a time, in order: nothing is asked before the catalog is open.
let queue = Promise.resolve();
self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    try {
      const handler = handlers[data.type];
      if (!handler) throw new Error(`unknown message ${data.type}`);
      if (data.type !== "open" && !app) throw new Error("the catalog is not open");
      self.postMessage({ id: data.id, ...(await handler(data)) });
    } catch (e) {
      self.postMessage({ id: data.id, error: e instanceof Error ? e.message : String(e) });
    }
  });
};
