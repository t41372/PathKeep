use crate::{
    archive::{export_archive_database, open_archive_connection},
    config::{ProjectPaths, ensure_paths, save_config},
    models::{AppConfig, ArchiveMode, RemoteBackupPreview, RemoteBackupResult, S3CredentialInput},
    utils::{now_rfc3339, sha256_hex},
};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::tempdir;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const TEST_CURL_BIN_ENV: &str = "BHB_TEST_CURL_BIN";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleManifest {
    created_at: String,
    archive_mode: String,
    bucket: String,
    object_key: String,
    files: Vec<BundleManifestFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BundleManifestFile {
    relative_path: String,
    sha256: String,
    size_bytes: u64,
}

pub fn preview_remote_backup(
    paths: &ProjectPaths,
    config: &AppConfig,
) -> Result<RemoteBackupPreview> {
    ensure_paths(paths)?;
    validate_remote_backup_config(config)?;
    let created_at = now_rfc3339();
    let bundle_path = planned_bundle_path(paths, &created_at);
    let object_key = remote_object_key(config, &bundle_path);
    let upload_url = upload_url(config, &object_key)?;
    let preview_command = preview_command(config, &bundle_path, &upload_url);

    let mut warnings = Vec::new();
    if matches!(config.archive_mode, ArchiveMode::Plaintext) {
        warnings.push(
            "The remote bundle will contain a plaintext archive because local encryption is currently disabled."
                .to_string(),
        );
    }
    if !config.remote_backup.credentials_saved {
        warnings.push(
            "Remote credentials are not stored yet. Save the access key and secret before using Apply."
                .to_string(),
        );
    }
    if config.remote_backup.endpoint.as_deref().is_some() {
        warnings.push(
            "A custom S3-compatible endpoint is configured. Verify TLS and bucket permissions before enabling automatic upload."
                .to_string(),
        );
    }

    Ok(RemoteBackupPreview {
        bundle_path: bundle_path.display().to_string(),
        object_key,
        upload_url,
        preview_command,
        manual_steps: vec![
            "Create the remote backup bundle from the app so the archive and manifest snapshot stay in sync.".to_string(),
            "Review the generated object key and upload URL.".to_string(),
            "Run the preview command with real S3 credentials or let the app upload it.".to_string(),
        ],
        warnings,
    })
}

pub fn run_remote_backup(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    credentials: &S3CredentialInput,
) -> Result<RemoteBackupResult> {
    ensure_paths(paths)?;
    validate_remote_backup_config(config)?;

    let created_at = now_rfc3339();
    let bundle_path = build_bundle(paths, config, key, &created_at)?;
    let object_key = remote_object_key(config, &bundle_path);
    let upload_url = upload_url(config, &object_key)?;

    let output = Command::new(curl_binary())
        .arg("--fail")
        .arg("--silent")
        .arg("--show-error")
        .arg("--aws-sigv4")
        .arg(format!("aws:amz:{}:s3", config.remote_backup.region))
        .arg("--user")
        .arg(format!("{}:{}", credentials.access_key_id, credentials.secret_access_key))
        .arg("-T")
        .arg(bundle_path.as_os_str())
        .arg(&upload_url)
        .output()
        .context("uploading bundle with curl")?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let uploaded = output.status.success();
    let message = if uploaded {
        "Remote backup uploaded successfully.".to_string()
    } else if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("curl exited with status {}", output.status)
    };

    let mut next_config = config.clone();
    next_config.remote_backup.last_error = (!uploaded).then_some(message.clone());
    if uploaded {
        next_config.remote_backup.last_uploaded_at = Some(now_rfc3339());
        next_config.remote_backup.last_uploaded_object_key = Some(object_key.clone());
        next_config.remote_backup.last_error = None;
    }
    save_config(paths, &next_config)?;

    Ok(RemoteBackupResult {
        uploaded,
        bundle_path: bundle_path.display().to_string(),
        object_key,
        upload_url,
        message,
    })
}

fn validate_remote_backup_config(config: &AppConfig) -> Result<()> {
    if config.remote_backup.bucket.trim().is_empty() {
        anyhow::bail!("remote backup bucket is required")
    }
    if config.remote_backup.region.trim().is_empty() {
        anyhow::bail!("remote backup region is required")
    }
    Ok(())
}

fn planned_bundle_path(paths: &ProjectPaths, created_at: &str) -> PathBuf {
    let remote_dir = paths.exports_dir.join("remote-backups");
    let timestamp = created_at.replace(':', "-");
    remote_dir.join(format!("pathkeep-{timestamp}.zip"))
}

fn build_bundle(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    created_at: &str,
) -> Result<PathBuf> {
    let bundle_path = planned_bundle_path(paths, created_at);
    ensure_parent_dir(&bundle_path)?;

    let tempdir = tempdir().context("creating remote backup staging dir")?;
    let archive_copy_path = tempdir.path().join("history-vault.sqlite");
    copy_archive_database(paths, config, key, &archive_copy_path)?;

    let mut manifest_files = Vec::new();
    let file = File::create(&bundle_path).context(format!("creating {}", bundle_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    #[rustfmt::skip]
    add_file_to_zip(&mut zip, &archive_copy_path, "archive/history-vault.sqlite", options, &mut manifest_files)?;
    #[rustfmt::skip]
    add_file_to_zip(&mut zip, &paths.config_path, "config/config.json", options, &mut manifest_files)?;
    #[rustfmt::skip]
    add_dir_to_zip_if_exists(&mut zip, &paths.manifests_dir, "audit/manifests", options, &mut manifest_files)?;
    let scheduler_dir = paths.audit_repo_path.join("scheduler");
    if scheduler_dir.exists() {
        add_dir_to_zip(&mut zip, &scheduler_dir, "audit/scheduler", options, &mut manifest_files)?;
    }

    let manifest = BundleManifest {
        created_at: created_at.to_string(),
        archive_mode: match config.archive_mode {
            ArchiveMode::Encrypted => "encrypted".to_string(),
            ArchiveMode::Plaintext => "plaintext".to_string(),
        },
        bucket: config.remote_backup.bucket.clone(),
        object_key: remote_object_key(config, &bundle_path),
        files: manifest_files,
    };
    zip.start_file("metadata/bundle-manifest.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    zip.finish()?;

    Ok(bundle_path)
}

fn copy_archive_database(
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

fn add_dir_to_zip(
    zip: &mut ZipWriter<File>,
    source_dir: &Path,
    zip_prefix: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<BundleManifestFile>,
) -> Result<()> {
    for entry in WalkDir::new(source_dir).into_iter().filter_map(std::result::Result::ok) {
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

#[rustfmt::skip]
fn ensure_parent_dir(path: &Path) -> Result<()> { if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; } Ok(()) }

#[rustfmt::skip]
fn add_dir_to_zip_if_exists(zip: &mut ZipWriter<File>, path: &Path, prefix: &str, options: SimpleFileOptions, manifest_files: &mut Vec<BundleManifestFile>) -> Result<()> { if path.exists() { add_dir_to_zip(zip, path, prefix, options, manifest_files)?; } Ok(()) }

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    source_path: &Path,
    zip_path: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<BundleManifestFile>,
) -> Result<()> {
    let bytes =
        fs::read(source_path).with_context(|| format!("reading {}", source_path.display()))?;
    zip.start_file(zip_path.replace('\\', "/"), options)?;
    zip.write_all(&bytes)?;
    manifest_files.push(BundleManifestFile {
        relative_path: zip_path.replace('\\', "/"),
        sha256: sha256_hex(&bytes),
        size_bytes: bytes.len() as u64,
    });
    Ok(())
}

fn remote_object_key(config: &AppConfig, bundle_path: &Path) -> String {
    let prefix = config.remote_backup.prefix.trim_matches('/');
    let name = bundle_path.file_name().unwrap_or_default().to_string_lossy();
    if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") }
}

fn upload_url(config: &AppConfig, object_key: &str) -> Result<String> {
    let trimmed_object_key = object_key.trim_start_matches('/');
    if let Some(endpoint) = config.remote_backup.endpoint.as_deref() {
        let endpoint = normalize_endpoint(endpoint);
        if config.remote_backup.path_style {
            return Ok(format!(
                "{}/{}/{}",
                endpoint, config.remote_backup.bucket, trimmed_object_key
            ));
        }

        return Ok(format!(
            "{}/{}",
            inject_bucket(&endpoint, &config.remote_backup.bucket)?,
            trimmed_object_key
        ));
    }

    if config.remote_backup.path_style {
        Ok(format!(
            "https://s3.{}.amazonaws.com/{}/{}",
            config.remote_backup.region, config.remote_backup.bucket, trimmed_object_key
        ))
    } else {
        Ok(format!(
            "https://{}.s3.{}.amazonaws.com/{}",
            config.remote_backup.bucket, config.remote_backup.region, trimmed_object_key
        ))
    }
}

fn preview_command(config: &AppConfig, bundle_path: &Path, upload_url: &str) -> String {
    format!(
        "curl --fail --show-error --aws-sigv4 \"aws:amz:{}:s3\" --user \"$S3_ACCESS_KEY_ID:$S3_SECRET_ACCESS_KEY\" -T '{}' '{}'",
        config.remote_backup.region,
        shell_escape(bundle_path.display().to_string()),
        shell_escape(upload_url.to_string()),
    )
}

fn normalize_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

fn inject_bucket(endpoint: &str, bucket: &str) -> Result<String> {
    let scheme = if let Some(rest) = endpoint.strip_prefix("https://") {
        ("https://", rest)
    } else if let Some(rest) = endpoint.strip_prefix("http://") {
        ("http://", rest)
    } else {
        anyhow::bail!("unsupported endpoint scheme")
    };
    Ok(format!("{}{}.{}", scheme.0, bucket, scheme.1))
}

fn shell_escape(value: String) -> String {
    value.replace('\'', "'\"'\"'")
}

fn curl_binary() -> String {
    std::env::var(TEST_CURL_BIN_ENV).unwrap_or_else(|_| "curl".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archive::ensure_archive_initialized,
        models::{AiSettings, RemoteBackupConfig},
    };
    use std::{
        io::Read,
        os::unix::fs::PermissionsExt,
        sync::{Mutex, OnceLock},
    };

    fn remote_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn sample_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
    }

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

    fn initialize_plaintext_archive(paths: &ProjectPaths, config: &AppConfig) {
        ensure_archive_initialized(paths, config, None).expect("initialize archive");
        save_config(paths, config).expect("save config");
    }

    fn install_fake_curl(bin_dir: &Path, body: &str) -> PathBuf {
        let script_path = bin_dir.join("curl");
        fs::create_dir_all(bin_dir).expect("create fake curl dir");
        fs::write(&script_path, body).expect("write fake curl");
        let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).expect("chmod");
        script_path
    }

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

    #[test]
    fn upload_url_supports_aws_and_custom_endpoint_layouts() {
        let mut config = sample_config();
        let aws_path = upload_url(&config, "pathkeep/archive.zip").expect("aws");
        assert_eq!(
            aws_path,
            "https://s3.us-east-1.amazonaws.com/example-bucket/pathkeep/archive.zip"
        );

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
        let curl_path = install_fake_curl(
            &bin_dir,
            &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexit 0\n", log_path.display()),
        );
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

    #[test]
    fn remote_backup_failure_persists_last_error() {
        let _guard = remote_test_lock().lock().expect("remote test lock");
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = sample_config();
        initialize_plaintext_archive(&paths, &config);
        let bin_dir = dir.path().join("fake-bin");
        let curl_path =
            install_fake_curl(&bin_dir, "#!/bin/sh\necho 'upload failed' >&2\nexit 23\n");
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

    #[test]
    fn remote_backup_failure_uses_stdout_or_status_when_stderr_is_empty() {
        let _guard = remote_test_lock().lock().expect("remote test lock");
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let config = sample_config();
        initialize_plaintext_archive(&paths, &config);
        let bin_dir = dir.path().join("fake-bin");

        let curl_with_stdout =
            install_fake_curl(&bin_dir, "#!/bin/sh\necho 'stdout only failure'\nexit 9\n");
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

        let curl_with_status = install_fake_curl(&bin_dir, "#!/bin/sh\nexit 18\n");
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

    #[test]
    fn encrypted_bundles_record_encrypted_manifest_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let mut config = sample_config();
        config.archive_mode = ArchiveMode::Encrypted;
        ensure_archive_initialized(&paths, &config, Some("bundle-secret"))
            .expect("initialize encrypted archive");
        save_config(&paths, &config).expect("save encrypted config");

        let bundle_path =
            build_bundle(&paths, &config, Some("bundle-secret"), "2026-04-04T01:30:00Z")
                .expect("build encrypted bundle");
        let file = File::open(&bundle_path).expect("open bundle");
        let mut archive = zip::ZipArchive::new(file).expect("zip archive");
        let mut manifest = String::new();
        archive
            .by_name("metadata/bundle-manifest.json")
            .expect("manifest entry")
            .read_to_string(&mut manifest)
            .expect("read manifest");
        assert!(manifest.contains("\"archiveMode\": \"encrypted\""));
    }

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
}
