//! Stars (favorites / 加星): user-authored favorites keyed by canonical entity.
//!
//! A star marks a page (`canonical_url`) or a source (`registrable_domain`) as
//! a favorite. Stars are user-authored content keyed by the **canonical
//! entity** — never by visit id — so a star survives re-import, dedup, and
//! profile changes, and rides the portable `.pathkeep-bundle` export with the
//! rest of the canonical archive. "Survives re-import" holds *because* the
//! canonical key is stable: tracking-param variants and host casing collapse
//! onto the same `canonical_url` (see `normalize_visit_url`), so re-importing
//! the same page under a different raw URL still resolves to the same star.
//!
//! ## Keying — a deliberate difference from annotations
//! Stars key by `canonical_url`; **annotations (notes/tags, migration 011) key
//! by RAW url**. This is intentional, not an inconsistency: a star expresses
//! *page identity* ("I care about this page however I reached it"), so it must
//! collapse tracking variants; a note expresses something about the *exact url*
//! the user was looking at, so it must NOT collapse them. `set_star`,
//! `unset_star`, `is_starred_batch`, and the `list_stars` enrichment all
//! canonicalize consistently so a page starred via one raw URL reads as starred
//! (and enriches with its real title + summed visit count) via any variant.
//!
//! ## Responsibilities
//! - Add / remove a star for a `url` or `domain` entity (`set_star` /
//!   `unset_star`), canonicalizing the supplied key the same way the
//!   annotations + intelligence refind surfaces do.
//! - Answer "is this starred?" for the currently-visible rows only
//!   (`is_starred_batch`), so the frontend never fans out across the archive.
//! - List the Starred hub (`list_stars`), ordered recently-starred or
//!   most-revisited, enriched with title / domain / visit count.
//! - Roll up per-kind counts (`star_counts`).
//!
//! ## Not responsible for
//! - The AI working-set / heavy-embedding priority hook — that selector will
//!   join `star` to the embedding tier later (see TODO in `list_stars`).
//! - `query_family` stars (deferred): the `entity_kind` column is text so the
//!   enum can grow without a migration.
//! - Visit-level metadata — stays in `archive::history`.
//!
//! ## Performance notes
//! - `star` is tiny by design (users star hundreds, not millions). Writes and
//!   status lookups are primary-key operations; `list_stars` walks the
//!   `(entity_kind, starred_at DESC)` index and enriches the small result set.
//!   URL enrichment first tries an exact `idx_urls_url` index seek on the
//!   canonical key (the common case where the stored row already matches), then
//!   falls back to a prefix-bounded range scan on the same index, canonicalizing
//!   only the candidate rows that share the page's host+path — so nothing scans
//!   the 14.4M-row archive even when the stored visits carry tracking params.
//! - `is_starred_batch` binds one parameter per visible key (bounded by the
//!   render window), so the IN-list never grows with the archive.

use crate::{
    archive::open_archive_connection,
    config::ProjectPaths,
    models::{AppConfig, SetStarRequest, StarCounts, StarEntityKind, StarListItem, StarSort},
    utils::now_rfc3339,
    visit_taxonomy::{
        normalize_visit_url, registrable_domain_for_host, registrable_domain_for_url,
    },
};
use anyhow::{Context, Result};
use reqwest::Url;
use rusqlite::{Connection, params, params_from_iter};
use std::collections::{HashMap, HashSet};

/// Upper bound on a single `list_stars` page. The Starred hub paginates the
/// contact sheet, so a generous-but-finite cap keeps the worst case bounded
/// even if a caller forgets to pass a limit.
const MAX_LIST_LIMIT: usize = 2_000;

/// Default `list_stars` page size when the caller does not pass one.
const DEFAULT_LIST_LIMIT: usize = 500;

/// Canonicalizes a raw entity key into the stable form stored in
/// `star.entity_key`. URLs go through `normalize_visit_url` (the same
/// canonicalization annotations + refind use); domains are reduced to their
/// registrable domain. Returns an error when the key cannot be canonicalized
/// (e.g. an unparseable URL) so the UI surfaces it instead of silently keying
/// by garbage.
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

/// Adds (or refreshes) a star for the canonical entity. Idempotent: re-starring
/// an already-starred entity refreshes `starred_at` and the audit profile
/// without erroring, so the optimistic toggle stays simple.
pub fn set_star(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: SetStarRequest,
) -> Result<()> {
    let entity_key = canonicalize_key(request.entity_kind, &request.entity_key)?;
    let connection = open_archive_connection(paths, config, key)?;
    let now = now_rfc3339();
    connection
        .execute(
            r#"INSERT INTO star(entity_kind, entity_key, starred_at, source_profile)
               VALUES(?1, ?2, ?3, ?4)
               ON CONFLICT(entity_kind, entity_key) DO UPDATE SET
                 starred_at = excluded.starred_at,
                 source_profile = COALESCE(excluded.source_profile, star.source_profile)"#,
            params![request.entity_kind.as_str(), entity_key, now, request.source_profile],
        )
        .context("writing star row")?;
    Ok(())
}

/// Removes a star. Removing a star that does not exist is a no-op (the
/// optimistic toggle may double-fire), so this never errors on a miss.
pub fn unset_star(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: SetStarRequest,
) -> Result<()> {
    let entity_key = canonicalize_key(request.entity_kind, &request.entity_key)?;
    let connection = open_archive_connection(paths, config, key)?;
    connection
        .execute(
            "DELETE FROM star WHERE entity_kind = ?1 AND entity_key = ?2",
            params![request.entity_kind.as_str(), entity_key],
        )
        .context("deleting star row")?;
    Ok(())
}

/// Returns the starred status of each supplied key. The map is keyed by the
/// **caller's raw key** (not the canonical form) so the frontend can look up
/// each visible row directly; canonicalization happens internally. Keys that
/// cannot be canonicalized are reported as `false` rather than failing the
/// whole batch — one malformed row must not blank the whole render window.
pub fn is_starred_batch(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    kind: StarEntityKind,
    keys: &[String],
) -> Result<HashMap<String, bool>> {
    let mut result = HashMap::with_capacity(keys.len());
    if keys.is_empty() {
        return Ok(result);
    }

    // Map canonical key -> the raw keys the caller asked about. Several raw
    // keys can collapse onto one canonical key (tracking-param variants), so
    // we fan the answer back out to every requesting raw key.
    let mut canonical_to_raw: HashMap<String, Vec<String>> = HashMap::new();
    for raw in keys {
        result.insert(raw.clone(), false);
        if let Ok(canonical) = canonicalize_key(kind, raw) {
            canonical_to_raw.entry(canonical).or_default().push(raw.clone());
        }
    }
    if canonical_to_raw.is_empty() {
        return Ok(result);
    }

    let connection = open_archive_connection(paths, config, key)?;
    let canonical_keys: Vec<&String> = canonical_to_raw.keys().collect();
    let placeholders = std::iter::repeat_n("?", canonical_keys.len()).collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT entity_key FROM star WHERE entity_kind = ?1 AND entity_key IN ({placeholders})"
    );
    let mut statement = connection.prepare(&sql)?;
    let mut bindings: Vec<&str> = Vec::with_capacity(canonical_keys.len() + 1);
    bindings.push(kind.as_str());
    for canonical in &canonical_keys {
        bindings.push(canonical.as_str());
    }
    let starred: HashSet<String> = statement
        .query_map(params_from_iter(bindings), |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<HashSet<_>>>()
        .context("decoding is_starred rows")?;

    for canonical in starred {
        if let Some(raw_keys) = canonical_to_raw.get(&canonical) {
            for raw in raw_keys {
                result.insert(raw.clone(), true);
            }
        }
    }
    Ok(result)
}

/// Lists starred entities for the Starred hub, newest-or-most-revisited first.
/// Pass `kind = None` to list every kind interleaved by the chosen sort.
///
/// The result is enriched with the page title, registrable domain, and total
/// visit count. Because stars key by `canonical_url` while `urls.url` stores the
/// RAW url, enrichment cannot assume an exact join: a page starred via its
/// canonical form must still resolve its title + SUMMED visit count even when
/// every stored visit carries tracking params or a non-normalized host. See
/// `enrich_entity` for the (still archive-bounded) matching strategy.
pub fn list_stars(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    kind: Option<StarEntityKind>,
    sort: StarSort,
    limit: Option<usize>,
) -> Result<Vec<StarListItem>> {
    let connection = open_archive_connection(paths, config, key)?;
    let cap = limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT);
    let rows = collect_star_rows(&connection, kind, cap)?;
    let mut items: Vec<StarListItem> = rows
        .into_iter()
        .map(|(entity_kind, entity_key, starred_at)| {
            let (domain, title, visit_count) =
                enrich_entity(&connection, entity_kind, &entity_key)?;
            Ok(StarListItem { entity_kind, entity_key, starred_at, domain, title, visit_count })
        })
        .collect::<Result<Vec<_>>>()?;

    // TODO(W-AI-4c/heavy-tier): the AI working-set selector will join this
    // starred set to the embedding tier so favorites are embedded first. The
    // read model above is intentionally queryable in isolation so that hook
    // can reuse it without changing the command surface.
    if matches!(sort, StarSort::MostRevisited) {
        // The visit-count re-order happens AFTER the recency cap, so the
        // most-revisited result is the top-by-visits *within the most-recently-
        // starred `cap` window*, not a global top-N across the whole star table.
        // This is intentional and correct for the star table's scale: `cap`
        // defaults to 500 and maxes at MAX_LIST_LIMIT (2_000), while users star
        // hundreds of things — so in practice the window always covers every
        // star and the ordering is effectively global. If the star set ever
        // exceeds the cap, the sort would push visit-count ordering into the
        // recency-bounded slice only; revisit this with an ORDER BY in SQL then.
        //
        // Tie-break: equal visit_count falls back to `starred_at DESC` so a more
        // recently starred favorite sorts first and the order stays deterministic
        // between renders (pinned by `list_stars_most_revisited_breaks_ties_by_recency`).
        items.sort_by(|a, b| {
            b.visit_count.cmp(&a.visit_count).then_with(|| b.starred_at.cmp(&a.starred_at))
        });
    }
    Ok(items)
}

/// Rolls up how many entities the user has starred, per kind. PK-cheap counts.
pub fn star_counts(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<StarCounts> {
    let connection = open_archive_connection(paths, config, key)?;
    let count_for = |kind: StarEntityKind| -> Result<i64> {
        connection
            .query_row(
                "SELECT COUNT(*) FROM star WHERE entity_kind = ?1",
                params![kind.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .with_context(|| format!("counting {} stars", kind.as_str()))
    };
    Ok(StarCounts {
        urls: count_for(StarEntityKind::Url)?,
        domains: count_for(StarEntityKind::Domain)?,
    })
}

/// Reads the raw star rows (kind, key, starred_at) ordered newest-first via the
/// kind index. `kind = None` lists every kind.
fn collect_star_rows(
    connection: &Connection,
    kind: Option<StarEntityKind>,
    cap: usize,
) -> Result<Vec<(StarEntityKind, String, String)>> {
    let mut rows = Vec::new();
    let mut push_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<()> {
        let kind_text: String = row.get(0)?;
        let entity_key: String = row.get(1)?;
        let starred_at: String = row.get(2)?;
        if let Some(parsed) = parse_kind(&kind_text) {
            rows.push((parsed, entity_key, starred_at));
        }
        Ok(())
    };
    match kind {
        Some(kind) => {
            let mut statement = connection.prepare(
                "SELECT entity_kind, entity_key, starred_at FROM star
                 WHERE entity_kind = ?1 ORDER BY starred_at DESC LIMIT ?2",
            )?;
            let mut query = statement.query(params![kind.as_str(), cap as i64])?;
            while let Some(row) = query.next()? {
                push_row(row)?;
            }
        }
        None => {
            let mut statement = connection.prepare(
                "SELECT entity_kind, entity_key, starred_at FROM star
                 ORDER BY starred_at DESC LIMIT ?1",
            )?;
            let mut query = statement.query(params![cap as i64])?;
            while let Some(row) = query.next()? {
                push_row(row)?;
            }
        }
    }
    Ok(rows)
}

/// Looks up the (domain, title, visit_count) enrichment for one starred entity.
/// For domain stars the domain is the key itself and the visit count is the sum
/// across every URL on that domain; for URL stars it resolves the title +
/// SUMMED visit count for the **canonical** page. Missing rows enrich to
/// empty/0 — the star is still valid even if the archive has not (yet) seen the
/// page.
fn enrich_entity(
    connection: &Connection,
    kind: StarEntityKind,
    entity_key: &str,
) -> Result<(String, String, i64)> {
    match kind {
        StarEntityKind::Url => {
            let (title, visit_count) = enrich_url_star(connection, entity_key)?;
            let domain = registrable_domain_for_url(entity_key).unwrap_or_default();
            Ok((domain, title, visit_count))
        }
        StarEntityKind::Domain => {
            // The visit count for a source is the sum over its URLs. We cannot
            // index by registrable domain on `urls` cheaply, so this is a
            // bounded `LIKE` over the host suffix; acceptable because the
            // starred-domain set is tiny and the hub paginates. `SUM` always
            // returns one row, so this never takes a no-rows path.
            let pattern = format!("%//{entity_key}/%");
            let alt_pattern = format!("%.{entity_key}/%");
            let visit_count: i64 = connection
                .query_row(
                    "SELECT COALESCE(SUM(visit_count), 0) FROM urls
                     WHERE url LIKE ?1 OR url LIKE ?2",
                    params![pattern, alt_pattern],
                    |row| row.get::<_, i64>(0),
                )
                .context("enriching domain star")?;
            Ok((entity_key.to_string(), String::new(), visit_count))
        }
    }
}

/// Resolves the (title, summed visit_count) for a URL star keyed by its
/// `canonical_url`.
///
/// `urls.url` stores the RAW url, so an exact `url = canonical` join misses any
/// page whose stored visits carry tracking params or a non-normalized host
/// (the very variants the star key collapses). Two passes, both archive-bounded:
///
/// 1. **Exact seek** on `idx_urls_url`. The common case — the stored url already
///    equals the canonical form — resolves in one index probe.
/// 2. **Prefix range scan** on `idx_urls_url` keyed by the canonical page's
///    `scheme://host/path` (no leading wildcard, so the index range applies),
///    then canonicalize each candidate raw url in Rust and SUM only those whose
///    canonical form equals the key. The candidate window is one page's worth of
///    raw variants, never the whole archive. The title is taken from the
///    most-recently-visited matching row.
///
/// Returns `("", 0)` when the archive has not (yet) seen the page — the star is
/// still valid; the hub just shows no title/count yet.
fn enrich_url_star(connection: &Connection, canonical_key: &str) -> Result<(String, i64)> {
    // Pass 1: exact match on the canonical key (index seek). Aggregates always
    // return exactly one row; an empty match yields (NULL, 0).
    let (exact_title, exact_count): (Option<String>, i64) = connection
        .query_row(
            "SELECT
               (SELECT title FROM urls WHERE url = ?1 ORDER BY last_visit_ms DESC LIMIT 1),
               COALESCE(SUM(visit_count), 0)
             FROM urls WHERE url = ?1",
            params![canonical_key],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
        )
        .context("enriching url star (exact)")?;
    if exact_count > 0 || exact_title.is_some() {
        return Ok((exact_title.unwrap_or_default(), exact_count));
    }

    // Pass 2: prefix range scan, then canonicalize candidates in Rust. Without a
    // prefix the LIKE would scan all of `urls`; anchoring on scheme://host/path
    // keeps the planner on the `idx_urls_url` range. When the key cannot be
    // parsed into a prefix (it always should — it came from `normalize_visit_url`
    // — but be defensive) fall back to the exact result.
    let Some(prefix) = canonical_prefix(canonical_key) else {
        return Ok((exact_title.unwrap_or_default(), exact_count));
    };
    let like = format!("{}%", escape_like(&prefix));
    let mut statement = connection.prepare(
        "SELECT url, COALESCE(title, ''), visit_count, last_visit_ms
         FROM urls WHERE url LIKE ?1 ESCAPE '\\' ORDER BY last_visit_ms DESC",
    )?;
    let mut rows = statement.query(params![like])?;
    let mut total: i64 = 0;
    let mut best_title = String::new();
    let mut have_title = false;
    while let Some(row) = rows.next()? {
        let raw_url: String = row.get(0)?;
        let title: String = row.get(1)?;
        let visit_count: i64 = row.get(2)?;
        // Only the candidates that actually canonicalize to the star key count;
        // the prefix can over-match (e.g. /foo vs /foobar share /foo).
        let matches = normalize_visit_url(&raw_url)
            .map(|normalized| normalized.canonical_url == canonical_key)
            .unwrap_or(false);
        if !matches {
            continue;
        }
        total += visit_count;
        // Rows arrive most-recent-first, so the first matching non-empty title
        // is the freshest one.
        if !have_title && !title.is_empty() {
            best_title = title;
            have_title = true;
        }
    }
    Ok((best_title, total))
}

/// Builds the `scheme://host/path` prefix of a canonical url for an index-range
/// `LIKE`. Drops the query string so every tracking-param variant of the same
/// page falls under the prefix. Returns `None` when the url cannot be parsed.
fn canonical_prefix(canonical_url: &str) -> Option<String> {
    let parsed = Url::parse(canonical_url).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
    Some(format!("{}://{}{}{}", parsed.scheme(), host, port, parsed.path()))
}

/// Escapes SQLite `LIKE` wildcards (`%`, `_`) and the escape char in a literal
/// prefix so a path containing them matches literally, not as a wildcard.
fn escape_like(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Parses the stored `entity_kind` text back into the enum. Unknown values
/// (e.g. a future `query_family`) are dropped from the MVP surfaces rather
/// than erroring, so a newer build's stars never crash an older read path.
fn parse_kind(text: &str) -> Option<StarEntityKind> {
    match text {
        "url" => Some(StarEntityKind::Url),
        "domain" => Some(StarEntityKind::Domain),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
