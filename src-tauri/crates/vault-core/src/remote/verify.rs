//! Remote-backup bundle verification.
//!
//! Verification turns a downloaded bundle into a structured readiness report
//! before any restore flow is allowed to trust the payload.
//!
//! ## Responsibilities
//! - Read and parse the bundle manifest.
//! - Check the bundle version and required restore entries.
//! - Verify detached manifest integrity and manifest-tracked file hashes.
//! - Open bundled SQLite databases in a temporary directory to prove restore
//!   readiness.
//!
//! ## Not responsible for
//! - Building or uploading bundles.
//! - Replacing the live archive with downloaded payloads.
//! - Recovering a missing or incorrect encrypted archive key.
//!
//! ## Dependencies
//! - `manifest` supplies the stable bundle schema and required entry list.
//! - `archive::apply_cipher_key` validates encrypted SQLite payloads.
//! - `models::RemoteBackupVerification` is the frontend-facing report contract.
//!
//! ## Performance notes
//! Manifest JSON is read into memory because it is intentionally small. SQLite
//! bundle entries are hashed and extracted through fixed-size buffers so large
//! archives do not require full-file buffering during verification.

use super::manifest::{BundleManifest, REMOTE_BUNDLE_VERSION, REQUIRED_BUNDLE_ENTRIES};
use crate::{
    archive::apply_cipher_key,
    models::{AppConfig, RemoteBackupVerification, RemoteBackupVerificationCheck},
    utils::sha256_hex,
};
use anyhow::{Context, Result};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use tempfile::tempdir;
use zip::ZipArchive;

const ZIP_READ_BUFFER_BYTES: usize = 64 * 1024;

/// Verifies a remote-backup bundle's integrity and restore readiness.
///
/// `key` is only required for encrypted bundles. The returned report separates
/// version, required-entry, checksum, and restore-open checks so the UI can show
/// exactly which part failed without changing the bundle contract.
pub fn verify_remote_backup(
    bundle_path: &Path,
    key: Option<&str>,
) -> Result<RemoteBackupVerification> {
    let file =
        File::open(bundle_path).with_context(|| format!("opening {}", bundle_path.display()))?;
    let mut archive = ZipArchive::new(file).context("opening remote backup bundle")?;
    let manifest_bytes = read_zip_entry(&mut archive, "metadata/bundle-manifest.json")?;
    let manifest = serde_json::from_slice::<BundleManifest>(&manifest_bytes)
        .context("parsing bundle manifest json")?;

    let version_supported = manifest.bundle_version == REMOTE_BUNDLE_VERSION;
    let required_entries_ready = required_bundle_entries_present(&mut archive);
    let (checksum_ok, checksum_message) =
        match verify_bundle_checksums(&mut archive, &manifest, &manifest_bytes) {
            Ok(result) => result,
            Err(error) => (false, format!("Checksum verification failed: {error:#}")),
        };
    let (restore_ready, restore_message, mut warnings) =
        match validate_restore_readiness(&mut archive, &manifest, key) {
            Ok(result) => result,
            Err(error) => (false, format!("Restore validation failed: {error:#}"), Vec::new()),
        };

    if manifest.archive_mode == "plaintext" {
        warnings.push(
            "The archive inside this bundle is plaintext at rest. Restore only onto a trusted local disk."
                .to_string(),
        );
    }

    Ok(RemoteBackupVerification {
        bundle_path: bundle_path.display().to_string(),
        bundle_version: manifest.bundle_version.clone(),
        app_version: manifest.app_version.clone(),
        created_at: manifest.created_at.clone(),
        archive_mode: manifest.archive_mode.clone(),
        object_key: manifest.object_key.clone(),
        restore_ready: version_supported && required_entries_ready && checksum_ok && restore_ready,
        checks: vec![
            RemoteBackupVerificationCheck {
                name: "bundle-version".to_string(),
                status: if version_supported { "ok" } else { "error" }.to_string(),
                message: if version_supported {
                    format!(
                        "Bundle version {} is supported by this PathKeep build.",
                        manifest.bundle_version
                    )
                } else {
                    format!(
                        "Bundle version {} is not supported by this PathKeep build (expected {}).",
                        manifest.bundle_version, REMOTE_BUNDLE_VERSION
                    )
                },
            },
            RemoteBackupVerificationCheck {
                name: "required-entries".to_string(),
                status: if required_entries_ready {
                    "ok"
                } else {
                    "error"
                }
                .to_string(),
                message: if required_entries_ready {
                    "Archive, config, and manifest entries are all present in the bundle."
                        .to_string()
                } else {
                    "One or more required restore entries are missing from the bundle.".to_string()
                },
            },
            RemoteBackupVerificationCheck {
                name: "checksums".to_string(),
                status: if checksum_ok { "ok" } else { "error" }.to_string(),
                message: checksum_message,
            },
            RemoteBackupVerificationCheck {
                name: "restore-validation".to_string(),
                status: if restore_ready { "ok" } else { "warning" }.to_string(),
                message: restore_message,
            },
        ],
        warnings,
        restore_steps: vec![
            "Download the bundle to a local disk before attempting restore.".to_string(),
            "Verify the manifest, checksums, and required entries before replacing a live archive."
                .to_string(),
            "If the archive is encrypted, unlock PathKeep with the matching database key before restore."
                .to_string(),
        ],
        manifest_files: manifest.verification_files(),
    })
}

/// Reads a small metadata entry from the zip into memory.
///
/// This helper is intentionally limited to manifest and config-style payloads.
/// Database entries use streaming helpers below so verification remains bounded
/// on large archives.
fn read_zip_entry(archive: &mut ZipArchive<File>, entry_name: &str) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(entry_name)
        .with_context(|| format!("reading {entry_name} from remote backup bundle"))?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Checks that every restore-critical path exists in the zip container.
fn required_bundle_entries_present(archive: &mut ZipArchive<File>) -> bool {
    REQUIRED_BUNDLE_ENTRIES.iter().all(|entry_name| archive.by_name(entry_name).is_ok())
}

/// Verifies detached manifest integrity, entry set drift, and payload hashes.
///
/// Payload checksum reads are streaming, which keeps the memory profile bounded
/// even when the archive database dominates the bundle.
fn verify_bundle_checksums(
    archive: &mut ZipArchive<File>,
    manifest: &BundleManifest,
    manifest_bytes: &[u8],
) -> Result<(bool, String)> {
    let expected_manifest_hash =
        String::from_utf8(read_zip_entry(archive, "metadata/bundle-manifest.sha256")?)
            .context("reading detached manifest checksum")?;
    let actual_manifest_hash = sha256_hex(manifest_bytes);
    let normalized_manifest_hash = expected_manifest_hash.trim();
    if normalized_manifest_hash != actual_manifest_hash {
        return Ok((
            false,
            format!(
                "metadata/bundle-manifest.json checksum drifted (expected {}, saw {})",
                normalized_manifest_hash, actual_manifest_hash
            ),
        ));
    }

    let mut actual_entries = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .map(|entry| entry.name().replace('\\', "/"))
                .map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    actual_entries.sort();

    let mut expected_entries =
        manifest.files.iter().map(|file| file.relative_path.clone()).collect::<Vec<_>>();
    expected_entries.extend([
        "metadata/bundle-manifest.json".to_string(),
        "metadata/bundle-manifest.sha256".to_string(),
    ]);
    expected_entries.sort();

    if actual_entries != expected_entries {
        return Ok((
            false,
            format!(
                "bundle entry set drifted (expected [{}], saw [{}])",
                expected_entries.join(", "),
                actual_entries.join(", ")
            ),
        ));
    }

    let mut mismatches = Vec::new();
    for file in &manifest.files {
        let (actual_sha256, actual_size) =
            match zip_entry_digest_and_size(archive, &file.relative_path) {
                Ok(result) => result,
                Err(_) => {
                    mismatches
                        .push(format!("{} is missing from the zip payload", file.relative_path));
                    continue;
                }
            };
        if actual_sha256 != file.sha256 || actual_size != file.size_bytes {
            mismatches.push(format!(
                "{} checksum or size drifted (expected {} / {}, saw {} / {})",
                file.relative_path, file.sha256, file.size_bytes, actual_sha256, actual_size
            ));
        }
    }

    if mismatches.is_empty() {
        Ok((
            true,
            format!(
                "Verified {} manifest-tracked bundle entries plus detached manifest integrity.",
                manifest.files.len()
            ),
        ))
    } else {
        Ok((false, mismatches.join(" | ")))
    }
}

/// Confirms bundled config and SQLite payloads can be read for restore.
///
/// The SQLite files are extracted to a temp directory by stream copy before
/// opening them. Encrypted bundles must receive the same key used by the live
/// archive, otherwise SQLCipher validation fails with a user-actionable error.
fn validate_restore_readiness(
    archive: &mut ZipArchive<File>,
    manifest: &BundleManifest,
    key: Option<&str>,
) -> Result<(bool, String, Vec<String>)> {
    let config_bytes = read_zip_entry(archive, "config/config.json")?;
    let _config: AppConfig =
        serde_json::from_slice(&config_bytes).context("parsing bundled config.json")?;

    let tempdir = tempdir().context("creating remote restore verification dir")?;
    let extracted_archive_path = tempdir.path().join("history-vault.sqlite");
    let extracted_source_evidence_path = tempdir.path().join("source-evidence.sqlite");
    extract_zip_entry_to_path(archive, "archive/history-vault.sqlite", &extracted_archive_path)?;
    extract_zip_entry_to_path(
        archive,
        "archive/source-evidence.sqlite",
        &extracted_source_evidence_path,
    )?;

    let connection = Connection::open(&extracted_archive_path)
        .with_context(|| format!("opening {}", extracted_archive_path.display()))?;
    let source_evidence_connection = Connection::open(&extracted_source_evidence_path)
        .with_context(|| format!("opening {}", extracted_source_evidence_path.display()))?;
    if manifest.archive_mode == "encrypted" {
        let key =
            key.context("unlock the archive before verifying an encrypted remote backup bundle")?;
        apply_cipher_key(&connection, key)?;
        apply_cipher_key(&source_evidence_connection, key)?;
    }

    connection
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .context("validating the bundled archive sqlite payload")?;
    source_evidence_connection
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .context("validating the bundled source-evidence sqlite payload")?;

    let warnings = if manifest.archive_mode == "encrypted" {
        vec![
            "Encrypted bundle validation succeeded with the current session database key."
                .to_string(),
        ]
    } else {
        Vec::new()
    };

    Ok((
        true,
        "Archive payloads opened successfully and the bundled config is readable.".to_string(),
        warnings,
    ))
}

/// Streams one zip entry through SHA-256 and returns digest plus byte count.
fn zip_entry_digest_and_size(
    archive: &mut ZipArchive<File>,
    entry_name: &str,
) -> Result<(String, u64)> {
    let mut entry = archive
        .by_name(entry_name)
        .with_context(|| format!("reading {entry_name} from remote backup bundle"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; ZIP_READ_BUFFER_BYTES];
    let mut size_bytes = 0_u64;

    loop {
        let read = entry.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes += read as u64;
    }

    Ok((hex::encode(hasher.finalize()), size_bytes))
}

/// Extracts one zip entry to disk using a bounded buffer.
fn extract_zip_entry_to_path(
    archive: &mut ZipArchive<File>,
    entry_name: &str,
    target_path: &Path,
) -> Result<()> {
    let mut entry = archive
        .by_name(entry_name)
        .with_context(|| format!("reading {entry_name} from remote backup bundle"))?;
    let mut target =
        File::create(target_path).with_context(|| format!("creating {}", target_path.display()))?;
    let mut buffer = [0_u8; ZIP_READ_BUFFER_BYTES];

    loop {
        let read = entry.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        target.write_all(&buffer[..read])?;
    }

    Ok(())
}
