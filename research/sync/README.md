# Sync between devices — the feasibility study

Not part of the app. The study the owner asked for on 2026-11-05 („mach dir mal bitte gedanken
über ein sync system … dass man seamless zwischen computer und handy switchen [kann] … mach mal
eine machbarkeits studie, was wie gut umsetzbar wäre und wie viel ressourcen das brauchen
würde“): `machbarkeit.html`, in German like the concepts in `folia/design/`, and the measurements
its numbers come from.

- `measure/sizes.py`: a profile in Betula's own store formats (`folia/crates/plans/src/studyplan.rs`,
  `folia/crates/stores/src/{bookmarks,collections}.rs`) in three sizes, raw, deflated and sealed
  (ChaCha20-Poly1305 with a 24-byte nonce and a 32-byte header), per store and whole; and the upper
  bound the stores' own read caps set. Needs `cryptography` (`pip install cryptography`).
- `measure/load.py`: what sync would cost the server for 500, 2,000 and 7,500 students (the BTU
  had about 7,500 in 2026), from the sizes above and the assumptions at its top.
- `measure/sse/`: what an idle live connection costs. `cargo build --release` (axum 0.8.9 and
  tokio 1.53.1, the versions of Folia's server) builds a server that holds server-sent-event
  streams; `proxy/` is a bare Go reverse proxy (`httputil.ReverseProxy`, net/http like Traefik)
  in front of it; `client.py` opens the streams. Run the server (`target/release/sync-sse-bench`),
  then `python3 client.py 10000`; with the proxy (`go build` in `proxy/`, then `./sync-sse-proxy`)
  `python3 client.py 8000 18778`. Both print their resident memory every two seconds.

Measured on 2026-11-05 in a cloud container (Linux 6.18):

| | |
|---|---|
| a profile, sealed | light 0.43 KB, typical 2.27 KB, heavy 15.35 KB (raw 0.5 / 5.2 / 62.4 KB) |
| upper bound by the stores' read caps | 4.54 MiB raw per profile |
| an idle SSE stream in axum/tokio | 18.7 KiB (10,000 streams: VmRSS 3,412 → 190,652 kB) |
| the same through the Go proxy | 62.5 KiB in the proxy (8,000 streams: 6,868 → 506,624 kB); the server behind it 18.8 KiB |
