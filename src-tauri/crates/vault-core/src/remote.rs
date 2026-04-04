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

    let output = Command::new("curl")
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
    remote_dir.join(format!("chrome-history-backup-{timestamp}.zip"))
}

fn build_bundle(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    created_at: &str,
) -> Result<PathBuf> {
    let bundle_path = planned_bundle_path(paths, created_at);
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tempdir = tempdir().context("creating remote backup staging dir")?;
    let archive_copy_path = tempdir.path().join("history-vault.sqlite");
    copy_archive_database(paths, config, key, &archive_copy_path)?;

    let mut manifest_files = Vec::new();
    let file = File::create(&bundle_path)
        .with_context(|| format!("creating {}", bundle_path.display()))?;
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
        &paths.config_path,
        "config/config.json",
        options,
        &mut manifest_files,
    )?;
    if paths.manifests_dir.exists() {
        add_dir_to_zip(
            &mut zip,
            &paths.manifests_dir,
            "audit/manifests",
            options,
            &mut manifest_files,
        )?;
    }
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

    let source = open_archive_connection(paths, config, key)?;
    let target_key = if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        Some(key.context(
            "the encrypted archive must be unlocked before creating a remote backup bundle",
        )?)
    } else {
        None
    };
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
