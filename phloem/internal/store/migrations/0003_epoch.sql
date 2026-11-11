-- What this database is (docs/phloem/protocol.md, since Phloem 0.5.0): its epoch, 16 random bytes
-- (as hex) it gets once, when it is made or first migrated to this version. The welcome names it,
-- and so does the refusal of a hello: a device that knew another epoch learns that Phloem lost
-- what it held, and makes its space anew from what it has (owner, 2026-11-11: „mach es im
-- zweifelsfall so, dass es aus den clients rekonstruiert wird, falls es mal verloren geht").
INSERT INTO meta (key, value) VALUES ('epoch', lower(hex(randomblob(16))));
