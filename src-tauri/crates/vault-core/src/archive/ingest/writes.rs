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
    let browser_product =
        profile.profile_id.split(':').next().unwrap_or(&profile.browser_family).to_string();
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
                browser_product,
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
           title = excluded.title,
           visit_count = excluded.visit_count,
           typed_count = excluded.typed_count,
           hidden = excluded.hidden,
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
           source_profile_id,
           created_by_run_id,
           payload_hash,
           recorded_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                favicon.page_url,
                favicon.icon_url,
                favicon.icon_type,
                favicon.width,
                favicon.height,
                favicon.last_updated_ms,
                favicon.last_updated_iso,
                favicon.image_data,
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
