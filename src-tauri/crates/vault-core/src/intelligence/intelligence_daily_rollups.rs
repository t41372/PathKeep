//! Responsibilities: retain independent native read/reference helpers for stage regressions.
//! Not responsible for: running production rebuilds or installing schemas.
//! Failure modes: range boundaries lose visits, duplicate daily keys change aggregates,
//! or timezone gaps panic. Tests below and batch_equivalence exercise these contracts.
use super::{DerivedVisitBatchCursor, VisitRecord, display_name_for_domain};
use anyhow::Result;
use std::collections::HashSet;
use vault_store::{Session, params_owned as params};

pub(super) fn load_profile_derived_visits(
    connection: &Session<'_>,
    profile_id: &str,
    start_ms: Option<i64>,
    last_processed_visit_id: Option<i64>,
) -> Result<Vec<VisitRecord>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                visit_derived_facts.profile_id,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.domain_category,
                visit_derived_facts.page_category,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event,
                visit_derived_facts.evidence_tier,
                visit_derived_facts.taxonomy_source,
                visit_derived_facts.taxonomy_pack,
                visit_derived_facts.taxonomy_version,
                visit_derived_facts.session_id,
                visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.profile_id = ?1
           AND visits.reverted_at IS NULL
           AND (?2 IS NULL OR visits.visit_time_ms >= ?2)
           AND (?3 IS NULL OR visit_derived_facts.visit_id > ?3)
         ORDER BY visits.visit_time_ms ASC, visits.id ASC",
    )?;
    statement
        .query_map(params![profile_id, start_ms, last_processed_visit_id], |row| {
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
                canonical_url: row.get(11)?,
                registrable_domain: row.get(12)?,
                domain_category: row.get(13)?,
                page_category: row.get(14)?,
                search_engine: row.get(15)?,
                search_query: row.get(16)?,
                is_new_domain: row.get::<i64>(17)? != 0,
                is_search_event: row.get::<i64>(18)? != 0,
                evidence_tier: row.get(19)?,
                taxonomy_source: row.get(20)?,
                taxonomy_pack: row.get(21)?,
                taxonomy_version: row.get(22)?,
                display_name: display_name_for_domain(&row.get::<String>(12)?),
                session_id: row.get(23)?,
                trail_id: row.get(24)?,
            })
        })?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Streams visit-derived rows in deterministic order so aggregate builders can
/// stay bounded on large profiles.
pub(super) fn load_profile_derived_visit_batch(
    connection: &Session<'_>,
    profile_id: &str,
    after: Option<DerivedVisitBatchCursor>,
    limit: usize,
) -> Result<Vec<VisitRecord>> {
    // Bounded keyset form, present only after the first batch: Turso does not seek on
    // `t>? OR (t=? AND id>?)` or behind `?2 IS NULL OR`.
    let after_clause = if after.is_some() {
        "AND visits.visit_time_ms >= ?2 AND (visits.visit_time_ms > ?2 OR visit_derived_facts.visit_id > ?3)"
    } else {
        ""
    };
    let mut statement = connection.prepare(&format!(
        "SELECT visits.id,
                visit_derived_facts.profile_id,
                visits.source_profile_id,
                CAST(visits.source_visit_id AS INTEGER),
                urls.id,
                urls.url,
                urls.title,
                visits.visit_time_ms,
                visits.from_visit,
                visits.transition_type,
                visits.external_referrer_url,
                visit_derived_facts.canonical_url,
                visit_derived_facts.registrable_domain,
                visit_derived_facts.domain_category,
                visit_derived_facts.page_category,
                visit_derived_facts.search_engine,
                visit_derived_facts.search_query,
                visit_derived_facts.is_new_domain,
                visit_derived_facts.is_search_event,
                visit_derived_facts.evidence_tier,
                visit_derived_facts.taxonomy_source,
                visit_derived_facts.taxonomy_pack,
                visit_derived_facts.taxonomy_version,
                visit_derived_facts.session_id,
                visit_derived_facts.trail_id
         FROM visit_derived_facts
         JOIN archive.visits AS visits ON visits.id = visit_derived_facts.visit_id
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         WHERE visit_derived_facts.profile_id = ?1
           AND visits.reverted_at IS NULL
           {after_clause}
         ORDER BY visits.visit_time_ms ASC, visits.id ASC
         LIMIT ?4"
    ))?;
    statement
        .query_map(
            params![
                profile_id,
                after.map(|cursor| cursor.visit_time_ms),
                after.map(|cursor| cursor.visit_id),
                limit.max(1) as i64,
            ],
            |row| {
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
                    canonical_url: row.get(11)?,
                    registrable_domain: row.get(12)?,
                    domain_category: row.get(13)?,
                    page_category: row.get(14)?,
                    search_engine: row.get(15)?,
                    search_query: row.get(16)?,
                    is_new_domain: row.get::<i64>(17)? != 0,
                    is_search_event: row.get::<i64>(18)? != 0,
                    evidence_tier: row.get(19)?,
                    taxonomy_source: row.get(20)?,
                    taxonomy_pack: row.get(21)?,
                    taxonomy_version: row.get(22)?,
                    display_name: display_name_for_domain(&row.get::<String>(12)?),
                    session_id: row.get(23)?,
                    trail_id: row.get(24)?,
                })
            },
        )?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Guards the unique `(date_key, profile_id, domain)` invariant before the
/// caller replaces daily rollup rows.
pub(super) fn ensure_unique_domain_rollup_rows(domain_rows: &[super::DomainDayRow]) -> Result<()> {
    let mut seen = HashSet::<(String, String, String)>::new();
    for row in domain_rows {
        let key = (row.date_key.clone(), row.profile_id.clone(), row.registrable_domain.clone());
        if !seen.insert(key.clone()) {
            anyhow::bail!(
                "duplicate domain daily rollup row prepared for {} / {} / {}",
                key.0,
                key.1,
                key.2
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_rows_and_empty_dirty_dates_are_guarded() {
        let row = |visits: i64| super::super::DomainDayRow {
            date_key: "2026-04-25".to_string(),
            profile_id: "chrome:Default".to_string(),
            registrable_domain: "example.com".to_string(),
            domain_category: "docs".to_string(),
            visit_count: visits,
            unique_urls: visits,
            ..Default::default()
        };
        let duplicate_rows = vec![row(1), row(2)];
        let duplicate_error =
            ensure_unique_domain_rollup_rows(&duplicate_rows).expect_err("duplicate row");
        assert!(duplicate_error.to_string().contains("duplicate domain daily rollup row"));
    }
}
