CREATE INDEX IF NOT EXISTS idx_favicons_page_lookup
  ON favicons(
    page_url,
    last_updated_ms DESC,
    width DESC,
    height DESC,
    id DESC
  )
  WHERE image_data IS NOT NULL;
