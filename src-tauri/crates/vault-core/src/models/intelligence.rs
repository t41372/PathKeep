use serde::{Deserialize, Serialize};

pub const TITLE_NORMALIZATION_PLUGIN_ID: &str = "title-normalization";
pub const READABLE_CONTENT_PLUGIN_ID: &str = "readable-content-refetch";
pub const TITLE_NORMALIZATION_PLUGIN_VERSION: &str = "m5-v1";
pub const READABLE_CONTENT_PLUGIN_VERSION: &str = "m4-v1";
pub const QUERY_GROUPS_MODULE_ID: &str = "query-groups";
pub const THREADS_MODULE_ID: &str = "threads";
pub const REFERENCE_PAGES_MODULE_ID: &str = "reference-pages";
pub const SOURCE_EFFECTIVENESS_MODULE_ID: &str = "source-effectiveness";
pub const TEMPLATE_SUMMARIES_MODULE_ID: &str = "template-summaries";
pub const QUERY_GROUPS_MODULE_VERSION: &str = "m5b-v1";
pub const THREADS_MODULE_VERSION: &str = "m5b-v1";
pub const REFERENCE_PAGES_MODULE_VERSION: &str = "m5b-v1";
pub const SOURCE_EFFECTIVENESS_MODULE_VERSION: &str = "m5b-v1";
pub const TEMPLATE_SUMMARIES_MODULE_VERSION: &str = "m5b-v1";
fn default_enrichment_enabled() -> bool {
    true
}

pub fn default_enrichment_plugin_preferences() -> Vec<EnrichmentPluginPreference> {
    vec![
        EnrichmentPluginPreference {
            plugin_id: TITLE_NORMALIZATION_PLUGIN_ID.to_string(),
            enabled: true,
        },
        EnrichmentPluginPreference {
            plugin_id: READABLE_CONTENT_PLUGIN_ID.to_string(),
            enabled: true,
        },
    ]
}

pub fn merge_enrichment_plugin_preferences(
    current: &[EnrichmentPluginPreference],
) -> Vec<EnrichmentPluginPreference> {
    let defaults = default_enrichment_plugin_preferences();
    let mut merged = Vec::with_capacity(defaults.len());
    for default in defaults {
        let enabled = current
            .iter()
            .find(|item| item.plugin_id == default.plugin_id)
            .map(|item| item.enabled)
            .unwrap_or(default.enabled);
        merged.push(EnrichmentPluginPreference { enabled, ..default });
    }
    merged
}

pub fn default_enrichment_plugin_states() -> Vec<EnrichmentPluginState> {
    vec![
        EnrichmentPluginState {
            id: TITLE_NORMALIZATION_PLUGIN_ID.to_string(),
            enabled: true,
            version: TITLE_NORMALIZATION_PLUGIN_VERSION.to_string(),
        },
        EnrichmentPluginState {
            id: READABLE_CONTENT_PLUGIN_ID.to_string(),
            enabled: true,
            version: READABLE_CONTENT_PLUGIN_VERSION.to_string(),
        },
    ]
}

pub fn merge_enrichment_plugin_states(
    current: &[EnrichmentPluginState],
) -> Vec<EnrichmentPluginState> {
    let defaults = default_enrichment_plugin_states();
    let mut merged = Vec::with_capacity(defaults.len());
    for default in defaults {
        if let Some(existing) = current.iter().find(|item| item.id == default.id) {
            merged.push(EnrichmentPluginState {
                id: default.id.clone(),
                enabled: existing.enabled,
                version: if existing.version.trim().is_empty() {
                    default.version.clone()
                } else {
                    existing.version.clone()
                },
            });
        } else {
            merged.push(default);
        }
    }

    for existing in current {
        if merged.iter().any(|item| item.id == existing.id) {
            continue;
        }
        merged.push(existing.clone());
    }

    merged
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentPluginState {
    pub id: String,
    pub enabled: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EnrichmentSettings {
    pub plugins: Vec<EnrichmentPluginState>,
}

impl Default for EnrichmentSettings {
    fn default() -> Self {
        Self { plugins: default_enrichment_plugin_states() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicModuleState {
    pub id: String,
    pub enabled: bool,
    pub version: String,
}

pub fn default_deterministic_module_states() -> Vec<DeterministicModuleState> {
    vec![
        DeterministicModuleState {
            id: QUERY_GROUPS_MODULE_ID.to_string(),
            enabled: true,
            version: QUERY_GROUPS_MODULE_VERSION.to_string(),
        },
        DeterministicModuleState {
            id: THREADS_MODULE_ID.to_string(),
            enabled: true,
            version: THREADS_MODULE_VERSION.to_string(),
        },
        DeterministicModuleState {
            id: REFERENCE_PAGES_MODULE_ID.to_string(),
            enabled: true,
            version: REFERENCE_PAGES_MODULE_VERSION.to_string(),
        },
        DeterministicModuleState {
            id: SOURCE_EFFECTIVENESS_MODULE_ID.to_string(),
            enabled: true,
            version: SOURCE_EFFECTIVENESS_MODULE_VERSION.to_string(),
        },
        DeterministicModuleState {
            id: TEMPLATE_SUMMARIES_MODULE_ID.to_string(),
            enabled: true,
            version: TEMPLATE_SUMMARIES_MODULE_VERSION.to_string(),
        },
    ]
}

pub fn merge_deterministic_module_states(
    current: &[DeterministicModuleState],
) -> Vec<DeterministicModuleState> {
    let defaults = default_deterministic_module_states();
    let mut merged = Vec::with_capacity(defaults.len());
    for default in defaults {
        if let Some(existing) = current.iter().find(|item| item.id == default.id) {
            merged.push(DeterministicModuleState {
                id: default.id.clone(),
                enabled: existing.enabled,
                version: if existing.version.trim().is_empty() {
                    default.version.clone()
                } else {
                    existing.version.clone()
                },
            });
        } else {
            merged.push(default);
        }
    }

    for existing in current {
        if merged.iter().any(|item| item.id == existing.id) {
            continue;
        }
        merged.push(existing.clone());
    }

    merged
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeterministicSettings {
    pub modules: Vec<DeterministicModuleState>,
}

impl Default for DeterministicSettings {
    fn default() -> Self {
        Self { modules: default_deterministic_module_states() }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentPluginPreference {
    pub plugin_id: String,
    pub enabled: bool,
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
    pub job_queue_paused: bool,
    pub job_queue_concurrency: u32,
    #[serde(default = "default_enrichment_enabled")]
    pub enrichment_enabled: bool,
    #[serde(default = "default_enrichment_plugin_preferences")]
    pub enrichment_plugins: Vec<EnrichmentPluginPreference>,
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
            job_queue_paused: false,
            job_queue_concurrency: 1,
            enrichment_enabled: true,
            enrichment_plugins: default_enrichment_plugin_preferences(),
            llm_provider_id: None,
            embedding_provider_id: None,
            retrieval_top_k: 8,
            assistant_system_prompt: "You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.".to_string(),
            llm_providers: Vec::new(),
            embedding_providers: Vec::new(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiIndexStatus {
    pub enabled: bool,
    pub assistant_enabled: bool,
    pub mcp_enabled: bool,
    pub skill_enabled: bool,
    pub state: String,
    pub ready: bool,
    pub indexed_items: usize,
    pub last_indexed_at: Option<String>,
    pub llm_provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
    pub queue_paused: bool,
    pub queue_concurrency: u32,
    pub queued_jobs: u32,
    pub running_jobs: u32,
    pub failed_jobs: u32,
    pub recent_jobs: Vec<AiQueueJob>,
    pub semantic_sidecar_bytes: u64,
    pub semantic_mirror_bytes: u64,
    pub estimated_embedding_tokens: u64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderCapabilityReport {
    pub supports_chat: bool,
    pub supports_embeddings: bool,
    pub supports_streaming: bool,
    pub supports_tool_use: bool,
    pub supports_structured_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderConnectionTestRequest {
    pub provider_id: String,
    pub purpose: AiProviderPurpose,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderConnectionTestReport {
    pub provider_id: String,
    pub purpose: String,
    pub model: String,
    pub ok: bool,
    pub latency_ms: u64,
    pub capabilities: AiProviderCapabilityReport,
    pub error_code: Option<String>,
    pub action_hint: Option<String>,
    pub retry_hint: Option<String>,
    pub warnings: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AiQueueJobState {
    #[default]
    Queued,
    Running,
    Succeeded,
    Failed,
    Paused,
    Cancelled,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AiQueueJobType {
    #[default]
    IndexBuild,
    IndexClear,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiQueueJob {
    pub id: i64,
    pub job_type: String,
    pub state: String,
    pub priority: i64,
    pub attempt: u32,
    pub max_attempts: u32,
    pub run_id: Option<i64>,
    pub summary: Option<String>,
    pub queued_at: String,
    pub available_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub heartbeat_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiQueueStatus {
    pub paused: bool,
    pub concurrency: u32,
    pub queued: u32,
    pub running: u32,
    pub failed: u32,
    pub recent_jobs: Vec<AiQueueJob>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClearDerivedIntelligenceReport {
    pub cleared_enrichment_rows: usize,
    pub cleared_feature_rows: usize,
    pub cleared_burst_rows: usize,
    pub cleared_query_group_rows: usize,
    pub cleared_topic_rows: usize,
    pub cleared_thread_rows: usize,
    pub cleared_reference_page_rows: usize,
    pub cleared_source_rows: usize,
    pub cleared_module_rows: usize,
    pub cleared_card_rows: usize,
    pub cleared_run_rows: usize,
    pub notes: Vec<String>,
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
    pub clear_only: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiIndexReport {
    pub job_id: Option<i64>,
    pub run_id: Option<i64>,
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
    pub cursor: Option<String>,
}

impl Default for AiSearchRequest {
    fn default() -> Self {
        Self { query: String::new(), profile_id: None, domain: None, limit: Some(8), cursor: None }
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
    pub next_cursor: Option<String>,
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
    pub state: String,
    pub answer: String,
    pub job_id: Option<i64>,
    pub run_id: Option<i64>,
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
    pub query_groups: usize,
    pub reference_pages: usize,
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
pub struct InsightQueryGroupSummary {
    pub query_group_id: String,
    pub profile_id: String,
    pub thread_id: Option<String>,
    pub title: String,
    pub root_query: String,
    pub latest_query: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub visit_count: usize,
    pub burst_count: usize,
    pub step_count: usize,
    pub confidence: f32,
    pub evidence_tier: String,
    pub chromium_enhanced: bool,
    pub steps: Vec<String>,
    pub stages: Vec<String>,
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
    pub query_group_count: usize,
    pub reopen_count: usize,
    pub open_loop_score: f32,
    pub confidence: f32,
    pub evidence_tier: String,
    pub dominant_topic_id: Option<String>,
    pub chromium_enhanced: bool,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightThreadDetail {
    pub summary: InsightThreadSummary,
    pub query_groups: Vec<InsightQueryGroupSummary>,
    pub visits: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightQueryLadder {
    pub query_group_id: Option<String>,
    pub root_term: String,
    pub profile_id: String,
    pub steps: Vec<String>,
    pub stages: Vec<String>,
    pub count: usize,
    pub confidence: f32,
    pub evidence_tier: String,
    pub chromium_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightReferencePageSummary {
    pub reference_page_id: String,
    pub profile_id: Option<String>,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub revisit_count: usize,
    pub cross_day_revisits: usize,
    pub query_group_count: usize,
    pub thread_count: usize,
    pub score: f32,
    pub evidence_tier: String,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightSourceEffectivenessSummary {
    pub source_id: String,
    pub profile_id: Option<String>,
    pub domain: String,
    pub source_role: String,
    pub query_group_count: usize,
    pub thread_count: usize,
    pub stable_landing_count: usize,
    pub reference_page_count: usize,
    pub reopen_support_count: usize,
    pub effectiveness_score: f32,
    pub evidence_tier: String,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightTemplateSummary {
    pub summary_id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub confidence: f32,
    pub profile_id: Option<String>,
    pub evidence: Vec<InsightEvidenceItem>,
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
pub struct InsightDomainStat {
    pub domain: String,
    pub visit_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightCanonicalSummary {
    pub window_visit_count: usize,
    pub window_unique_domains: usize,
    pub on_this_day: Vec<InsightEvidenceItem>,
    pub top_domains: Vec<InsightDomainStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InsightSnapshot {
    pub generated_at: String,
    pub window_days: u32,
    pub profile_id: Option<String>,
    pub status: InsightStatus,
    pub cards: Vec<InsightCard>,
    pub query_groups: Vec<InsightQueryGroupSummary>,
    pub topics: Vec<InsightTopicSummary>,
    pub threads: Vec<InsightThreadSummary>,
    pub query_ladders: Vec<InsightQueryLadder>,
    pub reference_pages: Vec<InsightReferencePageSummary>,
    pub source_effectiveness: Vec<InsightSourceEffectivenessSummary>,
    pub template_summaries: Vec<InsightTemplateSummary>,
    pub workflow_map: InsightWorkflowMap,
    pub profile_facets: Vec<InsightProfileFacet>,
    pub canonical: InsightCanonicalSummary,
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
    pub query_group_count: usize,
    pub topic_count: usize,
    pub thread_count: usize,
    pub reference_page_count: usize,
    pub source_count: usize,
    pub template_summary_count: usize,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceQueueStatus {
    pub queued: usize,
    pub running: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub last_activity_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentPluginStatus {
    pub plugin_id: String,
    pub source_kind: String,
    pub enabled: bool,
    pub stored_records: usize,
    pub queued_jobs: usize,
    pub running_jobs: usize,
    pub failed_jobs: usize,
    pub last_completed_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceJobOverview {
    pub id: i64,
    pub job_type: String,
    pub plugin_id: Option<String>,
    pub state: String,
    pub history_id: Option<i64>,
    pub profile_id: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub attempt: usize,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub last_error: Option<String>,
    pub retryable: bool,
    pub cancellable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeterministicModuleRuntimeStatus {
    pub module_id: String,
    pub enabled: bool,
    pub version: String,
    pub status: String,
    pub depends_on: Vec<String>,
    pub derived_tables: Vec<String>,
    pub last_run_id: Option<i64>,
    pub last_built_at: Option<String>,
    pub last_invalidated_at: Option<String>,
    pub stale_reason: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceRuntimeSnapshot {
    pub queue: IntelligenceQueueStatus,
    pub plugins: Vec<EnrichmentPluginStatus>,
    pub modules: Vec<DeterministicModuleRuntimeStatus>,
    pub recent_jobs: Vec<IntelligenceJobOverview>,
    pub notes: Vec<String>,
}
