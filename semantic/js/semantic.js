// The semantic search for a page: the work happens in a Web Worker (worker.js), this is its
// face. Typing fast never piles up work: while a query runs, only the newest one waits, and the
// ones it replaced resolve to null.
//
//   import { Semantic } from "/pkg/semantic.js";
//   const semantic = new Semantic({ model: "/api/semantic/model", index: "/api/semantic/index" });
//   await semantic.ready;                       // {rows, build, ms}: loaded (a second or two)
//   const found = await semantic.search("coding lernen", 20);
//   if (found) for (const { id, score } of found.hits) …   // null: a newer query took its place

export class Semantic {
  /** @param {{model: string, index: string, worker?: string | URL}} urls the packed model, the
   * index, and the worker script (beside this file unless given) */
  constructor({ model, index, worker = new URL("semantic-worker.js", import.meta.url) }) {
    this.worker = new Worker(worker);
    this.next = 0;
    this.pending = new Map();
    this.waiting = null; // the newest query, while another one runs
    this.running = false;
    this.worker.onmessage = ({ data }) => {
      const pending = this.pending.get(data.id);
      this.pending.delete(data.id);
      if (data.error) pending?.reject(new Error(data.error));
      else pending?.resolve(data);
    };
    this.worker.onerror = (event) => {
      for (const { reject } of this.pending.values()) reject(new Error(event.message || "the search worker failed"));
      this.pending.clear();
    };
    this.ready = this.#ask({ type: "init", model: String(new URL(model, location.href)), index: String(new URL(index, location.href)) });
  }

  /** The `k` modules closest to `query`, best first: {hits: [{id, score}], ms}; null when a newer
   * query replaced this one before it ran. */
  search(query, k = 20) {
    if (this.waiting) this.waiting.resolve(null);
    return new Promise((resolve, reject) => {
      this.waiting = { query, k, resolve, reject };
      this.#pump();
    });
  }

  /** Loads another index (a new snapshot) into the worker. */
  setIndex(index) {
    return this.#ask({ type: "index", index: String(new URL(index, location.href)) });
  }

  terminate() {
    this.worker.terminate();
  }

  async #pump() {
    if (this.running || !this.waiting) return;
    const { query, k, resolve, reject } = this.waiting;
    this.waiting = null;
    this.running = true;
    try {
      await this.ready;
      resolve(await this.#ask({ type: "search", query, k }));
    } catch (e) {
      reject(e);
    } finally {
      this.running = false;
      this.#pump();
    }
  }

  #ask(message) {
    const id = this.next++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.worker.postMessage({ ...message, id });
    });
  }
}
