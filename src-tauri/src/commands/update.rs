//! Tauri commands for the app-update flow.

#[cfg(not(test))]
use crate::updater;
#[cfg(not(test))]
use tauri::AppHandle;

#[cfg(not(test))]
#[tauri::command]
/// Checks whether a newer application build is available.
pub(crate) async fn check_for_app_update(app: AppHandle) -> vault_core::AppUpdateCheckResult {
    updater::check_for_app_update(app).await
}

#[cfg(not(test))]
#[tauri::command]
/// Downloads and installs a pending update using the Tauri updater plugin.
pub(crate) async fn download_and_install_app_update(
    app: AppHandle,
    request: Option<vault_core::AppUpdateInstallRequest>,
) -> vault_core::AppUpdateInstallState {
    updater::download_and_install_app_update(app, request).await
}

#[cfg(not(test))]
#[tauri::command]
/// Relaunches the app after an update has been staged successfully.
pub(crate) fn relaunch_after_update(app: AppHandle) -> bool {
    updater::relaunch_after_update(app)
}
