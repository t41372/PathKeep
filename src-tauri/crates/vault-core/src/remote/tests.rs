//! Regression coverage for remote-backup bundle contracts.
//!
//! These tests keep preview, upload, manifest, and verification behavior stable
//! while the production module stays split by responsibility.
//!
//! ## Responsibilities
//! - Exercise public remote backup preview, upload, and verification entry
//!   points.
//! - Cover internal URL helpers, bundle helper edges, manifest tampering, and
//!   fake-curl failure handling.
//! - Serialize environment-mutating upload checks with a process-local lock.
//!
//! ## Not responsible for
//! - Talking to a real S3 endpoint.
//! - Exercising Tauri command or worker bridge wrappers.
//! - Benchmarking large archive throughput.
//!
//! ## Dependencies
//! - `tempfile` supplies isolated app roots.
//! - `zip` rewrites bundle entries for tamper tests.
//! - `archive::ensure_archive_initialized` creates minimal SQLite payloads.
//!
//! ## Performance notes
//! Fixtures are intentionally tiny. Large-archive safety is enforced by the
//! production streaming implementation rather than by allocating giant test
//! payloads.

use super::{
    bundle::{build_bundle, copy_archive_database},
    manifest::REMOTE_BUNDLE_VERSION,
    transfer::{
        TEST_CURL_BIN_ENV, inject_bucket, normalize_endpoint, preview_command,
        preview_remote_backup, remote_object_key, run_remote_backup, shell_escape, upload_url,
        validate_remote_backup_config,
    },
    verify::verify_remote_backup,
};
use crate::{
    archive::ensure_archive_initialized,
    config::{project_paths_with_root, save_config},
    models::{AiSettings, AppConfig, ArchiveMode, RemoteBackupConfig, S3CredentialInput},
    utils::sha256_hex,
};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// Serializes tests that mutate the fake curl environment variable.
fn remote_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Creates project paths rooted at a temporary test directory.
fn sample_paths(root: &Path) -> crate::config::ProjectPaths {
    project_paths_with_root(root)
}

/// Returns a minimal plaintext app config with remote backup enabled.
fn sample_config() -> AppConfig {
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        remote_backup: RemoteBackupConfig {
            enabled: true,
            bucket: "example-bucket".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            prefix: "pathkeep".to_string(),
            path_style: true,
            upload_after_backup: false,
            credentials_saved: false,
            last_uploaded_at: None,
            last_uploaded_object_key: None,
            last_error: None,
        },
        ai: AiSettings::default(),
        ..AppConfig::default()
    }
}

/// Initializes a small plaintext archive and saves matching config.
fn initialize_plaintext_archive(paths: &crate::config::ProjectPaths, config: &AppConfig) {
    ensure_archive_initialized(paths, config, None).expect("initialize archive");
    save_config(paths, config).expect("save config");
}

/// Installs an executable fake curl script for upload command tests.
#[cfg(unix)]
fn install_fake_curl(bin_dir: &Path, body: &str) -> PathBuf {
    let script_path = bin_dir.join("curl");
    fs::create_dir_all(bin_dir).expect("create fake curl dir");
    fs::write(&script_path, body).expect("write fake curl");
    let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("chmod");
    script_path
}

/// Installs an executable fake curl batch file for upload command tests.
#[cfg(windows)]
fn install_fake_curl(bin_dir: &Path, body: &str) -> PathBuf {
    let script_path = bin_dir.join("curl.cmd");
    fs::create_dir_all(bin_dir).expect("create fake curl dir");
    fs::write(&script_path, body).expect("write fake curl");
    script_path
}

/// Returns a fake curl body that records argv and succeeds.
#[cfg(unix)]
fn fake_curl_success_log_body(log_path: &Path) -> String {
    format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 0\n", log_path.display())
}

/// Returns a fake curl body that records argv and succeeds.
#[cfg(windows)]
fn fake_curl_success_log_body(log_path: &Path) -> String {
    format!(
        "@echo off\r\nbreak > \"{}\"\r\n:loop\r\nif \"%~1\"==\"\" exit /b 0\r\necho %~1>>\"{}\"\r\nshift\r\ngoto loop\r\n",
        log_path.display(),
        log_path.display()
    )
}

/// Returns a fake curl body that fails with stderr.
#[cfg(unix)]
fn fake_curl_stderr_failure_body() -> &'static str {
    "#!/bin/sh\necho 'upload failed' >&2\nexit 23\n"
}

/// Returns a fake curl body that fails with stderr.
#[cfg(windows)]
fn fake_curl_stderr_failure_body() -> &'static str {
    "@echo off\r\necho upload failed 1>&2\r\nexit /b 23\r\n"
}

/// Returns a fake curl body that fails with stdout.
#[cfg(unix)]
fn fake_curl_stdout_failure_body() -> &'static str {
    "#!/bin/sh\necho 'stdout only failure'\nexit 9\n"
}

/// Returns a fake curl body that fails with stdout.
#[cfg(windows)]
fn fake_curl_stdout_failure_body() -> &'static str {
    "@echo off\r\necho stdout only failure\r\nexit /b 9\r\n"
}

/// Returns a fake curl body that fails by status alone.
#[cfg(unix)]
fn fake_curl_status_failure_body() -> &'static str {
    "#!/bin/sh\nexit 18\n"
}

/// Returns a fake curl body that fails by status alone.
#[cfg(windows)]
fn fake_curl_status_failure_body() -> &'static str {
    "@echo off\r\nexit /b 18\r\n"
}

/// Rewrites one bundle entry while preserving the rest of the zip payload.
fn rewrite_bundle_entry(bundle_path: &Path, entry_name: &str, replacement: &[u8]) -> PathBuf {
    let file = File::open(bundle_path).expect("open existing bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let rewritten_path = bundle_path.with_file_name(format!(
        "tampered-{}",
        bundle_path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let rewritten_file = File::create(&rewritten_path).expect("create rewritten bundle");
    let mut writer = ZipWriter::new(rewritten_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("bundle entry");
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read bundle entry");
        writer.start_file(name.clone(), options).expect("start zip entry");
        if name == entry_name {
            writer.write_all(replacement).expect("write replacement entry");
        } else {
            writer.write_all(&bytes).expect("write copied entry");
        }
    }

    writer.finish().expect("finish rewritten bundle");
    rewritten_path
}

/// Removes one bundle entry while preserving all other entries.
fn rewrite_bundle_without_entry(bundle_path: &Path, entry_name: &str) -> PathBuf {
    let file = File::open(bundle_path).expect("open existing bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let rewritten_path = bundle_path.with_file_name(format!(
        "missing-{}",
        bundle_path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let rewritten_file = File::create(&rewritten_path).expect("create rewritten bundle");
    let mut writer = ZipWriter::new(rewritten_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("bundle entry");
        let name = entry.name().to_string();
        if name == entry_name {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read bundle entry");
        writer.start_file(name, options).expect("start zip entry");
        writer.write_all(&bytes).expect("write copied entry");
    }

    writer.finish().expect("finish rewritten bundle");
    rewritten_path
}

/// Rewrites the manifest and detached checksum together so version-focused tests stay valid.
fn rewrite_bundle_manifest(
    bundle_path: &Path,
    mutate: impl FnOnce(&mut serde_json::Value),
) -> PathBuf {
    let file = File::open(bundle_path).expect("open existing bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let rewritten_path = bundle_path.with_file_name(format!(
        "manifest-{}",
        bundle_path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let rewritten_file = File::create(&rewritten_path).expect("create rewritten bundle");
    let mut writer = ZipWriter::new(rewritten_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut manifest = {
        let mut entry = archive.by_name("metadata/bundle-manifest.json").expect("manifest entry");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read manifest");
        serde_json::from_slice::<serde_json::Value>(&bytes).expect("manifest json")
    };
    mutate(&mut manifest);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).expect("manifest bytes");
    let manifest_hash = sha256_hex(&manifest_bytes);

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("bundle entry");
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read bundle entry");
        writer.start_file(name.clone(), options).expect("start zip entry");
        match name.as_str() {
            "metadata/bundle-manifest.json" => writer.write_all(&manifest_bytes),
            "metadata/bundle-manifest.sha256" => writer.write_all(manifest_hash.as_bytes()),
            _ => writer.write_all(&bytes),
        }
        .expect("write bundle entry");
    }

    writer.finish().expect("finish rewritten bundle");
    rewritten_path
}

/// Adds a non-manifest zip entry so checksum verification detects entry-set drift.
fn rewrite_bundle_with_extra_entry(
    bundle_path: &Path,
    entry_name: &str,
    contents: &[u8],
) -> PathBuf {
    let file = File::open(bundle_path).expect("open existing bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let rewritten_path = bundle_path.with_file_name(format!(
        "extra-{}",
        bundle_path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let rewritten_file = File::create(&rewritten_path).expect("create rewritten bundle");
    let mut writer = ZipWriter::new(rewritten_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("bundle entry");
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read bundle entry");
        writer.start_file(name, options).expect("start zip entry");
        writer.write_all(&bytes).expect("write copied entry");
    }
    writer.start_file(entry_name, options).expect("start extra entry");
    writer.write_all(contents).expect("write extra entry");

    writer.finish().expect("finish rewritten bundle");
    rewritten_path
}

/// Corrupts the compressed payload for one existing zip entry while preserving
/// the central-directory file set, so checksum verification exercises read
/// failures rather than entry-set drift.
fn rewrite_bundle_with_corrupt_entry_data(bundle_path: &Path, entry_name: &str) -> PathBuf {
    let corrupted_path = bundle_path.with_file_name(format!(
        "corrupt-{}",
        bundle_path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::copy(bundle_path, &corrupted_path).expect("copy bundle");
    let file = File::open(&corrupted_path).expect("open copied bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let entry = archive.by_name(entry_name).expect("bundle entry");
    let data_start = entry.data_start() as usize;
    let compressed_size = entry.compressed_size() as usize;
    drop(entry);
    drop(archive);

    assert!(compressed_size > 0, "entry has compressed bytes");
    let mut bytes = fs::read(&corrupted_path).expect("read copied bundle");
    let corrupt_at = data_start + compressed_size / 2;
    bytes[corrupt_at] ^= 0xFF;
    fs::write(&corrupted_path, bytes).expect("write corrupted bundle");
    corrupted_path
}

/// Covers preview warnings, path shape, and object key derivation.
#[test]
fn preview_remote_backup_includes_expected_warning_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let mut config = sample_config();
    config.remote_backup.endpoint = Some("s3.example.test".to_string());
    let preview = preview_remote_backup(&paths, &config).expect("preview");
    let expected_object_key = remote_object_key(&config, Path::new(&preview.bundle_path));
    assert!(preview.bundle_path.contains("remote-backups"));
    assert_eq!(preview.object_key, expected_object_key);
    assert!(preview.object_key.starts_with("pathkeep/"));
    assert!(preview.object_key.ends_with(".zip"));
    assert!(preview.upload_url.ends_with(&preview.object_key));
    assert!(preview.preview_command.contains("--aws-sigv4"));
    assert_eq!(preview.manual_steps.len(), 3);
    assert_eq!(preview.warnings.len(), 3);
}

/// Covers AWS and custom endpoint URL layout contracts.
#[test]
fn upload_url_supports_aws_and_custom_endpoint_layouts() {
    let mut config = sample_config();
    let aws_path = upload_url(&config, "pathkeep/archive.zip").expect("aws");
    assert_eq!(aws_path, "https://s3.us-east-1.amazonaws.com/example-bucket/pathkeep/archive.zip");

    config.remote_backup.path_style = false;
    let aws_hosted = upload_url(&config, "pathkeep/archive.zip").expect("aws hosted");
    assert_eq!(
        aws_hosted,
        "https://example-bucket.s3.us-east-1.amazonaws.com/pathkeep/archive.zip"
    );

    config.remote_backup.endpoint = Some("storage.example.test/root/".to_string());
    config.remote_backup.path_style = true;
    let custom_path = upload_url(&config, "pathkeep/archive.zip").expect("custom path");
    assert_eq!(
        custom_path,
        "https://storage.example.test/root/example-bucket/pathkeep/archive.zip"
    );

    config.remote_backup.path_style = false;
    let custom_hosted = upload_url(&config, "pathkeep/archive.zip").expect("custom hosted");
    assert_eq!(
        custom_hosted,
        "https://example-bucket.storage.example.test/root/pathkeep/archive.zip"
    );
}

/// Covers bundle contents and optional audit manifest inclusion.
#[test]
fn bundle_build_writes_archive_config_and_manifest_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);
    fs::create_dir_all(&paths.manifests_dir).expect("manifests dir");
    fs::write(paths.manifests_dir.join("run-1.json"), "{}").expect("write manifest");

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    assert!(bundle_path.exists());
    let file = File::open(&bundle_path).expect("open bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    assert!(archive.by_name("archive/history-vault.sqlite").is_ok());
    assert!(archive.by_name("config/config.json").is_ok());
    assert!(archive.by_name("audit/manifests/run-1.json").is_ok());
    assert!(archive.by_name("metadata/bundle-manifest.json").is_ok());
}

/// Covers successful upload command execution and persisted success metadata.
#[test]
fn run_remote_backup_uses_curl_and_updates_saved_config() {
    let _guard = remote_test_lock().lock().expect("remote test lock");
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let mut config = sample_config();
    config.remote_backup.credentials_saved = true;
    initialize_plaintext_archive(&paths, &config);
    let bin_dir = dir.path().join("fake-bin");
    let log_path = dir.path().join("curl.log");
    let curl_path = install_fake_curl(&bin_dir, &fake_curl_success_log_body(&log_path));
    unsafe {
        std::env::set_var(TEST_CURL_BIN_ENV, &curl_path);
    }

    let result = run_remote_backup(
        &paths,
        &config,
        None,
        &S3CredentialInput {
            access_key_id: "abc".to_string(),
            secret_access_key: "def".to_string(),
        },
    )
    .expect("remote backup");

    unsafe {
        std::env::remove_var(TEST_CURL_BIN_ENV);
    }

    assert!(result.uploaded);
    assert!(fs::read_to_string(&log_path).expect("read curl log").contains("--aws-sigv4"));
    let saved = crate::config::load_config(&paths).expect("load config");
    assert!(saved.remote_backup.last_uploaded_at.is_some());
    assert_eq!(saved.remote_backup.last_uploaded_object_key, Some(result.object_key.clone()));
    assert_eq!(saved.remote_backup.last_error, None);
}

/// Covers stderr propagation and persisted failure state.
#[test]
fn remote_backup_failure_persists_last_error() {
    let _guard = remote_test_lock().lock().expect("remote test lock");
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);
    let bin_dir = dir.path().join("fake-bin");
    let curl_path = install_fake_curl(&bin_dir, fake_curl_stderr_failure_body());
    unsafe {
        std::env::set_var(TEST_CURL_BIN_ENV, &curl_path);
    }

    let result = run_remote_backup(
        &paths,
        &config,
        None,
        &S3CredentialInput {
            access_key_id: "abc".to_string(),
            secret_access_key: "def".to_string(),
        },
    )
    .expect("remote backup failure");

    unsafe {
        std::env::remove_var(TEST_CURL_BIN_ENV);
    }

    assert!(!result.uploaded);
    assert!(result.message.contains("upload failed"));
    let saved = crate::config::load_config(&paths).expect("load config");
    assert_eq!(saved.remote_backup.last_error, Some("upload failed".to_string()));
}

/// Covers stdout and status-code fallback error messages.
#[test]
fn remote_backup_failure_uses_stdout_or_status_when_stderr_is_empty() {
    let _guard = remote_test_lock().lock().expect("remote test lock");
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);
    let bin_dir = dir.path().join("fake-bin");

    let curl_with_stdout = install_fake_curl(&bin_dir, fake_curl_stdout_failure_body());
    unsafe {
        std::env::set_var(TEST_CURL_BIN_ENV, &curl_with_stdout);
    }
    let stdout_result = run_remote_backup(
        &paths,
        &config,
        None,
        &S3CredentialInput {
            access_key_id: "abc".to_string(),
            secret_access_key: "def".to_string(),
        },
    )
    .expect("stdout failure");
    assert!(!stdout_result.uploaded);
    assert_eq!(stdout_result.message, "stdout only failure");

    let curl_with_status = install_fake_curl(&bin_dir, fake_curl_status_failure_body());
    unsafe {
        std::env::set_var(TEST_CURL_BIN_ENV, &curl_with_status);
    }
    let status_result = run_remote_backup(
        &paths,
        &config,
        None,
        &S3CredentialInput {
            access_key_id: "abc".to_string(),
            secret_access_key: "def".to_string(),
        },
    )
    .expect("status failure");
    assert!(!status_result.uploaded);
    assert!(status_result.message.contains("curl exited with status"));

    unsafe {
        std::env::remove_var(TEST_CURL_BIN_ENV);
    }
}

/// Covers optional scheduler artifacts and encrypted/missing archive validation.
#[test]
fn bundle_helpers_cover_scheduler_encrypted_and_validation_edges() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let mut config = sample_config();
    initialize_plaintext_archive(&paths, &config);
    fs::create_dir_all(paths.audit_repo_path.join("scheduler")).expect("scheduler dir");
    fs::write(paths.audit_repo_path.join("scheduler/run.json"), "{}").expect("write scheduler");

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    let file = File::open(&bundle_path).expect("open bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    assert!(archive.by_name("audit/scheduler/run.json").is_ok());

    config.archive_mode = ArchiveMode::Encrypted;
    let encrypted_error =
        copy_archive_database(&paths, &config, None, &dir.path().join("copy.sqlite"))
            .expect_err("encrypted bundle should require a key");
    let encrypted_message = encrypted_error.to_string();
    assert!(!encrypted_message.is_empty());

    let missing_paths = sample_paths(&dir.path().join("missing"));
    let missing_error = copy_archive_database(
        &missing_paths,
        &sample_config(),
        None,
        &dir.path().join("missing.sqlite"),
    )
    .expect_err("missing archive should fail");
    assert!(missing_error.to_string().contains("archive database has not been created yet"));

    let mut invalid = sample_config();
    invalid.remote_backup.bucket = "example-bucket".to_string();
    invalid.remote_backup.region.clear();
    assert_eq!(
        validate_remote_backup_config(&invalid).expect_err("region required").to_string(),
        "remote backup region is required"
    );
}

/// Covers encrypted bundle manifest mode and detached manifest checksum.
#[test]
fn encrypted_bundles_record_encrypted_manifest_mode() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let mut config = sample_config();
    config.archive_mode = ArchiveMode::Encrypted;
    ensure_archive_initialized(&paths, &config, Some("bundle-secret"))
        .expect("initialize encrypted archive");
    save_config(&paths, &config).expect("save encrypted config");

    let bundle_path = build_bundle(&paths, &config, Some("bundle-secret"), "2026-04-04T01:30:00Z")
        .expect("build encrypted bundle");
    let file = File::open(&bundle_path).expect("open bundle");
    let mut archive = zip::ZipArchive::new(file).expect("zip archive");
    let mut manifest = String::new();
    archive
        .by_name("metadata/bundle-manifest.json")
        .expect("manifest entry")
        .read_to_string(&mut manifest)
        .expect("read manifest");
    assert!(manifest.contains("\"bundleVersion\": \"pathkeep.remote-backup.v1\""));
    assert!(manifest.contains("\"archiveMode\": \"encrypted\""));
    let mut manifest_hash = String::new();
    archive
        .by_name("metadata/bundle-manifest.sha256")
        .expect("manifest hash entry")
        .read_to_string(&mut manifest_hash)
        .expect("read manifest hash");
    assert_eq!(manifest_hash.trim(), sha256_hex(manifest.as_bytes()));
}

/// Covers happy-path plaintext bundle verification.
#[test]
fn verify_remote_backup_reports_restore_ready_for_plaintext_bundle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    let verification = verify_remote_backup(&bundle_path, None).expect("verify bundle");

    assert!(verification.restore_ready);
    assert_eq!(verification.bundle_version, REMOTE_BUNDLE_VERSION);
    assert_eq!(verification.archive_mode, "plaintext");
    assert_eq!(verification.checks.len(), 4);
    assert!(
        verification.checks.iter().all(|check| check.status == "ok" || check.status == "warning")
    );
    assert!(
        verification
            .manifest_files
            .iter()
            .any(|file| file.relative_path == "archive/history-vault.sqlite")
    );
}

/// Covers unsupported but checksum-valid bundle versions.
#[test]
fn verify_remote_backup_reports_unsupported_bundle_version() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    let unsupported_path = rewrite_bundle_manifest(&bundle_path, |manifest| {
        manifest["bundleVersion"] = serde_json::Value::String("pathkeep.remote-backup.v999".into());
    });
    let verification = verify_remote_backup(&unsupported_path, None).expect("verify bundle");

    let version = verification
        .checks
        .iter()
        .find(|check| check.name == "bundle-version")
        .expect("bundle version check");
    assert_eq!(version.status, "error");
    assert!(version.message.contains("not supported"));
    assert!(!verification.restore_ready);
}

/// Covers payload checksum drift detection.
#[test]
fn verify_remote_backup_detects_checksum_drift() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    let tampered_path =
        rewrite_bundle_entry(&bundle_path, "archive/history-vault.sqlite", b"tampered");
    let verification = verify_remote_backup(&tampered_path, None).expect("verify tampered");

    assert!(!verification.restore_ready);
    assert_eq!(
        verification
            .checks
            .iter()
            .find(|check| check.name == "checksums")
            .expect("checksum check")
            .status,
        "error"
    );
}

/// Covers zip entries that drift away from the manifest file set.
#[test]
fn verify_remote_backup_detects_entry_set_drift() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    let tampered_path = rewrite_bundle_with_extra_entry(&bundle_path, "extra/untracked.txt", b"x");
    let verification = verify_remote_backup(&tampered_path, None).expect("verify tampered");

    let checksum =
        verification.checks.iter().find(|check| check.name == "checksums").expect("checksum check");
    assert_eq!(checksum.status, "error");
    assert!(checksum.message.contains("bundle entry set drifted"));
    assert!(!verification.restore_ready);

    let missing_payload_path =
        rewrite_bundle_without_entry(&bundle_path, "archive/history-vault.sqlite");
    let missing_payload =
        verify_remote_backup(&missing_payload_path, None).expect("verify missing payload");
    let checksum = missing_payload
        .checks
        .iter()
        .find(|check| check.name == "checksums")
        .expect("checksum check");
    assert_eq!(checksum.status, "error");
    assert!(checksum.message.contains("bundle entry set drifted"));
    assert!(checksum.message.contains("archive/history-vault.sqlite"));
    assert!(!missing_payload.restore_ready);

    let corrupt_payload_path =
        rewrite_bundle_with_corrupt_entry_data(&bundle_path, "archive/history-vault.sqlite");
    let corrupt_payload =
        verify_remote_backup(&corrupt_payload_path, None).expect("verify corrupt payload");
    let checksum = corrupt_payload
        .checks
        .iter()
        .find(|check| check.name == "checksums")
        .expect("checksum check");
    assert_eq!(checksum.status, "error");
    assert!(checksum.message.contains("missing from the zip payload"));
    assert!(!corrupt_payload.restore_ready);
}

/// Covers manifest JSON tampering detection through detached checksum mismatch.
#[test]
fn verify_remote_backup_rejects_manifest_tampering() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    let tampered_path = rewrite_bundle_entry(
        &bundle_path,
        "metadata/bundle-manifest.json",
        br#"{"bundleVersion":"pathkeep.remote-backup.v1","appVersion":"tampered","createdAt":"2026-04-04T01:30:00Z","archiveMode":"plaintext","bucket":"example-bucket","objectKey":"pathkeep/tampered.zip","files":[]}"#,
    );
    let verification = verify_remote_backup(&tampered_path, None).expect("verify tampered");

    assert!(!verification.restore_ready);
    assert_eq!(
        verification
            .checks
            .iter()
            .find(|check| check.name == "checksums")
            .expect("checksum check")
            .status,
        "error"
    );
}

/// Covers missing detached manifest checksum detection.
#[test]
fn verify_remote_backup_rejects_missing_manifest_checksum_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let config = sample_config();
    initialize_plaintext_archive(&paths, &config);

    let bundle_path =
        build_bundle(&paths, &config, None, "2026-04-04T01:30:00Z").expect("build bundle");
    let tampered_path =
        rewrite_bundle_without_entry(&bundle_path, "metadata/bundle-manifest.sha256");
    let verification = verify_remote_backup(&tampered_path, None).expect("verify tampered");

    assert!(!verification.restore_ready);
    assert_eq!(
        verification
            .checks
            .iter()
            .find(|check| check.name == "required-entries")
            .expect("required entries check")
            .status,
        "error"
    );
}

/// Covers encrypted restore validation with and without the active database key.
#[test]
fn verify_remote_backup_requires_key_for_encrypted_bundle_and_reports_success_warning() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = sample_paths(dir.path());
    let mut config = sample_config();
    config.archive_mode = ArchiveMode::Encrypted;
    ensure_archive_initialized(&paths, &config, Some("bundle-secret"))
        .expect("initialize encrypted archive");
    save_config(&paths, &config).expect("save encrypted config");

    let bundle_path = build_bundle(&paths, &config, Some("bundle-secret"), "2026-04-04T01:30:00Z")
        .expect("build encrypted bundle");
    let missing_key =
        verify_remote_backup(&bundle_path, None).expect("verify encrypted bundle without key");
    assert!(!missing_key.restore_ready);
    assert!(
        missing_key
            .checks
            .iter()
            .find(|check| check.name == "restore-validation")
            .expect("restore validation")
            .message
            .contains("unlock the archive")
    );

    let with_key =
        verify_remote_backup(&bundle_path, Some("bundle-secret")).expect("verify encrypted bundle");
    assert!(with_key.restore_ready);
    assert!(
        with_key
            .warnings
            .iter()
            .any(|warning| warning.contains("Encrypted bundle validation succeeded"))
    );
}

/// Covers low-level endpoint and shell escaping helpers.
#[test]
fn endpoint_and_shell_helpers_stay_stable() {
    assert_eq!(normalize_endpoint("example.test/path/"), "https://example.test/path");
    assert_eq!(
        normalize_endpoint("https://storage.example.test/root"),
        "https://storage.example.test/root"
    );
    assert_eq!(
        inject_bucket("https://storage.example.test", "bucket").expect("inject bucket"),
        "https://bucket.storage.example.test"
    );
    assert_eq!(
        inject_bucket("http://storage.example.test", "bucket").expect("inject bucket"),
        "http://bucket.storage.example.test"
    );
    assert!(inject_bucket("ftp://bad.example", "bucket").is_err());
    assert_eq!(shell_escape("a'b".to_string()), "a'\"'\"'b");
    assert!(
        preview_command(
            &sample_config(),
            Path::new("/tmp/archive.zip"),
            "https://example.test/upload"
        )
        .contains("archive.zip")
    );
    assert!(validate_remote_backup_config(&AppConfig::default()).is_err());
}
