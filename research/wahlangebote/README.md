# Wahlangebote, Gruppen und Zusatzangebote — the detection, tried on canary

Not part of the app. The prototype of the rules in `folia/design/wahlangebote/konzept.html`
(owner, 2026-10-19: „ich will nicht, dass wir sonderfälle per hand eintragen, sondern eine lösung
haben, die in die zukunft funktioniert … Bitte bilde alles außer daten passen nicht zusammen in der
DB und dem UI ab"), run on a Folia snapshot and held against a review by hand of the same one.

- `python/detect.py catalog.db 2026W detect.json`: per module and semester (per location course
  where Radix found courses), the extras and the choices — `program` and `group` (one content more
  than once, by whom QIS names at each option or not), `topic` (different contents to choose
  from) — with how many SWS each choice covers and how many options one takes.
- `python/evaluate.py detect.json data/manual-2026W.json`: what the rules find for each class of
  the review: `A` choice catalogues, `B` the same event more than once, `C` extras, `D` data that
  does not fit (ignored, as asked).
- `data/manual-2026W.json`: the review of 2026-10-19 on canary's snapshot (Radix 0.9.1): the 115
  modules whose events hold at least 1.5 times the module's SWS, overall or in one form, each with
  its class and a note.

On that snapshot (WiSe 2026/27, 1,441 modules with dates): A 68 of 76, B 18 of 19, C 3 of 4, and
none of the 16 of D; 23 more beyond the review (15 tutorials and office hours, 7 modules with
„(Gruppe N)", one catalogue of Stegreife), all right by hand. 112 modules in all: 72 with a
choice of topics, 14 with groups, 8 with one content per program, 21 with extras.

A snapshot of canary is its `/api/db` (closed testing: log in at `/access` first).
