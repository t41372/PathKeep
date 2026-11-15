//! Responsibilities: exercise each multi-file install journal boundary.
//! Not responsible for: database engines or password derivation.
//! Failure modes: lost original, mixed committed files, stale WAL, non-idempotent recovery, a
//! journal acting on another archive's files, a staged file taken for the original it replaced, a
//! rollback cut short that cannot be finished, a failed publish rolled back against its record.
use super::install::*;
use crate::project_paths_with_root;

#[test]
fn every_interrupted_swap_rolls_back() {
    for (interrupted_after, keep) in (0..=4)
        .flat_map(|steps| [(steps, Keep::Backup), (steps, Keep::SafetyCopy(Before::Restore))])
    {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        std::fs::create_dir_all(stage.join("archive")).unwrap();
        std::fs::create_dir_all(root.path().join("archive")).unwrap();
        for name in ["archive/history-vault.db", "archive/source-evidence.db"] {
            std::fs::write(root.path().join(name), b"old").unwrap();
            std::fs::write(stage.join(name), b"new").unwrap();
        }
        std::fs::write(root.path().join("archive/.pk-owner.lock"), b"owner").unwrap();
        let files = vec!["archive/history-vault.db".into(), "archive/source-evidence.db".into()];
        let mut journal = prepare(&paths, &stage, files, keep).unwrap();
        for step in 0..interrupted_after {
            install_step(&paths, &mut journal, step).unwrap();
        }
        recover(&paths).unwrap();
        recover(&paths).unwrap();
        for name in ["archive/history-vault.db", "archive/source-evidence.db"] {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"old");
        }
        assert_eq!(std::fs::read(root.path().join("archive/.pk-owner.lock")).unwrap(), b"owner");
        // Nothing was replaced, so nothing is kept: no copy, no backup folder.
        assert!(!root.path().join("raw-snapshots").exists(), "{keep:?}");
        let backups = std::fs::read_dir(root.path())
            .unwrap()
            .filter(|e| {
                e.as_ref().unwrap().file_name().to_string_lossy().starts_with("maintenance.bak-")
            })
            .count();
        assert_eq!(backups, 0, "{keep:?}");
    }
}

#[test]
fn a_published_generation_finishes_cleanup_without_restoring_old_files() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    std::fs::create_dir_all(root.path().join("archive")).unwrap();
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(root.path().join("config.json"), b"old").unwrap();
    std::fs::write(stage.join("config.json"), b"new").unwrap();
    let mut journal = prepare(&paths, &stage, vec!["config.json".into()], Keep::Backup).unwrap();
    install_step(&paths, &mut journal, 0).unwrap();
    install_step(&paths, &mut journal, 1).unwrap();
    publish(&paths, &mut journal).unwrap();
    recover(&paths).unwrap();
    recover(&paths).unwrap();
    assert_eq!(std::fs::read(root.path().join("config.json")).unwrap(), b"new");
}

#[test]
fn an_old_format_import_journal_next_to_a_native_archive_is_set_aside_not_rolled_back() {
    let (root, vault, _) = super::tests::encrypted();
    vault.close().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let config_before = std::fs::read(&paths.config_path).unwrap();
    // The shape an interrupted old-format import left: no "stage", an old config to restore.
    let journal = serde_json::json!({
        "version": 1,
        "previous_config": {"initialized": false},
        "files": [{"target": "archive/history-vault.sqlite", "backup": "archive/history-vault.sqlite.bak"}],
    });
    let marker = root.path().join("archive/.pk-import-journal.json");
    std::fs::write(&marker, serde_json::to_vec(&journal).unwrap()).unwrap();

    assert!(!recover(&paths).unwrap());
    assert!(!marker.exists());
    assert!(root.path().join("archive/.pk-import-journal.legacy-stale.json").exists());
    assert_eq!(std::fs::read(&paths.config_path).unwrap(), config_before);
    let config = crate::load_config(&paths).unwrap();
    assert!(crate::Vault::open_password(paths, config, "old password", None).is_ok());
}

fn old_and_new(root: &std::path::Path, stage: &std::path::Path) -> Vec<std::path::PathBuf> {
    let files = ["archive/history-vault.db", "archive/keys.envelope", "config.json"];
    for name in files {
        let (live, staged) = (root.join(name), stage.join(name));
        std::fs::create_dir_all(live.parent().unwrap()).unwrap();
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(live, b"old").unwrap();
        std::fs::write(staged, b"new").unwrap();
    }
    files.iter().map(Into::into).collect()
}

// A release that cannot read the new archive stops at its config; one that finds none takes the
// folder for a first run. So at no step of an install, and no step of its recovery, may the
// control be missing, and it is always one whole generation.
#[test]
fn the_control_is_whole_at_every_step_of_an_install_and_its_recovery() {
    for (steps, commit) in (0..=6).map(|steps| (steps, false)).chain([(6, true)]) {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = old_and_new(root.path(), &stage);
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        let control = root.path().join("config.json");
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
            let bytes = std::fs::read(&control).expect("the control went missing");
            assert!(bytes == b"old" || bytes == b"new", "{steps}/{step}");
        }
        if commit {
            publish(&paths, &mut journal).unwrap();
        }
        recover(&paths).unwrap();
        let expected: &[u8] = if commit { b"new" } else { b"old" };
        for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), expected, "{steps} {name}");
        }
        assert!(!pending(&paths), "{steps}");
    }
}

// The journal of PathKeep 0.5: the old name, no identity, and a control that was moved, not
// copied, into the backup. A crash during one of its installs is still undone.
#[test]
fn an_install_pathkeep_0_5_left_is_still_rolled_back() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-x");
    let backup = root.path().join("maintenance.bak-x");
    std::fs::create_dir_all(root.path().join("archive")).unwrap();
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::create_dir_all(&backup).unwrap();
    std::fs::write(backup.join("config.json"), b"old").unwrap();
    std::fs::write(stage.join("config.json"), b"new").unwrap();
    let journal = serde_json::json!({
        "stage": stage, "backup": backup, "keep_previous": false, "committed": false,
        "entries": [{"relative": "config.json", "previous": true, "staged": true}],
    });
    std::fs::write(
        root.path().join("archive/.pk-import-journal.json"),
        serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    assert!(pending(&paths));
    assert!(recover(&paths).unwrap());
    assert_eq!(std::fs::read(root.path().join("config.json")).unwrap(), b"old");
    assert!(!root.path().join("archive/.pk-import-journal.json").exists());
    assert!(!pending(&paths));
}

// Nothing is replayed from a record whose identity or version this build does not know, and
// nothing about the folder changes.
#[test]
fn a_journal_this_build_did_not_write_is_not_replayed() {
    for journal in [
        "not json".to_owned(),
        r#"{"stage":"s","backup":"b","entries":[],"keep_previous":false,"committed":false}"#
            .to_owned(),
        r#"{"identity":"pathkeep-install","version":2,"stage":"s","backup":"b","entries":[],"keep_previous":false,"committed":false}"#
            .to_owned(),
    ] {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        old_and_new(root.path(), &stage);
        std::fs::write(root.path().join("archive/.pk-install-v2.json"), &journal).unwrap();
        assert!(pending(&paths), "{journal}");
        let error = recover(&paths).expect_err("refused");
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{journal}");
        assert_eq!(std::fs::read(root.path().join("config.json")).unwrap(), b"old");
        assert!(root.path().join("archive/.pk-install-v2.json").exists());
    }
}

// An install brings its own control or leaves the live one alone; it never removes it.
#[test]
fn an_install_that_would_remove_the_control_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    std::fs::create_dir_all(root.path().join("archive")).unwrap();
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(root.path().join("config.json"), b"old").unwrap();
    assert!(prepare(&paths, &stage, vec!["config.json".into()], Keep::Nothing).is_err());
    assert!(!pending(&paths));
    assert_eq!(std::fs::read(root.path().join("config.json")).unwrap(), b"old");
}

fn journal_file(root: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(root.join("archive/.pk-install-v2.json")).unwrap())
        .unwrap()
}

// The journal names its stage and backup relative to the folder, so it means the same in a
// folder that was moved, and nothing in a folder it was carried into.
#[test]
fn the_journal_records_its_folders_relative_to_the_archive() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = old_and_new(root.path(), &stage);
    prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    let journal = journal_file(root.path());
    assert_eq!(journal["stage"], "staging/maintenance-test");
    let backup = journal["backup"].as_str().unwrap();
    assert!(backup.starts_with("maintenance.bak-") && !backup.contains('/'), "{backup}");
    assert!(root.path().join(backup).is_dir());
}

// A folder moved while an install was cut short is recovered where it now is.
#[test]
fn a_folder_moved_during_an_install_is_recovered_where_it_is() {
    let parent = tempfile::tempdir().unwrap();
    let (before, after) = (parent.path().join("before"), parent.path().join("after"));
    std::fs::create_dir(&before).unwrap();
    let stage = before.join("staging/maintenance-test");
    let files = old_and_new(&before, &stage);
    let mut journal =
        prepare(&project_paths_with_root(&before), &stage, files, Keep::Nothing).unwrap();
    for step in 0..4 {
        install_step(&project_paths_with_root(&before), &mut journal, step).unwrap();
    }
    std::fs::rename(&before, &after).unwrap();
    assert!(recover(&project_paths_with_root(&after)).unwrap());
    for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
        assert_eq!(std::fs::read(after.join(name)).unwrap(), b"old", "{name}");
    }
}

// The same for a journal PathKeep 0.5 left: its paths are absolute, of where the folder was.
#[test]
fn a_journal_of_0_5_in_a_moved_folder_is_recovered_where_it_is() {
    let parent = tempfile::tempdir().unwrap();
    let (before, after) = (parent.path().join("before"), parent.path().join("after"));
    let stage = before.join("staging/maintenance-x");
    let backup = before.join("maintenance.bak-x");
    std::fs::create_dir_all(before.join("archive")).unwrap();
    std::fs::create_dir_all(stage.join("archive")).unwrap();
    std::fs::create_dir_all(backup.join("archive")).unwrap();
    std::fs::write(backup.join("archive/history-vault.db"), b"old").unwrap();
    std::fs::write(before.join("archive/history-vault.db"), b"new").unwrap();
    let journal = serde_json::json!({
        "stage": stage, "backup": backup, "keep_previous": false, "committed": false,
        "entries": [{"relative": "archive/history-vault.db", "previous": true, "staged": true}],
    });
    std::fs::write(
        before.join("archive/.pk-import-journal.json"),
        serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    std::fs::rename(&before, &after).unwrap();
    assert!(recover(&project_paths_with_root(&after)).unwrap());
    assert_eq!(std::fs::read(after.join("archive/history-vault.db")).unwrap(), b"old");
}

// A journal carried into another archive's folder (a copy, a merge, a backup restore) names a
// backup folder that is not there. It changes nothing: not the files it would take for the new
// ones it installed, not the ones it would take for the old ones it would put back.
#[test]
fn a_journal_carried_into_another_folder_changes_nothing() {
    let parent = tempfile::tempdir().unwrap();
    let (first, second) = (parent.path().join("first"), parent.path().join("second"));
    std::fs::create_dir(&first).unwrap();
    let stage = first.join("staging/maintenance-test");
    let files = old_and_new(&first, &stage);
    let mut journal =
        prepare(&project_paths_with_root(&first), &stage, files, Keep::Nothing).unwrap();
    for step in 0..3 {
        install_step(&project_paths_with_root(&first), &mut journal, step).unwrap();
    }
    std::fs::create_dir_all(second.join("archive")).unwrap();
    for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
        std::fs::write(second.join(name), b"the other archive").unwrap();
    }
    std::fs::copy(
        first.join("archive/.pk-install-v2.json"),
        second.join("archive/.pk-install-v2.json"),
    )
    .unwrap();
    let error = recover(&project_paths_with_root(&second)).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-invalid"), "{error:#}");
    for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
        assert_eq!(std::fs::read(second.join(name)).unwrap(), b"the other archive", "{name}");
    }
    // The folder it came from is still recovered on its own.
    assert!(recover(&project_paths_with_root(&first)).unwrap());
    assert_eq!(std::fs::read(first.join("archive/history-vault.db")).unwrap(), b"old");
}

// A file that was there before the install and is now neither in place nor in the backup is not
// reported as rolled back. The journal stays, so nothing claims the archive is whole.
#[test]
fn an_original_in_neither_place_keeps_the_journal() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = old_and_new(root.path(), &stage);
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    install_step(&paths, &mut journal, 0).unwrap();
    let backup = journal_file(root.path())["backup"].as_str().unwrap().to_owned();
    std::fs::remove_file(root.path().join(&backup).join("archive/history-vault.db")).unwrap();
    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("neither in place nor in the backup"), "{error:#}");
    assert!(root.path().join("archive/.pk-install-v2.json").exists());
}

// The journal names the safety copy the replaced files become. Recovery moves them only where this
// folder's installs put them: beside the other copies, under the journal's own backup name. A
// journal naming any other place, by mistake or carried from elsewhere, moves nothing.
#[test]
fn a_journal_naming_a_copy_anywhere_else_moves_nothing() {
    for elsewhere in [
        "archive",
        "../outside",
        "raw-snapshots/maintenance/rekey-x",
        "raw-snapshots/maintenance/before-restore-another",
        "raw-snapshots/before-restore-x",
    ] {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = old_and_new(root.path(), &stage);
        let mut journal =
            prepare(&paths, &stage, files, Keep::SafetyCopy(Before::Restore)).unwrap();
        for step in 0..6 {
            install_step(&paths, &mut journal, step).unwrap();
        }
        publish(&paths, &mut journal).unwrap();
        let mut record = journal_file(root.path());
        let backup = record["backup"].as_str().unwrap().to_owned();
        assert!(record["kept_as"].as_str().unwrap().starts_with("raw-snapshots/maintenance/"));
        record["kept_as"] = elsewhere.into();
        std::fs::write(
            root.path().join("archive/.pk-install-v2.json"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();

        let error = recover(&paths).expect_err("refused");
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{elsewhere}: {error:#}");
        assert!(
            root.path().join(&backup).join("archive/history-vault.db").is_file(),
            "{elsewhere}"
        );
        assert!(!root.path().join("raw-snapshots").exists(), "{elsewhere}");
        assert!(pending(&paths), "{elsewhere}");
    }
}

/// Copies `from` into `to` file by file: new files with the same bytes, the way a copy of a
/// folder, a merge of two folders or a restore from a backup makes them.
fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let target = to.join(entry.path().strip_prefix(from).unwrap());
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// An install into a folder without an archive: every file is new, and nothing goes into the
/// backup.
fn new_only(root: &std::path::Path, stage: &std::path::Path) -> Vec<std::path::PathBuf> {
    let files = ["archive/history-vault.db", "archive/keys.envelope", "config.json"];
    for name in files {
        let staged = stage.join(name);
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(staged, b"new").unwrap();
    }
    std::fs::create_dir_all(root.join("archive")).unwrap();
    files.iter().map(Into::into).collect()
}

// An install cut short, then its journal carried into another archive's folder together with the
// backup folder it names (the two folders merged, or the journal's files restored over the other
// archive). The backup is there, but nothing in it and nothing in place is a file the install
// moved, so nothing changes: not the other archive's files the install would take for the ones it
// put in place, and not a place where the other archive has no file and the backup has one (an
// unencrypted archive has no key file; the encrypted one's must not land in it). Where the other
// archive has nothing but its control at the journal's places, its control alone tells the folders
// apart, and is not written over.
#[test]
fn a_journal_carried_in_with_its_backup_changes_nothing() {
    type Shape = fn(&std::path::Path, &std::path::Path) -> Vec<std::path::PathBuf>;
    let others: [&[&str]; 2] = [&["archive/history-vault.db", "config.json"], &["config.json"]];
    for ((shape, steps), other) in [old_and_new as Shape, new_only as Shape]
        .into_iter()
        .flat_map(|s| (0..=6).map(move |n| (s, n)))
        .flat_map(|run| others.map(|other| (run, other)))
    {
        let parent = tempfile::tempdir().unwrap();
        let (first, second) = (parent.path().join("first"), parent.path().join("second"));
        std::fs::create_dir(&first).unwrap();
        let stage = first.join("staging/maintenance-test");
        let files = shape(&first, &stage);
        let mut journal =
            prepare(&project_paths_with_root(&first), &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&project_paths_with_root(&first), &mut journal, step).unwrap();
        }
        std::fs::create_dir_all(second.join("archive")).unwrap();
        for name in other {
            std::fs::write(second.join(name), b"the other archive").unwrap();
        }
        let backup = journal_file(&first)["backup"].as_str().unwrap().to_owned();
        copy_tree(&first.join(&backup), &second.join(&backup));
        copy_tree(&first.join("staging"), &second.join("staging"));
        std::fs::copy(
            first.join("archive/.pk-install-v2.json"),
            second.join("archive/.pk-install-v2.json"),
        )
        .unwrap();
        let before = super::creation::tests::files(&second);

        let error = recover(&project_paths_with_root(&second)).expect_err("refused");
        assert!(format!("{error:#}").contains("archive-control-invalid"), "{steps}: {error:#}");
        assert_eq!(
            super::creation::tests::files(&second),
            before,
            "{steps} {other:?}: something changed"
        );
        // The folder it came from is still recovered on its own.
        recover(&project_paths_with_root(&first)).unwrap();
        assert!(!pending(&project_paths_with_root(&first)), "{steps}");
    }
}

// The backup folder no longer binds a journal to its folder, so a real one whose backup folder was
// lost while still empty (a copy tool that skips empty folders) is still rolled back.
#[test]
fn a_journal_whose_empty_backup_folder_was_lost_still_rolls_back() {
    type Shape = fn(&std::path::Path, &std::path::Path) -> Vec<std::path::PathBuf>;
    for (shape, steps) in [(old_and_new as Shape, 0)]
        .into_iter()
        .chain((0..=6).map(|steps| (new_only as Shape, steps)))
    {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = shape(root.path(), &stage);
        let before = super::creation::tests::files(root.path());
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
        }
        let backup = journal_file(root.path())["backup"].as_str().unwrap().to_owned();
        std::fs::remove_dir(root.path().join(backup)).expect("still empty");

        recover(&paths).unwrap_or_else(|error| panic!("{steps}: {error:#}"));
        assert!(!pending(&paths), "{steps}");
        let mut after = super::creation::tests::files(root.path());
        after.retain(|path, _| !path.starts_with(&stage));
        let mut before = before;
        before.retain(|path, _| !path.starts_with(&stage));
        assert_eq!(after, before, "{steps}");
    }
}

// A rollback cut short is finished by the next start, whatever it had already put back: cut after
// it took a staged file or folder out of its place, before the original came back, or after a
// whole entry. The control it wrote back is a new file with the old bytes: that is still this
// install's.
#[test]
fn a_rollback_cut_short_is_finished_by_the_next_start() {
    for (cut, steps) in ["native-install-discarded", "native-install-undone"]
        .into_iter()
        .flat_map(|cut| (1..=8).map(move |steps| (cut, steps)))
    {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = with_folder(root.path(), &stage);
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
        }
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _fault = crate::fault_inject::FaultGuard::panic_at(cut);
            recover(&paths)
        }));
        let reached = crashed.is_err();
        if cut == "native-install-discarded" && steps == 8 {
            // The staged folder, undone first, was taken out whole: all of it is in the stage and
            // nothing is left in its place.
            assert!(!root.path().join("derived/search-index").exists());
            assert_eq!(
                std::fs::read(stage.join(".discarded/0/derived/search-index/c")).unwrap(),
                b"c"
            );
        }
        recover(&paths).unwrap_or_else(|error| panic!("{cut} {steps}: {error:#}"));
        for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
            assert_eq!(
                std::fs::read(root.path().join(name)).unwrap(),
                b"old",
                "{cut} {steps} {name}"
            );
        }
        let mut names: Vec<_> = std::fs::read_dir(root.path().join("derived/search-index"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["a", "b"], "{cut} {steps}");
        assert!(!pending(&paths), "{cut} {steps}");
        // Every staged file put in place is taken out by a rename, so each cut is reached.
        assert!(reached || (cut == "native-install-discarded" && steps == 1), "{cut} {steps}");
    }
}

// The journal of PathKeep 0.5 records no identities: it is still bound to its folder by the backup
// folder it names, and in a folder without it nothing is removed.
#[test]
fn a_journal_of_0_5_without_its_backup_folder_changes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    std::fs::create_dir_all(root.path().join("archive")).unwrap();
    std::fs::write(root.path().join("archive/history-vault.db"), b"this archive").unwrap();
    std::fs::write(root.path().join("config.json"), b"this archive").unwrap();
    let elsewhere = std::path::Path::new("/elsewhere");
    let journal = serde_json::json!({
        "stage": elsewhere.join("staging/maintenance-x"),
        "backup": elsewhere.join("maintenance.bak-x"),
        "keep_previous": false, "committed": false,
        "entries": [{"relative": "archive/history-vault.db", "previous": false, "staged": true}],
    });
    std::fs::write(
        root.path().join("archive/.pk-import-journal.json"),
        serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    let before = super::creation::tests::files(root.path());
    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-invalid"), "{error:#}");
    assert_eq!(super::creation::tests::files(root.path()), before);
}

// Every entry is checked before the first one is undone. A journal that finds one file that is not
// its own (an older database restored from a backup in its place) changes nothing, not even the
// files it could have put back.
#[test]
fn a_journal_that_finds_one_file_not_its_own_changes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = old_and_new(root.path(), &stage);
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    for step in 0..6 {
        install_step(&paths, &mut journal, step).unwrap();
    }
    // The database is the first entry: undone last. Another one restored from a backup in its
    // place, as long as the one installed and with its time: only what it holds tells it apart.
    let database = root.path().join("archive/history-vault.db");
    let installed = std::fs::metadata(&database).unwrap().modified().unwrap();
    std::fs::write(&database, b"bad").unwrap();
    std::fs::File::options().write(true).open(&database).unwrap().set_modified(installed).unwrap();
    let before = super::creation::tests::files(root.path());

    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-invalid"), "{error:#}");
    assert_eq!(super::creation::tests::files(root.path()), before, "something changed");
    assert!(pending(&paths));
}

/// Replaces every file under `root` with a copy made now: another file, the same bytes, other
/// times, as a copy tool that does not keep them makes.
fn copy_without_times(root: &std::path::Path) {
    let files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect();
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
    for file in files {
        let bytes = std::fs::read(&file).unwrap();
        std::fs::remove_file(&file).unwrap();
        std::fs::write(&file, bytes).unwrap();
        std::fs::File::options().write(true).open(&file).unwrap().set_modified(later).unwrap();
    }
}

// The whole folder copied in the middle of an install by a tool that does not keep the files'
// times, or that gives every file a new number (exFAT or FAT under Linux after a remount): the
// rollback still happens, because a journal knows its files by what they hold.
#[test]
fn a_rollback_after_the_folder_was_copied_without_its_times_still_happens() {
    for steps in 0..=6 {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = old_and_new(root.path(), &stage);
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
        }
        copy_without_times(root.path());

        recover(&paths).unwrap_or_else(|error| panic!("{steps}: {error:#}"));
        for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"old", "{steps} {name}");
        }
        assert!(!pending(&paths), "{steps}");
    }
}

// The staged files are as long as the originals and were written in the same tick of the clock
// (a file system that keeps times to the second, 2 s or 10 ms, and two closes in one tick). Only
// what they hold tells them apart; at every cut each original still comes back.
#[test]
fn a_staged_file_as_long_and_as_old_as_its_original_is_not_taken_for_it() {
    for steps in 0..=6 {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = old_and_new(root.path(), &stage);
        let moment = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        for name in &files {
            for file in [root.path().join(name), stage.join(name)] {
                std::fs::File::options()
                    .write(true)
                    .open(file)
                    .unwrap()
                    .set_modified(moment)
                    .unwrap();
            }
        }
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
        }
        recover(&paths).unwrap_or_else(|error| panic!("{steps}: {error:#}"));
        for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"old", "{steps} {name}");
        }
        assert!(!pending(&paths), "{steps}");
    }
}

// The record that says published is on disk, and the publish still reports an error (the flush
// after its rename failed). The install does not roll back in memory against that record: a
// rollback cut short there would be finished forward by the next start, over originals it had not
// put back. It stops, and the next start goes the way the record says: the whole new archive.
#[test]
fn a_publish_that_fails_after_its_record_landed_is_left_to_the_next_start() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = old_and_new(root.path(), &stage);
    let _flush = crate::fault_inject::FaultGuard::error_at("native-install-publish-saved");
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _cut = crate::fault_inject::FaultGuard::panic_at("native-install-undone");
        let gate = crate::ControlGate::acquire(&paths);
        install(&gate, &stage, files, Keep::Nothing)
    }));
    assert!(
        matches!(outcome, Ok(Err(_))),
        "the install reports the error without undoing anything"
    );
    recover(&paths).unwrap();
    for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
        assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"new", "{name}");
    }
    assert!(!pending(&paths));
}

// A current journal keeps a control in place: it is copied into the backup, replaced by one
// rename and written back atomically. On a file system without a journal (exFAT), a power cut
// during the rename over it can leave none, with its copy in the backup: that folder is rolled
// back, from the copy, and the start that finds it is not refused for a missing control. Before
// the copy exists nothing says the control was the journal's, and nothing changes.
#[test]
fn a_control_lost_after_its_copy_was_made_comes_back_from_the_copy() {
    for steps in 0..=6 {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = old_and_new(root.path(), &stage);
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
        }
        std::fs::remove_file(root.path().join("config.json")).unwrap();
        assert!(
            matches!(
                crate::admission::inspect(root.path()),
                Ok(crate::admission::RootState::Interrupted(
                    crate::admission::Interrupted::Install
                ))
            ),
            "{steps}"
        );
        if steps < 5 {
            let before = super::creation::tests::files(root.path());
            let error = recover(&paths).expect_err("refused");
            assert!(
                format!("{error:#}").contains("neither in place nor in the backup"),
                "{error:#}"
            );
            assert_eq!(super::creation::tests::files(root.path()), before, "{steps}");
            continue;
        }
        recover(&paths).unwrap_or_else(|error| panic!("{steps}: {error:#}"));
        for name in ["archive/history-vault.db", "archive/keys.envelope", "config.json"] {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"old", "{steps} {name}");
        }
        assert!(!pending(&paths), "{steps}");
    }
}

// An install into a folder without an archive put new files where there were none. A rollback takes
// one out of its place only if it holds what was staged: another file put there since stays, and
// nothing changes.
#[test]
fn a_file_that_was_not_there_before_goes_only_if_it_is_the_staged_one() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = new_only(root.path(), &stage);
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    for step in 0..6 {
        install_step(&paths, &mut journal, step).unwrap();
    }
    std::fs::write(root.path().join("archive/keys.envelope"), b"put").unwrap();
    let before = super::creation::tests::files(root.path());

    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-invalid"), "{error:#}");
    assert_eq!(super::creation::tests::files(root.path()), before, "something changed");
}

// A journal of PathKeep 0.5 beside a control with a stamp was carried in: that release does not
// open such an archive, and this build replays its journals before it stamps one. It is set aside
// and nothing it names is touched, though the backup folder it names is here.
#[test]
fn a_0_5_journal_beside_a_stamp_is_set_aside() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-x");
    let backup = root.path().join("maintenance.bak-x");
    std::fs::create_dir_all(root.path().join("archive")).unwrap();
    std::fs::create_dir_all(stage.join("archive")).unwrap();
    std::fs::create_dir_all(backup.join("archive")).unwrap();
    std::fs::write(root.path().join("archive/history-vault.db"), b"this archive").unwrap();
    std::fs::write(backup.join("archive/history-vault.db"), b"another archive").unwrap();
    let stamped = crate::config::encode_config(
        &crate::AppConfig::default(),
        Some(&crate::admission::written_compatibility()),
    )
    .unwrap();
    std::fs::write(root.path().join("config.json"), stamped).unwrap();
    let journal = serde_json::json!({
        "stage": stage, "backup": backup, "keep_previous": false, "committed": false,
        "entries": [{"relative": "archive/history-vault.db", "previous": true, "staged": true}],
    });
    let record = root.path().join("archive/.pk-import-journal.json");
    std::fs::write(&record, serde_json::to_vec(&journal).unwrap()).unwrap();
    let mut before = super::creation::tests::files(root.path());
    let bytes = before.remove(&record).unwrap();

    assert!(!recover(&paths).unwrap());
    assert!(!pending(&paths));
    let mut after = super::creation::tests::files(root.path());
    let aside = after.remove(&root.path().join("archive/.pk-import-journal.legacy-stale.json"));
    assert_eq!(aside, Some(bytes), "set aside as it was");
    assert_eq!(after, before, "nothing else changed");
}

/// A folder entry, like the search index: two files in the live one, one in the staged one.
fn with_folder(root: &std::path::Path, stage: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = old_and_new(root, stage);
    for (folder, names) in [
        (root.join("derived/search-index"), &["a", "b"][..]),
        (stage.join("derived/search-index"), &["c"][..]),
    ] {
        std::fs::create_dir_all(&folder).unwrap();
        for name in names {
            std::fs::write(folder.join(name), name.as_bytes()).unwrap();
        }
    }
    files.push("derived/search-index".into());
    files
}

// A folder is known by what its files hold: not by its own times, which some file systems change
// when the folder moves, nor by the `.DS_Store` Finder leaves in it. The old one comes back at every
// cut, while a folder holding another file stops the rollback.
#[test]
fn a_folder_entry_is_known_by_what_is_in_it() {
    for steps in 0..=8 {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = with_folder(root.path(), &stage);
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
        }
        let backup = journal_file(root.path())["backup"].as_str().unwrap().to_owned();
        for folder in [
            root.path().join("derived/search-index"),
            root.path().join(&backup).join("derived/search-index"),
        ] {
            if folder.is_dir() {
                std::fs::write(folder.join(".DS_Store"), b"finder").unwrap();
                std::fs::File::open(&folder)
                    .unwrap()
                    .set_modified(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap();
            }
        }
        recover(&paths).unwrap_or_else(|error| panic!("{steps}: {error:#}"));
        let mut names: Vec<_> = std::fs::read_dir(root.path().join("derived/search-index"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.retain(|name| name != ".DS_Store");
        names.sort();
        assert_eq!(names, ["a", "b"], "{steps}");
    }

    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = with_folder(root.path(), &stage);
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    for step in 0..8 {
        install_step(&paths, &mut journal, step).unwrap();
    }
    std::fs::write(root.path().join("derived/search-index/d"), b"another folder's").unwrap();
    let before = super::creation::tests::files(root.path());
    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-invalid"), "{error:#}");
    assert_eq!(super::creation::tests::files(root.path()), before);
}

// A copy of the folder made while an install was cut short, and cut short itself while it copied
// the backup: the backup's file is shorter than the original was, and the complete staged file is
// in place. Putting the short one back would leave a damaged store and delete the whole one with
// the stage. The backup must hold what the original held.
#[test]
fn a_backup_cut_short_by_a_copy_is_not_put_back() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = old_and_new(root.path(), &stage);
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    for step in 0..2 {
        install_step(&paths, &mut journal, step).unwrap();
    }
    let backup = journal_file(root.path())["backup"].as_str().unwrap().to_owned();
    std::fs::write(root.path().join(&backup).join("archive/history-vault.db"), b"ol").unwrap();
    let before = super::creation::tests::files(root.path());
    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-control-invalid"), "{error:#}");
    assert_eq!(super::creation::tests::files(root.path()), before);
}

// The same copy, missing one file of the backup: the staged file is in place and nothing says the
// original was ever moved back. Taking it for the original would roll the other files back around
// it, a store of the new archive beside the old control.
#[test]
fn an_original_the_copy_left_out_is_not_taken_for_one_put_back() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = old_and_new(root.path(), &stage);
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    for step in 0..6 {
        install_step(&paths, &mut journal, step).unwrap();
    }
    let backup = journal_file(root.path())["backup"].as_str().unwrap().to_owned();
    std::fs::remove_file(root.path().join(&backup).join("archive/history-vault.db")).unwrap();
    let before = super::creation::tests::files(root.path());
    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("neither in place nor in the backup"), "{error:#}");
    assert_eq!(super::creation::tests::files(root.path()), before);
}

// An import into a new folder has no control until the staged one lands. A crash before that is an
// interrupted install, not an archive that lost its control: the next start rolls it back to the
// empty folder it was, and a first run can begin.
#[test]
fn an_import_into_a_new_folder_cut_short_is_rolled_back_at_the_next_start() {
    for steps in 0..6 {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let files = new_only(root.path(), &stage);
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        for step in 0..steps {
            install_step(&paths, &mut journal, step).unwrap();
        }
        assert!(
            matches!(
                crate::admission::inspect(root.path()),
                Ok(crate::admission::RootState::Interrupted(
                    crate::admission::Interrupted::Install
                ))
            ),
            "{steps}"
        );
        recover(&paths).unwrap_or_else(|error| panic!("{steps}: {error:#}"));
        assert!(
            matches!(
                crate::admission::inspect(root.path()),
                Ok(crate::admission::RootState::Uninitialized)
            ),
            "{steps}"
        );
        assert!(crate::admission::archive_files(root.path()).unwrap().is_empty(), "{steps}");
    }
}

// Something put into the folder during an install, where the install brings a file that was not
// there before, is not replaced: the move fails, and the rollback stops at a file it did not put
// there.
#[test]
fn a_file_that_appears_in_place_during_an_install_is_not_replaced() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let files = new_only(root.path(), &stage);
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    std::fs::write(root.path().join("archive/keys.envelope"), b"someone else's").unwrap();
    let failed = (0..6).find_map(|step| install_step(&paths, &mut journal, step).err());
    assert!(failed.is_some(), "the move over the other file went through");
    assert_eq!(
        std::fs::read(root.path().join("archive/keys.envelope")).unwrap(),
        b"someone else's"
    );
    let before = super::creation::tests::files(root.path());
    recover(&paths).expect_err("refused");
    assert_eq!(super::creation::tests::files(root.path()), before);
}

/// Where an install's file is at a cut rename on a file system without a journal: under both
/// names, which a hard link stands in for (one file, two names).
#[derive(Clone, Copy, Debug)]
enum Cut {
    /// Moving the original into the backup.
    IntoBackup,
    /// Moving the staged file into place.
    IntoPlace,
    /// Taking the staged file out of its place, in a rollback.
    Discarding,
    /// Putting the original back, in a rollback.
    PuttingBack,
    /// Renaming the staged control over the live one.
    OverControl,
}

// A rename cut by a power cut on exFAT or FAT32 can leave one file under two names. Deleting one
// frees the data the other still uses, and the next write there overwrites the archive. A rollback
// ends with the original in place, and no name that shared its data is deleted: it is in the
// quarantine. Two names of a file that leaves its place go together.
#[cfg(unix)]
#[test]
fn a_file_under_two_names_after_a_cut_rename_keeps_both() {
    use std::os::unix::fs::MetadataExt;
    let cuts =
        [Cut::IntoBackup, Cut::IntoPlace, Cut::Discarding, Cut::PuttingBack, Cut::OverControl];
    for (cut, same) in cuts.into_iter().flat_map(|cut| [(cut, false), (cut, true)]) {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("staging/maintenance-test");
        let db = "archive/history-vault.db";
        for (folder, bytes) in [(root.path(), &b"old"[..]), (stage.as_path(), b"new")] {
            std::fs::create_dir_all(folder.join("archive")).unwrap();
            std::fs::write(folder.join(db), if same { b"old" } else { bytes }).unwrap();
            std::fs::write(folder.join("config.json"), bytes).unwrap();
        }
        let files = vec![db.into(), "config.json".into()];
        let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
        let backup = root.path().join(journal_file(root.path())["backup"].as_str().unwrap());
        let (place, staged) = (root.path().join(db), stage.join(db));
        let link = |from: &std::path::Path, to: &std::path::Path| {
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::hard_link(from, to).unwrap();
        };
        let pair = match cut {
            Cut::IntoBackup => {
                link(&place, &backup.join(db));
                &place
            }
            Cut::IntoPlace => {
                install_step(&paths, &mut journal, 0).unwrap();
                link(&staged, &place);
                &place
            }
            Cut::Discarding => {
                install_step(&paths, &mut journal, 0).unwrap();
                install_step(&paths, &mut journal, 1).unwrap();
                link(&place, &stage.join(".discarded/0").join(db));
                &place
            }
            Cut::PuttingBack => {
                install_step(&paths, &mut journal, 0).unwrap();
                link(&backup.join(db), &place);
                &place
            }
            Cut::OverControl => {
                for step in 0..3 {
                    install_step(&paths, &mut journal, step).unwrap();
                }
                std::fs::remove_file(root.path().join("config.json")).unwrap();
                link(&stage.join("config.json"), &root.path().join("config.json"));
                &root.path().join("config.json").clone()
            }
        };
        let shared = std::fs::metadata(pair).unwrap().ino();
        let names_of = |ino: u64| -> Vec<std::path::PathBuf> {
            walkdir::WalkDir::new(root.path())
                .into_iter()
                .map(|entry| entry.unwrap())
                .filter(|entry| {
                    entry.file_type().is_file() && entry.metadata().unwrap().ino() == ino
                })
                .map(|entry| entry.into_path())
                .collect()
        };
        if matches!(cut, Cut::OverControl) || (matches!(cut, Cut::Discarding) && !same) {
            // The first file the rollback takes out of its place shares its data with a name
            // that stays until the stage goes. Written over, or moved onto that name, it would
            // leave one name alone with the data for the whole rest of the rollback: it is moved
            // under a name of its own, and both names go together with the stage.
            let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _cut = crate::fault_inject::FaultGuard::panic_at("native-install-discarded");
                recover(&paths)
            }));
            assert!(crashed.is_err(), "{cut:?} {same}: nothing was taken out of its place");
            assert_eq!(names_of(shared).len(), 2, "{same}: {:?}", names_of(shared));
        }

        recover(&paths).unwrap_or_else(|error| panic!("{cut:?} {same}: {error:#}"));
        assert_eq!(std::fs::read(&place).unwrap(), b"old", "{cut:?} {same}");
        assert_eq!(std::fs::read(root.path().join("config.json")).unwrap(), b"old", "{cut:?}");
        assert!(!pending(&paths), "{cut:?} {same}");
        let names = names_of(shared);
        let in_place =
            |name: &std::path::Path| name == place || name == root.path().join("config.json");
        if names.iter().any(|name| in_place(name)) {
            // The file in place is one of the two: its other name is kept, in the quarantine.
            assert_eq!(names.len(), 2, "{cut:?} {same}: {names:?}");
            assert!(
                names.iter().any(|name| name.starts_with(root.path().join("quarantine"))),
                "{cut:?} {same}: {names:?}"
            );
        } else {
            // Neither stays in place: both went with the stage.
            assert!(names.is_empty(), "{cut:?} {same}: {names:?}");
        }
    }
}

// A store the install brings is staged with everything in its main file, so its live log is an
// entry the install removes: moved into the backup, nothing put in its place. A power cut on exFAT
// during that move leaves the log under both names. The rollback keeps the log in place and its
// other name in the quarantine. A hard link stands in for the two names.
#[cfg(unix)]
#[test]
fn a_file_the_install_removes_keeps_both_names_after_a_cut_move() {
    use std::os::unix::fs::MetadataExt;
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("staging/maintenance-test");
    let (db, log) = ("archive/history-vault.db", "archive/history-vault.db-wal");
    for folder in [root.path(), stage.as_path()] {
        std::fs::create_dir_all(folder.join("archive")).unwrap();
    }
    for (name, live, staged) in [(db, &b"old"[..], Some(&b"new"[..])), (log, b"old log", None)]
        .into_iter()
        .chain([("config.json", &b"old"[..], Some(&b"new"[..]))])
    {
        std::fs::write(root.path().join(name), live).unwrap();
        if let Some(bytes) = staged {
            std::fs::write(stage.join(name), bytes).unwrap();
        }
    }
    let files = vec![db.into(), log.into(), "config.json".into()];
    let mut journal = prepare(&paths, &stage, files, Keep::Nothing).unwrap();
    install_step(&paths, &mut journal, 0).unwrap();
    install_step(&paths, &mut journal, 1).unwrap();
    let backup = root.path().join(journal_file(root.path())["backup"].as_str().unwrap());
    std::fs::create_dir_all(backup.join("archive")).unwrap();
    std::fs::hard_link(root.path().join(log), backup.join(log)).unwrap();

    recover(&paths).unwrap_or_else(|error| panic!("{error:#}"));
    assert_eq!(std::fs::read(root.path().join(db)).unwrap(), b"old");
    assert_eq!(std::fs::read(root.path().join(log)).unwrap(), b"old log");
    assert_eq!(std::fs::metadata(root.path().join(log)).unwrap().nlink(), 2, "a name was deleted");
    assert!(root.path().join("quarantine").is_dir());
    assert!(!pending(&paths));
}
