use super::*;
use crate::{
    archive::open_intelligence_connection,
    config::{ProjectPaths, ensure_paths, project_paths_with_root},
    models::{AppConfig, ArchiveMode},
};
use rusqlite::{Connection, params};
use serde_json::json;
use tempfile::tempdir;

use super::{
    claims::{queued_enrichment_candidates_page, try_claim_enrichment_job},
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
                (?1, ?2, 22, 'queued', 10, 0, 'job-queued', ?3, '{}', ?4, ?4, ?4)",
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
