CREATE INDEX IF NOT EXISTS idx_visits_visible_profile_time_id
  ON visits(source_profile_id, visit_time_ms ASC, id ASC)
  WHERE reverted_at IS NULL;
