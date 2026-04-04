use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vault_core::{
    AppConfig, AppSnapshot, ArchiveMode, ExportRequest, HealthReport, HistoryQuery,
    HistoryQueryResponse, ImportBatchDetail, KeyringStatusReport, RemoteBackupPreview,
    RemoteBackupResult, S3CredentialInput, SchedulePlan, TakeoutInspection, TakeoutRequest,
    archive_status, doctor, ensure_archive_initialized, export_history, import_takeout,
    inspect_takeout, list_history, load_config, load_import_batches, load_recent_runs,
    preview_import_batch, preview_remote_backup, project_paths, rekey_archive, revert_import_batch,
    run_backup, run_remote_backup, save_config,
};
use vault_platform::{
    ScheduleParameters, apply_schedule, keyring_clear_database_key, keyring_clear_s3_credentials,
    keyring_get_database_key, keyring_get_s3_credentials, keyring_set_database_key,
    keyring_set_s3_credentials, keyring_status, preview_schedule, s3_credentials_saved,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RekeyRequest {
    pub new_mode: ArchiveMode,
    pub new_key: Option<String>,
}

fn hydrate_derived_config_state(config: &mut AppConfig) {
    config.remote_backup.credentials_saved = s3_credentials_saved();
}

pub fn app_snapshot(session_database_key: Option<&str>) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    let browser_profiles = vault_core::discover_profiles()?;
    let archive_status = archive_status(&paths, &config, session_database_key)?;
    let recent_runs = load_recent_runs(&paths, &config, session_database_key).unwrap_or_default();
    let recent_import_batches =
        load_import_batches(&paths, &config, session_database_key).unwrap_or_default();

    Ok(AppSnapshot {
        directories: vault_core::AppDirectories {
            app_root: paths.app_root.display().to_string(),
            config_path: paths.config_path.display().to_string(),
            archive_database_path: paths.archive_database_path.display().to_string(),
            audit_repo_path: paths.audit_repo_path.display().to_string(),
            manifests_dir: paths.manifests_dir.display().to_string(),
            exports_dir: paths.exports_dir.display().to_string(),
            raw_snapshots_dir: paths.raw_snapshots_dir.display().to_string(),
            staging_dir: paths.staging_dir.display().to_string(),
            quarantine_dir: paths.quarantine_dir.display().to_string(),
            schedule_dir: paths.schedule_dir.display().to_string(),
            stronghold_path: paths.stronghold_path.display().to_string(),
            stronghold_salt_path: paths.stronghold_salt_path.display().to_string(),
        },
        config,
        archive_status,
        keyring_status: keyring_status(),
        browser_profiles,
        recent_runs,
        recent_import_batches,
    })
}

pub fn save_user_config(
    config: &AppConfig,
    session_database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let mut next_config = config.clone();
    hydrate_derived_config_state(&mut next_config);
    save_config(&paths, &next_config)?;
    app_snapshot(session_database_key)
}

pub fn initialize_archive_database(
    config: &AppConfig,
    database_key: Option<&str>,
) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let mut next_config = config.clone();
    hydrate_derived_config_state(&mut next_config);
    save_config(&paths, &next_config)?;
    ensure_archive_initialized(&paths, &next_config, database_key)?;
    app_snapshot(database_key)
}

pub fn rekey_archive_database(
    old_key: Option<&str>,
    request: &RekeyRequest,
) -> Result<AppSnapshot> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    rekey_archive(&paths, &config, old_key, request.new_mode.clone(), request.new_key.as_deref())?;
    let mut next_config = config;
    next_config.archive_mode = request.new_mode.clone();
    next_config.initialized = true;
    save_config(&paths, &next_config)?;
    app_snapshot(request.new_key.as_deref().or(old_key))
}

pub fn run_backup_now(
    session_database_key: Option<&str>,
    due_only: bool,
) -> Result<vault_core::BackupReport> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    let mut report = run_backup(&paths, &config, session_database_key, due_only)?;
    if !report.due_skipped
        && config.remote_backup.enabled
        && config.remote_backup.upload_after_backup
    {
        match keyring_get_s3_credentials()? {
            Some(credentials) => {
                let remote = run_remote_backup(&paths, &config, session_database_key, &credentials)?;
                if remote.uploaded {
                    report.remote_backup = Some(remote);
                } else {
                    report.warnings.push(remote.message.clone());
                    report.remote_backup = Some(remote);
                }
            }
            None => report
                .warnings
                .push("Remote backup is enabled, but S3 credentials are not stored in the system keyring.".to_string()),
        }
    }
    Ok(report)
}

pub fn query_history(
    session_database_key: Option<&str>,
    query: HistoryQuery,
) -> Result<HistoryQueryResponse> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    list_history(&paths, &config, session_database_key, query)
}

pub fn export_query(
    session_database_key: Option<&str>,
    request: ExportRequest,
) -> Result<vault_core::ExportResult> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    export_history(&paths, &config, session_database_key, request)
}

pub fn preview_remote_backup_bundle() -> Result<RemoteBackupPreview> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    preview_remote_backup(&paths, &config)
}

pub fn upload_remote_backup_bundle(
    session_database_key: Option<&str>,
) -> Result<RemoteBackupResult> {
    let paths = project_paths()?;
    let mut config = load_config(&paths)?;
    hydrate_derived_config_state(&mut config);
    let credentials = keyring_get_s3_credentials()?
        .context("store S3 credentials in Settings before running a remote backup")?;
    run_remote_backup(&paths, &config, session_database_key, &credentials)
}

pub fn inspect_takeout_source(request: &TakeoutRequest) -> Result<TakeoutInspection> {
    let paths = project_paths()?;
    inspect_takeout(&paths, request)
}

pub fn import_takeout_source(
    session_database_key: Option<&str>,
    request: &TakeoutRequest,
) -> Result<TakeoutInspection> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    import_takeout(&paths, &config, session_database_key, request)
}

pub fn preview_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    preview_import_batch(&paths, &config, session_database_key, batch_id)
}

pub fn revert_import_batch_detail(
    session_database_key: Option<&str>,
    batch_id: i64,
) -> Result<ImportBatchDetail> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    revert_import_batch(&paths, &config, session_database_key, batch_id)
}

pub fn doctor_report(session_database_key: Option<&str>) -> Result<HealthReport> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    doctor(&paths, &config, session_database_key)
}

pub fn preview_schedule_plan(
    platform: Option<&str>,
    executable_path: Option<PathBuf>,
) -> Result<SchedulePlan> {
    let paths = project_paths()?;
    let config = load_config(&paths)?;
    let executable = executable_path
        .or_else(|| std::env::current_exe().ok())
        .context("resolving executable path for schedule preview")?;
    preview_schedule(
        platform,
        executable.as_path(),
        &paths,
        &ScheduleParameters {
            due_after_hours: config.due_after_hours,
            check_interval_hours: config.schedule_check_interval_hours,
        },
    )
}

pub fn apply_schedule_plan(plan: &SchedulePlan) -> Result<vault_core::ApplyResult> {
    let paths = project_paths()?;
    apply_schedule(plan, &paths)
}

pub fn read_database_key_from_keyring() -> Result<Option<String>> {
    keyring_get_database_key()
}

pub fn write_database_key_to_keyring(key: &str) -> Result<KeyringStatusReport> {
    keyring_set_database_key(key)?;
    Ok(keyring_status())
}

pub fn clear_database_key_from_keyring() -> Result<KeyringStatusReport> {
    keyring_clear_database_key()?;
    Ok(keyring_status())
}

pub fn reset_local_secret_vault() -> Result<()> {
    let paths = project_paths()?;
    if paths.stronghold_path.exists() {
        std::fs::remove_file(&paths.stronghold_path)
            .with_context(|| format!("removing {}", paths.stronghold_path.display()))?;
    }
    Ok(())
}

pub fn keyring_report() -> KeyringStatusReport {
    keyring_status()
}

pub fn store_s3_credentials(credentials: &S3CredentialInput) -> Result<()> {
    keyring_set_s3_credentials(credentials)
}

pub fn clear_s3_credentials() -> Result<()> {
    keyring_clear_s3_credentials()
}

pub fn run_worker_cli(arguments: &[String]) -> Result<String> {
    let command = arguments.first().map(String::as_str).unwrap_or("snapshot");
    match command {
        "backup" => {
            let due_only = arguments.iter().any(|arg| arg == "--due-only");
            let key = read_database_key_from_keyring()?;
            let report = run_backup_now(key.as_deref(), due_only)?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "doctor" => {
            let key = read_database_key_from_keyring()?;
            let report = doctor_report(key.as_deref())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "remote-backup" => {
            let key = read_database_key_from_keyring()?;
            let report = upload_remote_backup_bundle(key.as_deref())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        other => anyhow::bail!("unknown worker command: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;
    use vault_core::{ArchiveMode, project_paths};

    const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
    const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
    const TEST_KEYRING_OVERRIDE_ENV: &str = "CHB_TEST_KEYRING_DIR";

    fn initialized_config() -> AppConfig {
        AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            due_after_hours: 72,
            checkpoint_days: 1,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        }
    }

    fn chrome_user_data_fixture(root: &Path) -> PathBuf {
        let chrome_root = root.join("chrome-user-data");
        let profile_dir = chrome_root.join("Default");
        fs::create_dir_all(&profile_dir).expect("create chrome profile dir");
        fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"tester@example.com"}}}}"#,
        )
        .expect("write local state");

        let history = Connection::open(profile_dir.join("History")).expect("open source history");
        history
            .execute_batch(
                "
                CREATE TABLE urls (
                  id INTEGER PRIMARY KEY,
                  url TEXT NOT NULL,
                  title TEXT,
                  visit_count INTEGER NOT NULL,
                  typed_count INTEGER NOT NULL,
                  last_visit_time INTEGER NOT NULL,
                  hidden INTEGER NOT NULL
                );
                CREATE TABLE visits (
                  id INTEGER PRIMARY KEY,
                  url INTEGER NOT NULL,
                  visit_time INTEGER NOT NULL,
                  from_visit INTEGER,
                  transition INTEGER,
                  visit_duration INTEGER,
                  is_known_to_sync INTEGER,
                  visited_link_id INTEGER,
                  external_referrer_url TEXT,
                  app_id TEXT
                );
                CREATE TABLE downloads (
                  id INTEGER PRIMARY KEY,
                  guid TEXT,
                  current_path TEXT,
                  target_path TEXT,
                  start_time INTEGER,
                  received_bytes INTEGER,
                  total_bytes INTEGER,
                  state INTEGER,
                  mime_type TEXT,
                  original_mime_type TEXT
                );
                CREATE TABLE keyword_search_terms (
                  keyword_id INTEGER,
                  url_id INTEGER,
                  term TEXT,
                  normalized_term TEXT
                );",
            )
            .expect("create history schema");
        history
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (1, 'https://example.com', 'Example', 1, 1, 1, 0)",
                [],
            )
            .expect("insert url");
        history
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (1, 1, 1, NULL, 805306368, 24000, 1, 3, 'https://ref.example', 'com.example.app')",
                [],
            )
            .expect("insert visit");
        history
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (1, 'guid-1', '/tmp/current', '/tmp/target', 1, 1, 2, 3, 'text/html', 'text/plain')",
                [],
            )
            .expect("insert download");
        history
            .execute(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (1, 1, 'chrome history', 'chrome history')",
                [],
            )
            .expect("insert search term");

        chrome_root
    }

    #[test]
    fn app_snapshot_and_worker_cli_cover_main_local_flows() {
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let config = initialized_config();
        let snapshot = initialize_archive_database(&config, None).expect("initialize archive");
        assert!(snapshot.archive_status.initialized);
        assert_eq!(snapshot.browser_profiles.len(), 1);
        assert_eq!(snapshot.browser_profiles[0].profile_id, "chrome:Default");

        let backup_json = run_worker_cli(&["backup".to_string()]).expect("backup json");
        let backup: vault_core::BackupReport =
            serde_json::from_str(&backup_json).expect("parse backup report");
        assert_eq!(backup.run.expect("run").new_visits, 1);

        let doctor_json = run_worker_cli(&["doctor".to_string()]).expect("doctor json");
        let doctor: HealthReport = serde_json::from_str(&doctor_json).expect("parse doctor report");
        assert!(!doctor.checks.is_empty());

        let paths = project_paths().expect("project paths");
        assert!(paths.archive_database_path.exists());
    }

    #[test]
    fn worker_cli_rejects_unknown_commands() {
        let error = run_worker_cli(&["wat".to_string()]).expect_err("unknown command should fail");
        assert!(error.to_string().contains("unknown worker command"));
    }
}
