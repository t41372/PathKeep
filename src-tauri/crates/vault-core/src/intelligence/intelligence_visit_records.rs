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
pub(super) fn load_visible_visits(
    connection: &Connection,
    profile_id: Option<&str>,
    limit: Option<u32>,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
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
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    let rows = statement.query_map([profile_id], visit_from_row)?;
    let mut visits = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    hydrate_search_terms(connection, &mut visits)?;
    Ok(trim_visits_to_limit(visits, limit))
}

fn trim_visits_to_limit(mut visits: Vec<VisitRecord>, limit: Option<u32>) -> Vec<VisitRecord> {
    if let Some(limit) = limit {
        let keep = limit.max(1) as usize;
        if visits.len() > keep {
            visits = visits.split_off(visits.len() - keep);
        }
    }
    visits
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
    use super::{VisitRecord, trim_visits_to_limit};

    fn visit(visit_id: i64) -> VisitRecord {
        VisitRecord {
            visit_id,
            profile_id: "chrome:Default".to_string(),
            source_profile_id: 1,
            source_visit_id: visit_id,
            source_url_id: visit_id,
            url: format!("https://example.com/{visit_id}"),
            title: None,
            visit_time_ms: visit_id,
            from_visit: None,
            transition_type: None,
            external_referrer_url: None,
            canonical_url: format!("https://example.com/{visit_id}"),
            registrable_domain: "example.com".to_string(),
            domain_category: "reference".to_string(),
            page_category: "article".to_string(),
            search_engine: None,
            search_query: None,
            is_new_domain: false,
            is_search_event: false,
            evidence_tier: "deterministic".to_string(),
            taxonomy_source: "rules".to_string(),
            taxonomy_pack: None,
            taxonomy_version: None,
            display_name: None,
            session_id: None,
            trail_id: None,
        }
    }

    #[test]
    fn trim_visits_to_limit_keeps_the_newest_tail_and_treats_zero_as_one() {
        let visits = vec![visit(1), visit(2), visit(3)];

        let limited = trim_visits_to_limit(visits.clone(), Some(2));
        assert_eq!(limited.iter().map(|visit| visit.visit_id).collect::<Vec<_>>(), vec![2, 3]);

        let zero = trim_visits_to_limit(visits.clone(), Some(0));
        assert_eq!(zero.iter().map(|visit| visit.visit_id).collect::<Vec<_>>(), vec![3]);

        let unlimited = trim_visits_to_limit(visits, None);
        assert_eq!(unlimited.len(), 3);
    }
}
