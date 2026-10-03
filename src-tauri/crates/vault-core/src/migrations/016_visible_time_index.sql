-- Migration 016 — index visible visits by time for the History list.
--
-- The History list reads visible visits newest-first (or oldest-first) across
-- every profile, 100 rows at a time:
--
--   SELECT ... FROM visits JOIN urls ... JOIN source_profiles ...
--   WHERE visits.reverted_at IS NULL [AND filters]
--   ORDER BY visits.visit_time_ms DESC, visits.id DESC
--   LIMIT 101
--
-- The visible-visit time indexes that existed are keyed by profile (007) or by
-- URL (005) first, so an all-profile page had to sort every visible visit in a
-- temp B-tree before returning 101 rows: about 0.9 s at 1M visits on a fast
-- machine, so many seconds and hundreds of MB of sort space at the 14.4M
-- target. Walking this index returns the first page after reading ~101 entries,
-- and a cursor page seeks straight to its position.
--
-- Cost: one entry per visible visit (~20 bytes), about 300 MB at 14.4M visits,
-- built once on upgrade behind the "Upgrading your archive" screen.
CREATE INDEX IF NOT EXISTS idx_visits_visible_time_id
  ON visits(visit_time_ms, id)
  WHERE reverted_at IS NULL;
