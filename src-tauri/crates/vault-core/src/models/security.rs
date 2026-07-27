//! Security and keyring read models.

use super::ArchiveMode;
use serde::{Deserialize, Serialize};

/// Snapshot of keyring availability and whether a database key is stored.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyringStatusReport {
    pub available: bool,
    pub backend: String,
    pub stored_secret: bool,
    pub message: Option<String>,
}

/// Combined security-status payload returned to the shell.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SecurityStatus {
    pub initialized: bool,
    pub mode: String,
    pub encrypted: bool,
    pub unlocked: bool,
    pub database_path: String,
    pub stronghold_path: String,
    pub remember_database_key_in_keyring: bool,
    pub last_successful_backup_at: Option<String>,
    pub last_rekey_at: Option<String>,
    pub last_rekey_run_id: Option<i64>,
    pub last_rekey_snapshot_path: Option<String>,
    pub keyring_status: KeyringStatusReport,
    /// Diagnostic English prose kept only as the last-resort fallback when a
    /// warning carries no code this build knows how to localize.
    pub warnings: Vec<String>,
    /// Stable warning codes aligned index-for-index with [`Self::warnings`].
    ///
    /// This exists so the Security route never localizes by matching backend
    /// English sentences. Entries are `""` when the warning is an opaque
    /// pass-through (for example an arbitrary archive-open failure), which tells
    /// the shell to render the diagnostic prose verbatim instead of guessing.
    #[serde(default)]
    pub warning_codes: Vec<String>,
}

/// Stable code for "this archive is encrypted and still needs its password".
pub const SECURITY_WARNING_ENCRYPTED_NEEDS_PASSWORD: &str = "encrypted-needs-password";

/// Stable code for "remember-the-key is on but the host has no keyring".
pub const SECURITY_WARNING_REMEMBER_KEY_NO_KEYRING: &str = "remember-key-no-keyring";

/// Stable code for "remember-the-key is on but no secret is stored yet".
pub const SECURITY_WARNING_REMEMBERED_KEY_MISSING: &str = "remembered-key-missing";

/// Preview payload for an archive rekey/mode-switch operation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RekeyPreview {
    pub current_mode: ArchiveMode,
    pub next_mode: ArchiveMode,
    pub requires_new_key: bool,
    pub snapshot_path: String,
    pub temp_database_path: String,
    pub steps: Vec<String>,
    /// Diagnostic English prose kept only as the last-resort fallback when a
    /// warning carries no code this build knows how to localize.
    pub warnings: Vec<String>,
    /// Stable warning codes aligned index-for-index with [`Self::warnings`].
    ///
    /// Mirrors [`SecurityStatus::warning_codes`]: the Security route localizes
    /// rekey preview warnings off these codes instead of matching backend
    /// English sentences. Entries are `""` for opaque pass-throughs, telling
    /// the shell to render the diagnostic prose verbatim.
    #[serde(default)]
    pub warning_codes: Vec<String>,
}

/// Stable code for "the archive is locked; unlock before executing the rekey".
pub const REKEY_WARNING_ARCHIVE_LOCKED: &str = "archive-locked";

/// Stable code for "an encrypted rekey still needs its new database key".
pub const REKEY_WARNING_NEW_KEY_REQUIRED: &str = "new-key-required";

/// Stable code for "target mode equals current mode — the rewrite is a key
/// rotation / validation pass rather than a mode switch".
pub const REKEY_WARNING_SAME_MODE_REWRITE: &str = "same-mode-rewrite";
