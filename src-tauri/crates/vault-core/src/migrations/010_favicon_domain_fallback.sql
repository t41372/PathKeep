ALTER TABLE favicons ADD COLUMN page_host TEXT;
ALTER TABLE favicons ADD COLUMN page_registrable_domain TEXT;

CREATE INDEX IF NOT EXISTS idx_favicons_host_profile_lookup
ON favicons(
  source_profile_id,
  page_host,
  last_updated_ms DESC,
  width DESC,
  height DESC,
  id DESC
)
WHERE page_host IS NOT NULL
  AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL);

CREATE INDEX IF NOT EXISTS idx_favicons_host_lookup
ON favicons(
  page_host,
  last_updated_ms DESC,
  width DESC,
  height DESC,
  id DESC
)
WHERE page_host IS NOT NULL
  AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL);

CREATE INDEX IF NOT EXISTS idx_favicons_registrable_profile_lookup
ON favicons(
  source_profile_id,
  page_registrable_domain,
  last_updated_ms DESC,
  width DESC,
  height DESC,
  id DESC
)
WHERE page_registrable_domain IS NOT NULL
  AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL);

CREATE INDEX IF NOT EXISTS idx_favicons_registrable_lookup
ON favicons(
  page_registrable_domain,
  last_updated_ms DESC,
  width DESC,
  height DESC,
  id DESC
)
WHERE page_registrable_domain IS NOT NULL
  AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL);
