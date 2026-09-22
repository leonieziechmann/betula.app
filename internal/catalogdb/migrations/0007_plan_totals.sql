-- What a study plan adds up to, as the regulation itself prints it.
--
-- A plan row says „Komplex Praktische Informatik, 10-24 LP". Three such rows are
-- anything between 30 and 72 LP, so a plan with elective budgets cannot be added
-- up from its rows: adding the lower bounds made Informatik 2008 a degree of 166
-- LP instead of 180, and the last two semesters, whose rows share one merged
-- column, had no sum at all. The regulation does print the answer — „Summe
-- Komplexe des Fachstudiums 44" over those three rows, „Summe Studium 32 28 30
-- 30 60" over the whole table — and until now that was thrown away after it had
-- been validated.
--
-- plan_total keeps each printed sum with the semesters it covers and the rows it
-- counts. A sum is stored only where those rows reach it (exactly, or within the
-- range their budgets leave open), so a total is evidence, never an assumption.
-- A module over several semesters belongs to none of them alone — the plan does
-- not say how it splits — so it only raises max_credits of a semester it reaches
-- into, and a semester whose modules all reach into it that way names no row at
-- all (entry_count 0). The printed sum is then the only thing the plan says about
-- that semester, and dropping it would leave the semester without a figure.

CREATE TABLE plan_total (
	program_id      TEXT    NOT NULL REFERENCES plan(program_id) ON DELETE CASCADE,
	ord             INTEGER NOT NULL,
	label           TEXT    NOT NULL,          -- as printed: „Summe Studium", „Summe Komplex Mathematik"
	scope           TEXT    NOT NULL CHECK (scope IN ('plan', 'section')),  -- the whole plan of these semesters, or a part of it
	specialization  TEXT,                      -- the plan variant this sum belongs to, where the document prints several
	start_semester  INTEGER NOT NULL,
	end_semester    INTEGER NOT NULL,          -- equals start_semester unless the sum covers a merged column
	credits         REAL    NOT NULL,          -- what the plan prints
	min_credits     REAL    NOT NULL,          -- what the rows inside these semesters come to …
	max_credits     REAL    NOT NULL,          -- … and what they and a row reaching into them could come to
	is_choice       INTEGER NOT NULL CHECK (is_choice IN (0, 1)),  -- every row named lies inside, and one of them prints a range: this sum is the only statement of how much they count for
	entry_count     INTEGER NOT NULL,
	source_evidence TEXT,                      -- the PDF cell the value was read from
	PRIMARY KEY (program_id, ord)
);

CREATE TABLE plan_total_entry (
	program_id TEXT    NOT NULL,
	total_ord  INTEGER NOT NULL,
	entry_ord  INTEGER NOT NULL,               -- plan_entry.ord of a row this sum counts
	PRIMARY KEY (program_id, total_ord, entry_ord),
	FOREIGN KEY (program_id, total_ord) REFERENCES plan_total(program_id, ord) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX plan_total_entry_by_entry ON plan_total_entry (program_id, entry_ord);

-- The sums of a plan, in the order the document prints them.
CREATE VIEW v_program_plan_total AS
SELECT t.program_id, t.ord, t.label, t.scope, t.specialization,
       t.start_semester, t.end_semester, t.credits, t.min_credits, t.max_credits,
       t.is_choice, t.entry_count, t.source_evidence
FROM plan_total t;

-- Which rows each sum counts.
CREATE VIEW v_program_plan_total_entry AS
SELECT te.program_id, te.total_ord, te.entry_ord
FROM plan_total_entry te;
