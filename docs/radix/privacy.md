# Personal data: the privacy register

Betula names the teachers of the university: the responsible persons of a module
(`module_person`), the persons of an event (`event_person`), the instructor of a date
(`event_date.instructor`), and in free text their names and addresses (module remarks, the comments of
dates, literature). It copies them from the university's public pages. Who asks that their data be
taken out (DSGVO Art. 17, 21) is put into the register, and from then on nothing Radix builds or
keeps names them (owner, 2026-11-19: „aktuell ist es noch nicht möglich, personenbezogene Daten
manuell/automatisiert aus dem Datensatz raus zu nehmen … Das gleiche muss auch für die History dann
funktionieren"). Package `radix/internal/privacy`, migration 0025.

## 1. The register

`privacy_person` in `radix.db`: surname and given name (both: a surname alone would take out every
namesake), further forms of the name one a line (an e-mail address, a maiden name), a note where the
request came from (a reference, never contact data), when it was added. The person stays in it for as
long as their data is to stay out: the crawl reads their name again from the university's pages every
week, and the register is what keeps it out of the next build, automatically.

The register is never published (the export drops `privacy_person` and `privacy_meta`, and its VACUUM
leaves nothing of them in the file) and never written into the repository, which is public. It travels
with `radix.db` wherever that is copied (a seed, the hand-over of the crawl, the canary's copy), so no
instance builds the catalog without it.

```bash
radix privacy add --surname Mustermann --given Max --note "Mail vom 2026-11-19"   # [--form max@example.org]...
radix privacy list
radix privacy remove --id 3            # what the build makes again comes back with it; what the history lost does not
radix privacy find --surname Mustermann --given Max --history history.db --raw    # where they are named (Art. 15)
```

On the server, in the Radix container of the colour that crawls (its image has no shell):
`docker exec <container> /bin/radix privacy add --db /data/radix.db --surname … --given …`. Every
command refuses a `--db` that does not exist rather than make one: a fresh database has an empty
register. The register has an identity of its own (`privacy_meta` `register_id`, made by the
migration): the history follows one register and refuses another radix.db.

After `privacy remove` the person's data comes back with the next build where the build makes it
from the archive (modules, events, programs); the study plans and the summaries of the semantic
search, which no build makes again, keep `[entfernt]` until a plan is scanned or a summary written
again.

## 2. What the rules take out

On folded text (lower case, without diacritics: „Müller", „MÜLLER" and „Muller" are one; the umlauts
also as „ue"):

- **a row of a list of names** (`module_person`, `event_person`) whose name is the person's surname and
  given name, in either order („Mustermann, Max", „Max Mustermann"; titles after it as QIS writes
  them): the row goes as a whole. So does a row of the surname alone, as the event search writes the
  persons of an event („Mustermann"), in the person's own modules and events, and anywhere when no
  namesake with another given name stands in the lists;
- **in every text**: given name or its initial with the surname, in either order, with a comma or
  without („Prof. Dr. Max Mustermann", „M. Mustermann", „M.Mustermann", „Mustermann, M.",
  „Mustermann Max"), with further given names written with a capital („Max Peter Mustermann", not
  „Max und Mustermann"), a title or salutation with the surname („Prof. Mustermann", „Herr
  Mustermann"), their e-mail addresses, also those written to keep harvesters off
  („max.mustermann(at)b-tu.de", „[at]", „(dot)"; local part `given.surname`, `g.surname`, `surname` and
  the like, any domain), the register's further forms;
- **the surname alone** in the rows of the person's own modules and events: those whose lists of names
  hold them, and the events of those modules (`module_event`), as „Dozent: Mustermann" in a date or a
  remark of their module; and in an instructor of a date (`event_date.instructor`, which QIS writes by
  surname) when no namesake with another given name stands in the lists.

A value of an enumeration („winter", „block", „nord": lower case, one word) is never taken for a
surname, so a person called Winter leaves the modules of the winter semester as they are.

What is taken out of a text reads `[entfernt]`; an instructor left with nothing else is NULL. A name
joined by a hyphen to another („Mustermann-Schulz") is somebody else, and so is a namesake with another
given name; a namesake with the same initial, or called „Frau Mustermann" in a text, is taken out with
the person (one place too many rather than one too few), and so is a namesake the lists know by the surname
alone. The rules apply to every TEXT column of every table but primary keys, addresses (`url`, `*_url`)
and hashes.

## 3. Where it applies

| Where | How |
|---|---|
| the catalog | every build, last of the content and before the content digest (step `privacy`, `catalogbuild.redactPersons`): what any step wrote goes, so the digest changes and the next export publishes the catalog without them. The caches beside it too (the summaries of the semantic search) |
| validate | fails while anything the rules find in free text or a list of names is left (`no person of the privacy register is left in the catalog`); it names the table, column and row, never the person. Info: `persons in the privacy register` |
| the snapshots | the export after the build that first applied a changed register removes every older snapshot (`export.privacy_pruned`): they still name the person. When that build changed nothing published (the person was not in it), the cycle removes them all the same. Folia replaces its copy within a minute, browsers theirs on the next visit |
| the history | every version (`docs/radix/history.md`): rows of the lists go from every state, names from every text; versions the redaction made equal become one. Applied when the register changed since it was last applied (`history_meta` `privacy_digest`), by the service's stage or `radix history redact`; `radix history record` applies the register to a snapshot before it records it, and refuses without one once one was applied, and with another radix.db's (`register_id`) |
| the summaries and vectors of the semantic search | the redacted texts have new hashes: their summaries and vectors are made anew by the next semantic stage |

`build.finished` counts `privacy_people`, `privacy_rows`, `privacy_values`.

## 4. What it does not reach yet

- **The raw archive** (`raw_page` in `radix.db`) keeps the pages as the university serves them, the
  name in them, and so does **Cortex**'s store, with every version for 180 days. Neither is published,
  and the crawl reads the pages again; `radix privacy find --raw` counts what the archive holds.
  Taking the name out there too means rewriting what is stored at the moment it is stored, in Radix and
  in Cortex (whose blobs are named by the hash of what the university sent): not done.
- **Exports made before**: a `radix history export` or a snapshot someone downloaded keeps what it held.
- **betula.app** runs Radix 0.10.0 (2026-11-19), which has no register: a person added on canary is out
  of canary's catalog until the next build of master gives canary a fresh copy of the public site's
  radix.db, without them. The register lasts where the crawl's radix.db lives: on the public site, once
  it runs Radix 0.15.0 or later, and from there it reaches every canary.

## 5. Checks

`radix/internal/privacy`: the forms of a name and what is not the person (part of a word, a hyphenated
other name, the surname alone outside their rows, another given name or address), folding with the
offsets back into the text, Apply on a miniature catalog (rows, texts, the instructor, idempotent),
Find without a change, the register, the surname alone of the event search (in the events of the
person's modules, and everywhere without a namesake), a surname that is a word of an enumeration, the
addresses written for harvesters, further given names. `radix/internal/catalogbuild`: a person added is out of the next
build, validate holds, the snapshot's file names them nowhere, the snapshot before is removed, and
validate fails on a name written after the build. Made-up persons only: no entry of a real register
belongs in the repository.
