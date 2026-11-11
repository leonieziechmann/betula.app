-- What the BTU has for every program (owner, 2026-11-10: Radix fetches the BTU's page of
-- Grundsatzordnungen itself, archives it as its other sources, and the build turns its sections into
-- a table): the documents of that page, German
-- (b-tu.de/studium/im-studium/studienorganisation/pruefungen/ordnungen-und-formulare/grundsatzordnungen)
-- and English (b-tu.de/en/study/during-studies/study-organization/examinations/regulations-and-forms/
-- basic-regulations), raw_page source 'btu_general_documents', keys 'de' and 'en'. Until then Folia
-- wrote the Rahmenordnungen and the Immatrikulationsordnung by hand (folia/crates/planner/src/study/
-- btu.rs) and a check said when they drifted apart.
--
-- general_document.lang     the page that lists it: 'de' or 'en'; the English page lists the BTU's
--                           translations where the German one has the regulation, and the German
--                           amendments
-- general_document.ord      its place on that page, top to bottom
-- general_document.section  the heading of its section, as the page writes it
-- general_document.topic    what it is about, where Folia names it (normalize.GeneralTopic): the
--                           Rahmenordnung for Bachelor- or for Master-Studiengänge, the
--                           Immatrikulationsordnung (their sections, amendments and all), the
--                           Ordnung zum Fachübergreifenden Studium, the Brandenburgisches
--                           Hochschulgesetz; NULL for the rest of the page
-- general_document.kind     'statute' in the section itself, 'amendment' in its fold of amendments
--                           („Änderungssatzungen"), 'other' in another fold
-- general_document.title    what the page writes for it, without „(PDF)"; its dates are the BTU's,
--                           and some are wrong („Zweite Änderungssatzung … vom 12. September 2016" of
--                           ABl. 17/2022)
-- general_document.issue    its issue of the Amtsblatt, „13/2016" (normalize.GazetteIssue: the
--                           number begins the name of its file), NULL where none can be told (a
--                           translation, the law)
-- general_document.url      its address
--
-- Existing data is not rewritten: the table stays empty until a build finds the page archived.

CREATE TABLE general_document (
	lang       TEXT    NOT NULL CHECK (lang IN ('de', 'en')),
	ord        INTEGER NOT NULL CHECK (ord >= 1),
	section    TEXT    NOT NULL,
	topic      TEXT             CHECK (topic IN ('framework_bachelor', 'framework_master', 'enrolment', 'fues', 'higher_education_act')),
	kind       TEXT    NOT NULL CHECK (kind IN ('statute', 'amendment', 'other')),
	title      TEXT    NOT NULL,
	issue      TEXT,
	url        TEXT    NOT NULL,
	source_url TEXT    NOT NULL,
	PRIMARY KEY (lang, ord)
) WITHOUT ROWID;

CREATE VIEW v_general_document AS
SELECT lang, ord, section, topic, kind, title, issue, url FROM general_document;
