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
    /// The archive database (`archive/history-vault.db`), as `vault_store::Paths` lays it out.
    pub archive_db: PathBuf,
    /// Source evidence (`archive/source-evidence.db`).
    pub source_evidence_db: PathBuf,
    pub derived_dir: PathBuf,
    /// The full-text search index, a folder of segments (`derived/search-index`).
    pub search_index_dir: PathBuf,
    /// Insights and the queues (`derived/intelligence.db`).
    pub intelligence_db: PathBuf,
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
    /// AI vector sidecar plane (`derived/vectors/`); rebuildable, rides export via `derived/`.
    pub vectors_dir: PathBuf,
    /// Ask's conversations (`derived/agent.db`).
    pub agent_db: PathBuf,
    /// The v0.3 files an archive may still be in, read only to convert it.
    pub legacy: LegacyPaths,
    /// Downloaded embedding/reranker models (`<root>/models/`); re-downloadable, NOT exported.
    pub models_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub rust_log_path: PathBuf,
    pub frontend_log_path: PathBuf,
    pub crash_reports_dir: PathBuf,
    pub rust_panic_report_path: PathBuf,
    pub frontend_error_report_path: PathBuf,
    pub stronghold_path: PathBuf,
    pub stronghold_salt_path: PathBuf,
}

/// The SQLCipher databases of PathKeep 0.3, under the same root. Only finding an archive that
/// still has to be converted and the conversion itself (`vault-convert-sqlcipher`) read them;
/// everything else uses the v0.4 files in [`ProjectPaths`]. They used to share field names with
/// those, which `Vault::open` rewrote, so a reader that did not go through it got a file that no
/// longer exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyPaths {
    pub archive: PathBuf,
    pub source_evidence: PathBuf,
    pub search: PathBuf,
    pub intelligence: PathBuf,
    pub agent: PathBuf,
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
    let store = vault_store::Paths::under(&root);
    ProjectPaths {
        config_path: root.join("config.json"),
        archive_db: store.archive,
        source_evidence_db: store.source_evidence,
        derived_dir: derived_dir.clone(),
        search_index_dir: derived_dir.join("search-index"),
        intelligence_db: store.intelligence,
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
        vectors_dir: derived_dir.join("vectors"),
        agent_db: store.agent,
        legacy: LegacyPaths {
            archive: root.join("archive").join("history-vault.sqlite"),
            source_evidence: root.join("archive").join("source-evidence.sqlite"),
            search: derived_dir.join("history-search.sqlite"),
            intelligence: derived_dir.join("history-intelligence.sqlite"),
            agent: derived_dir.join("agent.sqlite"),
        },
        models_dir: root.join("models"),
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
        paths.archive_db.parent().expect("archive db parent"),
        paths.source_evidence_db.parent().expect("source evidence db parent"),
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
        &paths.vectors_dir,
        &paths.models_dir,
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
///
/// Persists through [`crate::durable_io::atomic_durable_write`] (temp -> F_FULLFSYNC
/// -> rename -> dir fsync) rather than a bare `fs::write`, so a crash mid-save leaves
/// `config.json` as EITHER the complete old config OR the complete new one — never the
/// truncated/empty file that silently dropped a mode change in the 2026-06-30 incident.
/// `ensure_paths` first guarantees the parent directory exists (the durable write
/// renames a sibling temp onto the destination and does not create parents).
pub fn save_config(paths: &ProjectPaths, config: &AppConfig) -> Result<()> {
    ensure_paths(paths)?;
    let content = serde_json::to_string_pretty(config)?;
    crate::durable_io::atomic_durable_write(&paths.config_path, content.as_bytes())
        .with_context(|| format!("writing {}", paths.config_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
}
