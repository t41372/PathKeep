ALTER TABLE source_profiles ADD COLUMN browser_family TEXT;
ALTER TABLE source_profiles ADD COLUMN browser_product TEXT;
UPDATE source_profiles
SET browser_family = COALESCE(
      NULLIF(browser_family, ''),
      CASE
        WHEN browser_kind IN ('chrome', 'chromium', 'edge', 'edge-dev', 'brave', 'vivaldi', 'arc', 'opera', 'opera-gx')
          THEN 'chromium'
        WHEN browser_kind IN ('firefox', 'librewolf', 'floorp', 'waterfox')
          THEN 'firefox'
        WHEN browser_kind = 'safari'
          THEN 'safari'
        WHEN browser_kind = 'takeout'
          THEN 'takeout'
        ELSE browser_kind
      END
    ),
    browser_product = COALESCE(NULLIF(browser_product, ''), browser_kind);

ALTER TABLE profile_watermarks ADD COLUMN last_source_batch_id INTEGER;
