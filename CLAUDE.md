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
  before the site's legal texts are set, not to keep anything secret. The password is in the
  environment variable `CANARY_PW` (owner, 2026-10-17), and it never goes into the repository,
  which is public.

## Design

Betula does not say no (owner, 2026-10-18: „Betula soll nie sagen. Nein, das geht nicht wenn es
nicht ein absolut triftigen Grund dafür gibt. Weil in der Regel hat der Nutzer einen Plan was er
machen möchte und irgendeine abitreere Regel die wir uns ausgedacht haben könnte sein use case
kaputt machen, obwohl der total valid ist."):

- What a student asks for and is easy to do is allowed, even where it looks odd: a module planned
  into a semester behind them or into one that does not offer it, a value outside what is usual.
  Instead of a disabled control or a refusal, Betula does it and says what is unusual next to the
  value (a note, a warning), so the choice is an informed one.
- A no needs a reason that holds: what cannot be (data that does not exist, a semester before the
  study began, what would break what the student keeps), not a rule made up for the usual case.
  Defaults and suggestions follow the usual case; limits do not.
- It does not mean building for every case (owner, same day: „Das heißt aber nicht, dass wir für
  jeden anwendungsfall entwickeln müssen. Aber wenn das einfach möglich umzusetzen ist, sollte es
  erlaubt sein."): what takes real work for a rare case may wait; what only a limit stands in the
  way of is allowed.

No action is a key's alone (owner, 2026-10-20: „Außerdem finde ich, dass es ein no go ist, dass
aktionen nur über shortcuts erreichbar sind. Ich möchte entweder etablierte konzepte, wie ctrl+c
und ctrl+v, die keine visuellen hints brauchen. Aber das was betula selbst macht soll als komponent
da sein, welches auch per button bedient werden kann."):

- Everything Betula does is there as a component one clicks or taps (a button, an entry of a
  menu); a key is a quicker way to it, never the only one.
- What every program does by a key (Ctrl+C, Ctrl+V, Esc, Enter, the arrows, F2, Entf) needs no
  visible hint; Betula's own keys are named beside their button or entry.

What picks stands above what it changes (owner, 2026-10-27, of the Termin's view: „Einfaches
Prinzip. Wenn man solche Buttons hat, dann ändern die nur den Inhalt unter ihnen. Ansonsten muss man
nochmal über das Konzept nachdenken."):

- A control that picks what a view shows (segments, tabs, the tiles of groups, the arrows of a date)
  changes only what stands below it, never what stands above it. A view whose pick would change
  what stands above it needs another order, not a tweak.

## Versions

Folia, Radix and Cortex each have a version by SemVer, and every change raises the version of
each of them whose shipped sources it changes, without being asked (owner, 2026-10-15: „immer wenn
wir changes machen update die version number based on sem ver"):

- PATCH for a fix and for a small change of behaviour, even one that is seen (an animation, a
  timing, a gesture, a text, a look tuned); MINOR only for a larger change or a new feature, not
  for every change one notices (owner, 2026-10-18: „dass die minor versions nicht ganz so
  inflationär verwendet werden sollen. Gerade so kleine behavior changes können immer einfach nur
  die letzte nummer sein. In der mitte ist dann für größere changes/neue features da").
- MAJOR for what breaks those who use it (Radix: a snapshot Folia cannot read, its flags or its
  environment; Cortex: its client's API; Folia: addresses that go away, what a browser keeps that
  it cannot read any more), and never alone: a change that would raise it is talked over with the
  owner first („die major version besprechen wir zusammen").
- Docs, `design/`, `research/` and the checks alone raise nothing. A version the owner names wins.
- The bump is a commit of the branch on top of `develop` as it is when the branch is merged: a
  branch that `develop` overtook raises the newer version again, so `develop` always says what is
  in it, and `master` takes the versions with the release.
- Folia: `version` of the crates at the app's version, those at the same version as
  `folia/crates/server/Cargo.toml` (the libraries at 0.1.0 stay; `cargo` updates
  `folia/Cargo.lock`); the page's ground and `x-build` show it. Radix:
  `radix/internal/version/version.go`, which every snapshot carries (`radix_version`). Cortex:
  `cortexVersion` in `cortex/cmd/cortex/main.go`.

## Layout

`radix/` and `cortex/` are the two Go modules (run `go` in each; Radix imports `cortex/client` through a
`replace ../cortex` in `radix/go.mod`), `folia/` the Cargo workspace (run `cargo` and
`folia/scripts/*` there; crates under `folia/crates/<crate>`, package `folia-<crate>`). Docs live
in `docs/radix`, `docs/cortex`, `docs/folia`, `docs/history`; experiments in `research/`. The README has the table.
