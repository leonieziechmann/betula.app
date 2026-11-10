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
is opened in front or becomes visible takes it (`steal`), so the phone in the hand is the one that
is live, and so does a tab that switches a profile on, joins one or hands it on (`lead_here`): the
connection is in the tab the person acts in, never only in a tab in the background that may not
come to it (a tab a browser froze, one of an older build; 2026-11-10 a laptop's profile was
switched on in one tab while no tab connected, and its records never reached Phloem). The leader
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
| `betula.sync.p<id>.status` | the leader's word: `online <n> direct <d> relayed <r>` or `offline` (an older build's `online <n>` reads with none direct). Online means connected and the profile at Phloem (since Folia 1.90.0): a profile switched on is not until its first snapshot is kept, nor a device that joined a space still empty | |
| `betula.sync.p<id>.relay` | the slots whose last connection needed TURN, with when, kept a day (`live::read_relayed`) | when a connection opens |
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

**Two ways to Phloem** (since Folia 1.90.0; owner, 2026-11-10: „wenn eine methode nicht
funktioniert muss dann halt ausgewichen werden auf eine andere die grantiert läuft"). The
connection is the WebSocket, or the same frames over plain HTTPS requests to the same host
(`sync/browser/poll.rs`, protocol.md §4 „The socket over HTTPS", Phloem 0.4.0): one GET at a time
waits for what Phloem says, POSTs carry what the device says, in order. It goes the way that
brought the last welcome; after two tries in a row without a welcome, the other way every second
try. A connection without a welcome within 10 s counts as failed (a proxy that takes the upgrade
and then holds it). So a network that breaks WebSockets but lets the site load syncs all the
same; the live way (WebRTC) goes on top of either.

**A device that joined a space still empty** (the device that gave the profile had not sent its
records yet, which a build before 1.90.0 could do): it takes no entry then, since Phloem hands the
snapshot on a welcome alone, but connects again from 0 when one comes, and two seconds after
another device of the profile came online.

**The live way** (step 2, `sync/live.rs` and `sync/browser/rtc.rs`; owner, 2026-11-08: „time für
phase 2 bei dem sync"). Every two devices of the profile online connect directly with WebRTC: of
two, the one of the smaller slot offers and the other answers, so that no connection is set up
from both sides at once. The signals go through Phloem, sealed with the live key (HKDF info
`live`) to the space, `signal` and both slots: Phloem can neither read them nor hand them to
another device. The first attempt takes STUN alone and counts as failed after 8 s; the next ones
take TURN as well (15 s), then again after 30 s, 1, 2, 4 and at most 5 minutes; a device that
needed TURN with another takes it at once for a day (`betula.sync.p<id>.relay`). The ICE servers
and the TURN credential come from Phloem (`turn`, per socket: a new socket asks again). A
negotiated data channel (unordered, reliable) carries each change of this device as the sealed
batch Phloem keeps, at once to the other device and to Phloem; what comes over it is taken as an
entry is, without its number, and Phloem's copy of it works no second time. An open connection
stays until it fails or the other device closes it; one that is not open goes with its device.
The profile page says „direkt verbunden" or „über Betula weitergeleitet".

## 3. The actions

| | |
|---|---|
| „Einschalten und Code zeigen" (the dialog „Gerät verbinden" of a private profile; „Betula Sync aktivieren" until 2026-11-08) | a key, a device id, `POST /sync/v1/space`, the profile's records as its first snapshot; then the link as below |
| „Gerät verbinden" (a synced profile; „Weiteres Gerät verbinden" until 2026-11-08) | a parcel with the key, sealed with a key the link alone holds (`/sync#<id ‖ key>`; `/verbinden#…` until 2026-11-08, which leads there), ten minutes at Phloem, for one device. Made only once the profile is online in this tab with its records at Phloem (`Sync::pair`, at most 30 s, `ONLINE_WAIT_MS`; then „nicht erreichbar" with „Erneut versuchen"): a link made before would hand on a profile that is not there |
| joining | the parcel opened, a new profile of the browser, its records from Phloem |
| „Mit dem Handy verbinden" (`/pair`) and „Profil vom Handy holen" (the device that wants a profile, a computer that has no camera; 2026-11-08) | an id of 16 random bytes and an ECDH key pair (P-256) made for this once, the code `/pair#<id ‖ public key>` (65 bytes uncompressed; `url::PAIR`) and its four digits (SHA-256 of `"betula-request" ‖ id ‖ key`, the first four bytes mod 10 000); then `GET /sync/v1/pair` for the id, every 3 s in the first minute, every 6 s until the third, every 15 s until the tenth (some 70 questions, within the 120 an address may ask in an hour), until the parcel is there; the profile joins as above. No change of Phloem |
| „Ja, „Lena" verbinden" (the device that has it, after the code and its digits) | a private profile is switched on first; an ECDH key pair of its own, the key of the parcel by HKDF-SHA256 of the shared secret (salt the id, info `betula-request`), the profile's key sealed with it (associated data `"request"`), `POST /sync/v1/pair` with the request's id and `own public key ‖ sealed`: whoever photographs the code cannot open what is sent to it; as the link, only once the profile is online here with its records at Phloem |
| the recovery code | `K` in base32 (Crockford), 13 groups of four, the last 4 bits a check: most typos are told before any request |
| „Nur dieses Gerät trennen" | its record of the devices deleted, what waits goes out, then `leave`; the copy stays, private |
| the devices (2026-11-09) | each device writes a record of itself into the profile, `D\|<device id>`: slot, kind, model (Chrome's `userAgentData`), browser, system, since, last seen (`folia_stores::sync::devices`, `Writer::put`; no store projects it, and a build without the list keeps it as it is); on every welcome where it is not as it is, and once an hour while connected. Phloem's presence by slot says who is online, the live way how. The leader keeps them for every tab (`betula.sync.p<id>.devices`, `.slots`) |
| „Entfernen" of another device | its record marked removed: it stands in no list, and the device that finds its own record so leaves (as „Nur dieses Gerät trennen") the next time it is online. Phloem cannot take a slot from a device that never comes back: for a lost device „Sync beenden" and a new start shut it out |
| „Sync beenden" | `DELETE /sync/v1/space`: gone at once; every device is private again with its copy, the others hear `gone` |
| a profile deleted | leaves its space for this device alone; the others keep it |
| „Profil leeren", „Rückgängig" | sync like any change |

While devices connect, a bar shows the way, and then the success of concept A of
folia/design/verbunden/konzepte.html (owner, 2026-11-09: „Ja nimm bitte option a. Aber mach mal
eine animation mit progress bar für den sync an sich. So dass man sieht, dass da was passiert";
`folia_widgets::syncdone`): `ConnectProgress` names the steps — Betula Sync switched on (a private
profile), the profile sent, the other device fetching it, the profile received, the data in line —,
ticks those done, pulses the one under way and fills its part of the bar slowly, and shows each for
550 ms at least, so that steps of a few milliseconds are seen as well; it follows what really
happens: the giver's send, Phloem's `pair` frame that its parcel was opened (`Sync::taken`), one
more device of the profile online; the device that got the profile, its own connection with the
profile there. It ends only then: after 8 s without, a line under the bar says the profile is not
here yet and Betula keeps trying (until Folia 1.90.0 the bar ended after 8 s all the same, and
said „Verbunden" for a profile that never came). `Connected` then: a tick, „Verbunden" (from the setup „Alles eingerichtet" with what
was set up), what is on now in three rows (on both devices, in line at once and offline, sealed
without an account), that more devices are connected on the profile page („Gerät verbinden"), and
„Fertig".

Every way of connecting ends on the start page (owner, 2026-11-09: „dass man wieder bei der main
page landet, wenn das sync fertig ist und nicht im profil"): „Fertig" of the device that gave its
profile (the question after a code, the dialog „Gerät verbinden") and of the device that got one
(`/pair`, a link of `/sync#…`, the dialog's „Profil vom Handy holen") is a link to `/`, which the
dialog closes on its way; from the setup's last step it ends the setup there (`/?setup=done`).

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

The Merkliste has the same dialog (owner, 2026-11-09: „die Merkliste soll auch per dem neuen sync
share dialog funktionieren"; `folia/crates/bookmarks/src/share.rs`), for the whole list (the menu
of „Alle") or one collection (its menu's „Teilen …"):

| | |
|---|---|
| a fixed copy for everybody | the link of today, `/bookmarks#m=<code>` (`bookmarks::share_fragment`): the list after the `#`, so the notes may come along too |
| what stays current | a part at Phloem: `bookmarks::ListPart`, the collection's marked modules with its name (of the whole list every module and every collection), the notes where the share takes them along. `/bookmarks#live=<id ‖ key>` |
| one person | a parcel as a calendar's, `/bookmarks#once=<id ‖ key>` |
| the shares in force | lines `s` of the collections' store (`collections::ListShare`), records `C\|s\|<id>`: renewed by the leader when the marks, the collections or the notes change; a collection deleted takes its shares along, and their parts go |
| following | „Als Liste folgen": a line `f` of the collections' store (`collections::Followed`, records `C\|f\|<id>`), a list of its own under „Geteilt mit dir" beside the collections, under a number of theirs (`?collection=<n>`), named as the sharer names it until it is renamed; the page lists its modules with the sharer's collections and notes, the rows' marks are the visitor's, „Alle übernehmen" takes everything (`Shelf::adopt`). Watched and fetched as a calendar's part; a part that is gone leaves the list with its last state, „Behalten" and „Löschen" |

The start page's setup ends with it (owner, 2026-11-08: „Mach das mal als step in die einführung
mit rein. So als letzten schritt. So dass man es gleich auch auf dem handy hat";
`folia/crates/home/src/home/devices.rs`, docs/folia/frontend.md „The start page"): after
„Studium anlegen", „Betula Sync aktivieren" with what the server keeps above it, then the link for
the phone at once, with the profile page's QR code (`folia_widgets::syncpair`), and „Verbunden"
once one more device is online. On a phone's layout it names the address a laptop opens, `/pair`,
whose code the phone scans (owner, the same day: „Oh der pc prozess muss anders sein. Da muss ein
link kommen wie betula.app/pair wo dann der qr code gezeigt wird, den man am handy scannen kann um
sich einzuloggen"), with „Code scannen" to Betula's scanner (`/sync?from=setup`, whose „Fertig"
ends the setup on the start page once the code is answered), since the camera app would open the code in a browser that
may keep other storage than Betula on the home screen; its step 1 leads to `/pair` with „Profil
übernehmen".

## 5. Checks

- `cargo test -p folia-sync`: the stores' round trips, the cases of two devices, a simulation of
  three and four devices online and offline over a network that reorders and doubles, one joining
  from a snapshot, shares and follows among them (`FOLIA_SYNC_SEEDS=600` and `--release` for a
  longer search).
- `cargo test -p folia-stores sync`: the codes, the frames, what the browser keeps.
- `folia/e2e/sync.mjs`: two browsers of one profile against a running Phloem; the start page's
  setup to its last step, the study on a second browser through its QR code's link; on a phone
  the address `/pair` for the laptop, its code opened by the phone, the digits alike on both, a
  private profile switched on on the way, a code that opens once; „Profil vom Handy holen" in the
  profile page's dialog answered by a synced phone; the scanner of `/sync` with a camera that films
  a code.
- `folia/e2e/share.mjs`: sharing and following with four browsers against a running Phloem
  (Folia with `FOLIA_SYNC_PARTS` naming it).
- `folia/e2e/list-share.mjs`: a collection of the Merkliste shared and followed with three
  browsers against a running Phloem; the whole Merkliste as the link of today.
- `folia/e2e/live.mjs`: two browsers connect directly, and with Phloem's entries to the second held
  back a mark still arrives; the signals reach Phloem sealed. `sync.mjs` keeps WebRTC off
  (Playwright's offline mode does not reach it).
- `deploy/test/turn.sh` (as root, network namespaces) with `folia/e2e/turn.mjs`: two homes behind
  NATs that give every destination a port of its own; the direct attempt fails, TURN opens, a mark
  goes through the relay alone, and Phloem's firewall counts no drop.
