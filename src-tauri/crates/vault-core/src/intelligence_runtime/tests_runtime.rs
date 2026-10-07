//! Responsibilities: native recovery and runtime projection contracts.
//! Not responsible for: queue execution or frontend rendering.
//! Failure modes: reopen strands claims, reads reclaim live work, disabled modules
//! leak enabled state, malformed metadata hides notes, or version drift looks ready.
use super::*;
use vault_store::{Db, Priority, params_owned as params};

fn running(vault: &crate::Vault, kind: &str) {
    let kind = kind.to_owned();
    vault.write(Db::Intelligence,Priority::Interactive,move |s| {
        s.execute("INSERT INTO intelligence_jobs(job_type,plugin_id,state,priority,attempt,dedupe_key,payload_json,created_at,scheduled_at,updated_at) VALUES(?1,?2,'running',10,1,'recovery','{}','','','')",params![kind,TITLE_NORMALIZATION_PLUGIN_ID])?;Ok(())
    }).unwrap();
}
fn state(vault: &crate::Vault) -> String {
    vault
        .store()
        .read(Db::Intelligence, |s| {
            s.query_one(
                "SELECT state FROM intelligence_jobs WHERE dedupe_key='recovery'",
                (),
                |r| Ok(r.get::<String>(0)?),
            )
        })
        .unwrap()
}
fn recovery(kind: &str) {
    for key in [None, Some([77; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        running(&vault, kind);
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let reopened = crate::Vault::open(paths, config, key).unwrap();
        assert_eq!(state(&reopened), "queued");
        assert_eq!(load_intelligence_runtime(&reopened).unwrap().queue.queued, 1);
    }
}
#[test]
fn runtime_snapshot_returns_plugin_defaults_before_archive_exists() {
    let owner = crate::Vault::open_temp().unwrap();
    let mut config = AppConfig::default();
    config.ai.enrichment_enabled = false;
    config.deterministic.modules[0].enabled = false;
    let snapshot = load_intelligence_runtime(&owner.with_config(config)).unwrap();
    assert_eq!(snapshot.plugins.len(), 2);
    assert!(snapshot.notes.iter().any(|n| n.contains("Initialize the archive")));
    assert!(snapshot.notes.iter().any(|n| n.contains("Enrichment plugins are disabled")));
    assert!(
        snapshot
            .modules
            .iter()
            .any(|m| m.status == "disabled" && m.notes.iter().any(|n| n.contains("Disabled")))
    );
}
#[test]
fn load_intelligence_runtime_recovers_interrupted_deterministic_jobs() {
    recovery(FULL_REBUILD_JOB_TYPE);
}
#[test]
fn load_intelligence_runtime_recovers_interrupted_enrichment_jobs() {
    recovery(ENRICHMENT_JOB_TYPE);
}
#[test]
fn load_intelligence_runtime_only_recovers_running_jobs_once_per_archive() {
    let vault = crate::Vault::open_temp().unwrap();
    running(&vault, FULL_REBUILD_JOB_TYPE);
    for _ in 0..2 {
        assert_eq!(load_intelligence_runtime(&vault).unwrap().queue.running, 1);
        assert_eq!(state(&vault), "running");
    }
}

#[test]
fn runtime_snapshot_projects_disabled_stale_and_failed_runtime_rows() {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths().clone();
    let mut config = vault.config().clone();
    vault
        .write(Db::Intelligence, Priority::Interactive, move |connection| {
            let now = crate::utils::now_rfc3339();
            let modules = built_in_deterministic_modules();
            assert!(modules.len() >= 3);
            config
                .deterministic
                .modules
                .iter_mut()
                .find(|module| module.id == modules[0].id)
                .expect("first module config")
                .enabled = false;

            connection
                .execute(
                    "INSERT INTO deterministic_module_runtime
             (module_id, version, status, depends_on_json, derived_tables_json, last_run_id,
              last_built_at, last_invalidated_at, stale_reason, notes_json, updated_at)
             VALUES (?1, ?2, 'ready', '{not-json', '{not-json', 1, ?3, NULL, NULL,
              '[\"A legacy uncoded note.\"]', ?3)",
                    params![modules[0].id, modules[0].version, now],
                )
                .expect("insert disabled stored module");
            connection
                .execute(
                    "INSERT INTO deterministic_module_runtime
             (module_id, version, status, depends_on_json, derived_tables_json, last_run_id,
              last_built_at, last_invalidated_at, stale_reason, notes_json, updated_at)
             VALUES (?1, 'old-version', 'ready', '[]', '[]', 2, ?2, NULL, NULL, '[]', ?2)",
                    params![modules[1].id, now],
                )
                .expect("insert old-version module");
            connection
                .execute(
                    "INSERT INTO deterministic_module_runtime
             (module_id, version, status, depends_on_json, derived_tables_json, last_run_id,
              last_built_at, last_invalidated_at, stale_reason, notes_json, updated_at)
             VALUES (?1, ?2, 'ready', '[]', '[]', 3, NULL, NULL, NULL, '[]', ?3)",
                    params![modules[2].id, modules[2].version, now],
                )
                .expect("insert missing timestamp module");
            connection
                .execute(
                    "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, updated_at, last_error)
             VALUES (?1, ?2, 41, 'failed', 10, 1, 'readable-content-refetch:runtime-snapshot',
              ?3, '{}', ?4, ?4, ?4, 'fetch failed')",
                    params![
                        ENRICHMENT_JOB_TYPE,
                        READABLE_CONTENT_PLUGIN_ID,
                        serde_json::to_string(&EnrichmentJobPayload {
                            history_id: Some(1),
                            profile_id: Some("chrome:Default".to_string()),
                            url: "https://example.com/article".to_string(),
                            title: Some("Example".to_string()),
                        })
                        .expect("payload"),
                        now,
                    ],
                )
                .expect("insert failed enrichment job");

            let snapshot = super::snapshot::load_intelligence_runtime_from_connection(
                connection, &paths, &config,
            )
            .expect("runtime snapshot");
            let disabled = snapshot
                .modules
                .iter()
                .find(|module| module.module_id == modules[0].id)
                .expect("disabled module");
            assert_eq!(disabled.status, "disabled");
            assert!(!disabled.depends_on.is_empty() || !disabled.derived_tables.is_empty());
            assert!(disabled.notes.iter().any(|note| note.contains("Disabled")));
            // Legacy `notes_json` rows hold plain strings. They must survive as opaque
            // pass-throughs rather than being pattern-matched back into codes.
            let legacy = disabled
                .note_details
                .iter()
                .find(|note| note.message == "A legacy uncoded note.")
                .expect("legacy note preserved");
            assert_eq!(legacy.code, "");
            assert!(
                disabled
                    .note_details
                    .iter()
                    .any(|note| note.code == crate::models::DERIVED_NOTE_MODULE_DISABLED)
            );
            let version_stale = snapshot
                .modules
                .iter()
                .find(|module| module.module_id == modules[1].id)
                .expect("version stale module");
            assert_eq!(version_stale.status, "stale");
            assert!(
                version_stale
                    .stale_reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("version changed"))
            );
            assert_eq!(
                version_stale.stale_reason_code.as_deref(),
                Some(crate::models::DERIVED_STALE_MODULE_VERSION_CHANGED)
            );
            assert!(
                version_stale
                    .note_details
                    .iter()
                    .any(|note| note.code == crate::models::DERIVED_NOTE_MODULE_VERSION_MISMATCH)
            );
            let timestamp_stale = snapshot
                .modules
                .iter()
                .find(|module| module.module_id == modules[2].id)
                .expect("timestamp stale module");
            assert_eq!(timestamp_stale.status, "stale");
            assert!(
                timestamp_stale
                    .stale_reason
                    .as_deref()
                    .is_some_and(|reason| reason.contains("Missing build timestamp"))
            );
            assert_eq!(
                timestamp_stale.stale_reason_code.as_deref(),
                Some(crate::models::DERIVED_STALE_MISSING_BUILD_TIMESTAMP)
            );
            assert!(
                snapshot
                    .plugins
                    .iter()
                    .any(|plugin| plugin.plugin_id == READABLE_CONTENT_PLUGIN_ID
                        && plugin.failed_jobs == 1
                        && plugin.last_error.as_deref() == Some("fetch failed"))
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn runtime_snapshot_marks_missing_disabled_module_rows() {
    let vault = crate::Vault::open_temp().unwrap();
    let mut config = vault.config().clone();
    let modules = built_in_deterministic_modules();
    let module = modules.first().expect("built-in module");
    config
        .deterministic
        .modules
        .iter_mut()
        .find(|entry| entry.id == module.id)
        .expect("module config")
        .enabled = false;

    let snapshot = load_intelligence_runtime(&vault.with_config(config)).expect("runtime snapshot");
    let disabled = snapshot
        .modules
        .iter()
        .find(|entry| entry.module_id == module.id)
        .expect("disabled module fallback");

    assert_eq!(disabled.status, "disabled");
    assert!(!disabled.enabled);
    assert!(disabled.notes.iter().any(|note| note == "Disabled in Settings."));
    assert!(
        disabled
            .note_details
            .iter()
            .any(|note| note.code == crate::models::DERIVED_NOTE_MODULE_DISABLED)
    );
}
