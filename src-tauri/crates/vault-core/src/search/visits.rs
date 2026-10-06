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

/// Stars are resolved by canonical keys before ranking, never after taking a candidate page.
pub(crate) fn starred_pages(vault: &Vault, ids: &[u64]) -> anyhow::Result<Vec<u64>> {
    let mut result = Vec::new();
    for ids in ids.chunks(1024) {
        let ids = ids.iter().map(|id| *id as i64).collect::<Vec<_>>();
        result.extend(vault.store().read(Db::Archive,move|s| {
            s.query(&format!("SELECT p.id FROM pages p WHERE p.id IN ({}) AND EXISTS(SELECT 1 FROM star st WHERE (st.entity_kind='url' AND st.entity_key=p.canonical_url) OR (st.entity_kind='domain' AND st.entity_key=p.registrable_domain))",vec!["?";ids.len()].join(",")),ids,|r|Ok(r.get::<i64>(0)? as u64))
        })?);
    }
    Ok(result)
}
