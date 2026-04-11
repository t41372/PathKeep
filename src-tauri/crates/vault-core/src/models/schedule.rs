//! Backup schedule preview/apply models.

use serde::{Deserialize, Serialize};

/// One generated file that a schedule plan asks the user or platform to create.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFile {
    pub relative_path: String,
    pub absolute_path: Option<String>,
    pub purpose: String,
    pub contents: String,
}

/// Preview of the native schedule plan for one platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulePlan {
    pub platform: String,
    pub label: String,
    pub executable_path: String,
    pub generated_files: Vec<GeneratedFile>,
    pub manual_steps: Vec<String>,
    pub apply_commands: Vec<Vec<String>>,
    pub rollback_commands: Vec<Vec<String>>,
    pub apply_supported: bool,
}

/// Runtime schedule status surfaced to the shell.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleStatus {
    pub platform: String,
    pub label: String,
    pub due_after_hours: u64,
    pub check_interval_hours: u64,
    pub apply_supported: bool,
    pub install_state: String,
    pub detected_files: Vec<String>,
    pub manual_steps: Vec<String>,
    pub audit_path: Option<String>,
    pub last_successful_backup_at: Option<String>,
    pub warnings: Vec<String>,
}

/// Result payload for schedule apply/remove actions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub applied: bool,
    pub platform: String,
    pub files: Vec<String>,
    pub audit_path: Option<String>,
    pub message: String,
}
