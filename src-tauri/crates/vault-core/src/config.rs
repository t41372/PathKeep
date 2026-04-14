//! Project-path and config persistence helpers.
//!
//! The rest of the backend assumes a stable directory layout. This module is
//! the source of truth for that layout and for reading/writing the serialized
//! app config under the project root.

use crate::{
    models::{AppConfig, normalize_app_config},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CURRENT_APP_QUALIFIER: &str = "com";
const CURRENT_APP_ORGANIZATION: &str = "yi-ting";
const CURRENT_APP_NAME: &str = "pathkeep";
const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Canonical absolute paths for all backend-managed project artifacts.
pub struct ProjectPaths {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub archive_database_path: PathBuf,
    pub derived_dir: PathBuf,
    pub search_database_path: PathBuf,
    pub intelligence_database_path: PathBuf,
    pub audit_repo_path: PathBuf,
    pub manifests_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub raw_snapshots_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub quarantine_dir: PathBuf,
    pub schedule_dir: PathBuf,
    pub sidecars_dir: PathBuf,
    pub semantic_index_dir: PathBuf,
    pub intelligence_blobs_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub rust_log_path: PathBuf,
    pub frontend_log_path: PathBuf,
    pub crash_reports_dir: PathBuf,
    pub rust_panic_report_path: PathBuf,
    pub frontend_error_report_path: PathBuf,
    pub stronghold_path: PathBuf,
    pub stronghold_salt_path: PathBuf,
}

/// Resolves the current project paths using the active project root.
pub fn project_paths() -> Result<ProjectPaths> {
    let root = project_root()?;
    Ok(project_paths_with_root(&root))
}

/// Builds the full project path layout for a specific root directory.
pub fn project_paths_with_root(root: &Path) -> ProjectPaths {
    let root = root.to_path_buf();
    let logs_dir = root.join("logs");
    let crash_reports_dir = root.join("diagnostics").join("crash-reports");
    let derived_dir = root.join("derived");
    let sidecars_dir = root.join("sidecars");
    ProjectPaths {
        config_path: root.join("config.json"),
        archive_database_path: root.join("archive").join("history-vault.sqlite"),
        derived_dir: derived_dir.clone(),
        search_database_path: derived_dir.join("history-search.sqlite"),
        intelligence_database_path: derived_dir.join("history-intelligence.sqlite"),
        audit_repo_path: root.join("audit"),
        manifests_dir: root.join("audit").join("manifests"),
        exports_dir: root.join("exports"),
        raw_snapshots_dir: root.join("raw-snapshots"),
        staging_dir: root.join("staging"),
        quarantine_dir: root.join("quarantine"),
        schedule_dir: root.join("schedule"),
        sidecars_dir: sidecars_dir.clone(),
        semantic_index_dir: sidecars_dir.join("semantic-index"),
        intelligence_blobs_dir: sidecars_dir.join("intelligence-blobs"),
        logs_dir: logs_dir.clone(),
        rust_log_path: logs_dir.join("rust.log"),
        frontend_log_path: logs_dir.join("frontend.log"),
        crash_reports_dir: crash_reports_dir.clone(),
        rust_panic_report_path: crash_reports_dir.join("rust-panic-latest.json"),
        frontend_error_report_path: crash_reports_dir.join("frontend-error-latest.json"),
        stronghold_path: root.join("vault.hold"),
        stronghold_salt_path: root.join("stronghold-salt.txt"),
        app_root: root,
    }
}

/// Resolves the active project root, honoring test and development overrides.
fn project_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV) {
        return Ok(PathBuf::from(path));
    }

    let dirs = ProjectDirs::from(CURRENT_APP_QUALIFIER, CURRENT_APP_ORGANIZATION, CURRENT_APP_NAME)
        .context("resolving current project directories")?;
    Ok(dirs.data_local_dir().to_path_buf())
}

/// Ensures the expected project directory layout exists on disk.
pub fn ensure_paths(paths: &ProjectPaths) -> Result<()> {
    for dir in [
        &paths.app_root,
        paths.archive_database_path.parent().expect("archive db parent"),
        &paths.derived_dir,
        &paths.audit_repo_path,
        &paths.manifests_dir,
        &paths.exports_dir,
        &paths.raw_snapshots_dir,
        &paths.staging_dir,
        &paths.quarantine_dir,
        &paths.schedule_dir,
        &paths.sidecars_dir,
        &paths.semantic_index_dir,
        &paths.intelligence_blobs_dir,
        &paths.logs_dir,
        &paths.crash_reports_dir,
    ] {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }

    if !paths.stronghold_salt_path.exists() {
        fs::write(&paths.stronghold_salt_path, format!("{}\n", now_rfc3339()))
            .with_context(|| format!("writing {}", paths.stronghold_salt_path.display()))?;
    }
    Ok(())
}

/// Loads the persisted app config, normalizing newly added runtime defaults.
pub fn load_config(paths: &ProjectPaths) -> Result<AppConfig> {
    if !paths.config_path.exists() {
        let mut config = AppConfig::default();
        normalize_app_config(&mut config);
        return Ok(config);
    }

    let content = fs::read_to_string(&paths.config_path)
        .with_context(|| format!("reading {}", paths.config_path.display()))?;
    let mut config = serde_json::from_str::<AppConfig>(&content).context("parsing config json")?;
    normalize_app_config(&mut config);
    Ok(config)
}

/// Saves the app config to disk after normalizing runtime defaults.
pub fn save_config(paths: &ProjectPaths, config: &AppConfig) -> Result<()> {
    ensure_paths(paths)?;
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&paths.config_path, content)
        .with_context(|| format!("writing {}", paths.config_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::ArchiveMode,
        utils::{restore_test_env_var, test_env_lock},
    };
    use directories::ProjectDirs;
    use tempfile::tempdir;

    #[test]
    fn project_paths_honor_override_environment() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempdir().expect("tempdir");
        let original_override = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }
        let paths = project_paths().expect("project paths");
        restore_test_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_override.as_deref());

        assert_eq!(paths.app_root, dir.path());
        assert_eq!(paths.archive_database_path, dir.path().join("archive/history-vault.sqlite"));
        assert_eq!(paths.search_database_path, dir.path().join("derived/history-search.sqlite"));
        assert_eq!(
            paths.intelligence_database_path,
            dir.path().join("derived/history-intelligence.sqlite")
        );
    }

    #[test]
    fn ensure_paths_and_config_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        ensure_paths(&paths).expect("ensure paths");
        assert!(paths.stronghold_salt_path.exists());
        assert!(paths.manifests_dir.exists());
        assert!(paths.derived_dir.exists());
        assert!(paths.semantic_index_dir.exists());
        assert!(paths.intelligence_blobs_dir.exists());

        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Encrypted,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        };
        save_config(&paths, &config).expect("save config");

        let loaded = load_config(&paths).expect("load config");
        assert!(loaded.initialized);
        assert_eq!(loaded.selected_profile_ids, vec!["chrome:Default".to_string()]);
        assert!(matches!(loaded.archive_mode, ArchiveMode::Encrypted));
        let saved_content = fs::read_to_string(&paths.config_path).expect("read saved config");
        assert!(saved_content.contains(r#""archiveMode": "Encrypted""#));
    }

    #[test]
    fn load_config_returns_defaults_when_missing_and_errors_on_invalid_json() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        let default_config = load_config(&paths).expect("missing config should default");
        assert!(!default_config.initialized);

        fs::write(&paths.config_path, "{not-json").expect("write invalid config");
        let error = load_config(&paths).expect_err("invalid config should fail");
        assert!(error.to_string().contains("parsing config json"));
    }

    #[test]
    fn load_config_accepts_legacy_lowercase_archive_mode_values() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());

        fs::write(
            &paths.config_path,
            r#"{
  "initialized": true,
  "archiveMode": "plaintext",
  "preferredLanguage": "system",
  "dueAfterHours": 72,
  "scheduleCheckIntervalHours": 6,
  "checkpointDays": 1,
  "captureFavicons": true,
  "selectedProfileIds": [],
  "gitEnabled": false,
  "rememberDatabaseKeyInKeyring": false,
  "appAutostart": false,
  "appLock": {
    "enabled": false,
    "idleTimeoutMinutes": 5,
    "biometricEnabled": false,
    "passcodeEnabled": false,
    "passcodeConfigured": false,
    "recoveryHint": null
  },
  "analytics": {
    "enabled": false,
    "consentGrantedAt": null
  },
  "remoteBackup": {
    "enabled": false,
    "bucket": "",
    "region": "us-east-1",
    "endpoint": null,
    "prefix": "pathkeep",
    "pathStyle": true,
    "uploadAfterBackup": false,
    "credentialsSaved": false,
    "lastUploadedAt": null,
    "lastUploadedObjectKey": null,
    "lastError": null
  },
  "enrichment": {
    "plugins": []
  },
  "ai": {
    "enabled": false,
    "assistantEnabled": false,
    "semanticIndexEnabled": false,
    "mcpEnabled": false,
    "skillEnabled": false,
    "autoIndexAfterBackup": false,
    "jobQueuePaused": false,
    "jobQueueConcurrency": 1,
    "enrichmentEnabled": true,
    "enrichmentPlugins": [],
    "llmProviderId": null,
    "embeddingProviderId": null,
    "retrievalTopK": 8,
    "assistantSystemPrompt": "",
    "llmProviders": [],
    "embeddingProviders": []
  }
}"#,
        )
        .expect("write legacy lowercase config");

        let loaded = load_config(&paths).expect("load legacy lowercase config");
        assert!(matches!(loaded.archive_mode, ArchiveMode::Plaintext));
    }

    #[test]
    fn project_root_uses_current_app_directory_when_it_already_exists() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = tempdir().expect("tempdir");
        let original_home = std::env::var_os("HOME");
        let original_override = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV);
        unsafe {
            std::env::set_var("HOME", dir.path());
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        }

        let current_root =
            ProjectDirs::from(CURRENT_APP_QUALIFIER, CURRENT_APP_ORGANIZATION, CURRENT_APP_NAME)
                .expect("project dirs")
                .data_local_dir()
                .to_path_buf();
        fs::create_dir_all(&current_root).expect("create current root");

        let resolved = project_root().expect("project root");
        assert_eq!(resolved, current_root);

        restore_test_env_var("HOME", original_home.as_deref());
        restore_test_env_var(PROJECT_ROOT_OVERRIDE_ENV, original_override.as_deref());
    }

    #[test]
    fn restore_env_var_sets_and_clears_values() {
        let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let value = std::ffi::OsString::from("fixture-root");
        restore_test_env_var(PROJECT_ROOT_OVERRIDE_ENV, Some(value.as_os_str()));
        assert_eq!(std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV), Some(value.clone()));

        restore_test_env_var(PROJECT_ROOT_OVERRIDE_ENV, None);
        assert!(std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV).is_none());
    }
}
