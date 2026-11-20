INSERT INTO incoming.ah_revision_chunks SELECT c.* FROM main.ah_revision_chunks c JOIN incoming.copy_revisions w ON w.id=c.revision_id;
INSERT INTO incoming.ah_parse_reports SELECT p.* FROM main.ah_parse_reports p JOIN incoming.copy_revisions w ON w.id=p.revision_id;
INSERT INTO incoming.ah_reference_bindings SELECT b.* FROM main.ah_reference_bindings b JOIN incoming.copy_revisions w ON w.id=b.parent_revision;
INSERT INTO incoming.ah_reference_observations SELECT o.* FROM main.ah_reference_observations o JOIN incoming.copy_revisions w ON w.id=o.parent_revision;

INSERT INTO incoming.ah_source_conversations SELECT c.* FROM main.ah_source_conversations c
JOIN main.ah_import_conversations i ON i.conversation_id=c.conversation_id WHERE i.import_id=?1 AND i.eligible=1;
INSERT INTO incoming.ah_user_state SELECT u.* FROM main.ah_user_state u
JOIN main.ah_import_conversations i ON i.conversation_id=u.conversation_id WHERE i.import_id=?1 AND i.eligible=1;
INSERT INTO incoming.ah_conversation_digests SELECT d.* FROM main.ah_conversation_digests d
JOIN main.ah_import_conversations i ON i.conversation_id=d.conversation_id WHERE i.import_id=?1 AND i.eligible=1;

DROP TABLE incoming.copy_chunks;
DROP TABLE incoming.copy_revisions;
