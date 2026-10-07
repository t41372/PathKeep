CREATE TABLE IF NOT EXISTS favicon_blobs (
  blob_hash   TEXT PRIMARY KEY,
  image_data  BLOB NOT NULL,
  recorded_at TEXT NOT NULL
);

ALTER TABLE favicons ADD COLUMN image_blob_hash TEXT REFERENCES favicon_blobs(blob_hash);

DROP INDEX IF EXISTS idx_favicons_recall_lookup;
CREATE INDEX IF NOT EXISTS idx_favicons_recall_lookup
ON favicons(
  source_profile_id,
  page_url,
  last_updated_ms DESC,
  width DESC,
  height DESC,
  id DESC
)
WHERE image_blob_hash IS NOT NULL OR image_data IS NOT NULL;

DROP INDEX IF EXISTS idx_favicons_page_lookup;
CREATE INDEX IF NOT EXISTS idx_favicons_page_lookup
ON favicons(
  page_url,
  last_updated_ms DESC,
  width DESC,
  height DESC,
  id DESC
)
WHERE image_blob_hash IS NOT NULL OR image_data IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_favicons_blob_hash
ON favicons(image_blob_hash)
WHERE image_blob_hash IS NOT NULL;
