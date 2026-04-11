//! Remote-backup configuration and verification models.

use serde::{Deserialize, Serialize};

/// Persisted remote-backup settings stored inside app config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteBackupConfig {
    pub enabled: bool,
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub prefix: String,
    pub path_style: bool,
    pub upload_after_backup: bool,
    pub credentials_saved: bool,
    pub last_uploaded_at: Option<String>,
    pub last_uploaded_object_key: Option<String>,
    pub last_error: Option<String>,
}

impl Default for RemoteBackupConfig {
    /// Returns the accepted defaults for remote-backup settings.
    fn default() -> Self {
        Self {
            enabled: false,
            bucket: String::new(),
            region: "us-east-1".to_string(),
            endpoint: None,
            prefix: "pathkeep".to_string(),
            path_style: true,
            upload_after_backup: false,
            credentials_saved: false,
            last_uploaded_at: None,
            last_uploaded_object_key: None,
            last_error: None,
        }
    }
}

/// Credentials payload used when storing S3-compatible secrets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct S3CredentialInput {
    pub access_key_id: String,
    pub secret_access_key: String,
}

/// Preview payload for a remote-backup upload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupPreview {
    pub bundle_path: String,
    pub object_key: String,
    pub upload_url: String,
    pub preview_command: String,
    pub manual_steps: Vec<String>,
    pub warnings: Vec<String>,
}

/// Result payload for a remote-backup upload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupResult {
    pub uploaded: bool,
    pub bundle_path: String,
    pub object_key: String,
    pub upload_url: String,
    pub message: String,
}

/// One verification check performed against a remote-backup bundle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupVerificationCheck {
    pub name: String,
    pub status: String,
    pub message: String,
}

/// One manifest file entry found inside a remote-backup bundle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupVerificationFile {
    pub relative_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

/// Full verification report for a remote-backup bundle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupVerification {
    pub bundle_path: String,
    pub bundle_version: String,
    pub app_version: String,
    pub created_at: String,
    pub archive_mode: String,
    pub object_key: String,
    pub restore_ready: bool,
    pub checks: Vec<RemoteBackupVerificationCheck>,
    pub warnings: Vec<String>,
    pub restore_steps: Vec<String>,
    pub manifest_files: Vec<RemoteBackupVerificationFile>,
}
