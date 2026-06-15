//! Canonical visit-loading and site-dictionary application helpers.
//!
//! ## Responsibilities
//! - Load visible archive visits into the normalized `VisitRecord` shape used
//!   by rebuilds and route-level reads.
//! - Hydrate search terms and apply site-dictionary classification once per
//!   loaded batch.
//! - Persist visit-derived facts and maintain profile-scoped "new domain"
//!   bookkeeping for incremental rebuilds.
//!
//! ## Not responsible for
//! - Deciding whether a rebuild stage should run incrementally or fall back.
//! - Computing daily rollups, sessions, trails, or other structural aggregates.
//! - Route-specific SQL for `/intelligence` read models.
//!
//! ## Dependencies
//! - Parent-module `VisitRecord` contract and site-dictionary loaders.
//! - Canonical archive `visits`, `urls`, and `search_terms` tables.
//! - Shared local-day helper for dirty-date tracking.
//!
//! ## Performance notes
//! - Search-term hydration batches URL ids in chunks of 400 to avoid oversized
//!   `IN (...)` clauses.
//! - Persistence stays inside one transaction per batch so large rebuilds do
//!   not pay per-row commit overhead.

use super::{
    SiteDictionaryEntry, VisitRecord, classify_visit, load_enabled_search_engine_rules,
    load_site_dictionary_overrides, local_date_key, normalize_query,
};
use anyhow::Result;
use rusqlite::{Connection, Row, params};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Loads visible visits for one optional profile scope and applies the
/// site-dictionary classification contract before callers build higher-level
/// deterministic state.
///
/// `limit` keeps only the newest visits and is pushed into SQL so a bounded
/// scoped read never materializes a full profile (millions of rows) just to
/// trim the tail afterwards. Rows are still returned in ascending
/// `(visit_time_ms, id)` order so downstream deterministic builders see the
/// same sequence regardless of the limit. `None` performs an unbounded load,
/// which is only used by the full-recompute fallback path that must visit every
/// row anyway.
pub(super) fn load_visible_visits(
    connection: &Connection,
    profile_id: Option<&str>,
    limit: Option<u32>,
) -> Result<Vec<VisitRecord>> {
    // Fetch the newest rows first so `LIMIT` bounds the scan to the requested
    // tail, then reverse into ascending order below to match the contract.
    let order = match limit {
        Some(_) => "DESC",
        None => "ASC",
    };
    let mut statement = connection.prepare(&format!(
        "SELECT visits.id,
                source_profiles.profile_key,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
           AND (?1 IS NULL OR source_profiles.profile_key = ?1)
         ORDER BY visits.visit_time_ms {order}, visits.id {order}
         LIMIT ?2",
    ))?;
    // `Some(0)` keeps the single newest visit to preserve the historical
    // tail-trim contract; `None` uses -1 so SQLite treats it as no limit.
    let sql_limit = match limit {
        Some(limit) => i64::from(limit.max(1)),
        None => -1,
    };
    let rows =
        statement.query_map(params![profile_id, sql_limit], visit_from_row)?;
    let mut visits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    if limit.is_some() {
        // Restore ascending `(visit_time_ms, id)` order after the DESC fetch.
        visits.reverse();
    }
    hydrate_search_terms(connection, &mut visits)?;
    Ok(visits)
}

/// Normalizes one archive row into the unclassified `VisitRecord` shell used
/// by both route reads and rebuild pipelines.
pub(super) fn visit_from_row(row: &Row<'_>) -> rusqlite::Result<VisitRecord> {
    Ok(VisitRecord {
        visit_id: row.get(0)?,
        profile_id: row.get(1)?,
        source_profile_id: row.get(2)?,
        source_visit_id: row.get(3)?,
        source_url_id: row.get(4)?,
        url: row.get(5)?,
        title: row.get(6)?,
        visit_time_ms: row.get(7)?,
        from_visit: row.get(8)?,
        transition_type: row.get(9)?,
        external_referrer_url: row.get(10)?,
        canonical_url: String::new(),
        registrable_domain: String::new(),
        domain_category: "unknown".to_string(),
        page_category: "unknown".to_string(),
        search_engine: None,
        search_query: None,
        is_new_domain: false,
        is_search_event: false,
        evidence_tier: "tier-c".to_string(),
        taxonomy_source: "unknown".to_string(),
        taxonomy_pack: None,
        taxonomy_version: None,
        display_name: None,
        session_id: None,
        trail_id: None,
    })
}

/// Loads the set of already-known domains for one profile so incremental
/// rebuilds can preserve first-seen semantics across batches.
pub(super) fn load_seen_domains(
    connection: &Connection,
    profile_id: &str,
) -> Result<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT registrable_domain
         FROM visit_derived_facts
         WHERE profile_id = ?1",
    )?;
    statement
        .query_map([profile_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<HashSet<_>>>()
        .map_err(Into::into)
}

/// Marks batch-local visits as first-seen domains relative to the previously
/// persisted profile state.
pub(super) fn compute_is_new_domain_with_seen(
    visits: &mut [VisitRecord],
    seen_domains: &mut HashSet<String>,
) {
    for visit in visits {
        visit.is_new_domain = seen_domains.insert(visit.registrable_domain.clone());
    }
}

/// Returns the distinct local day keys touched by one visit batch so later
/// stages can bound their dirty-window work.
pub(super) fn unique_date_keys(visits: &[VisitRecord]) -> Vec<String> {
    visits
        .iter()
        .map(|visit| local_date_key(visit.visit_time_ms))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Upserts visit-derived facts for one batch after classification has already
/// filled every deterministic visit field.
pub(super) fn persist_visit_derived_facts(
    connection: &Connection,
    visits: &[VisitRecord],
    computed_at: &str,
) -> Result<()> {
    let tx = connection.unchecked_transaction()?;
    let mut statement = tx.prepare(
        "INSERT INTO visit_derived_facts (
           visit_id, profile_id, session_id, trail_id, registrable_domain, canonical_url,
           domain_category, page_category, search_engine, search_query, is_new_domain,
           is_search_event, evidence_tier, taxonomy_source, taxonomy_pack, taxonomy_version,
           computed_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(visit_id) DO UPDATE SET
           profile_id = excluded.profile_id,
           session_id = excluded.session_id,
           trail_id = excluded.trail_id,
           registrable_domain = excluded.registrable_domain,
           canonical_url = excluded.canonical_url,
           domain_category = excluded.domain_category,
           page_category = excluded.page_category,
           search_engine = excluded.search_engine,
           search_query = excluded.search_query,
           is_new_domain = excluded.is_new_domain,
           is_search_event = excluded.is_search_event,
           evidence_tier = excluded.evidence_tier,
           taxonomy_source = excluded.taxonomy_source,
           taxonomy_pack = excluded.taxonomy_pack,
           taxonomy_version = excluded.taxonomy_version,
           computed_at = excluded.computed_at",
    )?;
    for visit in visits {
        statement.execute(params![
            visit.visit_id,
            visit.profile_id,
            visit.session_id,
            visit.trail_id,
            visit.registrable_domain,
            visit.canonical_url,
            visit.domain_category,
            visit.page_category,
            visit.search_engine,
            visit.search_query,
            i64::from(visit.is_new_domain),
            i64::from(visit.is_search_event),
            visit.evidence_tier,
            visit.taxonomy_source,
            visit.taxonomy_pack,
            visit.taxonomy_version,
            computed_at,
        ])?;
    }
    drop(statement);
    tx.commit()?;
    Ok(())
}

/// Buckets visits by profile while preserving the archive order required by
/// later deterministic builders.
pub(super) fn build_profile_state(visits: Vec<VisitRecord>) -> BTreeMap<String, Vec<VisitRecord>> {
    let mut profiles = BTreeMap::<String, Vec<VisitRecord>>::new();
    for visit in visits {
        profiles.entry(visit.profile_id.clone()).or_default().push(visit);
    }
    profiles
}

/// Marks the first occurrence of each domain inside an in-memory profile slice.
pub(super) fn compute_is_new_domain(visits: &mut [VisitRecord]) {
    let mut seen = HashSet::<String>::new();
    for visit in visits {
        if seen.insert(visit.registrable_domain.clone()) {
            visit.is_new_domain = true;
        }
    }
}

/// Hydrates normalized search terms for a visit batch before site-dictionary
/// classification fills the rest of the deterministic visit fields.
pub(super) fn hydrate_search_terms(
    connection: &Connection,
    visits: &mut [VisitRecord],
) -> Result<()> {
    let overrides = load_site_dictionary_overrides(connection)?;
    let search_rules = load_enabled_search_engine_rules(connection)?;
    let profile_url_ids =
        visits.iter().fold(HashMap::<String, HashSet<i64>>::new(), |mut acc, visit| {
            acc.entry(visit.profile_id.clone()).or_default().insert(visit.source_url_id);
            acc
        });
    let mut query_map = HashMap::<(String, i64), String>::new();
    for (profile_id, url_ids) in profile_url_ids {
        let ids = url_ids.into_iter().collect::<Vec<_>>();
        for chunk in ids.chunks(400) {
            let placeholders = std::iter::repeat_n("?", chunk.len()).collect::<Vec<_>>().join(", ");
            let sql = format!(
                "SELECT source_profiles.profile_key, search_terms.url_id, search_terms.normalized_term
                 FROM archive.search_terms AS search_terms
                 JOIN archive.source_profiles AS source_profiles
                   ON source_profiles.id = search_terms.source_profile_id
                 WHERE source_profiles.profile_key = ?1
                   AND search_terms.reverted_at IS NULL
                   AND search_terms.url_id IN ({placeholders})"
            );
            let mut statement = connection.prepare(&sql)?;
            let params = std::iter::once(&profile_id as &dyn rusqlite::ToSql)
                .chain(chunk.iter().map(|value| value as &dyn rusqlite::ToSql));
            let rows = statement.query_map(rusqlite::params_from_iter(params), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?))
            })?;
            for row in rows {
                let (profile_id, url_id, query) = row?;
                query_map.entry((profile_id, url_id)).or_insert(query);
            }
        }
    }

    for visit in visits {
        let query = query_map.get(&(visit.profile_id.clone(), visit.source_url_id)).cloned();
        let dictionary = classify_visit(
            &visit.url,
            visit.title.as_deref(),
            query.as_deref(),
            query.is_some(),
            visit.external_referrer_url.as_deref(),
            visit.from_visit,
            &overrides,
            &search_rules,
        );
        apply_site_dictionary(visit, query, dictionary);
    }
    Ok(())
}

fn apply_site_dictionary(
    visit: &mut VisitRecord,
    query: Option<String>,
    dictionary: SiteDictionaryEntry,
) {
    visit.canonical_url = dictionary.canonical_url;
    visit.registrable_domain = dictionary.registrable_domain;
    visit.domain_category = dictionary.domain_category;
    visit.page_category = dictionary.page_category;
    visit.search_engine = dictionary.search_engine;
    visit.search_query = query
        .filter(|value| !value.trim().is_empty())
        .map(|value| normalize_query(&value))
        .or(dictionary.search_query);
    visit.is_search_event = visit.search_query.is_some() && visit.search_engine.is_some();
    visit.evidence_tier = dictionary.evidence_tier;
    visit.taxonomy_source = dictionary.taxonomy_source;
    visit.taxonomy_pack = dictionary.taxonomy_pack;
    visit.taxonomy_version = dictionary.taxonomy_version;
    visit.display_name = dictionary.display_name;
}

#[cfg(test)]
mod tests {
    use super::{VisitRecord, load_visible_visits};
    use crate::archive::{open_archive_connection, open_intelligence_connection};
    use crate::config::project_paths_with_root;
    use crate::models::{AppConfig, ArchiveMode};
    use rusqlite::Connection;

    /// Inserts run id 1 so the URL/visit foreign keys on `created_by_run_id`
    /// resolve.
    fn seed_run(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO runs (
                    id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only
                 ) VALUES (1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0)",
                [],
            )
            .expect("insert run");
    }

    /// Inserts one archive profile so visits can resolve `profile_key`.
    fn seed_profile(connection: &Connection, id: i64, profile_key: &str) {
        connection
            .execute(
                "INSERT INTO source_profiles (
                    id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at
                 ) VALUES (?1, 'chrome', '1', ?2, '/tmp/profile', '2026-04-14T00:00:00Z', 1, ?2, '2026-04-14T00:00:00Z')",
                rusqlite::params![id, profile_key],
            )
            .expect("insert profile");
    }

    /// Inserts one URL + visit pair (and an optional search term) for a profile.
    #[allow(clippy::too_many_arguments)]
    fn seed_visit(
        connection: &Connection,
        source_profile_id: i64,
        visit_id: i64,
        url: &str,
        title: &str,
        visit_time_ms: i64,
        normalized_search_term: Option<&str>,
    ) {
        let url_id = visit_id + 100;
        connection
            .execute(
                "INSERT INTO urls (
                    id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                    source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
                 ) VALUES (?1, ?2, ?3, 1, 0, ?4, '2026-04-14T00:00:00Z', ?4, '2026-04-14T00:00:00Z', ?5, 1, ?6, 0, ?7, '2026-04-14T00:00:00Z')",
                rusqlite::params![url_id, url, title, visit_time_ms, source_profile_id, url_id + 1000, format!("hash-{visit_id}")],
            )
            .expect("insert url");
        connection
            .execute(
                "INSERT INTO visits (
                    id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                    source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
                 ) VALUES (?1, ?2, ?3, ?4, '2026-04-14T00:00:00Z', 1, 0, ?5, 1, NULL, 0, ?6, ?7, '2026-04-14T00:00:00Z')",
                rusqlite::params![visit_id, url_id, visit_id.to_string(), visit_time_ms, source_profile_id, format!("fingerprint-{visit_id}"), format!("visit-hash-{visit_id}")],
            )
            .expect("insert visit");
        if let Some(term) = normalized_search_term {
            connection
                .execute(
                    "INSERT INTO search_terms (
                        id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
                     ) VALUES (?1, ?2, ?3, ?3, ?4, 1, ?5)",
                    rusqlite::params![
                        visit_id + 10_000,
                        url_id,
                        term,
                        source_profile_id,
                        format!("chrome:Profile{source_profile_id}"),
                    ],
                )
                .expect("insert search term");
        }
    }

    /// Opens an intelligence connection (archive attached) seeded with two
    /// profiles. The default profile owns three visits in ascending time order
    /// (ids 1-3, the first a Google search); the second profile owns two.
    fn seeded_connection() -> (tempfile::TempDir, Connection) {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        seed_run(&archive);
        seed_profile(&archive, 1, "chrome:Profile1");
        seed_profile(&archive, 2, "chrome:Profile2");
        seed_visit(
            &archive,
            1,
            1,
            "https://www.google.com/search?q=sqlite+wal",
            "sqlite wal - Google Search",
            1_711_929_600_000,
            Some("sqlite wal"),
        );
        seed_visit(&archive, 1, 2, "https://example.com/two", "Two", 1_711_929_660_000, None);
        seed_visit(&archive, 1, 3, "https://example.com/three", "Three", 1_712_016_000_000, None);
        seed_visit(&archive, 2, 20, "https://work.example/a", "Work A", 1_712_000_000_000, None);
        seed_visit(&archive, 2, 21, "https://work.example/b", "Work B", 1_712_000_060_000, None);
        drop(archive);
        let connection = open_intelligence_connection(&paths, &config, None).expect("intelligence");
        (root, connection)
    }

    fn visit_ids(visits: &[VisitRecord]) -> Vec<i64> {
        visits.iter().map(|visit| visit.visit_id).collect()
    }

    #[test]
    fn load_visible_visits_unbounded_returns_every_profile_in_ascending_order() {
        let (_root, connection) = seeded_connection();

        let visits = load_visible_visits(&connection, None, None).expect("load all");

        // Ascending by (visit_time_ms, id) across both profiles. Visit 3 is the
        // chronologically newest even though profile 2's ids are higher, proving
        // ordering keys on time rather than id.
        assert_eq!(visit_ids(&visits), vec![1, 2, 20, 21, 3]);
    }

    #[test]
    fn load_visible_visits_scopes_to_one_profile() {
        let (_root, connection) = seeded_connection();

        let visits =
            load_visible_visits(&connection, Some("chrome:Profile1"), None).expect("scoped load");

        assert_eq!(visit_ids(&visits), vec![1, 2, 3]);
        assert!(visits.iter().all(|visit| visit.profile_id == "chrome:Profile1"));
    }

    #[test]
    fn load_visible_visits_limit_keeps_the_newest_tail_in_ascending_order() {
        let (_root, connection) = seeded_connection();

        // Newest two of the scoped profile, still ascending after the DESC fetch.
        let limited = load_visible_visits(&connection, Some("chrome:Profile1"), Some(2))
            .expect("limited load");
        assert_eq!(visit_ids(&limited), vec![2, 3]);
    }

    #[test]
    fn load_visible_visits_limit_applies_to_unscoped_reads_across_profiles() {
        let (_root, connection) = seeded_connection();

        // No profile filter: the newest two rows globally by time are visit 21
        // (1_712_000_060_000) and visit 3 (1_712_016_000_000), returned
        // ascending after the DESC-limited fetch.
        let limited = load_visible_visits(&connection, None, Some(2)).expect("global limited load");
        assert_eq!(visit_ids(&limited), vec![21, 3]);
    }

    #[test]
    fn load_visible_visits_limit_zero_keeps_a_single_newest_visit() {
        let (_root, connection) = seeded_connection();

        // `Some(0)` preserves the historical keep-one tail-trim contract.
        let zero =
            load_visible_visits(&connection, Some("chrome:Profile1"), Some(0)).expect("zero load");
        assert_eq!(visit_ids(&zero), vec![3]);
    }

    #[test]
    fn load_visible_visits_limit_above_row_count_returns_all_rows() {
        let (_root, connection) = seeded_connection();

        let limited = load_visible_visits(&connection, Some("chrome:Profile1"), Some(999))
            .expect("oversized limit");
        assert_eq!(visit_ids(&limited), vec![1, 2, 3]);
    }

    #[test]
    fn load_visible_visits_hydrates_search_terms_within_the_limit_window() {
        let (_root, connection) = seeded_connection();

        // Visit 1 is the Google search; it must classify as a search event when
        // it falls inside the bounded window, proving hydration runs on the
        // trimmed-in-SQL set rather than the full table.
        let limited = load_visible_visits(&connection, Some("chrome:Profile1"), Some(3))
            .expect("limited load");
        let search_visit =
            limited.iter().find(|visit| visit.visit_id == 1).expect("search visit present");
        assert_eq!(search_visit.search_query.as_deref(), Some("sqlite wal"));
        assert!(search_visit.is_search_event);
    }
}
