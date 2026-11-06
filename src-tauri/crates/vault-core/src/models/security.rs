//! Security and keyring read models.

use super::{AppSnapshot, ArchiveMode};
use serde::{Deserialize, Serialize};

/// What an archive encryption change did. Once the archive has changed the command no longer
/// fails, so what could not follow it is said here instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RekeyOutcome {
    /// `None` when App Lock locked the window before the change finished: the snapshot is not
    /// read while it is locked.
    pub snapshot: Option<AppSnapshot>,
    /// The keychain could not be set to match the archive, so the setting to remember its key
    /// stays as it was.
    pub keychain_failed: bool,
}

/// Snapshot of keyring availability and whether a database key is stored.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyringStatusReport {
    pub available: bool,
    pub backend: String,
    pub stored_secret: bool,
    pub message: Option<String>,
}

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
    /// The Security route localizes rekey preview warnings off these codes
    /// instead of matching backend English sentences. Entries are `""` for
    /// opaque pass-throughs, telling the shell to render the diagnostic prose
    /// verbatim.
    #[serde(default)]
    pub warning_codes: Vec<String>,
}

/// Stable code for "an encrypted rekey still needs its new database key".
pub const REKEY_WARNING_NEW_KEY_REQUIRED: &str = "new-key-required";

/// Stable code for "target mode equals current mode — the rewrite is a key
/// rotation / validation pass rather than a mode switch".
pub const REKEY_WARNING_SAME_MODE_REWRITE: &str = "same-mode-rewrite";
