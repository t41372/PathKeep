use super::ArchiveMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyringStatusReport {
    pub available: bool,
    pub backend: String,
    pub stored_secret: bool,
    pub message: Option<String>,
}
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
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RekeyPreview {
    pub current_mode: ArchiveMode,
    pub next_mode: ArchiveMode,
    pub requires_new_key: bool,
    pub snapshot_path: String,
    pub temp_database_path: String,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}
