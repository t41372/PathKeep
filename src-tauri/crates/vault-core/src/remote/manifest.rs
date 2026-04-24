//! Remote-backup manifest schema and conversion helpers.
//!
//! The manifest is the on-disk contract that lets a downloaded bundle prove
//! what it contains before PathKeep attempts restore validation.
//!
//! ## Responsibilities
//! - Define the current bundle version string and required restore entries.
//! - Serialize and deserialize manifest files using the stable camelCase shape.
//! - Convert manifest file rows into verification DTO rows without leaking
//!   internal bundle structs across the rest of the crate.
//!
//! ## Not responsible for
//! - Building zip entries or computing file hashes.
//! - Deciding whether a bundle can be restored.
//! - Upload URL generation.
//!
//! ## Dependencies
//! - `serde` preserves the manifest JSON contract.
//! - `models::RemoteBackupVerificationFile` is the public DTO family consumed
//!   by verification callers.
//!
//! ## Performance notes
//! Manifest rows are small metadata records. Large bundle payloads remain in
//! `bundle` and `verify`, where they are streamed instead of buffered.

use crate::models::RemoteBackupVerificationFile;
use serde::{Deserialize, Serialize};

pub(super) const REMOTE_BUNDLE_VERSION: &str = "pathkeep.remote-backup.v1";
pub(super) const REQUIRED_BUNDLE_ENTRIES: &[&str] = &[
    "archive/history-vault.sqlite",
    "archive/source-evidence.sqlite",
    "config/config.json",
    "metadata/bundle-manifest.json",
    "metadata/bundle-manifest.sha256",
];

/// Records the bundle-level identity and every payload entry hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BundleManifest {
    pub(super) bundle_version: String,
    pub(super) app_version: String,
    pub(super) created_at: String,
    pub(super) archive_mode: String,
    pub(super) bucket: String,
    pub(super) object_key: String,
    pub(super) files: Vec<BundleManifestFile>,
}

/// Describes one manifest-tracked file inside a remote-backup bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BundleManifestFile {
    pub(super) relative_path: String,
    pub(super) sha256: String,
    pub(super) size_bytes: u64,
}

/// Keeps public verification rows derived from the exact manifest entries.
impl BundleManifest {
    /// Returns frontend-facing file rows without exposing internal manifest
    /// ownership. The conversion is O(n) in manifest rows and does not touch
    /// the zip payload bytes.
    pub(super) fn verification_files(&self) -> Vec<RemoteBackupVerificationFile> {
        self.files
            .iter()
            .cloned()
            .map(|file| RemoteBackupVerificationFile {
                relative_path: file.relative_path,
                sha256: file.sha256,
                size_bytes: file.size_bytes,
            })
            .collect()
    }
}
