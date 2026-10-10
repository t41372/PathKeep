//! Responsibilities: preserve profile/page identity and canonical source facts.
//! Not responsible for: parsing, batch scheduling, evidence or run lifecycle.
//! Failure modes: duplicate source IDs/fingerprints, older metadata replacing newer
//! observations, unpublished page anchors, ignored visits widening bounds, lost nullable fields or duplicate
//! favicon bytes. Tests: dedup_scenarios* and writes_tests in both key modes.

use crate::archive::favicon_metadata::favicon_url_metadata;
use crate::archive::visit_event_fingerprint;
use crate::utils::{now_rfc3339, sha256_hex, unix_micros_to_chrome_time};
use crate::visit_taxonomy::registrable_domain_for_url;
use browser_history_parser::{
    ParsedDownload, ParsedFavicon, ParsedSearchTerm, ParsedUrl, ParsedVisit,
};
use std::collections::HashMap;
use vault_store::{Result, Session, params};

/// Buffers visit-time bounds per canonical URL so one post-pass can update URL ranges.
#[derive(Debug)]
pub(super) struct UrlVisitBounds {
    pub(super) first_visit_ms: i64,
    pub(super) first_visit_iso: String,
    pub(super) last_visit_ms: i64,
    pub(super) last_visit_iso: String,
}

/// Upserts the stable source-profile identity row and returns its canonical id.
pub(crate) fn upsert_source_profile(
    archive: &Session<'_>,
    profile: &crate::models::BrowserProfile,
) -> Result<i64> {
    let enabled = archive
        .query(
            "SELECT enabled FROM source_profiles WHERE profile_key=?1",
            [profile.profile_id.clone()],
            |r| Ok(r.get::<i64>(0)?),
        )?
        .pop();
    if enabled != Some(1) {
        crate::archive::totals::add(archive, "sourceProfiles", 1)?;
    }
    let browser_kind =
        profile.profile_id.split(':').next().unwrap_or(&profile.browser_family).to_string();
    let browser_product = if profile.browser_name.trim().is_empty() {
        browser_kind.clone()
    } else {
        profile.browser_name.clone()
    };
    archive.query_one(
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
            profile.browser_family.clone(),
            browser_product,
            profile.browser_version.clone(),
            profile.profile_name.clone(),
            profile.profile_path.clone(),
            now_rfc3339(),
            profile.profile_id.clone(),
            profile.user_name.clone(),
            now_rfc3339(),
        ],
        |row| Ok(row.get(0)?),
    )
}

/// Upserts one canonical URL row and returns its stable archive id.
pub(super) fn upsert_url(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url: &ParsedUrl,
    payload_hash: &str,
) -> Result<(i64, i64, Option<i64>)> {
    let recorded_at = now_rfc3339();
    let previous=archive.query("SELECT page_id,last_visit_ms FROM urls WHERE source_profile_id=?1 AND source_url_id=?2",
        params![source_profile_id,url.source_url_id],|r|Ok((r.get::<i64>(0)?,r.get::<Option<i64>>(1)?)))?.pop();
    // An older observation keeps the current URL. Do not allocate an unreferenced page.
    let page_id = match previous {
        Some((page, last)) if last.is_none_or(|last| url.last_visit_ms <= last) => page,
        _ => archive.query_one(
            "INSERT INTO pages(url,registrable_domain,canonical_url) VALUES(?1,?2,?3)
             ON CONFLICT(url) DO UPDATE SET url=excluded.url RETURNING id",
            params![
                url.url.clone(),
                registrable_domain_for_url(&url.url),
                crate::pages::canonical_key(&url.url)
            ],
            |r| Ok(r.get::<i64>(0)?),
        )?,
    };
    archive.query_one(
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
           recorded_at,
           page_id
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(source_profile_id, source_url_id) DO UPDATE SET
           url = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.url
             ELSE urls.url
           END,
           title = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.title
             ELSE urls.title
           END,
           visit_count = MAX(urls.visit_count, excluded.visit_count),
           typed_count = MAX(urls.typed_count, excluded.typed_count),
           hidden = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.hidden
             ELSE urls.hidden
           END,
           payload_hash = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.payload_hash
             ELSE urls.payload_hash
           END,
           recorded_at = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.recorded_at
             ELSE urls.recorded_at
           END,
           last_visit_ms = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_ms
             ELSE urls.last_visit_ms
           END,
           last_visit_iso = CASE
             WHEN excluded.last_visit_ms > urls.last_visit_ms THEN excluded.last_visit_iso
             ELSE urls.last_visit_iso
           END,
           page_id = CASE WHEN excluded.last_visit_ms > urls.last_visit_ms
             THEN excluded.page_id ELSE urls.page_id END
         RETURNING id,page_id",
        params![
            url.url.clone(),
            url.title.clone(),
            url.visit_count,
            url.typed_count,
            url.last_visit_ms,
            url.last_visit_iso.clone(),
            source_profile_id,
            run_id,
            url.source_url_id,
            url.hidden as i64,
            payload_hash,
            recorded_at,
            page_id,
        ],
        |row| Ok((row.get(0)?, row.get(1)?, previous.map(|(page, _)| page))),
    )
}

/// A new visit that another source's copy already stands in for: it is written hidden behind
/// that copy (`archive::same_visit`).
pub(super) type HiddenBehind = Option<i64>;

/// Inserts one canonical visit row when it has not already been observed for this profile;
/// its id, or `None` when it was already there.
pub(super) fn insert_visit(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
    hidden: HiddenBehind,
) -> Result<Option<i64>> {
    let fingerprint = visit_event_fingerprint(
        "chromium-history",
        &visit.url,
        unix_micros_to_chrome_time(visit.visit_time_ms.saturating_mul(1_000)),
        visit.title.as_deref(),
        visit.transition,
        visit.app_id.as_deref(),
    );
    write_visit(
        archive,
        run_id,
        source_profile_id,
        url_id,
        visit,
        payload_hash,
        &fingerprint,
        None,
        hidden,
    )
}

/// Import origins preserve their existing fingerprint domain and batch identity.
#[allow(clippy::too_many_arguments)]
pub(super) fn insert_import_visit(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
    origin: &super::consumer::ImportOrigin,
    hidden: HiddenBehind,
) -> Result<Option<i64>> {
    let fingerprint = visit_event_fingerprint(
        &origin.source_kind,
        &visit.url,
        visit.visit_time_ms,
        visit.title.as_deref(),
        visit.transition,
        visit.app_id.as_deref(),
    );
    write_visit(
        archive,
        run_id,
        source_profile_id,
        url_id,
        visit,
        payload_hash,
        &fingerprint,
        Some(origin.batch_id),
        hidden,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_visit(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    url_id: i64,
    visit: &ParsedVisit,
    payload_hash: &str,
    fingerprint: &str,
    batch_id: Option<i64>,
    hidden: HiddenBehind,
) -> Result<Option<i64>> {
    let hidden_at = hidden.map(|_| now_rfc3339());
    let ids = archive.query(
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
               import_batch_id,
               same_as,
               reverted_at,
               reverted_by_run_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                     ?18, ?19, CASE WHEN ?18 IS NULL THEN NULL ELSE ?8 END)
             RETURNING id",
        params![
            url_id,
            visit.source_visit_id.to_string(),
            visit.visit_time_ms,
            visit.visit_time_iso.clone(),
            visit.transition,
            visit.visit_duration_ms,
            source_profile_id,
            run_id,
            visit.from_visit,
            visit.is_known_to_sync as i64,
            visit.visited_link_id,
            visit.external_referrer_url.clone(),
            visit.app_id.clone(),
            fingerprint,
            payload_hash,
            now_rfc3339(),
            batch_id,
            hidden,
            hidden_at,
        ],
        |row| Ok(row.get::<i64>(0)?),
    )?;
    Ok(ids.into_iter().next())
}

/// Inserts one canonical download row when it is new for the source profile.
pub(super) fn insert_download(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    download: &ParsedDownload,
    payload_hash: &str,
) -> Result<u64> {
    archive.execute(
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
            download.guid.clone(),
            download.current_path.clone(),
            download.target_path.clone(),
            download.start_time_ms,
            download.start_time_iso.clone(),
            download.total_bytes,
            download.received_bytes,
            download.state,
            download.mime_type.clone(),
            download.original_mime_type.clone(),
            source_profile_id,
            run_id,
            payload_hash,
            now_rfc3339(),
        ],
    )
}

/// Inserts one normalized search-term row linked to the canonical URL it came from.
pub(super) fn insert_search_term(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    profile_id: &str,
    url_id: i64,
    term: &ParsedSearchTerm,
) -> Result<u64> {
    archive.execute(
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
            term.term.clone(),
            term.normalized_term.clone(),
            source_profile_id,
            run_id,
            profile_id,
            term.keyword_id,
            now_rfc3339(),
        ],
    )
}

/// Inserts one favicon payload row for later Explorer/detail rendering.
pub(super) fn insert_favicon(
    archive: &Session<'_>,
    run_id: i64,
    source_profile_id: i64,
    favicon: &ParsedFavicon,
    payload_hash: &str,
) -> Result<u64> {
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

    archive.execute(
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
            favicon.page_url.clone(),
            favicon.icon_url.clone(),
            favicon.icon_type,
            favicon.width,
            favicon.height,
            favicon.last_updated_ms,
            favicon.last_updated_iso.clone(),
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
}

/// Updates the stored first/last visit bounds for one canonical URL after visit inserts.
pub(super) fn sync_url_bounds(
    archive: &Session<'_>,
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
            bounds.first_visit_iso.clone(),
            bounds.last_visit_ms,
            bounds.last_visit_iso.clone()
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
#[path = "writes_tests.rs"]
mod tests;
