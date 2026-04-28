use super::*;
use crate::{
    VISIT_DERIVED_FACTS_MODULE_ID,
    archive::open_intelligence_connection,
    config::{ProjectPaths, ensure_paths, project_paths_with_root},
    models::{AppConfig, ArchiveMode, EnrichmentPluginPreference},
};
use rusqlite::{Connection, params};
use serde_json::json;
use tempfile::tempdir;

use super::{
    claims::{
        claim_enrichment_job_by_id, queued_enrichment_candidates_page, try_claim_enrichment_job,
    },
    recovery::requeue_running_enrichment_jobs_for_run,
    snapshot::load_queue_status,
};

fn sample_paths(root: &std::path::Path) -> ProjectPaths {
    project_paths_with_root(root)
}

fn setup_runtime_archive() -> (tempfile::TempDir, ProjectPaths, AppConfig) {
    let root = tempdir().expect("tempdir");
    let paths = sample_paths(root.path());
    ensure_paths(&paths).expect("ensure paths");
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
    ensure_intelligence_runtime_schema(&connection).expect("runtime schema");
    (root, paths, config)
}

#[test]
fn queue_jobs_can_be_enqueued_and_loaded() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");
    enqueue_enrichment_job(
        &connection,
        7,
        &BUILT_IN_ENRICHMENT_PLUGINS[0],
        &EnrichmentJobPayload {
            history_id: 3,
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/docs".to_string(),
            title: Some("Docs".to_string()),
        },
    )
    .expect("enqueue");

    let queue = load_queue_status(&connection).expect("queue status");
    assert_eq!(queue.queued, 1);

    let jobs = super::snapshot::load_recent_jobs(&connection).expect("recent jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].plugin_id.as_deref(), Some(TITLE_NORMALIZATION_PLUGIN_ID));
}

#[test]
fn deterministic_rebuild_jobs_are_traced_in_runtime_queue() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");

    let job_id = enqueue_deterministic_rebuild_job(
        &connection,
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("chrome:Default".to_string()),
            full_rebuild: true,
            limit: None,
        },
        "Archive changed.",
    )
    .expect("enqueue deterministic rebuild");

    let queue = load_queue_status(&connection).expect("queue status");
    assert_eq!(queue.queued, 1);

    let claimed =
        claim_deterministic_rebuild_job(&connection, job_id).expect("claim deterministic job");
    assert_eq!(
        claimed.expect("deterministic payload").request.profile_id.as_deref(),
        Some("chrome:Default")
    );

    let jobs = super::snapshot::load_recent_jobs(&connection).expect("recent jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_type, FULL_REBUILD_JOB_TYPE);
    assert_eq!(jobs[0].title.as_deref(), Some("chrome:Default · full Core Intelligence rebuild"));
}

#[test]
fn enqueue_runtime_helpers_dedupe_refresh_and_mark_stale_contracts() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");

    let invalid = enqueue_core_intelligence_job(
        &connection,
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
        enqueue_core_intelligence_job(&connection, VISIT_DERIVE_JOB_TYPE, &request, "first")
            .expect("enqueue first deterministic job");
    let refreshed_id =
        enqueue_core_intelligence_job(&connection, VISIT_DERIVE_JOB_TYPE, &request, "second")
            .expect("refresh deterministic job");
    assert_eq!(first_id, refreshed_id);
    connection
        .execute(
            "UPDATE intelligence_jobs SET state = 'running', last_error = 'old', stop_requested = 1 WHERE id = ?1",
            [first_id],
        )
        .expect("mark deterministic job running");
    let running_id =
        enqueue_core_intelligence_job(&connection, VISIT_DERIVE_JOB_TYPE, &request, "third")
            .expect("running deterministic job is deduped without reset");
    assert_eq!(running_id, first_id);
    let (state, last_error, stop_requested): (String, Option<String>, i64) = connection
        .query_row(
            "SELECT state, last_error, stop_requested FROM intelligence_jobs WHERE id = ?1",
            [first_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("load refreshed deterministic state");
    assert_eq!(state, "running");
    assert_eq!(last_error.as_deref(), Some("old"));
    assert_eq!(stop_requested, 1);
    let trigger_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM intelligence_job_triggers WHERE job_id = ?1",
            [first_id],
            |row| row.get(0),
        )
        .expect("deterministic trigger count");
    assert_eq!(trigger_count, 3);

    let enrichment_payload = EnrichmentJobPayload {
        history_id: 42,
        profile_id: "chrome:Default".to_string(),
        url: "https://example.com/docs".to_string(),
        title: Some("Docs".to_string()),
    };
    enqueue_enrichment_job(&connection, 7, &BUILT_IN_ENRICHMENT_PLUGINS[0], &enrichment_payload)
        .expect("enqueue enrichment");
    connection
        .execute(
            "UPDATE intelligence_jobs SET state = 'failed', last_error = 'old failure', stop_requested = 1 WHERE plugin_id = ?1",
            [TITLE_NORMALIZATION_PLUGIN_ID],
        )
        .expect("mark enrichment failed");
    enqueue_enrichment_job(&connection, 8, &BUILT_IN_ENRICHMENT_PLUGINS[0], &enrichment_payload)
        .expect("refresh enrichment");
    let (state, last_error, stop_requested): (String, Option<String>, i64) = connection
        .query_row(
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
    enqueue_enrichment_job(&connection, 9, &BUILT_IN_ENRICHMENT_PLUGINS[0], &enrichment_payload)
        .expect("dedupe running enrichment");
    let running_error: Option<String> = connection
        .query_row(
            "SELECT last_error FROM intelligence_jobs WHERE plugin_id = ?1",
            [TITLE_NORMALIZATION_PLUGIN_ID],
            |row| row.get(0),
        )
        .expect("running enrichment remains untouched");
    assert_eq!(running_error.as_deref(), Some("active"));

    persist_deterministic_module_runtime_updates(
        &connection,
        &[
            DeterministicModuleRuntimeUpdate {
                module_id: "unknown-module".to_string(),
                status: "stale".to_string(),
                last_run_id: None,
                last_built_at: None,
                last_invalidated_at: Some("2026-04-26T00:00:00Z".to_string()),
                stale_reason: Some("ignored".to_string()),
                notes: vec!["ignored".to_string()],
            },
            DeterministicModuleRuntimeUpdate {
                module_id: VISIT_DERIVED_FACTS_MODULE_ID.to_string(),
                status: "ready".to_string(),
                last_run_id: Some(99),
                last_built_at: Some("2026-04-26T00:00:00Z".to_string()),
                last_invalidated_at: None,
                stale_reason: None,
                notes: vec!["fresh".to_string()],
            },
        ],
    )
    .expect("persist module updates");
    let module_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM deterministic_module_runtime", [], |row| row.get(0))
        .expect("module runtime count");
    assert_eq!(module_count, 1);

    mark_all_deterministic_modules_stale(&connection, "coverage stale")
        .expect("mark deterministic modules stale");
    let stale_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM deterministic_module_runtime WHERE status = 'stale' AND stale_reason = 'coverage stale'",
            [],
            |row| row.get(0),
        )
        .expect("stale module count");
    assert_eq!(stale_count, built_in_deterministic_modules().len() as i64);
}

#[test]
fn recent_jobs_surface_progress_for_running_deterministic_rebuilds() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");

    let job_id = enqueue_deterministic_rebuild_job(
        &connection,
        &CoreIntelligenceRebuildRequest::default(),
        "Archive changed.",
    )
    .expect("enqueue deterministic rebuild");
    claim_deterministic_rebuild_job(&connection, job_id).expect("claim deterministic rebuild");
    update_intelligence_job_artifact(
        &connection,
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

    let jobs = super::snapshot::load_recent_jobs(&connection).expect("recent jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].progress_label.as_deref(), Some("Scoring visits"));
    assert_eq!(jobs[0].progress_detail.as_deref(), Some("12,000 / 64,781 visits"));
    assert_eq!(jobs[0].progress_current, Some(12_000));
    assert_eq!(jobs[0].progress_total, Some(64_781));
    assert_eq!(jobs[0].progress_percent, Some(43.5));
    assert_eq!(jobs[0].execution_mode.as_deref(), Some("incremental"));
    assert_eq!(jobs[0].affected_profiles.as_deref(), Some(&["chrome:Default".to_string()][..]));
    assert_eq!(jobs[0].dirty_visit_count, Some(321));
    assert_eq!(
        jobs[0].dirty_date_keys.as_deref(),
        Some(&["2024-04-02".to_string(), "2024-04-03".to_string()][..])
    );
    assert!(jobs[0].heartbeat_at.is_some());
}

#[test]
fn job_control_state_transitions_persist_success_failure_and_worker_cancel() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");

    let success_id = enqueue_deterministic_rebuild_job(
        &connection,
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("profile-success".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        "success fixture",
    )
    .expect("enqueue success");
    claim_deterministic_rebuild_job(&connection, success_id).expect("claim success");
    assert!(
        mark_intelligence_job_succeeded(
            &connection,
            success_id,
            &json!({ "status": "complete", "processedVisits": 3 }),
        )
        .expect("mark success")
    );
    assert!(
        !mark_intelligence_job_succeeded(&connection, success_id, &json!({}))
            .expect("second success should not update")
    );

    let failed_id = enqueue_deterministic_rebuild_job(
        &connection,
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("profile-failed".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        "failure fixture",
    )
    .expect("enqueue failed");
    claim_deterministic_rebuild_job(&connection, failed_id).expect("claim failed");
    assert!(
        mark_intelligence_job_failed(&connection, failed_id, "stage failed").expect("mark failed")
    );
    assert!(
        !mark_intelligence_job_failed(&connection, failed_id, "again")
            .expect("second failed should not update")
    );

    let cancelled_id = enqueue_deterministic_rebuild_job(
        &connection,
        &CoreIntelligenceRebuildRequest {
            profile_id: Some("profile-cancelled".to_string()),
            ..CoreIntelligenceRebuildRequest::default()
        },
        "cancel fixture",
    )
    .expect("enqueue cancel");
    claim_deterministic_rebuild_job(&connection, cancelled_id).expect("claim cancel");
    connection
        .execute("UPDATE intelligence_jobs SET stop_requested = 1 WHERE id = ?1", [cancelled_id])
        .expect("request stop");
    assert!(
        !mark_intelligence_job_succeeded(&connection, cancelled_id, &json!({}))
            .expect("stop-requested success should not update")
    );
    assert!(
        !mark_intelligence_job_failed(&connection, cancelled_id, "cancelled")
            .expect("stop-requested failure should not update")
    );
    assert!(
        mark_running_intelligence_job_cancelled(&connection, cancelled_id, "cancelled from test")
            .expect("mark cancelled")
    );
    assert!(
        !mark_running_intelligence_job_cancelled(&connection, failed_id, "already failed")
            .expect("failed job should not cancel")
    );

    let rows = connection
        .prepare(
            "SELECT id, state, last_error, cancellation_reason FROM intelligence_jobs ORDER BY id",
        )
        .expect("prepare state query")
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .expect("query states")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect states");
    assert_eq!(rows[0].1, "succeeded");
    assert_eq!(rows[1].1, "failed");
    assert_eq!(rows[1].2.as_deref(), Some("stage failed"));
    assert_eq!(rows[2].1, "cancelled");
    assert_eq!(rows[2].3.as_deref(), Some("cancelled from test"));
}

#[test]
fn deterministic_rebuild_jobs_run_before_optional_enrichment() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");

    enqueue_enrichment_job(
        &connection,
        7,
        &BUILT_IN_ENRICHMENT_PLUGINS[1],
        &EnrichmentJobPayload {
            history_id: 3,
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/docs".to_string(),
            title: Some("Docs".to_string()),
        },
    )
    .expect("enqueue enrichment");
    let enrichment_job_id =
        next_queued_enrichment_job(&connection).expect("next enrichment job").expect("enrichment");

    let deterministic_job_id = enqueue_deterministic_rebuild_job(
        &connection,
        &CoreIntelligenceRebuildRequest::default(),
        "Archive changed.",
    )
    .expect("enqueue deterministic rebuild");

    let next_job =
        next_queued_intelligence_job(&connection).expect("next queued job").expect("queued job");
    assert_eq!(next_job.id, deterministic_job_id);
    assert_eq!(next_job.job_type, FULL_REBUILD_JOB_TYPE);
    assert_ne!(enrichment_job_id, deterministic_job_id);
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
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");
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
                    history_id: 3,
                    profile_id: "chrome:Default".to_string(),
                    url: "https://example.com/docs".to_string(),
                    title: Some("Docs".to_string()),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert running jobs");

    let requeued_for_run =
        requeue_running_enrichment_jobs_for_run(&connection, 11).expect("requeue run jobs");
    assert_eq!(requeued_for_run, 1);
    let first_state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-1'", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("first state");
    let second_state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-2'", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("second state");
    assert_eq!(first_state, "queued");
    assert_eq!(second_state, "running");

    let requeued_all = super::recovery::requeue_running_enrichment_jobs(&connection)
        .expect("requeue all running jobs");
    assert_eq!(requeued_all, 1);
    let final_state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-2'", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("final state");
    assert_eq!(final_state, "queued");
}

#[test]
fn claim_enrichment_jobs_scans_past_disallowed_queue_fronts() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");
    let now = crate::utils::now_rfc3339();
    let payload = |history_id: i64| {
        serde_json::to_string(&EnrichmentJobPayload {
            history_id,
            profile_id: "chrome:Default".to_string(),
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
        &connection,
        &[TITLE_NORMALIZATION_PLUGIN_ID.to_string()],
        &std::collections::HashSet::from([99]),
        1,
    )
    .expect("claim enrichment jobs");

    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].payload.history_id, 99);
    let state = connection
        .query_row(
            "SELECT state FROM intelligence_jobs WHERE dedupe_key = 'job-target'",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("job state");
    assert_eq!(state, "running");
}

#[test]
fn compare_and_set_claim_skips_jobs_taken_by_another_connection() {
    let root = tempdir().expect("tempdir");
    let database_path = root.path().join("queue.sqlite");
    let connection = Connection::open(&database_path).expect("open queue db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");
    let competing_connection = Connection::open(&database_path).expect("open competing queue db");
    let now = crate::utils::now_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, updated_at)
             VALUES (?1, ?2, 11, 'queued', 10, 0, 'job-race', ?3, '{}', ?4, ?4, ?4)",
            params![
                ENRICHMENT_JOB_TYPE,
                TITLE_NORMALIZATION_PLUGIN_ID,
                serde_json::to_string(&EnrichmentJobPayload {
                    history_id: 42,
                    profile_id: "chrome:Default".to_string(),
                    url: "https://example.com/race".to_string(),
                    title: Some("Race".to_string()),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert queued job");

    let snapshot = queued_enrichment_candidates_page(
        &connection,
        &[TITLE_NORMALIZATION_PLUGIN_ID.to_string()],
        None,
        1,
    )
    .expect("queued candidate page");
    assert_eq!(snapshot.len(), 1);

    let claimed_at = crate::utils::now_rfc3339();
    assert!(
        try_claim_enrichment_job(&competing_connection, snapshot[0].id, &claimed_at)
            .expect("competing claim"),
        "the first claimer should win"
    );
    assert!(
        !try_claim_enrichment_job(&connection, snapshot[0].id, &claimed_at)
            .expect("stale snapshot claim"),
        "stale snapshots must not double-claim the same job"
    );

    let (state, attempt) = connection
        .query_row(
            "SELECT state, attempt FROM intelligence_jobs WHERE dedupe_key = 'job-race'",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .expect("job state after competing claims");
    assert_eq!(state, "running");
    assert_eq!(attempt, 1);

    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, updated_at)
             VALUES (?1, ?2, 12, 'queued', 10, 0, 'job-blocked', ?3, '{}', ?4, ?4, ?4)",
            params![
                ENRICHMENT_JOB_TYPE,
                TITLE_NORMALIZATION_PLUGIN_ID,
                serde_json::to_string(&EnrichmentJobPayload {
                    history_id: 43,
                    profile_id: "chrome:Default".to_string(),
                    url: "https://example.com/blocked".to_string(),
                    title: Some("Blocked".to_string()),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert blocked queued job");
    let blocked_id = connection.last_insert_rowid();
    connection
        .execute(
            &format!(
                "CREATE TRIGGER block_enrichment_claim
             BEFORE UPDATE OF state ON intelligence_jobs
             WHEN OLD.id = {blocked_id} AND NEW.state = 'running'
             BEGIN
               SELECT RAISE(IGNORE);
             END"
            ),
            [],
        )
        .expect("claim blocker trigger");
    assert!(claim_enrichment_job_by_id(&connection, blocked_id).expect("blocked claim").is_none());
}

#[test]
fn retry_and_cancel_require_valid_job_states() {
    let (_root, paths, config) = setup_runtime_archive();
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
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
                    history_id: 9,
                    profile_id: "chrome:Default".to_string(),
                    url: "https://example.com/docs".to_string(),
                    title: Some("Docs".to_string()),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert jobs");

    let retry_error = retry_intelligence_job(&paths, &config, None, 1)
        .expect_err("retry should reject succeeded jobs");
    assert!(retry_error.to_string().contains("cannot be retried"));

    let cancel_error = cancel_intelligence_job(&paths, &config, None, 1)
        .expect_err("cancel should reject succeeded jobs");
    assert!(cancel_error.to_string().contains("cannot be cancelled"));

    cancel_intelligence_job(&paths, &config, None, 2).expect("cancel queued job");
    let cancelled_state = connection
        .query_row("SELECT state FROM intelligence_jobs WHERE id = 2", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("cancelled state");
    assert_eq!(cancelled_state, "cancelled");

    retry_intelligence_job(&paths, &config, None, 3).expect("retry failed job");
    retry_intelligence_job(&paths, &config, None, 4).expect("retry cancelled job");
    let retried_states = connection
        .prepare("SELECT id, state FROM intelligence_jobs WHERE id IN (3, 4) ORDER BY id")
        .expect("prepare retried states")
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .expect("query retried states")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect retried states");
    assert_eq!(retried_states, vec![(3, "queued".to_string()), (4, "queued".to_string())]);

    cancel_intelligence_job(&paths, &config, None, 5).expect("cancel running job");
    let running_cancel: (String, i64) = connection
        .query_row("SELECT state, stop_requested FROM intelligence_jobs WHERE id = 5", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .expect("running cancel state");
    assert_eq!(running_cancel, ("cancelled".to_string(), 1));
}

#[test]
fn retry_and_cancel_fail_for_missing_jobs() {
    let (_root, paths, config) = setup_runtime_archive();

    let retry_error = retry_intelligence_job(&paths, &config, None, 404)
        .expect_err("retry should fail for missing jobs");
    assert!(retry_error.to_string().contains("404"));

    let cancel_error = cancel_intelligence_job(&paths, &config, None, 404)
        .expect_err("cancel should fail for missing jobs");
    assert!(cancel_error.to_string().contains("404"));
}

#[test]
fn next_queued_intelligence_job_recovers_expired_leases() {
    let connection = Connection::open_in_memory().expect("memory db");
    ensure_intelligence_runtime_schema(&connection).expect("queue schema");
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

    let next_job = next_queued_intelligence_job(&connection)
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
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .expect("query recovered states")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect recovered states");
    assert_eq!(states[0].0, "expired-cancelled");
    assert_eq!(states[0].1, "cancelled");
    assert_eq!(states[1].0, "expired-queued");
    assert_eq!(states[1].1, "queued");
    assert!(
        states[1].2.as_deref().is_some_and(|value| value.contains("expired intelligence lease"))
    );
}
