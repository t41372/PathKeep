CREATE INDEX IF NOT EXISTS idx_visits_visible_url_time
  ON visits(
    url_id,
    visit_time_ms DESC,
    id DESC
  )
  WHERE reverted_at IS NULL;
