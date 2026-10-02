// The probe's worker: rusqlite on OPFS (the Rust half above) beside sql.js in memory, the same
// queries on both. Messages: {sql: [...]} → {times}.
import init, * as probe from "./pkg/probe.js";
const say = (m) => self.postMessage(m);
const t = () => performance.now();
self.onmessage = async ({ data }) => {
  const out = {};
  try {
    let s = t();
    await init({ module_or_path: "./pkg/probe_bg.wasm" });
    await probe.init();
    out.install = t() - s;
    s = t();
    if (!probe.has("/catalog.db")) {
      const bytes = new Uint8Array(await (await fetch("/api/db")).arrayBuffer());
      out.download = t() - s; s = t();
      probe.import_db("/catalog.db", bytes);
      out.import = t() - s;
    }
    const variants = [["opfs, cache 2 MB", false, 2000], ["opfs, cache 64 MB", false, 65536]];
    for (const [label, memory, cache] of variants) {
      s = t(); probe.open("/catalog.db", memory, cache); out[label + ": open"] = t() - s;
      for (const round of [1, 2]) {
        const times = {};
        for (const sql of data.sql) { s = t(); probe.rows(sql); times[sql.slice(0, 40)] = Math.round((t() - s) * 10) / 10; }
        out[label + ": round " + round] = times;
      }
    }
    s = t();
    const copy = new Uint8Array(await (await fetch("/api/db")).arrayBuffer());
    probe.import_memory("/memory.db", copy);
    out["memory: import"] = t() - s;
    s = t(); probe.open("/memory.db", true, 2000); out["memory: open"] = t() - s;
    for (const round of [1, 2]) {
      const times = {};
      for (const sql of data.sql) { s = t(); probe.rows(sql); times[sql.slice(0, 40)] = Math.round((t() - s) * 10) / 10; }
      out["rusqlite in memory: round " + round] = times;
    }
    // sql.js, the whole file in memory, as the app does today
    const code = await (await fetch("/assets/sql-wasm.js")).text();
    const initSqlJs = new Function(code + "; return initSqlJs;")();
    const SQL = await initSqlJs({ locateFile: (f) => "/assets/" + f });
    s = t();
    const bytes = new Uint8Array(await (await fetch("/api/db")).arrayBuffer());
    const db = new SQL.Database(bytes);
    out.sqljsOpen = t() - s;
    out.sqljs = {};
    for (const sql of data.sql) {
      s = t();
      const st = db.prepare(sql); let n = 0; while (st.step()) { st.get(); n++; } st.free();
      out.sqljs[sql.slice(0, 40)] = Math.round((t() - s) * 10) / 10;
    }
  } catch (e) {
    out.error = String(e && e.message || e);
  }
  say(out);
};
