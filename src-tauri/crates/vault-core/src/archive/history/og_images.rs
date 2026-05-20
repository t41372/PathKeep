//! Per-page-URL og:image cache.
//!
//! ## Responsibilities
//! - Persist og:image fetch outcomes (success, failure, missing, blocked)
//!   keyed by exact page URL, with content-hash deduplication of the image
//!   bytes themselves.
//! - Resolve cached preview payloads for already-visible card-mode history
//!   rows.
//! - Track `last_shown_at` so user-configured LRU eviction has a signal.
//! - Run eviction passes in the four supported modes (Off / TimeTtl /
//!   SizeCap / Lru) and report what was reclaimed.
//!
//! ## Not responsible for
//! - Fetching the page or parsing its <meta> tags. The worker layer
//!   (vault-worker) drives the network IO and hands us the resolved bytes.
//! - Choosing which rows are visible. The Explorer route owns that and
//!   calls `load_og_images` with the URLs it wants hydrated.
//!
//! ## Dependencies
//! - The archive SQLite connection bootstrap from `archive::schema`.
//! - `models::archive::HistoryOgImage*` for the transport shape.
//! - `utils::sha256_hex` for content addressing and `utils::now_rfc3339`
//!   for audit timestamps.
//!
//! ## Performance notes
//! - The read path is a single exact-URL index lookup per entry; there is
//!   no host fallback by design. At 14.4 M visits the unique-URL cardinality
//!   is much smaller, and exact-URL lookup keeps every read O(log n).
//! - Cleanup passes are bounded by the eviction target bytes and never
//!   walk the table when `mode == Off`.

// C3 (worker fetch queue) and C4 (Tauri commands) wire these symbols up.
// C2 lands the schema + module skeleton with full test coverage; the
// `dead_code` allow keeps cargo build green for this commit only — C3
// removes it as soon as the worker imports begin to reference these
// functions for real.
#![allow(dead_code)]

use super::super::open_archive_connection;
use crate::{
    config::ProjectPaths,
    models::{
        AppConfig, HistoryOgImage, HistoryOgImageLookupEntry, HistoryOgImageLookupResult,
        OgImageCleanupMode, OgImageCleanupReport, OgImageStorageStats,
    },
    utils::{image_data_to_data_url, now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;

/// Fetch outcome string stored in `og_images.fetch_status`. Kept as a single
/// list so the worker, lookup, and stats paths agree on the spelling.
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

/// Inserts (or replaces by `page_url`) one og:image fetch outcome plus its
/// blob bytes. Identical bytes are deduplicated in `og_image_blobs`.
pub fn upsert_og_image(connection: &Connection, insert: &OgImageInsert<'_>) -> Result<()> {
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
                        &hash,
                        bytes,
                        mime,
                        bytes.len() as i64,
                        insert.width,
                        insert.height,
                        &now,
                    ],
                )
                .context("inserting og_image blob")?;
            Some(hash)
        }
        _ => None,
    };

    // Delete first so the AUTOINCREMENT id can be reused safely. INSERT OR
    // REPLACE would zero out `fetch_attempts` and `last_shown_at` if we kept
    // those columns out of the row payload.
    connection
        .execute(
            "DELETE FROM og_images WHERE page_url = ?1",
            params![insert.page_url],
        )
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
                &now,
                insert.refetch_after,
                insert.fetch_attempts,
                insert.created_by_run_id,
            ],
        )
        .context("inserting og_image row")?;

    Ok(())
}

/// Hydrates already-visible card-mode rows with their cached og:image bytes.
/// Returns one result per unique URL — duplicates in the request are
/// collapsed before SQL is touched.
pub fn load_og_images(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    entries: Vec<HistoryOgImageLookupEntry>,
) -> Result<Vec<HistoryOgImageLookupResult>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let connection = open_archive_connection(paths, config, key)?;
    let mut statement = connection.prepare(LOAD_OG_IMAGE_SQL)?;
    let mut seen = HashSet::new();
    let mut results = Vec::with_capacity(entries.len());

    for entry in entries {
        if !seen.insert(entry.url.clone()) {
            continue;
        }
        let row = statement
            .query_row(params![&entry.url], |row| {
                Ok((row.get::<_, Option<Vec<u8>>>(0)?, row.get::<_, String>(1)?))
            })
            .optional()?;
        let (bytes, fetch_status) = match row {
            Some((bytes, status)) => (bytes, status),
            None => (None, "pending".to_string()),
        };
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
}

/// Bumps `last_shown_at` for one or more URLs so LRU eviction has a fresh
/// signal. Missing URLs are silently skipped; the call is a no-op when
/// `urls` is empty.
pub fn mark_og_images_shown(connection: &Connection, urls: &[String]) -> Result<()> {
    if urls.is_empty() {
        return Ok(());
    }
    let now = now_rfc3339();
    let mut statement = connection.prepare(UPDATE_LAST_SHOWN_SQL)?;
    for url in urls {
        statement.execute(params![url, &now])?;
    }
    Ok(())
}

/// Reports the current cache footprint so Settings can display
/// "X rows · Y blobs · Z bytes" without scanning the row table twice.
pub fn storage_stats(connection: &Connection) -> Result<OgImageStorageStats> {
    let stats = connection
        .query_row(STATS_SQL, [], |row| {
            Ok(OgImageStorageStats {
                row_count: row.get(0)?,
                blob_count: row.get(1)?,
                total_bytes: row.get(2)?,
                oldest_fetched_at: row.get(3)?,
            })
        })
        .context("loading og:image storage stats")?;
    Ok(stats)
}

/// Empties both og:image tables. Used by Settings → "Clear all link
/// previews" and by the integration tests for tear-down between cases.
pub fn clear_cache(connection: &Connection) -> Result<OgImageCleanupReport> {
    let before = storage_stats(connection)?;
    connection.execute("DELETE FROM og_images", [])?;
    connection.execute("DELETE FROM og_image_blobs", [])?;
    Ok(OgImageCleanupReport {
        deleted_rows: before.row_count,
        deleted_blobs: before.blob_count,
        reclaimed_bytes: before.total_bytes,
    })
}

/// Runs one eviction pass for the chosen mode. Always GCs orphaned blobs at
/// the end, even when the chosen mode deleted nothing — that catches blobs
/// that may have been orphaned by a prior `upsert_og_image` race.
pub fn run_cleanup(
    connection: &Connection,
    mode: OgImageCleanupMode,
) -> Result<OgImageCleanupReport> {
    let before = storage_stats(connection)?;
    let mut deleted_rows = 0_i64;

    match mode {
        OgImageCleanupMode::Off => {
            // Still GC orphans below — but never evict rows.
        }
        OgImageCleanupMode::TimeTtl { max_age_days } => {
            let cutoff = cutoff_iso(max_age_days);
            deleted_rows = connection.execute(
                "DELETE FROM og_images WHERE fetched_at < ?1",
                params![cutoff],
            )? as i64;
        }
        OgImageCleanupMode::SizeCap { max_bytes } => {
            deleted_rows = evict_until_under_size(connection, max_bytes, false)?;
        }
        OgImageCleanupMode::Lru { max_bytes } => {
            deleted_rows = evict_until_under_size(connection, max_bytes, true)?;
        }
    }

    let deleted_blobs = collect_orphan_blobs(connection)?;
    let after = storage_stats(connection)?;
    Ok(OgImageCleanupReport {
        deleted_rows,
        deleted_blobs,
        reclaimed_bytes: (before.total_bytes - after.total_bytes).max(0),
    })
}

fn cutoff_iso(max_age_days: u32) -> String {
    let now = chrono::Utc::now();
    let cutoff = now - chrono::Duration::days(i64::from(max_age_days));
    cutoff.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn evict_until_under_size(
    connection: &Connection,
    max_bytes: u64,
    use_last_shown: bool,
) -> Result<i64> {
    // Pull every row's (page_url, blob_hash, byte_size) ordered by eviction
    // priority. Rows without a blob still contribute zero bytes but they get
    // evicted first under both Size and LRU modes since they take a row slot
    // for no benefit.
    //
    // Important: bytes can only be reclaimed when the LAST row referencing a
    // blob is deleted, because identical bytes are deduped in og_image_blobs.
    // The earlier implementation subtracted byte_size for *every* evicted
    // row, which double-counts on shared blobs and stops eviction early
    // (leaving the cache above the cap). We now track a refcount per blob
    // and only debit `total_bytes` when the refcount hits zero.
    let order_clause = if use_last_shown {
        // NULLS FIRST means rows never shown evict before rows shown long ago.
        "last_shown_at IS NULL DESC, last_shown_at ASC, fetched_at ASC"
    } else {
        "fetched_at ASC"
    };

    // refcount = number of og_images rows pointing at each blob_hash. We
    // build this up-front; the eviction walk decrements it.
    let mut refcount: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    let mut blob_bytes: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    {
        let mut refstmt = connection.prepare(
            "SELECT og_images.image_blob_hash, og_image_blobs.byte_size
             FROM og_images
             LEFT JOIN og_image_blobs
               ON og_image_blobs.blob_hash = og_images.image_blob_hash
             WHERE og_images.image_blob_hash IS NOT NULL",
        )?;
        let mut rows = refstmt.query([])?;
        while let Some(row) = rows.next()? {
            let hash: String = row.get(0)?;
            let bytes: i64 = row.get::<_, Option<i64>>(1)?.unwrap_or(0);
            *refcount.entry(hash.clone()).or_insert(0) += 1;
            blob_bytes.entry(hash).or_insert(bytes);
        }
    }

    let sql = format!(
        "SELECT og_images.page_url,
                og_images.image_blob_hash
         FROM og_images
         ORDER BY {order_clause}",
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query([])?;

    let mut total_bytes: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(byte_size), 0) FROM og_image_blobs",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let cap = max_bytes as i64;
    let mut evict_urls = Vec::new();
    while total_bytes > cap {
        match rows.next()? {
            Some(row) => {
                let url: String = row.get(0)?;
                let blob_hash: Option<String> = row.get(1)?;
                evict_urls.push(url);
                if let Some(hash) = blob_hash {
                    if let Some(remaining) = refcount.get_mut(&hash) {
                        *remaining -= 1;
                        if *remaining <= 0 {
                            // This was the last row referencing the blob —
                            // its bytes will actually be reclaimed by the
                            // orphan GC pass that runs after the evict.
                            total_bytes -= *blob_bytes.get(&hash).unwrap_or(&0);
                        }
                    }
                }
            }
            None => break,
        }
    }
    drop(rows);
    drop(statement);

    let mut deleted = 0_i64;
    for url in evict_urls {
        deleted += connection.execute(
            "DELETE FROM og_images WHERE page_url = ?1",
            params![url],
        )? as i64;
    }
    Ok(deleted)
}

fn collect_orphan_blobs(connection: &Connection) -> Result<i64> {
    let deleted = connection
        .execute(
            "DELETE FROM og_image_blobs
             WHERE blob_hash NOT IN (
               SELECT image_blob_hash
               FROM og_images
               WHERE image_blob_hash IS NOT NULL
             )",
            [],
        )
        .context("collecting orphan og:image blobs")?;
    Ok(deleted as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::schema::create_schema;

    fn open_test_archive() -> Connection {
        let connection = Connection::open_in_memory().expect("memory db");
        create_schema(&connection).expect("create schema");
        connection
    }

    fn ok_insert<'a>(page_url: &'a str, bytes: &'a [u8]) -> OgImageInsert<'a> {
        OgImageInsert {
            page_url,
            page_host: Some("example.com"),
            source_og_url: Some("https://example.com/og.png"),
            image_bytes: Some(bytes),
            mime: Some("image/png"),
            width: Some(1200),
            height: Some(630),
            fetch_status: fetch_status::OK,
            http_status: Some(200),
            refetch_after: None,
            fetch_attempts: 1,
            created_by_run_id: None,
        }
    }

    fn miss_insert(page_url: &str) -> OgImageInsert<'_> {
        OgImageInsert {
            page_url,
            page_host: Some("example.com"),
            source_og_url: None,
            image_bytes: None,
            mime: None,
            width: None,
            height: None,
            fetch_status: fetch_status::MISSING,
            http_status: Some(200),
            refetch_after: Some("2026-06-19T00:00:00Z"),
            fetch_attempts: 1,
            created_by_run_id: None,
        }
    }

    #[test]
    fn upsert_inserts_one_blob_for_identical_bytes() {
        let connection = open_test_archive();
        let bytes = b"\x89PNG\r\n\x1a\n\x00\x00fake-image";

        upsert_og_image(&connection, &ok_insert("https://example.com/a", bytes)).unwrap();
        upsert_og_image(&connection, &ok_insert("https://example.com/b", bytes)).unwrap();

        let stats = storage_stats(&connection).unwrap();
        assert_eq!(stats.row_count, 2);
        assert_eq!(stats.blob_count, 1);
        assert_eq!(stats.total_bytes, bytes.len() as i64);
    }

    #[test]
    fn lookup_by_page_url_returns_exact_match_only() {
        let connection = open_test_archive();
        let a_bytes = b"\x89PNGfake-a-image";
        let b_bytes = b"\x89PNGfake-b-image";
        upsert_og_image(&connection, &ok_insert("https://example.com/a", a_bytes)).unwrap();
        upsert_og_image(&connection, &ok_insert("https://example.com/b", b_bytes)).unwrap();

        // load_og_images opens a fresh connection from disk; in-memory dbs
        // can't be reopened, so the SQL behaviour is exercised directly here
        // and end-to-end through the worker/command integration tests.
        let mut statement = connection.prepare(LOAD_OG_IMAGE_SQL).unwrap();
        let (bytes, _status): (Option<Vec<u8>>, String) = statement
            .query_row(params!["https://example.com/a"], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(bytes.as_deref(), Some(&a_bytes[..]));
    }

    #[test]
    fn lookup_returns_none_for_unknown_page_with_known_host() {
        let connection = open_test_archive();
        let bytes = b"\x89PNGknown-page";
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/known", bytes),
        )
        .unwrap();

        let mut statement = connection.prepare(LOAD_OG_IMAGE_SQL).unwrap();
        let result: Option<(Option<Vec<u8>>, String)> = statement
            .query_row(params!["https://example.com/unknown"], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()
            .unwrap();
        // Negative assertion guarding the "no host fallback" rule: a sibling
        // page on the same host must NOT return the known page's bytes.
        assert!(result.is_none());
    }

    #[test]
    fn upsert_replaces_existing_row_for_same_url() {
        let connection = open_test_archive();
        let bytes_v1 = b"\x89PNGversion-one";
        let bytes_v2 = b"\x89PNGversion-two";
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/page", bytes_v1),
        )
        .unwrap();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/page", bytes_v2),
        )
        .unwrap();

        let stats = storage_stats(&connection).unwrap();
        assert_eq!(stats.row_count, 1);
        // Both blobs persist until orphan GC runs, the row points at v2.
        let active: String = connection
            .query_row(
                "SELECT image_blob_hash FROM og_images WHERE page_url = ?1",
                params!["https://example.com/page"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active, sha256_hex(bytes_v2));

        let report = run_cleanup(&connection, OgImageCleanupMode::Off).unwrap();
        assert_eq!(report.deleted_blobs, 1);
        let stats_after = storage_stats(&connection).unwrap();
        assert_eq!(stats_after.blob_count, 1);
    }

    #[test]
    fn cleanup_off_evicts_no_rows_but_still_collects_orphans() {
        let connection = open_test_archive();
        let bytes = b"\x89PNGorphan-bytes";
        upsert_og_image(&connection, &ok_insert("https://example.com/x", bytes)).unwrap();
        // Manually orphan the blob by clearing the FK.
        connection
            .execute(
                "UPDATE og_images SET image_blob_hash = NULL WHERE page_url = ?1",
                params!["https://example.com/x"],
            )
            .unwrap();

        let report = run_cleanup(&connection, OgImageCleanupMode::Off).unwrap();
        assert_eq!(report.deleted_rows, 0);
        assert_eq!(report.deleted_blobs, 1);
    }

    #[test]
    fn cleanup_time_ttl_deletes_only_expired_rows() {
        let connection = open_test_archive();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/fresh", b"\x89PNGfresh"),
        )
        .unwrap();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/stale", b"\x89PNGstale"),
        )
        .unwrap();
        connection
            .execute(
                "UPDATE og_images
                 SET fetched_at = '2000-01-01T00:00:00Z'
                 WHERE page_url = 'https://example.com/stale'",
                [],
            )
            .unwrap();

        let report =
            run_cleanup(&connection, OgImageCleanupMode::TimeTtl { max_age_days: 30 }).unwrap();
        assert_eq!(report.deleted_rows, 1);
        assert_eq!(report.deleted_blobs, 1);
        let surviving: Vec<String> = connection
            .prepare("SELECT page_url FROM og_images")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(surviving, vec!["https://example.com/fresh".to_string()]);
    }

    #[test]
    fn cleanup_size_cap_evicts_oldest_first_until_under_cap() {
        let connection = open_test_archive();
        // 100-byte payloads, three rows = 300 bytes. Cap at 150 → evict two.
        let bytes = vec![0_u8; 100];
        for (idx, url) in ["a", "b", "c"].iter().enumerate() {
            upsert_og_image(
                &connection,
                &ok_insert(&format!("https://example.com/{url}"), &bytes),
            )
            .unwrap();
            connection
                .execute(
                    "UPDATE og_images
                     SET fetched_at = ?1
                     WHERE page_url = ?2",
                    params![
                        format!("2026-05-{:02}T00:00:00Z", 10 + idx),
                        format!("https://example.com/{url}")
                    ],
                )
                .unwrap();
        }
        // Force each row to point at a distinct blob so byte accounting is
        // realistic for the eviction check.
        for (idx, url) in ["a", "b", "c"].iter().enumerate() {
            let mut bytes = b"\x89PNGunique-".to_vec();
            bytes.extend_from_slice(idx.to_string().as_bytes());
            upsert_og_image(
                &connection,
                &ok_insert(&format!("https://example.com/{url}"), &bytes),
            )
            .unwrap();
            connection
                .execute(
                    "UPDATE og_images
                     SET fetched_at = ?1
                     WHERE page_url = ?2",
                    params![
                        format!("2026-05-{:02}T00:00:00Z", 10 + idx),
                        format!("https://example.com/{url}")
                    ],
                )
                .unwrap();
        }

        let before = storage_stats(&connection).unwrap();
        let mode = OgImageCleanupMode::SizeCap {
            max_bytes: (before.total_bytes / 2) as u64,
        };
        let report = run_cleanup(&connection, mode).unwrap();
        let after = storage_stats(&connection).unwrap();
        assert!(report.deleted_rows >= 1, "expected at least one eviction");
        assert!(after.total_bytes <= (before.total_bytes / 2));
        // Oldest row ('/a') must be gone before younger rows under SizeCap.
        let surviving_a: Option<String> = connection
            .query_row(
                "SELECT page_url FROM og_images WHERE page_url = 'https://example.com/a'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert!(surviving_a.is_none());
    }

    #[test]
    fn cleanup_size_cap_accounts_for_shared_blobs_correctly() {
        // Regression for the byte-accounting bug where evicting a row that
        // shared its blob with other rows would still subtract the full
        // blob byte_size from `total_bytes`, stopping eviction early and
        // leaving the cache above the cap.
        let connection = open_test_archive();

        // Build a 600 KiB blob shared by /shared-a, /shared-b, /shared-c
        // (1.8 MiB worth of duplicate accounting if we naively subtracted
        // per row), and a separate 200 KiB blob held by /unique. Total
        // actual storage = 800 KiB. Cap at 700 KiB → ONLY the unique blob
        // can be reclaimed; the shared blob can't be evicted without
        // dropping all three rows.
        let big_bytes = vec![0xAB_u8; 600 * 1024];
        let small_bytes = {
            // distinct content so this is a distinct blob.
            let mut v = vec![0_u8; 200 * 1024];
            for (idx, byte) in v.iter_mut().enumerate() {
                *byte = (idx % 200) as u8;
            }
            v
        };
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/shared-a", &big_bytes),
        )
        .unwrap();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/shared-b", &big_bytes),
        )
        .unwrap();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/shared-c", &big_bytes),
        )
        .unwrap();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/unique", &small_bytes),
        )
        .unwrap();
        // Make /unique oldest so it would be evicted first by fetched_at.
        connection
            .execute(
                "UPDATE og_images SET fetched_at = '2026-01-01T00:00:00Z'
                 WHERE page_url = 'https://example.com/unique'",
                [],
            )
            .unwrap();

        let stats_before = storage_stats(&connection).unwrap();
        assert_eq!(stats_before.blob_count, 2);
        assert_eq!(
            stats_before.total_bytes,
            (big_bytes.len() + small_bytes.len()) as i64,
        );

        let report = run_cleanup(
            &connection,
            OgImageCleanupMode::SizeCap {
                max_bytes: 700 * 1024,
            },
        )
        .unwrap();
        // The unique blob accounts for the only bytes that can actually be
        // reclaimed without dropping all three shared rows. The eviction
        // should pick exactly the unique row (oldest fetched_at) and stop.
        assert_eq!(report.deleted_rows, 1);
        assert_eq!(report.deleted_blobs, 1);
        assert_eq!(report.reclaimed_bytes, small_bytes.len() as i64);

        // Shared rows survive.
        let surviving: Vec<String> = connection
            .prepare("SELECT page_url FROM og_images ORDER BY page_url")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(
            surviving,
            vec![
                "https://example.com/shared-a".to_string(),
                "https://example.com/shared-b".to_string(),
                "https://example.com/shared-c".to_string(),
            ],
        );
    }

    #[test]
    fn cleanup_lru_evicts_least_recently_shown_first() {
        let connection = open_test_archive();
        for url in ["a", "b", "c"] {
            let mut bytes = b"\x89PNGlru-".to_vec();
            bytes.extend_from_slice(url.as_bytes());
            upsert_og_image(
                &connection,
                &ok_insert(&format!("https://example.com/{url}"), &bytes),
            )
            .unwrap();
        }
        // /b shown most recently, /a shown long ago, /c never shown.
        connection
            .execute(
                "UPDATE og_images SET last_shown_at = '2026-05-18T00:00:00Z'
                 WHERE page_url = 'https://example.com/b'",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE og_images SET last_shown_at = '2026-01-01T00:00:00Z'
                 WHERE page_url = 'https://example.com/a'",
                [],
            )
            .unwrap();

        let before = storage_stats(&connection).unwrap();
        let mode = OgImageCleanupMode::Lru {
            max_bytes: (before.total_bytes / 3) as u64,
        };
        run_cleanup(&connection, mode).unwrap();

        // /c (never shown) and /a (oldest shown) should be gone before /b.
        let surviving: Vec<String> = connection
            .prepare("SELECT page_url FROM og_images ORDER BY page_url")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert!(surviving.contains(&"https://example.com/b".to_string()));
        assert!(!surviving.contains(&"https://example.com/c".to_string()));
    }

    #[test]
    fn negative_cache_row_is_persisted_with_no_blob() {
        let connection = open_test_archive();
        upsert_og_image(&connection, &miss_insert("https://example.com/missing")).unwrap();
        let stats = storage_stats(&connection).unwrap();
        assert_eq!(stats.row_count, 1);
        assert_eq!(stats.blob_count, 0);

        let status: String = connection
            .query_row(
                "SELECT fetch_status FROM og_images WHERE page_url = ?1",
                params!["https://example.com/missing"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, fetch_status::MISSING);
    }

    #[test]
    fn mark_shown_bumps_last_shown_at_for_provided_urls() {
        let connection = open_test_archive();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/page", b"\x89PNGpage"),
        )
        .unwrap();
        assert!(
            connection
                .query_row::<Option<String>, _, _>(
                    "SELECT last_shown_at FROM og_images WHERE page_url = ?1",
                    params!["https://example.com/page"],
                    |row| row.get(0),
                )
                .unwrap()
                .is_none()
        );

        mark_og_images_shown(&connection, &["https://example.com/page".into()]).unwrap();
        let stamped: Option<String> = connection
            .query_row(
                "SELECT last_shown_at FROM og_images WHERE page_url = ?1",
                params!["https://example.com/page"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stamped.is_some());
    }

    #[test]
    fn mark_shown_is_a_noop_for_empty_input() {
        let connection = open_test_archive();
        mark_og_images_shown(&connection, &[]).expect("noop on empty input");
    }

    #[test]
    fn clear_cache_empties_both_tables() {
        let connection = open_test_archive();
        upsert_og_image(
            &connection,
            &ok_insert("https://example.com/a", b"\x89PNGa"),
        )
        .unwrap();
        upsert_og_image(&connection, &miss_insert("https://example.com/b")).unwrap();

        let report = clear_cache(&connection).unwrap();
        assert_eq!(report.deleted_rows, 2);
        assert_eq!(report.deleted_blobs, 1);
        let stats = storage_stats(&connection).unwrap();
        assert_eq!(stats.row_count, 0);
        assert_eq!(stats.blob_count, 0);
    }

    #[test]
    fn load_og_images_collapses_duplicate_urls_via_tempdir_archive() {
        use crate::config::project_paths_with_root;

        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig::default();
        let bootstrap = open_archive_connection(&paths, &config, None).expect("open archive");
        let png_bytes: [u8; 9] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x01];
        upsert_og_image(
            &bootstrap,
            &ok_insert("https://example.com/p", &png_bytes),
        )
        .unwrap();
        drop(bootstrap);

        let results = load_og_images(
            &paths,
            &config,
            None,
            vec![
                HistoryOgImageLookupEntry {
                    url: "https://example.com/p".into(),
                },
                HistoryOgImageLookupEntry {
                    url: "https://example.com/p".into(),
                },
                HistoryOgImageLookupEntry {
                    url: "https://example.com/q".into(),
                },
            ],
        )
        .expect("load og images");
        assert_eq!(results.len(), 2);
        let p_hit = results
            .iter()
            .find(|r| r.url == "https://example.com/p")
            .expect("p result");
        assert!(p_hit.og_image.is_some());
        let q_miss = results
            .iter()
            .find(|r| r.url == "https://example.com/q")
            .expect("q result");
        assert!(q_miss.og_image.is_none());
        assert_eq!(q_miss.fetch_status, "pending");
    }
}
