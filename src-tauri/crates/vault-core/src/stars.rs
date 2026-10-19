//! Responsibilities: preserve canonical URL/domain favorites and indexed lookups.
//! Not responsible for: full-text indexing, AI scoring or opening connections.
//! Failure modes: normalization changes identity, idempotent writes lose provenance,
//! domain/prefix probes overmatch, hidden visits reappear, interactive writes starve.
//! Tests: stars::tests and archive::ingest::concurrency_tests.

use crate::{
    Vault,
    models::{SetStarRequest, StarCounts, StarEntityKind, StarListItem, StarSort},
    utils::now_rfc3339,
    visit_taxonomy::{
        normalize_visit_url, registrable_domain_for_host, registrable_domain_for_url,
    },
};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use vault_store::{Db, Priority, Session, params};
mod list;
#[cfg(test)]
mod pagination_tests;
const STAR_VISIT_CHUNK: usize = 500;

/// Favorites commit at interactive priority between import batches.
pub fn set_star(vault: &Vault, request: SetStarRequest) -> Result<()> {
    let key = canonicalize_key(request.entity_kind, &request.entity_key)?;
    vault.store().write(Db::Archive, Priority::Interactive, move |s| {
        s.execute(
            "INSERT INTO star(entity_kind,entity_key,starred_at,source_profile) VALUES(?1,?2,?3,?4)
            ON CONFLICT(entity_kind,entity_key) DO UPDATE SET starred_at=excluded.starred_at,
            source_profile=COALESCE(excluded.source_profile,star.source_profile)",
            params![request.entity_kind.as_str(), key, now_rfc3339(), request.source_profile],
        )?;
        Ok(())
    })?;
    Ok(())
}

/// A repeated removal is harmless, including after optimistic UI updates.
pub fn unset_star(vault: &Vault, request: SetStarRequest) -> Result<()> {
    let key = canonicalize_key(request.entity_kind, &request.entity_key)?;
    vault.store().write(Db::Archive, Priority::Interactive, move |s| {
        s.execute(
            "DELETE FROM star WHERE entity_kind=?1 AND entity_key=?2",
            params![request.entity_kind.as_str(), key],
        )?;
        Ok(())
    })?;
    Ok(())
}

/// Answers only the supplied visible keys; variants share their canonical favorite.
pub fn is_starred_batch(
    vault: &Vault,
    kind: StarEntityKind,
    keys: &[String],
) -> Result<HashMap<String, bool>> {
    let keys =
        keys.iter().map(|key| (key.clone(), canonicalize_key(kind, key).ok())).collect::<Vec<_>>();
    vault.read(Db::Archive, move |s| {
        let mut values = vec![vault_store::Value::Text(kind.as_str().to_owned())];
        values.extend(keys.iter().filter_map(|(_, key)| key.clone()).map(vault_store::Value::Text));
        let found = if values.len() == 1 {
            std::collections::HashSet::new()
        } else {
            s.query(
                &format!(
                    "SELECT entity_key FROM star WHERE entity_kind=? AND entity_key IN ({})",
                    vec!["?"; values.len() - 1].join(",")
                ),
                values,
                |r| Ok(r.get::<String>(0)?),
            )?
            .into_iter()
            .collect()
        };
        Ok(keys
            .into_iter()
            .map(|(original, key)| {
                let starred = key.is_some_and(|key| found.contains(&key));
                (original, starred)
            })
            .collect())
    })
}

/// Keeps the hub badges consistent with the canonical favorite set.
pub fn star_counts(vault: &Vault) -> Result<StarCounts> {
    vault.read(Db::Archive, |s| {
        Ok(StarCounts {
            urls: s.query_one("SELECT COUNT(*) FROM star WHERE entity_kind='url'", (), |r| {
                Ok(r.get(0)?)
            })?,
            domains: s.query_one(
                "SELECT COUNT(*) FROM star WHERE entity_kind='domain'",
                (),
                |r| Ok(r.get(0)?),
            )?,
        })
    })
}

/// Provides the hub with canonical titles and visit counts for its favorites.
pub fn list_stars(
    vault: &Vault,
    kind: Option<StarEntityKind>,
    sort: StarSort,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<crate::models::StarPage> {
    list::page(vault, kind, sort, limit, cursor)
}

/// Lets retrieval reuse one favorite set without a per-result query.
pub fn load_starred_matcher(vault: &Vault) -> Result<StarredMatcher> {
    vault.read(Db::Archive, load_starred_matcher_from)
}
fn load_starred_matcher_from(s: &Session<'_>) -> Result<StarredMatcher> {
    let mut matcher = StarredMatcher::default();
    s.for_each("SELECT entity_kind,entity_key FROM star", (), |r| {
        let key = r.get::<String>(1)?;
        match parse_kind(&r.get::<String>(0)?) {
            Some(StarEntityKind::Url) => {
                matcher.canonical_urls.insert(key);
            }
            Some(StarEntityKind::Domain) => {
                matcher.domains.insert(key);
            }
            None => {}
        }
        Ok(())
    })?;
    Ok(matcher)
}

/// Resolves only starred pages and domains for canonical eligibility checks.
pub fn starred_history_ids(vault: &Vault, matcher: &StarredMatcher) -> Result<HashSet<i64>> {
    let matcher = matcher.clone();
    vault.read(Db::Archive, move |s| visit_ids_for_url_ids(s, &starred_url_ids(s, &matcher)?))
}
fn starred_url_ids(s: &Session<'_>, matcher: &StarredMatcher) -> Result<HashSet<i64>> {
    let mut ids = HashSet::new();
    for url in &matcher.canonical_urls {
        ids.extend(url_ids_for_canonical(s, url)?);
    }
    for domain in &matcher.domains {
        ids.extend(url_ids_for_domain(s, domain)?);
    }
    Ok(ids)
}
fn url_ids_for_canonical(s: &Session<'_>, key: &str) -> Result<Vec<i64>> {
    Ok(s.query(crate::pages::URL_IDS_SQL, [key], |r| Ok(r.get(0)?))?)
}

const DOMAIN_URL_IDS_SQL: &str = "SELECT u.id FROM pages p JOIN urls u ON u.page_id=p.id WHERE p.registrable_domain IS NOT NULL AND p.registrable_domain=?1";

fn url_ids_for_domain(s: &Session<'_>, domain: &str) -> Result<Vec<i64>> {
    Ok(s.query(DOMAIN_URL_IDS_SQL, [domain], |r| Ok(r.get(0)?))?)
}
fn visit_ids_for_url_ids(s: &Session<'_>, url_ids: &HashSet<i64>) -> Result<HashSet<i64>> {
    let mut result = HashSet::new();
    let ids = url_ids.iter().copied().collect::<Vec<_>>();
    for chunk in ids.chunks(STAR_VISIT_CHUNK) {
        let sql = visit_ids_query(chunk.len());
        s.for_each(&sql, chunk.to_vec(), |r| {
            result.insert(r.get(0)?);
            Ok(())
        })?;
    }
    Ok(result)
}
fn visit_ids_query(count: usize) -> String {
    format!(
        "SELECT id FROM visits WHERE reverted_at IS NULL AND url_id IN ({})",
        vec!["?"; count].join(",")
    )
}
fn canonicalize_key(kind: StarEntityKind, raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("entity key is required");
    }
    match kind {
        StarEntityKind::Url => normalize_visit_url(trimmed)
            .map(|normalized| normalized.canonical_url)
            .with_context(|| format!("could not canonicalize URL `{trimmed}`")),
        StarEntityKind::Domain => {
            // Accept either a bare host/domain or a full URL for ergonomics:
            // the detail panel hands us a domain, but assistant evidence and
            // search rows may pass a URL.
            let domain = registrable_domain_for_url(trimmed)
                .filter(|domain| !domain.is_empty())
                .unwrap_or_else(|| registrable_domain_for_host(trimmed));
            if domain.is_empty() {
                anyhow::bail!("could not derive a registrable domain from `{trimmed}`");
            }
            Ok(domain)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StarredMatcher {
    canonical_urls: HashSet<String>,
    domains: HashSet<String>,
}

impl StarredMatcher {
    /// Returns whether nothing is starred (so callers can skip the boost/facet work entirely).
    pub fn is_empty(&self) -> bool {
        self.canonical_urls.is_empty() && self.domains.is_empty()
    }

    /// Returns whether a result's raw URL resolves to a starred page or a starred domain.
    ///
    /// Canonicalizes the URL through [`normalize_visit_url`] (the star-key canonicalization) so a
    /// tracking-param/host-casing variant of a starred page still reads as starred; an unparseable URL
    /// is simply not URL-starred. The domain check uses the registrable domain so a domain star covers
    /// every page on that source. PURE → unit-tested directly.
    pub fn is_starred(&self, url: &str) -> bool {
        if !self.canonical_urls.is_empty() {
            if let Some(canonical) =
                normalize_visit_url(url).map(|normalized| normalized.canonical_url)
            {
                if self.canonical_urls.contains(&canonical) {
                    return true;
                }
            }
        }
        if !self.domains.is_empty() {
            if let Some(domain) =
                registrable_domain_for_url(url).filter(|domain| !domain.is_empty())
            {
                if self.domains.contains(&domain) {
                    return true;
                }
            }
        }
        false
    }
}

fn parse_kind(text: &str) -> Option<StarEntityKind> {
    match text {
        "url" => Some(StarEntityKind::Url),
        "domain" => Some(StarEntityKind::Domain),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
