//! Remote-backup bundle construction.
//!
//! This module owns the write path from local archive databases and audit
//! artifacts into a single zip file plus manifest.
//!
//! ## Responsibilities
//! - Export archive and source-evidence SQLite snapshots into a staging area.
//! - Write remote-backup zip entries and manifest rows.
//! - Stream file bytes through zip and SHA-256 hashing without whole-file
//!   buffering.
//! - Keep optional audit manifests and scheduler artifacts in the bundle when
//!   they exist.
//!
//! ## Not responsible for
//! - Uploading a completed bundle.
//! - Verifying a downloaded bundle.
//! - Persisting remote-upload status back to config.
//!
//! ## Dependencies
//! - `archive` creates consistent SQLite export copies.
//! - `config::ProjectPaths` supplies archive, config, and audit locations.
//! - `manifest` supplies the stable remote-backup manifest shape.
//! - `zip` writes the portable container.
//!
//! ## Performance notes
//! The archive database can be large. `add_file_to_zip` streams each file in
//! 64 KiB chunks while updating SHA-256, keeping memory bounded by one chunk
//! per active entry plus zip writer state.

use super::{
    manifest::{BundleManifest, BundleManifestFile, REMOTE_BUNDLE_VERSION},
    transfer::remote_object_key,
};
use crate::{
    archive::{export_archive_database, open_archive_connection, open_source_evidence_connection},
    config::ProjectPaths,
    models::{AppConfig, ArchiveMode},
    utils::sha256_hex,
};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};
use tempfile::tempdir;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const ZIP_COPY_BUFFER_BYTES: usize = 64 * 1024;

/// Chooses the bundle path for one backup attempt from the creation timestamp.
///
/// The timestamp must already be stable for the current operation so preview
/// and build paths are deterministic inside their own call. The returned path
/// lives under `exports/remote-backups` and does not touch the filesystem.
pub(super) fn planned_bundle_path(paths: &ProjectPaths, created_at: &str) -> PathBuf {
    let remote_dir = paths.exports_dir.join("remote-backups");
    let timestamp = created_at.replace(':', "-");
    remote_dir.join(format!("pathkeep-{timestamp}.zip"))
}

/// Builds a restore-ready remote-backup bundle for one local archive snapshot.
///
/// `key` is required only when the local archive mode is encrypted; it is used
/// to unlock the source databases and to encrypt exported copies. The function
/// writes one zip file and streams payload entries, so peak memory does not grow
/// with archive size.
pub(super) fn build_bundle(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    created_at: &str,
) -> Result<PathBuf> {
    let bundle_path = planned_bundle_path(paths, created_at);
    ensure_parent_dir(&bundle_path)?;

    let tempdir = tempdir().context("creating remote backup staging dir")?;
    let archive_copy_path = tempdir.path().join("history-vault.sqlite");
    let source_evidence_copy_path = tempdir.path().join("source-evidence.sqlite");
    copy_archive_database(paths, config, key, &archive_copy_path)?;
    copy_source_evidence_database(paths, config, key, &source_evidence_copy_path)?;

    let mut manifest_files = Vec::new();
    let file = File::create(&bundle_path).context(format!("creating {}", bundle_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    add_file_to_zip(
        &mut zip,
        &archive_copy_path,
        "archive/history-vault.sqlite",
        options,
        &mut manifest_files,
    )?;
    add_file_to_zip(
        &mut zip,
        &source_evidence_copy_path,
        "archive/source-evidence.sqlite",
        options,
        &mut manifest_files,
    )?;
    add_file_to_zip(
        &mut zip,
        &paths.config_path,
        "config/config.json",
        options,
        &mut manifest_files,
    )?;
    add_dir_to_zip_if_exists(
        &mut zip,
        &paths.manifests_dir,
        "audit/manifests",
        options,
        &mut manifest_files,
    )?;
    let scheduler_dir = paths.audit_repo_path.join("scheduler");
    if scheduler_dir.exists() {
        add_dir_to_zip(&mut zip, &scheduler_dir, "audit/scheduler", options, &mut manifest_files)?;
    }

    let manifest = BundleManifest {
        bundle_version: REMOTE_BUNDLE_VERSION.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: created_at.to_string(),
        archive_mode: match config.archive_mode {
            ArchiveMode::Encrypted => "encrypted".to_string(),
            ArchiveMode::Plaintext => "plaintext".to_string(),
        },
        bucket: config.remote_backup.bucket.clone(),
        object_key: remote_object_key(config, &bundle_path),
        files: manifest_files,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    zip.start_file("metadata/bundle-manifest.json", options)?;
    zip.write_all(&manifest_bytes)?;
    zip.start_file("metadata/bundle-manifest.sha256", options)?;
    zip.write_all(format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes())?;
    zip.finish()?;

    Ok(bundle_path)
}

/// Exports the primary archive database into the bundle staging directory.
///
/// Missing archives fail fast because a remote backup without the canonical
/// archive cannot be restored. Encrypted archives require the active database
/// key and keep the staged copy encrypted with the same key.
pub(super) fn copy_archive_database(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    target_path: &Path,
) -> Result<()> {
    if !paths.archive_database_path.exists() {
        anyhow::bail!("archive database has not been created yet")
    }

    let target_key = if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        Some(key.context(
            "the encrypted archive must be unlocked before creating a remote backup bundle",
        )?)
    } else {
        None
    };
    let source = open_archive_connection(paths, config, key)?;
    export_archive_database(&source, target_path, target_key)?;
    Ok(())
}

/// Exports source-evidence provenance into the bundle staging directory.
///
/// Source evidence is part of the restore contract. It follows the same
/// encryption mode as the primary archive so a downloaded bundle does not mix
/// encrypted and plaintext local-state stores.
fn copy_source_evidence_database(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    target_path: &Path,
) -> Result<()> {
    let target_key = if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        Some(key.context(
            "the encrypted archive must be unlocked before creating a remote backup bundle",
        )?)
    } else {
        None
    };
    let source = open_source_evidence_connection(paths, config, key)?;
    export_archive_database(&source, target_path, target_key)?;
    Ok(())
}

/// Adds every regular file under `source_dir` to the zip under `zip_prefix`.
///
/// Directory traversal errors are surfaced instead of silently skipped because
/// the manifest must describe the exact payload that was intended for review.
fn add_dir_to_zip(
    zip: &mut ZipWriter<File>,
    source_dir: &Path,
    zip_prefix: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<BundleManifestFile>,
) -> Result<()> {
    for entry in WalkDir::new(source_dir) {
        let entry =
            entry.with_context(|| format!("walking remote bundle dir {}", source_dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(source_dir)
            .with_context(|| format!("stripping prefix for {}", path.display()))?;
        let zip_path = format!("{zip_prefix}/{}", relative.to_string_lossy());
        add_file_to_zip(zip, path, &zip_path, options, manifest_files)?;
    }
    Ok(())
}

/// Creates the parent directory for a bundle path when one is present.
fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Adds an optional artifact directory only when the local project produced it.
fn add_dir_to_zip_if_exists(
    zip: &mut ZipWriter<File>,
    path: &Path,
    prefix: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<BundleManifestFile>,
) -> Result<()> {
    if path.exists() {
        add_dir_to_zip(zip, path, prefix, options, manifest_files)?;
    }
    Ok(())
}

/// Streams one file into the zip and records its digest and byte size.
///
/// The source file is read once with a fixed-size buffer. That avoids the old
/// whole-file `fs::read` behavior, which could spike memory during large
/// archive backup creation.
fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    source_path: &Path,
    zip_path: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<BundleManifestFile>,
) -> Result<()> {
    let normalized_zip_path = zip_path.replace('\\', "/");
    let source =
        File::open(source_path).with_context(|| format!("reading {}", source_path.display()))?;
    let mut reader = BufReader::new(source);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; ZIP_COPY_BUFFER_BYTES];
    let mut size_bytes = 0_u64;

    zip.start_file(&normalized_zip_path, options)?;
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("reading {}", source_path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        zip.write_all(&buffer[..read])?;
        size_bytes += read as u64;
    }

    manifest_files.push(BundleManifestFile {
        relative_path: normalized_zip_path,
        sha256: hex::encode(hasher.finalize()),
        size_bytes,
    });
    Ok(())
}
