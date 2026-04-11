use serde::{Deserialize, Serialize};
use serde_json::Value;
use super::BackupRunOverview;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub generated_at: String,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HealthRepairReport {
    pub run_id: Option<i64>,
    pub repaired_import_audits: usize,
    pub repaired_visibility_rows: usize,
    pub cleared_derived_rows: usize,
    pub notes: Vec<String>,
}
