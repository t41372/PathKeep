//! Audit and health-report read models.

use super::{BackupRunOverview, BackupWarning};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One artifact linked from an archive run's audit trail.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuditArtifact {
    pub kind: String,
    pub path: String,
    pub checksum: Option<String>,
    pub size_bytes: Option<u64>,
    pub created_at: String,
    pub reason: Option<String>,
}

/// Full audit detail for one run-ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuditRunDetail {
    pub run: BackupRunOverview,
    pub trigger: String,
    pub timezone: Option<String>,
    pub due_only: bool,
    pub profile_scope: Vec<String>,
    pub warnings: Vec<String>,
    /// Coded mirror of `warnings`, index-aligned. Backup runs persist codes so
    /// Audit can localize them; other run types still store opaque messages and
    /// then carry an empty `code`.
    #[serde(default)]
    pub warning_details: Vec<BackupWarning>,
    pub error_message: Option<String>,
    pub stats: Value,
    pub manifest_path: Option<String>,
    pub manifest_hash: Option<String>,
    pub artifacts: Vec<AuditArtifact>,
}

/// One health/doctor check row.
///
/// `code` exists so the UI can name the check in the user's language: the
/// front-end looks the kebab-case slug up in its catalog and only falls back to
/// the English `name` when it meets an unknown code. `detail` stays raw
/// diagnostic prose (paths, counts, hash mismatches) and is rendered as such.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub code: String,
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

/// Full doctor report returned by the archive health read path.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub generated_at: String,
    pub checks: Vec<HealthCheck>,
}

/// Summary of conservative repair work performed by the doctor flow.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HealthRepairReport {
    pub run_id: Option<i64>,
    pub repaired_import_audits: usize,
    pub repaired_visibility_rows: usize,
    pub cleared_derived_rows: usize,
    pub notes: Vec<String>,
}
