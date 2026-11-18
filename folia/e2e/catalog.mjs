// The catalog the server serves (`/api/db`), as a checks script reads it: with the pass the
// download asks for (folia/crates/server/src/pass.rs), earned here as the data worker earns it,
// with Node's SHA-256. Without a pass on the server the first answer is the catalog.
//   import { catalogBytes } from "./catalog.mjs";
//   const bytes = await catalogBytes(base);    // a Uint8Array of the SQLite file
import { createHash } from "node:crypto";

const zeros = (text) => {
  let bits = 0;
  for (const byte of createHash("sha256").update(text).digest()) {
    if (byte) return bits + Math.clz32(byte) - 24;
    bits += 8;
  }
  return bits;
};

export async function catalogBytes(base, headers = {}) {
  let cookie = "";
  for (let attempt = 0; attempt < 3; attempt++) {
    const response = await fetch(base + "/api/db", { headers: { ...headers, ...(cookie ? { cookie } : {}) } });
    if (response.ok) return new Uint8Array(await response.arrayBuffer());
    if (response.status !== 401) throw new Error(`GET /api/db: HTTP ${response.status}`);
    const { challenge, seed, difficulty } = await response.json();
    let nonce = 0;
    while (zeros(seed + nonce) < difficulty) nonce++;
    const earned = await fetch(base + "/api/pass", { method: "POST", headers: { ...headers, ...(cookie ? { cookie } : {}) }, body: new URLSearchParams({ challenge, nonce: String(nonce) }) });
    await earned.arrayBuffer();
    if (!earned.ok) throw new Error(`POST /api/pass: HTTP ${earned.status}`);
    const pass = (earned.headers.get("set-cookie") || "").split(";")[0];
    cookie = [headers.cookie, pass].filter(Boolean).join("; ");
  }
  throw new Error("GET /api/db: no pass taken");
}
