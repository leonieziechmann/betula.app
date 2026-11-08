# Betula Sync in Folia

How a profile's devices stay alike once „Betula Sync" is switched on (owner, 2026-11-05: „Passt,
bau Schritt 1 jetzt so"). The design and its reasons: `research/sync/entwurf.html`; the service
and its protocol: `docs/phloem/protocol.md`. This file is about Folia's two parts:

| | |
|---|---|
| `folia/crates/sync` (`folia-sync`) | the merge: records, clocks, what a change writes, what text the records give back, the binary form. Pure Rust, no browser |
| `folia/crates/stores/src/sync` | the browser: keys and seals (WebCrypto), Phloem over HTTP and WebSocket, the tab that leads, localStorage |

## 1. Records

Every line of a store's text is a record under what the store itself tells lines apart by
(`folia_sync::schema`): a module once per semester, planned or visited; a placeholder by its pid;
a calendar by its number; a note once per module or Termin; a saved plan by its name; a field of
„Mein Studiengang" by its name. A set within a line (hidden kinds, hidden dates, a module's
collections) is a record per element, so two devices that add to one set keep both. A line of a
tag this build does not know is one record, kept as it is.

A change writes what differs between the store's text before and after it, both as the store
writes them (`Writer::changes`): nothing the store only read, normalised or left out ever becomes a
write, so a build that drops a line it cannot read never deletes it on other devices. Every write
carries a hybrid logical clock; of two writes of one record the later wins, a deletion included
(a tombstone, forgotten after 13 months). The records give every store its text back in the order
the store writes it (`project`); the tests read every tag of `PlanDoc` back character for
character.

What two devices do at once:

| | |
|---|---|
| the same module moved to two semesters | it stands in both, and a hint says so (`Conflict::Moved`) |
| a note typed on both | the later stands, the other is kept as a hint to take over (`ModuleNote`, `TerminNote`) |
| a mark taken away on one, set on the other | the later write |
| two calendars or collections made at once | two numbers (each device takes numbers of its own class, `folia_plans::ids`), both stand; two collections of one name as „… (2)" |
| something added to a calendar the other deleted | gone with the calendar: a calendar lives a life, and what belongs to it counts only within it; and a synced profile never gives a number out again that its records still keep a deletion of (13 months, `folia_sync::retired`), so a new calendar never inherits the old one's records or shares |
| one placeholder of a plan row made on both | the one of the smaller pid stands, and what counted for the other counts for it |

## 2. In the browser

**Keys.** The profile key `K` (32 random bytes) never leaves the devices but in the recovery code
and the pairing parcel. HKDF-SHA256 in WebCrypto gives the space, the token and the AES-256-GCM key
of the data (non-extractable); every batch and snapshot is sealed before it leaves (protocol.md §5).

**One tab leads.** A Web Lock (`betula.sync`) makes one tab of the browser the leader; a tab that
becomes visible takes it (`steal`), so the phone in the hand is the one that is live. The leader
alone writes a synced profile's records and talks to Phloem, for the active profile. A store's
change in another tab waits under `betula.sync.c.<profile>.<time>.<count>.<tab>` until the leader
takes it, in the order it was made.

**A change.** `Kept` remembers the text a store holds (what it read last, or wrote); its next write
is the change, whatever another tab or device wrote meanwhile. The leader writes its records, keeps
what waits at once, and sends it. What came from another device goes into the stores' texts where
they differ (`Profiles::wrote`: every tab's stores read again).

**What a synced profile keeps besides its stores** (`<id>` its number):

| key | | written |
|---|---|---|
| `betula.sync.p<id>.v1` | its key, the device's id, its slot | on switching on, joining, a new slot |
| `betula.sync.p<id>.out` | the writer's last number and clock, the batches waiting for `stored` | with every change |
| `betula.sync.p<id>.doc` | the entries held, the records, the conflicts | now and then, and when the tab is hidden; always after the stores were given what the records hold |
| `betula.sync.p<id>.status` | the leader's word: `online <n>` or `offline` | |
| `betula.sync.p<id>.ended` | the space is gone (ended on another device, or 13 months untouched) | |
| `betula.sync.p<id>.seen` | the hints a page has read | |
| `betula.sync.offset` | the server's clock less the browser's | on every welcome |

**When the leader starts** (a reload, a new leader, the active profile changed): it reads what is
kept, takes what other tabs left, and a store's text that holds more than the records (written by a
build or a tab that did not sync) is a change of this device. The records are never kept before
the stores were given them, so a start never takes what a store lacks for a deletion. A number is
never given twice: the writer goes on after the larger of the records' last number and the one kept
with what waits.

**Connection.** `hello` with the last entry held; the welcome brings a snapshot where the device is
behind Phloem's, and the entries after it. Then every change goes out at once, each batch once per
connection until `stored` (Phloem keeps a batch sent again once). A change larger than a batch goes
as a snapshot; so does the profile's first state. A lost connection is tried again soon, then less
often, at most every half minute, and at once when the network is back or the tab is shown.

## 3. The actions

| | |
|---|---|
| „Einschalten und Code zeigen" (the dialog „Gerät verbinden" of a private profile; „Betula Sync aktivieren" until 2026-11-08) | a key, a device id, `POST /sync/v1/space`, the profile's records as its first snapshot; then the link as below |
| „Gerät verbinden" (a synced profile; „Weiteres Gerät verbinden" until 2026-11-08) | a parcel with the key, sealed with a key the link alone holds (`/sync#<id ‖ key>`; `/verbinden#…` until 2026-11-08, which leads there), ten minutes at Phloem, for one device |
| „Mit dem Handy verbinden" (`/pair`, 2026-11-08) | the other way round, for a computer, which has no camera: it makes the parcel's id and key itself (`Sync::ask`, nothing sent) and shows `/pair#<id ‖ key>` as a QR code; the phone that opens it (its camera) shows which profile goes and what it holds, and „Verbinden" (a private profile is switched on on the way) seals the key with the question's key and leaves it under that id (`Sync::give`, `POST /sync/v1/pair`; 409 for a code answered already); the computer asks for it every 3 s in the first 90 s, then every 10 s, ten minutes at most (Phloem answers 120 such questions per address and hour), and joins as below (`Sync::collect`). No change of Phloem |
| joining | the parcel opened, a new profile of the browser, its records from Phloem |
| the recovery code | `K` in base32 (Crockford), 13 groups of four, the last 4 bits a check: most typos are told before any request |
| „Nur dieses Gerät trennen" | what waits goes out, then `leave`; the copy stays, private |
| „Sync beenden" | `DELETE /sync/v1/space`: gone at once; every device is private again with its copy, the others hear `gone` |
| a profile deleted | leaves its space for this device alone; the others keep it |
| „Profil leeren", „Rückgängig" | sync like any change |

## 4. Shares (step 3)

The dialog „Teilen" (`folia/crates/planner/src/studyplan/share_dialog.rs`; the concept and the
owner's decisions of 2026-11-08: `folia/design/teilen/teilen.html`) hands a calendar on, to
persons or into a calendar app, as a copy of now or one that stays current.

| | |
|---|---|
| a fixed copy for everybody | the link of today, `/studyplan?share=<code>`, or the address of today, `/calendar/<code>.ics`: no server keeps anything |
| what stays current | a part at Phloem: one semester of the calendar, `folia_calendar::live::LivePart` as a pack code, sealed with a key of its own (associated data `"part" ‖ id`); for a person nothing of what is hidden, for a calendar app the week as the student sees it; the Termine of one's own and the notes where the share takes them along. `/studyplan#live=<id ‖ key>`, `/calendar/s/<id ‖ key>.ics` |
| one person | a parcel that waits 7 days at most and opens once (`/studyplan#once=<id ‖ key>`): the part and its key, or a copy of now; opened again, expired or taken back, it says so and shows nothing |
| the shares in force | lines `s` of the Stundenplan (`folia_plans::shares`), records `P\|s\|<id>`: on every device of the profile, which renews their parts when the plan changes (only what differs from what it put, `share::Put`), moves a share on to the next semester as it begins, and deletes the parts of shares gone from the records |
| following | a calendar of one's own with a line `f` (`P\|f\|<calendar>`, it goes and comes back with its calendar): the leader watches its part (`watch`) and gives the calendar each new version (`PlanDoc::take_part`; a private profile fetches on opening and when the page comes into view); a part that is gone leaves the calendar „nicht mehr geteilt", with „Behalten" and „Löschen" |
| a calendar app's subscription | Folia fetches the part through Traefik's entrypoint `parts` (`--sync-parts`), opens it in memory with the key from the address and keeps nothing (`folia/crates/server/src/parts.rs`); no access log writes such an address (deploy/README.md section 15) |

A private profile offers Betula Sync wherever it makes a choice better („Profil speichern", one
click; decision 1), and the choices that need it save the profile on the way.

The start page's setup ends with it (owner, 2026-11-08: „Mach das mal als step in die einführung
mit rein. So als letzten schritt. So dass man es gleich auch auf dem handy hat";
`folia/crates/home/src/home/devices.rs`, docs/folia/frontend.md „The start page"): after
„Studium anlegen", „Betula Sync aktivieren" with what the server keeps above it, then the link for
the phone at once, with the profile page's QR code (`folia_widgets::syncpair`), and „Verbunden"
once one more device is online. On a phone's layout it names the address a laptop opens, `/pair`,
whose code the phone scans (owner, the same day: „Oh der pc prozess muss anders sein. Da muss ein
link kommen wie betula.app/pair wo dann der qr code gezeigt wird, den man am handy scannen kann um
sich einzuloggen"); its step 1 leads there with „Profil übernehmen".

## 5. Checks

- `cargo test -p folia-sync`: the stores' round trips, the cases of two devices, a simulation of
  three and four devices online and offline over a network that reorders and doubles, one joining
  from a snapshot, shares and follows among them (`FOLIA_SYNC_SEEDS=600` and `--release` for a
  longer search).
- `cargo test -p folia-stores sync`: the codes, the frames, what the browser keeps.
- `folia/e2e/sync.mjs`: two browsers of one profile against a running Phloem; the start page's
  setup to its last step, the study on a second browser through its QR code's link; on a phone
  the address `/pair` for the laptop, its code opened by the phone and „Verbinden", a private
  profile switched on on the way, a code that opens once.
- `folia/e2e/share.mjs`: sharing and following with four browsers against a running Phloem
  (Folia with `FOLIA_SYNC_PARTS` naming it).
