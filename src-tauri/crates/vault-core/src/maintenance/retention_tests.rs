//! Responsibilities: prove that "Free up space" deletes exactly what the user ticked: each safety
//! copy only when it is picked, the newest one too; the other snapshots without any listed copy;
//! the sizes it shows are the sizes it deletes; the ledger follows the files.
//! Not responsible for: what a copy holds (`snapshot` tests), restoring from one
//! (`restore_tests`, `safety_copy_tests`), listing old-format copies (the worker), the dialog (E2E
//! `free-space.spec.ts`).
//! Failure modes: a copy the user did not pick is deleted (the one just restored from, after a
//! restore); the copy a restore or an import just made cannot be freed; a copy made while the prune
//! waited goes with the other snapshots; an old-format copy's SQLite files are split from it; the
//! size shown is not what is deleted; a path that is not a listed copy is deleted on request; a
//! symlink is followed out of the data folder; ledger rows outlive their files, or go with files
//! that stay.
use super::snapshot::{list_snapshots, snapshot};
use super::{preview_retention, prune};
use crate::{RecoverySnapshot, RetentionPruneRequest, Vault};
use std::path::{Path, PathBuf};
use vault_store::Db;

/// Three safety copies, oldest first, and a browser checkpoint a backup saved.
fn archive_with_copies() -> (tempfile::TempDir, Vault, Vec<PathBuf>, PathBuf) {
    let (root, vault, _) = super::tests::encrypted();
    let maintenance = vault.paths().raw_snapshots_dir.join("maintenance");
    let mut copies = Vec::new();
    for name in ["rekey-a", "before-restore-b", "before-import-c"] {
        let path = maintenance.join(name);
        snapshot(&vault, &path).unwrap();
        super::record_restore(&vault, &path.display().to_string()).unwrap();
        // Folder times order the list, ten seconds apart.
        let when = std::time::SystemTime::now()
            - std::time::Duration::from_secs(60 - copies.len() as u64 * 10);
        std::fs::File::open(&path).unwrap().set_modified(when).unwrap();
        copies.push(path);
    }
    let checkpoint = vault.paths().raw_snapshots_dir.join("chrome-Default/2026-11-15T00-00-00Z-7");
    std::fs::create_dir_all(&checkpoint).unwrap();
    std::fs::write(checkpoint.join("History"), vec![7u8; 10_000]).unwrap();
    (root, vault, copies, checkpoint)
}

/// The copies as the worker lists the native ones, newest first.
fn listed(vault: &Vault) -> Vec<RecoverySnapshot> {
    let mut copies = list_snapshots(vault.paths()).unwrap();
    copies.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    copies
}

fn tree_bytes(path: &Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.metadata().unwrap().len())
        .sum()
}

/// The ledger's paths; its count in the archive totals agrees with them.
fn ledger(vault: &Vault) -> Vec<String> {
    let (mut rows, total) = vault
        .store()
        .read(Db::Archive, |s| {
            let rows =
                s.query("SELECT file_path FROM snapshots", (), |r| Ok(r.get::<String>(0)?))?;
            let total =
                s.query_one("SELECT value FROM archive_totals WHERE name='snapshots'", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })?;
            Ok((rows, total))
        })
        .unwrap();
    assert_eq!(total, rows.len() as i64, "the totals count the ledger rows");
    rows.sort();
    rows
}

fn request(buckets: &[&str], copies: &[&Path]) -> RetentionPruneRequest {
    RetentionPruneRequest {
        bucket_ids: buckets.iter().map(|id| id.to_string()).collect(),
        copies: copies.iter().map(|path| path.display().to_string()).collect(),
    }
}

fn run(
    vault: &Vault,
    request: &RetentionPruneRequest,
) -> anyhow::Result<crate::RetentionPruneResult> {
    prune(vault, request, || Ok(listed(vault)), &Default::default())
}

// The case of the report: the copy a restore just made is the newest, and it is the one the user
// frees. It goes, and nothing else does: not the copy they restored from, not the others.
#[test]
fn the_newest_copy_goes_when_picked_and_no_other_copy_does() {
    let (_root, vault, copies, checkpoint) = archive_with_copies();
    let newest = &copies[2];
    let preview = preview_retention(vault.paths(), &listed(&vault)).unwrap();
    let shown = preview
        .copies
        .iter()
        .find(|copy| Path::new(&copy.path) == newest.as_path())
        .expect("the newest copy is listed")
        .bytes;
    assert_eq!(shown, tree_bytes(newest), "the size shown is the whole copy");
    let restore_list = listed(&vault);
    let in_restore_list = restore_list.iter().find(|copy| Path::new(&copy.path) == newest).unwrap();
    assert_eq!(in_restore_list.size_bytes, shown, "the restore list shows the same size");

    let result = run(&vault, &request(&[], &[newest])).unwrap();
    assert!(!newest.exists(), "the copy picked is gone");
    assert_eq!(result.deleted_bytes, shown, "it deleted what it showed");
    for kept in &copies[..2] {
        assert!(kept.join("snapshot.json").is_file(), "{} was not picked", kept.display());
    }
    assert!(checkpoint.join("History").is_file(), "the other snapshots were not picked");
    let rows = ledger(&vault);
    assert!(!rows.contains(&newest.display().to_string()), "its ledger row goes with it");
    assert_eq!(rows.len(), 2, "{rows:?}");
}

// The row for the other snapshots (browser checkpoints, safety snapshots cut short before their
// seal) never takes a listed copy with it, the newest included, and shows only its own size. The
// archive a restore or an import replaced is a listed copy even when it has no seal: a row of its
// own, freed only when picked. This one has a control that cannot be read (a restore replaced a
// damaged archive), so nothing can seal it and it cannot be restored from.
#[test]
fn the_other_snapshots_go_without_any_copy() {
    let (_root, vault, copies, checkpoint) = archive_with_copies();
    let maintenance = vault.paths().raw_snapshots_dir.join("maintenance");
    let cut_short = maintenance.join("rekey-cut-short");
    std::fs::create_dir_all(cut_short.join("archive")).unwrap();
    std::fs::write(cut_short.join("archive/history-vault.db"), vec![1u8; 5_000]).unwrap();
    let unsealed = maintenance.join("before-restore-unsealed");
    std::fs::create_dir_all(unsealed.join("archive")).unwrap();
    std::fs::write(unsealed.join("archive/history-vault.db"), vec![2u8; 6_000]).unwrap();
    std::fs::write(unsealed.join("config.json"), b"{ damaged").unwrap();
    let preview = preview_retention(vault.paths(), &listed(&vault)).unwrap();
    let bucket = preview.buckets.iter().find(|bucket| bucket.id == "snapshots").unwrap();
    assert_eq!(bucket.bytes, tree_bytes(&checkpoint) + tree_bytes(&cut_short));
    let row = preview
        .copies
        .iter()
        .find(|copy| Path::new(&copy.path) == unsealed.as_path())
        .expect("the unsealed replaced archive is a row of its own");
    assert_eq!((row.source_op.as_str(), row.restorable), ("restore", false));
    assert_eq!(row.bytes, tree_bytes(&unsealed));

    let result = run(&vault, &request(&["snapshots"], &[])).unwrap();
    assert_eq!(result.deleted_bytes, bucket.bytes, "it deleted what it showed");
    assert!(!checkpoint.exists() && !cut_short.exists());
    assert!(unsealed.join("archive/history-vault.db").is_file(), "it was not picked");
    let result = run(&vault, &request(&[], &[&unsealed])).unwrap();
    assert_eq!(result.deleted_bytes, row.bytes);
    assert!(!unsealed.exists(), "picked, it goes");
    for copy in &copies {
        assert!(copy.join("snapshot.json").is_file(), "{} was not picked", copy.display());
    }
    assert_eq!(ledger(&vault).len(), 3, "the copies' rows stay");
}

// A restore that finishes while the prune waits for it makes a copy the preview never showed. The
// prune lists the copies once it runs, so the snapshots row leaves that copy too.
#[test]
fn a_copy_made_while_the_prune_waited_stays() {
    let (_root, vault, _copies, _checkpoint) = archive_with_copies();
    let elsewhere = tempfile::tempdir_in(vault.paths().app_root.parent().unwrap()).unwrap();
    let made = elsewhere.path().join("copy");
    snapshot(&vault, &made).unwrap();
    let late = vault.paths().raw_snapshots_dir.join("maintenance/before-restore-late");
    let restoring = crate::archive::ArchiveOpGate::acquire(vault.paths());
    std::thread::scope(|scope| {
        let pruning = scope.spawn(|| run(&vault, &request(&["snapshots"], &[])));
        // The prune is waiting for the gate by now; a listing taken before it would miss the copy.
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::rename(&made, &late).unwrap();
        drop(restoring);
        pruning.join().unwrap().unwrap();
    });
    assert!(late.join("snapshot.json").is_file(), "the copy made meanwhile stays");
}

// An old-format copy is one SQLite file with the files SQLite keeps beside it. Picked, all of them
// go and the size counted them; not picked, the snapshots row leaves all of them.
#[test]
fn an_old_format_copy_goes_with_the_files_beside_it() {
    let (_root, vault, _copies, checkpoint) = archive_with_copies();
    let folder = vault.paths().raw_snapshots_dir.join("rekey");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("archive-2026-01-01.sqlite");
    std::fs::write(&file, vec![3u8; 8_192]).unwrap();
    let wal = folder.join("archive-2026-01-01.sqlite-wal");
    std::fs::write(&wal, vec![4u8; 1_000]).unwrap();
    let old = RecoverySnapshot {
        id: file.display().to_string(),
        path: file.display().to_string(),
        source_op: "rekey".into(),
        size_bytes: 8_192,
        verified_openable: true,
        ..Default::default()
    };
    let with_old = |vault: &Vault| {
        let mut copies = listed(vault);
        copies.push(old.clone());
        copies
    };
    let preview = preview_retention(vault.paths(), &with_old(&vault)).unwrap();
    let shown = preview.copies.iter().find(|copy| copy.path == old.path).unwrap().bytes;
    assert_eq!(shown, 9_192, "the file and the one beside it");

    prune(&vault, &request(&["snapshots"], &[]), || Ok(with_old(&vault)), &Default::default())
        .unwrap();
    assert!(!checkpoint.exists());
    assert!(file.is_file() && wal.is_file(), "the snapshots row left the old copy whole");

    let result =
        prune(&vault, &request(&[], &[&file]), || Ok(with_old(&vault)), &Default::default())
            .unwrap();
    assert_eq!(result.deleted_bytes, shown);
    assert!(!file.exists() && !wal.exists(), "nothing of it is left behind");
}

// Only a listed copy is deleted by path; anything else fails the whole request before it deletes.
#[test]
fn only_listed_copies_are_deleted_by_path() {
    let (_root, vault, copies, checkpoint) = archive_with_copies();
    let elsewhere = tempfile::tempdir().unwrap();
    let outside = elsewhere.path().join("not-a-copy");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep.bin"), b"the user's").unwrap();
    for path in [
        outside.as_path(),
        vault.paths().raw_snapshots_dir.as_path(),
        vault.paths().app_root.as_path(),
    ] {
        let error =
            run(&vault, &request(&["snapshots"], &[&copies[0], path])).expect_err("refused");
        assert!(format!("{error:#}").contains("copy-not-listed"), "{}: {error:#}", path.display());
    }
    assert!(outside.join("keep.bin").is_file());
    assert!(checkpoint.join("History").is_file(), "nothing of the request ran");
    for copy in &copies {
        assert!(copy.join("snapshot.json").is_file());
    }
}

// A symlink in the snapshots folder (a user moved something elsewhere and linked it) goes as a
// link: what it points to, outside the data folder, is not touched.
#[cfg(unix)]
#[test]
fn a_symlink_among_the_snapshots_is_removed_without_what_it_points_to() {
    let (_root, vault, _copies, _checkpoint) = archive_with_copies();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(elsewhere.path().join("keep.bin"), b"the user's").unwrap();
    let link = vault.paths().raw_snapshots_dir.join("chrome-Default/linked");
    std::os::unix::fs::symlink(elsewhere.path(), &link).unwrap();

    run(&vault, &request(&["snapshots"], &[])).unwrap();
    assert!(std::fs::symlink_metadata(&link).is_err(), "the link is gone");
    assert_eq!(std::fs::read(elsewhere.path().join("keep.bin")).unwrap(), b"the user's");
}
