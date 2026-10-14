# Betula

## Branches

`develop` gathers the features; `master` is what canary deploys (owner, 2026-10-01: not ten
deploys an hour; `deploy/README.md` section 12).

- Work on top of `develop`, and merge a finished branch into `develop` with a merge commit
  (`Merge branch '<branch>' into develop: <what it brings>`), never straight into `master`. A push
  to `develop` builds and deploys nothing.
- `master` takes nothing but `develop` (`Merge branch 'develop' into master: <the features it
  brings>`): every push to `master` that changes the sources is built on GitHub and deployed to
  https://canary.betula.app.
- A finished feature goes to canary without being asked (owner, 2026-10-14: „Ja immer rauf auf
  canary dafür ist es da"): once it is merged into `develop`, release `develop` into `master` — one
  merge per finished feature, not per commit — and try the feature on canary once it is deployed
  (its `x-build` header changes). Canary is closed testing so that the owner can try features
  before the site's legal texts are set, not to keep anything secret; the owner gives the password,
  and it never goes into the repository, which is public.

## Layout

`radix/` and `cortex/` are the two Go modules (run `go` in each; Radix imports `cortex/client` through a
`replace ../cortex` in `radix/go.mod`), `folia/` the Cargo workspace (run `cargo` and
`folia/scripts/*` there; crates under `folia/crates/<crate>`, package `folia-<crate>`). Docs live
in `docs/radix`, `docs/cortex`, `docs/folia`, `docs/history`; experiments in `research/`. The README has the table.
