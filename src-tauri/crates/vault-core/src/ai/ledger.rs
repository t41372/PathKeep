//! AI run-ledger and semantic index bookkeeping helpers.
//!
//! ## Responsibilities
//! - own the compact semantic-index ledger row shape
//! - record unified `runs` lifecycle transitions for AI index and assistant work
//! - expose watermark and storage-size helpers consumed by AI status/read models
//!
//! ## Not responsible for
//! - executing embedding requests or semantic sidecar synchronization
//! - assistant retrieval composition or semantic result ranking
//! - Settings-facing read-model assembly
//!
//! ## Dependencies
//! - shared SQLite connections, constants, and utility helpers from the parent `ai` module
//!
//! ## Performance notes
//! - watermark and storage helpers aggregate directly in SQLite, avoiding full-row scans
//!   into memory

use super::*;

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

/// Loads the compact ledger row that tracks semantic-index readiness for one provider/model.
pub(super) fn load_index_ledger(
    connection: &Connection,
    provider_id: &str,
    model: &str,
) -> Result<AiIndexLedgerRow> {
    connection
        .query_row(
            "SELECT
                state,
                COALESCE(source_watermark, 0),
                last_indexed_at,
                last_failure_at,
                failure_reason
             FROM ai_index_ledger
             WHERE provider_id = ?1 AND model = ?2",
            params![provider_id, model],
            |row| {
                Ok(AiIndexLedgerRow {
                    state: row.get(0)?,
                    source_watermark: row.get(1)?,
                    last_indexed_at: row.get(2)?,
                    last_failure_at: row.get(3)?,
                    failure_reason: row.get(4)?,
                })
            },
        )
        .optional()
        .map(|row| row.unwrap_or_default())
        .context("loading AI index ledger")
}

/// Computes the visibility-aware watermark used to detect stale semantic indexes.
pub(super) fn current_source_watermark(connection: &Connection) -> Result<i64> {
    connection
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(id), 0)
             FROM archive.visits
             WHERE reverted_at IS NULL",
            [],
            |row| {
                let visible_rows = row.get::<_, i64>(0)?.max(0);
                let max_history_id = row.get::<_, i64>(1)?.max(0);
                Ok((visible_rows << 32) ^ max_history_id)
            },
        )
        .context("loading visibility-aware AI index watermark")
}

/// Returns the host timezone name recorded into unified AI runs.
pub(super) fn current_timezone_name() -> String {
    get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Opens one `runs` ledger row for a semantic build or assistant flow.
pub(super) fn begin_ai_run(
    connection: &Connection,
    run_type: &str,
    trigger: &str,
    stats_json: serde_json::Value,
) -> Result<i64> {
    let started_at = now_rfc3339();
    connection.execute(
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
         VALUES (?1, ?2, ?3, ?4, 'running', '[]', '[]', ?5, 0)",
        params![
            run_type,
            trigger,
            started_at,
            current_timezone_name(),
            serde_json::to_string(&stats_json)?,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

/// Marks one AI run as successful and stores the final stats payload.
pub(super) fn finalize_ai_run_success(
    connection: &Connection,
    run_id: i64,
    stats_json: serde_json::Value,
) -> Result<()> {
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
}

/// Marks one AI run as failed while preserving the final stats payload for audit.
pub(super) fn finalize_ai_run_failure(
    connection: &Connection,
    run_id: i64,
    error_message: &str,
    stats_json: serde_json::Value,
) -> Result<()> {
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
}

/// Writes the initial semantic-index ledger row for one build/clear run.
pub(super) fn record_index_ledger_start(
    connection: &Connection,
    provider: &AiProviderRuntime,
    run_id: i64,
    started_at: &str,
    source_watermark: i64,
    sidecar_table: &str,
    request: &AiIndexRequest,
) -> Result<()> {
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
}

/// Marks the semantic-index ledger row as ready/cleared after a successful run.
pub(super) fn record_index_ledger_success(
    connection: &Connection,
    provider: &AiProviderRuntime,
    run_id: i64,
    finished_at: &str,
    source_watermark: i64,
    sidecar_table: &str,
    request: &AiIndexRequest,
) -> Result<()> {
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
}

/// Records a failed semantic-index build into the compact ledger row.
pub(super) fn record_index_ledger_failure(
    connection: &Connection,
    provider: &AiProviderRuntime,
    run_id: i64,
    source_watermark: i64,
    sidecar_table: &str,
    _request: &AiIndexRequest,
    error_message: &str,
) -> Result<()> {
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
}

/// Estimates the on-disk size of SQLite compatibility metadata for semantic embeddings.
pub(super) fn ai_embeddings_storage_bytes(connection: &Connection) -> Result<u64> {
    if !crate::ai::search::sqlite_table_exists(connection, "ai_embeddings")? {
        return Ok(0);
    }
    let bytes: i64 = connection.query_row(
        "SELECT COALESCE(SUM(
            LENGTH(IFNULL(url, '')) +
            LENGTH(IFNULL(title, '')) +
            LENGTH(IFNULL(domain, '')) +
            LENGTH(IFNULL(visited_at, '')) +
            LENGTH(IFNULL(content_hash, '')) +
            LENGTH(IFNULL(provider_id, '')) +
            LENGTH(IFNULL(model, '')) +
            8
         ), 0)
         FROM ai_embeddings",
        [],
        |row: &Row<'_>| row.get(0),
    )?;
    Ok(bytes.max(0) as u64)
}

/// Estimates the input token volume represented by stored semantic embedding metadata.
pub(super) fn ai_embedding_token_estimate(connection: &Connection) -> Result<u64> {
    if !crate::ai::search::sqlite_table_exists(connection, "ai_embeddings")? {
        return Ok(0);
    }
    let characters: i64 = connection.query_row(
        "SELECT COALESCE(SUM(content_bytes), 0) FROM ai_embeddings",
        [],
        |row: &Row<'_>| row.get(0),
    )?;
    let characters = characters.max(0) as u64;
    Ok(characters.div_ceil(4))
}
