//! AI, enrichment, and Core Intelligence models.
//!
//! This file is large because it holds the cross-cutting transport models for
//! optional AI features, enrichment plugins, deterministic modules, queue
//! status, and rebuild/runtime surfaces. The types stay data-oriented on
//! purpose so the worker, Tauri shell, and tests can share one honest
//! contract.

use crate::intelligence_catalog::built_in_intelligence_modules;
use serde::{Deserialize, Serialize};

/// Built-in enrichment plugin ID for title normalization.
pub const TITLE_NORMALIZATION_PLUGIN_ID: &str = "title-normalization";
/// Built-in enrichment plugin ID reserved for future readable-content refetch.
pub const READABLE_CONTENT_PLUGIN_ID: &str = "readable-content-refetch";
/// Current version string for the title-normalization plugin.
pub const TITLE_NORMALIZATION_PLUGIN_VERSION: &str = "m5-v1";
/// Current version string for the deferred readable-content plugin.
pub const READABLE_CONTENT_PLUGIN_VERSION: &str = "m4-v1";
/// Built-in deterministic module ID for visit-derived facts.
pub const VISIT_DERIVED_FACTS_MODULE_ID: &str = "visit-derived-facts";
/// Built-in deterministic module ID for daily rollups.
pub const DAILY_ROLLUPS_MODULE_ID: &str = "daily-rollups";
/// Built-in deterministic module ID for browsing sessions.
pub const SESSIONS_MODULE_ID: &str = "sessions";
/// Built-in deterministic module ID for search trails and families.
pub const SEARCH_TRAILS_MODULE_ID: &str = "search-trails";
/// Built-in deterministic module ID for refind pages and source role signals.
pub const REFIND_PAGES_MODULE_ID: &str = "refind-pages";
/// Built-in deterministic module ID for activity-mix and digest composition.
pub const ACTIVITY_MIX_MODULE_ID: &str = "activity-mix";
/// Built-in deterministic module ID for search effectiveness / reopened investigations.
pub const SEARCH_EFFECTIVENESS_MODULE_ID: &str = "search-effectiveness";
/// Built-in deterministic module ID for domain deep-dive / rhythm / discovery surfaces.
pub const DOMAIN_DEEP_DIVE_MODULE_ID: &str = "domain-deep-dive";
/// Current version string for the visit-derived-facts module.
pub const VISIT_DERIVED_FACTS_MODULE_VERSION: &str = "ci-v1";
/// Current version string for the daily-rollups module.
pub const DAILY_ROLLUPS_MODULE_VERSION: &str = "ci-v1";
/// Current version string for the sessions module.
pub const SESSIONS_MODULE_VERSION: &str = "ci-v1";
/// Current version string for the search-trails module.
pub const SEARCH_TRAILS_MODULE_VERSION: &str = "ci-v1";
/// Current version string for the refind-pages module.
pub const REFIND_PAGES_MODULE_VERSION: &str = "ci-v1";
/// Current version string for the activity-mix module.
pub const ACTIVITY_MIX_MODULE_VERSION: &str = "ci-v1";
/// Current version string for the search-effectiveness module.
pub const SEARCH_EFFECTIVENESS_MODULE_VERSION: &str = "ci-v1";
/// Current version string for the domain-deep-dive module.
pub const DOMAIN_DEEP_DIVE_MODULE_VERSION: &str = "ci-v1";
/// Default for whether optional enrichment plugins are enabled at all.
fn default_enrichment_enabled() -> bool {
    true
}

/// Returns the accepted default on/off preferences for built-in enrichment plugins.
pub fn default_enrichment_plugin_preferences() -> Vec<EnrichmentPluginPreference> {
    vec![
        EnrichmentPluginPreference {
            plugin_id: TITLE_NORMALIZATION_PLUGIN_ID.to_string(),
            enabled: true,
        },
        EnrichmentPluginPreference {
            plugin_id: READABLE_CONTENT_PLUGIN_ID.to_string(),
            enabled: false,
        },
    ]
}

/// Merges persisted enrichment plugin preferences with current built-in defaults.
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

/// Returns the accepted default runtime state for built-in enrichment plugins.
pub fn default_enrichment_plugin_states() -> Vec<EnrichmentPluginState> {
    vec![
        EnrichmentPluginState {
            id: TITLE_NORMALIZATION_PLUGIN_ID.to_string(),
            enabled: true,
            version: TITLE_NORMALIZATION_PLUGIN_VERSION.to_string(),
        },
        EnrichmentPluginState {
            id: READABLE_CONTENT_PLUGIN_ID.to_string(),
            enabled: false,
            version: READABLE_CONTENT_PLUGIN_VERSION.to_string(),
        },
    ]
}

/// Merges persisted enrichment plugin state with current built-in defaults.
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
/// Runtime state for one enrichment plugin.
pub struct EnrichmentPluginState {
    pub id: String,
    pub enabled: bool,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
/// Enrichment settings stored in app config.
pub struct EnrichmentSettings {
    pub plugins: Vec<EnrichmentPluginState>,
}

impl Default for EnrichmentSettings {
    /// Returns enrichment settings with all built-in plugins represented.
    fn default() -> Self {
        Self { plugins: default_enrichment_plugin_states() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Runtime state for one deterministic module.
pub struct DeterministicModuleState {
    pub id: String,
    pub enabled: bool,
    pub version: String,
}

/// Returns the accepted default runtime state for built-in deterministic modules.
pub fn default_deterministic_module_states() -> Vec<DeterministicModuleState> {
    built_in_intelligence_modules()
        .iter()
        .map(|module| {
            let descriptor = module.descriptor();
            DeterministicModuleState {
                id: descriptor.id.to_string(),
                enabled: true,
                version: descriptor.version.to_string(),
            }
        })
        .collect()
}

/// Merges persisted deterministic module state with current built-in defaults.
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
    merged
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
/// Deterministic-intelligence settings stored in app config.
pub struct DeterministicSettings {
    pub modules: Vec<DeterministicModuleState>,
}

impl Default for DeterministicSettings {
    /// Returns deterministic settings with all built-in modules represented.
    fn default() -> Self {
        Self { modules: default_deterministic_module_states() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
/// Wire format used to talk to an AI provider.
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
/// Role an AI provider plays inside PathKeep.
pub enum AiProviderPurpose {
    #[serde(rename = "llm")]
    #[default]
    Llm,
    #[serde(rename = "embedding")]
    Embedding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
/// Stored configuration for one LLM or embedding provider.
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
    /// Returns the accepted defaults for a newly added provider row.
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
/// User-facing on/off preference for one enrichment plugin.
pub struct EnrichmentPluginPreference {
    pub plugin_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
/// Full optional-AI settings stored in app config.
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
    /// Returns the accepted defaults for optional AI features.
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
/// Shell-facing semantic-index readiness snapshot.
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
    pub semantic_metadata_bytes: u64,
    pub estimated_embedding_tokens: u64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Capability report for one configured AI provider.
pub struct AiProviderCapabilityReport {
    pub supports_chat: bool,
    pub supports_embeddings: bool,
    pub supports_streaming: bool,
    pub supports_tool_use: bool,
    pub supports_structured_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request payload for testing one provider connection.
pub struct AiProviderConnectionTestRequest {
    pub provider_id: String,
    pub purpose: AiProviderPurpose,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Serialized, UI-facing mirror of the in-engine `LlmCapabilities`.
///
/// The connection probe attaches this for LLM providers so Settings can show whether
/// streaming, tool calling, structured output, and prompt caching are available, plus the
/// known context window. `maxContextTokens` is `None` unless the transport reports one.
pub struct LlmProviderCapabilityReport {
    pub tool_call: bool,
    pub structured_output: bool,
    pub streaming: bool,
    pub prompt_cache: bool,
    pub max_context_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Result payload for one provider connection probe.
pub struct AiProviderConnectionTestReport {
    pub provider_id: String,
    pub purpose: String,
    pub model: String,
    pub ok: bool,
    pub latency_ms: u64,
    pub capabilities: AiProviderCapabilityReport,
    /// LLM-only capability detail (streaming/tool/structured/cache/context). `None` for
    /// embedding providers, which do not have a chat capability surface.
    pub llm_capabilities: Option<LlmProviderCapabilityReport>,
    pub error_code: Option<String>,
    pub action_hint: Option<String>,
    pub retry_hint: Option<String>,
    pub warnings: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
/// Lifecycle state for one persistent AI queue job.
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
/// Kind of work represented by one AI queue job.
pub enum AiQueueJobType {
    #[default]
    IndexBuild,
    IndexClear,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Shell-facing read model for one persistent AI queue job.
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
/// Shell-facing aggregate AI queue status.
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
/// Report returned when derived intelligence state is cleared.
pub struct ClearDerivedIntelligenceReport {
    pub cleared_visit_derived_fact_rows: usize,
    pub cleared_daily_rollup_rows: usize,
    pub cleared_structural_rows: usize,
    pub cleared_runtime_rows: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Secret-storage payload for one provider API key.
pub struct AiProviderSecretInput {
    pub provider_id: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Request payload for building or clearing the semantic index.
pub struct AiIndexRequest {
    pub provider_id: Option<String>,
    pub full_rebuild: bool,
    pub clear_only: bool,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Result payload for an index build/clear run.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
/// Conversational role for one chat-stream message, mirroring `LlmRole` on the IPC contract.
pub enum AiChatRole {
    System,
    #[default]
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// One message in a streaming chat request (hand-written camelCase DTO).
pub struct AiChatMessage {
    pub role: AiChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
/// Request payload for one streaming chat turn.
///
/// `providerId` is optional; when absent the worker uses the configured default LLM provider.
/// `temperature`/`maxTokens` override the provider defaults for this turn only.
pub struct AiChatSendRequest {
    pub provider_id: Option<String>,
    pub messages: Vec<AiChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl Default for AiChatSendRequest {
    /// Returns an empty default request used by shell forms before any input.
    fn default() -> Self {
        Self { provider_id: None, messages: Vec::new(), temperature: None, max_tokens: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Acknowledgement returned by `ai_chat_send`: the run id used to subscribe and cancel.
pub struct AiChatSendAck {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Result returned by `ai_chat_cancel`.
pub struct AiChatCancelResult {
    /// Whether a live run with this id was found and asked to stop.
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
/// One streamed chat chunk delivered over `pathkeep://ai-stream`.
///
/// Variants are tagged by `kind` so the front end can route tokens, reasoning, tool calls, the
/// terminal `done` marker, and a terminal `error` into distinct UI lanes without guessing.
pub enum AiChatStreamChunk {
    /// A fragment of the visible answer.
    Token { text: String },
    /// A fragment of the reasoning/thinking stream.
    Reasoning { text: String },
    /// A tool/function call the model requested (execution arrives in W-AI-7).
    ToolCall { name: String, arguments: String },
    /// Terminal success marker; no more chunks follow for this run.
    Done,
    /// Terminal failure marker carrying a user-facing message; no more chunks follow.
    Error { message: String },
}

/// Tauri event channel that carries [`AiChatStreamEvent`]s to the front end.
///
/// Defined here (a covered vault-core location) rather than inline in the desktop command so a
/// mutation of the literal is caught by a Rust test; the front-end `ai-stream.ts` pins its own
/// matching literal. Both sides MUST agree on this exact string.
pub const AI_CHAT_STREAM_EVENT: &str = "pathkeep://ai-stream";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Envelope emitted on `pathkeep://ai-stream` pairing a chunk with its run id.
pub struct AiChatStreamEvent {
    pub run_id: String,
    pub chunk: AiChatStreamChunk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request payload for semantic-plus-lexical history search.
pub struct AiSearchRequest {
    pub query: String,
    pub profile_id: Option<String>,
    pub domain: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

impl Default for AiSearchRequest {
    /// Returns the default search request used by empty shell forms.
    fn default() -> Self {
        Self { query: String::new(), profile_id: None, domain: None, limit: Some(8), cursor: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// One semantic/lexical search hit returned by AI history search.
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
/// Full response for semantic-plus-lexical history search.
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
/// Request payload for a first-party assistant question.
pub struct AiAssistantRequest {
    pub question: String,
    pub profile_id: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Citation for one visit used in an assistant answer or insight.
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
/// Result payload for an assistant answer.
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
/// High-level readiness summary for Core Intelligence runtime state.
pub struct IntelligenceStatus {
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
/// Aggregate queue counters for deterministic intelligence jobs.
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
/// Runtime status for one enrichment plugin.
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
/// Shell-facing overview of one recent intelligence job.
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
    pub updated_at: String,
    pub heartbeat_at: Option<String>,
    pub progress_label: Option<String>,
    pub progress_detail: Option<String>,
    pub progress_current: Option<usize>,
    pub progress_total: Option<usize>,
    pub progress_percent: Option<f32>,
    pub execution_mode: Option<String>,
    pub affected_profiles: Option<Vec<String>>,
    pub dirty_visit_count: Option<usize>,
    pub dirty_date_keys: Option<Vec<String>>,
    pub fallback_reason: Option<String>,
    pub last_error: Option<String>,
    pub retryable: bool,
    pub cancellable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Runtime status for one deterministic module.
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
/// Combined runtime snapshot for intelligence queues, plugins, and modules.
pub struct IntelligenceRuntimeSnapshot {
    pub queue: IntelligenceQueueStatus,
    pub plugins: Vec<EnrichmentPluginStatus>,
    pub modules: Vec<DeterministicModuleRuntimeStatus>,
    pub recent_jobs: Vec<IntelligenceJobOverview>,
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ai_chat_stream_event_channel_is_pinned() {
        // The desktop command emits on this exact channel; the FE `ai-stream.ts` pins its match.
        assert_eq!(AI_CHAT_STREAM_EVENT, "pathkeep://ai-stream");
    }

    #[test]
    fn ai_chat_stream_chunk_variants_serialize_with_kind_tag() {
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::Token { text: "hi".to_string() }).unwrap(),
            json!({ "kind": "token", "text": "hi" })
        );
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::Reasoning { text: "why".to_string() }).unwrap(),
            json!({ "kind": "reasoning", "text": "why" })
        );
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::ToolCall {
                name: "search".to_string(),
                arguments: "{}".to_string(),
            })
            .unwrap(),
            json!({ "kind": "toolCall", "name": "search", "arguments": "{}" })
        );
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::Done).unwrap(),
            json!({ "kind": "done" })
        );
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::Error { message: "boom".to_string() }).unwrap(),
            json!({ "kind": "error", "message": "boom" })
        );
    }

    #[test]
    fn ai_chat_role_serializes_kebab_case() {
        assert_eq!(serde_json::to_value(AiChatRole::System).unwrap(), json!("system"));
        assert_eq!(serde_json::to_value(AiChatRole::User).unwrap(), json!("user"));
        assert_eq!(serde_json::to_value(AiChatRole::Assistant).unwrap(), json!("assistant"));
        assert_eq!(serde_json::to_value(AiChatRole::Tool).unwrap(), json!("tool"));
    }

    #[test]
    fn ai_chat_send_request_uses_camel_case_keys() {
        let value = serde_json::to_value(AiChatSendRequest {
            provider_id: Some("p1".to_string()),
            messages: vec![AiChatMessage { role: AiChatRole::User, content: "hi".to_string() }],
            temperature: Some(0.5),
            max_tokens: Some(64),
        })
        .unwrap();
        assert_eq!(value["providerId"], json!("p1"));
        assert_eq!(value["maxTokens"], json!(64));
        // The nested message also uses camelCase role/content keys.
        assert_eq!(value["messages"][0], json!({ "role": "user", "content": "hi" }));
    }

    #[test]
    fn ai_chat_send_ack_and_stream_event_use_run_id_camel_case() {
        assert_eq!(
            serde_json::to_value(AiChatSendAck { run_id: "chat-1".to_string() }).unwrap(),
            json!({ "runId": "chat-1" })
        );
        assert_eq!(
            serde_json::to_value(AiChatStreamEvent {
                run_id: "chat-1".to_string(),
                chunk: AiChatStreamChunk::Done,
            })
            .unwrap(),
            json!({ "runId": "chat-1", "chunk": { "kind": "done" } })
        );
    }

    #[test]
    fn ai_chat_cancel_result_uses_camel_case_key() {
        assert_eq!(
            serde_json::to_value(AiChatCancelResult { cancelled: true }).unwrap(),
            json!({ "cancelled": true })
        );
    }

    #[test]
    fn llm_provider_capability_report_uses_camel_case_keys() {
        let value = serde_json::to_value(LlmProviderCapabilityReport {
            tool_call: true,
            structured_output: true,
            streaming: true,
            prompt_cache: false,
            max_context_tokens: Some(8192),
        })
        .unwrap();
        assert_eq!(value["toolCall"], json!(true));
        assert_eq!(value["structuredOutput"], json!(true));
        assert_eq!(value["promptCache"], json!(false));
        assert_eq!(value["maxContextTokens"], json!(8192));
    }
}
