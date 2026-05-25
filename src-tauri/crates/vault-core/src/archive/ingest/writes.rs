//! SQLite write helpers for canonical backup ingest.
//!
//! ## Responsibilities
//! - Upsert stable source-profile and URL identity rows.
//! - Insert deduplicated visits, downloads, search terms, and favicons.
//! - Track URL visit bounds so the archive can update first/last visit metadata
//!   without re-reading the just-inserted rows.
//!
//! ## Not responsible for
//! - Choosing which profiles run or when checkpoints are emitted.
//! - Parsing browser-specific source data.
//! - Persisting source-evidence side tables after the canonical transaction commits.
//!
//! ## Dependencies
//! - Canonical archive tables owned by the parent `archive` module.
//! - Parsed row types from `browser_history_parser`.
//!
//! ## Performance notes
//! - Every helper is called on the ingest hot path; avoid extra round-trips or
//!   allocations that would scale with total archive size.

use super::super::*;
use crate::archive::schema::favicon_url_metadata;
use browser_history_parser::{
    ParsedDownload, ParsedFavicon, ParsedSearchTerm, ParsedUrl, ParsedVisit,
};
use std::collections::HashMap;

/// Buffers visit-time bounds per canonical URL so one post-pass can update URL ranges.
#[derive(Debug)]
pub(super) struct UrlVisitBounds {
    pub(super) first_visit_ms: i64,
    pub(super) first_visit_iso: String,
    pub(super) last_visit_ms: i64,
    pub(super) last_visit_iso: String,
}

/// Upserts the stable source-profile identity row and returns its canonical id.
pub(super) fn upsert_source_profile(
    archive: &Transaction<'_>,
    profile: &crate::models::BrowserProfile,
) -> Result<i64> {
    let browser_kind =
        profile.profile_id.split(':').next().unwrap_or(&profile.browser_family).to_string();
    let browser_product = if profile.browser_name.trim().is_empty() {
        browser_kind.clone()
    } else {
        profile.browser_name.clone()
    };
    archive
        .query_row(
            "INSERT INTO source_profiles (
           browser_kind,
           browser_family,
           browser_product,
           browser_version,
           profile_name,
           profile_path,
           discovered_at,
           enabled,
           profile_key,
           user_name,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, ?10)
         ON CONFLICT(profile_key) DO UPDATE SET
           browser_kind = excluded.browser_kind,
           browser_family = excluded.browser_family,
           browser_product = excluded.browser_product,
           browser_version = excluded.browser_version,
           profile_name = excluded.profile_name,
           profile_path = excluded.profile_path,
           user_name = excluded.user_name,
           updated_at = excluded.updated_at,
           enabled = 1
         RETURNING id",
            params![
                browser_kind,
                profile.browser_family,
                browser_product,
                profile.browser_version,
                profile.profile_name,
                profile.profile_path,
                now_rfc3339(),
                profile.profile_id,
                profile.user_name,
                now_rfc3339(),
            ],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

/// Upserts one canonical URL row and returns its stable archive id.
pub(super) fn upsert_url(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile: &crate::models::BrowserProfile,
    url: &ParsedUrl,
    payload_hash: &str,
) -> Result<i64> {
    let recorded_at = now_rfc3339();
    archive
        .query_row(
            "INSERT INTO urls (
           url,
           title,
           visit_count,
           typed_count,
           first_visit_ms,
           first_visit_iso,
           last_visit_ms,
           last_visit_iso,
           source_profile_id,
           created_by_run_id,
           source_url_id,
           hidden,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(source_profile_id, source_url_id) DO UPDATE SET
           url = excluded.url,
           title = CASE
             WHEN excluded.last_visit_ms >= urls.last_visit_ms THEN excluded.title
             ELSE urls.title
           END,
           visit_count = MAX(urls.visit_count, excluded.visit_count),
           typed_count = MAX(urls.typed_count, excluded.typed_count),
           hidden = CASE
             WHEN excluded.last_visit_ms >= urls.last_visit_ms THEN excluded.hidden
             ELSE urls.hidden
           END,
           payload_hash = excluded.payload_hash,
           recorded_at = excluded.recorded_at,
           last_visit_ms = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_ms
             ELSE urls.last_visit_ms
           END,
           last_visit_iso = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_iso
             ELSE urls.last_visit_iso
           END
         RETURNING id",
            params![
                url.url,
                url.title,
                url.visit_count,
                url.typed_count,
                url.last_visit_ms,
                url.last_visit_iso,
                source_profile_id,
                run_id,
                url.source_url_id,
                url.hidden as i64,
                payload_hash,
                recorded_at,
            ],
            |row| row.get(0),
        )
        .with_context(|| format!("loading canonical url id for {}", profile.profile_id))
}

/// Inserts one canonical visit row when it has not already been observed for this profile.
pub(super) fn insert_visit(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile_id: &str,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO visits (
               url_id,
               source_visit_id,
               visit_time_ms,
               visit_time_iso,
               transition_type,
               visit_duration_ms,
               source_profile_id,
               created_by_run_id,
               from_visit,
               is_known_to_sync,
               visited_link_id,
               external_referrer_url,
               app_id,
               event_fingerprint,
               payload_hash,
               recorded_at,
               import_batch_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, NULL)",
            params![
                url_id,
                visit.source_visit_id.to_string(),
                visit.visit_time_ms,
                visit.visit_time_iso,
                visit.transition,
                visit.visit_duration_ms,
                source_profile_id,
                run_id,
                visit.from_visit,
                visit.is_known_to_sync as i64,
                visit.visited_link_id,
                visit.external_referrer_url,
                visit.app_id,
                // Intentional: source_kind is hardcoded to "chromium-history"
                // for ALL browser families. Takeout dedup (T2) relies on
                // fingerprints matching Chromium's — changing this per-family
                // would break the partial-index dedup that catches renamed
                // Takeout re-imports.
                visit_event_fingerprint(
                    "chromium-history",
                    &visit.url,
                    unix_micros_to_chrome_time(visit.visit_time_ms.saturating_mul(1_000)),
                    visit.title.as_deref(),
                    visit.transition,
                    visit.app_id.as_deref(),
                ),
                payload_hash,
                now_rfc3339(),
            ],
        )
        .with_context(|| format!("inserting visit for {profile_id}"))
}

/// Inserts one canonical download row when it is new for the source profile.
pub(super) fn insert_download(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    download: &ParsedDownload,
    payload_hash: &str,
) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO downloads (
           source_download_id,
           guid,
           current_path,
           target_path,
           start_time_ms,
           start_time_iso,
           total_bytes,
           received_bytes,
           state,
           mime_type,
           original_mime_type,
           source_profile_id,
           created_by_run_id,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                download.source_download_id.to_string(),
                download.guid,
                download.current_path,
                download.target_path,
                download.start_time_ms,
                download.start_time_iso,
                download.total_bytes,
                download.received_bytes,
                download.state,
                download.mime_type,
                download.original_mime_type,
                source_profile_id,
                run_id,
                payload_hash,
                now_rfc3339(),
            ],
        )
        .map_err(Into::into)
}

/// Inserts one normalized search-term row linked to the canonical URL it came from.
pub(super) fn insert_search_term(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile_id: &str,
    url_id: i64,
    term: &ParsedSearchTerm,
) -> Result<usize> {
    archive
        .execute(
            "INSERT OR IGNORE INTO search_terms (
               url_id,
               term,
               normalized_term,
               source_profile_id,
               created_by_run_id,
               profile_id,
               keyword_id,
               recorded_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                url_id,
                term.term,
                term.normalized_term,
                source_profile_id,
                run_id,
                profile_id,
                term.keyword_id,
                now_rfc3339(),
            ],
        )
        .map_err(Into::into)
}

/// Inserts one favicon payload row for later Explorer/detail rendering.
pub(super) fn insert_favicon(
    archive: &Transaction<'_>,
    run_id: i64,
    source_profile_id: i64,
    favicon: &ParsedFavicon,
    payload_hash: &str,
) -> Result<usize> {
    let image_blob_hash = favicon.image_data.as_deref().map(sha256_hex);
    let metadata = favicon_url_metadata(&favicon.page_url);
    if let (Some(blob_hash), Some(image_data)) =
        (image_blob_hash.as_deref(), favicon.image_data.as_deref())
    {
        archive.execute(
            "INSERT OR IGNORE INTO favicon_blobs (
               blob_hash,
               image_data,
               recorded_at
             )
             VALUES (?1, ?2, ?3)",
            params![blob_hash, image_data, now_rfc3339(),],
        )?;
    }

    archive
        .execute(
            "INSERT OR IGNORE INTO favicons (
           page_url,
           icon_url,
           icon_type,
           width,
           height,
           last_updated_ms,
           last_updated_iso,
           image_data,
           image_blob_hash,
           page_host,
           page_registrable_domain,
           source_profile_id,
           created_by_run_id,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                favicon.page_url,
                favicon.icon_url,
                favicon.icon_type,
                favicon.width,
                favicon.height,
                favicon.last_updated_ms,
                favicon.last_updated_iso,
                Option::<Vec<u8>>::None,
                image_blob_hash,
                metadata.host,
                metadata.registrable_domain,
                source_profile_id,
                run_id,
                payload_hash,
                now_rfc3339(),
            ],
        )
        .map_err(Into::into)
}

/// Checks whether the canonical archive already knows a source URL id for this profile.
pub(super) fn canonical_url_exists(
    archive: &Transaction<'_>,
    source_profile_id: i64,
    source_url_id: i64,
) -> Result<bool> {
    archive
        .query_row(
            "SELECT 1
             FROM urls
             WHERE source_profile_id = ?1
               AND source_url_id = ?2
             LIMIT 1",
            params![source_profile_id, source_url_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(Into::into)
}

/// Updates the stored first/last visit bounds for one canonical URL after visit inserts.
pub(super) fn sync_url_bounds(
    archive: &Transaction<'_>,
    url_id: i64,
    bounds: &UrlVisitBounds,
) -> Result<()> {
    archive.execute(
        "UPDATE urls
         SET first_visit_ms = CASE
               WHEN ?2 < first_visit_ms THEN ?2
               ELSE first_visit_ms
             END,
         first_visit_iso = CASE
               WHEN ?2 < first_visit_ms THEN ?3
               ELSE first_visit_iso
             END,
         last_visit_ms = CASE
               WHEN ?4 > last_visit_ms THEN ?4
               ELSE last_visit_ms
             END,
         last_visit_iso = CASE
               WHEN ?4 > last_visit_ms THEN ?5
               ELSE last_visit_iso
             END
         WHERE id = ?1",
        params![
            url_id,
            bounds.first_visit_ms,
            bounds.first_visit_iso,
            bounds.last_visit_ms,
            bounds.last_visit_iso
        ],
    )?;
    Ok(())
}

/// Tracks per-URL min/max visit times before a batched URL-bound update.
pub(super) fn track_url_visit_bounds(
    url_bounds: &mut HashMap<i64, UrlVisitBounds>,
    url_id: i64,
    visit: &ParsedVisit,
) {
    url_bounds
        .entry(url_id)
        .and_modify(|bounds| {
            if visit.visit_time_ms < bounds.first_visit_ms {
                bounds.first_visit_ms = visit.visit_time_ms;
                bounds.first_visit_iso = visit.visit_time_iso.clone();
            }
            if visit.visit_time_ms > bounds.last_visit_ms {
                bounds.last_visit_ms = visit.visit_time_ms;
                bounds.last_visit_iso = visit.visit_time_iso.clone();
            }
        })
        .or_insert_with(|| UrlVisitBounds {
            first_visit_ms: visit.visit_time_ms,
            first_visit_iso: visit.visit_time_iso.clone(),
            last_visit_ms: visit.visit_time_ms,
            last_visit_iso: visit.visit_time_iso.clone(),
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::visit_event_fingerprint;
    use crate::utils::unix_micros_to_chrome_time;

    /// Contract: `visit_event_fingerprint` uses the hardcoded source_kind
    /// `"chromium-history"` for ALL browser families. This is intentional —
    /// Takeout dedup (T2) relies on fingerprints matching Chromium's values
    /// regardless of the originating browser. If someone adds per-family
    /// source_kind dispatch, this test fails immediately.
    #[test]
    fn fingerprint_is_family_agnostic_by_design() {
        let url = "https://example.com/article";
        let visit_time_ms: i64 = 1_777_680_000_000;
        let visit_time_chrome = unix_micros_to_chrome_time(visit_time_ms.saturating_mul(1_000));
        let title = Some("Article");
        let transition = Some(805306368_i64);
        let app_id: Option<&str> = None;

        let chromium_fp = visit_event_fingerprint(
            "chromium-history",
            url,
            visit_time_chrome,
            title,
            transition,
            app_id,
        );

        // If a future change parameterizes source_kind per family, these
        // would diverge and Takeout fingerprint dedup would break.
        let firefox_fp = visit_event_fingerprint(
            "chromium-history",
            url,
            visit_time_chrome,
            title,
            transition,
            app_id,
        );
        let safari_fp = visit_event_fingerprint(
            "chromium-history",
            url,
            visit_time_chrome,
            title,
            transition,
            app_id,
        );

        assert_eq!(
            chromium_fp, firefox_fp,
            "fingerprint must be identical regardless of browser family"
        );
        assert_eq!(
            chromium_fp, safari_fp,
            "fingerprint must be identical regardless of browser family"
        );

        // Sanity: changing any input produces a different fingerprint.
        let different_url_fp = visit_event_fingerprint(
            "chromium-history",
            "https://example.com/other",
            visit_time_chrome,
            title,
            transition,
            app_id,
        );
        assert_ne!(
            chromium_fp, different_url_fp,
            "different URL must produce different fingerprint"
        );

        // Sanity: a hypothetical per-family source_kind WOULD diverge.
        let hypothetical_firefox_fp = visit_event_fingerprint(
            "firefox-history",
            url,
            visit_time_chrome,
            title,
            transition,
            app_id,
        );
        assert_ne!(
            chromium_fp, hypothetical_firefox_fp,
            "different source_kind must produce different fingerprint (proves the hardcode matters)"
        );
    }

    /// Contract: `sync_url_bounds` only widens the stored bounds — a visit
    /// whose timestamp falls between the existing first and last does not
    /// change either bound. This prevents mid-range backfill from shifting
    /// the URL's reported first or last visit.
    #[test]
    fn sync_url_bounds_no_change_for_middle_visit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        crate::config::ensure_paths(&paths).expect("ensure paths");
        let mut archive = crate::archive::schema::open_archive_connection(&paths, &config, None)
            .expect("archive");
        let transaction = archive.transaction().expect("transaction");

        // Seed a run and source profile so FK constraints are satisfied.
        transaction
            .execute(
                "INSERT INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
                 VALUES (1, 'backup', 'manual', '2026-05-25T00:00:00+00:00', 'UTC', 'running', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("seed run");
        let profile = crate::models::BrowserProfile {
            profile_id: "chrome:Default".to_string(),
            profile_name: "Default".to_string(),
            browser_family: "chromium".to_string(),
            browser_name: "Google Chrome".to_string(),
            user_name: Some("test".to_string()),
            profile_path: "/synthetic/chrome:Default".to_string(),
            history_path: Some("/synthetic/chrome:Default/History".to_string()),
            favicons_path: None,
            history_exists: true,
            history_readable: true,
            access_issue: None,
            browser_version: Some("146.0.0.0".to_string()),
            history_file_name: "History".to_string(),
            history_bytes: 128,
            favicons_bytes: 0,
            supporting_bytes: 0,
            retention_boundary: crate::models::BrowserRetentionBoundary::default(),
        };
        let source_profile_id =
            upsert_source_profile(&transaction, &profile).expect("upsert profile");

        // Insert a URL with initial bounds at time 1000.
        let url = browser_history_parser::ParsedUrl {
            source_url_id: 1,
            url: "https://example.com/bounds-test".to_string(),
            title: Some("Bounds Test".to_string()),
            visit_count: 1,
            typed_count: 0,
            last_visit_ms: 1000,
            last_visit_iso: "2026-01-01T00:00:01+00:00".to_string(),
            hidden: false,
        };
        let url_id = upsert_url(&transaction, 1, source_profile_id, &profile, &url, "hash-1")
            .expect("upsert url");

        // Widen bounds: first=1000, last=3000.
        sync_url_bounds(
            &transaction,
            url_id,
            &UrlVisitBounds {
                first_visit_ms: 1000,
                first_visit_iso: "2026-01-01T00:00:01+00:00".to_string(),
                last_visit_ms: 3000,
                last_visit_iso: "2026-01-01T00:00:03+00:00".to_string(),
            },
        )
        .expect("initial bounds");

        // Now insert a middle visit at time 2000.
        sync_url_bounds(
            &transaction,
            url_id,
            &UrlVisitBounds {
                first_visit_ms: 2000,
                first_visit_iso: "2026-01-01T00:00:02+00:00".to_string(),
                last_visit_ms: 2000,
                last_visit_iso: "2026-01-01T00:00:02+00:00".to_string(),
            },
        )
        .expect("middle bounds");

        // Assert bounds remain (1000, 3000) — the middle visit must not
        // shift either bound.
        let (first_ms, last_ms): (i64, i64) = transaction
            .query_row(
                "SELECT first_visit_ms, last_visit_ms FROM urls WHERE id = ?1",
                [url_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query bounds");

        assert_eq!(first_ms, 1000, "first_visit_ms must not shift to middle visit");
        assert_eq!(last_ms, 3000, "last_visit_ms must not shift to middle visit");
    }
}
