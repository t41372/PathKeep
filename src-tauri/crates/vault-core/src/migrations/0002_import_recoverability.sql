CREATE INDEX IF NOT EXISTS idx_visit_events_import_batch_id ON visit_events(import_batch_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_visit_events_profile_event_fingerprint ON visit_events(profile_id, event_fingerprint)
WHERE event_fingerprint IS NOT NULL AND event_fingerprint != '';
CREATE INDEX IF NOT EXISTS idx_raw_row_versions_import_batch_id ON raw_row_versions(import_batch_id);
