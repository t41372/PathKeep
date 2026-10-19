//! Responsibilities: atomically persist preview outcomes, recency and eviction.
//! Not responsible for: HTTP fetching or browser history selection.
//! Failure modes: split blob/row writes, duplicate bytes, lost negative cache,
//! shared blobs counted twice in eviction, recency cleared by idempotent fetches.
//! Tests: og_images::tests and og_images_fetch cache regression tests.
use crate::{
    Vault,
    models::{
        HistoryOgImage, HistoryOgImageLookupEntry, HistoryOgImageLookupResult, OgImageCleanupMode,
        OgImageCleanupReport, OgImageCoverageStats, OgImageStorageStats,
    },
    utils::{image_data_to_data_url, now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use std::collections::HashSet;
use vault_store::{Db, Priority, Session, params};
mod reads;
pub use reads::*;
#[cfg(test)]
mod tests;
pub mod fetch_status {
    pub const OK: &str = "ok";
    pub const MISSING: &str = "missing";
    pub const HTTP_ERROR: &str = "http_error";
    pub const PARSE_ERROR: &str = "parse_error";
    pub const TOO_LARGE: &str = "too_large";
    pub const UNSUPPORTED_MIME: &str = "unsupported_mime";
    pub const BLOCKED: &str = "blocked";
}

/// Bytes + metadata for a fresh og:image fetch outcome. Used by the worker
/// to hand a resolved row to `upsert_og_image`.
pub struct OgImageInsert<'a> {
    pub page_url: &'a str,
    pub page_host: Option<&'a str>,
    pub source_og_url: Option<&'a str>,
    pub image_bytes: Option<&'a [u8]>,
    pub mime: Option<&'a str>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub fetch_status: &'a str,
    pub http_status: Option<i64>,
    pub refetch_after: Option<&'a str>,
    pub fetch_attempts: i64,
    pub created_by_run_id: Option<i64>,
}

const LOAD_OG_IMAGE_SQL: &str = r#"
SELECT og_image_blobs.image_data, og_images.fetch_status
FROM og_images
LEFT JOIN og_image_blobs ON og_image_blobs.blob_hash = og_images.image_blob_hash
WHERE og_images.page_url = ?1
LIMIT 1
"#;

const UPDATE_LAST_SHOWN_SQL: &str = r#"
UPDATE og_images
SET last_shown_at = ?2
WHERE page_url = ?1
"#;

const STATS_SQL: &str = r#"
SELECT
  (SELECT COUNT(*) FROM og_images),
  (SELECT COUNT(*) FROM og_image_blobs),
  (SELECT COALESCE(SUM(byte_size), 0) FROM og_image_blobs),
  (SELECT MIN(fetched_at) FROM og_images)
"#;

// Coverage denominator = distinct HTTPS pages.
//   - https-only: the fetcher short-circuits http:// to `parse_error` without ever
//     hitting the wire (see the prefetch query below + `og_images_fetch`), so http
//     pages can NEVER carry a preview and must not inflate the denominator.
//   - COUNT(DISTINCT url): `urls` has no unique constraint on `url` (its dedup key
//     is per-profile), so the same page under two profiles is two rows — but a
//     preview is keyed per distinct `page_url`. Distinct keeps numerator and
//     denominator in the same units.
//   - byte-range (not `LIKE`): default `LIKE` is case-insensitive and cannot seek,
//     so a prefix range lets the BINARY `idx_urls_url` / the `og_images` UNIQUE
//     index range-seek instead of full-scanning. `'https:/0'` is the prefix
//     `'https://'` with its last byte incremented (exclusive upper bound).
// On-demand for Settings only — never on a hot path.
const COVERAGE_STATS_SQL: &str = r#"
SELECT
  (SELECT COUNT(DISTINCT url) FROM urls
     WHERE url >= 'https://' AND url < 'https:/0'),
  (SELECT COUNT(*) FROM og_images
     WHERE page_url >= 'https://' AND page_url < 'https:/0'),
  (SELECT COUNT(*) FROM og_images WHERE fetch_status = 'ok')
"#;

fn write_image(connection: &Session<'_>, insert: &OgImageInsert<'_>) -> Result<()> {
    let now = now_rfc3339();
    let image_blob_hash = match (insert.image_bytes, insert.mime) {
        (Some(bytes), Some(mime)) if !bytes.is_empty() => {
            let hash = sha256_hex(bytes);
            connection
                .execute(
                    "INSERT OR IGNORE INTO og_image_blobs (
                       blob_hash,
                       image_data,
                       mime,
                       byte_size,
                       width,
                       height,
                       recorded_at
                     )
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        hash.as_str(),
                        bytes,
                        mime,
                        bytes.len() as i64,
                        insert.width,
                        insert.height,
                        now.as_str(),
                    ],
                )
                .context("inserting og_image blob")?;
            Some(hash)
        }
        _ => None,
    };

    struct ExistingOgImageRow {
        fetch_status: String,
        image_blob_hash: Option<String>,
        http_status: Option<i64>,
        page_host: Option<String>,
        source_og_url: Option<String>,
    }
    let existing: Option<ExistingOgImageRow> = connection
        .query(
            "SELECT fetch_status, image_blob_hash, http_status, page_host, source_og_url \
             FROM og_images WHERE page_url = ?1",
            params![insert.page_url],
            |row| {
                Ok(ExistingOgImageRow {
                    fetch_status: row.get(0)?,
                    image_blob_hash: row.get(1)?,
                    http_status: row.get(2)?,
                    page_host: row.get(3)?,
                    source_og_url: row.get(4)?,
                })
            },
        )
        .map(|mut rows| rows.pop())
        .context("loading existing og_image row for idempotency check")?;

    if let Some(existing_row) = existing.as_ref() {
        if existing_row.fetch_status == insert.fetch_status
            && existing_row.image_blob_hash.as_deref() == image_blob_hash.as_deref()
            && existing_row.http_status == insert.http_status
            && existing_row.page_host.as_deref() == insert.page_host
            && existing_row.source_og_url.as_deref() == insert.source_og_url
        {
            connection
                .execute(
                    "UPDATE og_images
                     SET fetched_at = ?2,
                         refetch_after = ?3,
                         fetch_attempts = ?4
                     WHERE page_url = ?1",
                    params![
                        insert.page_url,
                        now.as_str(),
                        insert.refetch_after,
                        insert.fetch_attempts,
                    ],
                )
                .context("updating og_image refetch window")?;
            return Ok(());
        }
    }

    // Delete first so the AUTOINCREMENT id can be reused safely. INSERT OR
    // REPLACE would zero out `fetch_attempts` and `last_shown_at` if we kept
    // those columns out of the row payload.
    connection
        .execute("DELETE FROM og_images WHERE page_url = ?1", params![insert.page_url])
        .context("clearing prior og_image row")?;

    connection
        .execute(
            "INSERT INTO og_images (
               page_url,
               page_host,
               source_og_url,
               image_blob_hash,
               fetch_status,
               http_status,
               fetched_at,
               last_shown_at,
               refetch_after,
               fetch_attempts,
               created_by_run_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10)",
            params![
                insert.page_url,
                insert.page_host,
                insert.source_og_url,
                image_blob_hash,
                insert.fetch_status,
                insert.http_status,
                now.as_str(),
                insert.refetch_after,
                insert.fetch_attempts,
                insert.created_by_run_id,
            ],
        )
        .context("inserting og_image row")?;

    Ok(())
}

/// Own the fetch result before handing it to the writer thread.
pub fn upsert_og_image(vault: &Vault, insert: &OgImageInsert<'_>) -> Result<()> {
    let page = insert.page_url.to_owned();
    let host = insert.page_host.map(str::to_owned);
    let source = insert.source_og_url.map(str::to_owned);
    let bytes = insert.image_bytes.map(<[u8]>::to_vec);
    let mime = insert.mime.map(str::to_owned);
    let status = insert.fetch_status.to_owned();
    let refetch = insert.refetch_after.map(str::to_owned);
    let (width, height, http, attempts, run) = (
        insert.width,
        insert.height,
        insert.http_status,
        insert.fetch_attempts,
        insert.created_by_run_id,
    );
    vault.write(Db::Archive, Priority::Interactive, move |s| {
        write_image(
            s,
            &OgImageInsert {
                page_url: &page,
                page_host: host.as_deref(),
                source_og_url: source.as_deref(),
                image_bytes: bytes.as_deref(),
                mime: mime.as_deref(),
                width,
                height,
                fetch_status: &status,
                http_status: http,
                refetch_after: refetch.as_deref(),
                fetch_attempts: attempts,
                created_by_run_id: run,
            },
        )
    })
}

/// Records display recency so LRU eviction follows actual use.
pub fn mark_og_images_shown(vault: &Vault, urls: &[String]) -> Result<()> {
    let urls = urls.to_vec();
    vault.write(Db::Archive, Priority::Interactive, move |s| {
        let now = now_rfc3339();
        for url in urls {
            s.execute(UPDATE_LAST_SHOWN_SQL, params![url, now.as_str()])?;
        }
        Ok(())
    })
}

/// Removes both cache rows and bytes atomically for the explicit clear action.
pub fn clear_cache(vault: &Vault) -> Result<OgImageCleanupReport> {
    vault.write(Db::Archive, Priority::Interactive, |s| {
        let before = reads::stats(s)?;
        s.execute("DELETE FROM og_images", ())?;
        s.execute("DELETE FROM og_image_blobs", ())?;
        Ok(OgImageCleanupReport {
            deleted_rows: before.row_count,
            deleted_blobs: before.blob_count,
            reclaimed_bytes: before.total_bytes,
        })
    })
}

/// Applies the selected retention policy without splitting shared-blob accounting.
pub fn run_cleanup(vault: &Vault, mode: OgImageCleanupMode) -> Result<OgImageCleanupReport> {
    vault.write(Db::Archive, Priority::Batch, move |s| {
        let before = reads::stats(s)?;
        let deleted_rows = match mode {
            OgImageCleanupMode::Off => 0,
            OgImageCleanupMode::TimeTtl { max_age_days } => s
                .execute("DELETE FROM og_images WHERE fetched_at<?1", [cutoff_iso(max_age_days)])?
                as i64,
            OgImageCleanupMode::SizeCap { max_bytes } => {
                evict_until_under_size(s, max_bytes, false)?
            }
            OgImageCleanupMode::Lru { max_bytes } => evict_until_under_size(s, max_bytes, true)?,
        };
        collect_orphan_blobs(s)?;
        let after = reads::stats(s)?;
        Ok(OgImageCleanupReport {
            deleted_rows,
            deleted_blobs: before.blob_count - after.blob_count,
            reclaimed_bytes: (before.total_bytes - after.total_bytes).max(0),
        })
    })
}

fn cutoff_iso(days: u32) -> String {
    (chrono::Utc::now() - chrono::Duration::days(i64::from(days)))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
fn evict_until_under_size(s: &Session<'_>, max_bytes: u64, lru: bool) -> Result<i64> {
    let order = if lru {
        "last_shown_at IS NULL DESC,last_shown_at,fetched_at,id"
    } else {
        "fetched_at,id"
    };
    let index = if lru { "idx_og_images_lru" } else { "idx_og_images_eviction" };
    let sql = format!(
        "SELECT id,image_blob_hash FROM og_images INDEXED BY {index} ORDER BY {order} LIMIT 1000"
    );
    let mut bytes = reads::stats(s)?.total_bytes;
    let mut removed = 0;
    while bytes > max_bytes as i64 {
        let rows = s.query(&sql, (), |r| Ok((r.get::<i64>(0)?, r.get::<Option<String>>(1)?)))?;
        if rows.is_empty() {
            break;
        }
        for (id, hash) in rows {
            if bytes <= max_bytes as i64 {
                break;
            }
            s.execute("DELETE FROM og_images WHERE id=?1", [id])?;
            removed += 1;
            if let Some(hash) = hash {
                let size=s.query_one("SELECT COALESCE((SELECT byte_size FROM og_image_blobs b WHERE blob_hash=?1 AND NOT EXISTS(SELECT 1 FROM og_images i WHERE i.image_blob_hash=b.blob_hash)),0)",[hash.as_str()],|r|Ok(r.get::<i64>(0)?))?;
                bytes -= size;
                s.execute("DELETE FROM og_image_blobs WHERE blob_hash=?1 AND NOT EXISTS(SELECT 1 FROM og_images WHERE image_blob_hash=?1)",[hash])?;
            }
        }
    }
    Ok(removed)
}
fn collect_orphan_blobs(s: &Session<'_>) -> Result<i64> {
    Ok(s.execute("DELETE FROM og_image_blobs WHERE NOT EXISTS(SELECT 1 FROM og_images WHERE image_blob_hash=og_image_blobs.blob_hash)",())? as i64)
}
