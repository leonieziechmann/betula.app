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
| something added to a calendar the other deleted | gone with the calendar: a calendar lives a life, and what belongs to it counts only within it, also when its number is taken again |
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
| „Betula Sync aktivieren" | a key, a device id, `POST /sync/v1/space`, the profile's records as its first snapshot |
| „Weiteres Gerät verbinden" | a parcel with the key, sealed with a key the link alone holds (`/sync#<id ‖ key>`; `/verbinden#…` until 2026-11-08, which leads there), ten minutes at Phloem, for one device |
| joining | the parcel opened, a new profile of the browser, its records from Phloem |
| the recovery code | `K` in base32 (Crockford), 13 groups of four, the last 4 bits a check: most typos are told before any request |
| „Nur dieses Gerät trennen" | what waits goes out, then `leave`; the copy stays, private |
| „Sync beenden" | `DELETE /sync/v1/space`: gone at once; every device is private again with its copy, the others hear `gone` |
| a profile deleted | leaves its space for this device alone; the others keep it |
| „Profil leeren", „Rückgängig" | sync like any change |

## 4. Checks

- `cargo test -p folia-sync`: the stores' round trips, the cases of two devices, a simulation of
  three and four devices online and offline over a network that reorders and doubles, one joining
  from a snapshot (`FOLIA_SYNC_SEEDS=600` and `--release` for a longer search).
- `cargo test -p folia-stores sync`: the codes, the frames, what the browser keeps.
- `folia/e2e/sync.mjs`: two browsers of one profile against a running Phloem.
