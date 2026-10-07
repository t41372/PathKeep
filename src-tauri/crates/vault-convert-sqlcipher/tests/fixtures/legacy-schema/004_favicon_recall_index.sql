CREATE INDEX IF NOT EXISTS idx_favicons_recall_lookup
  ON favicons(
    source_profile_id,
    page_url,
    last_updated_ms DESC,
    width DESC,
    height DESC,
    id DESC
  )
  WHERE image_data IS NOT NULL;
