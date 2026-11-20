//! Responsibilities: prove that the archive a restore or an import replaces becomes a safety copy:
//! listed with the others under the operation it was made before, beside them where storage usage
//! counts it and freeing space removes it, sealed, without the search index a restore builds again,
//! and restoring the archive it was. Also when the process dies after the new archive is published
//! and before the copy is made.
//! Not responsible for: the install journal's rollback (`install_tests`), the agent conversations
//! a restore keeps (`restore_tests`), how freeing space chooses what to remove (`retention`).
//! Failure modes: the replaced archive is left in a folder no screen shows and nothing frees; the
//! copy is listed but does not restore (no seal, a wrong one, or one that requires the agent
//! conversations a restore left in place); restoring it does not bring back what was replaced; a
//! crash after the publish leaves the replaced files hidden; a rename cut on a file system without
//! a journal, which leaves the replaced archive under both names, is taken for another copy already
//! there, and the archive stays hidden and unsealed; an import into an empty folder leaves an empty
//! copy behind; the copy keeps a search index nothing reads; a copy whose seal could not be written
//! (an error while reading it) can never be restored, though its files are whole; a restore seals a
//! safety snapshot cut short before its seal, which may lack files, or writes a seal into a folder
//! outside the archive's own copies.
use super::creation::tests::{ADDED, marker, open, released_archive, write_marker};
use super::snapshot::{list_snapshots, restore_snapshot, snapshot};
use crate::{ApplyImportOptions, ProjectPaths, RecoverySnapshot, Vault, project_paths_with_root};
use std::path::{Path, PathBuf};
use vault_store::Db;

/// The safety copies made before `op`, as the restore list shows them.
fn copies_before(paths: &ProjectPaths, op: &str) -> Vec<RecoverySnapshot> {
    list_snapshots(paths).unwrap().into_iter().filter(|copy| copy.source_op == op).collect()
}

/// The one copy made before `op`, checked to be offered for a restore.
fn the_copy_before(paths: &ProjectPaths, op: &str) -> PathBuf {
    let copies = copies_before(paths, op);
    assert_eq!(copies.len(), 1, "{copies:?}");
    assert!(copies[0].verified_openable, "{copies:?}");
    let copy = PathBuf::from(&copies[0].path);
    let snapshots = paths.app_root.join("quarantine/v06/previous-generations");
    assert!(
        copy.starts_with(&snapshots),
        "{} is not counted or freed with the others",
        copy.display()
    );
    copy
}

/// Folders of replaced files in the archive's root, which no screen lists.
fn hidden_backups(paths: &ProjectPaths) -> Vec<String> {
    std::fs::read_dir(&paths.app_root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("maintenance.bak-"))
        .collect()
}

/// A snapshot of the released archive at `paths`, taken outside it.
fn older_copy(paths: &ProjectPaths) -> (tempfile::TempDir, PathBuf) {
    let copies = tempfile::tempdir().unwrap();
    let older = copies.path().join("older");
    let vault = open(paths).unwrap();
    snapshot(&vault, &older).unwrap();
    vault.close().unwrap();
    (copies, older)
}

#[test]
fn repair_source_listing_uses_only_regular_inventory_members() {
    let temp = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(temp.path());
    let root = paths.artifact_path(vault_layout::ArtifactId::NativeSnapshots);
    let evidence = vault_layout::database_path(Db::AgentHistoryEvidence);
    let model = vault_layout::database_path(Db::AgentHistoryReadModel);
    let held = format!("{}/{evidence}", vault_layout::HELD_CONVERSATIONS);
    let older = format!("{}/older-2/{evidence}", vault_layout::HELD_CONVERSATIONS);
    let unrelated = format!("other/{evidence}");
    let wal = format!("{evidence}-wal");
    for (name, marked, sealed, files, expected) in [
        ("old", false, true, vec![], "missingEvidence"),
        ("pair", false, true, vec![evidence, model], "checkRequired"),
        ("only-evidence", false, true, vec![evidence], "missingReadModel"),
        ("marked-model", true, true, vec![model, &wal, &unrelated], "missingEvidence"),
        ("marked-evidence", true, true, vec![evidence], "checkRequired"),
        ("current-held", true, true, vec![&held], "checkRequired"),
        ("older-held", true, true, vec![&older], "checkRequired"),
        ("before-restore-unsealed", false, false, vec![evidence, model], "unsealed"),
    ] {
        let copy = root.join(name);
        std::fs::create_dir_all(&copy).unwrap();
        for file in files
            .into_iter()
            .chain(marked.then_some(vault_layout::UNREADABLE_CONVERSATIONS))
            .chain(sealed.then_some(super::snapshot::SEAL))
        {
            let member = copy.join(file);
            std::fs::create_dir_all(member.parent().unwrap()).unwrap();
            // Listing must neither parse the seal nor try to open a conversation database.
            std::fs::write(member, b"unverified").unwrap();
        }
        let row = list_snapshots(&paths)
            .unwrap()
            .into_iter()
            .find(|r| Path::new(&r.path) == copy)
            .unwrap();
        assert_eq!(serde_json::json!(row)["conversationRepairSource"], expected, "{name}");
    }
}

#[test]
fn a_restore_keeps_the_archive_it_replaced_as_a_copy_that_brings_it_back() {
    let (_root, paths) = released_archive();
    let (_copies, older) = older_copy(&paths);
    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "replaced-by-the-restore");
    write_marker(live.store(), Db::AgentHistoryEvidence, "conversations");

    let restored = restore_snapshot(&live, &older, None, Default::default()).unwrap();
    assert_eq!(marker(&restored.vault, Db::Archive).as_deref(), Some("released-archive"));
    assert!(hidden_backups(&paths).is_empty(), "{:?}", hidden_backups(&paths));
    let copy = the_copy_before(&paths, "restore");
    assert_eq!(restored.previous_files, copy);
    assert!(!copy.join("derived/search-index").exists(), "a restore builds its own");
    // The restore left the conversations where they were, so the copy has none.
    for added in ADDED {
        assert!(!copy.join(added).exists(), "{added}");
    }

    let back = restore_snapshot(&restored.vault, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&back.vault, Db::Archive).as_deref(), Some("replaced-by-the-restore"));
    assert_eq!(marker(&back.vault, Db::AgentHistoryEvidence).as_deref(), Some("conversations"));
    // Going back kept the archive it replaced too.
    assert_eq!(copies_before(&paths, "restore").len(), 2);
}

#[test]
fn a_crash_after_the_restore_is_published_still_makes_the_copy() {
    let (_root, paths) = released_archive();
    let (_copies, older) = older_copy(&paths);
    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "replaced-by-the-restore");
    let fault = crate::fault_inject::FaultGuard::panic_at("native-install-published");
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        restore_snapshot(&live, &older, None, Default::default())
    }));
    drop(fault);
    assert!(crashed.is_err(), "the restore was cut after its publish");
    drop(live);

    // What the next start runs before anything opens the archive.
    super::recover_native_maintenance(&paths).unwrap();
    assert!(hidden_backups(&paths).is_empty(), "{:?}", hidden_backups(&paths));
    let copy = the_copy_before(&paths, "restore");
    let reopened = open(&paths).unwrap();
    assert_eq!(marker(&reopened, Db::Archive).as_deref(), Some("released-archive"));
    let back = restore_snapshot(&reopened, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&back.vault, Db::Archive).as_deref(), Some("replaced-by-the-restore"));
}

// The seal of a replaced archive is written after the new archive is in place, by reading every
// file again. An error then (a file another program held, a read that failed once) leaves the copy
// without one. Its files are the archive as it was replaced, moved whole, so the copy is still
// offered, and the restore that needs it seals it first.
#[test]
fn a_replaced_archive_left_unsealed_is_sealed_by_the_restore_that_needs_it() {
    let (_root, paths) = released_archive();
    let (_copies, older) = older_copy(&paths);
    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "replaced-by-the-restore");
    let restored = restore_snapshot(&live, &older, None, Default::default()).unwrap();
    let copy = PathBuf::from(&copies_before(&paths, "restore")[0].path);
    // What an error while sealing leaves.
    std::fs::remove_file(copy.join("snapshot.json")).unwrap();

    assert_eq!(the_copy_before(&paths, "restore"), copy);
    let back = restore_snapshot(&restored.vault, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&back.vault, Db::Archive).as_deref(), Some("replaced-by-the-restore"));
    assert!(copy.join("snapshot.json").is_file(), "sealed as the restore found it");
}

// When the seal cannot be written (here a failure made at that step), the restore still finishes,
// with the new archive in place, and says why, so the user hears of a copy that may not be readable
// now rather than when they need it. The copy is offered as not checked yet; the restore from it
// checks it first, and from then on it is checked.
#[test]
fn a_copy_the_restore_could_not_seal_is_reported_and_offered_as_not_checked() {
    let (_root, paths) = released_archive();
    let (_copies, older) = older_copy(&paths);
    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "replaced-by-the-restore");
    let fault = crate::fault_inject::FaultGuard::error_at("native-seal-before-restore");
    let restored = restore_snapshot(&live, &older, None, Default::default()).unwrap();
    drop(fault);
    let reason = restored.previous_unsealed.clone().expect("the restore says why");
    assert!(reason.contains("simulated error"), "{reason}");
    let listed = copies_before(&paths, "restore");
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(listed[0].verified_openable && listed[0].unchecked, "{listed:?}");

    let copy = PathBuf::from(&listed[0].path);
    let back = restore_snapshot(&restored.vault, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&back.vault, Db::Archive).as_deref(), Some("replaced-by-the-restore"));
    assert_eq!(back.previous_unsealed, None);
    let listed = copies_before(&paths, "restore");
    assert!(listed.iter().all(|copy| !copy.unchecked), "{listed:?}");
}

// The same for an import: it finishes and says why, and the copy of what it replaced is offered
// as not checked yet.
#[test]
fn a_copy_the_import_could_not_seal_is_reported_and_offered_as_not_checked() {
    let (_source_root, source) = released_archive();
    let bundles = tempfile::tempdir().unwrap();
    let bundle = bundles.path().join("moved.pathkeep");
    {
        let vault = open(&source).unwrap();
        crate::migration::native::export(
            &vault,
            &bundle,
            "export password",
            Default::default(),
            |_| {},
        )
        .unwrap();
        vault.close().unwrap();
    }
    let (_root, paths) = released_archive();
    let live = open(&paths).unwrap();
    let options = ApplyImportOptions {
        confirm_overwrite: true,
        source_archive_key: Some("export password".into()),
    };
    let fault = crate::fault_inject::FaultGuard::error_at("native-seal-before-import");
    let (report, _imported) = crate::migration::native::import(
        &paths,
        Some(&live),
        None,
        &bundle,
        &options,
        Default::default(),
    )
    .unwrap();
    drop(fault);
    assert!(report.kept_previous_as_copy);
    let reason = report.previous_copy_unchecked.expect("the import says why");
    assert!(reason.contains("simulated error"), "{reason}");
    let listed = copies_before(&paths, "import");
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(listed[0].verified_openable && listed[0].unchecked, "{listed:?}");
}

// Only a replaced archive in the archive's own folder of copies is sealed by a restore. A safety
// snapshot without a seal was cut short while it was written and may lack files; a folder named like
// a copy elsewhere is not PathKeep's to write in. Both are refused with nothing written.
#[test]
fn a_restore_seals_no_snapshot_cut_short_and_no_folder_elsewhere() {
    let (_root, paths) = released_archive();
    let (_copies, older) = older_copy(&paths);
    let live = open(&paths).unwrap();
    let restored = restore_snapshot(&live, &older, None, Default::default()).unwrap();
    let copy = PathBuf::from(&copies_before(&paths, "restore")[0].path);
    std::fs::remove_file(copy.join("snapshot.json")).unwrap();
    let folder = copy.parent().unwrap();
    let cut_short = folder.join("rekey-cut-short");
    super::move_copies_tests::copy_tree(&copy, &cut_short);
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = elsewhere.path().join(copy.file_name().unwrap());
    super::move_copies_tests::copy_tree(&copy, &outside);

    for refused in [&cut_short, &outside] {
        assert!(
            restore_snapshot(&restored.vault, refused, None, Default::default()).is_err(),
            "{} restored",
            refused.display()
        );
        assert!(!refused.join("snapshot.json").exists(), "{} was sealed", refused.display());
    }
    assert_eq!(marker(&restored.vault, Db::Archive).as_deref(), Some("released-archive"));
}

// On FAT32 or exFAT a power cut during the rename that makes the replaced archive a copy can leave
// it under both names, each the same data (the install module's docs). Finishing does not take that
// for another copy already there: the copy is sealed and offered for a restore, and the backup's
// name goes into the quarantine, which the storage screen counts and the user frees once the disk
// is checked. Deleting it would free the copy's data; leaving it in the root hides it.
#[test]
fn a_replaced_archive_left_under_both_names_by_a_cut_rename_still_becomes_the_copy() {
    let (_root, paths) = released_archive();
    let (_copies, older) = older_copy(&paths);
    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "replaced-by-the-restore");
    let fault = crate::fault_inject::FaultGuard::panic_at("native-install-published");
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        restore_snapshot(&live, &older, None, Default::default())
    }));
    drop(fault);
    assert!(crashed.is_err(), "the restore was cut after its publish");
    drop(live);
    let backup = hidden_backups(&paths);
    assert_eq!(backup.len(), 1, "{backup:?}");
    let backup = paths.app_root.join(&backup[0]);
    let journal = vault_layout::spec(vault_layout::ArtifactId::InstallJournalV2).path.relative();
    let journal: serde_json::Value =
        serde_json::from_slice(&std::fs::read(paths.app_root.join(journal)).unwrap()).unwrap();
    let copy = paths.app_root.join(journal["kept_as"].as_str().expect("the copy's name"));
    // The rename was cut: the copy's name has the replaced archive too.
    super::move_copies_tests::copy_tree(&backup, &copy);
    let replaced = std::fs::read(backup.join("archive/history-vault.db")).unwrap();

    super::recover_native_maintenance(&paths).unwrap();
    assert!(hidden_backups(&paths).is_empty(), "{:?}", hidden_backups(&paths));
    assert_eq!(the_copy_before(&paths, "restore"), copy);
    let quarantined: Vec<PathBuf> = walkdir::WalkDir::new(&paths.quarantine_dir)
        .into_iter()
        .map(|entry| entry.unwrap().into_path())
        .filter(|path| !path.starts_with(&copy) && path.ends_with("archive/history-vault.db"))
        .collect();
    assert_eq!(quarantined.len(), 1, "{quarantined:?}");
    assert_eq!(std::fs::read(&quarantined[0]).unwrap(), replaced);
    let reopened = open(&paths).unwrap();
    assert_eq!(marker(&reopened, Db::Archive).as_deref(), Some("released-archive"));
    let back = restore_snapshot(&reopened, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&back.vault, Db::Archive).as_deref(), Some("replaced-by-the-restore"));
}

#[test]
fn an_import_keeps_the_archive_it_replaced_and_into_an_empty_folder_keeps_nothing() {
    let (_source_root, source) = released_archive();
    let bundles = tempfile::tempdir().unwrap();
    let bundle = bundles.path().join("moved.pathkeep");
    {
        let vault = open(&source).unwrap();
        write_marker(vault.store(), Db::Archive, "from-the-other-computer");
        crate::migration::native::export(
            &vault,
            &bundle,
            "export password",
            Default::default(),
            |_| {},
        )
        .unwrap();
        vault.close().unwrap();
    }
    let import = |paths: &ProjectPaths, current: Option<&Vault>| {
        let options = ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some("export password".into()),
        };
        crate::migration::native::import(
            paths,
            current,
            None,
            &bundle,
            &options,
            Default::default(),
        )
        .unwrap()
    };

    let empty_root = tempfile::tempdir().unwrap();
    let empty = project_paths_with_root(empty_root.path());
    let (report, installed) = import(&empty, None);
    assert!(!report.kept_previous_as_copy);
    installed.vault.close().unwrap();
    assert!(copies_before(&empty, "import").is_empty());
    assert!(hidden_backups(&empty).is_empty(), "{:?}", hidden_backups(&empty));
    let snapshots = empty.artifact_path(vault_layout::ArtifactId::NativeSnapshots);
    let kept: Vec<_> = std::fs::read_dir(&snapshots).into_iter().flatten().collect();
    assert!(kept.is_empty(), "{kept:?}");

    let (_root, paths) = released_archive();
    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "replaced-by-the-import");
    let (report, imported) = import(&paths, Some(&live));
    assert!(report.kept_previous_as_copy);
    assert_eq!(marker(&imported.vault, Db::Archive).as_deref(), Some("from-the-other-computer"));
    assert!(hidden_backups(&paths).is_empty(), "{:?}", hidden_backups(&paths));
    let copy = the_copy_before(&paths, "import");
    assert!(!copy.join("derived/search-index").exists(), "a restore builds its own");
    let back =
        restore_snapshot(&imported.vault, Path::new(&copy), None, Default::default()).unwrap();
    assert_eq!(marker(&back.vault, Db::Archive).as_deref(), Some("replaced-by-the-import"));
}

// The copy of an encrypted archive holds its key file, the archive's own, so it restores with the
// password the archive had: the one it still has, since a restore keeps it.
#[test]
fn the_copy_of_an_encrypted_archive_restores_with_its_password() {
    let (_root, vault, _) = super::tests::encrypted();
    let paths = vault.paths().clone();
    let copies = tempfile::tempdir().unwrap();
    let older = copies.path().join("older");
    write_marker(vault.store(), Db::Archive, "in-the-older-copy");
    snapshot(&vault, &older).unwrap();
    write_marker(vault.store(), Db::Archive, "replaced-by-the-restore");

    let restored =
        restore_snapshot(&vault, &older, Some("old password"), Default::default()).unwrap();
    let copy = the_copy_before(&paths, "restore");
    assert!(copies_before(&paths, "restore")[0].encrypted);
    let back =
        restore_snapshot(&restored.vault, &copy, Some("old password"), Default::default()).unwrap();
    assert_eq!(marker(&back.vault, Db::Archive).as_deref(), Some("replaced-by-the-restore"));
}
