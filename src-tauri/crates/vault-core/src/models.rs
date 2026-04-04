use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveMode {
    #[default]
    Plaintext,
    Encrypted,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum LanguagePreference {
    #[serde(rename = "system")]
    #[default]
    System,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "zh-TW")]
    ZhTw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteBackupConfig {
    pub enabled: bool,
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub prefix: String,
    pub path_style: bool,
    pub upload_after_backup: bool,
    pub credentials_saved: bool,
    pub last_uploaded_at: Option<String>,
    pub last_uploaded_object_key: Option<String>,
    pub last_error: Option<String>,
}

impl Default for RemoteBackupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bucket: String::new(),
            region: "us-east-1".to_string(),
            endpoint: None,
            prefix: "browser-history-backup".to_string(),
            path_style: true,
            upload_after_backup: false,
            credentials_saved: false,
            last_uploaded_at: None,
            last_uploaded_object_key: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub initialized: bool,
    pub archive_mode: ArchiveMode,
    pub preferred_language: LanguagePreference,
    pub due_after_hours: u64,
    pub schedule_check_interval_hours: u64,
    pub checkpoint_days: u64,
    pub capture_favicons: bool,
    pub selected_profile_ids: Vec<String>,
    pub git_enabled: bool,
    pub remember_database_key_in_keyring: bool,
    pub app_autostart: bool,
    pub remote_backup: RemoteBackupConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            initialized: false,
            archive_mode: ArchiveMode::Plaintext,
            preferred_language: LanguagePreference::System,
            due_after_hours: 72,
            schedule_check_interval_hours: 6,
            checkpoint_days: 90,
            capture_favicons: true,
            selected_profile_ids: Vec::new(),
            git_enabled: true,
            remember_database_key_in_keyring: false,
            app_autostart: false,
            remote_backup: RemoteBackupConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDirectories {
    pub app_root: String,
    pub config_path: String,
    pub archive_database_path: String,
    pub audit_repo_path: String,
    pub manifests_dir: String,
    pub exports_dir: String,
    pub raw_snapshots_dir: String,
    pub staging_dir: String,
    pub quarantine_dir: String,
    pub schedule_dir: String,
    pub stronghold_path: String,
    pub stronghold_salt_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStatus {
    pub initialized: bool,
    pub encrypted: bool,
    pub unlocked: bool,
    pub database_path: String,
    pub last_successful_backup_at: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyringStatusReport {
    pub available: bool,
    pub backend: String,
    pub stored_secret: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub profile_id: String,
    pub profile_name: String,
    pub browser_family: String,
    pub browser_name: String,
    pub user_name: Option<String>,
    pub profile_path: String,
    pub history_path: Option<String>,
    pub favicons_path: Option<String>,
    pub history_exists: bool,
    pub browser_version: Option<String>,
    pub history_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupRunOverview {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub manifest_hash: Option<String>,
    pub profiles_processed: usize,
    pub new_visits: usize,
    pub new_urls: usize,
    pub new_downloads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupProfileSummary {
    pub profile_id: String,
    pub new_visits: usize,
    pub new_urls: usize,
    pub new_downloads: usize,
    pub raw_rows: usize,
    pub checkpoint_created: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupReport {
    pub due_skipped: bool,
    pub reason: Option<String>,
    pub run: Option<BackupRunOverview>,
    pub profiles: Vec<BackupProfileSummary>,
    pub manifest_path: Option<String>,
    pub git_commit: Option<String>,
    pub warnings: Vec<String>,
    pub remote_backup: Option<RemoteBackupResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub directories: AppDirectories,
    pub config: AppConfig,
    pub archive_status: ArchiveStatus,
    pub keyring_status: KeyringStatusReport,
    #[serde(alias = "chromeProfiles")]
    pub browser_profiles: Vec<BrowserProfile>,
    pub recent_runs: Vec<BackupRunOverview>,
    pub recent_import_batches: Vec<ImportBatchOverview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub q: Option<String>,
    pub profile_id: Option<String>,
    pub domain: Option<String>,
    pub limit: Option<u32>,
}

impl Default for HistoryQuery {
    fn default() -> Self {
        Self { q: None, profile_id: None, domain: None, limit: Some(150) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub visit_time: i64,
    pub duration_ms: Option<i64>,
    pub transition: Option<i64>,
    pub source_visit_id: i64,
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQueryResponse {
    pub total: usize,
    pub items: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Html,
    Markdown,
    Text,
    Jsonl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub query: HistoryQuery,
    pub format: ExportFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub format: ExportFormat,
    pub path: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct S3CredentialInput {
    pub access_key_id: String,
    pub secret_access_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupPreview {
    pub bundle_path: String,
    pub object_key: String,
    pub upload_url: String,
    pub preview_command: String,
    pub manual_steps: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupResult {
    pub uploaded: bool,
    pub bundle_path: String,
    pub object_key: String,
    pub upload_url: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutRequest {
    pub source_path: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutFileReport {
    pub path: String,
    pub kind: String,
    pub status: String,
    pub records: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutPreviewEntry {
    pub source_path: String,
    pub url: String,
    pub title: Option<String>,
    pub visited_at: String,
    pub source_visit_id: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchOverview {
    pub id: i64,
    pub source_kind: String,
    pub source_path: String,
    pub profile_id: String,
    pub created_at: String,
    pub imported_at: Option<String>,
    pub reverted_at: Option<String>,
    pub status: String,
    pub candidate_items: usize,
    pub imported_items: usize,
    pub duplicate_items: usize,
    pub visible_items: usize,
    pub audit_path: Option<String>,
    pub git_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchDetail {
    pub batch: ImportBatchOverview,
    pub preview_entries: Vec<TakeoutPreviewEntry>,
    pub recognized_files: Vec<TakeoutFileReport>,
    pub quarantined_files: Vec<TakeoutFileReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TakeoutInspection {
    pub source_path: String,
    pub dry_run: bool,
    pub recognized_files: Vec<TakeoutFileReport>,
    pub quarantined_files: Vec<TakeoutFileReport>,
    pub candidate_items: usize,
    pub imported_items: usize,
    pub duplicate_items: usize,
    pub preview_entries: Vec<TakeoutPreviewEntry>,
    pub import_batch: Option<ImportBatchOverview>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedFile {
    pub relative_path: String,
    pub absolute_path: Option<String>,
    pub purpose: String,
    pub contents: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub applied: bool,
    pub platform: String,
    pub files: Vec<String>,
    pub audit_path: Option<String>,
    pub message: String,
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
