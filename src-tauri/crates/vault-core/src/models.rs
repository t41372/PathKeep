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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AiRequestFormat {
    #[serde(rename = "openai")]
    #[default]
    OpenAi,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "google")]
    Google,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "lm-studio")]
    LmStudio,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AiProviderPurpose {
    #[serde(rename = "llm")]
    #[default]
    Llm,
    #[serde(rename = "embedding")]
    Embedding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiProviderConfig {
    pub id: String,
    pub name: String,
    pub purpose: AiProviderPurpose,
    pub request_format: AiRequestFormat,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub api_key_saved: bool,
    pub default_model: String,
    pub model_catalog: Vec<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub dimensions: Option<u32>,
    pub notes: Option<String>,
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::OpenAi,
            enabled: false,
            base_url: None,
            api_key_saved: false,
            default_model: String::new(),
            model_catalog: Vec::new(),
            temperature: Some(0.2),
            max_tokens: Some(1200),
            dimensions: None,
            notes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    pub enabled: bool,
    pub assistant_enabled: bool,
    pub semantic_index_enabled: bool,
    pub mcp_enabled: bool,
    pub skill_enabled: bool,
    pub auto_index_after_backup: bool,
    pub llm_provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
    pub retrieval_top_k: u32,
    pub assistant_system_prompt: String,
    pub llm_providers: Vec<AiProviderConfig>,
    pub embedding_providers: Vec<AiProviderConfig>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            assistant_enabled: false,
            semantic_index_enabled: false,
            mcp_enabled: false,
            skill_enabled: false,
            auto_index_after_backup: false,
            llm_provider_id: None,
            embedding_provider_id: None,
            retrieval_top_k: 8,
            assistant_system_prompt: "You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.".to_string(),
            llm_providers: Vec::new(),
            embedding_providers: Vec::new(),
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
    pub ai: AiSettings,
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
            ai: AiSettings::default(),
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
pub struct AppBuildInfo {
    pub product_name: String,
    pub version: String,
    pub git_commit_short: String,
    pub git_commit_full: String,
    pub git_dirty: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiIndexStatus {
    pub enabled: bool,
    pub assistant_enabled: bool,
    pub mcp_enabled: bool,
    pub skill_enabled: bool,
    pub ready: bool,
    pub indexed_items: usize,
    pub last_indexed_at: Option<String>,
    pub llm_provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
    pub warning: Option<String>,
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
    pub ai_status: AiIndexStatus,
    pub insight_status: InsightStatus,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderSecretInput {
    pub provider_id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiIndexRequest {
    pub provider_id: Option<String>,
    pub full_rebuild: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiIndexReport {
    pub provider_id: String,
    pub model: String,
    pub indexed_items: usize,
    pub updated_items: usize,
    pub skipped_items: usize,
    pub removed_items: usize,
    pub last_indexed_at: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSearchRequest {
    pub query: String,
    pub profile_id: Option<String>,
    pub domain: Option<String>,
    pub limit: Option<u32>,
}

impl Default for AiSearchRequest {
    fn default() -> Self {
        Self { query: String::new(), profile_id: None, domain: None, limit: Some(8) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSearchEntry {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub score: f32,
    pub match_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSearchResponse {
    pub total: usize,
    pub provider_id: String,
    pub model: String,
    pub items: Vec<AiSearchEntry>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAssistantRequest {
    pub question: String,
    pub profile_id: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiCitation {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub visited_at: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiAssistantResponse {
    pub answer: String,
    pub provider_id: String,
    pub embedding_provider_id: String,
    pub citations: Vec<AiCitation>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightStatus {
    pub ready: bool,
    pub last_run_at: Option<String>,
    pub runs: usize,
    pub cards: usize,
    pub topics: usize,
    pub threads: usize,
    pub content_coverage: f32,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightEvidenceItem {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub visited_at: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightCard {
    pub card_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub window_days: u32,
    pub profile_id: Option<String>,
    pub score: f32,
    pub chromium_enhanced: bool,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightTopicSummary {
    pub topic_id: String,
    pub label: String,
    pub profile_scope: String,
    pub window_days: u32,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub visit_count: usize,
    pub revisit_count: usize,
    pub trend_slope: f32,
    pub burst_score: f32,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightThreadSummary {
    pub thread_id: String,
    pub title: String,
    pub profile_id: String,
    pub status: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub visit_count: usize,
    pub reopen_count: usize,
    pub open_loop_score: f32,
    pub dominant_topic_id: Option<String>,
    pub chromium_enhanced: bool,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightThreadDetail {
    pub summary: InsightThreadSummary,
    pub visits: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightQueryLadder {
    pub root_term: String,
    pub profile_id: String,
    pub steps: Vec<String>,
    pub stages: Vec<String>,
    pub count: usize,
    pub chromium_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightWorkflowRole {
    pub role: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightWorkflowEdge {
    pub from_role: String,
    pub to_role: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightWorkflowMap {
    pub profile_id: Option<String>,
    pub roles: Vec<InsightWorkflowRole>,
    pub edges: Vec<InsightWorkflowEdge>,
    pub chromium_enhanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightProfileFacet {
    pub key: String,
    pub label: String,
    pub value: String,
    pub confidence: f32,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightSnapshot {
    pub generated_at: String,
    pub window_days: u32,
    pub profile_id: Option<String>,
    pub status: InsightStatus,
    pub cards: Vec<InsightCard>,
    pub topics: Vec<InsightTopicSummary>,
    pub threads: Vec<InsightThreadSummary>,
    pub query_ladders: Vec<InsightQueryLadder>,
    pub workflow_map: InsightWorkflowMap,
    pub profile_facets: Vec<InsightProfileFacet>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunInsightsRequest {
    pub profile_id: Option<String>,
    pub window_days: Option<u32>,
    pub full_rebuild: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunInsightsReport {
    pub run_id: i64,
    pub processed_visits: usize,
    pub enriched_visits: usize,
    pub failed_enrichments: usize,
    pub topic_count: usize,
    pub thread_count: usize,
    pub card_count: usize,
    pub content_coverage: f32,
    pub last_run_at: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExplainInsightRequest {
    pub insight_id: String,
    pub insight_kind: String,
    pub profile_id: Option<String>,
    pub window_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightExplanation {
    pub explanation: String,
    pub used_llm: bool,
    pub citations: Vec<InsightEvidenceItem>,
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{AiSearchRequest, InsightStatus};

    #[test]
    fn ai_search_request_defaults_to_eight_results() {
        let request = AiSearchRequest::default();
        assert_eq!(request.query, "");
        assert_eq!(request.profile_id, None);
        assert_eq!(request.domain, None);
        assert_eq!(request.limit, Some(8));
    }

    #[test]
    fn insight_status_defaults_to_empty_state() {
        let status = InsightStatus::default();
        assert!(!status.ready);
        assert_eq!(status.cards, 0);
        assert_eq!(status.content_coverage, 0.0);
    }
}
