use crate::{
    config::ProjectPaths,
    models::ChromeProfile,
    utils::{file_sha256_hex, now_rfc3339},
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";

#[derive(Debug)]
pub struct FileFingerprint {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug)]
pub struct ProfileSnapshot {
    pub profile: ChromeProfile,
    pub temp_dir: TempDir,
    pub history_path: PathBuf,
    pub favicons_path: Option<PathBuf>,
    pub source_hashes: Vec<FileFingerprint>,
}

pub fn discover_profiles() -> Result<Vec<ChromeProfile>> {
    let user_data_dir = chrome_user_data_dir()?;
    let local_state_path = user_data_dir.join("Local State");
    if !local_state_path.exists() {
        return Ok(Vec::new());
    }

    let local_state = fs::read_to_string(&local_state_path)
        .with_context(|| format!("reading {}", local_state_path.display()))?;
    let json: Value = serde_json::from_str(&local_state)?;
    let info_cache = json
        .get("profile")
        .and_then(|profile| profile.get("info_cache"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let chrome_version = fs::read_to_string(user_data_dir.join("Last Version"))
        .ok()
        .map(|content| content.trim().to_string());

    let mut profiles = info_cache
        .into_iter()
        .map(|(profile_id, details)| {
            let profile_path = user_data_dir.join(&profile_id);
            let history_path = profile_path.join("History");
            let favicons_path = profile_path.join("Favicons");
            ChromeProfile {
                profile_name: details
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or(&profile_id)
                    .to_string(),
                user_name: details
                    .get("user_name")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                profile_path: profile_path.display().to_string(),
                history_path: history_path.exists().then(|| history_path.display().to_string()),
                favicons_path: favicons_path.exists().then(|| favicons_path.display().to_string()),
                history_exists: history_path.exists(),
                chrome_version: chrome_version.clone(),
                profile_id,
            }
        })
        .collect::<Vec<_>>();

    profiles.sort_by(|left, right| left.profile_name.cmp(&right.profile_name));
    Ok(profiles)
}

pub fn stage_profile_snapshot(
    paths: &ProjectPaths,
    profile: &ChromeProfile,
) -> Result<ProfileSnapshot> {
    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("{}-{}", profile.profile_id, now_rfc3339().replace(':', "-")))
        .tempdir_in(&paths.staging_dir)
        .with_context(|| format!("creating temp dir in {}", paths.staging_dir.display()))?;
    let source_dir = PathBuf::from(&profile.profile_path);
    let history_path = copy_database_with_sidecars(&source_dir, "History", temp_dir.path())?;
    let favicons_path = if profile.favicons_path.is_some() {
        Some(copy_database_with_sidecars(&source_dir, "Favicons", temp_dir.path())?)
    } else {
        None
    };

    let mut source_hashes = Vec::new();
    for path in [Some(history_path.clone()), favicons_path.clone()].into_iter().flatten() {
        source_hashes.push(FileFingerprint {
            sha256: file_sha256_hex(&path)?,
            path: path.display().to_string(),
        });
    }

    Ok(ProfileSnapshot {
        profile: profile.clone(),
        temp_dir,
        history_path,
        favicons_path,
        source_hashes,
    })
}

pub fn chrome_user_data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(CHROME_USER_DATA_OVERRIDE_ENV) {
        return Ok(PathBuf::from(path));
    }

    let home =
        directories::UserDirs::new().context("resolving home directory")?.home_dir().to_path_buf();

    let path = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Google/Chrome")
    } else if cfg!(target_os = "windows") {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("reading LOCALAPPDATA")?
            .join("Google/Chrome/User Data")
    } else {
        home.join(".config/google-chrome")
    };

    Ok(path)
}

fn copy_database_with_sidecars(
    source_dir: &Path,
    base_name: &str,
    destination_dir: &Path,
) -> Result<PathBuf> {
    let destination = destination_dir.join(base_name);
    fs::copy(source_dir.join(base_name), &destination).with_context(|| {
        format!("copying {} to {}", source_dir.join(base_name).display(), destination.display())
    })?;

    for suffix in ["-wal", "-shm", "-journal"] {
        let source_sidecar = source_dir.join(format!("{base_name}{suffix}"));
        if source_sidecar.exists() {
            let target_sidecar = destination_dir.join(format!("{base_name}{suffix}"));
            fs::copy(&source_sidecar, &target_sidecar).with_context(|| {
                format!("copying {} to {}", source_sidecar.display(), target_sidecar.display())
            })?;
        }
    }

    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectPaths;
    use std::io::Write;
    use tempfile::tempdir;

    fn sample_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
    }

    #[test]
    fn discover_profiles_reads_local_state_from_override() {
        let dir = tempdir().expect("tempdir");
        let default_dir = dir.path().join("Default");
        fs::create_dir_all(&default_dir).expect("create profile");
        fs::write(default_dir.join("History"), b"sqlite").expect("write history");
        fs::write(dir.path().join("Last Version"), "135.0.0.0").expect("write last version");
        fs::write(
            dir.path().join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Personal","user_name":"tim@example.com"}}}}"#,
        )
        .expect("write local state");

        unsafe {
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, dir.path());
        }
        let profiles = discover_profiles().expect("discover");
        unsafe {
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        }

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].profile_name, "Personal");
        assert_eq!(profiles[0].user_name.as_deref(), Some("tim@example.com"));
        assert!(profiles[0].history_exists);
        assert_eq!(profiles[0].chrome_version.as_deref(), Some("135.0.0.0"));
    }

    #[test]
    fn stage_profile_snapshot_copies_database_and_sidecars() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        fs::create_dir_all(&paths.staging_dir).expect("create staging");

        let profile_dir = dir.path().join("Profile 1");
        fs::create_dir_all(&profile_dir).expect("create profile dir");
        fs::write(profile_dir.join("History"), b"history-db").expect("write history");
        fs::write(profile_dir.join("History-journal"), b"journal").expect("write journal");
        fs::write(profile_dir.join("Favicons"), b"favicons-db").expect("write favicons");

        let profile = ChromeProfile {
            profile_id: "Profile 1".to_string(),
            profile_name: "Work".to_string(),
            user_name: None,
            profile_path: profile_dir.display().to_string(),
            history_path: Some(profile_dir.join("History").display().to_string()),
            favicons_path: Some(profile_dir.join("Favicons").display().to_string()),
            history_exists: true,
            chrome_version: Some("135.0.0.0".to_string()),
        };

        let snapshot = stage_profile_snapshot(&paths, &profile).expect("snapshot");
        assert!(snapshot.history_path.exists());
        assert!(snapshot.temp_dir.path().join("History-journal").exists());
        assert!(snapshot.favicons_path.as_ref().expect("favicons").exists());
        assert_eq!(snapshot.source_hashes.len(), 2);
    }

    #[test]
    fn copy_database_with_sidecars_copies_known_sidecars_only() {
        let source = tempdir().expect("source");
        let destination = tempdir().expect("dest");
        fs::write(source.path().join("History"), b"history").expect("history");
        fs::write(source.path().join("History-wal"), b"wal").expect("wal");
        fs::write(source.path().join("History-shm"), b"shm").expect("shm");
        let mut ignored = fs::File::create(source.path().join("History-random")).expect("ignored");
        ignored.write_all(b"ignored").expect("write ignored");

        let copied = copy_database_with_sidecars(source.path(), "History", destination.path())
            .expect("copy");

        assert_eq!(copied, destination.path().join("History"));
        assert!(destination.path().join("History").exists());
        assert!(destination.path().join("History-wal").exists());
        assert!(destination.path().join("History-shm").exists());
        assert!(!destination.path().join("History-random").exists());
    }
}
