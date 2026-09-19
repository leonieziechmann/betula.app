# Spikes

Throwaway experiments behind `docs/frontend-phase0.md` §2. They are not part of the Cargo
workspace; each has its own lock file and `target/`.

## leptos-dispose

The disposed-signal crash of the old app in 70 lines. `Cargo.toml` pins `leptos = "=0.7.8"`
(panics after a few clicks on „next page, id first"); change it to `=0.8.20` and it survives.

```bash
cd spikes/leptos-dispose && trunk serve --port 8791
```

## leptos-ssr

Leptos 0.8.20 with server rendering and hydration on the shared `catalog` crate, built
without `cargo-leptos`:

```bash
cd spikes/leptos-ssr
cargo build --release --features ssr
cargo build --lib --target wasm32-unknown-unknown --features hydrate --profile wasm-release
wasm-bindgen --target web --no-typescript --out-dir site/pkg --out-name spike \
  target/wasm32-unknown-unknown/wasm-release/spike_ssr.wasm
target/release/spike-ssr ../../snapshot/catalog-<hash>.db site     # http://127.0.0.1:8793
```

`wasm-bindgen` must have the version of the `wasm-bindgen` crate (pinned to 0.2.128 here; Trunk
keeps that binary in its cache directory). Smoke walk against it:

```bash
cd e2e && npm install
SMOKE_BASE_URL=http://127.0.0.1:8793 SMOKE_BROWSER_CHANNEL=msedge \
SMOKE_OPTIONS='{"selectors":{"programLink":"#programs a","programPage":"#program-name","tab":".tabs a","areaChip":".chip"}}' \
node run.mjs
```
