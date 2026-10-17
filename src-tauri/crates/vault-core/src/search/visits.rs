//! Responsibilities: canonical keyed hydration for AI, code mode and regex evidence.
//! Not responsible for: candidate generation, recency windows or opening storage.
//! Failure modes: old IDs disappear, revoked visits leak, scopes mix different visits,
//! text source IDs fail conversion. Tests: keyed-visit and regex projection contracts.
use super::{SearchHit, SearchRequest, eligibility::Scope};
use crate::{HistoryEntry, Vault};
use vault_store::{Db, Session, Store, Value};

/// Fetches every requested visible identity in bounded SQL batches, including old visits.
pub fn visits(vault: &Vault, ids: &[i64]) -> anyhow::Result<Vec<HistoryEntry>> {
    Ok(evidence(vault, ids, &SearchRequest::default())?.into_iter().map(|hit| hit.visit).collect())
}
/// Semantic candidates use the same canonical same-visit scope as lexical retrieval.
pub(crate) fn evidence(
    vault: &Vault,
    ids: &[i64],
    request: &SearchRequest,
) -> anyhow::Result<Vec<SearchHit>> {
    let query = super::query::build(&request.query, false);
    let scope = Scope::new(
        &SearchRequest { starred: request.starred || query.starred, ..request.clone() },
        &query.filters,
    );
    evidence_from(vault.store(), ids, &scope)
}
pub(super) fn evidence_from(
    store: &Store,
    ids: &[i64],
    scope: &Scope,
) -> anyhow::Result<Vec<SearchHit>> {
    let mut result = Vec::new();
    for ids in ids.chunks(512) {
        let ids = ids.to_vec();
        let scope = scope.clone();
        result.extend(store.read(Db::Archive, move |s| rows(s, &ids, &scope))?);
    }
    Ok(result)
}
fn rows(s: &Session<'_>, ids: &[i64], scope: &Scope) -> vault_store::Result<Vec<SearchHit>> {
    let mut params = scope.params.clone();
    let placeholders = ids
        .iter()
        .map(|id| {
            params.push(Value::Integer(*id));
            format!("?{}", params.len())
        })
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT v.id,sp.profile_key,p.url,u.title,v.visit_time_ms,
        v.visit_duration_ms,v.transition_type,v.source_visit_id,v.app_id,d.enrichment_text,p.id
        FROM visits v CROSS JOIN urls u ON u.id=v.url_id CROSS JOIN pages p ON p.id=u.page_id
        JOIN source_profiles sp ON sp.id=v.source_profile_id
        LEFT JOIN search_documents d ON d.page_id=p.id
        WHERE v.id IN ({placeholders}) AND {} ORDER BY v.id",
        scope.sql
    );
    s.query(&sql, params, |row| {
        let url = row.get::<String>(2)?;
        let time = row.get::<i64>(4)?;
        Ok(SearchHit {
            reason: None,
            page_id: row.get(10)?,
            score: 0.,
            visit: HistoryEntry {
                id: row.get(0)?,
                profile_id: row.get::<Option<String>>(1)?.unwrap_or_default(),
                domain: crate::utils::url_domain(&url),
                url,
                title: row.get(3)?,
                visit_time: time,
                visited_at: chrono::DateTime::from_timestamp_millis(time)
                    .map(|time| time.to_rfc3339())
                    .unwrap_or_default(),
                duration_ms: row.get(5)?,
                transition: row.get(6)?,
                source_visit_id: row
                    .get::<Option<String>>(7)?
                    .and_then(|id| id.parse().ok())
                    .unwrap_or(0),
                app_id: row.get(8)?,
                enrichment_excerpt: row
                    .get::<Option<String>>(9)?
                    .and_then(|text| crate::archive::cap_enrichment_excerpt(&text)),
                favicon: None,
                visit_count: None,
            },
        })
    })
}

/// Each branch seeks the page index from the star set, independent of result count.
pub(super) const STARRED_PAGES: &str = "SELECT p.id FROM star st CROSS JOIN pages p INDEXED BY idx_pages_canonical_url ON p.canonical_url=st.entity_key WHERE st.entity_kind='url'
UNION SELECT p.id FROM star st CROSS JOIN pages p INDEXED BY idx_pages_registrable_domain ON p.registrable_domain=st.entity_key WHERE st.entity_kind='domain' AND p.registrable_domain IS NOT NULL";
pub(crate) fn starred_pages(vault: &Vault) -> anyhow::Result<Vec<u64>> {
    Ok(vault
        .store()
        .read(Db::Archive, |s| s.query(STARRED_PAGES, (), |r| Ok(r.get::<i64>(0)? as u64)))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starred_page_union_seeks_both_page_indexes() {
        let vault = Vault::open_temp().unwrap();
        let plan = vault
            .store()
            .read(Db::Archive, |s| {
                s.query(&format!("EXPLAIN QUERY PLAN {STARRED_PAGES}"), (), |r| {
                    Ok(r.get::<String>(3)?)
                })
            })
            .unwrap()
            .join("\n");
        for index in ["idx_pages_canonical_url", "idx_pages_registrable_domain"] {
            assert!(
                plan.lines().any(|line| line.contains("SEARCH p") && line.contains(index)),
                "{plan}"
            );
        }
        assert!(!plan.contains("SCAN p"), "{plan}");
    }
}
