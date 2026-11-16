//! Responsibilities: prove that a restore brings back browsing data under the archive's own
//! encryption and password and leaves its agent conversations where they are: a copy under the
//! same key installs as it is; a copy from before encryption was turned on, or under the key of
//! before a mode change, is written under the archive's key; an encrypted copy restored into an
//! archive that is now unencrypted is written unencrypted; with no archive open, an encrypted
//! archive's key is needed, and without it nothing changes.
//! Not responsible for: the snapshot format (`maintenance::tests`), bringing an archive up to this
//! build's layout (`creation::tests`), the copy of what a restore replaced (`safety_copy_tests`).
//! Failure modes: a restore turns encryption off and writes the conversations unencrypted; puts
//! back a password changed since; converts, moves or empties the conversations; leaves files under
//! two keys, or the copy's key file over the archive's; is refused for a key it does not need, or
//! goes ahead without one it does.
use super::creation::tests::{ADDED, files, marker, open, released_archive, write_marker};
use super::snapshot::{restore_snapshot, restore_snapshot_to, snapshot};
use crate::{ArchiveMode, ProjectPaths, Vault};
use vault_store::Db;

/// The browsing stores, which a restore brings back.
const BROWSING: [&str; 4] = [
    "archive/history-vault.db",
    "archive/source-evidence.db",
    "derived/intelligence.db",
    "derived/agent.db",
];

/// The number the file system gives the file at `relative`: the same before and after means the
/// same file, not a copy put in its place. The inode on Unix; on Windows the file index, not the
/// creation time, which NTFS gives a file made under a name another left moments before.
fn file_number(paths: &ProjectPaths, relative: &str) -> u64 {
    let path = paths.app_root.join(relative);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path).unwrap().ino()
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let file = std::fs::File::open(path).unwrap();
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: the handle stays open for the call, which fills the structure it is given.
        let found = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
        assert_ne!(found, 0, "{}", std::io::Error::last_os_error());
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)
    }
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

fn switch(
    vault: &Vault,
    current: Option<&str>,
    mode: ArchiveMode,
    password: Option<&str>,
) -> Vault {
    super::switch_mode(vault, current, mode, password, Default::default(), |_| {}).unwrap().vault
}

/// The archive is encrypted, `password` unlocks its key, and every store opens under that key.
fn opens_with(paths: &ProjectPaths, password: &str) {
    let config = crate::load_config(paths).unwrap();
    assert_eq!(config.archive_mode, ArchiveMode::Encrypted);
    let (vault, _) = Vault::open_password(paths.clone(), config, password, None)
        .expect("opens with the archive's password");
    vault.close().unwrap();
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
    let before = ADDED.map(|added| file_number(&paths, added));

    let installed = restore_snapshot(&live, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&installed.vault, Db::Archive).as_deref(), Some("released-archive"));
    assert_eq!(conversations(&installed.vault), saved("saved-since", "read-since"));
    assert_eq!(ADDED.map(|added| file_number(&paths, added)), before, "the same files, not copies");
    for added in ADDED {
        assert!(!installed.previous_files.join(added).exists(), "{added} was not moved aside");
    }
}

// A password change rewraps the same key: the copy installs as it is, under the password the
// archive has now, not the one it had when the copy was made.
#[test]
fn a_password_changed_since_the_copy_stays() {
    let (_root, vault, _) = super::tests::encrypted();
    let paths = vault.paths().clone();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("copy");
    write_marker(vault.store(), Db::Archive, "in-the-copy");
    snapshot(&vault, &copy).unwrap();
    super::rewrap_password(&vault, "old password", "new password").unwrap();
    write_marker(vault.store(), Db::Archive, "changed-since");
    save_conversations(&vault, "saved-since", "read-since");
    let envelope = std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap();
    let before = ADDED.map(|added| file_number(&paths, added));

    let installed =
        restore_snapshot(&vault, &copy, Some("old password"), Default::default()).unwrap();
    assert_eq!(marker(&installed.vault, Db::Archive).as_deref(), Some("in-the-copy"));
    assert_eq!(conversations(&installed.vault), saved("saved-since", "read-since"));
    assert_eq!(ADDED.map(|added| file_number(&paths, added)), before, "not converted");
    installed.vault.close().unwrap();
    assert_eq!(std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap(), envelope);
    assert!(Vault::unlock_data_key(&paths, "old password").is_err(), "the old password is back");
    opens_with(&paths, "new password");
}

// The case the user decided on: a copy from before encryption was turned on must not turn it off
// and write everything, the conversations included, unencrypted.
#[test]
fn a_copy_from_before_encryption_is_restored_under_the_current_key_and_password() {
    let (_root, paths) = released_archive();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("plaintext-copy");
    {
        let vault = open(&paths).unwrap();
        snapshot(&vault, &copy).unwrap();
        vault.close().unwrap();
    }
    let vault = open(&paths).unwrap();
    let encrypted = switch(&vault, None, ArchiveMode::Encrypted, Some("new password"));
    write_marker(encrypted.store(), Db::Archive, "changed-since");
    save_conversations(&encrypted, "saved-encrypted", "read-encrypted");
    let before = ADDED.map(|added| file_number(&paths, added));

    let installed = restore_snapshot(&encrypted, &copy, None, Default::default()).unwrap();
    assert_eq!(marker(&installed.vault, Db::Archive).as_deref(), Some("released-archive"));
    assert_eq!(conversations(&installed.vault), saved("saved-encrypted", "read-encrypted"));
    assert_eq!(ADDED.map(|added| file_number(&paths, added)), before, "the conversations stay put");
    installed.vault.close().unwrap();
    for file in BROWSING.into_iter().chain(ADDED) {
        assert!(!plaintext_on_disk(&paths, file), "{file} is unencrypted");
    }
    opens_with(&paths, "new password");
}

// An archive whose encryption the user turned off stays that way; its key file is not put back.
#[test]
fn an_encrypted_copy_restored_into_an_unencrypted_archive_stays_unencrypted() {
    let (_root, vault, _) = super::tests::encrypted();
    let paths = vault.paths().clone();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("encrypted-copy");
    write_marker(vault.store(), Db::Archive, "in-the-copy");
    snapshot(&vault, &copy).unwrap();
    let plain = switch(&vault, Some("old password"), ArchiveMode::Plaintext, None);
    save_conversations(&plain, "saved-plain", "read-plain");

    let installed =
        restore_snapshot(&plain, &copy, Some("old password"), Default::default()).unwrap();
    assert_eq!(marker(&installed.vault, Db::Archive).as_deref(), Some("in-the-copy"));
    assert_eq!(conversations(&installed.vault), saved("saved-plain", "read-plain"));
    assert!(installed.data_key.is_none());
    installed.vault.close().unwrap();
    assert_eq!(crate::load_config(&paths).unwrap().archive_mode, ArchiveMode::Plaintext);
    assert!(
        !paths.app_root.join("archive/keys.envelope").exists(),
        "the copy's key file came back"
    );
    for file in BROWSING.into_iter().chain(ADDED) {
        assert!(plaintext_on_disk(&paths, file), "{file} is encrypted");
    }
}

// Two mode changes since the copy: it is under a key the archive no longer has, and is rewritten
// under the current one.
#[test]
fn a_copy_under_an_older_key_is_rewritten_under_the_current_one() {
    let (_root, vault, _) = super::tests::encrypted();
    let paths = vault.paths().clone();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("old-key-copy");
    write_marker(vault.store(), Db::Archive, "in-the-copy");
    snapshot(&vault, &copy).unwrap();
    let plain = switch(&vault, Some("old password"), ArchiveMode::Plaintext, None);
    let newest = switch(&plain, None, ArchiveMode::Encrypted, Some("newest password"));

    let installed =
        restore_snapshot(&newest, &copy, Some("old password"), Default::default()).unwrap();
    assert_eq!(marker(&installed.vault, Db::Archive).as_deref(), Some("in-the-copy"));
    installed.vault.close().unwrap();
    assert!(Vault::unlock_data_key(&paths, "old password").is_err());
    opens_with(&paths, "newest password");
}

// The archive may not open at all (that is when a restore is needed most). An encrypted one's key
// is then given by its current password; without it nothing changes.
#[test]
fn without_an_open_archive_an_encrypted_one_needs_its_key_or_nothing_changes() {
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
        let encrypted = switch(&vault, None, ArchiveMode::Encrypted, Some("new password"));
        save_conversations(&encrypted, "saved-encrypted", "read-encrypted");
        encrypted.close().unwrap();
    }

    let before = files(root.path());
    let Err(error) = restore_snapshot_to(&paths, None, None, &copy, None, None, Default::default())
    else {
        panic!("restored without the archive's key")
    };
    assert!(format!("{error:#}").contains("archive-current-password-required"), "{error:#}");
    assert_eq!(files(root.path()), before, "nothing was changed");

    let numbers = ADDED.map(|added| file_number(&paths, added));
    let key = Vault::unlock_data_key(&paths, "new password").unwrap();
    let installed =
        restore_snapshot_to(&paths, None, None, &copy, None, Some(&key), Default::default())
            .unwrap();
    assert_eq!(conversations(&installed.vault), saved("saved-encrypted", "read-encrypted"));
    assert_eq!(ADDED.map(|added| file_number(&paths, added)), numbers);
    installed.vault.close().unwrap();
    opens_with(&paths, "new password");
}

// What belongs to this computer is not the copy's to bring back, as with an import: a restore must
// not turn App Lock off, nor store the key in a keychain the user stopped using, or clear it.
#[test]
fn a_restore_keeps_this_computers_app_lock_and_keychain_choice() {
    let (_root, paths) = released_archive();
    let copies = tempfile::tempdir().unwrap();
    let copy = copies.path().join("copy");
    {
        let vault = open(&paths).unwrap();
        snapshot(&vault, &copy).unwrap();
        vault.close().unwrap();
    }
    let live = open(&paths).unwrap();
    let mut here = crate::load_config(&paths).unwrap();
    assert!(!here.app_lock.enabled && !here.remember_database_key_in_keyring);
    here.app_lock.enabled = true;
    here.app_lock.idle_timeout_minutes = 7;
    here.remember_database_key_in_keyring = true;
    crate::save_config(&crate::ControlGate::acquire(&paths), &here).unwrap();

    let installed = restore_snapshot(&live, &copy, None, Default::default()).unwrap();
    for config in [installed.vault.config().clone(), crate::load_config(&paths).unwrap()] {
        assert!(config.app_lock.enabled);
        assert_eq!(config.app_lock.idle_timeout_minutes, 7);
        assert!(config.remember_database_key_in_keyring);
    }
}
