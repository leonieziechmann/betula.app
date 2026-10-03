# Betula

## Branches

`develop` gathers the features; `master` is what canary deploys (owner, 2026-10-01: not ten
deploys an hour; `deploy/README.md` section 12).

- Work on top of `develop`, and merge a finished branch into `develop` with a merge commit
  (`Merge branch '<branch>' into develop: <what it brings>`), never straight into `master`. A push
  to `develop` builds and deploys nothing.
- `master` takes nothing but `develop`, and only when the owner asks for a release: every push to
  `master` that changes the sources is built on GitHub and deployed to https://canary.betula.app.

## Layout

`radix/` and `cortex/` are the two Go modules (run `go` in each; Radix imports `cortex/client` through a
`replace ../cortex` in `radix/go.mod`), `folia/` the Cargo workspace (run `cargo` and
`folia/scripts/*` there; crates under `folia/crates/<crate>`, package `folia-<crate>`). Docs live
in `docs/radix`, `docs/cortex`, `docs/folia`, `docs/history`; experiments in `research/`. The README has the table.
