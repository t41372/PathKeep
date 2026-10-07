//! Responsibilities: read semantic freshness from the same open Turso owner as lexical recall.
//! Not responsible for: queue internals, embedding writes or opening database files.
//! Failure modes: search silently opens a legacy archive, or freshness uses hidden visits.
//! Tests: semantic search/freshness suites with real Vault fixtures.
use crate::{AiSemanticStaleness, Vault};
use vault_store::{Db, params};
pub(super) fn staleness(
    vault: &Vault,
    provider: &str,
    model: &str,
) -> anyhow::Result<Option<AiSemanticStaleness>> {
    let provider = provider.to_owned();
    let model = model.to_owned();
    let row=vault.store().read(Db::Intelligence,move|s| {
        let rows=s.query("SELECT COALESCE(source_watermark,0),last_indexed_at FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",params![provider,model],|r|Ok((r.get::<i64>(0)?,r.get::<Option<String>>(1)?)))?;
        Ok(rows.into_iter().next())
    })?;
    let Some((watermark, indexed_at)) = row else { return Ok(None) };
    if watermark != 0 {
        let visible = vault.store().read(Db::Archive, |s| {
            s.query_one(
                "SELECT COUNT(*),COALESCE(MAX(id),0) FROM visits WHERE reverted_at IS NULL",
                (),
                |r| Ok((r.get::<i64>(0)? << 32) ^ r.get::<i64>(1)?),
            )
        })?;
        if visible != watermark {
            return Ok(Some(AiSemanticStaleness::Watermark));
        }
    }
    if let Some(indexed_at) = indexed_at {
        let latest = vault.store().read(Db::Intelligence, |s| {
            s.query_one(
                "SELECT MAX(fetched_at) FROM page_content_enrichments WHERE fetch_status='success'",
                (),
                |r| Ok(r.get::<Option<String>>(0)?),
            )
        })?;
        if latest.as_ref().is_some_and(|at| *at > indexed_at) {
            return Ok(Some(AiSemanticStaleness::Enrichment));
        }
    }
    Ok(None)
}
