//! Responsibilities: tell a file's at-rest mode from its header, the launch-recovery outcome
//! types the worker and the recovery screen share, and launch recovery for the native archive.
//! Not responsible for: archives of the old SQLCipher format (`vault_convert_sqlcipher` recovers
//! those before converting them), or native maintenance itself (`crate::maintenance`).
//! Failure modes: a config whose mode no longer matches the archive's header (the next open
//! asks for no key, or for one it can't use). Tests: at_rest::tests.

use crate::{
    config::{ProjectPaths, load_config, save_config},
    models::{AppConfig, ArchiveMode, RecoverySnapshot},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

/// The 16-byte magic every plaintext SQLite database file begins with. An
/// encrypted SQLCipher file starts with a random salt instead, so the presence
/// of this header is a cheap, key-free signal that a file is plaintext.
const SQLITE_FILE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// On-disk encryption state of a database file, detected without a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskEncryptionMode {
    /// File begins with the SQLite plaintext header.
    Plaintext,
    /// File exists but does not begin with the plaintext header (SQLCipher salt).
    Encrypted,
    /// File is missing or shorter than the 16-byte header (nothing to reconcile).
    Absent,
}

/// Reads the first 16 bytes to classify a file's at-rest mode without a key.
pub fn detect_disk_encryption_mode(path: &Path) -> DiskEncryptionMode {
    let mut header = [0u8; 16];
    match fs::File::open(path).and_then(|mut file| file.read_exact(&mut header)) {
        Ok(()) if &header == SQLITE_FILE_HEADER => DiskEncryptionMode::Plaintext,
        Ok(()) => DiskEncryptionMode::Encrypted,
        Err(_) => DiskEncryptionMode::Absent,
    }
}

/// Error-message prefix the worker tags a launch-recovery failure with so the FE can route it
/// to the Phase-D recovery screen instead of treating it as an opaque bootstrap error. Mirrors
/// `IMPORT_SOURCE_KEY_REQUIRED_PREFIX`; the JSON [`ArchiveRecoveryReport`] rides after the colon.
pub const ARCHIVE_RECOVERY_REQUIRED_PREFIX: &str = "archive_recovery_required";

/// Why a launch-time recovery could not safely self-heal — the Phase-D recovery-screen
/// classification. Serializable; `DiskEncryptionMode` is deliberately NOT exposed (the public
/// surface speaks [`ArchiveMode`], mapping an absent/unreadable file to `None`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveRecoveryKind {
    /// An interrupted whole-app import could not reach one consistent at-rest mode.
    InterruptedImportModeDrift,
    /// An interrupted rekey could not be resolved (e.g. the canonical archive is gone).
    InterruptedRekeyUnresolved,
    /// An interrupted full-archive restore could not be completed or rolled back (snapshot +
    /// quarantined originals both unusable).
    InterruptedRestoreUnresolved,
    /// Config↔file at-rest drift that could not be safely reconciled (reserved for Phase D;
    /// the launch heal converges every observable drift, so this is not produced today).
    AtRestDriftUnresolved,
}

/// The structured, serializable feed the worker turns into the Phase-D recovery screen when a
/// launch recovery cannot self-heal. Carries the detected modes + the verified safety snapshots
/// a one-click restore can choose from, plus the underlying error chain for the screen and logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveRecoveryReport {
    /// What kind of unrecoverable state was found.
    pub kind: ArchiveRecoveryKind,
    /// The at-rest mode `config.json` currently declares.
    pub config_mode: ArchiveMode,
    /// The canonical history-vault's real on-disk mode (`None` = absent / unreadable).
    pub history_vault_mode: Option<ArchiveMode>,
    /// The source-evidence DB's real on-disk mode (`None` = absent / unreadable).
    pub source_evidence_mode: Option<ArchiveMode>,
    /// Verified rekey safety-snapshot paths a Phase-D one-click restore can offer. Legacy
    /// rekey-only path-string list, kept for the existing wire contract.
    pub available_snapshots: Vec<String>,
    /// Rich, keyless metadata for every verified full-archive safety snapshot (rekey/reconcile/
    /// import) the Phase-D recovery screen can offer. Superset of `available_snapshots`.
    pub recovery_snapshots: Vec<RecoverySnapshot>,
    /// The underlying error chain (`format!("{err:#}")`), for the recovery screen + logs.
    pub detail: String,
}

/// Outcome of the launch-time at-rest reconcile.
///
/// Internally tagged (`outcome`) so the FE can switch on a single discriminant. `Healed` and
/// `Unrecoverable` carry the data Phase D needs; `Healthy` is the overwhelmingly common path.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum LaunchRecovery {
    /// Config already matches the canonical file's real at-rest mode, OR the archive is
    /// uninitialized (nothing on disk yet). The cheap no-op path.
    Healthy,
    /// `config.json` was corrected to the canonical files' real on-disk at-rest mode (the
    /// 2026-06-30 incident: a stale `Plaintext` config over encrypted files).
    Healed {
        /// The stale mode the config declared before the heal.
        from_mode: ArchiveMode,
        /// The real on-disk mode the config was corrected to.
        to_mode: ArchiveMode,
    },
    /// The state cannot be safely reconciled at launch — feeds the Phase-D recovery screen.
    Unrecoverable(ArchiveRecoveryReport),
}

/// Maps a key-free on-disk detection to the public [`ArchiveMode`], collapsing
/// `Absent`/unreadable to `None` so `DiskEncryptionMode` never crosses the public surface.
pub(crate) fn disk_mode_to_archive_mode(mode: DiskEncryptionMode) -> Option<ArchiveMode> {
    match mode {
        DiskEncryptionMode::Plaintext => Some(ArchiveMode::Plaintext),
        DiskEncryptionMode::Encrypted => Some(ArchiveMode::Encrypted),
        DiskEncryptionMode::Absent => None,
    }
}

/// Finishes interrupted native maintenance and makes `config.json` match the native archive's
/// real at-rest mode. Header reads only. The worker calls this when the native archive exists
/// or an install is pending; otherwise `vault_convert_sqlcipher::recover_on_launch` handles the
/// old files.
pub fn recover_archive_on_launch(
    paths: &ProjectPaths,
    config: &AppConfig,
) -> Result<LaunchRecovery> {
    crate::maintenance::recover_native_maintenance(paths)?;
    let mode = detect_disk_encryption_mode(&paths.archive_db);
    if let Some(mode) = disk_mode_to_archive_mode(mode) {
        let mut current = load_config(paths).unwrap_or_else(|_| config.clone());
        if current.archive_mode != mode {
            let from_mode = current.archive_mode;
            current.archive_mode = mode.clone();
            save_config(paths, &current)?;
            return Ok(LaunchRecovery::Healed { from_mode, to_mode: mode });
        }
    }
    Ok(LaunchRecovery::Healthy)
}

/// The cross-file invariant from quality-matrix rule 2: the at-rest mode `config.json` records
/// (read through the real `load_config`) equals the header of every persistent native database
/// that exists. Header reads only. A test post-condition for every archive mutation; the
/// 2026-06-30 incident was exactly this invariant broken (encrypted files, Plaintext config).
#[cfg(test)]
pub(crate) fn check_config_disk_consistency(paths: &ProjectPaths) -> Result<()> {
    let expected = load_config(paths)?.archive_mode;
    let files = vault_store::Paths::under(&paths.app_root);
    for db in vault_store::Db::ALL.into_iter().filter(|db| *db != vault_store::Db::Scratch) {
        let path = files.get(db);
        let Some(on_disk) = disk_mode_to_archive_mode(detect_disk_encryption_mode(path)) else {
            continue;
        };
        anyhow::ensure!(
            on_disk == expected,
            "config\u{2194}disk at-rest invariant violated: config.json declares {expected:?} but \
             {db:?} ({}) is {on_disk:?}",
            path.display(),
        );
    }
    Ok(())
}

/// Outcome of reconciling config to the archive's real at-rest mode, surfaced to the UI so a
/// silent self-heal can still be reported honestly.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReport {
    /// True when the recorded mode had to change to match the files.
    pub repaired: bool,
    /// The drifted on-disk mode that was found (`None` when nothing was repaired).
    pub from_mode: Option<ArchiveMode>,
    /// The mode config and files now agree on.
    pub to_mode: ArchiveMode,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detect_disk_encryption_mode_classifies_each_state() {
        let dir = tempdir().expect("tempdir");
        let plaintext = dir.path().join("plain.sqlite");
        fs::write(&plaintext, b"SQLite format 3\0and then some").expect("write plain");
        assert_eq!(detect_disk_encryption_mode(&plaintext), DiskEncryptionMode::Plaintext);

        let encrypted = dir.path().join("enc.sqlite");
        fs::write(&encrypted, [0u8; 32]).expect("write enc");
        assert_eq!(detect_disk_encryption_mode(&encrypted), DiskEncryptionMode::Encrypted);

        let absent = dir.path().join("missing.sqlite");
        assert_eq!(detect_disk_encryption_mode(&absent), DiskEncryptionMode::Absent);

        let too_short = dir.path().join("short.sqlite");
        fs::write(&too_short, b"SQLite").expect("write short");
        assert_eq!(detect_disk_encryption_mode(&too_short), DiskEncryptionMode::Absent);
    }
}
