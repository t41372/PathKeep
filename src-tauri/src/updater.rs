#![cfg_attr(test, allow(dead_code))]

//! Desktop updater orchestration.
//!
//! This module adapts the Tauri updater plugin into PathKeep's read models and
//! progress events. It keeps update state explicit for the UI: check results,
//! download/install progress, and relaunch signaling all flow through typed
//! payloads instead of implicit plugin callbacks.

use chrono::Utc;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;
use url::Url;
use vault_core::{
    AppUpdateAvailability, AppUpdateCheckResult, AppUpdateInstallRequest, AppUpdateInstallState,
    PendingAppUpdate,
};

/// Fallback releases page shown when updater metadata is unavailable.
pub(crate) const RELEASES_PAGE_URL: &str = "https://github.com/t41372/PathKeep/releases";
/// Frontend event name used for updater progress snapshots.
pub(crate) const UPDATER_PROGRESS_EVENT: &str = "pathkeep://updater-progress";
const TEST_UPDATER_ENDPOINTS_ENV: &str = "PATHKEEP_TEST_UPDATER_ENDPOINTS";

/// Checks for an available application update and normalizes the result for the UI.
pub(crate) async fn check_for_app_update<R: Runtime>(app: AppHandle<R>) -> AppUpdateCheckResult {
    let checked_at = now_iso();
    let current_version = app.package_info().version.to_string();

    if vault_core::test_support::network_is_blocked() {
        return check_failure(
            checked_at,
            Some(current_version),
            std::io::Error::from(std::io::ErrorKind::NetworkUnreachable).to_string(),
        );
    }

    let updater = match updater_for_handle(&app) {
        Ok(updater) => updater,
        Err(error) => {
            return check_failure(checked_at, Some(current_version), error);
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let published_at = format_release_date(update.date);
            check_available(
                checked_at,
                UpdateMetadata {
                    current_version: update.current_version,
                    version: update.version,
                    body: update.body,
                    download_url: update.download_url,
                },
                published_at,
            )
        }
        Ok(None) => check_none(checked_at, current_version),
        Err(error) => check_failure(checked_at, Some(current_version), error.to_string()),
    }
}

/// Downloads and installs a pending update while emitting progress snapshots.
pub(crate) async fn download_and_install_app_update<R: Runtime>(
    app: AppHandle<R>,
    request: Option<AppUpdateInstallRequest>,
) -> AppUpdateInstallState {
    let expected_version = request.and_then(|request| request.expected_version);
    let updater = match updater_for_handle(&app) {
        Ok(updater) => updater,
        Err(error) => return install_error_state(None, None, None, error),
    };

    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => return install_uptodate_state(),
        Err(error) => return install_error_state(None, None, None, error.to_string()),
    };

    if let Some(expected_version) = expected_version.as_ref() {
        if expected_version != &update.version {
            return install_error_state(
                Some(update.version.clone()),
                None,
                None,
                format!(
                    "Updater metadata changed while preparing the install. Expected {expected_version}, got {}.",
                    update.version
                ),
            );
        }
    }

    let progress_app = app.clone();
    let install_app = app.clone();
    let version = update.version.clone();
    let progress = Arc::new(Mutex::new((0_u64, None::<u64>)));
    emit_update_progress(&app, downloading_state(&version, 0, None));

    let progress_for_download = Arc::clone(&progress);
    let progress_for_install = Arc::clone(&progress);
    let version_for_download = version.clone();
    let version_for_install = version.clone();
    let result = update
        .download_and_install(
            move |chunk_length, total_length| {
                let mut progress = progress_for_download.lock().expect("progress lock");
                progress.0 += chunk_length as u64;
                progress.1 = total_length;
                emit_update_progress(
                    &progress_app,
                    downloading_state(&version_for_download, progress.0, progress.1),
                );
            },
            move || {
                let progress = progress_for_install.lock().expect("progress lock");
                emit_update_progress(
                    &install_app,
                    installing_state(&version_for_install, progress.0, progress.1),
                );
            },
        )
        .await;

    let (downloaded_bytes, content_length) = *progress.lock().expect("progress lock");

    match result {
        Ok(()) => {
            let installed = installed_state(&update.version, downloaded_bytes, content_length);
            emit_update_progress(&app, installed.clone());
            installed
        }
        Err(error) => install_error_state(
            Some(update.version),
            Some(downloaded_bytes),
            content_length,
            error.to_string(),
        ),
    }
}

/// Requests an app restart after a successful update install.
pub(crate) fn relaunch_after_update<R: Runtime>(app: AppHandle<R>) -> bool {
    app.request_restart();
    true
}

/// Builds the updater instance, including any debug-only endpoint overrides.
fn updater_for_handle<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app.updater_builder();
    if let Some(endpoints) = runtime_updater_endpoints()? {
        builder = builder.endpoints(endpoints).map_err(|error| error.to_string())?;
    }
    builder.build().map_err(|error| error.to_string())
}

/// Reads optional debug-only updater endpoint overrides from the environment.
fn runtime_updater_endpoints() -> Result<Option<Vec<Url>>, String> {
    #[cfg(debug_assertions)]
    {
        if let Some(raw) = std::env::var_os(TEST_UPDATER_ENDPOINTS_ENV) {
            let value = raw.to_string_lossy();
            let endpoints = value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| Url::parse(value).map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            if !endpoints.is_empty() {
                return Ok(Some(endpoints));
            }
        }
    }

    Ok(None)
}

#[derive(Debug, Clone)]
struct UpdateMetadata {
    current_version: String,
    version: String,
    body: Option<String>,
    download_url: Url,
}

fn check_available(
    checked_at: String,
    update: UpdateMetadata,
    published_at: Option<String>,
) -> AppUpdateCheckResult {
    AppUpdateCheckResult {
        availability: AppUpdateAvailability {
            supported: true,
            checked_at,
            available: true,
            current_version: Some(update.current_version.clone()),
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            published_at: published_at.clone(),
            error: None,
            download_url: Some(update.download_url.to_string()),
        },
        pending_update: Some(PendingAppUpdate {
            current_version: Some(update.current_version),
            version: update.version,
            notes: update.body,
            published_at,
            download_url: Some(update.download_url.to_string()),
        }),
    }
}

fn check_none(checked_at: String, current_version: String) -> AppUpdateCheckResult {
    AppUpdateCheckResult {
        availability: AppUpdateAvailability {
            supported: true,
            checked_at,
            available: false,
            current_version: Some(current_version.clone()),
            version: Some(current_version),
            notes: None,
            published_at: None,
            error: None,
            download_url: Some(RELEASES_PAGE_URL.to_string()),
        },
        pending_update: None,
    }
}

/// Returns the current UTC timestamp in ISO/RFC3339 form for update read models.
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Formats an optional release date for the desktop read model.
fn format_release_date<T: ToString>(date: Option<T>) -> Option<String> {
    date.map(|value| value.to_string())
}

/// Builds a failed update-check payload that still tells the UI when the check happened.
fn check_failure(
    checked_at: String,
    current_version: Option<String>,
    error: String,
) -> AppUpdateCheckResult {
    AppUpdateCheckResult {
        availability: AppUpdateAvailability {
            supported: true,
            checked_at,
            available: false,
            current_version,
            version: None,
            notes: None,
            published_at: None,
            error: Some(error),
            download_url: Some(RELEASES_PAGE_URL.to_string()),
        },
        pending_update: None,
    }
}

/// Builds a failed install-progress payload with the bytes downloaded so far.
fn install_error_state(
    version: Option<String>,
    downloaded_bytes: Option<u64>,
    content_length: Option<u64>,
    error: String,
) -> AppUpdateInstallState {
    AppUpdateInstallState {
        phase: "error".to_string(),
        version,
        downloaded_bytes,
        content_length,
        message: Some(error),
    }
}

fn install_uptodate_state() -> AppUpdateInstallState {
    AppUpdateInstallState {
        phase: "uptodate".to_string(),
        version: None,
        downloaded_bytes: None,
        content_length: None,
        message: Some("PathKeep is already up to date.".to_string()),
    }
}

fn downloading_state(
    version: &str,
    downloaded_bytes: u64,
    content_length: Option<u64>,
) -> AppUpdateInstallState {
    AppUpdateInstallState {
        phase: "downloading".to_string(),
        version: Some(version.to_string()),
        downloaded_bytes: Some(downloaded_bytes),
        content_length,
        message: Some(format!("Downloading PathKeep {version}...")),
    }
}

fn installing_state(
    version: &str,
    downloaded_bytes: u64,
    content_length: Option<u64>,
) -> AppUpdateInstallState {
    AppUpdateInstallState {
        phase: "installing".to_string(),
        version: Some(version.to_string()),
        downloaded_bytes: Some(downloaded_bytes),
        content_length,
        message: Some(format!("Installing PathKeep {version}...")),
    }
}

fn installed_state(
    version: &str,
    downloaded_bytes: u64,
    content_length: Option<u64>,
) -> AppUpdateInstallState {
    AppUpdateInstallState {
        phase: "installed".to_string(),
        version: Some(version.to_string()),
        downloaded_bytes: Some(downloaded_bytes),
        content_length,
        message: Some(format!(
            "PathKeep {version} is ready. Restart to finish switching versions."
        )),
    }
}

fn emit_update_progress<R: Runtime>(app: &AppHandle<R>, state: AppUpdateInstallState) {
    let _ = app.emit(UPDATER_PROGRESS_EVENT, &state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn network_switch_blocks_update_check_without_connecting() {
        if !cfg!(debug_assertions) {
            return;
        }
        // Do not mutate the process environment while other updater tests run.
        if std::env::var_os("PATHKEEP_NETWORK_TEST_CHILD").is_none() {
            let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "--exact",
                    "updater::tests::network_switch_blocks_update_check_without_connecting",
                    "--nocapture",
                ])
                .env("PATHKEEP_NETWORK_TEST_CHILD", "1")
                .env("PATHKEEP_TEST_BLOCK_NETWORK", "1")
                .env_remove(TEST_UPDATER_ENDPOINTS_ENV)
                .status()
                .expect("isolated updater test");
            assert!(status.success());
            return;
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        listener.set_nonblocking(true).expect("nonblocking listener");
        let endpoint = format!("http://{}/latest.json", listener.local_addr().unwrap());
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".to_string(),
            serde_json::json!({ "pubkey": "test-public-key", "endpoints": [endpoint], "dangerousInsecureTransportProtocol": true }),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .expect("mock app");
        let result = check_for_app_update(app.handle().clone()).await;
        assert!(!result.availability.available);
        assert!(result.availability.supported);
        assert!(result.pending_update.is_none());
        assert_eq!(
            result.availability.error,
            Some(std::io::Error::from(std::io::ErrorKind::NetworkUnreachable).to_string())
        );
        assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    }
}
