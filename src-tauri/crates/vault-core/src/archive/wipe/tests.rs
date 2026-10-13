//! Real wipes against a real temp archive built by the normal backup pipeline.
//!
//! Each test maps to one of the failure modes listed in the parent module docs.

use super::*;
use crate::{
    archive::{run_backup, tests::seed_chrome_fixture},
    config::{load_config, project_paths_with_root, save_config},
    utils::{restore_test_env_var, test_env_lock},
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    ffi::OsString,
    sync::{Mutex, MutexGuard},
};
use tempfile::TempDir;

const CHROME_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";

/// The archive database a wipe must remove.
fn archive_file(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join("archive/history-vault.db")
}

/// A temp app root holding a backed-up archive, plus the fixture browser profile it came from.
struct Fixture {
    _guard: MutexGuard<'static, ()>,
    original_chrome_env: Option<OsString>,
    _dir: TempDir,
    paths: ProjectPaths,
    config: AppConfig,
    chrome_root: PathBuf,
    outside_dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        restore_test_env_var(CHROME_OVERRIDE_ENV, self.original_chrome_env.as_deref());
    }
}

fn backed_up_archive() -> Fixture {
    let guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempfile::tempdir().expect("tempdir");
    // The browser profile and the "other disk" live beside the app root, never inside it.
    let chrome_root = seed_chrome_fixture(&dir.path().join("browsers"));
    let outside_dir = dir.path().join("other-disk");
    fs::create_dir_all(&outside_dir).expect("outside dir");
    fs::write(outside_dir.join("keep.bin"), b"not PathKeep's").expect("outside file");
    let original_chrome_env = std::env::var_os(CHROME_OVERRIDE_ENV);
    unsafe { std::env::set_var(CHROME_OVERRIDE_ENV, &chrome_root) };

    let paths = project_paths_with_root(&dir.path().join("app"));
    let mut config = AppConfig {
        initialized: true,
        selected_profile_ids: vec!["chrome:Default".to_string()],
        ..AppConfig::default()
    };
    config
        .ai
        .llm_providers
        .push(crate::models::AiProviderConfig { id: "llm-1".to_string(), ..Default::default() });
    save_config(&paths, &config).expect("save config");
    let vault = crate::Vault::open(paths.clone(), config.clone(), None).expect("open archive");
    let visits = run_backup(&vault, false).expect("backup").run.expect("run").new_visits;
    assert!(visits > 0);
    vault.close().expect("close archive");

    // Things a real install accumulates beside the archive.
    let write = |path: PathBuf, bytes: &[u8]| {
        fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
        fs::write(path, bytes).expect("write");
    };
    write(paths.raw_snapshots_dir.join("fixture/History"), b"source");
    write(paths.audit_repo_path.join("fixture.json"), b"{}");
    write(paths.exports_dir.join("history.csv"), b"url,title\n");
    write(paths.models_dir.join("bge").join("model.safetensors"), &[7; 4096]);
    write(paths.app_root.join("app-lock-passcode.json"), b"{}");
    write(paths.stronghold_path.clone(), b"vault");
    write(paths.app_root.join("derived.bak-20260101T000000Z").join("old.sqlite"), b"old");
    write(paths.archive_database_path.with_extension("sqlite.bak-20260101T000000Z"), b"old");
    write(paths.rust_log_path.clone(), b"log line\n");
    write(paths.schedule_dir.join("attempts").join("a.json"), b"{}");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_dir, paths.models_dir.join("external")).expect("symlink");

    Fixture {
        _guard: guard,
        original_chrome_env,
        _dir: dir,
        paths,
        config,
        chrome_root,
        outside_dir,
    }
}

fn file_contents_under(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), fs::read(&path).expect("read file"));
            }
        }
    }
    files
}

fn no_quiesce() -> Result<()> {
    Ok(())
}

fn no_secrets(_: &WipeOutsideState) -> Result<()> {
    Ok(())
}

fn no_schedule(_: &SchedulePlan) -> Result<()> {
    panic!("no automatic backup was named, so none may be removed")
}

/// An installed automatic backup as the worker would plan it; only its identity matters here.
fn installed_plan() -> SchedulePlan {
    SchedulePlan {
        platform: "macos".to_string(),
        label: "test.pathkeep.backup".to_string(),
        executable_path: "/Applications/PathKeep.app/Contents/MacOS/pathkeep-worker".to_string(),
        generated_files: Vec::new(),
        manual_steps: Vec::new(),
        manual_step_details: Vec::new(),
        apply_commands: Vec::new(),
        rollback_commands: Vec::new(),
        apply_supported: true,
    }
}

#[test]
fn wipe_removes_everything_listed_and_nothing_else() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let browser_before = file_contents_under(&fixture.chrome_root);
    let preview = preview_data_wipe(paths).expect("preview");
    let secrets = WipeOutsideState::from_config(&fixture.config, None);
    let quiesced = Cell::new(false);
    let cleared = Mutex::new(None);

    wipe_all_data(
        paths,
        &secrets,
        || {
            // Workers stop on a missing config, so it must be gone before they are waited on,
            // and the archive must still be there for them to finish against.
            assert!(!paths.config_path.exists());
            assert!(archive_file(paths).exists());
            assert!(data_wipe_interrupted(paths), "the marker is written before deleting");
            quiesced.set(true);
            Ok(())
        },
        |secrets| {
            assert!(!archive_file(paths).exists(), "secrets go after the files");
            *cleared.lock().expect("cleared") = Some(secrets.provider_ids.clone());
            Ok(())
        },
        no_schedule,
    )
    .expect("wipe");

    assert!(quiesced.get());
    assert_eq!(cleared.into_inner().expect("cleared"), Some(secrets.provider_ids));
    for item in &preview.items {
        assert!(fs::symlink_metadata(&item.path).is_err(), "{} survived the wipe", item.path);
    }
    assert!(!data_wipe_interrupted(paths));
    assert_eq!(file_contents_under(&fixture.chrome_root), browser_before);
    assert!(fixture.outside_dir.join("keep.bin").exists(), "a symlink target was followed");
    assert!(paths.rust_log_path.exists());
    assert!(!paths.schedule_dir.exists(), "scheduled-run history goes with the archive");
    assert!(
        paths.archive_database_path.with_file_name(ARCHIVE_WRITE_LOCK_FILE).exists(),
        "the write-lock sentinel must survive"
    );

    let config = load_config(paths).expect("config after wipe");
    assert!(!config.initialized);
    assert!(!paths.app_root.join("archive/history-vault.db").exists());
}

#[test]
fn an_interrupted_wipe_finishes_with_the_saved_provider_list() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let secrets = WipeOutsideState::from_config(&fixture.config, Some(installed_plan()));
    assert_eq!(secrets.provider_ids, vec!["llm-1".to_string()]);

    // Stands in for a crash after the config is gone but before any archive file is.
    let error =
        wipe_all_data(paths, &secrets, || anyhow::bail!("power cut"), no_secrets, no_schedule)
            .expect_err("the cut wipe reports failure");
    assert!(error.to_string().contains("power cut"));
    assert!(data_wipe_interrupted(paths));
    assert!(archive_file(paths).exists());
    assert!(!paths.config_path.exists(), "the app now reads as not initialized");

    let cleared = Mutex::new(None);
    let removed = Mutex::new(None);
    let finished = finish_interrupted_data_wipe(
        paths,
        no_quiesce,
        |secrets| {
            *cleared.lock().expect("cleared") = Some(secrets.provider_ids.clone());
            Ok(())
        },
        |plan| {
            *removed.lock().expect("removed") = Some(plan.label.clone());
            Ok(())
        },
    )
    .expect("finish");
    assert_eq!(finished, Some(WipeReport { schedule_removed: true, schedule_error: None }));
    assert_eq!(cleared.into_inner().expect("cleared"), Some(secrets.provider_ids));
    assert_eq!(
        removed.into_inner().expect("removed"),
        Some(installed_plan().label),
        "the task planned before the crash is the one removed"
    );
    assert!(!archive_file(paths).exists());
    assert!(!data_wipe_interrupted(paths));
    assert!(
        finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
            .expect("nothing to finish")
            .is_none()
    );
}

#[test]
fn a_failed_keychain_step_keeps_the_marker_for_a_retry() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    wipe_all_data(
        paths,
        &WipeOutsideState::default(),
        no_quiesce,
        |_| anyhow::bail!("keychain locked"),
        no_schedule,
    )
    .expect_err("keychain failure surfaces");
    assert!(!archive_file(paths).exists(), "files go before secrets");
    assert!(data_wipe_interrupted(paths));
    assert!(
        finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
            .expect("retry")
            .is_some()
    );
    assert!(!data_wipe_interrupted(paths));
}

#[cfg(unix)]
#[test]
fn the_wipe_waits_for_a_process_holding_the_archive_write_lock() {
    let fixture = backed_up_archive();
    let paths = fixture.paths.clone();
    let foreign = super::super::write_lock::hold_write_lock_as_foreign_process_for_test(&paths);

    let wipe = std::thread::spawn({
        let paths = paths.clone();
        move || {
            wipe_all_data(&paths, &WipeOutsideState::default(), no_quiesce, no_secrets, no_schedule)
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(archive_file(&paths).exists(), "the wipe must not run under another writer");
    assert!(paths.config_path.exists());

    drop(foreign);
    wipe.join().expect("wipe thread").expect("wipe after the lock frees");
    assert!(!archive_file(&paths).exists());
}
