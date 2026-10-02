# folia/ — the minimal version of Folia's restructuring

Branch `spike/folia-next`. The plan is `docs/folia-refactor.md`; this is its phase 0 (§11.1), a
thin slice through every new part, beside today's crates and merged nowhere. What it found is in
§11.1.1 of the plan.

A workspace of its own (`folia/Cargo.toml`, its own lock file), so nothing here changes the build
that ships. It uses today's `catalog/` as it is, but for one line (`catalog/src/filter.rs`, R29).

| Crate (`folia/crates/…`) | Layer | What it is |
|---|---|---|
| `folia-pages` (`pages`) | domain | the pages' questions to the data worker and the answers (postcard), and the loaders that answer them |
| `folia-design` (`design`) | UI base | the frame, the icon set, the mark, a skeleton; `style.css` |
| `folia-shell` (`shell`) | UI base | rail, header, footer, background; `style.css` |
| `folia-data` (`data`) | UI base | `DataClient`, `use_ask` |
| `folia-catalog-ui` (`catalog-ui`) | features | the list and the module page: views iso, pages web; `style.css` |
| `folia-app` (`app`) | composition | the route table and the UI bundle (`#[lazy]` module page) |
| `folia-worker` (`worker`) | composition | the data worker: `worker.js` (sql.js) and its Rust half |
| `folia-site` (`site`) | composition | the site's pages from the same views |
| `folia-next-server` (`next-server`) | composition | `folia-next`: the site, the app document, the snapshot, the files; `tests/layers.rs` |

`folia/layers.toml` is the table the layer test reads. `folia/probes/sqlite-opfs` is the probe of
the worker's step 2 (rusqlite on `sqlite-wasm-rs`, OPFS and in memory, against sql.js).

## Running it

Needs the `wasm32-unknown-unknown` target, `wasm-bindgen` 0.2.128 and a snapshot
(`curl --compressed -o snapshot/catalog.db https://betula.app/api/db` in the repository's root).

```bash
bash folia/scripts/build.sh --dev                 # the UI bundle, the worker, the server (no --dev: release)
cd folia && ./target/debug/folia-next --snapshot ../snapshot/catalog.db   # http://127.0.0.1:8090/catalog
node folia/e2e/minimal.mjs                        # the check; SMOKE_BROWSER_PATH=<chromium> without an installed one
cd folia && cargo test                            # answers through postcard, the layers
```

The split bundle: `cargo install --locked wasm_split_cli_support --features build-binary`, then
`WASM_SPLIT=<its wasm-split-cli> bash folia/scripts/build-split.sh` and the server with
`FOLIA_SPLIT=1`. The probe: build `folia/probes/sqlite-opfs` for `wasm32-unknown-unknown`
(release), `wasm-bindgen --target web --out-dir pkg --out-name probe …`, open
`http://127.0.0.1:8090/probe/index.html` (`window.result`).

`/spike/next-snapshot` makes the server announce the same bytes under a new ETag, so that the switch
to new data can be watched (`folia/e2e/minimal.mjs` does).
