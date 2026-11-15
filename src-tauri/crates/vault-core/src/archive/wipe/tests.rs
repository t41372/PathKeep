//! Real wipes against a real temp archive built by the normal backup pipeline.
//!
//! Each test maps to one of the failure modes listed in the parent module docs.

use super::*;
use crate::{
    archive::{run_backup, tests::seed_chrome_fixture},
    config::{load_config, project_paths_with_root, save_config},
    fault_inject::FaultGuard,
    utils::{restore_test_env_var, test_env_lock},
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    ffi::OsString,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, MutexGuard},
};
use tempfile::TempDir;

const CHROME_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";

/// The points a crash can end a wipe at before its commit. The next start undoes the wipe.
const BEFORE_COMMIT: [&str; 5] = [
    "wipe-intent-written",
    "wipe-control-emptied",
    "wipe-targets-recorded",
    "wipe-target-moved",
    "wipe-moved",
];
/// And after it. The next start finishes the wipe.
const AFTER_COMMIT: [&str; 3] = ["wipe-commit-saved", "wipe-committed", "wipe-files-removed"];

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
    save_config(&crate::ControlGate::acquire(&paths), &config).expect("save config");
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
    // Taking the write lock leaves its sentinel, which a wipe keeps; every wipe takes it.
    drop(ArchiveWriteLock::acquire(&paths).expect("write lock"));

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

/// Every file under `root` by its path from `root`, with its bytes. A symlinked folder is
/// followed, so a target emptied through a link shows.
fn file_contents_under(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = fs::read(&path).expect("read file");
                files.insert(path.strip_prefix(root).expect("under root").to_path_buf(), bytes);
            }
        }
    }
    files
}

/// Fails naming each path whose file is missing, new or changed, rather than printing every byte.
#[track_caller]
fn assert_same_files(
    after: &BTreeMap<PathBuf, Vec<u8>>,
    before: &BTreeMap<PathBuf, Vec<u8>>,
    what: &str,
) {
    let differ: Vec<String> = before
        .keys()
        .chain(after.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| match (before.get(path), after.get(path)) {
            (Some(_), None) => format!("gone: {}", path.display()),
            (None, Some(_)) => format!("new: {}", path.display()),
            _ => format!("changed: {}", path.display()),
        })
        .collect();
    assert!(differ.is_empty(), "{what}: {differ:#?}");
}

/// The `wipe-trash-*` folders in the data folder.
fn trashes(paths: &ProjectPaths) -> Vec<PathBuf> {
    fs::read_dir(&paths.app_root)
        .expect("read data folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("wipe-trash-"))
        })
        .collect()
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

/// Runs a wipe that dies at `cut`, as a crash or a power cut ends it: nothing after that point
/// runs, in the wipe or in any cleanup of its own.
fn crash_at(paths: &ProjectPaths, outside: &WipeOutsideState, cut: &str) {
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        let _fault = FaultGuard::panic_at(cut);
        let _ = wipe_all_data(paths, |_| Ok(outside.clone()), no_quiesce, no_secrets, no_schedule);
    }));
    assert!(crashed.is_err(), "{cut}: the cut was not reached");
}

/// What the next start does with an interrupted wipe.
fn next_start(paths: &ProjectPaths) -> Option<SettledWipe> {
    finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
        .unwrap_or_else(|error| panic!("{error:#}"))
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
        |_| Ok(secrets.clone()),
        || {
            // Workers stop on a not-initialized config, so it must be emptied before they are
            // waited on; it is not removed, so a release that cannot read the archive never
            // finds the folder without one. The archive must still be there for them to finish
            // against.
            assert!(!load_config(paths).expect("emptied config").initialized);
            assert!(archive_file(paths).exists());
            assert!(data_wipe_interrupted(paths), "the intent is written before anything moves");
            quiesced.set(true);
            Ok(())
        },
        |secrets| {
            assert!(!archive_file(paths).exists(), "secrets go after the files");
            assert!(trashes(paths).is_empty(), "secrets go after the trash");
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
    assert!(trashes(paths).is_empty());
    assert_same_files(
        &file_contents_under(&fixture.chrome_root),
        &browser_before,
        "the browser profile",
    );
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

// A wipe that fails before everything is in the trash (a worker that would not stop, a file that
// would not move) has deleted nothing: it puts everything back, byte for byte, the control the
// user had too, before it reports the failure.
#[test]
fn a_wipe_that_fails_before_its_commit_puts_everything_back() {
    for failure in [
        "quiesce",
        "wipe-control-emptied",
        "wipe-targets-recorded",
        "wipe-target-moved",
        "wipe-moved",
    ] {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        let before = file_contents_under(&paths.app_root);
        let secrets = WipeOutsideState::from_config(&fixture.config, None);
        let error = {
            let _fault = (failure != "quiesce").then(|| FaultGuard::error_at(failure));
            wipe_all_data(
                paths,
                |_| Ok(secrets.clone()),
                || {
                    if failure == "quiesce" {
                        anyhow::bail!("a worker would not stop")
                    }
                    Ok(())
                },
                |_| panic!("{failure}: nothing is cleared while nothing is deleted"),
                no_schedule,
            )
            .expect_err("the failure is reported")
        };
        assert!(!format!("{error:#}").contains("archive-wipe-unfinished"), "{failure}: {error:#}");
        assert!(!data_wipe_interrupted(paths), "{failure}");
        assert!(trashes(paths).is_empty(), "{failure}");
        assert_same_files(&file_contents_under(&paths.app_root), &before, failure);
    }
}

// A crash can land between any two durable steps. Until the intent is gone the control is there
// (emptied, so nothing reads the old settings). The next start undoes a wipe that had not moved
// everything into its trash, leaving every file as it was, and finishes one that had. After the
// intent is gone only an empty control can be left, in a folder with nothing else of the user's.
#[test]
fn the_control_is_there_at_every_cut_and_the_next_start_settles_the_wipe() {
    for cut in BEFORE_COMMIT.into_iter().chain(AFTER_COMMIT).chain(["wipe-intent-removed"]) {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        let before = file_contents_under(&paths.app_root);
        crash_at(paths, &WipeOutsideState::from_config(&fixture.config, None), cut);
        assert!(paths.config_path.exists(), "{cut}: the control went missing");
        if cut == "wipe-intent-removed" {
            assert!(!data_wipe_interrupted(paths), "{cut}");
            assert!(!load_config(paths).expect("config").initialized, "{cut}");
            assert!(crate::admission::archive_files(&paths.app_root).unwrap().is_empty(), "{cut}");
            // Setup over the empty control left behind makes a new archive that opens: the
            // stamp of the deleted one, which says its stores are there, is not carried over.
            let mut fresh = AppConfig { initialized: true, ..AppConfig::default() };
            crate::models::normalize_app_config(&mut fresh);
            save_config(&crate::ControlGate::acquire(paths), &fresh).expect("setup");
            let opened = crate::Vault::open(paths.clone(), fresh, None)
                .unwrap_or_else(|error| panic!("{cut}: the new archive did not open: {error:#}"));
            opened.close().unwrap();
            continue;
        }
        assert!(data_wipe_interrupted(paths), "{cut}: the intent went missing");
        let settled = next_start(paths);
        assert!(!data_wipe_interrupted(paths), "{cut}");
        assert!(trashes(paths).is_empty(), "{cut}: a trash was left");
        if BEFORE_COMMIT.contains(&cut) {
            assert_eq!(settled, Some(SettledWipe::RolledBack), "{cut}");
            assert_same_files(
                &file_contents_under(&paths.app_root),
                &before,
                &format!("{cut}: not all is back"),
            );
        } else {
            assert!(matches!(settled, Some(SettledWipe::Finished(_))), "{cut}: {settled:?}");
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
}

// Undoing is itself cut short: everything is back, the emptied control not yet replaced. The
// next start finishes undoing it.
#[test]
fn a_crash_while_a_wipe_is_undone_leaves_the_rest_to_the_next_start() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let before = file_contents_under(&paths.app_root);
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        let _failure = FaultGuard::error_at("wipe-moved");
        let _crash = FaultGuard::panic_at("wipe-put-back");
        let _ = wipe_all_data(
            paths,
            |_| Ok(WipeOutsideState::default()),
            no_quiesce,
            no_secrets,
            no_schedule,
        );
    }));
    assert!(crashed.is_err());
    assert!(data_wipe_interrupted(paths));
    assert!(!load_config(paths).unwrap().initialized, "the control is still the emptied one");
    assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
    assert_same_files(&file_contents_under(&paths.app_root), &before, "");
}

// After its commit a wipe cut short is finished at the next start with what the user had set up
// when it began: the provider list and the automatic backup are gone from `config.json` by then.
#[test]
fn a_wipe_finished_at_the_next_start_clears_what_was_set_up_when_it_began() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let secrets = WipeOutsideState::from_config(&fixture.config, Some(installed_plan()));
    assert_eq!(secrets.provider_ids, vec!["llm-1".to_string()]);
    crash_at(paths, &secrets, "wipe-committed");
    assert!(!load_config(paths).expect("config").initialized, "the app reads as not initialized");

    let cleared = Mutex::new(None);
    let removed = Mutex::new(None);
    let quiesced = Cell::new(false);
    let settled = finish_interrupted_data_wipe(
        paths,
        || {
            assert_eq!(trashes(paths).len(), 1, "background work stops before anything goes");
            quiesced.set(true);
            Ok(())
        },
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
    assert_eq!(
        settled,
        Some(SettledWipe::Finished(WipeReport { schedule_removed: true, schedule_error: None }))
    );
    assert!(quiesced.get());
    assert_eq!(cleared.into_inner().expect("cleared"), Some(secrets.provider_ids));
    assert_eq!(
        removed.into_inner().expect("removed"),
        Some(installed_plan().label),
        "the task planned before the crash is the one removed"
    );
    assert!(!archive_file(paths).exists());
    assert!(!data_wipe_interrupted(paths));
    assert!(next_start(paths).is_none());
}

// The archive is gone by then: the error says so, for the window to tell the user.
#[test]
fn a_failed_keychain_step_keeps_the_intent_for_a_retry() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let error = wipe_all_data(
        paths,
        |_| Ok(WipeOutsideState::default()),
        no_quiesce,
        |_| anyhow::bail!("keychain locked"),
        no_schedule,
    )
    .map(|_| ())
    .expect_err("keychain failure surfaces");
    assert!(format!("{error:#}").contains("archive-wipe-unfinished:"), "{error:#}");
    assert!(!archive_file(paths).exists(), "files go before secrets");
    assert!(trashes(paths).is_empty());
    assert!(data_wipe_interrupted(paths));
    assert!(matches!(next_start(paths), Some(SettledWipe::Finished(_))));
    assert!(!data_wipe_interrupted(paths));
}

// "Delete all data" again before a restart: the intent of the first, whose keychain step failed,
// still names the provider keys to clear. The second wipe settles it before writing its own,
// whose list, read from the emptied control, would be empty.
#[test]
fn a_second_wipe_settles_the_intent_an_earlier_one_left() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let secrets = WipeOutsideState::from_config(&fixture.config, None);
    wipe_all_data(
        paths,
        |_| Ok(secrets.clone()),
        no_quiesce,
        |_| anyhow::bail!("keychain locked"),
        no_schedule,
    )
    .expect_err("keychain failure surfaces");
    let cleared = Mutex::new(Vec::new());
    wipe_all_data(
        paths,
        |config| Ok(WipeOutsideState::from_config(config, None)),
        no_quiesce,
        |secrets| {
            cleared.lock().expect("cleared").push(secrets.provider_ids.clone());
            Ok(())
        },
        no_schedule,
    )
    .expect("the second wipe");
    let cleared = cleared.into_inner().expect("cleared");
    assert!(cleared.contains(&secrets.provider_ids), "the first wipe's keys stayed: {cleared:?}");
    assert!(!data_wipe_interrupted(paths));
    assert!(trashes(paths).is_empty());
    assert!(!paths.config_path.exists());
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
            wipe_all_data(
                &paths,
                |_| Ok(WipeOutsideState::default()),
                no_quiesce,
                no_secrets,
                no_schedule,
            )
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(archive_file(&paths).exists(), "the wipe must not run under another writer");
    assert!(paths.config_path.exists());

    drop(foreign);
    wipe.join().expect("wipe thread").expect("wipe after the lock frees");
    assert!(!archive_file(&paths).exists());
}

// Something now stands where an entry of the trash goes back, and beside it: a store and its log
// from somewhere else. Undoing gives back the archive the user confirmed deleting, whole: a store
// is never left beside another store's log, which SQLite would read with it. What stood there is
// moved into the trash, which is kept, listed with the next "Delete all data" and removed by it.
#[test]
fn undoing_gives_the_archive_back_whole_and_keeps_what_stood_in_its_place() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let before = file_contents_under(&paths.app_root);
    crash_at(paths, &WipeOutsideState::default(), "wipe-moved");
    for suffix in ["", "-wal", "-shm"] {
        let path = PathBuf::from(format!("{}{suffix}", archive_file(paths).display()));
        fs::write(path, b"another store's").unwrap();
    }

    assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
    let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("the trash is kept");
    let trash_name = PathBuf::from(trash.file_name().unwrap());
    let (kept, after): (BTreeMap<_, _>, BTreeMap<_, _>) = file_contents_under(&paths.app_root)
        .into_iter()
        .partition(|(path, _)| path.starts_with(&trash_name));
    assert_same_files(&after, &before, "the archive the user confirmed deleting");
    assert_eq!(kept.len(), 3, "{:#?}", kept.keys());
    for (path, bytes) in &kept {
        assert!(path.strip_prefix(&trash_name).unwrap().starts_with(DISPLACED), "{path:?}");
        assert_eq!(bytes, b"another store's", "{path:?}");
    }

    let preview = preview_data_wipe(paths).unwrap();
    assert!(preview.items.iter().any(|item| Path::new(&item.path) == trash), "{preview:?}");
    wipe_all_data(paths, |_| Ok(WipeOutsideState::default()), no_quiesce, no_secrets, no_schedule)
        .expect("wipe");
    assert!(trashes(paths).is_empty());
}

// After the commit, whatever appears at a target before the wipe ends (a worker that did not
// stop, a scheduled run) is part of the folder the user confirmed deleting: it goes too.
#[test]
fn what_appears_at_a_target_after_the_commit_goes_too() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    crash_at(paths, &WipeOutsideState::default(), "wipe-committed");
    fs::create_dir_all(paths.app_root.join("derived")).unwrap();
    fs::write(paths.app_root.join("derived/agent.db"), b"written since").unwrap();
    assert!(matches!(next_start(paths), Some(SettledWipe::Finished(_))));
    assert!(!paths.app_root.join("derived").exists());
    assert!(matches!(
        crate::admission::inspect(&paths.app_root),
        Ok(crate::admission::RootState::Uninitialized)
    ));
}

// After the commit someone put back the control the user confirmed deleting. The wipe still
// deletes its own trash, which is what the user confirmed; nothing else of the folder is its to
// delete any more, and no key is cleared for that control.
#[test]
fn where_the_confirmed_control_came_back_after_the_commit_only_the_trash_goes() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let confirmed = fs::read(&paths.config_path).unwrap();
    crash_at(
        paths,
        &WipeOutsideState::from_config(&fixture.config, Some(installed_plan())),
        "wipe-committed",
    );
    fs::write(&paths.config_path, &confirmed).unwrap();
    fs::create_dir_all(paths.app_root.join("derived")).unwrap();
    fs::write(paths.app_root.join("derived/agent.db"), b"written since").unwrap();
    let settled = finish_interrupted_data_wipe(
        paths,
        no_quiesce,
        |_| panic!("no keys are cleared beside another control"),
        no_schedule,
    )
    .unwrap();
    assert_eq!(settled, Some(SettledWipe::Finished(WipeReport::default())));
    assert!(trashes(paths).is_empty());
    assert!(!archive_file(paths).exists());
    assert!(!data_wipe_interrupted(paths));
    assert_eq!(fs::read(&paths.config_path).unwrap(), confirmed);
    assert_eq!(fs::read(paths.app_root.join("derived/agent.db")).unwrap(), b"written since");
}

// The intent is written whole or not at all. One that is not this build's shape was not left by
// a wipe, and one that names as its trash anything but a `wipe-trash-*` folder directly in the
// data folder would move into it or delete it: nothing is moved or deleted on either.
#[test]
fn an_intent_this_build_did_not_write_moves_and_deletes_nothing() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let control = fs::read_to_string(&paths.config_path).unwrap();
    let shaped = |trash: &str, committed: bool| {
        serde_json::json!({
            "identity": "pathkeep-wipe", "version": 1, "control": control, "emptied": control,
            "trash": trash, "committed": committed, "providerIds": [], "schedule": null,
        })
        .to_string()
    };
    for intent in [
        "{\"providerIds\":".to_string(),
        r#"{"identity":"pathkeep-wipe","version":2,"providerIds":[]}"#.to_string(),
        r#"{"identity":"something-else","version":1,"providerIds":[]}"#.to_string(),
        r#"{"providerIds":[]}"#.to_string(),
        shaped("archive", true),
        shaped("archive", false),
        shaped("derived", true),
        shaped("", true),
        shaped("../wipe-trash-elsewhere", true),
        shaped("wipe-trash-x/../archive", false),
        shaped(&paths.app_root.join("wipe-trash-x").display().to_string(), true),
    ] {
        fs::write(paths.app_root.join(".pk-wipe-v2.json"), &intent).unwrap();
        let before = file_contents_under(&paths.app_root);
        let error = finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
            .expect_err("refused");
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{intent}: {error:#}");
        assert_same_files(&file_contents_under(&paths.app_root), &before, &intent);
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
    let quiesced = Cell::new(false);
    let settled = finish_interrupted_data_wipe(
        paths,
        || {
            assert!(archive_file(paths).exists(), "background work stops before anything goes");
            quiesced.set(true);
            Ok(())
        },
        no_secrets,
        no_schedule,
    )
    .unwrap();
    assert!(matches!(settled, Some(SettledWipe::Finished(_))), "{settled:?}");
    assert!(quiesced.get());
    assert!(!data_wipe_interrupted(paths));
    assert!(!archive_file(paths).exists());
    assert!(!paths.config_path.exists());
}

// PathKeep 0.5 removes the control right after writing its marker, before it deletes anything, and
// it cannot read a stamped archive at all. So its marker beside a control, stamped or not, is from
// a wipe that deleted nothing yet, or from another folder: it is discarded and nothing is deleted.
// The user still sees the archive and can delete it again.
#[test]
fn a_pathkeep_0_5_marker_beside_a_control_deletes_nothing() {
    for stamped in [true, false] {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        assert_eq!(
            crate::admission::root_layout(&paths.app_root).unwrap(),
            vault_layout::ArchiveLayout::AgentHistoryV2,
            "the fixture's first open stamped it"
        );
        if !stamped {
            let control = crate::config::encode_config(&fixture.config, None).unwrap();
            fs::write(&paths.config_path, control).unwrap();
        }
        let marker =
            serde_json::to_vec(&WipeOutsideState::from_config(&fixture.config, None)).unwrap();
        fs::write(paths.app_root.join(".pk-wipe-in-progress.json"), marker).unwrap();
        let mut before = file_contents_under(&paths.app_root);
        assert_eq!(next_start(paths), Some(SettledWipe::Discarded), "stamped: {stamped}");
        assert!(!data_wipe_interrupted(paths), "stamped: {stamped}: the marker is discarded");
        before.remove(Path::new(".pk-wipe-in-progress.json"));
        assert_same_files(
            &file_contents_under(&paths.app_root),
            &before,
            &format!("stamped: {stamped}"),
        );
    }
}

/// Copies `from` into `to` file by file: new files with the same bytes, the way a copy of a
/// folder, a clone or a restore from a backup makes them. Symlinks are copied as links; sockets
/// and the like, which no copy tool carries, are left out.
#[cfg(unix)]
fn copy_tree(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let target = to.join(entry.path().strip_prefix(from).unwrap());
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).unwrap();
        } else if entry.file_type().is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path()).unwrap(), &target).unwrap();
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

// A wipe cut short, and a copy of the folder made before the next start (a backup of the folder
// restored somewhere else, a clone). The copy holds the same control, intent and trash, files of
// its own with the same bytes. It is the same folder to the record, and undoing the wipe there
// gives the copy its archive back; the folder itself gets its own back too.
#[cfg(unix)]
#[test]
fn a_copy_of_the_folder_made_during_a_wipe_gets_its_archive_back() {
    for cut in BEFORE_COMMIT {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        let before = file_contents_under(&paths.app_root);
        crash_at(paths, &WipeOutsideState::default(), cut);
        let copy = project_paths_with_root(&fixture._dir.path().join("copy"));
        copy_tree(&paths.app_root, &copy.app_root);

        assert_eq!(next_start(&copy), Some(SettledWipe::RolledBack), "{cut}");
        assert_same_files(
            &file_contents_under(&copy.app_root),
            &before,
            &format!("{cut}: the copy"),
        );
        assert_eq!(next_start(paths), Some(SettledWipe::RolledBack), "{cut}");
        assert_same_files(&file_contents_under(&paths.app_root), &before, cut);
    }
}

// A wipe the user confirmed for one archive, cut short, and another archive's control in the
// folder by the next start (restored from a backup, imported, carried in): the record does not
// describe it. Nothing is put back or deleted, the trash included, and the record is discarded.
#[test]
fn a_wipe_record_does_not_touch_another_archive() {
    for cut in ["wipe-target-moved", "wipe-committed"] {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        let confirmed = fs::read(&paths.config_path).unwrap();
        crash_at(paths, &WipeOutsideState::default(), cut);
        let mut other = fixture.config.clone();
        other.preferred_language = crate::LanguagePreference::ZhTw;
        save_config(&crate::ControlGate::acquire(paths), &other).unwrap();
        let mut before = file_contents_under(&paths.app_root);

        assert_eq!(next_start(paths), Some(SettledWipe::Discarded), "{cut}");
        assert!(!data_wipe_interrupted(paths), "{cut}: the stale record is discarded");
        before.remove(Path::new(".pk-wipe-v2.json"));
        let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("the trash is kept");
        // The record was the only copy of the control the user confirmed: it stays with the trash.
        let control = trash.join("config.json");
        assert_eq!(fs::read(&control).unwrap(), confirmed, "{cut}");
        before.insert(control.strip_prefix(&paths.app_root).unwrap().to_path_buf(), confirmed);
        assert_same_files(&file_contents_under(&paths.app_root), &before, cut);
    }
}

// The record carries the control the user confirmed and the emptied control it writes, exactly,
// so the check after a crash compares bytes on disk with bytes in the record, whichever build
// settles the wipe, and undoing it writes back the very bytes there were.
#[test]
fn the_record_holds_both_controls_and_names_its_trash() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let confirmed = fs::read_to_string(&paths.config_path).unwrap();
    crash_at(paths, &WipeOutsideState::default(), "wipe-control-emptied");
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.app_root.join(".pk-wipe-v2.json")).unwrap())
            .unwrap();
    assert_eq!(record["control"], confirmed);
    assert_eq!(
        record["emptied"].as_str().unwrap().as_bytes(),
        fs::read(&paths.config_path).unwrap()
    );
    assert_eq!(record["committed"], false);
    let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("one trash");
    assert_eq!(record["trash"].as_str(), trash.file_name().unwrap().to_str());
    assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
    assert_eq!(fs::read_to_string(&paths.config_path).unwrap(), confirmed);
}

// A folder without a control (nothing but downloaded models in it, say) is wiped too. The emptied
// control goes in before the intent, so the intent never lies alone, which would read as an
// archive whose control is missing; and undoing the wipe leaves the folder without one again.
#[test]
fn a_wipe_where_there_was_no_control_never_leaves_its_intent_alone() {
    let _guard = test_env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(&dir.path().join("app"));
    fs::create_dir_all(paths.models_dir.join("bge")).unwrap();
    fs::write(paths.models_dir.join("bge/model.safetensors"), [7; 64]).unwrap();
    // Taking the write lock leaves its sentinel, which a wipe keeps.
    drop(ArchiveWriteLock::acquire(&paths).unwrap());
    let before = file_contents_under(&paths.app_root);
    for cut in ["wipe-intent-written", "wipe-target-moved", "wipe-moved"] {
        crash_at(&paths, &WipeOutsideState::default(), cut);
        assert!(paths.config_path.exists(), "{cut}: the intent lies alone");
        // Stamped: PathKeep 0.5 would take a not-initialized control without one for a first run.
        let control = crate::admission::guard(&paths.app_root).unwrap().expect("a control");
        assert!(control.compatibility.is_some(), "{cut}: the emptied control has no stamp");
        assert_eq!(next_start(&paths), Some(SettledWipe::RolledBack), "{cut}");
        assert_same_files(&file_contents_under(&paths.app_root), &before, cut);
    }
    wipe_all_data(&paths, |_| Ok(WipeOutsideState::default()), no_quiesce, no_secrets, no_schedule)
        .expect("wipe");
    assert!(!paths.models_dir.exists());
    assert!(!paths.config_path.exists());
    assert!(trashes(&paths).is_empty());
}

// A crash during an atomic write leaves its temporary file beside the target, a full copy of the
// settings if the target was config.json. The wipe removes it with the rest.
#[test]
fn a_leftover_temporary_settings_file_does_not_survive_the_wipe() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let leftover = paths.app_root.join(".pk-durable-1a2b3c");
    fs::write(&leftover, fs::read(&paths.config_path).unwrap()).unwrap();
    wipe_all_data(paths, |_| Ok(WipeOutsideState::default()), no_quiesce, no_secrets, no_schedule)
        .expect("wipe");
    assert!(!leftover.exists());
}

// Nothing is restored or imported over a folder whose confirmed wipe has not been settled: the
// next start would act on it, in a folder that no longer holds what it was written for.
#[test]
fn nothing_is_installed_while_a_wipe_is_pending() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    crash_at(paths, &WipeOutsideState::default(), "wipe-intent-written");
    let restore = crate::maintenance::restore_snapshot_to(
        paths,
        None,
        None,
        &paths.app_root.join("raw-snapshots/none"),
        None,
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

// A copy of the folder made during a wipe, cut short while it copied the trash: an entry the wipe
// moved is in neither place. Undoing it would report the archive back while a part is gone.
// Nothing moves, and the record stays.
#[test]
fn a_target_in_neither_place_stops_the_undo() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    crash_at(paths, &WipeOutsideState::default(), "wipe-moved");
    let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("one trash");
    fs::remove_file(trash.join("archive/history-vault.db")).unwrap();
    let before = file_contents_under(&paths.app_root);

    let error = finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
        .expect_err("refused");
    assert!(format!("{error:#}").contains("neither in the archive nor in"), "{error:#}");
    assert!(data_wipe_interrupted(paths));
    assert_same_files(&file_contents_under(&paths.app_root), &before, "");
}

// The commit is written, and the flush after its rename fails: the record on disk may say
// committed. Putting everything back now would meet that record at the next start, which would
// delete what was put back. The wipe stops where it is, says so, and the next start goes the way
// the record on disk says.
#[test]
fn a_commit_that_reports_an_error_is_left_to_the_next_start() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let error = {
        let _flush = FaultGuard::error_at("wipe-commit-saved");
        wipe_all_data(
            paths,
            |_| Ok(WipeOutsideState::default()),
            no_quiesce,
            no_secrets,
            no_schedule,
        )
        .expect_err("the failure is reported")
    };
    assert!(format!("{error:#}").contains("archive-wipe-stopped"), "{error:#}");
    assert!(data_wipe_interrupted(paths));
    assert!(!archive_file(paths).exists(), "nothing was put back in the same call");
    assert!(matches!(next_start(paths), Some(SettledWipe::Finished(_))));
    assert!(!paths.config_path.exists());
    assert!(trashes(paths).is_empty());
}

// A power cut on exFAT while an entry was put back can leave it under both names, in its place
// and in the trash. Deleting the one in the trash would free data the archive still uses: undoing
// deletes nothing that stood in an entry's place, and the trash that holds the other name is kept.
// A hard link stands in for the two names.
#[cfg(unix)]
#[test]
fn an_entry_under_two_names_after_a_cut_move_keeps_both() {
    use std::os::unix::fs::MetadataExt;
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let original = fs::read(archive_file(paths)).unwrap();
    crash_at(paths, &WipeOutsideState::default(), "wipe-moved");
    let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("one trash");
    fs::hard_link(trash.join("archive/history-vault.db"), archive_file(paths)).unwrap();

    assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
    assert_eq!(fs::read(archive_file(paths)).unwrap(), original);
    assert_eq!(fs::metadata(archive_file(paths)).unwrap().nlink(), 2, "a name was deleted");
    assert_eq!(trashes(paths).len(), 1, "the trash holding the other name is kept");
}
