use crate::{models::AppConfig, utils::now_rfc3339};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const CURRENT_APP_NAME: &str = "Chrome History Backup";
const LEGACY_APP_NAME: &str = "Chrome History Vault";
const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPaths {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub archive_database_path: PathBuf,
    pub audit_repo_path: PathBuf,
    pub manifests_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub raw_snapshots_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub quarantine_dir: PathBuf,
    pub schedule_dir: PathBuf,
    pub stronghold_path: PathBuf,
    pub stronghold_salt_path: PathBuf,
}

pub fn project_paths() -> Result<ProjectPaths> {
    let root = project_root()?;
    Ok(ProjectPaths {
        config_path: root.join("config.json"),
        archive_database_path: root.join("archive").join("history-vault.sqlite"),
        audit_repo_path: root.join("audit"),
        manifests_dir: root.join("audit").join("manifests"),
        exports_dir: root.join("exports"),
        raw_snapshots_dir: root.join("raw-snapshots"),
        staging_dir: root.join("staging"),
        quarantine_dir: root.join("quarantine"),
        schedule_dir: root.join("schedule"),
        stronghold_path: root.join("vault.hold"),
        stronghold_salt_path: root.join("stronghold-salt.txt"),
        app_root: root,
    })
}

fn project_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(PROJECT_ROOT_OVERRIDE_ENV) {
        return Ok(PathBuf::from(path));
    }

    let dirs = ProjectDirs::from("dev", "Codex", CURRENT_APP_NAME)
        .context("resolving current project directories")?;
    let root = dirs.data_local_dir().to_path_buf();

    if root.exists() {
        return Ok(root);
    }

    if let Some(legacy_dirs) = ProjectDirs::from("dev", "Codex", LEGACY_APP_NAME) {
        let legacy_root = legacy_dirs.data_local_dir().to_path_buf();
        if legacy_root.exists() {
            if let Some(parent) = root.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            fs::rename(&legacy_root, &root).with_context(|| {
                format!("migrating app data from {} to {}", legacy_root.display(), root.display())
            })?;
        }
    }

    Ok(root)
}

pub fn ensure_paths(paths: &ProjectPaths) -> Result<()> {
    for dir in [
        &paths.app_root,
        paths.archive_database_path.parent().expect("archive db parent"),
        &paths.audit_repo_path,
        &paths.manifests_dir,
        &paths.exports_dir,
        &paths.raw_snapshots_dir,
        &paths.staging_dir,
        &paths.quarantine_dir,
        &paths.schedule_dir,
    ] {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }

    if !paths.stronghold_salt_path.exists() {
        fs::write(&paths.stronghold_salt_path, format!("{}\n", now_rfc3339()))
            .with_context(|| format!("writing {}", paths.stronghold_salt_path.display()))?;
    }
    Ok(())
}

pub fn load_config(paths: &ProjectPaths) -> Result<AppConfig> {
    if !paths.config_path.exists() {
        return Ok(AppConfig::default());
    }

    let content = fs::read_to_string(&paths.config_path)
        .with_context(|| format!("reading {}", paths.config_path.display()))?;
    serde_json::from_str(&content).context("parsing config json")
}

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
    use crate::models::ArchiveMode;
    use tempfile::tempdir;

    #[test]
    fn project_paths_honor_override_environment() {
        let dir = tempdir().expect("tempdir");
        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        }
        let paths = project_paths().expect("project paths");
        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        }

        assert_eq!(paths.app_root, dir.path());
        assert_eq!(paths.archive_database_path, dir.path().join("archive/history-vault.sqlite"));
    }

    #[test]
    fn ensure_paths_and_config_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let paths = ProjectPaths {
            app_root: dir.path().to_path_buf(),
            config_path: dir.path().join("config.json"),
            archive_database_path: dir.path().join("archive/history-vault.sqlite"),
            audit_repo_path: dir.path().join("audit"),
            manifests_dir: dir.path().join("audit/manifests"),
            exports_dir: dir.path().join("exports"),
            raw_snapshots_dir: dir.path().join("raw-snapshots"),
            staging_dir: dir.path().join("staging"),
            quarantine_dir: dir.path().join("quarantine"),
            schedule_dir: dir.path().join("schedule"),
            stronghold_path: dir.path().join("vault.hold"),
            stronghold_salt_path: dir.path().join("stronghold-salt.txt"),
        };

        ensure_paths(&paths).expect("ensure paths");
        assert!(paths.stronghold_salt_path.exists());
        assert!(paths.manifests_dir.exists());

        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Encrypted,
            selected_profile_ids: vec!["Default".to_string()],
            ..AppConfig::default()
        };
        save_config(&paths, &config).expect("save config");

        let loaded = load_config(&paths).expect("load config");
        assert!(loaded.initialized);
        assert_eq!(loaded.selected_profile_ids, vec!["Default".to_string()]);
        assert!(matches!(loaded.archive_mode, ArchiveMode::Encrypted));
    }
}
