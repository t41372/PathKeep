#![cfg_attr(test, allow(dead_code))]

use chrono::Utc;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_updater::UpdaterExt;
use url::Url;
use vault_core::{
    AppUpdateAvailability, AppUpdateCheckResult, AppUpdateInstallRequest, AppUpdateInstallState,
    PendingAppUpdate,
};

pub(crate) const RELEASES_PAGE_URL: &str =
    "https://github.com/t41372/BrowserHistoryBackup/releases";
pub(crate) const UPDATER_PROGRESS_EVENT: &str = "pathkeep://updater-progress";
const TEST_UPDATER_ENDPOINTS_ENV: &str = "PATHKEEP_TEST_UPDATER_ENDPOINTS";

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
        Ok(None) => AppUpdateCheckResult {
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
        },
        Err(error) => check_failure(checked_at, Some(current_version), error.to_string()),
    }
}

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
        Ok(None) => {
            return AppUpdateInstallState {
                phase: "uptodate".to_string(),
                version: None,
                downloaded_bytes: None,
                content_length: None,
                message: Some("PathKeep is already up to date.".to_string()),
            };
        }
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
    emit_update_progress(
        &app,
        AppUpdateInstallState {
            phase: "downloading".to_string(),
            version: Some(version.clone()),
            downloaded_bytes: Some(0),
            content_length: None,
            message: Some(format!("Downloading PathKeep {version}...")),
        },
    );

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
                    AppUpdateInstallState {
                        phase: "downloading".to_string(),
                        version: Some(version_for_download.clone()),
                        downloaded_bytes: Some(progress.0),
                        content_length: progress.1,
                        message: Some(format!("Downloading PathKeep {}...", version_for_download)),
                    },
                );
            },
            move || {
                let progress = progress_for_install.lock().expect("progress lock");
                emit_update_progress(
                    &install_app,
                    AppUpdateInstallState {
                        phase: "installing".to_string(),
                        version: Some(version_for_install.clone()),
                        downloaded_bytes: Some(progress.0),
                        content_length: progress.1,
                        message: Some(format!("Installing PathKeep {}...", version_for_install)),
                    },
                );
            },
        )
        .await;

    let (downloaded_bytes, content_length) = *progress.lock().expect("progress lock");

    match result {
        Ok(()) => {
            let installed = AppUpdateInstallState {
                phase: "installed".to_string(),
                version: Some(update.version.clone()),
                downloaded_bytes: Some(downloaded_bytes),
                content_length,
                message: Some(format!(
                    "PathKeep {} is ready. Restart to finish switching versions.",
                    update.version
                )),
            };
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

pub(crate) fn relaunch_after_update<R: Runtime>(app: AppHandle<R>) -> bool {
    app.request_restart();
    true
}

fn updater_for_handle<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app.updater_builder();
    if let Some(endpoints) = runtime_updater_endpoints()? {
        builder = builder.endpoints(endpoints).map_err(|error| error.to_string())?;
    }
    builder.build().map_err(|error| error.to_string())
}

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

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn format_release_date<T: ToString>(date: Option<T>) -> Option<String> {
    date.map(|value| value.to_string())
}

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

fn emit_update_progress<R: Runtime>(app: &AppHandle<R>, state: AppUpdateInstallState) {
    let _ = app.emit(UPDATER_PROGRESS_EVENT, &state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_updater_endpoint_override_parses_csv_values() {
        let original = std::env::var_os(TEST_UPDATER_ENDPOINTS_ENV);
        unsafe {
            std::env::set_var(
                TEST_UPDATER_ENDPOINTS_ENV,
                "http://127.0.0.1:1977/latest.json,https://example.com/latest.json",
            );
        }

        let endpoints =
            runtime_updater_endpoints().expect("endpoints").expect("override endpoints");

        unsafe {
            if let Some(value) = original {
                std::env::set_var(TEST_UPDATER_ENDPOINTS_ENV, value);
            } else {
                std::env::remove_var(TEST_UPDATER_ENDPOINTS_ENV);
            }
        }

        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].as_str(), "http://127.0.0.1:1977/latest.json");
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
    }
}
