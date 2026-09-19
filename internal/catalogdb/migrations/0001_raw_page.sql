-- Raw page archive: the latest fetched body of every source page.
-- Canonical data is derived from these rows, so a parser fix never needs the network.
-- The archive is not part of the exported snapshot.
CREATE TABLE raw_page (
	id           INTEGER PRIMARY KEY,
	source       TEXT    NOT NULL,   -- module_catalog | module_page | qis_event | qis_fues_list | qis_tree
	key          TEXT    NOT NULL,   -- module id, veranstid, QIS node URL, 'list'
	source_url   TEXT    NOT NULL,
	fetched_at   TEXT    NOT NULL,   -- RFC 3339, UTC: last successful request
	changed_at   TEXT    NOT NULL,   -- RFC 3339, UTC: last time content_hash changed
	http_status  INTEGER NOT NULL,
	content_hash TEXT    NOT NULL,   -- sha256 of the uncompressed body, '' without body
	body_gz      BLOB,               -- gzip; NULL when the response had no usable body
	UNIQUE (source, key)
);

CREATE INDEX idx_raw_page_source_fetched ON raw_page(source, fetched_at);
