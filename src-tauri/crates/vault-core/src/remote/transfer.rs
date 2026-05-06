//! Remote-backup preview and upload execution.
//!
//! This module owns user-visible remote backup planning and the curl-backed
//! upload step. It intentionally keeps network I/O outside bundle construction
//! so bundle integrity remains testable without remote credentials.
//!
//! ## Responsibilities
//! - Validate the remote backup configuration required for preview and upload.
//! - Build preview DTOs with object keys, upload URLs, commands, and warnings.
//! - Execute the explicit S3-compatible curl upload.
//! - Persist last-upload success or failure state into config.
//!
//! ## Not responsible for
//! - Writing zip entries or bundle manifests.
//! - Verifying restore readiness after download.
//! - Managing upload scheduling or retry queues.
//!
//! ## Dependencies
//! - `bundle` creates the local zip payload used by upload.
//! - `config` persists remote backup state.
//! - `utils::now_rfc3339` supplies stable timestamps for paths and status.
//!
//! ## Performance notes
//! Preview is metadata-only. Upload delegates streaming file transfer to curl;
//! bundle creation remains the expensive local step and is isolated in
//! `bundle::build_bundle`.

use super::bundle::{build_bundle, planned_bundle_path};
use crate::{
    config::{ProjectPaths, ensure_paths, save_config},
    models::{AppConfig, ArchiveMode, RemoteBackupPreview, RemoteBackupResult, S3CredentialInput},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use std::{
    path::Path,
    process::{Command, Output},
};

pub(super) const TEST_CURL_BIN_ENV: &str = "BHB_TEST_CURL_BIN";

/// Builds the preview payload for the next remote-backup upload.
///
/// The preview does not create a bundle or touch the network. It validates the
/// minimum S3-compatible configuration and returns the path, object key, upload
/// URL, and manual command a user can review before applying.
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

/// Creates and uploads a remote-backup bundle using the supplied credentials.
///
/// `key` is required when the archive is encrypted. `credentials` are passed
/// only to curl's SigV4 upload command and are not persisted here; the saved
/// config records success or the best available error message.
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

    let output = run_curl_upload(config, credentials, &bundle_path, &upload_url)?;
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

/// Rejects remote-backup configs that cannot produce an addressable object.
///
/// Empty bucket or region values are user/configuration errors. Credential
/// presence is not checked here because preview must still explain missing
/// credentials before upload.
pub(super) fn validate_remote_backup_config(config: &AppConfig) -> Result<()> {
    if config.remote_backup.bucket.trim().is_empty() {
        anyhow::bail!("remote backup bucket is required")
    }
    if config.remote_backup.region.trim().is_empty() {
        anyhow::bail!("remote backup region is required")
    }
    Ok(())
}

/// Runs the curl upload command with the exact arguments PathKeep previews.
fn run_curl_upload(
    config: &AppConfig,
    credentials: &S3CredentialInput,
    bundle_path: &Path,
    upload_url: &str,
) -> Result<Output> {
    Command::new(curl_binary())
        .arg("--fail")
        .arg("--silent")
        .arg("--show-error")
        .arg("--aws-sigv4")
        .arg(format!("aws:amz:{}:s3", config.remote_backup.region))
        .arg("--user")
        .arg(format!("{}:{}", credentials.access_key_id, credentials.secret_access_key))
        .arg("-T")
        .arg(bundle_path.as_os_str())
        .arg(upload_url)
        .output()
        .context("uploading bundle with curl")
}

/// Derives the remote object key from the configured prefix and bundle name.
///
/// Prefix slashes are normalized away so callers can accept either `pathkeep`
/// or `/pathkeep/` without changing the upload target shape.
pub(super) fn remote_object_key(config: &AppConfig, bundle_path: &Path) -> String {
    let prefix = config.remote_backup.prefix.trim_matches('/');
    let name = bundle_path.file_name().unwrap_or_default().to_string_lossy();
    if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") }
}

/// Builds the S3-compatible upload URL for AWS or a custom endpoint.
///
/// `object_key` may contain a leading slash from external callers; the URL
/// contract strips it so the final address has one separator between bucket and
/// object key.
pub(super) fn upload_url(config: &AppConfig, object_key: &str) -> Result<String> {
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

/// Produces the manual curl command shown during preview.
///
/// The command intentionally uses environment variables for credentials, so the
/// preview DTO can be displayed or copied without embedding secrets.
pub(super) fn preview_command(config: &AppConfig, bundle_path: &Path, upload_url: &str) -> String {
    #[cfg(windows)]
    {
        return format!(
            "curl.exe --fail --show-error --aws-sigv4 \"aws:amz:{}:s3\" --user \"%S3_ACCESS_KEY_ID%:%S3_SECRET_ACCESS_KEY%\" -T \"{}\" \"{}\"",
            config.remote_backup.region,
            windows_cmd_escape(bundle_path.display().to_string()),
            windows_cmd_escape(upload_url.to_string()),
        );
    }

    #[cfg(not(windows))]
    format!(
        "curl --fail --show-error --aws-sigv4 \"aws:amz:{}:s3\" --user \"$S3_ACCESS_KEY_ID:$S3_SECRET_ACCESS_KEY\" -T '{}' '{}'",
        config.remote_backup.region,
        shell_escape(bundle_path.display().to_string()),
        shell_escape(upload_url.to_string()),
    )
}

/// Normalizes a custom endpoint into a scheme-bearing URL without a trailing slash.
pub(super) fn normalize_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Injects a bucket label into the host portion of a virtual-hosted endpoint.
///
/// Only HTTP and HTTPS endpoints are accepted because shell upload commands and
/// user-facing preview copy are scoped to S3-compatible HTTP APIs.
pub(super) fn inject_bucket(endpoint: &str, bucket: &str) -> Result<String> {
    let scheme = if let Some(rest) = endpoint.strip_prefix("https://") {
        ("https://", rest)
    } else if let Some(rest) = endpoint.strip_prefix("http://") {
        ("http://", rest)
    } else {
        anyhow::bail!("unsupported endpoint scheme")
    };
    Ok(format!("{}{}.{}", scheme.0, bucket, scheme.1))
}

/// Escapes single quotes for a shell string already wrapped in single quotes.
pub(super) fn shell_escape(value: String) -> String {
    value.replace('\'', "'\"'\"'")
}

/// Escapes double quotes for the Windows `cmd.exe` command preview.
#[cfg(windows)]
fn windows_cmd_escape(value: String) -> String {
    value.replace('"', "\\\"")
}

/// Returns the curl binary path, overridden by tests for hermetic upload checks.
fn curl_binary() -> String {
    std::env::var(TEST_CURL_BIN_ENV).unwrap_or_else(|_| "curl".to_string())
}
