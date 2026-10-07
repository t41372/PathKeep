//! Responsibilities: carry fetched page content from the old per-visit table into
//! the page-keyed `page_content_enrichments`, verify it, and republish each page's
//! best enrichment into search; supply the summaries embedding hashes depend on.
//! Not responsible for: fetching content, queue execution or embedding generation.
//! Failure modes: old keys miss URL variants, an older fetch overwrites a newer one,
//! a row is silently dropped, the search text differs from what live fetching would
//! publish, or embedding hashes use a different summary than live indexing.
//! Tests: vault-convert-sqlcipher conversion acceptance.
use crate::Vault;
use anyhow::Result;
use std::collections::HashMap;
use vault_store::{CancelToken, Db, OptionalRow, Priority, Value, params};

/// One row of the old `visit_content_enrichments`, keyed by visit.
#[derive(Debug, Clone)]
pub struct LegacyEnrichment {
    pub history_id: i64,
    pub content_source: String,
    pub fetch_status: String,
    pub fetched_at: String,
    pub final_url: Option<String>,
    pub language: Option<String>,
    pub readable_title: Option<String>,
    pub readable_text_blob_path: Option<String>,
    pub readable_text_bytes: i64,
    pub text_hash: Option<String>,
    pub snippet_json: String,
    pub extraction_json: String,
    pub pipeline_version: String,
    pub extractor_version: Option<i64>,
    pub enrichment_summary: Option<String>,
    pub refetch_after: Option<String>,
    pub http_status: Option<i64>,
}

/// Writes one page of old rows under their page's canonical URL. Per page and
/// source the newest fetch wins; replaying a page changes nothing. Rows whose
/// visit has no URL any more (removed before conversion) are left out.
pub fn import_enrichments(vault: &Vault, rows: Vec<LegacyEnrichment>) -> Result<()> {
    let keys = canonical_keys(vault, &rows)?;
    vault.store().write(Db::Intelligence, Priority::Batch, move |s| {
        for row in rows {
            let Some(key) = keys.get(&row.history_id) else { continue };
            s.execute(
                "INSERT INTO page_content_enrichments(canonical_url,content_source,fetch_status,fetched_at,final_url,language,readable_title,readable_text_blob_path,readable_text_bytes,text_hash,snippet_json,extraction_json,pipeline_version,extractor_version,enrichment_summary,refetch_after,http_status)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
                 ON CONFLICT(canonical_url,content_source) DO UPDATE SET fetch_status=excluded.fetch_status,fetched_at=excluded.fetched_at,final_url=excluded.final_url,language=excluded.language,readable_title=excluded.readable_title,readable_text_blob_path=excluded.readable_text_blob_path,readable_text_bytes=excluded.readable_text_bytes,text_hash=excluded.text_hash,snippet_json=excluded.snippet_json,extraction_json=excluded.extraction_json,pipeline_version=excluded.pipeline_version,extractor_version=excluded.extractor_version,enrichment_summary=excluded.enrichment_summary,refetch_after=excluded.refetch_after,http_status=excluded.http_status
                 WHERE excluded.fetched_at>page_content_enrichments.fetched_at",
                vec![
                    Value::Text(key.clone()),
                    Value::Text(row.content_source),
                    Value::Text(row.fetch_status),
                    Value::Text(row.fetched_at),
                    row.final_url.map_or(Value::Null, Value::Text),
                    row.language.map_or(Value::Null, Value::Text),
                    row.readable_title.map_or(Value::Null, Value::Text),
                    row.readable_text_blob_path.map_or(Value::Null, Value::Text),
                    Value::Integer(row.readable_text_bytes),
                    row.text_hash.map_or(Value::Null, Value::Text),
                    Value::Text(row.snippet_json),
                    Value::Text(row.extraction_json),
                    Value::Text(row.pipeline_version),
                    row.extractor_version.map_or(Value::Null, Value::Integer),
                    row.enrichment_summary.map_or(Value::Null, Value::Text),
                    row.refetch_after.map_or(Value::Null, Value::Text),
                    row.http_status.map_or(Value::Null, Value::Integer),
                ],
            )?;
        }
        Ok(())
    })?;
    Ok(())
}

/// Fails unless every old row whose page still exists is represented by its
/// page's row for the same source, fetched at the same time or later.
pub fn verify_enrichments(vault: &Vault, rows: &[LegacyEnrichment]) -> Result<()> {
    let keys = canonical_keys(vault, rows)?;
    let wanted = rows
        .iter()
        .filter_map(|row| {
            keys.get(&row.history_id)
                .map(|key| (key.clone(), row.content_source.clone(), row.fetched_at.clone()))
        })
        .collect::<Vec<_>>();
    vault.store().read(Db::Intelligence, move |s| {
        for (key, source, fetched_at) in wanted {
            let stored = s
                .query_one(
                    "SELECT fetched_at FROM page_content_enrichments WHERE canonical_url=?1 AND content_source=?2",
                    params![key.as_str(), source.as_str()],
                    |r| Ok(r.get::<String>(0)?),
                )
                .optional()?;
            if stored.as_deref().is_none_or(|stored| stored < fetched_at.as_str()) {
                return Err(vault_store::Error::Operation(
                    anyhow::anyhow!(
                        "conversion-digest-mismatch: enrichment {source} for {key} was not carried over"
                    )
                    .into_boxed_dyn_error(),
                ));
            }
        }
        Ok(())
    })?;
    Ok(())
}

fn canonical_keys(vault: &Vault, rows: &[LegacyEnrichment]) -> Result<HashMap<i64, String>> {
    let ids = rows.iter().map(|row| row.history_id).collect();
    Ok(visit_urls(vault, ids)?
        .into_iter()
        .map(|(visit, url)| (visit, super::canonical_url_key(&url)))
        .collect())
}

/// Publishes each page's best enrichment (the order live reads use) into its
/// search document. Paged by canonical URL; running it again changes nothing.
pub fn project(vault: &Vault, cancel: &CancelToken) -> Result<()> {
    let mut cursor = String::new();
    loop {
        cancel.check()?;
        let after = cursor.clone();
        let best = vault.store().read(Db::Intelligence, move |s| {
            let keys = s.query(
                "SELECT DISTINCT canonical_url FROM page_content_enrichments WHERE fetch_status='success' AND canonical_url>?1 ORDER BY canonical_url LIMIT 256",
                [after],
                |r| Ok(r.get::<String>(0)?),
            )?;
            let sql = format!(
                "SELECT enrichment_summary,extraction_json FROM page_content_enrichments WHERE canonical_url=?1 AND fetch_status='success' ORDER BY {} LIMIT 1",
                crate::enrichment::BEST_ENRICHMENT_ORDER
            );
            let mut best = Vec::with_capacity(keys.len());
            for key in keys {
                let (summary, metadata) = s.query_one(&sql, [key.as_str()], |r| {
                    Ok((r.get::<Option<String>>(0)?, r.get::<String>(1)?))
                })?;
                let text =
                    crate::archive::enrichment_text_for_index(summary.as_deref(), Some(&metadata));
                best.push((key, text));
            }
            Ok(best)
        })?;
        let Some((last, _)) = best.last() else { break };
        cursor = last.clone();
        for (key, text) in best {
            let pages =
                vault.store().read(Db::Archive, move |s| crate::pages::ids_for_key(s, &key))?;
            crate::enrichment::search_text::publish(vault, pages, text)?;
        }
    }
    Ok(())
}

/// The summary each visit's embedding hash is built from: its page's best
/// enrichment, exactly as live indexing reads it.
pub fn embedding_summaries(vault: &Vault, history_ids: Vec<i64>) -> Result<HashMap<i64, String>> {
    let paths = vault.paths().clone();
    Ok(vault.store().read(Db::Intelligence, move |s| {
        let best =
            crate::enrichment::load_best_enrichment_map_by_history_ids(&paths, s, &history_ids)
                .map_err(|e| vault_store::Error::Operation(e.into_boxed_dyn_error()))?;
        Ok(best
            .into_iter()
            .filter_map(|(id, stored)| {
                crate::ai::enrichment_summary_for_embedding(Some(&stored))
                    .map(|s| (id, s.to_owned()))
            })
            .collect())
    })?)
}

/// Embedding totals are kept by the indexer as it writes; converted rows arrive
/// without it, so the totals are recounted once all of them are in.
pub fn refresh_embedding_totals(vault: &Vault) -> Result<()> {
    vault.store().write(Db::Intelligence, Priority::Batch, |s| {
        s.execute("DELETE FROM ai_embedding_totals", ())?;
        s.execute(
            "INSERT INTO ai_embedding_totals(provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at)
             SELECT provider_id,model,COUNT(*),SUM(content_bytes),SUM(metadata_bytes),MAX(indexed_at) FROM ai_embeddings GROUP BY provider_id,model",
            (),
        )?;
        Ok(())
    })?;
    Ok(())
}

/// Conversion's visit-dependent keys derive from verified raw URL facts in bulk.
pub fn visit_urls(vault: &Vault, ids: Vec<i64>) -> Result<HashMap<i64, String>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(vault.store().read(Db::Archive, move |s| {
        let binds = (1..=ids.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
        let rows = s.query(
            &format!("SELECT v.id,u.url FROM visits v JOIN urls u ON u.id=v.url_id WHERE v.id IN ({binds})"),
            ids, |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?)),
        )?;
        Ok(rows.into_iter().collect())
    })?)
}
