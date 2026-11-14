//! Responsibilities: expose typed catalog-backed paths and persist the existing app config.
//! Not responsible for: declaring lifecycle membership, opening stores or changing disk formats.

use crate::models::{AppConfig, normalize_app_config};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_layout::ArtifactId as A;

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

impl ProjectPaths {
    /// Startup identification intentionally probes only the catalog's native primary.
    pub fn native_archive_path(&self) -> PathBuf {
        self.artifact_path(vault_layout::db_artifact(vault_layout::primary_database()))
    }

    /// Old-format recovery keeps its own primary instead of borrowing the native role set.
    pub fn legacy_archive_path(&self) -> PathBuf {
        self.artifact_path(vault_layout::legacy_primary())
    }

    /// Operation projections honor the existing caller-configurable typed paths. Artifacts
    /// without an exposed override use their catalog location under the supplied root.
    pub fn artifact_path(&self, id: A) -> PathBuf {
        match id {
            A::Root => self.app_root.clone(),
            A::History => self.archive_db.clone(),
            A::SourceEvidence => self.source_evidence_db.clone(),
            A::Intelligence => self.intelligence_db.clone(),
            A::Assistant => self.agent_db.clone(),
            A::Config => self.config_path.clone(),
            A::DerivedRoot => self.derived_dir.clone(),
            A::Search => self.search_index_dir.clone(),
            A::Audit => self.audit_repo_path.clone(),
            A::Manifests => self.manifests_dir.clone(),
            A::Exports => self.exports_dir.clone(),
            A::RawSnapshots => self.raw_snapshots_dir.clone(),
            A::Staging => self.staging_dir.clone(),
            A::Quarantine => self.quarantine_dir.clone(),
            A::Schedule => self.schedule_dir.clone(),
            A::Sidecars => self.sidecars_dir.clone(),
            A::LegacySemantic => self.semantic_index_dir.clone(),
            A::TextBlobs => self.intelligence_blobs_dir.clone(),
            A::Vectors => self.vectors_dir.clone(),
            A::Models => self.models_dir.clone(),
            A::Logs => self.logs_dir.clone(),
            A::CrashReports => self.crash_reports_dir.clone(),
            A::RustLog => self.rust_log_path.clone(),
            A::FrontendLog => self.frontend_log_path.clone(),
            A::RustPanic => self.rust_panic_report_path.clone(),
            A::FrontendError => self.frontend_error_report_path.clone(),
            A::LegacyHistory => self.legacy.archive.clone(),
            A::LegacyEvidence => self.legacy.source_evidence.clone(),
            A::LegacyIntelligence => self.legacy.intelligence.clone(),
            A::LegacySearch => self.legacy.search.clone(),
            A::LegacyAssistant => self.legacy.agent.clone(),
            A::ArchiveRoot => self.archive_db.parent().expect("archive parent").to_owned(),
            A::NativeSnapshots | A::LegacySnapshots => self.raw_snapshots_dir.join(
                Path::new(vault_layout::spec(id).path.relative())
                    .strip_prefix(vault_layout::spec(A::RawSnapshots).path.relative())
                    .unwrap(),
            ),
            A::MaintenanceStages => self.staging_dir.clone(),
            A::ScheduleLogs | A::ScheduleAttempts => self.schedule_dir.join(
                Path::new(vault_layout::spec(id).path.relative())
                    .strip_prefix(vault_layout::spec(A::Schedule).path.relative())
                    .unwrap(),
            ),
            A::SchedulerAudit => self.audit_repo_path.join(
                Path::new(vault_layout::spec(id).path.relative())
                    .strip_prefix(vault_layout::spec(A::Audit).path.relative())
                    .unwrap(),
            ),
            _ => vault_layout::path(&self.app_root, id),
        }
    }
}

/// Resolves the current project paths using the active project root.
pub fn project_paths() -> Result<ProjectPaths> {
    let root = project_root()?;
    Ok(project_paths_with_root(&root))
}

/// Builds the full project path layout for a specific root directory.
pub fn project_paths_with_root(root: &Path) -> ProjectPaths {
    let root = root.to_path_buf();
    let at = |id| vault_layout::path(&root, id);
    let logs_dir = at(A::Logs);
    let crash_reports_dir = at(A::CrashReports);
    let derived_dir = at(A::DerivedRoot);
    let sidecars_dir = at(A::Sidecars);
    let store = vault_store::Paths::under(&root);
    ProjectPaths {
        config_path: at(A::Config),
        archive_db: store.archive,
        source_evidence_db: store.source_evidence,
        derived_dir: derived_dir.clone(),
        search_index_dir: at(A::Search),
        intelligence_db: store.intelligence,
        audit_repo_path: at(A::Audit),
        manifests_dir: at(A::Manifests),
        exports_dir: at(A::Exports),
        raw_snapshots_dir: at(A::RawSnapshots),
        staging_dir: at(A::Staging),
        quarantine_dir: at(A::Quarantine),
        schedule_dir: at(A::Schedule),
        sidecars_dir: sidecars_dir.clone(),
        semantic_index_dir: at(A::LegacySemantic),
        intelligence_blobs_dir: at(A::TextBlobs),
        vectors_dir: at(A::Vectors),
        agent_db: store.agent,
        legacy: LegacyPaths {
            archive: at(A::LegacyHistory),
            source_evidence: at(A::LegacyEvidence),
            search: at(A::LegacySearch),
            intelligence: at(A::LegacyIntelligence),
            agent: at(A::LegacyAssistant),
        },
        models_dir: at(A::Models),
        logs_dir: logs_dir.clone(),
        rust_log_path: at(A::RustLog),
        frontend_log_path: at(A::FrontendLog),
        crash_reports_dir: crash_reports_dir.clone(),
        rust_panic_report_path: at(A::RustPanic),
        frontend_error_report_path: at(A::FrontendError),
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
    let directories: Vec<_> = vault_layout::scaffolding()
        .into_iter()
        .map(|(id, parent)| {
            let path = paths.artifact_path(id);
            if parent { path.parent().expect("database parent").to_owned() } else { path }
        })
        .collect();
    for directory in directories {
        fs::create_dir_all(&directory)
            .with_context(|| format!("creating {}", directory.display()))?;
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
