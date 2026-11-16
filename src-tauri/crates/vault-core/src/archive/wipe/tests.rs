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

// After the commit and a crash, something is at a target: the user may have put it there from a
// backup while PathKeep was not running. The committed intent says the user confirmed deleting
// what is in its trash, not what stands in the folder now. The wipe deletes its trash and leaves
// that where it is; the folder then reads as an archive whose control is missing, which the next
// start reports instead of deleting it.
#[test]
fn what_appears_at_a_target_after_the_commit_is_left_where_it_is() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    crash_at(paths, &WipeOutsideState::default(), "wipe-committed");
    fs::create_dir_all(paths.app_root.join("derived")).unwrap();
    fs::write(paths.app_root.join("derived/agent.db"), b"put back since").unwrap();
    assert!(matches!(next_start(paths), Some(SettledWipe::Finished(_))));
    assert_eq!(fs::read(paths.app_root.join("derived/agent.db")).unwrap(), b"put back since");
    assert!(trashes(paths).is_empty(), "the trash is what the user confirmed deleting");
    assert!(!data_wipe_interrupted(paths));
    let refusal = crate::admission::inspect(&paths.app_root).expect_err("not a first run");
    assert_eq!(refusal.reason, crate::admission::Reason::ControlMissing);
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
        crash_at(paths, &WipeOutsideState::default(), cut);
        let mut other = fixture.config.clone();
        other.preferred_language = crate::LanguagePreference::ZhTw;
        save_config(&crate::ControlGate::acquire(paths), &other).unwrap();
        let intent = paths.app_root.join(".pk-wipe-v2.json");
        let record = fs::read(&intent).unwrap();
        let mut before = file_contents_under(&paths.app_root);

        assert_eq!(next_start(paths), Some(SettledWipe::Discarded), "{cut}");
        assert!(!data_wipe_interrupted(paths), "{cut}: the stale record is no longer acted on");
        assert_eq!(trashes(paths).len(), 1, "{cut}: the trash is kept");
        // The record is the only copy of the control the user confirmed: it is kept whole.
        before.remove(Path::new(".pk-wipe-v2.json"));
        let kept = kept_records(paths);
        assert_eq!(kept.len(), 1, "{cut}: {kept:?}");
        assert_eq!(fs::read(&kept[0]).unwrap(), record, "{cut}");
        before.insert(kept[0].strip_prefix(&paths.app_root).unwrap().to_path_buf(), record);
        assert_same_files(&file_contents_under(&paths.app_root), &before, cut);
    }
}

/// The wipe records kept in the quarantine.
fn kept_records(paths: &ProjectPaths) -> Vec<PathBuf> {
    walkdir::WalkDir::new(&paths.quarantine_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name() == ".pk-wipe-v2.json")
        .map(|entry| entry.into_path())
        .collect()
}

// A record set aside keeps the control whether or not its trash is here (a copy may not have
// carried it), and never writes over anything: the trash may be in use as a rescue folder by now,
// its own `config.json` edited since.
#[test]
fn a_record_set_aside_keeps_its_control_without_its_trash_and_writes_over_nothing() {
    for trash_here in [false, true] {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        crash_at(paths, &WipeOutsideState::default(), "wipe-target-moved");
        let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("one trash");
        if trash_here {
            fs::write(trash.join("config.json"), b"edited since").unwrap();
        } else {
            fs::remove_dir_all(&trash).unwrap();
        }
        let mut other = fixture.config.clone();
        other.preferred_language = crate::LanguagePreference::ZhTw;
        save_config(&crate::ControlGate::acquire(paths), &other).unwrap();
        let record = fs::read(paths.app_root.join(".pk-wipe-v2.json")).unwrap();

        assert_eq!(next_start(paths), Some(SettledWipe::Discarded), "trash here: {trash_here}");
        let kept = kept_records(paths);
        assert_eq!(kept.len(), 1, "trash here: {trash_here}");
        assert_eq!(fs::read(&kept[0]).unwrap(), record);
        if trash_here {
            assert_eq!(fs::read(trash.join("config.json")).unwrap(), b"edited since");
        }
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

// A target that cannot be looked at (a disk error, a permission) is not one that is not there.
// Taken for absent, it would be left out of the record, stay in place while the rest moved, and be
// moved out as something the wipe did not record when the wipe is undone: the archive would come
// back without it, and nothing would say where it went.
#[cfg(unix)]
#[test]
fn a_target_that_cannot_be_looked_at_is_not_taken_for_one_that_is_not_there() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let folder = paths.app_root.join("archive");
    let mode = fs::metadata(&folder).unwrap().permissions().mode();
    // Readable, so its entries are listed, but not searchable, so none of them can be looked at.
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o600)).unwrap();
    let listed = wipe_targets(paths);
    fs::set_permissions(&folder, fs::Permissions::from_mode(mode)).unwrap();
    assert!(listed.is_err(), "{listed:?}");
}

// A store's main file and its companions (`-wal`, `-shm`) are one database: SQLite reads a main
// with whatever log lies beside it. The wipe moves the main first and the undo puts it back last,
// so at every cut a main in its place has its own companions beside it, never a mix of the
// archive's and something put there since.
#[test]
fn at_every_cut_a_store_in_place_has_its_own_companions() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    for suffix in ["-wal", "-shm"] {
        fs::write(format!("{}{suffix}", archive_file(paths).display()), suffix.as_bytes()).unwrap();
    }
    let before = file_contents_under(&paths.app_root);
    let family = |paths: &ProjectPaths| -> Vec<bool> {
        ["", "-wal", "-shm"]
            .iter()
            .map(|suffix| Path::new(&format!("{}{suffix}", archive_file(paths).display())).exists())
            .collect()
    };
    let whole_or_gone = |paths: &ProjectPaths, when: &str| {
        let here = family(paths);
        assert!(!here[0] || here.iter().all(|in_place| *in_place), "{when}: {here:?}");
    };
    let moves = intent_targets_after_moving(paths);
    for hit in 1..=moves {
        let crashed = catch_unwind(AssertUnwindSafe(|| {
            let _cut = FaultGuard::panic_at_hit("wipe-target-moved", hit);
            let _ = wipe_all_data(
                paths,
                |_| Ok(WipeOutsideState::default()),
                no_quiesce,
                no_secrets,
                no_schedule,
            );
        }));
        assert!(crashed.is_err());
        whole_or_gone(paths, &format!("move {hit}"));
        assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
        assert_same_files(&file_contents_under(&paths.app_root), &before, "after the undo");
    }
    for hit in 1..=moves {
        crash_at(paths, &WipeOutsideState::default(), "wipe-moved");
        let crashed = catch_unwind(AssertUnwindSafe(|| {
            let _cut = FaultGuard::panic_at_hit("wipe-entry-put-back", hit);
            let _ = next_start(paths);
        }));
        assert!(crashed.is_err(), "put-back {hit}");
        whole_or_gone(paths, &format!("put-back {hit}"));
        assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
        assert_same_files(&file_contents_under(&paths.app_root), &before, "after the undo");
    }
}

/// How many targets a wipe of `paths` moves, from the record of one cut after the moves.
fn intent_targets_after_moving(paths: &ProjectPaths) -> u32 {
    crash_at(paths, &WipeOutsideState::default(), "wipe-moved");
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.app_root.join(".pk-wipe-v2.json")).unwrap())
            .unwrap();
    let count = record["targets"].as_array().unwrap().len() as u32;
    assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
    count
}

// A main in its place while one of its companions is still in the trash is a state no cut of a
// wipe or of its undo leaves: something was put there since. Putting the companion back would lay
// the archive's log beside a store it may not belong to. The undo stops before anything moves.
#[test]
fn a_store_in_place_whose_companion_is_in_the_trash_stops_the_undo() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    fs::write(format!("{}-wal", archive_file(paths).display()), b"log").unwrap();
    crash_at(paths, &WipeOutsideState::default(), "wipe-moved");
    let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("one trash");
    fs::rename(trash.join("archive/history-vault.db"), archive_file(paths)).unwrap();
    let before = file_contents_under(&paths.app_root);

    let error = finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
        .expect_err("refused");
    assert!(format!("{error:#}").contains("history-vault.db-wal"), "{error:#}");
    assert!(data_wipe_interrupted(paths));
    assert_same_files(&file_contents_under(&paths.app_root), &before, "");
}

// An empty folder has its own place on disk too, and on exFAT a cut move can leave it under two
// names. A name the undo moved out of an entry's way is kept with the trash, files or not.
#[test]
fn a_folder_moved_out_of_the_way_keeps_the_trash_even_when_it_is_empty() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    fs::create_dir(paths.app_root.join("archive/empty")).unwrap();
    crash_at(paths, &WipeOutsideState::default(), "wipe-moved");
    fs::create_dir(paths.app_root.join("archive/empty")).unwrap();

    assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
    let [trash] = <[PathBuf; 1]>::try_from(trashes(paths)).expect("the trash is kept");
    assert!(trash.join(DISPLACED).join("0/archive/empty").is_dir());
    assert!(paths.app_root.join("archive/empty").is_dir());
}

// On a file system without a journal a power cut during the rename over `config.json` can leave
// none (the old name gone, the new one not yet there). The intent holds both controls, so a
// folder with no control and this build's intent is a wipe cut short, not an archive that lost
// its control: undone, the control the user had comes back; finished, the wipe ends as it would.
#[test]
fn a_wipe_whose_control_a_power_cut_took_is_still_settled() {
    for cut in ["wipe-control-emptied", "wipe-target-moved", "wipe-committed"] {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        let confirmed = fs::read(&paths.config_path).unwrap();
        let before = file_contents_under(&paths.app_root);
        let outside = WipeOutsideState::from_config(&fixture.config, None);
        crash_at(paths, &outside, cut);
        fs::remove_file(&paths.config_path).unwrap();
        let cleared = Cell::new(false);

        let settled = finish_interrupted_data_wipe(
            paths,
            no_quiesce,
            |_| {
                cleared.set(true);
                Ok(())
            },
            no_schedule,
        )
        .unwrap_or_else(|error| panic!("{cut}: {error:#}"));
        if cut == "wipe-committed" {
            assert!(matches!(settled, Some(SettledWipe::Finished(_))), "{cut}");
            assert!(cleared.get(), "{cut}: the wipe the user confirmed clears its keys");
            assert!(!archive_file(paths).exists());
            assert!(matches!(
                crate::admission::inspect(&paths.app_root),
                Ok(crate::admission::RootState::Uninitialized)
            ));
        } else {
            assert_eq!(settled, Some(SettledWipe::RolledBack), "{cut}");
            assert_eq!(fs::read(&paths.config_path).unwrap(), confirmed, "{cut}");
            assert_same_files(&file_contents_under(&paths.app_root), &before, cut);
        }
    }
}

// Without a control, the trash is what binds the record to the folder, and finishing the wipe
// deletes it first. A second cut there, or a keychain that fails after it, must not leave a folder
// every later start refuses as one that lost its control: the emptied control the record holds
// goes back before anything is deleted, and the next start finishes the wipe.
#[test]
fn a_wipe_whose_control_a_power_cut_took_finishes_after_a_second_interruption() {
    for second in ["wipe-control-rewritten", "wipe-files-removed", "a keychain error"] {
        let fixture = backed_up_archive();
        let paths = &fixture.paths;
        crash_at(paths, &WipeOutsideState::from_config(&fixture.config, None), "wipe-committed");
        fs::remove_file(&paths.config_path).unwrap();
        if second == "a keychain error" {
            finish_interrupted_data_wipe(
                paths,
                no_quiesce,
                |_| anyhow::bail!("the keychain is locked"),
                no_schedule,
            )
            .expect_err("the keychain failed");
            assert!(trashes(paths).is_empty(), "{second}: the trash went first");
        } else {
            let crashed = catch_unwind(AssertUnwindSafe(|| {
                let _cut = FaultGuard::panic_at(second);
                let _ = finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule);
            }));
            assert!(crashed.is_err(), "{second}: the cut was not reached");
        }
        assert!(paths.config_path.exists(), "{second}: a control stands in the folder meanwhile");
        let cleared = Cell::new(false);
        let settled = finish_interrupted_data_wipe(
            paths,
            no_quiesce,
            |_| {
                cleared.set(true);
                Ok(())
            },
            no_schedule,
        )
        .unwrap_or_else(|error| panic!("{second}: {error:#}"));
        assert!(matches!(settled, Some(SettledWipe::Finished(_))), "{second}");
        assert!(cleared.get(), "{second}: the keys are cleared");
        assert!(!data_wipe_interrupted(paths), "{second}");
        assert!(
            matches!(
                crate::admission::inspect(&paths.app_root),
                Ok(crate::admission::RootState::Uninitialized)
            ),
            "{second}"
        );
    }
}

// Undoing a wipe whose control a power cut took puts the archive back one entry at a time. Cut
// midway, the folder holds the archive's files again, and without a control PathKeep 0.5 would take
// them for a first run: the stamped emptied control goes back before anything moves, and the next
// start completes the undo with the control the user had.
#[test]
fn an_undo_without_a_control_puts_the_stamped_one_back_before_anything_moves() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    let confirmed = fs::read(&paths.config_path).unwrap();
    let before = file_contents_under(&paths.app_root);
    crash_at(paths, &WipeOutsideState::from_config(&fixture.config, None), "wipe-moved");
    fs::remove_file(&paths.config_path).unwrap();
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        let _cut = FaultGuard::panic_at("wipe-entry-put-back");
        let _ = finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule);
    }));
    assert!(crashed.is_err(), "the cut was not reached");
    let control = fs::read(&paths.config_path).expect("a control while the archive comes back");
    let parsed = crate::admission::parse_control(&paths.app_root, &control).unwrap();
    assert!(parsed.compatibility.is_some(), "the control keeps PathKeep 0.5 out");
    assert!(
        !crate::load_config(paths).unwrap().initialized,
        "the empty control, not the archive's, while the archive is only partly back"
    );
    assert_eq!(next_start(paths), Some(SettledWipe::RolledBack));
    assert_eq!(fs::read(&paths.config_path).unwrap(), confirmed);
    assert_same_files(&file_contents_under(&paths.app_root), &before, "undone");
}

// The record names every target, and a name it cannot hold the same way on every system (here a
// backslash, a separator on Windows) would be read back as another path after a crash. The wipe
// stops before it records or moves anything, and everything is as it was.
#[cfg(unix)]
#[test]
fn a_name_the_record_cannot_hold_stops_the_wipe_before_anything_moves() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    fs::write(paths.app_root.join("archive/notes\\2026.txt"), b"a file of the user's").unwrap();
    let before = file_contents_under(&paths.app_root);
    let secrets = WipeOutsideState::from_config(&fixture.config, None);
    let error = wipe_all_data(
        paths,
        |_| Ok(secrets.clone()),
        no_quiesce,
        |_| panic!("nothing is cleared while nothing is deleted"),
        no_schedule,
    )
    .expect_err("refused");
    assert!(format!("{error:#}").contains("notes\\2026.txt"), "{error:#}");
    assert!(!data_wipe_interrupted(paths));
    assert!(trashes(paths).is_empty());
    assert_same_files(&file_contents_under(&paths.app_root), &before, "refused");
}

// An intent naming its targets with Windows's separator is not read as one file with a backslash
// in its name: it does not decode, and nothing moves.
#[test]
fn an_intent_naming_a_path_another_system_reads_otherwise_moves_nothing() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    crash_at(paths, &WipeOutsideState::from_config(&fixture.config, None), "wipe-targets-recorded");
    let intent = paths.app_root.join(WIPE_INTENT_FILE);
    let text = fs::read_to_string(&intent).unwrap();
    assert!(text.contains("archive/"), "{text}");
    fs::write(&intent, text.replace("archive/", "archive\\\\")).unwrap();
    let before = file_contents_under(&paths.app_root);
    let error = finish_interrupted_data_wipe(paths, no_quiesce, no_secrets, no_schedule)
        .expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-invalid"), "{error:#}");
    assert_same_files(&file_contents_under(&paths.app_root), &before, "refused");
}

// The wipe records and moves a store's main file before its companions, whatever order the folder
// lists them in (the order they were made in on exFAT, a hash on APFS and ext4): the undo, going
// the other way, relies on it. Many stores, each written companions first, so that no listing order
// passes by chance.
#[test]
fn a_store_is_recorded_before_its_companions_whatever_the_listing_order() {
    let fixture = backed_up_archive();
    let paths = &fixture.paths;
    for store in 0..24 {
        for suffix in ["-wal", "-shm", ""] {
            fs::write(paths.app_root.join(format!("archive/store-{store}.db{suffix}")), suffix)
                .unwrap();
        }
    }
    crash_at(paths, &WipeOutsideState::from_config(&fixture.config, None), "wipe-targets-recorded");
    let intent: serde_json::Value =
        serde_json::from_slice(&fs::read(paths.app_root.join(WIPE_INTENT_FILE)).unwrap()).unwrap();
    let targets: Vec<&str> =
        intent["targets"].as_array().unwrap().iter().map(|t| t.as_str().unwrap()).collect();
    let at = |name: &str| {
        targets.iter().position(|target| *target == name).unwrap_or_else(|| panic!("{name}"))
    };
    for store in 0..24 {
        let main = at(&format!("archive/store-{store}.db"));
        for suffix in ["-wal", "-shm"] {
            assert!(main < at(&format!("archive/store-{store}.db{suffix}")), "{targets:?}");
        }
    }
}

// Without a control, the trash the intent names is what says the intent is this folder's: it is
// made before the intent, under a random name, and stays until the confirmed control is back or
// everything in it is deleted. An intent carried into another data folder that lost its control,
// without that trash, writes no control there, clears no keys and moves nothing: the folder is
// refused as one whose control is missing, and every file, the intent included, stays as it is.
#[test]
fn an_intent_carried_into_a_folder_without_a_control_writes_and_moves_nothing() {
    for cut in ["wipe-control-emptied", "wipe-target-moved", "wipe-committed"] {
        // One fixture at a time: each holds the test lock until it is dropped.
        let intent = {
            let first = backed_up_archive();
            crash_at(&first.paths, &WipeOutsideState::from_config(&first.config, None), cut);
            fs::read(first.paths.app_root.join(WIPE_INTENT_FILE)).unwrap()
        };
        let second = backed_up_archive();
        let paths = &second.paths;
        fs::remove_file(&paths.config_path).unwrap();
        fs::write(paths.app_root.join("archive/only-here.txt"), b"never seen by the other wipe")
            .unwrap();
        fs::write(paths.app_root.join(WIPE_INTENT_FILE), &intent).unwrap();
        let before = file_contents_under(&paths.app_root);
        assert!(
            !matches!(
                crate::admission::inspect(&paths.app_root),
                Ok(crate::admission::RootState::Interrupted(_))
            ),
            "{cut}"
        );

        let error = finish_interrupted_data_wipe(
            paths,
            no_quiesce,
            |_| panic!("{cut}: the keys of another archive's wipe are not cleared"),
            no_schedule,
        )
        .expect_err("refused");
        assert!(format!("{error:#}").contains("archive-control-missing"), "{cut}: {error:#}");
        assert!(!paths.config_path.exists(), "{cut}: no control was written");
        assert_same_files(&file_contents_under(&paths.app_root), &before, cut);
    }
}
