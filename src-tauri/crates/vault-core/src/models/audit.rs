//! Audit and health-report read models.

use super::BackupRunOverview;
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
    pub error_message: Option<String>,
    pub stats: Value,
    pub manifest_path: Option<String>,
    pub manifest_hash: Option<String>,
    pub artifacts: Vec<AuditArtifact>,
}

/// One health/doctor check row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
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
