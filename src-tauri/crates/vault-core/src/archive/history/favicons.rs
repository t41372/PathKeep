//! Failure modes: future/unrelated icons, duplicate results, missing-profile errors, candidate scans.
//! Lazy Explorer favicon hydration.
//!
//! ## Responsibilities
//! - Resolve favicon payloads for already-visible history rows, and a site's own icon for
//!   screens that list sites.
//! - Preserve exact page → same-host fallback precedence. Cross-host
//!   fallbacks at the registrable-domain level are intentionally
//!   forbidden — `tim.github.io` must not borrow `nick.github.io`'s
//!   icon, and a Chronicle entry on `tim.dev` must not borrow GitHub's
//!   icon just because they share a registrable suffix.
//! - Deduplicate repeated lookup entries before querying Turso.
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
//! Turso 0.8.1 needs the explicit host IS NOT NULL predicate to match its partial index.
//!
//! ## Performance notes
//! - The lookup path reads one candidate per fallback stage and relies on
//!   dedicated favicon indexes. It must not degrade back to full-table scans.
//! - The registrable-domain indexes (`idx_favicons_registrable*`) still
//!   exist in the schema for forward-compatibility but are dormant. The
//!   per-row write cost is acceptable; revisiting their removal would
//!   require a schema migration.

use super::super::favicon_metadata::favicon_url_metadata;
use crate::{
    Vault,
    models::{HistoryFavicon, HistoryFaviconLookupEntry, HistoryFaviconLookupResult, SiteFavicon},
    utils::image_data_to_data_url,
};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use vault_store::{Db, Session, params};

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
    AND page_host IS NOT NULL
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
    AND page_host IS NOT NULL
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
    vault: &Vault,
    entries: Vec<HistoryFaviconLookupEntry>,
) -> Result<Vec<HistoryFaviconLookupResult>> {
    vault.read(Db::Archive, move |s| load_favicons(s, entries))
}

fn load_favicons(
    s: &Session<'_>,
    entries: Vec<HistoryFaviconLookupEntry>,
) -> Result<Vec<HistoryFaviconLookupResult>> {
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
                let profile_id = s
                    .query(LOAD_FAVICON_PROFILE_SQL, [entry.profile_id.as_str()], |r| {
                        Ok(r.get::<i64>(0)?)
                    })?
                    .pop();
                profile_ids.insert(entry.profile_id.clone(), profile_id);
                profile_id
            }
        };
        let image_data = if let Some(source_profile_id) = source_profile_id {
            load_entry_favicon(
                s,
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

/// The most recently stored icon of each site: one shown for the site's own host, else for
/// `www.` plus the site, in any profile. For screens that list sites, not visits (Insights);
/// History looks icons up through a visit instead. A site's icon is never borrowed from another
/// host under the same registrable domain (see the module note).
pub fn load_site_favicons(vault: &Vault, domains: Vec<String>) -> Result<Vec<SiteFavicon>> {
    vault.read(Db::Archive, move |s| {
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for domain in domains.into_iter().take(SITE_FAVICON_BATCH) {
            let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
            if domain.is_empty() || !seen.insert(domain.clone()) {
                continue;
            }
            let mut image = None;
            for host in [domain.clone(), format!("www.{domain}")] {
                // No profile is -1, so the cross-profile query looks in every profile.
                image = s
                    .query(LOAD_FAVICON_CROSS_PROFILE_HOST_SQL, params![-1, host, "", 0], |r| {
                        Ok(r.get::<Option<Vec<u8>>>(0)?)
                    })?
                    .pop()
                    .flatten();
                if image.is_some() {
                    break;
                }
            }
            results.push(SiteFavicon {
                favicon: image
                    .as_deref()
                    .and_then(image_data_to_data_url)
                    .map(|data_url| HistoryFavicon { data_url }),
                domain,
            });
        }
        Ok(results)
    })
}

/// At most this many sites per call: two indexed lookups each, for the rows on screen.
const SITE_FAVICON_BATCH: usize = 64;

fn load_entry_favicon(
    s: &Session<'_>,
    profile: i64,
    url: &str,
    time: i64,
    host: Option<&str>,
) -> Result<Option<Vec<u8>>> {
    for sql in [LOAD_FAVICON_SAME_PROFILE_PAGE_SQL, LOAD_FAVICON_CROSS_PROFILE_PAGE_SQL] {
        if let Some(bytes) = s
            .query(sql, params![profile, url, time], |r| Ok(r.get::<Option<Vec<u8>>>(0)?))?
            .pop()
            .flatten()
        {
            return Ok(Some(bytes));
        }
    }
    if let Some(host) = host {
        for sql in [LOAD_FAVICON_SAME_PROFILE_HOST_SQL, LOAD_FAVICON_CROSS_PROFILE_HOST_SQL] {
            if let Some(bytes) =
                s.query(sql, params![profile, host, url, time], |r| {
                    Ok(r.get::<Option<Vec<u8>>>(0)?)
                })?
                .pop()
                .flatten()
            {
                return Ok(Some(bytes));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "favicon_tests.rs"]
mod tests;
