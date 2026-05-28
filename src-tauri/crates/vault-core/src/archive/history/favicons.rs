//! Lazy Explorer favicon hydration.
//!
//! ## Responsibilities
//! - Resolve favicon payloads for already-visible history rows.
//! - Preserve exact page → same-host fallback precedence. Cross-host
//!   fallbacks at the registrable-domain level are intentionally
//!   forbidden — `tim.github.io` must not borrow `nick.github.io`'s
//!   icon, and a Chronicle entry on `tim.dev` must not borrow GitHub's
//!   icon just because they share a registrable suffix.
//! - Deduplicate repeated lookup entries before touching SQLite.
//!
//! ## Not responsible for
//! - Selecting visible history rows.
//! - Importing favicon source tables.
//! - Rendering image data in the frontend.
//! - Inventing icons when neither the exact page URL nor the same FQDN
//!   carries one — that's the fallback card's job (PaperContactFrame).
//!
//! ## Dependencies
//! - Archive connections and favicon URL metadata from the archive schema.
//! - `image_data_to_data_url` for shaping browser-safe payloads.
//!
//! ## Performance notes
//! - The lookup path prepares each statement once per request and relies on
//!   dedicated favicon indexes. It must not degrade back to full-table scans.
//! - The registrable-domain indexes (`idx_favicons_registrable*`) still
//!   exist in the schema for forward-compatibility but are dormant. The
//!   per-row write cost is acceptable; revisiting their removal would
//!   require a schema migration.

use super::super::{open_archive_connection, schema::favicon_url_metadata};
use crate::{
    config::ProjectPaths,
    models::{AppConfig, HistoryFavicon, HistoryFaviconLookupEntry, HistoryFaviconLookupResult},
    utils::image_data_to_data_url,
};
use anyhow::Result;
use rusqlite::{OptionalExtension, Params, Statement, params};
use std::collections::{HashMap, HashSet};

const LOAD_FAVICON_PROFILE_SQL: &str = r#"
SELECT id
FROM source_profiles
WHERE profile_key = ?1
LIMIT 1
"#;

pub(crate) const LOAD_FAVICON_SAME_PROFILE_PAGE_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_recall_lookup
  WHERE source_profile_id = ?1
    AND page_url = ?2
    AND (?3 <= 0 OR last_updated_ms <= ?3)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(crate) const LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_page_lookup
  WHERE source_profile_id != ?1
    AND page_url = ?2
    AND (?3 <= 0 OR last_updated_ms <= ?3)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(crate) const LOAD_FAVICON_SAME_PROFILE_HOST_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_host_profile_lookup
  WHERE source_profile_id = ?1
    AND page_host = ?2
    AND page_url != ?3
    AND (?4 <= 0 OR last_updated_ms <= ?4)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

pub(crate) const LOAD_FAVICON_CROSS_PROFILE_HOST_SQL: &str = r#"
SELECT COALESCE(favicon_blobs.image_data, favicon_match.image_data)
FROM (
  SELECT image_blob_hash, image_data
  FROM favicons INDEXED BY idx_favicons_host_lookup
  WHERE source_profile_id != ?1
    AND page_host = ?2
    AND page_url != ?3
    AND (?4 <= 0 OR last_updated_ms <= ?4)
    AND (image_blob_hash IS NOT NULL OR image_data IS NOT NULL)
  ORDER BY last_updated_ms DESC, width DESC, height DESC, id DESC
  LIMIT 1
) AS favicon_match
LEFT JOIN favicon_blobs
  ON favicon_blobs.blob_hash = favicon_match.image_blob_hash
"#;

// NOTE: registrable-domain fallback queries used to live here. They were
// removed when the fallback chain proved to leak icons across unrelated
// sites that share a registrable suffix (e.g. `*.github.io`,
// `*.vercel.app`, country-code TLDs missed by the public-suffix table).
// The dormant `idx_favicons_registrable*` indexes are kept until a
// dedicated schema migration cleans them up.
//
// Test that pinned the old behaviour:
//   archive::tests::lazy_history_favicon_lookup_respects_fallback_order
//   (the registrable-domain assertions now expect `None`).

/// Keeps the history query payload small by hydrating favicon bytes only for
/// rows the Explorer has already chosen to render.
pub fn load_history_favicons(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    entries: Vec<HistoryFaviconLookupEntry>,
) -> Result<Vec<HistoryFaviconLookupResult>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let connection = open_archive_connection(paths, config, key)?;
    let mut profile_statement = connection.prepare(LOAD_FAVICON_PROFILE_SQL)?;
    let mut same_profile_page_statement = connection.prepare(LOAD_FAVICON_SAME_PROFILE_PAGE_SQL)?;
    let mut cross_profile_page_statement =
        connection.prepare(LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL)?;
    let mut same_profile_host_statement = connection.prepare(LOAD_FAVICON_SAME_PROFILE_HOST_SQL)?;
    let mut cross_profile_host_statement =
        connection.prepare(LOAD_FAVICON_CROSS_PROFILE_HOST_SQL)?;
    let mut profile_ids = HashMap::<String, Option<i64>>::new();
    let mut seen = HashSet::new();
    let mut results = Vec::with_capacity(entries.len());

    for entry in entries {
        let cache_key = format!("{}\n{}\n{}", entry.profile_id, entry.url, entry.visit_time);
        if !seen.insert(cache_key) {
            continue;
        }

        let metadata = favicon_url_metadata(&entry.url);
        let source_profile_id = match profile_ids.get(&entry.profile_id) {
            Some(profile_id) => *profile_id,
            None => {
                let profile_id = profile_statement
                    .query_row([&entry.profile_id], |row| row.get::<_, i64>(0))
                    .optional()?;
                profile_ids.insert(entry.profile_id.clone(), profile_id);
                profile_id
            }
        };
        let image_data = if let Some(source_profile_id) = source_profile_id {
            load_entry_favicon(
                &mut same_profile_page_statement,
                &mut cross_profile_page_statement,
                &mut same_profile_host_statement,
                &mut cross_profile_host_statement,
                source_profile_id,
                &entry.url,
                entry.visit_time,
                metadata.host.as_deref(),
            )?
        } else {
            None
        };

        results.push(HistoryFaviconLookupResult {
            profile_id: entry.profile_id,
            url: entry.url,
            visit_time: entry.visit_time,
            favicon: image_data
                .as_deref()
                .and_then(image_data_to_data_url)
                .map(|data_url| HistoryFavicon { data_url }),
        });
    }

    Ok(results)
}

fn load_entry_favicon(
    same_profile_page_statement: &mut Statement<'_>,
    cross_profile_page_statement: &mut Statement<'_>,
    same_profile_host_statement: &mut Statement<'_>,
    cross_profile_host_statement: &mut Statement<'_>,
    source_profile_id: i64,
    url: &str,
    visit_time: i64,
    host: Option<&str>,
) -> Result<Option<Vec<u8>>> {
    if let Some(image_data) = query_favicon_statement(
        same_profile_page_statement,
        params![source_profile_id, url, visit_time],
    )? {
        return Ok(Some(image_data));
    }
    if let Some(image_data) = query_favicon_statement(
        cross_profile_page_statement,
        params![source_profile_id, url, visit_time],
    )? {
        return Ok(Some(image_data));
    }

    if let Some(host) = host {
        // Same-FQDN fallback only. Different pages on the same host share
        // an icon, so this is safe. Cross-FQDN guessing
        // (registrable-domain / parent domain / referer) was intentionally
        // removed — it leaked icons across unrelated sites that share a
        // public suffix.
        if let Some(image_data) = query_favicon_statement(
            same_profile_host_statement,
            params![source_profile_id, host, url, visit_time],
        )? {
            return Ok(Some(image_data));
        }
        if let Some(image_data) = query_favicon_statement(
            cross_profile_host_statement,
            params![source_profile_id, host, url, visit_time],
        )? {
            return Ok(Some(image_data));
        }
    }

    Ok(None)
}

fn query_favicon_statement<P: Params>(
    statement: &mut Statement<'_>,
    params: P,
) -> Result<Option<Vec<u8>>> {
    Ok(statement.query_row(params, |row| row.get::<_, Option<Vec<u8>>>(0)).optional()?.flatten())
}
