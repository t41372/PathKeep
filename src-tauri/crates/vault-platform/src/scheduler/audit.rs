//! Schedule audit artifact helpers.
//!
//! ## Responsibilities
//! - Write apply/remove/repair audit files for native scheduler operations.
//! - Resolve the latest scheduler audit artifact for status payloads.
//!
//! ## Not responsible for
//! - Running platform-specific scheduler commands.
//! - Interpreting install state or deciding whether an operation succeeded.
//!
//! ## Dependencies
//! - `vault_core::ProjectPaths` for repo-local audit paths.
//! - `chrono` for stable audit filenames.
//!
//! ## Performance notes
//! - Audit payloads are tiny JSON documents; directory scans are bounded to the
//!   scheduler audit folder and only happen during explicit status reads.

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::{ProjectPaths, models::SchedulePlan};

pub(super) fn write_macos_apply_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    plist_path: &str,
    status: &str,
) -> Result<PathBuf> {
    write_schedule_audit(
        paths,
        "apply",
        json!({
            "action": "apply",
            "platform": plan.platform,
            "label": plan.label,
            "plistPath": plist_path,
            "status": status,
        }),
    )
}

pub(super) fn write_macos_remove_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    removed_files: &[String],
    launchctl: &[String],
) -> Result<PathBuf> {
    write_schedule_audit(
        paths,
        "remove",
        json!({
            "action": "remove",
            "platform": plan.platform,
            "label": plan.label,
            "removedFiles": removed_files,
            "launchctl": launchctl,
        }),
    )
}

pub(super) fn write_macos_repair_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    removed_files: &[String],
    launchctl: &[String],
) -> Result<PathBuf> {
    write_schedule_audit(
        paths,
        "repair",
        json!({
            "action": "repair",
            "platform": plan.platform,
            "label": plan.label,
            "removedFiles": removed_files,
            "launchctl": launchctl,
        }),
    )
}

pub(super) fn write_windows_schedule_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    action: &str,
    xml_path: &Path,
    success: bool,
    status: &str,
) -> Result<PathBuf> {
    write_schedule_audit(
        paths,
        &format!("{action}-windows"),
        json!({
            "action": action,
            "platform": plan.platform,
            "label": plan.label,
            "xmlPath": xml_path.display().to_string(),
            "success": success,
            "status": status,
        }),
    )
}

pub(super) fn latest_schedule_audit_path(paths: &ProjectPaths) -> Option<String> {
    let scheduler_dir = paths.audit_repo_path.join("scheduler");
    let mut newest = fs::read_dir(&scheduler_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    newest.sort_by_key(|(modified, _)| *modified);
    newest.last().map(|(_, path)| path.display().to_string())
}

fn write_schedule_audit(
    paths: &ProjectPaths,
    action: &str,
    payload: serde_json::Value,
) -> Result<PathBuf> {
    let audit_path = paths
        .audit_repo_path
        .join("scheduler")
        .join(format!("{action}-{}.json", Utc::now().to_rfc3339().replace(':', "-")));
    ensure_parent_dir(&audit_path)?;
    fs::write(&audit_path, serde_json::to_string_pretty(&payload)?)?;
    Ok(audit_path)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
