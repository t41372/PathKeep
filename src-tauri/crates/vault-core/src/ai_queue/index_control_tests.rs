//! Responsibilities: protect index ownership and automatic-work consent across queue races.
//! Not responsible for: provider execution or UI flows (reviewer E2E covers those).
use super::*;
use crate::{AiProviderConfig, AppConfig, EmbeddingIndexRef, IndexBuildPause};

fn configured(model: &str) -> Vault {
    let mut config = AppConfig::default();
    config.ai.embedding_provider_id = Some("embed".into());
    config.ai.embedding_providers = vec![AiProviderConfig {
        id: "embed".into(),
        default_model: model.into(),
        ..Default::default()
    }];
    Vault::open_temp().unwrap().with_config(config)
}
fn request(vault: &Vault, automatic: bool) -> Option<AiQueueJob> {
    request_index_job(vault, &AiIndexRequest::default(), automatic).unwrap()
}
fn ready(vault: &Vault, model: &str, finished: &str) {
    let (model, finished) = (model.to_owned(), finished.to_owned());
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        s.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state,build_finished_at)
            VALUES('embed',?1,'test','test','ready',?2)", vault_store::params![model, finished])?;
        Ok(())
    }).unwrap();
}

/// Failure modes: repeated requests create competing writers; pause loses its cursor; a full
/// request silently replaces active partial work; a build reuses a clear; another writer starts
/// before the old writer stops.
#[test]
fn ai_queue_build_requests_reuse_progress_and_wait_for_the_previous_writer() {
    let vault = configured("old");
    let first = request(&vault, false).unwrap();
    assert_eq!(request(&vault, false).unwrap().id, first.id);
    let full = AiIndexRequest { full_rebuild: true, ..Default::default() };
    assert_eq!(request_index_job(&vault, &full, false).unwrap().unwrap().id, first.id);
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    persist_index_cursor(
        &vault,
        first.id,
        &IndexBackfillCursor {
            lineage: None,
            updated_so_far: 0,
            next_history_id: 65,
            embedded_so_far: 64,
            embed_target: 200,
            scan_target: 200,
        },
        None,
    )
    .unwrap();
    pause_ai_job(&vault, first.id).unwrap();
    mark_running_ai_job_cancelled(&vault, first.id, None).unwrap();
    assert_eq!(load_ai_job(&vault, first.id).unwrap().paused_by, Some(IndexBuildPause::User));
    assert_eq!(request(&vault, false).unwrap().id, first.id);
    let AiJobPayload::Index { cursor, .. } = load_ai_job_payload(&vault, first.id).unwrap() else {
        panic!("index")
    };
    assert_eq!(cursor.next_history_id, 65);
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    let mut config = vault.config().clone();
    config.ai.embedding_providers[0].default_model = "new".into();
    let changed = vault.with_config(config);
    let next = request(&changed, false).unwrap();
    assert_ne!(next.id, first.id);
    assert!(claim_next_ai_job(&changed, 300).unwrap().is_none());
    mark_running_ai_job_cancelled(&changed, first.id, None).unwrap();
    assert_eq!(claim_next_ai_job(&changed, 300).unwrap().unwrap().id, next.id);

    let vault = configured("model");
    let clear = enqueue_index_job(
        &vault,
        &AiIndexRequest {
            provider_id: Some("embed".into()),
            clear_only: true,
            ..Default::default()
        },
        false,
    )
    .unwrap();
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    let build = request(&vault, false).unwrap();
    assert_ne!(build.id, clear.id);
    assert_eq!(build.job_type, "index-build");
    assert!(ai_job_stop_requested(&vault, clear.id).unwrap());
    assert!(claim_next_ai_job(&vault, 300).unwrap().is_none());
    mark_running_ai_job_cancelled(&vault, clear.id, None).unwrap();
    assert_eq!(claim_next_ai_job(&vault, 300).unwrap().unwrap().id, build.id);
}

/// Failure modes: backup restarts an unfinished/cancelled first fill; it duplicates active work;
/// queue resume wakes a user/service pause; a pending pause becomes runnable after restart.
#[test]
fn ai_queue_backups_and_queue_resume_cannot_restart_work_the_user_stopped() {
    let vault = configured("model");
    assert!(request(&vault, true).is_none());
    ready(&vault, "model", "2026-10-15T00:00:00Z");
    let first = request(&vault, true).unwrap();
    assert!(request(&vault, true).is_none());
    pause_ai_job(&vault, first.id).unwrap();
    assert_eq!(resume_paused_jobs(&vault).unwrap(), 0);
    assert!(request(&vault, true).is_none());
    replay_ai_job(&vault, first.id, false).unwrap();
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    pause_ai_job(&vault, first.id).unwrap();
    recover_owner_claims(vault.store(), false).unwrap();
    assert_eq!(load_ai_job(&vault, first.id).unwrap().state, "paused");
    assert!(claim_next_ai_job(&vault, 300).unwrap().is_none());
    replay_ai_job(&vault, first.id, false).unwrap();
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    mark_ai_job_failed(
        &vault,
        first.id,
        None,
        &AiJobFailure {
            error_code: Some("service-unavailable".into()),
            error_message: "Failed to load model".into(),
            retryable: true,
            retry_after_seconds: 30,
            ..Default::default()
        },
        true,
    )
    .unwrap();
    assert_eq!(load_ai_job(&vault, first.id).unwrap().paused_by, Some(IndexBuildPause::Service));
    assert_eq!(resume_paused_jobs(&vault).unwrap(), 0);
    replay_ai_job(&vault, first.id, true).unwrap();
    assert_eq!(resume_paused_jobs(&vault).unwrap(), 1);
}

/// Failure modes: a partial chosen model hides the last complete index; removed services serve
/// deleted vectors; completion time loses to model name; ready chosen loses to newer old models.
#[test]
fn ai_queue_search_uses_the_latest_complete_configured_pair_until_chosen_is_ready() {
    let vault = configured("chosen");
    ready(&vault, "older", "2026-10-13T00:00:00Z");
    ready(&vault, "previous", "2026-10-14T00:00:00Z");
    let state = crate::semantic_index::load_state(&vault).unwrap();
    assert!(!state.chosen_complete);
    assert_eq!(
        state.serving,
        Some(EmbeddingIndexRef { provider_id: "embed".into(), model: "previous".into() })
    );
    ready(&vault, "chosen", "2026-10-12T00:00:00Z");
    let state = crate::semantic_index::load_state(&vault).unwrap();
    assert!(state.chosen_complete);
    assert_eq!(state.serving, state.chosen);
    let mut config = vault.config().clone();
    config.ai.embedding_providers.clear();
    let state = crate::semantic_index::load_state(&vault.with_config(config)).unwrap();
    assert!(state.serving.is_none());
}

/// Failure modes: a pause/cancel arriving between the worker's stop read and error write leaves
/// a running orphan or is overwritten as service failure; unchanged cursor reports erase retry
/// attempts; a queue resume erases a stop already seen by the worker.
#[test]
fn ai_queue_stop_wins_a_concurrent_failure_and_only_new_progress_renews_retries() {
    let vault = configured("model");
    let job = request(&vault, false).unwrap();
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    let cursor = IndexBackfillCursor { embed_target: 200, ..Default::default() };
    persist_index_cursor(&vault, job.id, &cursor, None).unwrap();
    assert_eq!(load_ai_job(&vault, job.id).unwrap().attempt, 1);
    let progress = IndexBackfillCursor { next_history_id: 65, embedded_so_far: 64, ..cursor };
    persist_index_cursor(&vault, job.id, &progress, None).unwrap();
    persist_index_cursor(&vault, job.id, &progress, None).unwrap();
    pause_queued_jobs(&vault).unwrap();
    assert_eq!(resume_paused_jobs(&vault).unwrap(), 0);
    assert!(ai_job_stop_requested(&vault, job.id).unwrap());
    let result = mark_ai_job_failed(
        &vault,
        job.id,
        None,
        &AiJobFailure {
            error_code: Some("service-unavailable".into()),
            error_message: "offline".into(),
            retryable: true,
            retry_after_seconds: 30,
            ..Default::default()
        },
        false,
    )
    .unwrap();
    assert_eq!(result.state, "paused");
    assert_eq!(result.paused_by, Some(IndexBuildPause::Queue));
    assert_eq!(resume_paused_jobs(&vault).unwrap(), 1);
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    persist_index_cursor(&vault, job.id, &progress, None).unwrap();
    assert_eq!(load_ai_job(&vault, job.id).unwrap().attempt, 1, "same cursor is not progress");
}

/// Failure modes: a model-less legacy job (including cursor zero) is assigned to today's model and skips unembedded
/// visits; an acknowledged queue resume is lost across a crash; an active queue pause wakes up.
#[test]
fn ai_queue_recovery_preserves_pause_ownership_and_retires_unknown_model_cursors() {
    for queue_paused in [false, true] {
        let vault = configured("model");
        let job = request(&vault, false).unwrap();
        claim_next_ai_job(&vault, 300).unwrap().unwrap();
        pause_queued_jobs(&vault).unwrap();
        recover_owner_claims(vault.store(), queue_paused).unwrap();
        assert_eq!(
            load_ai_job(&vault, job.id).unwrap().state,
            if queue_paused { "paused" } else { "queued" }
        );
    }
    for (provider_id, next_history_id) in [None, Some("embed".to_owned())]
        .into_iter()
        .flat_map(|id| [0, 65].map(|cursor| (id.clone(), cursor)))
    {
        let vault = configured("model");
        let job = request(&vault, false).unwrap();
        let legacy = AiJobPayload::Index {
            request: AiIndexRequest {
                provider_id,
                full_rebuild: true,
                scope: crate::ReembedScope::Full,
                ..Default::default()
            },
            model: None,
            cursor: IndexBackfillCursor {
                next_history_id,
                embedded_so_far: if next_history_id == 0 { 0 } else { 64 },
                ..Default::default()
            },
        };
        let json = serde_json::to_string(&legacy).unwrap();
        vault
            .write(Db::Intelligence, Priority::Interactive, move |s| {
                s.execute(
                    "UPDATE ai_jobs SET payload_json=?1 WHERE id=?2",
                    vault_store::params![json, job.id],
                )?;
                Ok(())
            })
            .unwrap();
        recover_owner_claims(vault.store(), false).unwrap();
        assert_eq!(load_ai_job(&vault, job.id).unwrap().state, "cancelled");
        assert!(claim_next_ai_job(&vault, 300).unwrap().is_none());
    }
}

/// Failure modes: a worker dies on the last service retry, startup requeues the claim and the
/// attempt guard turns an unavailable service into failed or replaces the service's words.
#[test]
fn ai_queue_an_interrupted_last_service_retry_pauses_with_the_original_error() {
    let vault = configured("model");
    let job = request(&vault, false).unwrap();
    vault.write(Db::Intelligence, Priority::Interactive, move |s| {
        s.execute("UPDATE ai_jobs SET attempt=max_attempts,error_code='service-unavailable',error_message='Failed to load model: Operation canceled' WHERE id=?1", [job.id])?; Ok(())
    }).unwrap();
    assert!(claim_next_ai_job(&vault, 300).unwrap().is_none());
    let paused = load_ai_job(&vault, job.id).unwrap();
    assert_eq!(paused.state, "paused");
    assert_eq!(paused.paused_by, Some(IndexBuildPause::Service));
    assert_eq!(paused.error_message.as_deref(), Some("Failed to load model: Operation canceled"));
}

/// Failure modes: corrupted JSON hides the whole status, an exhausted malformed queue head
/// rolls back quarantine and blocks valid work, or cancelling that row rolls back too.
#[test]
fn ai_queue_a_corrupt_payload_cannot_hide_or_block_valid_work() {
    let vault = configured("model");
    let broken = request(&vault, false).unwrap();
    vault
        .write(Db::Intelligence, Priority::Interactive, move |s| {
            s.execute(
                "UPDATE ai_jobs SET payload_json='{',attempt=max_attempts WHERE id=?1",
                [broken.id],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(load_ai_queue_status(&vault, false, 1, 8).is_ok());
    let valid = request(&vault, false).unwrap();
    assert_eq!(claim_next_ai_job(&vault, 300).unwrap().unwrap().id, valid.id);
    assert_eq!(load_ai_job(&vault, broken.id).unwrap().state, "failed");
    cancel_ai_job(&vault, valid.id).unwrap();
    mark_running_ai_job_cancelled(&vault, valid.id, None).unwrap();
    let another = request(&vault, false).unwrap();
    vault
        .write(Db::Intelligence, Priority::Interactive, move |s| {
            s.execute("UPDATE ai_jobs SET payload_json='{' WHERE id=?1", [another.id])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(cancel_ai_job(&vault, another.id).unwrap().state, "cancelled");
}

/// Failure modes: cancelling or pausing a queued update, rebuild or clear demotes the complete index,
/// making search drop it and the next build wipe it. Neither action may change readiness.
#[test]
fn ai_queue_stopping_a_queued_job_keeps_the_complete_index_ready() {
    for (pause, full_rebuild, clear_only) in [false, true].into_iter().flat_map(|pause| {
        [(false, false), (true, false), (false, true)].map(|(full, clear)| (pause, full, clear))
    }) {
        let vault = configured("model");
        ready(&vault, "model", "2026-10-15T00:00:00Z");
        let request = AiIndexRequest { full_rebuild, clear_only, ..Default::default() };
        let job = request_index_job(&vault, &request, false).unwrap().unwrap();
        if pause {
            pause_ai_job(&vault, job.id).unwrap();
        } else {
            cancel_ai_job(&vault, job.id).unwrap();
        }
        let state = crate::semantic_index::load_state(&vault).unwrap();
        assert!(state.chosen_complete);
        assert_eq!(state.serving, state.chosen);
    }
}
