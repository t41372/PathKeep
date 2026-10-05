//! Responsibilities: preserve exact-page image caching, dedup and eviction.
//! Not responsible for: network fetching.
//! Failure modes: bytes duplicate, negative cache forgotten, LRU recency lost,
//! shared blobs evict too early, row/blob writes partially commit.
use super::*;
use crate::test_support::store::Probe;

fn open_test_archive() -> Vault {
    Vault::open_temp().unwrap()
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

fn refetch_due_insert<'a>(page_url: &'a str, refetch_after: &'a str) -> OgImageInsert<'a> {
    OgImageInsert {
        page_url,
        page_host: Some("example.com"),
        source_og_url: None,
        image_bytes: None,
        mime: None,
        width: None,
        height: None,
        fetch_status: fetch_status::MISSING,
        http_status: Some(503),
        refetch_after: Some(refetch_after),
        fetch_attempts: 1,
        created_by_run_id: None,
    }
}

#[test]
fn list_urls_due_for_refetch_returns_rows_whose_window_has_elapsed() {
    let connection = open_test_archive();
    upsert_og_image(
        &connection,
        &refetch_due_insert("https://example.com/past", "2020-01-01T00:00:00Z"),
    )
    .unwrap();
    upsert_og_image(
        &connection,
        &refetch_due_insert("https://example.com/future", "2099-12-31T00:00:00Z"),
    )
    .unwrap();

    let due = list_urls_due_for_refetch(&connection, 10).unwrap();
    assert_eq!(due, vec!["https://example.com/past".to_string()]);
}

#[test]
fn list_urls_due_for_refetch_skips_rows_with_null_refetch_after() {
    let connection = open_test_archive();
    upsert_og_image(&connection, &ok_insert("https://example.com/cached", b"\x89PNGsmall"))
        .unwrap();
    let due = list_urls_due_for_refetch(&connection, 10).unwrap();
    assert!(due.is_empty());
}

#[test]
fn list_urls_due_for_refetch_honors_the_limit_and_orders_by_oldest_first() {
    let connection = open_test_archive();
    upsert_og_image(
        &connection,
        &refetch_due_insert("https://example.com/older", "2020-01-01T00:00:00Z"),
    )
    .unwrap();
    upsert_og_image(
        &connection,
        &refetch_due_insert("https://example.com/newer", "2024-01-01T00:00:00Z"),
    )
    .unwrap();
    let limited = list_urls_due_for_refetch(&connection, 1).unwrap();
    assert_eq!(limited, vec!["https://example.com/older".to_string()]);
}

#[test]
fn list_urls_due_for_refetch_returns_empty_when_limit_is_zero() {
    let connection = open_test_archive();
    upsert_og_image(
        &connection,
        &refetch_due_insert("https://example.com/x", "2020-01-01T00:00:00Z"),
    )
    .unwrap();
    let none = list_urls_due_for_refetch(&connection, 0).unwrap();
    assert!(none.is_empty());
}

#[test]
fn coverage_stats_counts_eligible_attempted_and_with_image() {
    let connection = open_test_archive();
    seed_url(&connection, 1, "https://example.com/a", 1_000);
    seed_url(&connection, 2, "https://example.com/b", 2_000);
    // The SAME https page seen under a second profile is a second `urls` row
    // but must count ONCE (the denominator is distinct pages).
    seed_url(&connection, 4, "https://example.com/a", 4_000);
    // http:// is never fetched (short-circuits to parse_error) → excluded.
    seed_url(&connection, 5, "http://legacy.example.com/x", 5_000);
    // A non-web scheme can never carry an og:image → excluded.
    seed_url(&connection, 3, "chrome://settings", 3_000);
    upsert_og_image(&connection, &ok_insert("https://example.com/a", b"\x89PNGdata")).unwrap();
    upsert_og_image(&connection, &miss_insert("https://example.com/b")).unwrap();

    let coverage = coverage_stats(&connection).expect("coverage stats");
    assert_eq!(
        coverage.eligible_pages, 2,
        "only DISTINCT https pages are eligible (dup counted once; http + chrome excluded)"
    );
    assert_eq!(coverage.attempted_pages, 2);
    assert_eq!(coverage.pages_with_image, 1);

    // Empty archive reports zeros, never a divide-by-zero upstream.
    let empty = open_test_archive();
    let none = coverage_stats(&empty).expect("empty coverage");
    assert_eq!(none.eligible_pages, 0);
    assert_eq!(none.attempted_pages, 0);
    assert_eq!(none.pages_with_image, 0);
}

/// Seeds a single `urls` row with `last_visit_ms` and a synthetic
/// `first_visit_iso` derived from the same value. Used by the
/// prefetch-sweep tests below — they only care about (url,
/// last_visit_ms) so the rest of the columns are filled with
/// minimum-valid defaults.
///
/// Also lazily seeds a `source_profiles` row id=1 and a `runs` row
/// id=1 the first time it's called — both columns are FKs and the
/// connection has `PRAGMA foreign_keys = ON`.
fn seed_url(connection: &Vault, id: i64, url: &str, last_visit_ms: i64) {
    ensure_url_seed_parents(connection);
    Probe::new(connection)
        .execute(
            "INSERT OR IGNORE INTO pages(id,url,canonical_url) VALUES(?1,?2,?2)",
            params![id, url],
        )
        .unwrap();
    let page: i64 = Probe::new(connection)
        .query_row("SELECT id FROM pages WHERE url=?1", [url], |r| Ok(r.get(0)?))
        .unwrap();
    let iso = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(last_visit_ms)
        .unwrap_or_else(chrono::Utc::now)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    Probe::new(connection)
        .execute(
            "INSERT INTO urls (
                   id, url, title, visit_count, typed_count,
                   first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                   source_profile_id, created_by_run_id,page_id
                 ) VALUES (?1, ?2, NULL, 1, 0, ?3, ?4, ?3, ?4, 1, 1,?5)",
            params![id, url, last_visit_ms, iso, page],
        )
        .expect("seed url");
}

fn ensure_url_seed_parents(connection: &Vault) {
    let existing: i64 = Probe::new(connection)
        .query_row("SELECT COUNT(*) FROM source_profiles WHERE id = 1", (), |row| Ok(row.get(0)?))
        .unwrap_or(0);
    if existing > 0 {
        return;
    }
    Probe::new(connection)
        .execute(
            "INSERT INTO source_profiles (
                   id, browser_kind, browser_version, profile_name, profile_path,
                   discovered_at, enabled, profile_key, user_name, updated_at,
                   browser_family, browser_product
                 ) VALUES (1, 'chrome', NULL, 'Default', '/tmp/profile',
                           '2026-01-01T00:00:00Z', 1, 'chrome:Default', NULL,
                           '2026-01-01T00:00:00Z', 'chromium', 'Chrome')",
            (),
        )
        .expect("seed source profile");
    Probe::new(connection)
        .execute(
            "INSERT INTO runs (
                   id, run_type, trigger, started_at, timezone, status,
                   profile_scope_json, warnings_json, stats_json, due_only
                 ) VALUES (1, 'backup', 'manual', '2026-01-01T00:00:00Z', 'UTC',
                           'success', '[]', '[]', '{}', 0)",
            (),
        )
        .expect("seed run");
}

#[test]
fn list_urls_for_prefetch_returns_empty_when_archive_has_no_urls() {
    let connection = open_test_archive();
    let urls = list_urls_for_prefetch(&connection, 10).unwrap();
    assert!(urls.is_empty());
}

#[test]
fn list_urls_for_prefetch_returns_empty_when_every_url_is_already_cached() {
    let connection = open_test_archive();
    seed_url(&connection, 1, "https://example.com/a", 1_000);
    upsert_og_image(&connection, &ok_insert("https://example.com/a", b"x")).unwrap();

    let urls = list_urls_for_prefetch(&connection, 10).unwrap();
    assert!(urls.is_empty());
}

#[test]
fn list_urls_for_prefetch_treats_negative_cache_rows_as_cached_too() {
    // Negative-cache rows (missing / parse_error etc.) still count
    // as "the worker has seen this URL" — the daily refetch sweep
    // owns retries for those, not the prefetch sweep.
    let connection = open_test_archive();
    seed_url(&connection, 1, "https://example.com/a", 1_000);
    upsert_og_image(&connection, &miss_insert("https://example.com/a")).unwrap();

    let urls = list_urls_for_prefetch(&connection, 10).unwrap();
    assert!(
        urls.is_empty(),
        "URLs with any og_images row (even MISSING) must be excluded from prefetch",
    );
}

#[test]
fn list_urls_for_prefetch_skips_http_urls() {
    // The prefetch query explicitly filters `url LIKE 'https://%'`
    // because `fetch_og_image_for` would short-circuit any http://
    // URL to `parse_error`, wasting fetch budget.
    let connection = open_test_archive();
    seed_url(&connection, 1, "http://insecure.example.com/page", 1_000);
    seed_url(&connection, 2, "https://example.com/safe", 2_000);

    let urls = list_urls_for_prefetch(&connection, 10).unwrap();
    assert_eq!(urls, vec!["https://example.com/safe".to_string()]);
}

#[test]
fn list_urls_for_prefetch_orders_by_last_visit_descending() {
    // Newest visit first: the user is most likely to scroll there
    // next, so the worker should warm those covers first.
    let connection = open_test_archive();
    seed_url(&connection, 1, "https://example.com/old", 1_000);
    seed_url(&connection, 2, "https://example.com/new", 9_000);
    seed_url(&connection, 3, "https://example.com/mid", 5_000);

    let urls = list_urls_for_prefetch(&connection, 10).unwrap();
    assert_eq!(
        urls,
        vec![
            "https://example.com/new".to_string(),
            "https://example.com/mid".to_string(),
            "https://example.com/old".to_string(),
        ],
    );
}

#[test]
fn list_urls_for_prefetch_honors_the_limit() {
    let connection = open_test_archive();
    for id in 1..=5 {
        seed_url(&connection, id, &format!("https://example.com/page/{id}"), id * 1000);
    }

    let two = list_urls_for_prefetch(&connection, 2).unwrap();
    assert_eq!(two.len(), 2);
    // Newest two: ids 5 and 4 (last_visit_ms 5000 and 4000).
    assert_eq!(two[0], "https://example.com/page/5");
    assert_eq!(two[1], "https://example.com/page/4");
}

#[test]
fn list_urls_for_prefetch_returns_empty_when_limit_is_zero() {
    // 0 must short-circuit at the rust level — passing 0 to the
    // SQLite `LIMIT` would actually return 0 rows too, but the
    // early-return keeps the prepared-statement cost off the
    // hot path on `Off`-mode tick.
    let connection = open_test_archive();
    seed_url(&connection, 1, "https://example.com/a", 1_000);
    let urls = list_urls_for_prefetch(&connection, 0).unwrap();
    assert!(urls.is_empty());
}

#[test]
fn list_urls_for_prefetch_handles_mixed_cached_and_uncached_set() {
    // Realistic mix: some URLs cached, some not. Only the uncached
    // ones surface, in newest-first order.
    let connection = open_test_archive();
    seed_url(&connection, 1, "https://example.com/cached-old", 1_000);
    seed_url(&connection, 2, "https://example.com/uncached-new", 5_000);
    seed_url(&connection, 3, "https://example.com/uncached-mid", 3_000);
    seed_url(&connection, 4, "https://example.com/cached-mid", 2_000);
    upsert_og_image(&connection, &ok_insert("https://example.com/cached-old", b"x")).unwrap();
    upsert_og_image(&connection, &ok_insert("https://example.com/cached-mid", b"y")).unwrap();

    let urls = list_urls_for_prefetch(&connection, 10).unwrap();
    assert_eq!(
        urls,
        vec![
            "https://example.com/uncached-new".to_string(),
            "https://example.com/uncached-mid".to_string(),
        ],
    );
}

#[test]
fn list_urls_for_prefetch_tolerates_extremely_large_limits() {
    // Make sure the `i64::try_from(limit)` saturating-cast path
    // does not panic on a huge usize.
    let connection = open_test_archive();
    seed_url(&connection, 1, "https://example.com/a", 1_000);
    let urls = list_urls_for_prefetch(&connection, usize::MAX).unwrap();
    assert_eq!(urls, vec!["https://example.com/a".to_string()]);
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
    let (bytes, _status): (Option<Vec<u8>>, String) = Probe::new(&connection)
        .query_row(LOAD_OG_IMAGE_SQL, params!["https://example.com/a"], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(bytes.as_deref(), Some(&a_bytes[..]));
}

#[test]
fn lookup_returns_none_for_unknown_page_with_known_host() {
    let connection = open_test_archive();
    let bytes = b"\x89PNGknown-page";
    upsert_og_image(&connection, &ok_insert("https://example.com/known", bytes)).unwrap();

    let result: Option<(Option<Vec<u8>>, String)> = Probe::new(&connection)
        .query(LOAD_OG_IMAGE_SQL, params!["https://example.com/unknown"], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap()
        .pop();
    // Negative assertion guarding the "no host fallback" rule: a sibling
    // page on the same host must NOT return the known page's bytes.
    assert!(result.is_none());
}

#[test]
fn upsert_replaces_existing_row_for_same_url() {
    let connection = open_test_archive();
    let bytes_v1 = b"\x89PNGversion-one";
    let bytes_v2 = b"\x89PNGversion-two";
    upsert_og_image(&connection, &ok_insert("https://example.com/page", bytes_v1)).unwrap();
    upsert_og_image(&connection, &ok_insert("https://example.com/page", bytes_v2)).unwrap();

    let stats = storage_stats(&connection).unwrap();
    assert_eq!(stats.row_count, 1);
    // Both blobs persist until orphan GC runs, the row points at v2.
    let active: String = Probe::new(&connection)
        .query_row(
            "SELECT image_blob_hash FROM og_images WHERE page_url = ?1",
            params!["https://example.com/page"],
            |row| Ok(row.get(0)?),
        )
        .unwrap();
    assert_eq!(active, sha256_hex(bytes_v2));

    let report = run_cleanup(&connection, OgImageCleanupMode::Off).unwrap();
    assert_eq!(report.deleted_blobs, 1);
    let stats_after = storage_stats(&connection).unwrap();
    assert_eq!(stats_after.blob_count, 1);
}

#[test]
fn upsert_is_idempotent_when_identity_tuple_is_unchanged() {
    let connection = open_test_archive();
    let url = "https://www.google.com/search?q=吉野家";
    let first = OgImageInsert {
        page_url: url,
        page_host: Some("www.google.com"),
        source_og_url: None,
        image_bytes: None,
        mime: None,
        width: None,
        height: None,
        fetch_status: fetch_status::MISSING,
        http_status: None,
        refetch_after: Some("2026-06-19T00:00:00Z"),
        fetch_attempts: 1,
        created_by_run_id: None,
    };
    upsert_og_image(&connection, &first).unwrap();
    let original_id: i64 = Probe::new(&connection)
        .query_row("SELECT id FROM og_images WHERE page_url = ?1", params![url], |row| {
            Ok(row.get(0)?)
        })
        .unwrap();

    // Same identity tuple, but the orchestrator bumped `fetch_attempts`
    // and shifted the negative-cache window. The row must keep its id
    // (no DELETE+INSERT churn) but reflect the new bookkeeping fields.
    let next =
        OgImageInsert { refetch_after: Some("2026-07-31T00:00:00Z"), fetch_attempts: 2, ..first };
    upsert_og_image(&connection, &next).unwrap();
    let (id_after, attempts_after, refetch_after_after): (i64, i64, String) =
        Probe::new(&connection)
            .query_row(
                "SELECT id, fetch_attempts, refetch_after FROM og_images WHERE page_url = ?1",
                params![url],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
    assert_eq!(id_after, original_id, "idempotent upsert must keep the same row id");
    assert_eq!(attempts_after, 2);
    assert_eq!(refetch_after_after, "2026-07-31T00:00:00Z");
}

#[test]
fn cleanup_off_evicts_no_rows_but_still_collects_orphans() {
    let connection = open_test_archive();
    let bytes = b"\x89PNGorphan-bytes";
    upsert_og_image(&connection, &ok_insert("https://example.com/x", bytes)).unwrap();
    // Manually orphan the blob by clearing the FK.
    Probe::new(&connection)
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
    upsert_og_image(&connection, &ok_insert("https://example.com/fresh", b"\x89PNGfresh")).unwrap();
    upsert_og_image(&connection, &ok_insert("https://example.com/stale", b"\x89PNGstale")).unwrap();
    Probe::new(&connection)
        .execute(
            "UPDATE og_images
                 SET fetched_at = '2000-01-01T00:00:00Z'
                 WHERE page_url = 'https://example.com/stale'",
            (),
        )
        .unwrap();

    let report =
        run_cleanup(&connection, OgImageCleanupMode::TimeTtl { max_age_days: 30 }).unwrap();
    assert_eq!(report.deleted_rows, 1);
    assert_eq!(report.deleted_blobs, 1);
    let surviving: Vec<String> = Probe::new(&connection)
        .query("SELECT page_url FROM og_images", (), |row| Ok(row.get::<String>(0)?))
        .unwrap();
    assert_eq!(surviving, vec!["https://example.com/fresh".to_string()]);
}

#[test]
fn cleanup_size_cap_evicts_oldest_first_until_under_cap() {
    let connection = open_test_archive();
    // 100-byte payloads, three rows = 300 bytes. Cap at 150 → evict two.
    let bytes = vec![0_u8; 100];
    for (idx, url) in ["a", "b", "c"].iter().enumerate() {
        upsert_og_image(&connection, &ok_insert(&format!("https://example.com/{url}"), &bytes))
            .unwrap();
        Probe::new(&connection)
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
        upsert_og_image(&connection, &ok_insert(&format!("https://example.com/{url}"), &bytes))
            .unwrap();
        Probe::new(&connection)
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
    let mode = OgImageCleanupMode::SizeCap { max_bytes: (before.total_bytes / 2) as u64 };
    let report = run_cleanup(&connection, mode).unwrap();
    let after = storage_stats(&connection).unwrap();
    assert!(report.deleted_rows >= 1, "expected at least one eviction");
    assert!(after.total_bytes <= (before.total_bytes / 2));
    // Oldest row ('/a') must be gone before younger rows under SizeCap.
    // Count rows whose page_url matches /a via COUNT(*) so the row
    // mapper closure walks its success path (vs. the prior None-arm
    // pattern that left the mapper dead).
    let surviving_a_count: i64 = Probe::new(&connection)
        .query_row(
            "SELECT COUNT(*) FROM og_images WHERE page_url = 'https://example.com/a'",
            (),
            |row| Ok(row.get(0)?),
        )
        .unwrap();
    assert_eq!(surviving_a_count, 0);
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
    upsert_og_image(&connection, &ok_insert("https://example.com/shared-a", &big_bytes)).unwrap();
    upsert_og_image(&connection, &ok_insert("https://example.com/shared-b", &big_bytes)).unwrap();
    upsert_og_image(&connection, &ok_insert("https://example.com/shared-c", &big_bytes)).unwrap();
    upsert_og_image(&connection, &ok_insert("https://example.com/unique", &small_bytes)).unwrap();
    // Make /unique oldest so it would be evicted first by fetched_at.
    Probe::new(&connection)
        .execute(
            "UPDATE og_images SET fetched_at = '2026-01-01T00:00:00Z'
                 WHERE page_url = 'https://example.com/unique'",
            (),
        )
        .unwrap();

    let stats_before = storage_stats(&connection).unwrap();
    assert_eq!(stats_before.blob_count, 2);
    assert_eq!(stats_before.total_bytes, (big_bytes.len() + small_bytes.len()) as i64,);

    let report =
        run_cleanup(&connection, OgImageCleanupMode::SizeCap { max_bytes: 700 * 1024 }).unwrap();
    // The unique blob accounts for the only bytes that can actually be
    // reclaimed without dropping all three shared rows. The eviction
    // should pick exactly the unique row (oldest fetched_at) and stop.
    assert_eq!(report.deleted_rows, 1);
    assert_eq!(report.deleted_blobs, 1);
    assert_eq!(report.reclaimed_bytes, small_bytes.len() as i64);

    // Shared rows survive.
    let surviving: Vec<String> = Probe::new(&connection)
        .query("SELECT page_url FROM og_images ORDER BY page_url", (), |row| {
            Ok(row.get::<String>(0)?)
        })
        .unwrap();
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
        upsert_og_image(&connection, &ok_insert(&format!("https://example.com/{url}"), &bytes))
            .unwrap();
    }
    // /b shown most recently, /a shown long ago, /c never shown.
    Probe::new(&connection)
        .execute(
            "UPDATE og_images SET last_shown_at = '2026-05-18T00:00:00Z'
                 WHERE page_url = 'https://example.com/b'",
            (),
        )
        .unwrap();
    Probe::new(&connection)
        .execute(
            "UPDATE og_images SET last_shown_at = '2026-01-01T00:00:00Z'
                 WHERE page_url = 'https://example.com/a'",
            (),
        )
        .unwrap();

    let before = storage_stats(&connection).unwrap();
    let mode = OgImageCleanupMode::Lru { max_bytes: (before.total_bytes / 3) as u64 };
    run_cleanup(&connection, mode).unwrap();

    // /c (never shown) and /a (oldest shown) should be gone before /b.
    let surviving: Vec<String> = Probe::new(&connection)
        .query("SELECT page_url FROM og_images ORDER BY page_url", (), |row| {
            Ok(row.get::<String>(0)?)
        })
        .unwrap();
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

    let status: String = Probe::new(&connection)
        .query_row(
            "SELECT fetch_status FROM og_images WHERE page_url = ?1",
            params!["https://example.com/missing"],
            |row| Ok(row.get(0)?),
        )
        .unwrap();
    assert_eq!(status, fetch_status::MISSING);
}

#[test]
fn mark_shown_bumps_last_shown_at_for_provided_urls() {
    let connection = open_test_archive();
    upsert_og_image(&connection, &ok_insert("https://example.com/page", b"\x89PNGpage")).unwrap();
    assert!(
        Probe::new(&connection)
            .query_row::<Option<String>>(
                "SELECT last_shown_at FROM og_images WHERE page_url = ?1",
                params!["https://example.com/page"],
                |row| Ok(row.get(0)?),
            )
            .unwrap()
            .is_none()
    );

    mark_og_images_shown(&connection, &["https://example.com/page".into()]).unwrap();
    let stamped: Option<String> = Probe::new(&connection)
        .query_row(
            "SELECT last_shown_at FROM og_images WHERE page_url = ?1",
            params!["https://example.com/page"],
            |row| Ok(row.get(0)?),
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
    upsert_og_image(&connection, &ok_insert("https://example.com/a", b"\x89PNGa")).unwrap();
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
    let vault = Vault::open_temp().unwrap();
    let png_bytes: [u8; 9] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x01];
    upsert_og_image(&vault, &ok_insert("https://example.com/p", &png_bytes)).unwrap();

    let results = load_og_images(
        &vault,
        vec![
            HistoryOgImageLookupEntry { url: "https://example.com/p".into() },
            HistoryOgImageLookupEntry { url: "https://example.com/p".into() },
            HistoryOgImageLookupEntry { url: "https://example.com/q".into() },
        ],
    )
    .expect("load og images");
    assert_eq!(results.len(), 2);
    let p_hit = results.iter().find(|r| r.url == "https://example.com/p").expect("p result");
    // The History detail panel and row thumbnails render this data URL as-is.
    assert_eq!(p_hit.fetch_status, "ok");
    assert_eq!(
        p_hit.og_image.as_ref().map(|image| image.data_url.as_str()),
        Some("data:image/png;base64,iVBORw0KGgoB")
    );
    let q_miss = results.iter().find(|r| r.url == "https://example.com/q").expect("q result");
    assert!(q_miss.og_image.is_none());
    assert_eq!(q_miss.fetch_status, "pending");
}

#[test]
fn prefetch_and_eviction_walk_indexes_without_sorting_the_archive() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.store().read(Db::Archive,|s| {
        for (query,index) in [
            (super::reads::PREFETCH_SQL.replace("?1","10"),"idx_urls_last_visit_ms"),
            ("SELECT id,image_blob_hash FROM og_images INDEXED BY idx_og_images_eviction ORDER BY fetched_at,id LIMIT 1000".into(),"idx_og_images_eviction"),
            ("SELECT id,image_blob_hash FROM og_images INDEXED BY idx_og_images_lru ORDER BY last_shown_at IS NULL DESC,last_shown_at,fetched_at,id LIMIT 1000".into(),"idx_og_images_lru"),
        ] {
            let plan=s.query(&format!("EXPLAIN QUERY PLAN {query}"),(),|r|Ok(r.get::<String>(3)?))?.join("\n");
            assert!(plan.contains(index),"{plan}");
            assert!(!plan.to_ascii_uppercase().contains("TEMP B-TREE"),"{plan}");
            assert!(!plan.to_ascii_uppercase().contains("SORT"),"{plan}");
        }
        Ok(())
    }).unwrap();
}
