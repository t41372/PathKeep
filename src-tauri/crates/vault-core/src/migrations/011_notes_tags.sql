-- url_annotations and url_tags tables.
--
-- These store per-URL notes and tags written from the Browse detail panel.
-- Keyed by URL (not by visit id) so a note travels across re-visits of the
-- same page. profile_id is captured at write time for audit purposes but is
-- not part of the primary key — annotations are user-authored content and
-- should not be partitioned by the browser they originally came from.

CREATE TABLE IF NOT EXISTS url_annotations (
  url            TEXT PRIMARY KEY,
  notes          TEXT NOT NULL DEFAULT '',
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL,
  source_profile TEXT
);

CREATE TABLE IF NOT EXISTS url_tags (
  url            TEXT NOT NULL,
  tag            TEXT NOT NULL,
  created_at     TEXT NOT NULL,
  source_profile TEXT,
  PRIMARY KEY (url, tag)
);

CREATE INDEX IF NOT EXISTS idx_url_annotations_updated_at
  ON url_annotations(updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_url_tags_tag
  ON url_tags(tag, url);
