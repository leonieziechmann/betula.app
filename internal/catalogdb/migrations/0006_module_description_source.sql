-- The catalog is maintained in QIS; b-tu.de/modul renders a copy of it that can be a
-- semester behind: in September 2026 the copy still named the events of the summer
-- while QIS already had the winter ones. The fields of a module are now read from the
-- QIS description wherever QIS has one, and this column says which page they came
-- from, so that a validation baseline and docs/data-sources.md can name the source.
-- The modules QIS no longer lists (no longer offered) keep their copy as the source.
ALTER TABLE module ADD COLUMN description_source TEXT
	CHECK (description_source IN ('qis', 'btu_cms'));
