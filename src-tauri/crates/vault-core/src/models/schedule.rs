//! Backup schedule preview/apply models.
//!
//! ## Responsibilities
//! - Define the IPC/read-model contract for native scheduled backup setup.
//! - Keep status issues, verification checks, and manual recovery steps typed so
//!   UI surfaces do not need to parse host-specific warning strings.
//!
//! ## Not responsible for
//! - Installing, removing, repairing, or inspecting native schedulers.
//! - Translating issue codes into user-facing localized copy.
//!
//! ## Dependencies
//! - `serde` for Tauri IPC payloads and browser-preview fixtures.
//!
//! ## Performance notes
//! - These DTOs stay small and cloneable. Large file contents only appear in
//!   preview artifacts that the user explicitly reviews.

use serde::{Deserialize, Serialize};

/// Durable result of one native-scheduler worker invocation.
///
/// This record intentionally lives outside the encrypted archive so failures
/// that happen before the archive can open remain observable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledBackupAttempt {
    pub id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub outcome: ScheduledBackupAttemptOutcome,
    pub phase: ScheduledBackupAttemptPhase,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
    pub run_id: Option<i64>,
}

/// Stable outcome taxonomy for scheduled-backup attempts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ScheduledBackupAttemptOutcome {
    Running,
    Success,
    Skipped,
    Deferred,
    Failed,
    Interrupted,
}

/// Furthest lifecycle boundary reached by a scheduled-backup attempt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ScheduledBackupAttemptPhase {
    WorkerEntry,
    Keyring,
    Config,
    Backup,
    Finished,
}

/// Typed execution-health evidence derived from the bounded attempt ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledBackupHealthIssue {
    pub code: String,
    pub severity: String,
    pub detected_at: String,
    pub related_attempt_id: Option<String>,
    pub evidence: Vec<String>,
}

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
    #[serde(default)]
    pub manual_step_details: Vec<ScheduleManualStep>,
    pub apply_commands: Vec<Vec<String>>,
    pub rollback_commands: Vec<Vec<String>>,
    pub apply_supported: bool,
}

/// One typed manual step shown by state-driven schedule setup and repair UI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleManualStep {
    pub id: String,
    pub title_key: String,
    pub summary_key: String,
    pub why_key: String,
    pub command: Option<Vec<String>>,
    pub file_path: Option<String>,
    pub file_contents: Option<String>,
    pub directory_path: Option<String>,
    pub can_auto_run: bool,
    pub can_verify: bool,
}

/// One user-actionable schedule issue.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleIssue {
    pub code: String,
    pub severity: String,
    pub title_key: String,
    pub detail_key: String,
    pub consequence_key: String,
    pub evidence: Vec<String>,
    pub repair_action: Option<String>,
    pub dismissible: bool,
}

/// One verification line in the state-driven schedule UI.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleVerificationCheck {
    pub key: String,
    pub status: String,
    pub label_key: String,
    pub detail_key: String,
    pub evidence: Vec<String>,
}

/// Last native schedule operation known to the current status payload.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleLastAction {
    pub action: String,
    pub status: String,
    pub message: String,
    pub at: String,
    pub audit_path: Option<String>,
}

/// Runtime schedule status surfaced to the shell.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleStatus {
    pub platform: String,
    pub label: String,
    pub due_after_hours: f64,
    pub check_interval_hours: f64,
    pub apply_supported: bool,
    pub install_state: String,
    pub detected_files: Vec<String>,
    pub manual_steps: Vec<String>,
    #[serde(default)]
    pub manual_step_details: Vec<ScheduleManualStep>,
    pub audit_path: Option<String>,
    pub last_successful_backup_at: Option<String>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub issues: Vec<ScheduleIssue>,
    #[serde(default)]
    pub verification_checks: Vec<ScheduleVerificationCheck>,
    pub checked_at: Option<String>,
    pub last_action: Option<ScheduleLastAction>,
    #[serde(default)]
    pub recent_attempts: Vec<ScheduledBackupAttempt>,
    pub last_scheduled_success_at: Option<String>,
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
    #[serde(default)]
    pub step_results: Vec<ScheduleVerificationCheck>,
}
