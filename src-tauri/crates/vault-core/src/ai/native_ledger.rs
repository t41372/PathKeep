//! Responsibilities: preserve complete indexes during updates and persist build/run state.
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
    pub source_watermark: i64,
    pub last_indexed_at: Option<String>,
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
        "SELECT COALESCE(source_watermark,0),last_indexed_at
        FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
        params![provider, model],
        |r| Ok(AiIndexLedgerRow { source_watermark: r.get(0)?, last_indexed_at: r.get(1)? }),
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
         WHERE id = ?3 AND status = 'running'",
            params![finished_at, serde_json::to_string(&stats_json)?, run_id],
        )?;
        Ok(())
    })
}

/// Ends an AI run that failed with `error`, `interrupted` when it was cancelled. The caller goes
/// on to return `error`, so a failure to record the end is printed rather than returned (it hid
/// why the run failed); the next time the archive opens, a run left running is interrupted.
pub(super) fn end_failed_ai_run(
    vault: &crate::Vault,
    run_id: i64,
    error: &anyhow::Error,
    stats_json: serde_json::Value,
) {
    let status = crate::archive::write_runs::failure_status(error);
    let message = format!("{error:#}");
    let recorded =
        vault.write(vault_store::Db::Archive, vault_store::Priority::Batch, move |connection| {
            connection.execute(
                "UPDATE runs
             SET finished_at = ?1,
                 status = ?2,
                 error_message = ?3,
                 stats_json = ?4
             WHERE id = ?5 AND status = 'running'",
                params![
                    now_rfc3339(),
                    status,
                    message.as_str(),
                    serde_json::to_string(&stats_json)?,
                    run_id
                ],
            )?;
            Ok(())
        });
    if let Err(recording) = recorded {
        eprintln!(
            "pathkeep: AI run {run_id} failed ({error:#}), and so did recording it: {recording:#}"
        );
    }
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
        let wipes = request.full_rebuild || request.clear_only || request.scope == crate::ReembedScope::Full;
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
           state = CASE WHEN ?9 = 0 AND ai_index_ledger.state = 'ready' THEN 'ready' ELSE excluded.state END,
           source_watermark = CASE WHEN ?9 = 0 AND ai_index_ledger.state = 'ready' THEN ai_index_ledger.source_watermark ELSE excluded.source_watermark END,
           last_run_id = excluded.last_run_id,
           build_started_at = excluded.build_started_at,
           build_finished_at = CASE WHEN ?9 = 0 AND ai_index_ledger.state = 'ready' THEN ai_index_ledger.build_finished_at ELSE NULL END,
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
                wipes as i64,
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
    queue_job_id: Option<i64>,
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

        if let Some(id) = queue_job_id {
            let can_finish = connection.query_one(
                "SELECT state='running' AND stop_requested=0 FROM ai_jobs WHERE id=?1",
                [id],
                |r| Ok(r.get::<i64>(0)?),
            )? != 0;
            if !can_finish {
                return Err(AiRunCancelled::new("Index build stopped before completion.").into());
            }
        }
        connection.execute(
            "UPDATE ai_index_ledger
         SET state = CASE WHEN state = 'ready' AND ?1 = 'incomplete' THEN 'ready' ELSE ?1 END,
             sidecar_table = ?2,
             index_version = ?3,
             source_watermark = CASE WHEN state = 'ready' AND ?1 = 'incomplete' THEN source_watermark ELSE ?4 END,
             last_run_id = ?5,
             build_finished_at = CASE WHEN state = 'ready' AND ?1 = 'incomplete' THEN build_finished_at ELSE ?6 END,
             last_indexed_at = CASE WHEN state = 'ready' AND ?1 = 'incomplete' THEN last_indexed_at ELSE ?7 END,
             last_cleared_at = CASE WHEN ?8 = 1 THEN ?7 ELSE last_cleared_at END,
             last_failure_at = NULL,
             failure_reason = NULL
         WHERE provider_id = ?9 AND model = ?10",
            params![
                if request.clear_only {
                    "cleared"
                } else if request.scope == crate::ReembedScope::WorkingSet {
                    "incomplete"
                } else {
                    "ready"
                },
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
         SET state = 'incomplete',
             sidecar_table = ?1,
             index_version = ?2,
             source_watermark = ?3,
             last_run_id = ?4,
             build_finished_at = ?5,
             last_failure_at = ?5,
             failure_reason = ?6
         WHERE provider_id = ?7 AND model = ?8 AND state IN ('building','clearing')",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Failure modes: an interrupted non-wiping update drops a ready index from search;
    /// a wiping build advertises ready while deleting vectors; a first fill claims completion;
    /// a working-set success demotes a complete index or certifies unscanned visits/enrichment.
    /// Only a full-archive success may advance complete coverage watermarks and timestamps.
    #[test]
    fn ai_queue_ledger_preserves_ready_updates_and_marks_wiping_builds() {
        for (full_rebuild, clear_only, scope, expected) in [
            (false, false, crate::ReembedScope::Incremental, "ready"),
            (false, false, crate::ReembedScope::WorkingSet, "ready"),
            (true, false, crate::ReembedScope::Incremental, "building"),
            (false, false, crate::ReembedScope::Full, "building"),
            (false, true, crate::ReembedScope::Incremental, "clearing"),
        ] {
            let vault = crate::Vault::open_temp().unwrap();
            let provider = AiProviderRuntime {
                config: crate::AiProviderConfig {
                    id: "embed".into(),
                    default_model: "model".into(),
                    ..Default::default()
                },
                api_key: None,
            };
            vault.write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |s| {
                s.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state,build_finished_at,source_watermark,last_indexed_at) VALUES('embed','model','test','test','ready','2026-10-14T00:00:00Z',7,'2026-10-14T00:00:00Z')", ())?;
                Ok(())
            }).unwrap();
            let request = AiIndexRequest { full_rebuild, clear_only, scope, ..Default::default() };
            record_index_ledger_start(
                &vault,
                &provider,
                1,
                "2026-10-15T00:00:00Z",
                9,
                "test",
                &request,
            )
            .unwrap();
            let read = || {
                vault
                    .read(vault_store::Db::Intelligence, |s| {
                        Ok(s.query_one(
                            "SELECT state,build_finished_at,source_watermark,last_indexed_at FROM ai_index_ledger",
                            (),
                            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?, r.get::<i64>(2)?, r.get::<Option<String>>(3)?)),
                        )?)
                    })
                    .unwrap()
            };
            assert_eq!(read().0, expected);
            if expected == "ready" {
                assert_eq!(read().1.as_deref(), Some("2026-10-14T00:00:00Z"));
                assert_eq!(read().2, 7);
                assert_eq!(read().3.as_deref(), Some("2026-10-14T00:00:00Z"));
            }
            record_index_ledger_failure(&vault, &provider, 1, 9, "test", &request, "interrupted")
                .unwrap();
            assert_eq!(read().0, if expected == "ready" { "ready" } else { "incomplete" });
            if expected == "ready" {
                record_index_ledger_success(
                    &vault,
                    &provider,
                    1,
                    "2026-10-15T00:00:00Z",
                    9,
                    "test",
                    &request,
                    None,
                )
                .unwrap();
                let (watermark, finished) = if request.scope == crate::ReembedScope::WorkingSet {
                    (7, "2026-10-14T00:00:00Z")
                } else {
                    (9, "2026-10-15T00:00:00Z")
                };
                assert_eq!(read().0, "ready");
                assert_eq!(read().1.as_deref(), Some(finished));
                assert_eq!(read().2, watermark);
                assert_eq!(read().3.as_deref(), Some(finished));
            }
        }
        let vault = crate::Vault::open_temp().unwrap();
        let provider =
            AiProviderRuntime { config: crate::AiProviderConfig::default(), api_key: None };
        record_index_ledger_start(
            &vault,
            &provider,
            1,
            "now",
            0,
            "test",
            &AiIndexRequest::default(),
        )
        .unwrap();
        assert_eq!(
            vault
                .read(vault_store::Db::Intelligence, |s| Ok(s.query_one(
                    "SELECT state FROM ai_index_ledger",
                    (),
                    |r| Ok(r.get::<String>(0)?)
                )?))
                .unwrap(),
            "building"
        );
    }
}
