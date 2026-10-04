//! Schedule audit artifact helpers.
//!
//! ## Responsibilities
//! - Write apply/remove/repair audit files for native scheduler operations.
//!   Every file carries `action`, `at`, `success` and `files`, so the latest
//!   one can be read back as the status payload's `lastAction`.
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
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::{
    ProjectPaths,
    models::{ScheduleLastAction, SchedulePlan},
};

pub(super) fn write_macos_apply_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    plist_path: &str,
    status: &str,
    success: bool,
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
            "success": success,
            "files": [plist_path],
        }),
    )
}

pub(super) fn write_macos_remove_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    removed_files: &[String],
    launchctl: &[String],
    success: bool,
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
            "status": launchctl.join("; "),
            "success": success,
            "files": removed_files,
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
            "status": launchctl.join("; "),
            "success": true,
            "files": removed_files,
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
            "files": if action == "apply" {
                vec![xml_path.display().to_string()]
            } else {
                vec![format!("Task Scheduler:{}", plan.label)]
            },
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

/// Reads the newest scheduler audit file back as the last install / remove /
/// repair PathKeep performed, for the Verify view.
///
/// Files written before `success` existed report `status: "unknown"` rather
/// than a guessed outcome. An unreadable or foreign file yields `None`.
pub(super) fn latest_schedule_action(paths: &ProjectPaths) -> Option<ScheduleLastAction> {
    let audit_path = latest_schedule_audit_path(paths)?;
    let payload: Value = serde_json::from_str(&fs::read_to_string(&audit_path).ok()?).ok()?;
    let action = payload.get("action")?.as_str()?.to_string();
    let status = match payload.get("success").and_then(Value::as_bool) {
        Some(true) => "ok",
        Some(false) => "failed",
        None => "unknown",
    };
    let at = payload.get("at").and_then(Value::as_str).map(ToOwned::to_owned).or_else(|| {
        let modified = fs::metadata(&audit_path).ok()?.modified().ok()?;
        Some(chrono::DateTime::<Utc>::from(modified).to_rfc3339())
    })?;
    let strings = |key: &str| -> Vec<String> {
        match payload.get(key) {
            Some(Value::Array(items)) => {
                items.iter().filter_map(Value::as_str).map(ToOwned::to_owned).collect()
            }
            Some(Value::String(item)) => vec![item.clone()],
            _ => Vec::new(),
        }
    };
    let mut files = strings("files");
    if files.is_empty() && payload.get("files").is_none() {
        files =
            ["plistPath", "xmlPath", "removedFiles"].iter().flat_map(|key| strings(key)).collect();
    }
    Some(ScheduleLastAction {
        action,
        status: status.to_string(),
        message: payload.get("status").and_then(Value::as_str).unwrap_or_default().to_string(),
        at,
        audit_path: Some(audit_path),
        files,
    })
}

fn write_schedule_audit(
    paths: &ProjectPaths,
    action: &str,
    mut payload: serde_json::Value,
) -> Result<PathBuf> {
    let now = Utc::now().to_rfc3339();
    if let Value::Object(fields) = &mut payload {
        fields.insert("at".to_string(), Value::String(now.clone()));
    }
    let audit_path = paths
        .audit_repo_path
        .join("scheduler")
        .join(format!("{action}-{}.json", now.replace(':', "-")));
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn latest_action_reads_the_newest_audit_file_back() {
        let dir = tempdir().expect("tempdir");
        let paths = vault_core::config::project_paths_with_root(dir.path());
        assert!(latest_schedule_action(&paths).is_none());

        let plan = SchedulePlan {
            platform: "macos".to_string(),
            label: "com.example.backup".to_string(),
            executable_path: "/tmp/pathkeep-worker".to_string(),
            generated_files: Vec::new(),
            manual_steps: Vec::new(),
            manual_step_details: Vec::new(),
            apply_commands: Vec::new(),
            rollback_commands: Vec::new(),
            apply_supported: true,
        };
        write_macos_apply_audit(&paths, &plan, "/agents/a.plist", "bootstrapped", true)
            .expect("apply audit");
        let applied = latest_schedule_action(&paths).expect("apply action");
        assert_eq!(applied.action, "apply");
        assert_eq!(applied.status, "ok");
        assert_eq!(applied.files, vec!["/agents/a.plist".to_string()]);
        assert!(chrono::DateTime::parse_from_rfc3339(&applied.at).is_ok());

        // The newest file wins by modification time; keep the two apart.
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_macos_remove_audit(&paths, &plan, &[], &["bootout: not loaded".to_string()], false)
            .expect("remove audit");
        let removed = latest_schedule_action(&paths).expect("remove action");
        assert_eq!(removed.action, "remove");
        assert_eq!(removed.status, "failed");
        assert!(removed.files.is_empty());
        assert_eq!(removed.message, "bootout: not loaded");
        assert!(removed.audit_path.as_deref().is_some_and(|path| path.contains("remove-")));
    }

    #[test]
    fn audit_files_from_before_the_outcome_field_report_unknown() {
        let dir = tempdir().expect("tempdir");
        let paths = vault_core::config::project_paths_with_root(dir.path());
        let scheduler = paths.audit_repo_path.join("scheduler");
        fs::create_dir_all(&scheduler).expect("dir");
        fs::write(
            scheduler.join("apply-old.json"),
            r#"{"action":"apply","plistPath":"/agents/old.plist","status":"ok"}"#,
        )
        .expect("write old audit");

        let action = latest_schedule_action(&paths).expect("old action");
        assert_eq!(action.status, "unknown");
        assert_eq!(action.files, vec!["/agents/old.plist".to_string()]);
    }
}
