//! Responsibilities: read exact-page preview results and bounded fetch candidates.
//! Not responsible for: fetching or changing cache state.
//! Failure modes: host fallback serves another page, duplicate request URLs,
//! negative cache retried early, prefetch includes HTTP or cached pages, tied candidates become unreachable.
//! Tests: og_images::tests.
use super::*;

pub(super) fn prefetch_sql(ties: bool, after: bool) -> String {
    let bounds = if ties {
        "last_visit_ms=?1 AND id<?2"
    } else if after {
        "last_visit_ms<?1"
    } else {
        "last_visit_ms<=?1"
    };
    format!(
        "SELECT url,last_visit_ms,id FROM urls INDEXED BY idx_urls_last_visit_ms WHERE url LIKE 'https://%' AND {bounds} AND NOT EXISTS(SELECT 1 FROM og_images WHERE page_url=urls.url) ORDER BY last_visit_ms DESC,id DESC LIMIT ?3"
    )
}
fn refetch_sql(ties: bool, after: bool) -> String {
    let bounds = if ties {
        "refetch_after=?1 AND id>?2"
    } else if after {
        "refetch_after>?1"
    } else {
        "refetch_after>=?1"
    };
    format!(
        "SELECT page_url,refetch_after,id FROM og_images WHERE refetch_after IS NOT NULL AND {bounds} AND refetch_after<=?3 ORDER BY refetch_after,id LIMIT ?4"
    )
}

/// Background producers can continue a bounded candidate page without losing tied rows.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OgImageUrlPage {
    pub items: Vec<String>,
    pub next_cursor: Option<String>,
}

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

/// Respects negative-cache deadlines and exposes every due row through a keyset.
pub fn list_urls_due_for_refetch(
    vault: &Vault,
    limit: usize,
    cursor: Option<String>,
) -> Result<OgImageUrlPage> {
    let after = cursor.is_some();
    let (time, id): (String, i64) =
        cursor.map(|s| serde_json::from_str(&s)).transpose()?.unwrap_or_default();
    vault.read(Db::Archive, move |s| {
        let limit = limit.min(1000);
        let mut rows = Vec::new();
        let now = now_rfc3339();
        let map = |r: &vault_store::Row| -> vault_store::Result<_> {
            Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?))
        };
        if limit > 0 {
            if after {
                rows = s.query(
                    &refetch_sql(true, true),
                    params![time.as_str(), id, now.as_str(), limit as i64 + 1],
                    map,
                )?;
            }
            if rows.len() <= limit {
                rows.extend(s.query(
                    &refetch_sql(false, after),
                    params![time.as_str(), id, now.as_str(), (limit + 1 - rows.len()) as i64],
                    map,
                )?);
            }
        }
        url_page(rows, limit)
    })
}
/// Reads uncached HTTPS observations in recency order; the cursor reaches the rest.
pub fn list_urls_for_prefetch(
    vault: &Vault,
    limit: usize,
    cursor: Option<String>,
) -> Result<OgImageUrlPage> {
    let after = cursor.is_some();
    let (time, id): (i64, i64) =
        cursor.map(|s| serde_json::from_str(&s)).transpose()?.unwrap_or((i64::MAX, i64::MAX));
    vault.read(Db::Archive, move |s| {
        let limit = limit.min(1000);
        let mut rows = Vec::new();
        let map = |r: &vault_store::Row| -> vault_store::Result<_> {
            Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?))
        };
        if limit > 0 {
            if after {
                rows =
                    s.query(&prefetch_sql(true, true), params![time, id, limit as i64 + 1], map)?;
            }
            if rows.len() <= limit {
                rows.extend(s.query(
                    &prefetch_sql(false, after),
                    params![time, id, (limit + 1 - rows.len()) as i64],
                    map,
                )?);
            }
        }
        url_page(rows, limit)
    })
}
fn url_page<T: serde::Serialize>(
    mut rows: Vec<(String, T, i64)>,
    limit: usize,
) -> Result<OgImageUrlPage> {
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next_cursor = if more {
        rows.last().map(|(_, time, id)| serde_json::to_string(&(time, id))).transpose()?
    } else {
        None
    };
    Ok(OgImageUrlPage { items: rows.into_iter().map(|(url, _, _)| url).collect(), next_cursor })
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
