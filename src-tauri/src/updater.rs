#![cfg_attr(test, allow(dead_code))]
#![allow(unexpected_cfgs)]

//! Desktop updater orchestration.
//!
//! This module adapts the Tauri updater plugin into PathKeep's read models and
//! progress events. It keeps update state explicit for the UI: check results,
//! download/install progress, and relaunch signaling all flow through typed
//! payloads instead of implicit plugin callbacks.

use chrono::Utc;
#[cfg(not(coverage))]
use std::sync::{Arc, Mutex};
#[cfg(not(coverage))]
use tauri::Emitter;
use tauri::{AppHandle, Runtime};
#[cfg(not(coverage))]
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
#[cfg(any(test, not(coverage)))]
const TEST_UPDATER_ENDPOINTS_ENV: &str = "PATHKEEP_TEST_UPDATER_ENDPOINTS";
#[cfg(coverage)]
const COVERAGE_UPDATER_STATE_ENV: &str = "PATHKEEP_COVERAGE_UPDATER_STATE";

/// Checks for an available application update and normalizes the result for the UI.
#[cfg(not(coverage))]
pub(crate) async fn check_for_app_update<R: Runtime>(app: AppHandle<R>) -> AppUpdateCheckResult {
    let checked_at = now_iso();
    let current_version = app.package_info().version.to_string();

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

/// Checks the deterministic coverage-mode updater adapter.
#[cfg(coverage)]
pub(crate) async fn check_for_app_update<R: Runtime>(app: AppHandle<R>) -> AppUpdateCheckResult {
    let checked_at = now_iso();
    let current_version = app.package_info().version.to_string();
    match coverage_updater_state().as_deref() {
        Some("available") => check_available(
            checked_at,
            coverage_update_metadata(&current_version),
            Some("2026-04-25".to_string()),
        ),
        Some("error") => check_failure(
            checked_at,
            Some(current_version),
            "coverage updater check failed".to_string(),
        ),
        _ => check_none(checked_at, current_version),
    }
}

/// Downloads and installs a pending update while emitting progress snapshots.
#[cfg(not(coverage))]
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

/// Downloads and installs through the deterministic coverage-mode updater adapter.
#[cfg(coverage)]
pub(crate) async fn download_and_install_app_update<R: Runtime>(
    app: AppHandle<R>,
    request: Option<AppUpdateInstallRequest>,
) -> AppUpdateInstallState {
    let expected_version = request.and_then(|request| request.expected_version);
    let Some(state) = coverage_updater_state() else {
        return install_uptodate_state();
    };
    if state == "check-error" {
        return install_error_state(None, None, None, "coverage updater check failed".to_string());
    }
    if state == "uptodate" {
        return install_uptodate_state();
    }

    let update = coverage_update_metadata(&app.package_info().version.to_string());
    if let Some(expected_version) = expected_version.as_ref() {
        if expected_version != &update.version {
            return install_error_state(
                Some(update.version),
                None,
                None,
                format!(
                    "Updater metadata changed while preparing the install. Expected {expected_version}, got {}.",
                    coverage_update_version()
                ),
            );
        }
    }

    emit_update_progress(&app, downloading_state(&update.version, 0, None));
    emit_update_progress(&app, downloading_state(&update.version, 128, Some(256)));
    emit_update_progress(&app, installing_state(&update.version, 128, Some(256)));
    if state == "install-error" {
        return install_error_state(
            Some(update.version),
            Some(128),
            Some(256),
            "coverage updater install failed".to_string(),
        );
    }
    let installed = installed_state(&update.version, 256, Some(256));
    emit_update_progress(&app, installed.clone());
    installed
}

/// Requests an app restart after a successful update install.
#[cfg(not(coverage))]
pub(crate) fn relaunch_after_update<R: Runtime>(app: AppHandle<R>) -> bool {
    app.request_restart();
    true
}

/// Reports restart intent in coverage mode without invoking the mock runtime restart hook.
#[cfg(coverage)]
pub(crate) fn relaunch_after_update<R: Runtime>(_app: AppHandle<R>) -> bool {
    true
}

/// Builds the updater instance, including any debug-only endpoint overrides.
#[cfg(not(coverage))]
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
#[cfg(any(test, not(coverage)))]
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
#[cfg(any(test, not(coverage)))]
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

#[cfg(coverage)]
fn coverage_updater_state() -> Option<String> {
    std::env::var(COVERAGE_UPDATER_STATE_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(coverage)]
fn coverage_update_version() -> String {
    "9.9.9".to_string()
}

#[cfg(coverage)]
fn coverage_update_metadata(current_version: &str) -> UpdateMetadata {
    UpdateMetadata {
        current_version: current_version.to_string(),
        version: coverage_update_version(),
        body: Some("Coverage release notes".to_string()),
        download_url: Url::parse("https://example.com/pathkeep.dmg").expect("coverage URL"),
    }
}

#[cfg(not(coverage))]
fn emit_update_progress<R: Runtime>(app: &AppHandle<R>, state: AppUpdateInstallState) {
    let _ = app.emit(UPDATER_PROGRESS_EVENT, &state);
}

#[cfg(coverage)]
fn emit_update_progress<R: Runtime>(_app: &AppHandle<R>, _state: AppUpdateInstallState) {
    let _ = UPDATER_PROGRESS_EVENT;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_updater_endpoint_override_parses_csv_values() {
        let original = std::env::var_os(TEST_UPDATER_ENDPOINTS_ENV);
        unsafe {
            std::env::set_var(TEST_UPDATER_ENDPOINTS_ENV, "https://restore.example/latest.json");
        }
        let restore_original = std::env::var_os(TEST_UPDATER_ENDPOINTS_ENV);
        unsafe {
            std::env::set_var(
                TEST_UPDATER_ENDPOINTS_ENV,
                "http://127.0.0.1:1977/latest.json,https://example.com/latest.json",
            );
        }

        let endpoints =
            runtime_updater_endpoints().expect("endpoints").expect("override endpoints");
        unsafe {
            std::env::remove_var(TEST_UPDATER_ENDPOINTS_ENV);
        }
        let absent_endpoints = runtime_updater_endpoints().expect("absent endpoints");

        unsafe {
            if let Some(value) = restore_original {
                std::env::set_var(TEST_UPDATER_ENDPOINTS_ENV, value);
            } else {
                std::env::remove_var(TEST_UPDATER_ENDPOINTS_ENV);
            }
        }
        unsafe {
            if let Some(value) = original {
                std::env::set_var(TEST_UPDATER_ENDPOINTS_ENV, value);
            } else {
                std::env::remove_var(TEST_UPDATER_ENDPOINTS_ENV);
            }
        }

        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].as_str(), "http://127.0.0.1:1977/latest.json");
        assert!(absent_endpoints.is_none());
    }

    #[test]
    fn helper_states_keep_release_fallback_truthful() {
        let check = check_failure(
            "2026-04-10T00:00:00Z".to_string(),
            Some("0.1.0".to_string()),
            "offline".to_string(),
        );
        assert!(!check.availability.available);
        assert_eq!(check.availability.download_url.as_deref(), Some(RELEASES_PAGE_URL));

        let install = install_error_state(
            Some("0.2.0".to_string()),
            Some(40),
            Some(100),
            "signature mismatch".to_string(),
        );
        assert_eq!(install.phase, "error");
        assert_eq!(install.downloaded_bytes, Some(40));
        assert!(install.message.as_deref().expect("message").contains("signature mismatch"));

        let available = check_available(
            "2026-04-10T00:00:00Z".to_string(),
            UpdateMetadata {
                current_version: "0.1.0".to_string(),
                version: "0.2.0".to_string(),
                body: Some("notes".to_string()),
                download_url: Url::parse("https://example.com/pathkeep.dmg").expect("url"),
            },
            Some("2026-04-09".to_string()),
        );
        assert!(available.availability.available);
        assert_eq!(available.pending_update.expect("pending").version, "0.2.0");

        let none = check_none("2026-04-10T00:00:00Z".to_string(), "0.1.0".to_string());
        assert!(!none.availability.available);
        assert!(none.pending_update.is_none());

        assert_eq!(install_uptodate_state().phase, "uptodate");
        assert_eq!(downloading_state("0.2.0", 1, Some(2)).downloaded_bytes, Some(1));
        assert_eq!(installing_state("0.2.0", 2, Some(2)).phase, "installing");
        assert_eq!(installed_state("0.2.0", 2, Some(2)).phase, "installed");
    }

    #[cfg(coverage)]
    #[tokio::test]
    async fn coverage_updater_adapter_covers_available_error_install_and_restart_states() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");

        unsafe {
            std::env::set_var(COVERAGE_UPDATER_STATE_ENV, "available");
        }
        let available = check_for_app_update(app.handle().clone()).await;
        assert!(available.availability.available);
        assert_eq!(available.pending_update.expect("pending").version, coverage_update_version());
        let installed = download_and_install_app_update(
            app.handle().clone(),
            Some(AppUpdateInstallRequest { expected_version: Some(coverage_update_version()) }),
        )
        .await;
        assert_eq!(installed.phase, "installed");
        assert_eq!(installed.downloaded_bytes, Some(256));

        let mismatch = download_and_install_app_update(
            app.handle().clone(),
            Some(AppUpdateInstallRequest { expected_version: Some("0.0.0".to_string()) }),
        )
        .await;
        assert_eq!(mismatch.phase, "error");

        unsafe {
            std::env::set_var(COVERAGE_UPDATER_STATE_ENV, "install-error");
        }
        let install_error = download_and_install_app_update(app.handle().clone(), None).await;
        assert_eq!(install_error.phase, "error");
        assert_eq!(install_error.downloaded_bytes, Some(128));

        unsafe {
            std::env::set_var(COVERAGE_UPDATER_STATE_ENV, "check-error");
        }
        let check_error = download_and_install_app_update(app.handle().clone(), None).await;
        assert_eq!(check_error.phase, "error");

        unsafe {
            std::env::set_var(COVERAGE_UPDATER_STATE_ENV, "error");
        }
        let failed_check = check_for_app_update(app.handle().clone()).await;
        assert!(failed_check.availability.error.is_some());

        unsafe {
            std::env::set_var(COVERAGE_UPDATER_STATE_ENV, "uptodate");
        }
        let none = check_for_app_update(app.handle().clone()).await;
        assert!(!none.availability.available);
        let uptodate = download_and_install_app_update(app.handle().clone(), None).await;
        assert_eq!(uptodate.phase, "uptodate");
        assert!(relaunch_after_update(app.handle().clone()));

        unsafe {
            std::env::remove_var(COVERAGE_UPDATER_STATE_ENV);
        }
        let absent_state = download_and_install_app_update(app.handle().clone(), None).await;
        assert_eq!(absent_state.phase, "uptodate");
    }

    #[cfg(not(coverage))]
    #[tokio::test]
    async fn updater_entrypoints_degrade_truthfully_without_plugin_runtime() {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".to_string(),
            serde_json::json!({
                "pubkey": "test-public-key",
                "endpoints": []
            }),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .expect("mock app");
        let checked = check_for_app_update(app.handle().clone()).await;
        assert!(!checked.availability.available);
        assert!(checked.availability.supported);
        assert!(checked.availability.checked_at.contains('T'));
        assert!(checked.availability.error.is_some());
        assert_eq!(checked.availability.download_url.as_deref(), Some(RELEASES_PAGE_URL));
        assert!(checked.pending_update.is_none());

        let install = download_and_install_app_update(
            app.handle().clone(),
            Some(AppUpdateInstallRequest { expected_version: Some("9.9.9".to_string()) }),
        )
        .await;
        assert_eq!(install.phase, "error");
        assert_eq!(install.version, None);
        assert!(install.message.is_some());

        emit_update_progress(
            app.handle(),
            AppUpdateInstallState {
                phase: "installed".to_string(),
                version: Some("9.9.9".to_string()),
                downloaded_bytes: Some(10),
                content_length: Some(10),
                message: Some("ready".to_string()),
            },
        );
    }

    #[test]
    fn updater_format_helpers_cover_optional_release_dates() {
        assert!(now_iso().contains('T'));
        assert_eq!(format_release_date(Some("2026-04-17")), Some("2026-04-17".to_string()));
        assert_eq!(format_release_date::<&str>(None), None);
    }
}
