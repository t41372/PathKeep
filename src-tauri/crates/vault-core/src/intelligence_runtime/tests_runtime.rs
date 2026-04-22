use super::*;
use crate::{
    archive::open_intelligence_connection,
    config::{ProjectPaths, ensure_paths, project_paths_with_root},
    models::{AppConfig, ArchiveMode},
    utils::test_env_lock,
};
use rusqlite::params;
use tempfile::tempdir;

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
fn runtime_snapshot_returns_plugin_defaults_before_archive_exists() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = tempdir().expect("tempdir");
    let paths = sample_paths(root.path());
    let config = AppConfig::default();

    let snapshot = load_intelligence_runtime(&paths, &config, None).expect("runtime snapshot");
    assert_eq!(snapshot.plugins.len(), 2);
    assert!(snapshot.notes.iter().any(|note| note.contains("Initialize the archive")));
}

#[test]
fn load_intelligence_runtime_recovers_interrupted_deterministic_jobs() {
    let (_root, paths, config) = setup_runtime_archive();
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
    let now = crate::utils::now_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, updated_at)
             VALUES (?1, NULL, NULL, 'running', 5, 1, 'core-intelligence:all:false:full', ?2,
                     '{}', ?3, ?3, ?3, ?3)",
            params![
                FULL_REBUILD_JOB_TYPE,
                serde_json::to_string(&DeterministicRebuildJobPayload {
                    job_type: FULL_REBUILD_JOB_TYPE.to_string(),
                    request: CoreIntelligenceRebuildRequest::default(),
                    reason: "recover me".to_string(),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert interrupted deterministic job");

    let snapshot = load_intelligence_runtime(&paths, &config, None).expect("load runtime snapshot");
    assert!(snapshot.notes.iter().any(|note| note.contains("Recovered 1 interrupted")));

    let (state, last_error) = connection
        .query_row(
            "SELECT state, last_error FROM intelligence_jobs WHERE job_type = ?1",
            [FULL_REBUILD_JOB_TYPE],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .expect("recovered job state");
    assert_eq!(state, "queued");
    assert!(
        last_error
            .expect("recovery note")
            .contains("restarted before this Core Intelligence job finished")
    );
}

#[test]
fn load_intelligence_runtime_recovers_interrupted_enrichment_jobs() {
    let (_root, paths, config) = setup_runtime_archive();
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
    let now = crate::utils::now_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, updated_at)
             VALUES (?1, ?2, 41, 'running', 10, 1, 'readable-content-refetch:1', ?3,
                     '{}', ?4, ?4, ?4, ?4)",
            params![
                ENRICHMENT_JOB_TYPE,
                READABLE_CONTENT_PLUGIN_ID,
                serde_json::to_string(&EnrichmentJobPayload {
                    history_id: 1,
                    profile_id: "chrome:Default".to_string(),
                    url: "https://example.com/article".to_string(),
                    title: Some("Example".to_string()),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert interrupted enrichment job");

    let snapshot = load_intelligence_runtime(&paths, &config, None).expect("load runtime snapshot");
    assert!(snapshot.notes.iter().any(|note| note.contains("Recovered 1 interrupted enrichment")));

    let state = connection
        .query_row(
            "SELECT state FROM intelligence_jobs WHERE job_type = ?1",
            [ENRICHMENT_JOB_TYPE],
            |row| row.get::<_, String>(0),
        )
        .expect("recovered job state");
    assert_eq!(state, "queued");
}

#[test]
fn load_intelligence_runtime_only_recovers_running_jobs_once_per_archive() {
    let (_root, paths, config) = setup_runtime_archive();
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open runtime archive");
    let now = crate::utils::now_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, updated_at)
             VALUES (?1, NULL, NULL, 'running', ?2, 1, 'core-intelligence:all:false:full', ?3,
                     '{}', ?4, ?4, ?4, ?4)",
            params![
                FULL_REBUILD_JOB_TYPE,
                FULL_REBUILD_PRIORITY,
                serde_json::to_string(&DeterministicRebuildJobPayload {
                    job_type: FULL_REBUILD_JOB_TYPE.to_string(),
                    request: CoreIntelligenceRebuildRequest::default(),
                    reason: "recover me".to_string(),
                })
                .expect("payload"),
                now,
            ],
        )
        .expect("insert interrupted deterministic job");

    let first_snapshot =
        load_intelligence_runtime(&paths, &config, None).expect("first runtime snapshot");
    assert!(
        first_snapshot
            .notes
            .iter()
            .any(|note| note.contains("Recovered 1 interrupted deterministic"))
    );

    connection
        .execute(
            "UPDATE intelligence_jobs
             SET state = 'running',
                 started_at = ?1,
                 updated_at = ?1,
                 last_error = NULL
             WHERE job_type = ?2",
            params![crate::utils::now_rfc3339(), FULL_REBUILD_JOB_TYPE],
        )
        .expect("mark deterministic job running again");

    let second_snapshot =
        load_intelligence_runtime(&paths, &config, None).expect("second runtime snapshot");
    assert!(
        second_snapshot
            .notes
            .iter()
            .all(|note| !note.contains("Recovered 1 interrupted deterministic"))
    );

    let state = connection
        .query_row(
            "SELECT state FROM intelligence_jobs WHERE job_type = ?1",
            [FULL_REBUILD_JOB_TYPE],
            |row| row.get::<_, String>(0),
        )
        .expect("deterministic job state after second load");
    assert_eq!(state, "running");
}
