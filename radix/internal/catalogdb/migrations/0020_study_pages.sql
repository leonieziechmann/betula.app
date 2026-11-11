-- The BTU's pages around the study (owner, 2026-11-11: „Mach die Rund ums Studium Seiten auch über
-- Radix"): the tiles of its page „Studienorganisation", German
-- (b-tu.de/studium/im-studium/studienorganisation) and English
-- (b-tu.de/en/study/during-studies/study-organization), raw_page source 'btu_study_organisation',
-- keys 'de' and 'en'. Folia's „Rund ums Studium" in „Ordnungen & Dokumente" shows them. Until then
-- Folia wrote 14 of them by hand (folia/crates/planner/src/study/btu.rs).
--
-- study_page.lang      the page that lists it: 'de' or 'en'
-- study_page.ord       its place on that page, top to bottom
-- study_page.section   the heading of its group („Prüfungen", „Formalia"), as the page writes it
-- study_page.title     the tile's title, as the page writes it
-- study_page.abstract  the sentence under it; NULL where there is none
-- study_page.url       its address: mostly the BTU's site, a few elsewhere (the student body's
--                      Semesterticket); a fragment points into a page
--
-- Existing data is not rewritten: the table stays empty until a build finds the page archived.

CREATE TABLE study_page (
	lang       TEXT    NOT NULL CHECK (lang IN ('de', 'en')),
	ord        INTEGER NOT NULL CHECK (ord >= 1),
	section    TEXT    NOT NULL,
	title      TEXT    NOT NULL,
	abstract   TEXT,
	url        TEXT    NOT NULL,
	source_url TEXT    NOT NULL,
	PRIMARY KEY (lang, ord)
) WITHOUT ROWID;

CREATE VIEW v_study_page AS
SELECT lang, ord, section, title, abstract, url FROM study_page;
