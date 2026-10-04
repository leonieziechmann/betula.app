-- The semantic search (Folia's crate semantic/): one vector per module, which a reader's
-- query is compared with. Radix computes them (its semantic stage, internal/service, with the
-- crate as WebAssembly, internal/embed), so a snapshot carries the vectors and nothing else of it.
--
-- module_summary      what Gemini wrote about a module's text: a German and an English
--                     summary and search terms. Embedded with the module's text, it finds the
--                     module by what students type rather than by the words of the
--                     description. Not published (export drops it): it is not the
--                     university's text.
-- passage_embedding   the vector of one passage (a module's text, with its summary when there
--                     is one) under the model that computed it. Not published either.
-- module_vector       derived by every build: each module's vector, looked up in
--                     passage_embedding by the hash of the module's passage of this build.
--                     Published, read through v_module_vector.
--
-- The two caches are a source of their own, like plan: no build touches them, and they are
-- keyed by hashes, not by module, so a module that leaves the lists and comes back, or two
-- modules with the same text, cost no second request. A module whose text changed gets its
-- vector one cycle later: the semantic stage runs after the build that brought the text.

CREATE TABLE module_summary (
	text_hash   TEXT PRIMARY KEY,                -- semantic.Text.Hash: titles, contents, learning outcomes
	summary_de  TEXT NOT NULL,
	summary_en  TEXT NOT NULL,
	keywords    TEXT NOT NULL,                   -- one search term a line
	model       TEXT NOT NULL,                   -- the Gemini model that wrote it
	created_at  TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE passage_embedding (
	passage_hash TEXT PRIMARY KEY,               -- semantic.PassageHash of the passage
	model        TEXT NOT NULL,                  -- the encoder's ID (internal/embed): a hash of the model file
	scale        REAL NOT NULL CHECK (scale > 0),
	vector       BLOB NOT NULL,                  -- packed as published (below)
	created_at   TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE module_vector (
	module_id   TEXT PRIMARY KEY REFERENCES module(id) ON DELETE CASCADE,
	scale       REAL NOT NULL CHECK (scale > 0),
	vector      BLOB NOT NULL
) WITHOUT ROWID;

-- The contract for Folia: a module's vector is the model's dims values (384 for e5-small) of 4
-- bits, two to a byte in `vector`: a value v of -7..7 is the nibble v + 8, the first value in
-- the low nibble; the vector is the values times `scale`, of unit length up to their rounding
-- (semantic::quantize, which also reads them: semantic::Index::push_codes). 4 bits rather than 8
-- halve the snapshot's vectors to about 1 MB. Modules without a vector yet have no row.
CREATE VIEW v_module_vector AS
SELECT module_id, scale, vector FROM module_vector;
