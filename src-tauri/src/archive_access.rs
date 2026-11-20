//! Startup and recovery commands shared by the desktop and development bridge.
//! Admission owns the disk-format rules. This module presents their result without opening a store.

use crate::{command_error::CommandError, session::SessionState};
use serde::Serialize;
use std::path::PathBuf;
use vault_core::admission::{self, Reason, RootState, Stamp};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Access {
    state: &'static str,
    recovery_only: bool,
    code: Option<String>,
    detail: Option<String>,
    root: PathBuf,
    build_level: u64,
    archive: Option<Stamp>,
}

/// Command allowlist for a foreground launch that could not reserve an archive.
pub(crate) fn recovery_command(command: &str) -> bool {
    matches!(
        command,
        "repair_archive_control"
            | "restart_after_archive_recovery"
            | "unlock_app_session"
            | "app_lock_status"
            | "preview_archive_rollback"
            | "prepare_archive_rollback"
            | "resolve_archive_handoff"
            | "get_archive_access"
            | "app_build_info"
            | "list_recovery_snapshots"
            | "run_full_archive_restore"
            | "check_for_app_update"
            | "download_and_install_app_update"
            | "relaunch_after_update"
            | "get_storage_volume"
            | "confirm_interrupted_wipe"
    )
}

pub(crate) fn access(session: &SessionState) -> Result<Access, CommandError> {
    let paths = vault_core::project_paths().map_err(|e| CommandError::from_backend(&e))?;
    let mut report = Access {
        state: "open",
        recovery_only: crate::desktop_integration::start_was_refused(),
        code: None,
        detail: None,
        archive: None,
        root: paths.app_root.clone(),
        build_level: admission::PRODUCT_ROOT_LEVEL,
    };
    match vault_core::maintenance::rollback::pending(&paths.app_root) {
        Ok(Some(_)) => {
            report.state = "needsRecovery";
            report.code = Some("archive-handoff-choice-required".into());
            return Ok(report);
        }
        Err(e) => {
            report.state = "needsRecovery";
            report.code = Some("archive-handoff-invalid".into());
            report.detail = Some(format!("{e:#}"));
            return Ok(report);
        }
        Ok(None) => {}
    }
    match admission::inspect(&paths.app_root) {
        Ok(RootState::Admitted(c)) => {
            report.archive = c.compatibility.map(|c| c.stamp());
            if let Some(file) =
                vault_core::maintenance::control_recovery::missing_released_store(&paths)
                    .map_err(|e| CommandError::from_backend(&e))?
            {
                report.state = "needsRecovery";
                report.code = Some("archive-store-missing".into());
                report.detail = Some(file.display().to_string());
                return Ok(report);
            }
            if !vault_core::load_config(&paths)
                .map_err(|e| CommandError::from_backend(&e))?
                .initialized
            {
                report.state = "notInitialized";
            } else if let Some(detail) = session.open_failure() {
                let error = CommandError::from(detail.clone());
                match error.code.as_deref() {
                    Some(
                        "archive-store-missing"
                        | "archive-control-invalid"
                        | "agent-history-key-required",
                    ) => report.state = "needsRecovery",
                    Some("archive-schema-too-new") => report.state = "refusedRead",
                    _ => return Ok(report),
                }
                report.code = error.code;
                report.detail = Some(detail);
            }
        }
        Ok(RootState::Uninitialized) => report.state = "notInitialized",
        Ok(RootState::Interrupted(_)) => {
            report.state = "needsRecovery";
            report.code = Some(
                if vault_worker::legacy_wipe_needs_confirmation(&paths) {
                    "archive-wipe-confirmation-required"
                } else {
                    "archive-operation-interrupted"
                }
                .into(),
            );
        }
        Err(reason) => {
            report.state = match reason.reason {
                Reason::WriterTooOld => "refusedWrite",
                Reason::ReaderTooOld | Reason::EnvelopeTooNew | Reason::ControlUnreadable => {
                    "refusedRead"
                }
                _ => "needsRecovery",
            };
            report.code = Some(reason.reason.code().into());
            report.archive = reason.stamp;
            report.detail = Some(reason.detail);
        }
    }
    Ok(report)
}

pub(crate) fn storage_volume(folder: Option<PathBuf>) -> Result<serde_json::Value, CommandError> {
    let paths = vault_core::project_paths().map_err(|e| CommandError::from_backend(&e))?;
    let folder = folder.unwrap_or(paths.app_root);
    let fat = vault_core::storage_volume::fat_data_folder(&folder)
        .map_err(|e| CommandError::internal(e.to_string()))?;
    Ok(serde_json::json!({"folder":folder, "fat":fat}))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Repair {
    Settings,
    Ask,
    Intelligence,
}

pub(crate) fn repair(
    session: &SessionState,
    repair: Repair,
    password: Option<String>,
) -> Result<(), CommandError> {
    let paths = vault_core::project_paths().map_err(|e| CommandError::from_backend(&e))?;
    let _lease = session.begin_maintenance()?;
    match repair {
        Repair::Settings => vault_core::maintenance::control_recovery::rebuild_settings(
            &paths,
            session.owner(),
            password.as_deref(),
        ),
        Repair::Ask | Repair::Intelligence => {
            let role = match repair {
                Repair::Ask => vault_store::Db::Agent,
                _ => vault_store::Db::Intelligence,
            };
            vault_core::maintenance::control_recovery::replace_missing_secondary(
                &paths,
                session.owner(),
                role,
                password.as_deref(),
            )
        }
    }
    .map_err(|e| CommandError::from_backend(&e))
}

pub(crate) fn notices()
-> Result<Vec<vault_core::maintenance::notices::RecoveryNotice>, CommandError> {
    let paths = vault_core::project_paths().map_err(|e| CommandError::from_backend(&e))?;
    Ok(vault_core::maintenance::notices::take(&paths.app_root))
}
