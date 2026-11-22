CREATE TABLE incoming.copy_revisions(id INTEGER PRIMARY KEY);
INSERT INTO incoming.copy_revisions
WITH RECURSIVE wanted(id) AS (
    SELECT r.id FROM main.ah_import_conversations i
    JOIN main.ah_source_conversations c ON c.conversation_id=i.conversation_id
    JOIN main.ah_revisions r ON r.source_id=c.source_id
    WHERE i.import_id=?1 AND i.eligible=1 AND r.state='complete'
    UNION
    SELECT b.target_revision FROM main.ah_reference_bindings b JOIN wanted w ON w.id=b.parent_revision
    UNION
    SELECT r.manifest_revision_id FROM main.ah_revisions r JOIN wanted w ON w.id=r.id
    WHERE r.manifest_revision_id IS NOT NULL
) SELECT id FROM wanted;

INSERT INTO incoming.ah_sources
SELECT s.* FROM main.ah_sources s WHERE EXISTS (
    SELECT 1 FROM main.ah_revisions r JOIN incoming.copy_revisions w ON w.id=r.id WHERE r.source_id=s.id
);
UPDATE incoming.ah_sources SET latest_revision_id=NULL
WHERE latest_revision_id NOT IN (SELECT id FROM incoming.copy_revisions);
INSERT INTO incoming.ah_revisions SELECT r.* FROM main.ah_revisions r JOIN incoming.copy_revisions w ON w.id=r.id;

CREATE TABLE incoming.copy_chunks(hash TEXT PRIMARY KEY);
INSERT OR IGNORE INTO incoming.copy_chunks
SELECT c.chunk_hash FROM main.ah_revision_chunks c JOIN incoming.copy_revisions w ON w.id=c.revision_id;
