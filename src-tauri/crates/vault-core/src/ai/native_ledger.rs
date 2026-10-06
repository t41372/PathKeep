//! Responsibilities: read and update native semantic metadata and archive run state.
//! Not responsible for: vectors, model calls or search candidate selection.
//! Failure modes: status scans the archive, provider counts mix models, or the
//! watermark ignores visibility. Native queue/index tests exercise this boundary.

use super::*;
use vault_store::{OptionalRow, Session, params_owned as params};

/// One persisted semantic-index ledger row for the selected provider/model pair.
///
/// The status/read-model layer only needs the latest readiness and failure bookkeeping,
/// not the full historical `runs` stream.
#[derive(Debug, Clone, Default)]
pub(super) struct AiIndexLedgerRow {
    pub state: String,
    pub source_watermark: i64,
    pub last_indexed_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub failure_reason: Option<String>,
}

/// Returns the host timezone name recorded into unified AI runs.
pub(super) fn current_timezone_name() -> String {
    get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

pub(super) fn load_index_ledger(
    s: &Session<'_>,
    provider: &str,
    model: &str,
) -> Result<AiIndexLedgerRow> {
    Ok(s.query_one(
        "SELECT state,COALESCE(source_watermark,0),last_indexed_at,last_failure_at,failure_reason
        FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
        params![provider, model],
        |r| {
            Ok(AiIndexLedgerRow {
                state: r.get(0)?,
                source_watermark: r.get(1)?,
                last_indexed_at: r.get(2)?,
                last_failure_at: r.get(3)?,
                failure_reason: r.get(4)?,
            })
        },
    )
    .optional()?
    .unwrap_or_default())
}

pub(super) fn current_source_watermark(vault: &crate::Vault) -> Result<i64> {
    vault.read(vault_store::Db::Archive, |s| {
        let count =
            s.query_one("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
                Ok(r.get::<i64>(0)?)
            })?;
        let max_id = if count == 0 {
            0
        } else {
            s.query_one(
                "SELECT id FROM visits WHERE reverted_at IS NULL ORDER BY id DESC LIMIT 1",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?
        };
        Ok((count << 32) ^ max_id)
    })
}

pub(super) fn provider_embedding_count(
    s: &Session<'_>,
    provider: &str,
    model: &str,
) -> Result<i64> {
    Ok(s.query_one(
        "SELECT item_count FROM ai_embedding_totals WHERE provider_id=?1 AND model=?2",
        params![provider, model],
        |r| Ok(r.get::<i64>(0)?),
    )
    .optional()?
    .unwrap_or(0))
}
pub(super) fn ai_embeddings_storage_bytes(s: &Session<'_>) -> Result<u64> {
    Ok(s.query_one("SELECT COALESCE(SUM(metadata_bytes),0) FROM ai_embedding_totals", (), |r| {
        Ok(r.get::<i64>(0)?)
    })? as u64)
}
pub(super) fn ai_embedding_token_estimate(s: &Session<'_>) -> Result<u64> {
    Ok((s.query_one("SELECT COALESCE(SUM(content_bytes),0) FROM ai_embedding_totals", (), |r| {
        Ok(r.get::<i64>(0)?)
    })? as u64)
        .div_ceil(4))
}

pub(super) fn semantic_index_staleness_reason(
    s: &Session<'_>,
    provider: &str,
    model: &str,
    source_watermark: i64,
    current_watermark: i64,
    last_indexed_at: Option<&str>,
) -> Result<Option<AiSemanticStaleness>> {
    if provider_embedding_count(s, provider, model)? == 0 {
        return Ok(None);
    }
    if source_watermark != 0 && current_watermark != source_watermark {
        return Ok(Some(AiSemanticStaleness::Watermark));
    }
    if let Some(last) = last_indexed_at {
        let newest = s.query_one("SELECT fetched_at FROM page_content_enrichments WHERE fetch_status='success' ORDER BY fetched_at DESC LIMIT 1",(),|r|Ok(r.get::<String>(0)?)).optional()?;
        if newest.as_deref().is_some_and(|value| value > last) {
            return Ok(Some(AiSemanticStaleness::Enrichment));
        }
    }
    Ok(None)
}

pub(super) fn begin_ai_run(
    vault: &crate::Vault,
    run_type: &str,
    trigger: &str,
    stats_json: serde_json::Value,
) -> Result<i64> {
    let run_type = run_type.to_owned();
    let trigger = trigger.to_owned();
    vault.write(vault_store::Db::Archive, vault_store::Priority::Batch, move |connection| {
        let run_type = run_type.as_str();
        let trigger = trigger.as_str();

        let started_at = now_rfc3339();
        let id = connection.query_one(
            "INSERT INTO runs (
           run_type,
           trigger,
           started_at,
           timezone,
           status,
           profile_scope_json,
           warnings_json,
           stats_json,
           due_only
         )
         VALUES (?1, ?2, ?3, ?4, 'running', '[]', '[]', ?5, 0) RETURNING id",
            params![
                run_type,
                trigger,
                started_at,
                current_timezone_name(),
                serde_json::to_string(&stats_json)?,
            ],
            |r| Ok(r.get::<i64>(0)?),
        )?;
        Ok(id)
    })
}

pub(super) fn finalize_ai_run_success(
    vault: &crate::Vault,
    run_id: i64,
    stats_json: serde_json::Value,
) -> Result<()> {
    vault.write(vault_store::Db::Archive, vault_store::Priority::Batch, move |connection| {
        let finished_at = now_rfc3339();
        connection.execute(
            "UPDATE runs
         SET finished_at = ?1,
             status = 'success',
             error_message = NULL,
             warnings_json = '[]',
             stats_json = ?2
         WHERE id = ?3",
            params![finished_at, serde_json::to_string(&stats_json)?, run_id],
        )?;
        Ok(())
    })
}

pub(super) fn finalize_ai_run_failure(
    vault: &crate::Vault,
    run_id: i64,
    error_message: &str,
    stats_json: serde_json::Value,
) -> Result<()> {
    let error_message = error_message.to_owned();
    vault.write(vault_store::Db::Archive, vault_store::Priority::Batch, move |connection| {
        let error_message = error_message.as_str();

        let finished_at = now_rfc3339();
        connection.execute(
            "UPDATE runs
         SET finished_at = ?1,
             status = 'failed',
             error_message = ?2,
             stats_json = ?3
         WHERE id = ?4",
            params![finished_at, error_message, serde_json::to_string(&stats_json)?, run_id],
        )?;
        Ok(())
    })
}

pub(super) fn record_index_ledger_start(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    run_id: i64,
    started_at: &str,
    source_watermark: i64,
    sidecar_table: &str,
    request: &AiIndexRequest,
) -> Result<()> {
    let provider = provider.clone();
    let started_at = started_at.to_owned();
    let sidecar_table = sidecar_table.to_owned();
    let request = request.clone();
    vault.write(vault_store::Db::Intelligence, vault_store::Priority::Batch, move |connection| {
        let provider = &provider;
        let started_at = started_at.as_str();
        let sidecar_table = sidecar_table.as_str();
        let request = &request;

        let state = if request.clear_only { "clearing" } else { "building" };
        connection.execute(
            "INSERT INTO ai_index_ledger (
           provider_id,
           model,
           sidecar_table,
           index_version,
           state,
           source_watermark,
           last_run_id,
           build_started_at,
           build_finished_at,
           last_indexed_at,
           last_cleared_at,
           last_failure_at,
           failure_reason
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, NULL, NULL, NULL)
         ON CONFLICT(provider_id, model) DO UPDATE SET
           sidecar_table = excluded.sidecar_table,
           index_version = excluded.index_version,
           state = excluded.state,
           source_watermark = excluded.source_watermark,
           last_run_id = excluded.last_run_id,
           build_started_at = excluded.build_started_at,
           build_finished_at = NULL,
           last_failure_at = NULL,
           failure_reason = NULL",
            params![
                provider.config.id,
                provider.config.default_model,
                sidecar_table,
                AI_INDEX_LEDGER_VERSION,
                state,
                source_watermark,
                run_id,
                started_at,
            ],
        )?;
        Ok(())
    })
}

pub(super) fn record_index_ledger_success(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    run_id: i64,
    finished_at: &str,
    source_watermark: i64,
    sidecar_table: &str,
    request: &AiIndexRequest,
) -> Result<()> {
    let provider = provider.clone();
    let finished_at = finished_at.to_owned();
    let sidecar_table = sidecar_table.to_owned();
    let request = request.clone();
    vault.write(vault_store::Db::Intelligence, vault_store::Priority::Batch, move |connection| {
        let provider = &provider;
        let finished_at = finished_at.as_str();
        let sidecar_table = sidecar_table.as_str();
        let request = &request;

        connection.execute(
            "UPDATE ai_index_ledger
         SET state = ?1,
             sidecar_table = ?2,
             index_version = ?3,
             source_watermark = ?4,
             last_run_id = ?5,
             build_finished_at = ?6,
             last_indexed_at = ?7,
             last_cleared_at = CASE WHEN ?8 = 1 THEN ?7 ELSE last_cleared_at END,
             last_failure_at = NULL,
             failure_reason = NULL
         WHERE provider_id = ?9 AND model = ?10",
            params![
                if request.clear_only { "cleared" } else { "ready" },
                sidecar_table,
                AI_INDEX_LEDGER_VERSION,
                source_watermark,
                run_id,
                finished_at,
                finished_at,
                request.clear_only as i64,
                provider.config.id,
                provider.config.default_model,
            ],
        )?;
        Ok(())
    })
}

pub(super) fn record_index_ledger_failure(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    run_id: i64,
    source_watermark: i64,
    sidecar_table: &str,
    _request: &AiIndexRequest,
    error_message: &str,
) -> Result<()> {
    let provider = provider.clone();
    let sidecar_table = sidecar_table.to_owned();
    let _request = _request.clone();
    let error_message = error_message.to_owned();
    vault.write(vault_store::Db::Intelligence, vault_store::Priority::Batch, move |connection| {
        let provider = &provider;
        let sidecar_table = sidecar_table.as_str();
        let _request = &_request;
        let error_message = error_message.as_str();

        let failed_at = now_rfc3339();
        connection.execute(
            "UPDATE ai_index_ledger
         SET state = 'failed',
             sidecar_table = ?1,
             index_version = ?2,
             source_watermark = ?3,
             last_run_id = ?4,
             build_finished_at = ?5,
             last_failure_at = ?5,
             failure_reason = ?6
         WHERE provider_id = ?7 AND model = ?8",
            params![
                sidecar_table,
                AI_INDEX_LEDGER_VERSION,
                source_watermark,
                run_id,
                failed_at,
                error_message,
                provider.config.id,
                provider.config.default_model,
            ],
        )?;
        Ok(())
    })
}
