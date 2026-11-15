//! Responsibilities: prove that a restore brings back browsing data and carries this archive's
//! agent conversations over: where they are under the same key (a password change since the copy
//! included), converted to the copy's key after a mode change, read from their own stores when no
//! archive is open, and refused with nothing changed when their key is needed and not given.
//! Not responsible for: the snapshot format (`maintenance::tests`) or bringing an archive up to
//! this build's layout (`creation::tests`).
//! Failure modes: a restore rolls the conversations back or empties them; a password change since
//! the copy forces a needless conversion; a mode change since leaves them under a key the restored
//! archive does not have; a restore of an archive that does not open is refused for want of a
//! conversion it cannot do, or does it with the wrong key.
use super::creation::tests::{ADDED, files, marker, open, released_archive, write_marker};
use super::snapshot::{restore_snapshot, restore_snapshot_to, snapshot};
use crate::{ArchiveMode, ProjectPaths, Vault};
use vault_store::Db;

fn inode(paths: &ProjectPaths, relative: &str) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(paths.app_root.join(relative)).unwrap().ino()
}

fn plaintext_on_disk(paths: &ProjectPaths, relative: &str) -> bool {
    crate::detect_disk_encryption_mode(&paths.app_root.join(relative))
        == crate::DiskEncryptionMode::Plaintext
}

fn save_conversations(vault: &Vault, evidence: &'static str, read_model: &'static str) {
    write_marker(vault.store(), Db::AgentHistoryEvidence, evidence);
    write_marker(vault.store(), Db::AgentHistoryReadModel, read_model);
}

fn conversations(vault: &Vault) -> (Option<String>, Option<String>) {
    (marker(vault, Db::AgentHistoryEvidence), marker(vault, Db::AgentHistoryReadModel))
}

fn saved(evidence: &str, read_model: &str) -> (Option<String>, Option<String>) {
    (Some(evidence.into()), Some(read_model.into()))
}

#[test]
fn a_restore_brings_browsing_back_and_leaves_the_conversations_where_they_are() {
    let (_root, paths) = released_archive();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("copy");
    {
        let vault = open(&paths).unwrap();
        save_conversations(&vault, "in-the-copy", "in-the-copy");
        snapshot(&vault, &copy).unwrap();
        vault.close().unwrap();
    }
    let live = open(&paths).unwrap();
    write_marker(live.store(), Db::Archive, "changed-since");
    save_conversations(&live, "saved-since", "read-since");
    let before = ADDED.map(|added| inode(&paths, added));

    let installed = restore_snapshot(&live, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&installed.vault, Db::Archive).as_deref(), Some("released-archive"));
    assert_eq!(conversations(&installed.vault), saved("saved-since", "read-since"));
    assert_eq!(ADDED.map(|added| inode(&paths, added)), before, "the same files, not copies");
    for added in ADDED {
        assert!(!installed.previous_files.join(added).exists(), "{added} was not moved aside");
    }
}

// A password change rewraps the same key: the copy's key file differs, its key does not.
#[test]
fn a_password_change_since_the_copy_converts_nothing() {
    let (_root, vault, _) = super::tests::encrypted();
    let paths = vault.paths().clone();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("copy");
    snapshot(&vault, &copy).unwrap();
    super::rewrap_password(&vault, "old password", "new password").unwrap();
    save_conversations(&vault, "saved-since", "read-since");
    let before = ADDED.map(|added| inode(&paths, added));

    let installed =
        restore_snapshot(&vault, &copy, Some("old password"), Default::default()).unwrap();
    assert_eq!(conversations(&installed.vault), saved("saved-since", "read-since"));
    assert_eq!(ADDED.map(|added| inode(&paths, added)), before, "not converted");
}

// A mode change makes a new key: the conversations are copied under the copy's, the old files
// are moved aside with the rest of what the restore replaced.
#[test]
fn a_mode_change_since_the_copy_converts_the_conversations_to_its_key() {
    let (_root, paths) = released_archive();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("plaintext-copy");
    {
        let vault = open(&paths).unwrap();
        snapshot(&vault, &copy).unwrap();
        vault.close().unwrap();
    }
    let vault = open(&paths).unwrap();
    let switched = super::switch_mode(
        &vault,
        None,
        ArchiveMode::Encrypted,
        Some("new password"),
        Default::default(),
        |_| {},
    )
    .unwrap();
    save_conversations(&switched.vault, "saved-encrypted", "read-encrypted");
    for added in ADDED {
        assert!(!plaintext_on_disk(&paths, added), "{added} is under the new key");
    }

    let installed = restore_snapshot(&switched.vault, &copy, None, Default::default()).unwrap();
    assert_eq!(crate::load_config(&paths).unwrap().archive_mode, ArchiveMode::Plaintext);
    assert_eq!(conversations(&installed.vault), saved("saved-encrypted", "read-encrypted"));
    for added in ADDED {
        assert!(plaintext_on_disk(&paths, added), "{added} is under the copy's key");
        assert!(installed.previous_files.join(added).is_file(), "{added} under the old key");
    }
}

// The archive may not open at all (that is when a restore is needed most). The conversations are
// then read from their own stores, with the key of the current password; without it nothing moves.
#[test]
fn without_an_open_archive_the_conversations_are_carried_with_its_key_or_not_at_all() {
    let (root, paths) = released_archive();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("plaintext-copy");
    {
        let vault = open(&paths).unwrap();
        snapshot(&vault, &copy).unwrap();
        vault.close().unwrap();
    }
    {
        let vault = open(&paths).unwrap();
        let switched = super::switch_mode(
            &vault,
            None,
            ArchiveMode::Encrypted,
            Some("new password"),
            Default::default(),
            |_| {},
        )
        .unwrap();
        save_conversations(&switched.vault, "saved-encrypted", "read-encrypted");
        switched.vault.close().unwrap();
    }

    let before = files(root.path());
    let Err(error) = restore_snapshot_to(&paths, None, None, &copy, None, None, Default::default())
    else {
        panic!("restored without the conversations' key")
    };
    assert!(format!("{error:#}").contains("agent-history-key-required"), "{error:#}");
    assert_eq!(files(root.path()), before, "nothing was changed");

    let key = Vault::unlock_data_key(&paths, "new password").unwrap();
    let installed =
        restore_snapshot_to(&paths, None, None, &copy, None, Some(&key), Default::default())
            .unwrap();
    assert_eq!(conversations(&installed.vault), saved("saved-encrypted", "read-encrypted"));
    for added in ADDED {
        assert!(plaintext_on_disk(&paths, added), "{added} is under the copy's key");
    }
}
