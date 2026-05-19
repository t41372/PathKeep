-- og_images and og_image_blobs tables.
--
-- og_images stores per-page-URL link preview metadata extracted from the
-- target page's <meta property="og:image"> (with twitter:image fallback).
-- Bytes are content-addressed in og_image_blobs so identical preview images
-- (e.g. a site default fallback shared across many pages) cost storage only
-- once. The read contract is exact-page-URL ONLY — host-level fallback is
-- intentionally NOT supported because two pages on the same host (GitHub
-- repos, Medium articles) routinely have different social cards.
--
-- og_images.last_shown_at is bumped by the frontend (debounced batched
-- command) when a card is rendered, so eviction policies can prefer LRU.
-- refetch_after lets the worker negative-cache failures and missing-image
-- pages without retry storms.

CREATE TABLE IF NOT EXISTS og_image_blobs (
  blob_hash   TEXT PRIMARY KEY,
  image_data  BLOB NOT NULL,
  mime        TEXT NOT NULL,
  byte_size   INTEGER NOT NULL,
  width       INTEGER,
  height      INTEGER,
  recorded_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS og_images (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  page_url          TEXT NOT NULL,
  page_host         TEXT,
  source_og_url     TEXT,
  image_blob_hash   TEXT REFERENCES og_image_blobs(blob_hash),
  fetch_status      TEXT NOT NULL,
  http_status       INTEGER,
  fetched_at        TEXT NOT NULL,
  last_shown_at     TEXT,
  refetch_after     TEXT,
  fetch_attempts    INTEGER NOT NULL DEFAULT 1,
  created_by_run_id INTEGER REFERENCES runs(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_og_images_page_url
  ON og_images(page_url);

CREATE INDEX IF NOT EXISTS idx_og_images_blob_hash
  ON og_images(image_blob_hash)
  WHERE image_blob_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_og_images_refetch
  ON og_images(refetch_after)
  WHERE refetch_after IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_og_images_last_shown
  ON og_images(last_shown_at)
  WHERE last_shown_at IS NOT NULL;
