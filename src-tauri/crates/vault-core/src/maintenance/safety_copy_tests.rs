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
//! crash after the publish leaves the replaced files hidden; an import into an empty folder leaves
//! an empty copy behind; the copy keeps a search index nothing reads.
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
    let snapshots = paths.artifact_path(vault_layout::ArtifactId::RawSnapshots);
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
