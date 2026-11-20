CREATE TABLE move_source(old TEXT PRIMARY KEY, new TEXT NOT NULL);
INSERT INTO move_source
SELECT id, CASE WHEN id LIKE 'move:%' THEN id ELSE 'move:' || ?1 || ':' || id END
FROM incoming.ah_sources;

CREATE TABLE move_revision(old INTEGER PRIMARY KEY, new INTEGER NOT NULL);
INSERT INTO move_revision
SELECT r.id, coalesce((SELECT min(h.id) FROM main.ah_revisions h
    WHERE h.source_id=m.new AND h.digest=r.digest AND h.capture_format=r.capture_format
        AND h.captured_at=r.captured_at), r.id+(SELECT coalesce(max(id),0) FROM main.ah_revisions))
FROM incoming.ah_revisions r JOIN move_source m ON m.old=r.source_id WHERE r.state='complete';

CREATE TABLE move_run(old INTEGER PRIMARY KEY, new INTEGER NOT NULL);
INSERT INTO move_run
SELECT r.id,coalesce(h.id,r.id+(SELECT coalesce(max(id),0) FROM main.ah_backup_runs))
FROM incoming.ah_backup_runs r LEFT JOIN main.ah_backup_runs h ON h.public_id=r.public_id;

CREATE TABLE move_note(old INTEGER PRIMARY KEY,new INTEGER NOT NULL);
INSERT INTO move_note
SELECT n.id,coalesce(h.id,n.id+(SELECT coalesce(max(id),0) FROM main.ah_capture_notes))
FROM incoming.ah_capture_notes n LEFT JOIN main.ah_capture_notes h ON
    h.root_id=n.root_id AND h.subject=n.subject AND h.code=n.code AND h.qualifier=n.qualifier;
