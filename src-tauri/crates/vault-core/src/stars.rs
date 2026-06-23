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
//!   falls back to a prefix-bounded RANGE SEEK on the same index — an explicit
//!   byte-range `url >= :prefix AND url < :prefix_upper` (NOT a `LIKE`, which the
//!   default `case_sensitive_like = OFF` would force into a full `SCAN urls`) —
//!   canonicalizing only the candidate rows that share the page's host+path, so
//!   nothing scans the 14.4M-row archive even when the stored visits carry
//!   tracking params. EXPLAIN QUERY PLAN confirms both passes `SEARCH urls`.
//! - Domain enrichment/resolution seeks the persisted `urls.registrable_domain`
//!   column (migration 015) — `WHERE registrable_domain = :domain` is a true
//!   `SEARCH urls USING INDEX idx_urls_registrable_domain`, replacing the old
//!   leading-wildcard host `LIKE` that full-`SCAN`ned `urls` once per star.
//! - `is_starred_batch` binds one parameter per visible key (bounded by the
//!   render window), so the IN-list never grows with the archive.
//! - `starred_history_ids` (the AI `is:starred` facet) resolves the tiny starred
//!   set FORWARD: starred URLs → `urls.id` via the `idx_urls_url` exact seek +
//!   prefix RANGE SEEK the enrichment uses; starred domains → `urls.id` via the
//!   `idx_urls_registrable_domain` seek; then `urls.id` → `visits.id` via a
//!   chunked `url_id IN (...)` predicate that rides `idx_visits_visible_url_time`.
//!   Every pass is a `SEARCH` (not a `SCAN`), bounded by the star set, never the
//!   14.4M visit archive — it does NOT scan the `visits ⋈ urls` join.

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

/// Chunk size for the `url_id IN (...)` predicate in [`visit_ids_for_url_ids`]. Kept well under
/// SQLite's default `SQLITE_MAX_VARIABLE_NUMBER` (999 on older builds) so the starred-visit resolution
/// never trips the bound-variable limit even if a domain star covers many `urls` rows.
const STAR_VISIT_CHUNK: usize = 500;

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

/// A pure, in-memory matcher over the (tiny) starred set used by AI search (W-AI-6).
///
/// Holds the canonicalized starred URL set + starred registrable-domain set so a hydrated search
/// result can be tested for starred-ness WITHOUT a per-result archive round-trip: the result's raw
/// URL is canonicalized the same way `set_star`/`is_starred_batch` canonicalize, then checked against
/// the two sets. A page is starred when its canonical URL is starred OR its registrable domain is
/// starred (a domain star covers every page on that source), mirroring the lexical `is:starred` facet.
/// Construct via [`load_starred_matcher`]; the set is bounded by the user's star count (hundreds), so
/// `is_starred` is O(1) and the whole matcher is cheap to clone into the result loop.
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

/// Loads the starred URL + domain sets into an in-memory [`StarredMatcher`] (W-AI-6).
///
/// One small read of the `star` table (tiny by design) — the bounded source for both the AI search
/// starred boost and the `is:starred` facet's content_key allowlist. Stored `entity_key`s are already
/// canonical (written through `canonicalize_key`), so they go straight into the matcher.
pub fn load_starred_matcher(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<StarredMatcher> {
    let connection = open_archive_connection(paths, config, key)?;
    load_starred_matcher_from(&connection)
}

/// Builds a [`StarredMatcher`] from an open archive connection (the testable inner read).
fn load_starred_matcher_from(connection: &Connection) -> Result<StarredMatcher> {
    let mut matcher = StarredMatcher::default();
    let mut statement = connection
        .prepare("SELECT entity_kind, entity_key FROM star")
        .context("preparing starred-set read")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let kind: String = row.get(0)?;
        let entity_key: String = row.get(1)?;
        match parse_kind(&kind) {
            Some(StarEntityKind::Url) => {
                matcher.canonical_urls.insert(entity_key);
            }
            Some(StarEntityKind::Domain) => {
                matcher.domains.insert(entity_key);
            }
            None => {}
        }
    }
    Ok(matcher)
}

/// Resolves the archive `visits.id` set whose page (or domain) is starred (W-AI-6 facet allowlist).
///
/// The `is:starred` facet needs the starred VISITS so AI search can map them to deduped content vectors
/// and restrict semantic recall to favorites. The work is bounded by the TINY starred set, never the
/// 14.4M visit archive: we resolve the starred set FORWARD to `urls.id` first, then read only the visits
/// for those ids — the inverse of the old "scan every visit, test each URL" pass (O(total) on the
/// interactive path). Concretely:
///
/// 1. **URL stars → `urls.id`** via the same disciplined two-pass strategy [`enrich_url_star`] uses: an
///    exact `idx_urls_url` seek on the canonical key, then a prefix-anchored RANGE SEEK
///    (`url >= 'scheme://host/path' AND url < upper`) confirming each candidate raw url canonicalizes
///    back to the key. Both passes ride `idx_urls_url` (SEARCH, not SCAN) and touch only one page's raw
///    variants.
/// 2. **Domain stars → `urls.id`** via an INDEX SEEK on the persisted `urls.registrable_domain` column
///    (`WHERE registrable_domain = :domain`, riding `idx_urls_registrable_domain`). The column stores
///    `registrable_domain_for_url(url)`, so the seek is EXACTLY equivalent to
///    [`StarredMatcher::is_starred`]'s domain test with no Rust re-check — bounded by the tiny
///    starred-domain set.
/// 3. **`urls.id` → `visits.id`** with a BOUNDED `url_id IN (...)` predicate (rides
///    `idx_visits_visible_url_time`, `reverted_at IS NULL`). The IN-list is chunked so it respects
///    SQLite's variable limit. Reverted visits are excluded so a facet never resolves a row the user
///    reverted.
///
/// Returns an empty set when nothing is starred (the caller then yields no facet hits).
///
/// NOTE on the mapping (per the W-AI-6 brief): the dedup `content_key` is `hash(canonical_url + title +
/// enrichment)`, which the canonical URL alone cannot reproduce (title/enrichment vary). So instead of
/// recomputing content keys, this resolves starred URLs → starred `visits.id` here, and the caller maps
/// those ids → `content_key`s through the authoritative `.pkmap` (the same visit→content adjacency the
/// hydration uses). The join is the source of truth, not a re-hash.
pub fn starred_history_ids(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    matcher: &StarredMatcher,
) -> Result<HashSet<i64>> {
    if matcher.is_empty() {
        return Ok(HashSet::new());
    }
    let connection = open_archive_connection(paths, config, key)?;
    let url_ids = starred_url_ids(&connection, matcher)?;
    visit_ids_for_url_ids(&connection, &url_ids)
}

/// Resolves the set of `urls.id` whose page (canonical URL) or domain is starred (the bounded forward
/// seek behind [`starred_history_ids`]).
///
/// Reuses the file's established index-bounded patterns so nothing scans the 14.4M-row archive: URL stars
/// resolve through [`url_ids_for_canonical`] (the [`enrich_url_star`] exact seek + prefix RANGE SEEK
/// strategy) and domain stars through [`url_ids_for_domain`] (an INDEX SEEK on the persisted
/// `registrable_domain` column, exactly equivalent to [`StarredMatcher::is_starred`]). The result is
/// bounded by the union of one page's raw variants per URL star plus the per-domain `urls` rows — both
/// tiny because the star set is tiny.
fn starred_url_ids(connection: &Connection, matcher: &StarredMatcher) -> Result<HashSet<i64>> {
    let mut url_ids = HashSet::new();
    for canonical in &matcher.canonical_urls {
        url_ids.extend(url_ids_for_canonical(connection, canonical)?);
    }
    for domain in &matcher.domains {
        url_ids.extend(url_ids_for_domain(connection, domain)?);
    }
    Ok(url_ids)
}

/// Collects the `urls.id`s whose raw url canonicalizes onto `canonical_key` (the URL-star forward seek).
///
/// Mirrors [`enrich_url_star`]'s two archive-bounded passes, but yields the matching `urls.id`s instead
/// of the title/visit-count aggregate:
/// 1. **Exact seek** on `idx_urls_url` for the common case where the stored row already equals the
///    canonical form.
/// 2. **Prefix RANGE SEEK** anchored on `scheme://host/path` via an explicit byte-range
///    (`url >= :prefix AND url < :prefix_upper`, so the BINARY `idx_urls_url` range applies — a `LIKE`
///    would full-`SCAN`), confirming each candidate raw url canonicalizes back to the key — the prefix
///    can over-match (`/foo` vs `/foobar`), so the Rust re-check keeps it honest. Both queries are added
///    so a page stored ONLY under a tracking-param variant still resolves (the very variants the star key
///    collapses).
///
/// RESIDUALS (documented, not fixed — both share the same root: a stored raw form that differs from the
/// canonical key in a way browsers never produce, where the tight per-page byte-range cannot anchor):
/// - **A-3, dot-segment paths**: a raw url whose stored PATH carries unresolved dot-segments
///   (e.g. `https://example.com/a/../b`, canonicalizing to `/b`) shares no `scheme://host/path` prefix
///   with the key. Browsers resolve `/../` before storing, so incidence is effectively zero.
/// - **Host casing (H-2 byte-range)**: the canonical prefix lowercases the host (`normalize_visit_url`),
///   but the byte-range is BINARY, so a stored MIXED-CASE host (`https://Example.com/post`) sorts before
///   the lowercase prefix and falls outside `[prefix, prefix_upper)`. The old case-insensitive `LIKE`
///   pre-filter caught it; the index-seekable range trades that for the bound. Browsers normalize hosts
///   to lowercase before storing (hosts are case-insensitive), so a stored mixed-case host is
///   effectively absent from real imports — same zero-incidence class as A-3.
///
/// In both cases the per-visit [`StarredMatcher::is_starred`] (which canonicalizes the full url) WOULD
/// match. The "fix" (a host-level or scheme-level prefix) would widen the per-page range toward an O(host)
/// or O(corpus) scan — the exact perf downside the bounded seek exists to avoid — so the residuals stand.
fn url_ids_for_canonical(connection: &Connection, canonical_key: &str) -> Result<Vec<i64>> {
    let mut ids = Vec::new();

    // Pass 1: exact match on the canonical key (index seek). The common case where the stored url
    // already equals the canonical form.
    let mut exact = connection.prepare("SELECT id FROM urls WHERE url = ?1")?;
    let mut rows = exact.query(params![canonical_key])?;
    while let Some(row) = rows.next()? {
        ids.push(row.get::<_, i64>(0)?);
    }

    // Pass 2: prefix RANGE SEEK on `idx_urls_url`, then canonicalize candidates in Rust. The explicit
    // byte-range `url >= :prefix AND url < :upper` (no LIKE) lets the BINARY index answer the range as a
    // true `SEARCH urls USING INDEX idx_urls_url`, where a case-insensitive `LIKE 'prefix%'` forces a
    // full `SCAN urls` (H-2). Without a parseable prefix (always parseable in production — the key came
    // from `normalize_visit_url` — but be defensive) the exact pass alone stands.
    let Some(prefix) = canonical_prefix(canonical_key) else {
        return Ok(ids);
    };
    let upper = prefix_upper_bound(&prefix);
    let mut scan = connection.prepare("SELECT id, url FROM urls WHERE url >= ?1 AND url < ?2")?;
    let mut rows = scan.query(params![prefix, TextBytes(&upper)])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let raw_url: String = row.get(1)?;
        // Only the candidates that actually canonicalize to the star key count; the prefix can
        // over-match (e.g. /foo vs /foobar share /foo).
        let matches = normalize_visit_url(&raw_url)
            .map(|normalized| normalized.canonical_url == canonical_key)
            .unwrap_or(false);
        if matches {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Collects the `urls.id`s on a starred registrable `domain` (the domain-star forward seek).
///
/// An INDEX SEEK on the persisted `urls.registrable_domain` column (migration 015): every `urls` row
/// stores `registrable_domain_for_url(url)` — the SAME function [`StarredMatcher::is_starred`]'s domain
/// arm tests — so `WHERE registrable_domain = :domain` is BOTH a true `SEARCH urls USING INDEX
/// idx_urls_registrable_domain` AND exactly equivalent to the per-visit matcher, with NO Rust re-check
/// needed (the column already IS the verdict). This replaces the old leading-wildcard host `LIKE`, which
/// forced a full `SCAN urls` once per starred domain (H-2) and needed a `registrable_domain_for_url`
/// re-check in Rust to undo its over-/under-recall. Because the column is the canonical verdict, the two
/// historical divergences a bare `LIKE` had are gone by construction:
/// - **Over-recall (A-1):** a domain embedded in a path/query on an unrelated host (e.g.
///   `https://news.other.com/x?img=//example.com/p.jpg`) has `registrable_domain = other.com`, so it is
///   simply not in the `example.com` index slot.
/// - **Ported under-recall (A-2):** a ported host (e.g. `https://example.com:8080/x`,
///   `http://localhost:3000/x`) classifies to its registrable domain like any other, so it is in the slot.
///
/// Bounded by the tiny starred-domain set. The empty-string sentinel rows (unclassifiable urls) sit in
/// their own index slot and never equal a real domain key.
fn url_ids_for_domain(connection: &Connection, domain: &str) -> Result<Vec<i64>> {
    let mut statement = connection.prepare("SELECT id FROM urls WHERE registrable_domain = ?1")?;
    let mut rows = statement.query(params![domain])?;
    let mut ids = Vec::new();
    while let Some(row) = rows.next()? {
        ids.push(row.get::<_, i64>(0)?);
    }
    Ok(ids)
}

/// Resolves the visible (`reverted_at IS NULL`) `visits.id`s for a bounded set of `urls.id`s (W-AI-6).
///
/// The final hop of [`starred_history_ids`]: a chunked `url_id IN (...)` predicate that rides
/// `idx_visits_visible_url_time`. The IN-list is split into [`STAR_VISIT_CHUNK`]-sized groups so it never
/// exceeds SQLite's bound-variable limit even if a domain star covers many pages, and the whole read is
/// bounded by the tiny starred `urls.id` set — never the 14.4M visit archive.
fn visit_ids_for_url_ids(connection: &Connection, url_ids: &HashSet<i64>) -> Result<HashSet<i64>> {
    let mut ids = HashSet::new();
    if url_ids.is_empty() {
        return Ok(ids);
    }
    let url_ids: Vec<i64> = url_ids.iter().copied().collect();
    for chunk in url_ids.chunks(STAR_VISIT_CHUNK) {
        let placeholders = std::iter::repeat_n("?", chunk.len()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id FROM visits WHERE reverted_at IS NULL AND url_id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql)?;
        let mut rows = statement.query(params_from_iter(chunk.iter()))?;
        while let Some(row) = rows.next()? {
            ids.insert(row.get::<_, i64>(0)?);
        }
    }
    Ok(ids)
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
            // The visit count for a source is the sum over its URLs. An INDEX SEEK on the persisted
            // `urls.registrable_domain` column (migration 015): each row stores
            // `registrable_domain_for_url(url)`, so `WHERE registrable_domain = :domain` is a true
            // `SEARCH urls USING INDEX idx_urls_registrable_domain` (not the old leading-wildcard host
            // `LIKE` full `SCAN urls` — H-2) and agrees EXACTLY with the `is:starred` facet's domain
            // resolution (`url_ids_for_domain`) and `StarredMatcher::is_starred`, because all three test
            // the same persisted verdict. No Rust re-check needed: rows whose registrable domain is NOT
            // the star key (incl. a domain embedded in a path/query on an unrelated host) are simply in a
            // different index slot, and ported hosts classify into this slot like any other.
            let visit_count: i64 = connection.query_row(
                "SELECT COALESCE(SUM(visit_count), 0) FROM urls WHERE registrable_domain = ?1",
                params![entity_key],
                |row| row.get(0),
            )?;
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
/// 2. **Prefix RANGE SEEK** on `idx_urls_url` keyed by the canonical page's
///    `scheme://host/path` via an explicit byte-range (`url >= :prefix AND
///    url < :prefix_upper`, so the BINARY index range applies — a `LIKE` would
///    full-`SCAN urls`), then canonicalize each candidate raw url in Rust and SUM
///    only those whose canonical form equals the key. The candidate window is one
///    page's worth of raw variants, never the whole archive. The title is taken
///    from the most-recently-visited matching row.
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

    // Pass 2: prefix RANGE SEEK on `idx_urls_url`, then canonicalize candidates in Rust. The explicit
    // byte-range `url >= :prefix AND url < :upper` (no LIKE) keeps the planner on the BINARY index range
    // (`SEARCH urls USING INDEX idx_urls_url`); a case-insensitive `LIKE 'prefix%'` would full-`SCAN urls`
    // (H-2). When the key cannot be parsed into a prefix (it always should — it came from
    // `normalize_visit_url` — but be defensive) fall back to the exact result.
    let Some(prefix) = canonical_prefix(canonical_key) else {
        return Ok((exact_title.unwrap_or_default(), exact_count));
    };
    let upper = prefix_upper_bound(&prefix);
    let mut statement = connection.prepare(
        "SELECT url, COALESCE(title, ''), visit_count, last_visit_ms
         FROM urls WHERE url >= ?1 AND url < ?2 ORDER BY last_visit_ms DESC",
    )?;
    let mut rows = statement.query(params![prefix, TextBytes(&upper)])?;
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
/// SEEK. Drops the query string so every tracking-param variant of the same
/// page falls under the prefix. Returns `None` when the url cannot be parsed.
fn canonical_prefix(canonical_url: &str) -> Option<String> {
    let parsed = Url::parse(canonical_url).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
    Some(format!("{}://{}{}{}", parsed.scheme(), host, port, parsed.path()))
}

/// Returns the EXCLUSIVE upper bound for a BINARY-collation prefix range seek:
/// the smallest byte string strictly greater than every string that starts with
/// `prefix`. This is the standard LIKE-optimization upper bound — increment the
/// last byte of the prefix — and it lets a BINARY index (`idx_urls_url`) answer
/// `url >= prefix AND url < upper` as a true index RANGE SEARCH, where a
/// case-insensitive `LIKE 'prefix%'` would force a full `SCAN urls` (the DB runs
/// with the default `case_sensitive_like = OFF`, so the planner cannot use the
/// BINARY index for the LIKE range — H-2).
///
/// INFALLIBLE by the caller's contract: `prefix` is always a non-empty
/// `canonical_prefix(...)` — a `scheme://host[:port]/path` string whose first
/// byte is the ASCII scheme. Because it is valid UTF-8 its last byte is an ASCII
/// byte, a UTF-8 lead byte (`<= 0xF4`), or a continuation byte (`<= 0xBF`) —
/// NEVER `0xFF` — so "increment the last byte" can never overflow and there is no
/// finite-bound edge to fall through. Returning [`Vec<u8>`] (not `String`) is
/// required: incrementing the final UTF-8 byte can yield a byte sequence that is
/// no longer valid UTF-8 (e.g. `…0xBF` -> `…0xC0`), which is a perfectly good
/// BINARY upper bound but not a representable `&str`. The caller binds the result
/// as TEXT (see [`TextBytes`]) so SQLite compares it against the `url` TEXT column
/// under BINARY collation; binding it as a BLOB would make `url < :upper` always
/// true (BLOB sorts after every TEXT value) and silently widen the seek to the
/// end of the index.
fn prefix_upper_bound(prefix: &str) -> Vec<u8> {
    debug_assert!(!prefix.is_empty(), "canonical_prefix never yields an empty prefix");
    let mut bytes = prefix.as_bytes().to_vec();
    // `prefix` is non-empty valid UTF-8, so there is a last byte and it is
    // `< 0xF5` (0xFF/0xFE are never valid UTF-8 bytes) — the increment can never
    // overflow, so this is total for every value the contract admits.
    let last = bytes.last_mut().expect("canonical_prefix never yields an empty prefix");
    *last += 1;
    bytes
}

/// Binds raw bytes to SQLite as a **TEXT** value (not a BLOB).
///
/// Exists solely so the BINARY prefix upper bound from [`prefix_upper_bound`]
/// (which may not be valid UTF-8) is compared against the `url` TEXT column as
/// TEXT. A `Vec<u8>` binds as a BLOB, and BLOB sorts after every TEXT storage
/// class, so `url < :blob_upper` would be true for every row — defeating the
/// bounded range seek the upper bound exists to create. SQLite TEXT values are
/// untyped byte strings (no UTF-8 validation on bind), so a non-UTF-8 upper bound
/// is a legal TEXT comparand.
struct TextBytes<'a>(&'a [u8]);

impl rusqlite::ToSql for TextBytes<'_> {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(rusqlite::types::ValueRef::Text(self.0)))
    }
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
