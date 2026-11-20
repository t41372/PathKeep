INSERT OR IGNORE INTO main.ah_sources
SELECT m.new,s.harness,s.root,s.relative_path,s.kind,s.retention,s.last_seen_at,s.missing_since,r.new,s.observed_at
FROM incoming.ah_sources s JOIN move_source m ON m.old=s.id LEFT JOIN move_revision r ON r.old=s.latest_revision_id;

INSERT OR IGNORE INTO main.ah_revisions
SELECT m.new,src.new,r.digest,r.byte_len,r.captured_at,r.state,r.capture_format,r.codec,
       r.bytes_retained,body.new,r.next_ordinal,r.staged_byte_len
FROM incoming.ah_revisions r JOIN move_revision m ON m.old=r.id
JOIN move_source src ON src.old=r.source_id LEFT JOIN move_revision body ON body.old=r.manifest_revision_id;

UPDATE main.ah_sources AS h
SET latest_revision_id=v.new, observed_at=i.observed_at, missing_since=i.missing_since,last_seen_at=i.last_seen_at
FROM incoming.ah_sources i JOIN move_source m ON m.old=i.id JOIN move_revision v ON v.old=i.latest_revision_id
WHERE h.id=m.new AND i.observed_at>=h.observed_at;

INSERT OR IGNORE INTO main.ah_revision_chunks
SELECT m.new,c.ordinal,c.chunk_hash,c.byte_len,c.byte_offset
FROM incoming.ah_revision_chunks c JOIN move_revision m ON m.old=c.revision_id;

INSERT OR IGNORE INTO main.ah_parse_reports
SELECT m.new,r.parser_version,r.report_json FROM incoming.ah_parse_reports r JOIN move_revision m ON m.old=r.revision_id;

INSERT INTO main.ah_reference_bindings(parent_revision,reference_key,target_revision,observed_at)
SELECT p.new,b.reference_key,t.new,b.observed_at FROM incoming.ah_reference_bindings b
JOIN move_revision p ON p.old=b.parent_revision JOIN move_revision t ON t.old=b.target_revision
WHERE NOT EXISTS(SELECT 1 FROM main.ah_reference_bindings h
 WHERE h.parent_revision=p.new AND h.reference_key=b.reference_key AND h.target_revision=t.new AND h.observed_at=b.observed_at);

INSERT INTO main.ah_reference_observations(parent_revision,reference_key,reason,observed_at)
SELECT p.new,o.reference_key,o.reason,o.observed_at FROM incoming.ah_reference_observations o
JOIN move_revision p ON p.old=o.parent_revision WHERE NOT EXISTS(SELECT 1 FROM main.ah_reference_observations h
 WHERE h.parent_revision=p.new AND h.reference_key=o.reference_key AND h.observed_at=o.observed_at);

INSERT OR IGNORE INTO main.ah_user_state
SELECT i.* FROM incoming.ah_user_state i WHERE NOT EXISTS
(SELECT 1 FROM main.ah_source_conversations h WHERE h.conversation_id=i.conversation_id);
INSERT OR IGNORE INTO main.ah_conversation_digests SELECT * FROM incoming.ah_conversation_digests;
INSERT OR REPLACE INTO main.ah_source_conversations
SELECT m.new,c.conversation_id,c.is_child,r.new FROM incoming.ah_source_conversations c
JOIN move_source m ON m.old=c.source_id JOIN move_revision r ON r.old=c.revision_id
JOIN incoming.ah_sources src ON src.id=c.source_id JOIN main.ah_sources h ON h.id=m.new
WHERE src.observed_at>=h.observed_at;

INSERT OR IGNORE INTO main.ah_backup_runs SELECT m.new,r.status,r.started_at,r.finished_at,r.report_json,r.public_id
FROM incoming.ah_backup_runs r JOIN move_run m ON m.old=r.id;
INSERT OR IGNORE INTO main.ah_capture_selections SELECT * FROM incoming.ah_capture_selections;
INSERT OR IGNORE INTO main.ah_desktop_backups SELECT m.new,d.task_id,d.request_id,d.request_hash,d.selection_id,
 d.scope_json,d.trigger,d.retry_of,d.stop_reason,d.acknowledged
FROM incoming.ah_desktop_backups d JOIN move_run m ON m.old=d.run_id;
INSERT OR IGNORE INTO main.ah_backup_deliveries SELECT * FROM incoming.ah_backup_deliveries;
INSERT OR IGNORE INTO main.ah_run_totals SELECT m.new,t.harness,t.root,t.metric,t.quantity FROM incoming.ah_run_totals t JOIN move_run m ON m.old=t.run_id;
INSERT OR IGNORE INTO main.ah_run_roots SELECT m.new,t.root_id,t.tool,t.state FROM incoming.ah_run_roots t JOIN move_run m ON m.old=t.run_id;
INSERT OR IGNORE INTO main.ah_run_file_problems SELECT r.new,coalesce(s.new,t.source_id),t.metric
FROM incoming.ah_run_file_problems t JOIN move_run r ON r.old=t.run_id LEFT JOIN move_source s ON s.old=t.source_id;
INSERT OR IGNORE INTO main.ah_run_files SELECT r.new,coalesce(s.new,t.source_id),v.new,t.bytes,t.outcome,t.classified
FROM incoming.ah_run_files t JOIN move_run r ON r.old=t.run_id LEFT JOIN move_source s ON s.old=t.source_id LEFT JOIN move_revision v ON v.old=t.revision_id;
INSERT OR IGNORE INTO main.ah_run_conversations SELECT r.new,t.conversation_id,s.new,t.outcome FROM incoming.ah_run_conversations t
JOIN move_run r ON r.old=t.run_id JOIN move_source s ON s.old=t.source_id;
INSERT OR IGNORE INTO main.ah_source_observations SELECT r.new,s.new,t.seen,t.observed_at FROM incoming.ah_source_observations t
JOIN move_run r ON r.old=t.run_id JOIN move_source s ON s.old=t.source_id;
INSERT OR IGNORE INTO main.ah_root_state SELECT t.harness,t.root,t.state,r.new FROM incoming.ah_root_state t JOIN move_run r ON r.old=t.run_id;
INSERT OR IGNORE INTO main.ah_capture_notes SELECT n.new,t.root_id,t.tool,t.subject,t.code,t.qualifier,r.new FROM incoming.ah_capture_notes t
JOIN move_note n ON n.old=t.id JOIN move_run r ON r.old=t.first_run;
INSERT OR IGNORE INTO main.ah_run_notes SELECT r.new,n.new,t.path,t.ordinal,t.detail FROM incoming.ah_run_notes t JOIN move_run r ON r.old=t.run_id JOIN move_note n ON n.old=t.note_id;
INSERT OR IGNORE INTO main.ah_run_note_counts SELECT r.new,t.root_id,t.tool,t.code,t.version,t.seen,t.fresh
FROM incoming.ah_run_note_counts t JOIN move_run r ON r.old=t.run_id;
INSERT OR IGNORE INTO main.ah_format_notices SELECT t.tool,t.version,r.new,t.dismissed FROM incoming.ah_format_notices t JOIN move_run r ON r.old=t.run_id;
INSERT INTO main.ah_folder_observations(path,exists_on_disk,observed_at,git_json)
SELECT t.path,t.exists_on_disk,t.observed_at,t.git_json FROM incoming.ah_folder_observations t
WHERE NOT EXISTS(SELECT 1 FROM main.ah_folder_observations h WHERE h.path=t.path AND h.observed_at=t.observed_at);
INSERT OR IGNORE INTO main.ah_project_identity SELECT * FROM incoming.ah_project_identity;

DROP TABLE move_note;
DROP TABLE move_run;
DROP TABLE move_revision;
DROP TABLE move_source;
