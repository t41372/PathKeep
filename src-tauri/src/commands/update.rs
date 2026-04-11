use crate::updater;
use tauri::AppHandle;

#[cfg(not(test))]
#[tauri::command]
pub(crate) async fn check_for_app_update(app: AppHandle) -> vault_core::AppUpdateCheckResult {
    updater::check_for_app_update(app).await
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) async fn download_and_install_app_update(
    app: AppHandle,
    request: Option<vault_core::AppUpdateInstallRequest>,
) -> vault_core::AppUpdateInstallState {
    updater::download_and_install_app_update(app, request).await
}

#[cfg(not(test))]
#[tauri::command]
pub(crate) fn relaunch_after_update(app: AppHandle) -> bool {
    updater::relaunch_after_update(app)
}
