//! Responsibilities: read exact-page preview results and bounded fetch candidates.
//! Not responsible for: fetching or changing cache state.
//! Failure modes: host fallback serves another page, duplicate request URLs,
//! negative cache retried early, prefetch includes HTTP or cached pages.
//! Tests: og_images::tests.
use super::*;

pub(super) const PREFETCH_SQL: &str = "SELECT url FROM urls INDEXED BY idx_urls_last_visit_ms WHERE url LIKE 'https://%' AND NOT EXISTS(SELECT 1 FROM og_images WHERE page_url=urls.url) ORDER BY last_visit_ms DESC LIMIT ?1";

/// Hydrates only requested exact URLs so pages never share an unrelated preview.
pub fn load_og_images(
    vault: &Vault,
    entries: Vec<HistoryOgImageLookupEntry>,
) -> Result<Vec<HistoryOgImageLookupResult>> {
    vault.read(Db::Archive, move |s| {
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for entry in entries {
            if !seen.insert(entry.url.clone()) {
                continue;
            }
            let row = s
                .query(LOAD_OG_IMAGE_SQL, [entry.url.as_str()], |r| {
                    Ok((r.get::<Option<Vec<u8>>>(0)?, r.get::<String>(1)?))
                })?
                .pop();
            let (bytes, fetch_status) = row.unwrap_or((None, "pending".into()));
            results.push(HistoryOgImageLookupResult {
                url: entry.url,
                og_image: bytes
                    .as_deref()
                    .and_then(image_data_to_data_url)
                    .map(|data_url| HistoryOgImage { data_url }),
                fetch_status,
            });
        }
        Ok(results)
    })
}

/// Respects negative-cache deadlines before scheduling another fetch.
pub fn list_urls_due_for_refetch(vault: &Vault, limit: usize) -> Result<Vec<String>> {
    vault.read(Db::Archive,move |s|Ok(s.query("SELECT page_url FROM og_images WHERE refetch_after IS NOT NULL AND refetch_after<=?1 ORDER BY refetch_after ASC LIMIT ?2",
        params![now_rfc3339(),limit.min(i64::MAX as usize) as i64],|r|Ok(r.get(0)?))?))
}
/// Bounds background work to uncached HTTPS pages.
pub fn list_urls_for_prefetch(vault: &Vault, limit: usize) -> Result<Vec<String>> {
    vault.read(Db::Archive, move |s| {
        Ok(s.query(PREFETCH_SQL, [limit.min(i64::MAX as usize) as i64], |r| Ok(r.get(0)?))?)
    })
}
/// Provides the cache footprint for storage controls.
pub fn storage_stats(vault: &Vault) -> Result<OgImageStorageStats> {
    vault.read(Db::Archive, stats)
}
pub(super) fn stats(s: &Session<'_>) -> Result<OgImageStorageStats> {
    Ok(s.query_one(STATS_SQL, (), |r| {
        Ok(OgImageStorageStats {
            row_count: r.get(0)?,
            blob_count: r.get(1)?,
            total_bytes: r.get(2)?,
            oldest_fetched_at: r.get(3)?,
        })
    })?)
}
/// Keeps preview coverage comparable across duplicate profile observations.
pub fn coverage_stats(vault: &Vault) -> Result<OgImageCoverageStats> {
    vault.read(Db::Archive, |s| {
        Ok(s.query_one(COVERAGE_STATS_SQL, (), |r| {
            Ok(OgImageCoverageStats {
                eligible_pages: r.get(0)?,
                attempted_pages: r.get(1)?,
                pages_with_image: r.get(2)?,
            })
        })?)
    })
}
