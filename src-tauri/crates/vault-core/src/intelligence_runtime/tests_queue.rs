//! Responsibilities: native queue transitions, stale claims, cancellation and complete scans.
//! Not responsible for: network fetching or model execution.
//! Failure modes: duplicate claims, malformed payloads retry forever, cancellation loses a race,
//! queue pages hide eligible work, or a read recovers a live claim.
use super::*;
use super::{claims::*, enqueue::*, job_control::*};
use crate::{
    VISIT_DERIVED_FACTS_MODULE_ID,
    models::{AppConfig, ArchiveMode, EnrichmentPluginPreference},
};
use serde_json::json;
use vault_store::params_owned as params;

use super::{
    claims::{
        claim_enrichment_job_by_id, queued_enrichment_candidates_page, try_claim_enrichment_job,
    },
    recovery::requeue_running_enrichment_jobs_for_run,
    snapshot::load_queue_status,
};

#[test]
fn queue_jobs_can_be_enqueued_and_loaded() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            enqueue_enrichment_job(
                connection,
                7,
                &BUILT_IN_ENRICHMENT_PLUGINS[0],
                &EnrichmentJobPayload {
                    history_id: Some(3),
                    profile_id: Some("chrome:Default".to_string()),
                    url: "https://example.com/docs".to_string(),
                    title: Some("Docs".to_string()),
                },
            )
            .expect("enqueue");

            let queue = load_queue_status(connection).expect("queue status");
            assert_eq!(queue.queued, 1);

            let jobs = super::snapshot::load_job_page(connection, None, 12)
                .map(|p| p.items)
                .expect("recent jobs");
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].plugin_id.as_deref(), Some(TITLE_NORMALIZATION_PLUGIN_ID));
            Ok(())
        })
        .unwrap();
}

#[test]
fn deterministic_rebuild_jobs_are_traced_in_runtime_queue() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let job_id = enqueue_deterministic_rebuild_job_in_session(
                connection,
                &CoreIntelligenceRebuildRequest {
                    profile_id: Some("chrome:Default".to_string()),
                    full_rebuild: true,
                    limit: None,
                },
                "Archive changed.",
            )
            .expect("enqueue deterministic rebuild");

            let queue = load_queue_status(connection).expect("queue status");
            assert_eq!(queue.queued, 1);

            let claimed = claim_deterministic_rebuild_job_in_session(connection, job_id)
                .expect("claim deterministic job");
            assert_eq!(
                claimed.expect("deterministic payload").request.profile_id.as_deref(),
                Some("chrome:Default")
            );

            let jobs = super::snapshot::load_job_page(connection, None, 12)
                .map(|p| p.items)
                .expect("recent jobs");
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].job_type, FULL_REBUILD_JOB_TYPE);
            assert_eq!(
                jobs[0].title.as_deref(),
                Some("chrome:Default · full Core Intelligence rebuild")
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn malformed_core_payload_is_quarantined_after_claim() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let now = crate::utils::now_rfc3339();
            connection
                .execute(
                    "INSERT INTO intelligence_jobs (
                job_type, state, priority, dedupe_key, payload_json, created_at, scheduled_at,
                updated_at
             ) VALUES (?1, 'queued', 10, 'malformed-core', '{', ?2, ?2, ?2)",
                    params![FULL_REBUILD_JOB_TYPE, now],
                )
                .expect("insert malformed core job");
            let job_id = connection
                .query_one("SELECT MAX(id) FROM intelligence_jobs", (), |r| Ok(r.get::<i64>(0)?))
                .unwrap();

            let error = claim_core_intelligence_job_in_session(connection, job_id)
                .expect_err("malformed core payload must fail");

            let (state, last_error): (String, String) = connection
                .query_one(
                    "SELECT state, last_error FROM intelligence_jobs WHERE id = ?1",
                    [job_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("quarantined core row");
            assert_eq!(state, "failed");
            assert!(last_error.contains(&error.to_string()));
            Ok(())
        })
        .unwrap();
}

#[test]
fn malformed_enrichment_payload_is_quarantined_before_claim() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let now = crate::utils::now_rfc3339();
            connection
                .execute(
                    "INSERT INTO intelligence_jobs (
                job_type, plugin_id, state, priority, dedupe_key, payload_json, created_at,
                scheduled_at, updated_at
             ) VALUES (?1, ?2, 'queued', 10, 'malformed-enrichment', '{', ?3, ?3, ?3)",
                    params![ENRICHMENT_JOB_TYPE, TITLE_NORMALIZATION_PLUGIN_ID, now],
                )
                .expect("insert malformed enrichment job");
            let job_id = connection
                .query_one("SELECT MAX(id) FROM intelligence_jobs", (), |r| Ok(r.get::<i64>(0)?))
                .unwrap();

            let error = claim_enrichment_job_by_id(connection, job_id)
                .expect_err("malformed enrichment payload must fail");

            let (state, last_error): (String, String) = connection
                .query_one(
                    "SELECT state, last_error FROM intelligence_jobs WHERE id = ?1",
                    [job_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("quarantined enrichment row");
            assert_eq!(state, "failed");
            assert!(last_error.contains(&error.to_string()));
            assert!(
                next_queued_enrichment_job_in_session(connection)
                    .expect("load next enrichment")
                    .is_none(),
                "the malformed queue head must not be selected forever"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn enqueue_runtime_helpers_dedupe_refresh_and_mark_stale_contracts() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.write(vault_store::Db::Intelligence,vault_store::Priority::Interactive, |connection| {

    let invalid = enqueue_core_intelligence_job_in_session(
        connection,
        "unknown-stage",
        &CoreIntelligenceRebuildRequest::default(),
        "invalid stage",
    )
    .expect_err("invalid core intelligence job type should fail");
    assert!(invalid.to_string().contains("unknown-stage"));

    let request = CoreIntelligenceRebuildRequest {
        profile_id: Some("chrome:Default".to_string()),
        full_rebuild: false,
        limit: Some(0),
    };
    let first_id =
        enqueue_core_intelligence_job_in_session(connection, VISIT_DERIVE_JOB_TYPE, &request, "first")
            .expect("enqueue first deterministic job");
    let refreshed_id =
        enqueue_core_intelligence_job_in_session(connection, VISIT_DERIVE_JOB_TYPE, &request, "second")
            .expect("refresh deterministic job");
    assert_eq!(first_id, refreshed_id);
    connection
        .execute(
            "UPDATE intelligence_jobs SET state = 'running', last_error = 'old', stop_requested = 1 WHERE id = ?1",
            [first_id],
        )
        .expect("mark deterministic job running");
    let running_id =
        enqueue_core_intelligence_job_in_session(connection, VISIT_DERIVE_JOB_TYPE, &request, "third")
            .expect("running deterministic job is deduped without reset");
    assert_eq!(running_id, first_id);
    let (state, last_error, stop_requested): (String, Option<String>, i64) = connection
        .query_one(
            "SELECT state, last_error, stop_requested FROM intelligence_jobs WHERE id = ?1",
            [first_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("load refreshed deterministic state");
    assert_eq!(state, "running");
    assert_eq!(last_error.as_deref(), Some("old"));
    assert_eq!(stop_requested, 1);
    let trigger_count: i64 = connection
        .query_one(
            "SELECT COUNT(*) FROM intelligence_job_triggers WHERE job_id = ?1",
            [first_id],
            |row| Ok(row.get(0)?),
        )
        .expect("deterministic trigger count");
    assert_eq!(trigger_count, 3);

    let enrichment_payload = EnrichmentJobPayload {
        history_id: Some(42),
        profile_id: Some("chrome:Default".to_string()),
        url: "https://example.com/docs".to_string(),
        title: Some("Docs".to_string()),
    };
    enqueue_enrichment_job(connection, 7, &BUILT_IN_ENRICHMENT_PLUGINS[0], &enrichment_payload)
        .expect("enqueue enrichment");
    connection
        .execute(
            "UPDATE intelligence_jobs SET state = 'failed', last_error = 'old failure', stop_requested = 1 WHERE plugin_id = ?1",
            [TITLE_NORMALIZATION_PLUGIN_ID],
        )
        .expect("mark enrichment failed");
    enqueue_enrichment_job(connection, 8, &BUILT_IN_ENRICHMENT_PLUGINS[0], &enrichment_payload)
        .expect("refresh enrichment");
    let (state, last_error, stop_requested): (String, Option<String>, i64) = connection
        .query_one(
            "SELECT state, last_error, stop_requested FROM intelligence_jobs WHERE plugin_id = ?1",
            [TITLE_NORMALIZATION_PLUGIN_ID],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("load refreshed enrichment state");
    assert_eq!(state, "queued");
    assert!(last_error.is_none());
    assert_eq!(stop_requested, 0);
    connection
        .execute(
            "UPDATE intelligence_jobs SET state = 'running', last_error = 'active' WHERE plugin_id = ?1",
            [TITLE_NORMALIZATION_PLUGIN_ID],
        )
        .expect("mark enrichment running");
    enqueue_enrichment_job(connection, 9, &BUILT_IN_ENRICHMENT_PLUGINS[0], &enrichment_payload)
        .expect("dedupe running enrichment");
    let running_error: Option<String> = connection
        .query_one(
            "SELECT last_error FROM intelligence_jobs WHERE plugin_id = ?1",
            [TITLE_NORMALIZATION_PLUGIN_ID],
            |row| Ok(row.get(0)?),
        )
        .expect("running enrichment remains untouched");
    assert_eq!(running_error.as_deref(), Some("active"));

    persist_deterministic_module_runtime_updates(
        connection,
        &[
            DeterministicModuleRuntimeUpdate {
                module_id: "unknown-module".to_string(),
                status: "stale".to_string(),
                last_run_id: None,
                last_built_at: None,
                last_invalidated_at: Some("2026-04-26T00:00:00Z".to_string()),
                stale_reason: Some("ignored".to_string()),
                stale_reason_code: None,
                notes: vec![crate::models::DerivedRuntimeNote::opaque("ignored")],
            },
            DeterministicModuleRuntimeUpdate {
                module_id: VISIT_DERIVED_FACTS_MODULE_ID.to_string(),
                status: "ready".to_string(),
                last_run_id: Some(99),
                last_built_at: Some("2026-04-26T00:00:00Z".to_string()),
                last_invalidated_at: None,
                stale_reason: None,
                stale_reason_code: None,
                notes: vec![crate::models::DerivedRuntimeNote::opaque("fresh")],
            },
        ],
    )
    .expect("persist module updates");
    let module_count: i64 = connection
        .query_one("SELECT COUNT(*) FROM deterministic_module_runtime", (), |row| Ok(row.get(0)?))
        .expect("module runtime count");
    assert_eq!(module_count, 1);

    mark_all_deterministic_modules_stale_in_session(connection, "coverage-stale-code", "coverage stale")
        .expect("mark deterministic modules stale");
    let stale_count: i64 = connection
        .query_one(
            "SELECT COUNT(*) FROM deterministic_module_runtime WHERE status = 'stale' AND stale_reason = 'coverage stale' AND stale_reason_code = 'coverage-stale-code'",
            (),
            |row| Ok(row.get(0)?),
        )
        .expect("stale module count");
    assert_eq!(stale_count, built_in_deterministic_modules().len() as i64);
    Ok(())
    }).unwrap();
}

#[test]
fn recent_jobs_surface_progress_for_running_deterministic_rebuilds() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let job_id = enqueue_deterministic_rebuild_job_in_session(
                connection,
                &CoreIntelligenceRebuildRequest::default(),
                "Archive changed.",
            )
            .expect("enqueue deterministic rebuild");
            claim_deterministic_rebuild_job_in_session(connection, job_id)
                .expect("claim deterministic rebuild");
            update_intelligence_job_artifact_in_session(
                connection,
                job_id,
                &json!({
                    "kind": "deterministic-rebuild",
                    "phase": "Scoring visits",
                    "detail": "12,000 / 64,781 visits",
                    "completedSteps": 4,
                    "totalSteps": 8,
                    "processedItems": 12_000,
                    "totalItems": 64_781,
                    "progressPercent": 43.5,
                    "executionMode": "incremental",
                    "affectedProfiles": ["chrome:Default"],
                    "dirtyVisitCount": 321,
                    "dirtyDateKeys": ["2024-04-02", "2024-04-03"],
                    "fallbackReason": null
                }),
            )
            .expect("update progress artifact");

            let jobs = super::snapshot::load_job_page(connection, None, 12)
                .map(|p| p.items)
                .expect("recent jobs");
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].progress_label.as_deref(), Some("Scoring visits"));
            assert_eq!(jobs[0].progress_detail.as_deref(), Some("12,000 / 64,781 visits"));
            assert_eq!(jobs[0].progress_current, Some(12_000));
            assert_eq!(jobs[0].progress_total, Some(64_781));
            assert_eq!(jobs[0].progress_percent, Some(43.5));
            assert_eq!(jobs[0].execution_mode.as_deref(), Some("incremental"));
            assert_eq!(
                jobs[0].affected_profiles.as_deref(),
                Some(&["chrome:Default".to_string()][..])
            );
            assert_eq!(jobs[0].dirty_visit_count, Some(321));
            assert_eq!(
                jobs[0].dirty_date_keys.as_deref(),
                Some(&["2024-04-02".to_string(), "2024-04-03".to_string()][..])
            );
            assert!(jobs[0].heartbeat_at.is_some());
            Ok(())
        })
        .unwrap();
}

#[test]
fn job_control_state_transitions_persist_success_failure_and_worker_cancel() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let success_id = enqueue_deterministic_rebuild_job_in_session(
                connection,
                &CoreIntelligenceRebuildRequest {
                    profile_id: Some("profile-success".to_string()),
                    ..CoreIntelligenceRebuildRequest::default()
                },
                "success fixture",
            )
            .expect("enqueue success");
            claim_deterministic_rebuild_job_in_session(connection, success_id)
                .expect("claim success");
            assert!(
                mark_intelligence_job_succeeded_in_session(
                    connection,
                    success_id,
                    &json!({ "status": "complete", "processedVisits": 3 }),
                )
                .expect("mark success")
            );
            assert!(
                !mark_intelligence_job_succeeded_in_session(connection, success_id, &json!({}))
                    .expect("second success should not update")
            );

            let failed_id = enqueue_deterministic_rebuild_job_in_session(
                connection,
                &CoreIntelligenceRebuildRequest {
                    profile_id: Some("profile-failed".to_string()),
                    ..CoreIntelligenceRebuildRequest::default()
                },
                "failure fixture",
            )
            .expect("enqueue failed");
            claim_deterministic_rebuild_job_in_session(connection, failed_id)
                .expect("claim failed");
            assert!(
                mark_intelligence_job_failed_in_session(connection, failed_id, "stage failed")
                    .expect("mark failed")
            );
            assert!(
                !mark_intelligence_job_failed_in_session(connection, failed_id, "again")
                    .expect("second failed should not update")
            );

            let cancelled_id = enqueue_deterministic_rebuild_job_in_session(
                connection,
                &CoreIntelligenceRebuildRequest {
                    profile_id: Some("profile-cancelled".to_string()),
                    ..CoreIntelligenceRebuildRequest::default()
                },
                "cancel fixture",
            )
            .expect("enqueue cancel");
            claim_deterministic_rebuild_job_in_session(connection, cancelled_id)
                .expect("claim cancel");
            connection
                .execute(
                    "UPDATE intelligence_jobs SET stop_requested = 1 WHERE id = ?1",
                    [cancelled_id],
                )
                .expect("request stop");
            assert!(
                !mark_intelligence_job_succeeded_in_session(connection, cancelled_id, &json!({}))
                    .expect("stop-requested success should not update")
            );
            assert!(
                !mark_intelligence_job_failed_in_session(connection, cancelled_id, "cancelled")
                    .expect("stop-requested failure should not update")
            );
            assert!(
                mark_running_intelligence_job_cancelled_in_session(
                    connection,
                    cancelled_id,
                    "cancelled from test"
                )
                .expect("mark cancelled")
            );
            assert!(
                !mark_running_intelligence_job_cancelled_in_session(
                    connection,
                    failed_id,
                    "already failed"
                )
                .expect("failed job should not cancel")
            );

            let rows = connection
        .prepare(
            "SELECT id, state, last_error, cancellation_reason FROM intelligence_jobs ORDER BY id",
        )
        .expect("prepare state query")
        .query_map((), |row| {
            Ok((
                row.get::< i64>(0)?,
                row.get::< String>(1)?,
                row.get::< Option<String>>(2)?,
                row.get::< Option<String>>(3)?,
            ))
        })
        .expect("query states")
        .collect::<vault_store::Result<Vec<_>>>()
        .expect("collect states");
            assert_eq!(rows[0].1, "succeeded");
            assert_eq!(rows[1].1, "failed");
            assert_eq!(rows[1].2.as_deref(), Some("stage failed"));
            assert_eq!(rows[2].1, "cancelled");
            assert_eq!(rows[2].3.as_deref(), Some("cancelled from test"));
            Ok(())
        })
        .unwrap();
}

#[test]
fn deterministic_rebuild_jobs_run_before_optional_enrichment() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            enqueue_enrichment_job(
                connection,
                7,
                &BUILT_IN_ENRICHMENT_PLUGINS[1],
                &EnrichmentJobPayload {
                    history_id: Some(3),
                    profile_id: Some("chrome:Default".to_string()),
                    url: "https://example.com/docs".to_string(),
                    title: Some("Docs".to_string()),
                },
            )
            .expect("enqueue enrichment");
            let enrichment_job_id = next_queued_enrichment_job_in_session(connection)
                .expect("next enrichment job")
                .expect("enrichment");

            let deterministic_job_id = enqueue_deterministic_rebuild_job_in_session(
                connection,
                &CoreIntelligenceRebuildRequest::default(),
                "Archive changed.",
            )
            .expect("enqueue deterministic rebuild");

            let next_job = next_queued_intelligence_job_in_session(connection)
                .expect("next queued job")
                .expect("queued job");
            assert_eq!(next_job.id, deterministic_job_id);
            assert_eq!(next_job.job_type, FULL_REBUILD_JOB_TYPE);
            assert_ne!(enrichment_job_id, deterministic_job_id);
            Ok(())
        })
        .unwrap();
}

#[test]
fn plugin_enabled_respects_global_toggle() {
    let mut config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Encrypted,
        ..AppConfig::default()
    };
    config.ai.enrichment_enabled = false;
    assert!(!enrichment_plugin_enabled(&config, TITLE_NORMALIZATION_PLUGIN_ID));
    config.ai.enrichment_enabled = true;
    config.enrichment.plugins.clear();
    config.ai.enrichment_plugins = vec![EnrichmentPluginPreference {
        plugin_id: TITLE_NORMALIZATION_PLUGIN_ID.to_string(),
        enabled: true,
    }];
    assert!(enrichment_plugin_enabled(&config, TITLE_NORMALIZATION_PLUGIN_ID));
    assert!(!enrichment_plugin_enabled(&config, "unknown-plugin"));
}

#[test]
fn running_jobs_can_be_requeued_globally_or_by_run() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let now = crate::utils::now_rfc3339();
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, updated_at)
             VALUES (?1, ?2, 11, 'running', 10, 1, 'job-1', ?3, '{}', ?4, ?4, ?4, ?4),
                    (?1, ?2, 12, 'running', 10, 1, 'job-2', ?3, '{}', ?4, ?4, ?4, ?4)",
                    params![
                        ENRICHMENT_JOB_TYPE,
                        TITLE_NORMALIZATION_PLUGIN_ID,
                        serde_json::to_string(&EnrichmentJobPayload {
                            history_id: Some(3),
                            profile_id: Some("chrome:Default".to_string()),
                            url: "https://example.com/docs".to_string(),
                            title: Some("Docs".to_string()),
                        })
                        .expect("payload"),
                        now,
                    ],
                )
                .expect("insert running jobs");

            let requeued_for_run =
                requeue_running_enrichment_jobs_for_run(connection, 11).expect("requeue run jobs");
            assert_eq!(requeued_for_run, 1);
            let first_state = connection
                .query_one(
                    "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-1'",
                    (),
                    |row| Ok(row.get::<String>(0)?),
                )
                .expect("first state");
            let second_state = connection
                .query_one(
                    "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-2'",
                    (),
                    |row| Ok(row.get::<String>(0)?),
                )
                .expect("second state");
            assert_eq!(first_state, "queued");
            assert_eq!(second_state, "running");

            let requeued_all = super::recovery::requeue_running_enrichment_jobs(connection)
                .expect("requeue all running jobs");
            assert_eq!(requeued_all, 1);
            let final_state = connection
                .query_one(
                    "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-2'",
                    (),
                    |row| Ok(row.get::<String>(0)?),
                )
                .expect("final state");
            assert_eq!(final_state, "queued");
            Ok(())
        })
        .unwrap();
}

#[test]
fn claim_enrichment_jobs_scans_past_disallowed_queue_fronts() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let now = crate::utils::now_rfc3339();
            let payload = |history_id: i64| {
                serde_json::to_string(&EnrichmentJobPayload {
                    history_id: Some(history_id),
                    profile_id: Some("chrome:Default".to_string()),
                    url: format!("https://example.com/{history_id}"),
                    title: Some(format!("Title {history_id}")),
                })
                .expect("payload")
            };

            for history_id in 1..=6 {
                connection
                    .execute(
                        "INSERT INTO intelligence_jobs
                 (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key,
                  payload_json, artifact_json, created_at, scheduled_at, updated_at)
                 VALUES (?1, ?2, ?3, 'queued', 10, 0, ?4, ?5, '{}', ?6, ?6, ?6)",
                        params![
                            ENRICHMENT_JOB_TYPE,
                            TITLE_NORMALIZATION_PLUGIN_ID,
                            history_id,
                            format!("job-blocked-{history_id}"),
                            payload(history_id),
                            now,
                        ],
                    )
                    .expect("insert blocked queued job");
            }
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, updated_at)
             VALUES (?1, ?2, 99, 'queued', 10, 0, 'job-target', ?3, '{}', ?4, ?4, ?4)",
                    params![ENRICHMENT_JOB_TYPE, TITLE_NORMALIZATION_PLUGIN_ID, payload(99), now,],
                )
                .expect("insert target job");

            let claimed = claim_enrichment_jobs(
                connection,
                &[TITLE_NORMALIZATION_PLUGIN_ID.to_string()],
                &std::collections::HashSet::from([99]),
                1,
            )
            .expect("claim enrichment jobs");

            assert_eq!(claimed.len(), 1);
            assert_eq!(claimed[0].payload.history_id, Some(99));
            let state = connection
                .query_one(
                    "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-target'",
                    (),
                    |row| Ok(row.get::<String>(0)?),
                )
                .expect("job state");
            assert_eq!(state, "running");
            Ok(())
        })
        .unwrap();
}

#[test]
fn compare_and_set_claim_skips_jobs_taken_by_another_connection() {
    for key in [None, Some([79; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let id = vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |s| {
                let payload = EnrichmentJobPayload {
                    history_id: Some(42),
                    profile_id: Some("chrome:Default".into()),
                    url: "https://example.com/race".into(),
                    title: Some("Race".into()),
                };
                enqueue_enrichment_job(s, 11, &BUILT_IN_ENRICHMENT_PLUGINS[0], &payload)?;
                Ok(queued_enrichment_candidates_page(
                    s,
                    &[TITLE_NORMALIZATION_PLUGIN_ID.into()],
                    None,
                    1,
                )?
                .remove(0)
                .id)
            })
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let vault = vault.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    vault
                        .write(
                            vault_store::Db::Intelligence,
                            vault_store::Priority::Interactive,
                            move |s| try_claim_enrichment_job(s, id, &crate::utils::now_rfc3339()),
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let won = handles.into_iter().map(|h| h.join().unwrap()).filter(|won| *won).count();
        assert_eq!(won, 1);
        let claimed = vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, move |s| {
                try_claim_enrichment_job(s, id, &crate::utils::now_rfc3339())
            })
            .unwrap();
        assert!(!claimed);
        let attempt = vault
            .store()
            .read(vault_store::Db::Intelligence, move |s| {
                s.query_one("SELECT attempt FROM intelligence_jobs WHERE id=?1", [id], |r| {
                    Ok(r.get::<i64>(0)?)
                })
            })
            .unwrap();
        assert_eq!(attempt, 1);
        cancel_intelligence_job(&vault, id).unwrap();
        let unavailable = vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, move |s| {
                claim_enrichment_job_by_id(s, id)
            })
            .unwrap();
        assert!(unavailable.is_none());
    }
}

#[test]
fn retry_and_cancel_require_valid_job_states() {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    vault
        .write(
            vault_store::Db::Intelligence,
            vault_store::Priority::Interactive,
            move |connection| {
                let now = crate::utils::now_rfc3339();
                connection
                    .execute(
                        "INSERT INTO intelligence_jobs
	             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
	              artifact_json, created_at, scheduled_at, updated_at)
	             VALUES
	                (?1, ?2, 21, 'succeeded', 10, 1, 'job-succeeded', ?3, '{}', ?4, ?4, ?4),
	                (?1, ?2, 22, 'queued', 10, 0, 'job-queued', ?3, '{}', ?4, ?4, ?4),
	                (?1, ?2, 23, 'failed', 10, 1, 'job-failed', ?3, '{}', ?4, ?4, ?4),
	                (?1, ?2, 24, 'cancelled', 10, 1, 'job-cancelled', ?3, '{}', ?4, ?4, ?4),
	                (?1, ?2, 25, 'running', 10, 1, 'job-running', ?3, '{}', ?4, ?4, ?4)",
                        params![
                            ENRICHMENT_JOB_TYPE,
                            TITLE_NORMALIZATION_PLUGIN_ID,
                            serde_json::to_string(&EnrichmentJobPayload {
                                history_id: Some(9),
                                profile_id: Some("chrome:Default".to_string()),
                                url: "https://example.com/docs".to_string(),
                                title: Some("Docs".to_string()),
                            })
                            .expect("payload"),
                            now,
                        ],
                    )
                    .expect("insert jobs");

                let retry_error = retry_intelligence_job_in_session(connection, &paths, &config, 1)
                    .expect_err("retry should reject succeeded jobs");
                assert!(retry_error.to_string().contains("cannot be retried"));

                let cancel_error =
                    cancel_intelligence_job_in_session(connection, &paths, &config, 1)
                        .expect_err("cancel should reject succeeded jobs");
                assert!(cancel_error.to_string().contains("cannot be cancelled"));

                cancel_intelligence_job_in_session(connection, &paths, &config, 2)
                    .expect("cancel queued job");
                let cancelled_state = connection
                    .query_one("SELECT state FROM intelligence_jobs WHERE id = 2", (), |row| {
                        Ok(row.get::<String>(0)?)
                    })
                    .expect("cancelled state");
                assert_eq!(cancelled_state, "cancelled");

                retry_intelligence_job_in_session(connection, &paths, &config, 3)
                    .expect("retry failed job");
                retry_intelligence_job_in_session(connection, &paths, &config, 4)
                    .expect("retry cancelled job");
                let retried_states = connection
                    .prepare(
                        "SELECT id, state FROM intelligence_jobs WHERE id IN (3, 4) ORDER BY id",
                    )
                    .expect("prepare retried states")
                    .query_map((), |row| Ok((row.get::<i64>(0)?, row.get::<String>(1)?)))
                    .expect("query retried states")
                    .collect::<vault_store::Result<Vec<_>>>()
                    .expect("collect retried states");
                assert_eq!(
                    retried_states,
                    vec![(3, "queued".to_string()), (4, "queued".to_string())]
                );

                cancel_intelligence_job_in_session(connection, &paths, &config, 5)
                    .expect("cancel running job");
                let running_cancel: (String, i64) = connection
                    .query_one(
                        "SELECT state, stop_requested FROM intelligence_jobs WHERE id = 5",
                        (),
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .expect("running cancel state");
                assert_eq!(running_cancel, ("running".to_string(), 1));
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn retry_and_cancel_fail_for_missing_jobs() {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths().clone();
    let config = vault.config().clone();
    vault
        .write(
            vault_store::Db::Intelligence,
            vault_store::Priority::Interactive,
            move |connection| {
                let retry_error =
                    retry_intelligence_job_in_session(connection, &paths, &config, 404)
                        .expect_err("retry should fail for missing jobs");
                assert!(retry_error.to_string().contains("404"));

                let cancel_error =
                    cancel_intelligence_job_in_session(connection, &paths, &config, 404)
                        .expect_err("cancel should fail for missing jobs");
                assert!(cancel_error.to_string().contains("404"));
                Ok(())
            },
        )
        .unwrap();
}

#[test]
fn next_queued_intelligence_job_recovers_expired_leases() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.write(vault_store::Db::Intelligence,vault_store::Priority::Interactive, |connection| {
    let expired_at = (Utc::now() - Duration::minutes(10)).to_rfc3339();
    let now = crate::utils::now_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, heartbeat_at, lease_owner,
              lease_expires_at, updated_at, stop_requested)
             VALUES
                (?1, NULL, NULL, 'running', ?2, 1, 'expired-queued', ?3, '{}', ?4, ?4, ?4, ?4, 'worker', ?5, ?4, 0),
                (?6, NULL, NULL, 'running', ?7, 1, 'expired-cancelled', ?8, '{}', ?4, ?4, ?4, ?4, 'worker', ?5, ?4, 1)",
            params![
                STRUCTURAL_REBUILD_JOB_TYPE,
                STRUCTURAL_REBUILD_PRIORITY,
                serde_json::to_string(&DeterministicRebuildJobPayload {
                    job_type: STRUCTURAL_REBUILD_JOB_TYPE.to_string(),
                    request: CoreIntelligenceRebuildRequest::default(),
                    reason: "expired lease".to_string(),
                })
                .expect("payload"),
                now,
                expired_at,
                DAILY_ROLLUP_JOB_TYPE,
                DAILY_ROLLUP_PRIORITY,
                serde_json::to_string(&DeterministicRebuildJobPayload {
                    job_type: DAILY_ROLLUP_JOB_TYPE.to_string(),
                    request: CoreIntelligenceRebuildRequest::default(),
                    reason: "expired cancelled lease".to_string(),
                })
                .expect("payload"),
            ],
        )
        .expect("insert expired jobs");

    let next_job = next_queued_intelligence_job_in_session(connection)
        .expect("next queued job after recovery")
        .expect("recovered queued job");
    assert_eq!(next_job.job_type, STRUCTURAL_REBUILD_JOB_TYPE);

    let states = connection
        .prepare(
            "SELECT dedupe_key, state, last_error
             FROM intelligence_jobs
             WHERE dedupe_key IN ('expired-queued', 'expired-cancelled')
             ORDER BY dedupe_key ASC",
        )
        .expect("prepare recovered states")
        .query_map((), |row| {
            Ok((
                row.get::< String>(0)?,
                row.get::< String>(1)?,
                row.get::< Option<String>>(2)?,
            ))
        })
        .expect("query recovered states")
        .collect::<vault_store::Result<Vec<_>>>()
        .expect("collect recovered states");
    assert_eq!(states[0].0, "expired-cancelled");
    assert_eq!(states[0].1, "cancelled");
    assert_eq!(states[1].0, "expired-queued");
    assert_eq!(states[1].1, "queued");
    assert!(
        states[1].2.as_deref().is_some_and(|value| value.contains("expired intelligence lease"))
    );
    Ok(())
    }).unwrap();
}

#[test]
fn content_fetch_jobs_dedupe_by_canonical_url_and_claim_by_type() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let payload = EnrichmentJobPayload {
                history_id: Some(1),
                profile_id: Some("chrome:Default".to_string()),
                url: "https://github.com/o/r?utm_source=x".to_string(),
                title: Some("o/r".to_string()),
            };
            let first = enqueue_content_fetch_job(connection, &payload).expect("enqueue");
            // A second enqueue of the SAME canonical URL (different tracking param) dedupes to the same job.
            let dup_payload = EnrichmentJobPayload {
                url: "https://github.com/o/r".to_string(),
                history_id: Some(2),
                ..payload.clone()
            };
            let second = enqueue_content_fetch_job(connection, &dup_payload).expect("enqueue dup");
            assert_eq!(first, second, "same canonical URL must dedupe to one content-fetch job");
            let job_count: i64 = connection
                .query_one(
                    "SELECT COUNT(*) FROM intelligence_jobs WHERE job_type = ?1",
                    [CONTENT_FETCH_JOB_TYPE],
                    |row| Ok(row.get(0)?),
                )
                .expect("count");
            assert_eq!(job_count, 1);

            // The next-due query surfaces it, and an enrichment-type claim must NOT claim a content-fetch job.
            let due = next_queued_content_fetch_job_in_session(connection)
                .expect("next due")
                .expect("a due job");
            assert_eq!(due, first);
            assert!(
                claim_enrichment_job_by_id(connection, first).expect("type-scoped claim").is_none(),
                "an enrichment-type claim must not claim a content-fetch job",
            );
            // The content-fetch claim DOES claim it.
            let claimed = claim_content_fetch_job_by_id(connection, first)
                .expect("claim")
                .expect("claimed job");
            assert_eq!(claimed.id, first);
            assert_eq!(claimed.plugin_id, CONTENT_FETCH_PLUGIN_ID);
            assert_eq!(
                claimed.payload.history_id,
                Some(2),
                "payload refreshed to the freshest visit"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn content_fetch_job_due_respects_version_and_negative_cache() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {

            // No row → due.
            assert!(
                content_fetch_job_due(connection, "1", "github-repo", 1)
                    .expect("due when unfetched")
            );

            // A version-matching row with NO refetch_after (a success) → NOT due.
            connection
                .execute(
                    "INSERT INTO page_content_enrichments
             (canonical_url, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
              pipeline_version, extractor_version, enrichment_summary, refetch_after)
             VALUES (1, 'github-repo', 'success', '2026-06-21T00:00:00Z', '[]', '{}', 'v1', 1,
                     's', NULL)",
                    (),
                )
                .expect("insert success row");
            assert!(
                !content_fetch_job_due(connection, "1", "github-repo", 1)
                    .expect("not due after success")
            );

            // A bumped extractor version → due again (bounded refetch of this source).
            assert!(
                content_fetch_job_due(connection, "1", "github-repo", 2)
                    .expect("due after version bump")
            );

            // A failure row whose refetch_after is in the FUTURE → NOT due (negative cache cooling).
            connection
                .execute(
                    "INSERT INTO page_content_enrichments
             (canonical_url, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
              pipeline_version, extractor_version, enrichment_summary, refetch_after)
             VALUES (2, 'github-repo', 'fetch-error', '2026-06-21T00:00:00Z', '[]', '{}', 'v1', 1,
                     NULL, '2999-01-01T00:00:00Z')",
                    (),
                )
                .expect("insert future-refetch row");
            assert!(
                !content_fetch_job_due(connection, "2", "github-repo", 1)
                    .expect("not due while cooling")
            );

            // A failure row whose refetch_after is in the PAST → due again.
            connection
                .execute(
                    "UPDATE page_content_enrichments SET refetch_after = '2000-01-01T00:00:00Z'
             WHERE canonical_url = 2",
                    (),
                )
                .expect("expire refetch_after");
            assert!(
                content_fetch_job_due(connection, "2", "github-repo", 1)
                    .expect("due after cooldown")
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn content_fetch_job_due_compares_timestamps_as_instants_at_the_boundary_second() {
    // CORR-4: `refetch_after` is `...Secs, true` (`...56Z`) while "now" carries fractional secs + a
    // numeric offset (`...56.789+00:00`); a lexical compare ('Z' > '.') would mis-read a due row as
    // not-due at the boundary second. Parsing to instants fixes it: a refetch_after that is the SAME
    // wall-clock second as (but slightly before) now must read DUE.
    let vault = crate::Vault::open_temp().unwrap();
    vault.write(vault_store::Db::Intelligence,vault_store::Priority::Interactive, |connection| {

    // refetch_after = one minute in the PAST, rounded to whole seconds (the `...Z` shape).
    let past = (chrono::Utc::now() - chrono::Duration::seconds(60))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    connection
        .execute(
            "INSERT INTO page_content_enrichments
             (canonical_url, content_source, fetch_status, fetched_at, snippet_json, extraction_json,
              pipeline_version, extractor_version, enrichment_summary, refetch_after)
             VALUES (9, 'github-repo', 'fetch-error', '2026-06-21T00:00:00Z', '[]', '{}', 'v1', 1,
                     NULL, ?1)",
            [past.as_str()],
        )
        .expect("insert boundary row");
    assert!(
        content_fetch_job_due(connection, "9", "github-repo", 1).expect("due"),
        "a refetch_after in the past must read DUE regardless of the Z/offset format mismatch"
    );

    // An UNPARSEABLE refetch_after is treated as due (fail toward refetching, never wedged).
    connection
        .execute(
            "UPDATE page_content_enrichments SET refetch_after = 'not-a-timestamp' WHERE canonical_url = 9",
            (),
        )
        .expect("garble refetch_after");
    assert!(content_fetch_job_due(connection, "9", "github-repo", 1).expect("due on bad stamp"));
    Ok(())
    }).unwrap();
}

#[test]
fn requeue_content_fetch_job_after_returns_a_running_row_to_queued() {
    // SEC-2: a rate-limited (running) job is requeued with a FUTURE scheduled_at, NOT cancelled, so the
    // queued-only drain re-picks it when due. A non-running row is a no-op.
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            let payload = EnrichmentJobPayload {
                history_id: Some(1),
                profile_id: Some("chrome:Default".to_string()),
                url: "https://github.com/o/r".to_string(),
                title: None,
            };
            let job_id = enqueue_content_fetch_job(connection, &payload).expect("enqueue");
            // Not running yet → requeue is a no-op (the WHERE state='running' guard).
            let future = "2999-01-01T00:00:00Z";
            assert!(
                !requeue_content_fetch_job_after(connection, job_id, future)
                    .expect("noop on queued")
            );

            // Claim → running, then requeue succeeds and parks it back to queued + future schedule.
            claim_content_fetch_job_by_id(connection, job_id).expect("claim").expect("claimed");
            assert!(requeue_content_fetch_job_after(connection, job_id, future).expect("requeue"));
            let (state, scheduled, lease): (String, String, Option<String>) = connection
                .query_one(
                    "SELECT state, scheduled_at, lease_owner FROM intelligence_jobs WHERE id = ?1",
                    [job_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("row");
            assert_eq!(state, "queued");
            assert_eq!(scheduled, future);
            assert!(lease.is_none(), "the lease is cleared on requeue");
            Ok(())
        })
        .unwrap();
}

#[test]
fn next_content_fetch_schedule_eta_reports_soonest_future_then_none() {
    // SEC-2 worker hook: a future-scheduled queued content-fetch job surfaces its ETA; an empty queue
    // (or only DUE rows) reports None.
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            // Empty queue → None.
            assert_eq!(
                next_content_fetch_schedule_eta_secs_in_session(connection).expect("eta empty"),
                None
            );

            let payload = EnrichmentJobPayload {
                history_id: Some(1),
                profile_id: Some("chrome:Default".to_string()),
                url: "https://github.com/o/r".to_string(),
                title: None,
            };
            let job_id = enqueue_content_fetch_job(connection, &payload).expect("enqueue");
            // A DUE (now-scheduled) job is NOT a future deferral → None.
            assert_eq!(
                next_content_fetch_schedule_eta_secs_in_session(connection).expect("eta due"),
                None
            );

            // Defer it ~30s into the future → the ETA is reported.
            let future = (chrono::Utc::now() + chrono::Duration::seconds(30))
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            connection
                .execute(
                    "UPDATE intelligence_jobs SET scheduled_at = ?1 WHERE id = ?2",
                    params![future, job_id],
                )
                .expect("defer");
            let eta = next_content_fetch_schedule_eta_secs_in_session(connection)
                .expect("eta future")
                .expect("some");
            assert!((1..=31).contains(&eta), "deferred ETA should be ~30s, got {eta}");

            // An unparseable scheduled_at is treated as due-now (floor of 1s) rather than wedging the lane.
            connection
                .execute(
                    "UPDATE intelligence_jobs SET scheduled_at = 'bad' WHERE id = ?1",
                    [job_id],
                )
                .expect("garble scheduled_at");
            // `next_queued_content_fetch_job`-style gating compares strings, so 'bad' > now_rfc3339 lexically,
            // surfacing it as a future row whose parse fails → ETA floor of 1.
            assert_eq!(
                next_content_fetch_schedule_eta_secs_in_session(connection).expect("eta bad"),
                Some(1)
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn job_history_pages_across_equal_update_times_and_seeks_to_the_cursor() {
    let vault = crate::Vault::open_temp().unwrap();
    vault
        .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |connection| {
            // Job 1 is the newest; jobs 2 to 9 share one update time, so pages split a tie.
            for (id, updated_at) in
                std::iter::once((1, "2026-01-02")).chain((2..=9).map(|id| (id, "2026-01-01")))
            {
                connection.execute(
                    "INSERT INTO intelligence_jobs(id,job_type,state,dedupe_key,payload_json,created_at,scheduled_at,updated_at)
                     VALUES(?1,'content-fetch','succeeded',?2,'{}',?3,?3,?3)",
                    params![id, format!("job:{id}"), updated_at],
                )?;
            }
            Ok(())
        })
        .unwrap();
    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        if cursor.is_some() {
            vault.store().capture_query_plans(vault_store::Db::Intelligence, true).unwrap();
        }
        let page = list_intelligence_jobs(&vault, cursor.clone(), 2).unwrap();
        vault.store().capture_query_plans(vault_store::Db::Intelligence, false).unwrap();
        // Later pages seek idx_intelligence_jobs_recent to the cursor instead of walking it
        // from the newest job, which made paging the whole history quadratic.
        let plans = vault.store().take_query_plans(vault_store::Db::Intelligence).unwrap();
        let plans: Vec<_> =
            plans.into_iter().filter(|plan| plan.sql.contains("FROM intelligence_jobs")).collect();
        assert_eq!(plans.len(), usize::from(cursor.is_some()));
        for plan in plans {
            assert!(
                plan.steps.iter().any(|step| step.starts_with("SEARCH intelligence_jobs")
                    && step.contains("idx_intelligence_jobs_recent (updated_at")),
                "{plan:?}"
            );
        }
        seen.extend(page.items.into_iter().map(|job| job.id));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, vec![1, 9, 8, 7, 6, 5, 4, 3, 2]);
}
