-- Search (docs/schema-v2.md, „Search“): what Folia's search compares a query with, folded once
-- by every build instead of by every reader. SQLite's LIKE folds ASCII only: „übung“ missed
-- „Übung“, „okologie“ found none of the 21 modules on „Ökologie“, and the words of a title were
-- found only as one phrase, in the order the title has them. Only the names of a module are
-- folded, not the texts of its description (owner, 2026-09-30): those are what the semantic
-- search compares a query with (0011).
--
-- module_folded   one row per module
--   title_de, title_en   normalize.SearchText of the title: lower case, ß → ss, without the
--                        diacritics of the common Latin letters (what Folia's catalog::search::fold
--                        does to a query), the words separated by one space, and after them once
--                        more as one word each part of the title written in parts („B.Sc.“ also
--                        as bsc); NULL without that title
--   initials             the first letters of the words of each title, the fillers („und“, „der“,
--                        „für“ …) left out, one word per title: „ti“ for „Theoretische Informatik“;
--                        NULL where no title has two
--   abbrevs              the module's abbreviations, folded into one word each („AuP-b“ is aupb):
--                        its own, every other a program gives it, and the known short forms of
--                        words of its titles (internal/abbrev: „bwl“ for Betriebswirtschaftslehre);
--                        separated by one space
--
-- v_module_search stays as it was for the readers of older schemas. Existing data is not rewritten:
-- until the next build module_folded is empty, and validate fails, so a migrated but unbuilt
-- database is never exported.

CREATE TABLE module_folded (
	module_id TEXT PRIMARY KEY REFERENCES module(id) ON DELETE CASCADE,
	title_de  TEXT,
	title_en  TEXT,
	initials  TEXT,
	abbrevs   TEXT
) WITHOUT ROWID;

CREATE VIEW v_module_folded AS
SELECT module_id, title_de, title_en, initials, abbrevs FROM module_folded;
