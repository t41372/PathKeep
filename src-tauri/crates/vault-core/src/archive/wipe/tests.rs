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
    let visits = run_backup(&vault, None).expect("backup").run.expect("run").new_visits;
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
    write(paths.app_root.join("stronghold-salt.txt"), b"2026-01-01T00:00:00Z\n");
    write(paths.app_root.join("derived.bak-20260101T000000Z").join("old.sqlite"), b"old");
    write(paths.legacy.archive.with_extension("sqlite.bak-20260101T000000Z"), b"old");
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
        wake_interval_seconds: 3_600,
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
            // Workers stop on a not-initialized config, so it must be emptied before they are
            // waited on; it is not removed, so a release that cannot read the archive never
            // finds the folder without one. The archive must still be there for them to finish
            // against.
            assert!(!load_config(paths).expect("emptied config").initialized);
            assert!(archive_file(paths).exists());
            assert!(data_wipe_interrupted(paths), "the intent is written before deleting");
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
        paths.archive_db.with_file_name(ARCHIVE_WRITE_LOCK_FILE).exists(),
        "the write-lock sentinel must survive"
    );

    let config = load_config(paths).expect("config after wipe");
    assert!(!config.initialized);
    assert!(!paths.config_path.exists(), "the control is removed, last");
    assert!(!paths.app_root.join("archive/history-vault.db").exists());
    assert!(matches!(
        crate::admission::inspect(&paths.app_root),
        Ok(crate::admission::RootState::Uninitialized)
    ));
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
    assert!(paths.config_path.exists(), "the control stays while data is there");
    assert!(!load_config(paths).expect("config").initialized, "the app reads as not initialized");

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

// A crash can land between any two durable steps. Until the intent is gone the control is there
// (emptied, so nothing reads the old settings), and the next start finishes the wipe. After the
// intent is gone only an empty control can be left, in a folder with nothing else of the user's.
#[test]
fn the_control_is_there_at_every_cut_and_the_next_start_finishes() {
    for cut in
        ["wipe-intent-written", "wipe-control-emptied", "wipe-files-removed", "wipe-intent-removed"]
    {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        let secrets = WipeOutsideState::from_config(&fixture.config, None);
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _fault = crate::fault_inject::FaultGuard::panic_at(cut);
            let _ = wipe_all_data(paths, &secrets, no_quiesce, no_secrets, no_schedule);
        }));
        assert!(crashed.is_err(), "{cut}: the cut was not reached");
        assert!(paths.config_path.exists(), "{cut}: the control went missing");
        if cut == "wipe-intent-removed" {
            assert!(!data_wipe_interrupted(paths), "{cut}");
            assert!(!load_config(paths).expect("config").initialized, "{cut}");
            assert!(crate::admission::archive_files(&paths.app_root).unwrap().is_empty(), "{cut}");
            continue;
        }
        assert!(data_wipe_interrupted(paths), "{cut}: the intent went missing");
        finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
            .unwrap_or_else(|error| panic!("{cut}: {error:#}"))
            .unwrap_or_else(|| panic!("{cut}: nothing was finished"));
        assert!(!data_wipe_interrupted(paths), "{cut}");
        assert!(!paths.config_path.exists(), "{cut}");
        assert!(!archive_file(paths).exists(), "{cut}");
        assert!(
            matches!(
                crate::admission::inspect(&paths.app_root),
                Ok(crate::admission::RootState::Uninitialized)
            ),
            "{cut}"
        );
    }
}

// The intent is written whole or not at all. One that is not this build's shape was not left by
// a wipe, so nothing is deleted on its say-so, whatever the control says.
#[test]
fn an_intent_this_build_did_not_write_deletes_nothing() {
    for intent in [
        "{\"providerIds\":",
        r#"{"identity":"pathkeep-wipe","version":2,"providerIds":[]}"#,
        r#"{"identity":"something-else","version":1,"providerIds":[]}"#,
        r#"{"providerIds":[]}"#,
    ] {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        fs::write(paths.app_root.join(".pk-wipe-v2.json"), intent).unwrap();
        let before = file_contents_under(&paths.app_root);
        let error = finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
            .expect_err("refused");
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{intent}: {error:#}");
        assert_eq!(file_contents_under(&paths.app_root), before, "{intent}");
    }
}

// A wipe PathKeep 0.5 started and a crash cut short: its own marker, its control already gone.
// That release would finish it at its next start, and so does this one.
#[test]
fn a_wipe_pathkeep_0_5_left_is_still_finished() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let marker = serde_json::to_vec(&WipeOutsideState::from_config(&fixture.config, None)).unwrap();
    fs::write(paths.app_root.join(".pk-wipe-in-progress.json"), marker).unwrap();
    fs::remove_file(&paths.config_path).unwrap();
    assert!(data_wipe_interrupted(paths));
    finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
        .expect("finish")
        .expect("there was one");
    assert!(!data_wipe_interrupted(paths));
    assert!(!archive_file(paths).exists());
    assert!(!paths.config_path.exists());
}

// PathKeep 0.5 cannot read a stamped archive, so it cannot have started deleting one: its marker
// beside a stamped control is left over from an archive that is gone, and deletes nothing.
#[test]
fn a_pathkeep_0_5_marker_beside_a_stamped_control_deletes_nothing() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    assert_eq!(
        crate::admission::root_layout(&paths.app_root).unwrap(),
        vault_layout::ArchiveLayout::AgentHistoryV2,
        "the fixture's first open stamped it"
    );
    let marker = serde_json::to_vec(&WipeOutsideState::from_config(&fixture.config, None)).unwrap();
    fs::write(paths.app_root.join(".pk-wipe-in-progress.json"), marker).unwrap();
    let mut before = file_contents_under(&paths.app_root);
    let finished =
        finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule).unwrap();
    assert!(finished.is_none());
    assert!(!data_wipe_interrupted(paths), "the stale marker is discarded");
    before.remove(&paths.app_root.join(".pk-wipe-in-progress.json"));
    // Taking the write lock creates its lock file; every file that was there is there unchanged.
    let after = file_contents_under(&paths.app_root);
    for (path, bytes) in &before {
        assert_eq!(after.get(path), Some(bytes), "{} changed", path.display());
    }
}

// A wipe the user confirmed for one archive, cut short, and another archive in the folder by the
// next start (restored from a backup, imported, carried in): the record does not describe it.
// Nothing of it is deleted, and the record is discarded.
#[test]
fn a_wipe_record_does_not_delete_another_archive() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let secrets = WipeOutsideState::from_config(&fixture.config, None);
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _fault = crate::fault_inject::FaultGuard::panic_at("wipe-intent-written");
        let _ = wipe_all_data(paths, &secrets, no_quiesce, no_secrets, no_schedule);
    }));
    assert!(crashed.is_err());
    let mut other = fixture.config.clone();
    other.preferred_language = crate::LanguagePreference::ZhTw;
    save_config(paths, &other).unwrap();
    let before = file_contents_under(&paths.app_root);
    let finished =
        finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule).unwrap();
    assert!(finished.is_none());
    assert!(!data_wipe_interrupted(paths), "the stale record is discarded");
    let mut after = file_contents_under(&paths.app_root);
    after.remove(&paths.app_root.join(".pk-wipe-v2.json"));
    let mut before = before;
    before.remove(&paths.app_root.join(".pk-wipe-v2.json"));
    assert_eq!(after, before, "nothing of the other archive changed");
}

// The record carries the emptied control it writes, exactly, so the check after a crash compares
// bytes on disk with bytes in the record, whichever build finishes the wipe.
#[test]
fn the_record_holds_the_emptied_control_it_writes() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let secrets = WipeOutsideState::from_config(&fixture.config, None);
    let confirmed = crate::utils::sha256_hex(&fs::read(&paths.config_path).unwrap());
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _fault = crate::fault_inject::FaultGuard::panic_at("wipe-control-emptied");
        let _ = wipe_all_data(paths, &secrets, no_quiesce, no_secrets, no_schedule);
    }));
    assert!(crashed.is_err());
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.app_root.join(".pk-wipe-v2.json")).unwrap())
            .unwrap();
    assert_eq!(record["control"], confirmed);
    assert_eq!(
        record["emptied"].as_str().unwrap().as_bytes(),
        fs::read(&paths.config_path).unwrap()
    );
    finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule).unwrap().unwrap();
    assert!(!archive_file(paths).exists());
}

// A crash during an atomic write leaves its temporary file beside the target, a full copy of the
// settings if the target was config.json. The wipe removes it with the rest.
#[test]
fn a_leftover_temporary_settings_file_does_not_survive_the_wipe() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let leftover = paths.app_root.join(".pk-durable-1a2b3c");
    fs::write(&leftover, fs::read(&paths.config_path).unwrap()).unwrap();
    wipe_all_data(paths, &WipeOutsideState::default(), no_quiesce, no_secrets, no_schedule)
        .expect("wipe");
    assert!(!leftover.exists());
}

// Nothing is restored or imported over a folder whose confirmed wipe has not finished: the next
// start would finish it and delete what was installed.
#[test]
fn nothing_is_installed_while_a_wipe_is_pending() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _fault = crate::fault_inject::FaultGuard::panic_at("wipe-intent-written");
        let _ =
            wipe_all_data(paths, &WipeOutsideState::default(), no_quiesce, no_secrets, no_schedule);
    }));
    assert!(crashed.is_err());
    let restore = crate::maintenance::restore_snapshot_to(
        paths,
        None,
        None,
        &paths.app_root.join("raw-snapshots/none"),
        None,
        Default::default(),
    );
    let import = crate::migration::native::import(
        paths,
        None,
        None,
        &paths.app_root.join("none.pkexport"),
        &crate::ApplyImportOptions {
            source_archive_key: Some("x".into()),
            confirm_overwrite: true,
        },
        Default::default(),
    );
    for result in [restore.map(|_| ()), import.map(|_| ())] {
        let error = result.expect_err("refused");
        assert!(format!("{error:#}").contains("archive-wipe-pending"), "{error:#}");
    }
}
