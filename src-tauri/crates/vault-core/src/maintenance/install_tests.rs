//! Responsibilities: exercise each multi-file install journal boundary.
//! Not responsible for: database engines or password derivation.
//! Failure modes: lost original, mixed committed files, stale WAL, non-idempotent recovery.
use super::install::*;
use crate::project_paths_with_root;

#[test]
fn every_interrupted_swap_rolls_back() {
    for interrupted_after in 0..=4 {
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
        let mut journal = prepare(&paths, &stage, files, true).unwrap();
        for step in 0..interrupted_after {
            install_step(&paths, &mut journal, step).unwrap();
        }
        recover(&paths).unwrap();
        recover(&paths).unwrap();
        for name in ["archive/history-vault.db", "archive/source-evidence.db"] {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"old");
        }
        assert_eq!(std::fs::read(root.path().join("archive/.pk-owner.lock")).unwrap(), b"owner");
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
    let mut journal = prepare(&paths, &stage, vec!["config.json".into()], true).unwrap();
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
        let mut journal = prepare(&paths, &stage, files, false).unwrap();
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
    assert!(prepare(&paths, &stage, vec!["config.json".into()], false).is_err());
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
    prepare(&paths, &stage, files, false).unwrap();
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
    let mut journal = prepare(&project_paths_with_root(&before), &stage, files, false).unwrap();
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
    let mut journal = prepare(&project_paths_with_root(&first), &stage, files, false).unwrap();
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
    let mut journal = prepare(&paths, &stage, files, false).unwrap();
    install_step(&paths, &mut journal, 0).unwrap();
    let backup = journal_file(root.path())["backup"].as_str().unwrap().to_owned();
    std::fs::remove_file(root.path().join(&backup).join("archive/history-vault.db")).unwrap();
    let error = recover(&paths).expect_err("refused");
    assert!(format!("{error:#}").contains("neither in place nor in the backup"), "{error:#}");
    assert!(root.path().join("archive/.pk-install-v2.json").exists());
}
