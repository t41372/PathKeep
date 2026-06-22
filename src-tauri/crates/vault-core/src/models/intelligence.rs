//! AI, enrichment, and Core Intelligence models.
//!
//! This file is large because it holds the cross-cutting transport models for
//! optional AI features, enrichment plugins, deterministic modules, queue
//! status, and rebuild/runtime surfaces. The types stay data-oriented on
//! purpose so the worker, Tauri shell, and tests can share one honest
//! contract.

use crate::intelligence_catalog::built_in_intelligence_modules;
use crate::models::AppConfig;
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

/// Built-in content-fetch extractor id for GitHub public repo metadata (W-ENRICH-1).
pub const GITHUB_REPO_EXTRACTOR_ID: &str = "github-repo";
/// Built-in content-fetch extractor id for the deterministic generic readable fallback (W-ENRICH-1).
pub const GENERIC_READABLE_EXTRACTOR_ID: &str = "generic-readable";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// User-facing on/off preference for one content-fetch extractor (06 §2a per-extractor toggle).
pub struct ContentFetchExtractorPreference {
    pub extractor_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Per-domain allow/block rule for content fetching (06 §2a per-domain toggle).
///
/// `allowed = false` blocks the domain; `allowed = true` is an explicit allow (reserved for a future
/// allow-list-only mode). The MVP runner treats only `allowed = false` as load-bearing (a block),
/// since the master switch is the gate; the explicit-allow shape is modelled so the FE can bind it.
pub struct ContentFetchDomainRule {
    pub domain: String,
    pub allowed: bool,
}

/// Returns the accepted default per-extractor content-fetch preferences (both built-ins ON).
///
/// "ON" here means "this extractor is permitted WHEN the master `content_fetch_enabled` switch is on"
/// — the master switch is hard-default-OFF (06 §2a), so a fresh install fetches nothing until the user
/// opts in. With the master on, both built-in extractors are enabled by default.
pub fn default_content_fetch_extractor_preferences() -> Vec<ContentFetchExtractorPreference> {
    vec![
        ContentFetchExtractorPreference {
            extractor_id: GITHUB_REPO_EXTRACTOR_ID.to_string(),
            enabled: true,
        },
        ContentFetchExtractorPreference {
            extractor_id: GENERIC_READABLE_EXTRACTOR_ID.to_string(),
            enabled: true,
        },
    ]
}

/// Merges persisted content-fetch extractor preferences with the current built-in defaults.
///
/// Mirrors [`merge_enrichment_plugin_preferences`]: every built-in extractor is represented (a newly
/// added built-in surfaces on the next normalize without a config migration, keeping its default
/// enabled state), and the user's stored on/off for an existing extractor is preserved. Unknown
/// stored ids (a removed extractor) are dropped so the surface stays the built-in set.
pub fn merge_content_fetch_extractor_preferences(
    current: &[ContentFetchExtractorPreference],
) -> Vec<ContentFetchExtractorPreference> {
    let defaults = default_content_fetch_extractor_preferences();
    defaults
        .into_iter()
        .map(|default| {
            let enabled = current
                .iter()
                .find(|item| item.extractor_id == default.extractor_id)
                .map(|item| item.enabled)
                .unwrap_or(default.enabled);
            ContentFetchExtractorPreference { enabled, ..default }
        })
        .collect()
}

/// Whether one content-fetch extractor is permitted under the current config's per-extractor prefs.
///
/// PURE → unit-tested. An extractor with no stored preference defaults to ENABLED (so a newly added
/// built-in works once the master switch is on, without a config migration); an explicit stored
/// `enabled: false` disables it. This is layered UNDER the master `content_fetch_enabled` gate (the
/// caller checks that first).
pub fn content_extractor_enabled(config: &AppConfig, extractor_id: &str) -> bool {
    config
        .ai
        .content_fetch_extractors
        .iter()
        .find(|pref| pref.extractor_id == extractor_id)
        .map(|pref| pref.enabled)
        .unwrap_or(true)
}

/// Whether a domain is on the per-domain content-fetch blocklist.
///
/// PURE → unit-tested. Case-insensitive match against any rule with `allowed = false`. A domain with
/// no rule (or only an explicit-allow rule) is NOT blocked. Centralized so the enqueue path + runner
/// share one decision.
pub fn content_fetch_domain_blocked(config: &AppConfig, domain: &str) -> bool {
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return false;
    }
    config
        .ai
        .content_fetch_domains
        .iter()
        .any(|rule| !rule.allowed && rule.domain.trim().to_ascii_lowercase() == domain)
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
    /// Master switch for site CONTENT fetching (W-ENRICH-1, 06 §2a). HARD-DEFAULT-OFF and INDEPENDENT
    /// of `enrichment_enabled` (which only governs the offline title plugin). The content-fetch job is
    /// a no-op unless this is on — no network egress happens until the user explicitly opts in.
    #[serde(default)]
    pub content_fetch_enabled: bool,
    /// Per-extractor content-fetch toggles (06 §2a). Defaults to both built-ins enabled, gated by the
    /// master switch above.
    #[serde(default = "default_content_fetch_extractor_preferences")]
    pub content_fetch_extractors: Vec<ContentFetchExtractorPreference>,
    /// Per-domain content-fetch allow/block rules (06 §2a). Empty by default (no domain blocked).
    #[serde(default)]
    pub content_fetch_domains: Vec<ContentFetchDomainRule>,
    pub llm_provider_id: Option<String>,
    pub embedding_provider_id: Option<String>,
    pub retrieval_top_k: u32,
    pub assistant_system_prompt: String,
    pub llm_providers: Vec<AiProviderConfig>,
    pub embedding_providers: Vec<AiProviderConfig>,
    /// RRF constant `k` for hybrid search fusion (W-AI-6, 05 §9.4).
    ///
    /// The Reciprocal Rank Fusion denominator: a result's contribution from a list is
    /// `weight / (rrf_k + rank)`. A larger `k` flattens the curve (rank position matters less); the
    /// canonical RRF default is 60. Clamped to `>= 1` on load so the denominator can never be zero.
    #[serde(default = "default_hybrid_rrf_k")]
    pub hybrid_rrf_k: u32,
    /// Weight applied to the lexical ranked list during RRF fusion (W-AI-6).
    ///
    /// Defaults to `1.0` (equal weight with semantic). Clamped to `[0.0, MAX_SEARCH_WEIGHT]` on load
    /// so a corrupt/hostile config can neither invert nor unboundedly dominate fusion. Zero disables
    /// the lexical list's RRF contribution (semantic-only ranking) without removing it as a recall set.
    #[serde(default = "default_search_weight")]
    pub lexical_weight: f32,
    /// Weight applied to the semantic ranked list during RRF fusion (W-AI-6).
    ///
    /// Defaults to `1.0` (equal weight with lexical). Clamped to `[0.0, MAX_SEARCH_WEIGHT]` on load.
    /// Zero disables the semantic list's RRF contribution (lexical-only ranking).
    #[serde(default = "default_search_weight")]
    pub semantic_weight: f32,
    /// BOUNDED additive boost applied to a starred result's normalized fusion score (W-AI-6, 05 §10).
    ///
    /// Added to the `[0, 1]`-normalized fusion score of a result whose page (or domain) is starred, so
    /// favorites rank higher WITHOUT turning semantic search into a bookmark list (05 §10:
    /// "過大 → 語義搜尋變書籤列表"). Conservative default (`0.15`) and clamped to
    /// `[0.0, MAX_STARRED_BOOST]` (`0.5`) on load: a single bounded bump can promote a *relevant*
    /// starred page but can never lift an irrelevant starred page above a strongly-relevant unstarred
    /// one (the normalized top is `1.0`, the boost cap is `0.5`). Zero disables the boost entirely.
    #[serde(default = "default_starred_boost")]
    pub starred_boost: f32,
}

/// Default RRF `k` constant (the canonical Reciprocal Rank Fusion value).
fn default_hybrid_rrf_k() -> u32 {
    60
}

/// Default RRF list weight (equal lexical/semantic blend).
fn default_search_weight() -> f32 {
    1.0
}

/// Default starred boost: a conservative bounded bump that promotes but never dominates.
fn default_starred_boost() -> f32 {
    0.15
}

/// Upper bound on a hybrid-fusion list weight (defensive clamp against a corrupt/hostile config).
pub const MAX_SEARCH_WEIGHT: f32 = 100.0;

/// Upper bound on the starred boost (05 §10 boundedness: keeps it below the normalized-score top of
/// `1.0` so an irrelevant favorite can never leapfrog a strongly-relevant unstarred result).
pub const MAX_STARRED_BOOST: f32 = 0.5;

impl AiSettings {
    /// Clamps the hybrid-search tuning knobs into their valid ranges (W-AI-6).
    ///
    /// Called from [`crate::config::normalize_app_config`] on every load so a hand-edited or older
    /// config can never feed an out-of-range value into fusion: `hybrid_rrf_k >= 1` (non-zero
    /// denominator), weights in `[0, MAX_SEARCH_WEIGHT]`, and `starred_boost` in
    /// `[0, MAX_STARRED_BOOST]`. NaN weights/boost reset to their conservative defaults rather than
    /// poisoning the score comparison. Idempotent — clamping an already-valid config is a no-op.
    pub fn normalize_search_knobs(&mut self) {
        self.hybrid_rrf_k = self.hybrid_rrf_k.max(1);
        self.lexical_weight = clamp_search_weight(self.lexical_weight, default_search_weight());
        self.semantic_weight = clamp_search_weight(self.semantic_weight, default_search_weight());
        self.starred_boost = if self.starred_boost.is_nan() {
            default_starred_boost()
        } else {
            self.starred_boost.clamp(0.0, MAX_STARRED_BOOST)
        };
    }
}

/// Clamps one fusion list weight into `[0, MAX_SEARCH_WEIGHT]`, resetting NaN to `fallback`.
fn clamp_search_weight(value: f32, fallback: f32) -> f32 {
    if value.is_nan() { fallback } else { value.clamp(0.0, MAX_SEARCH_WEIGHT) }
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
            // Hard-default-OFF: no site content is fetched until the user opts in (06 §2a).
            content_fetch_enabled: false,
            content_fetch_extractors: default_content_fetch_extractor_preferences(),
            content_fetch_domains: Vec::new(),
            llm_provider_id: None,
            embedding_provider_id: None,
            retrieval_top_k: 8,
            assistant_system_prompt: "You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.".to_string(),
            llm_providers: Vec::new(),
            embedding_providers: Vec::new(),
            hybrid_rrf_k: default_hybrid_rrf_k(),
            lexical_weight: default_search_weight(),
            semantic_weight: default_search_weight(),
            starred_boost: default_starred_boost(),
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
///
/// W-AI-7 additive fields (all `#[serde(default)]` so the frozen W-AI-1 plain-chat payload still
/// deserializes): `toolsEnabled` switches the run from plain streaming chat to the tool-executing
/// agent harness; `conversationId`/`messageId` link the durable agent run trace to the chat turn it
/// answers (used only on the agent path).
pub struct AiChatSendRequest {
    pub provider_id: Option<String>,
    pub messages: Vec<AiChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// When true, run the tool-executing agent harness instead of plain streaming chat (W-AI-7).
    #[serde(default)]
    pub tools_enabled: bool,
    /// Conversation this run answers (links the agent trace; agent path only).
    #[serde(default)]
    pub conversation_id: Option<String>,
    /// Message this run answers (links the agent trace; agent path only).
    #[serde(default)]
    pub message_id: Option<String>,
}

impl Default for AiChatSendRequest {
    /// Returns an empty default request used by shell forms before any input.
    fn default() -> Self {
        Self {
            provider_id: None,
            messages: Vec::new(),
            temperature: None,
            max_tokens: None,
            tools_enabled: false,
            conversation_id: None,
            message_id: None,
        }
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

// `Eq` is intentionally NOT derived: the additive W-AI-7 `Citations` variant carries `AiCitation`,
// whose `score: Option<f32>` is not `Eq`. `PartialEq` is enough for every consumer (`assert_eq!` /
// `matches!`); nothing pins a chunk in a `HashSet`/`BTreeMap` key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
/// One streamed chat chunk delivered over `pathkeep://ai-stream`.
///
/// Variants are tagged by `kind` so the front end can route tokens, reasoning, tool calls, the
/// terminal `done` marker, and a terminal `error` into distinct UI lanes without guessing.
///
/// W-AI-7 is strictly ADDITIVE: it appends `Usage` (per-turn token accounting for the budget
/// surface), `ToolResult` (the executed result the harness journals + streams), and `Citations`
/// (the run's accumulated evidence rows, emitted once right before `Done` so the front end can
/// render starrable evidence), and adds an optional `callId` to the existing `ToolCall` (so a
/// result can be correlated to its call). No existing variant is renamed/reordered and no existing
/// field is removed, so the W-AI-1 const-pin test and the plain (tools-off) streaming path stay
/// green.
pub enum AiChatStreamChunk {
    /// A fragment of the visible answer.
    Token { text: String },
    /// A fragment of the reasoning/thinking stream.
    Reasoning { text: String },
    /// A tool/function call the model requested. `callId` is `None` for the plain W-AI-1 path and
    /// `Some` when the agent harness (W-AI-7) reports the provider correlation id.
    ///
    /// `rename_all` is repeated on the variant because serde's container-level `rename_all` does
    /// NOT cascade to struct-variant fields; without it the additive `call_id` would wire as the
    /// snake_case `call_id`, breaking the FE camelCase contract.
    #[serde(rename_all = "camelCase")]
    ToolCall {
        name: String,
        arguments: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
    },
    /// The executed result of a tool call (W-AI-7). `callId` correlates to the originating
    /// `ToolCall`; `isError` is true when the tool failed and `result` is the honest error string
    /// (the run continues — one tool failure never aborts the whole run).
    #[serde(rename_all = "camelCase")]
    ToolResult { call_id: String, name: String, result: String, is_error: bool },
    /// Per-turn token accounting for the agent budget surface (W-AI-7).
    #[serde(rename_all = "camelCase")]
    Usage { prompt_tokens: u64, completion_tokens: u64 },
    /// The run's accumulated evidence rows (W-AI-7), emitted ONCE right before a terminal `Done`.
    ///
    /// Each citation carries its `canonicalUrl` (the W-STAR star key) so the front end renders
    /// starrable evidence rows without re-normalizing. Empty when no tool surfaced a row. This is
    /// the streaming twin of the harness's `record_citations` journal write (02 §G transparency).
    Citations { citations: Vec<AiCitation> },
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

// `Eq` dropped to match `AiChatStreamChunk` (its `Citations` variant is `PartialEq` only).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
/// Envelope emitted on `pathkeep://ai-stream` pairing a chunk with its run id.
pub struct AiChatStreamEvent {
    pub run_id: String,
    pub chunk: AiChatStreamChunk,
}

/// Tauri event channel that carries [`ModelDownloadProgressEvent`]s to the front end (W-AI-4b).
///
/// Defined here (a covered vault-core location) so a mutation of the literal is caught by a Rust
/// test; the front-end listener pins its own matching literal. Both sides MUST agree on this exact
/// string. Carries the consent-gated in-app embedding model download progress (§C.5).
pub const MODEL_DOWNLOAD_PROGRESS_EVENT: &str = "pathkeep://model-download-progress";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
/// One in-app embedding model download progress event delivered over
/// [`MODEL_DOWNLOAD_PROGRESS_EVENT`].
///
/// Mirrors the [`crate::ModelDownloadProgress`] trait callbacks so the worker can forward per-file
/// download progress to the UI without leaking hf-hub types across the IPC boundary. `kind`-tagged
/// so the front end can route each phase (started / finished / done / error) into the right UI state.
pub enum ModelDownloadProgressEvent {
    /// One model file started downloading (`total_bytes` is 0 when the size is unknown).
    FileStarted { file: String, total_bytes: u64 },
    /// One model file finished downloading + verifying.
    FileFinished { file: String },
    /// The whole model is present + verified; no more events follow for this run.
    Done,
    /// The download failed; carries a user-facing message. No more events follow.
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
/// One pinned evidence row reconstructed for a reopened assistant turn (W-AI-7 WU-7).
///
/// Mirrors the front-end `AiChatCitation` shape so a reopened conversation renders the SAME
/// starrable evidence rows the live turn streamed. The fields come from the durable `agent_citations`
/// journal (keyed by the run that answered this message), so the evidence — and its W-STAR star key —
/// survives a reopen exactly as it survived a crash. `profile_id` is not journaled with a citation
/// (the star key is the canonical url, not the profile), so it is empty on the reconstructed row;
/// every consumer (the evidence panel) keys off `canonical_url` / `url`, never `profile_id`.
pub struct AgentCitation {
    pub history_id: i64,
    /// Empty on a reconstructed citation: the agent_citations journal does not retain the profile.
    #[serde(default)]
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    /// ISO visit time; empty string when the journaled row had no timestamp (degrade, never drop).
    #[serde(default)]
    pub visited_at: String,
    pub score: Option<f32>,
    /// W-STAR star key (canonicalized url); the agent_citations journal always pins one.
    pub canonical_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Per-turn token accounting reconstructed for a reopened assistant turn (W-AI-7 WU-7).
///
/// Mirrors the front-end `AssistantUsage`; sourced from the `agent_runs` header (the run that
/// answered this message), so the reopened turn shows the same token footer the live turn did.
pub struct AgentUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
/// One persisted assistant-chat message as stored in `derived/agent.sqlite`.
///
/// Mirrors the front-end `ChatMessage` shape (id/role/content/reasoning/toolCalls/status) so a
/// past conversation can rehydrate the streaming chat hook verbatim. `toolCallsJson` is the
/// serialized `AssistantToolCall[]` exactly as the UI rendered it; persisting it as opaque JSON
/// keeps the agent sidecar decoupled from the tool schema (which W-AI-7 will evolve).
///
/// W-AI-7 WU-7 makes the full agent trace durable across reopen. `citations` and `usage` are NOT
/// stored on the message row — they live in the parallel agent run/citation journal — so they are
/// read-only RECONSTRUCTION fields, populated by [`crate::agent_store::load_conversation`] (which
/// joins the message's run → its citations + token tally) and ignored on save. They are
/// `#[serde(default, skip_serializing_if)]` so the persist-on-finalize save payload, which never
/// carries them, round-trips byte-for-byte unchanged (the W-AI-3 conversation-replace contract).
pub struct AgentMessage {
    pub id: String,
    /// `"user"` or `"assistant"` (the persisted transcript roles).
    pub role: String,
    pub content: String,
    /// Accumulated reasoning/thinking text (assistant turns only).
    pub reasoning: Option<String>,
    /// Serialized `AssistantToolCall[]` JSON (assistant turns only).
    pub tool_calls_json: Option<String>,
    /// Terminal turn status (`done` / `error` / `cancelled`); absent for user messages.
    pub status: Option<String>,
    /// Reconstructed evidence rows for this assistant turn (load-only; absent on save). Empty when
    /// the turn's run pinned no citations (or no run answered this message).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<AgentCitation>,
    /// Reconstructed per-turn token usage (load-only; absent on save). `None` when no run answered
    /// this message or the run never tallied tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<AgentUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Lightweight conversation row for the chat-history explorer list (no messages).
///
/// Bounded list reads return these (title + recency + size) so the explorer never loads message
/// bodies it will not show; the full transcript is fetched lazily by [`AgentConversationDetail`].
pub struct AgentConversationSummary {
    pub id: String,
    pub title: String,
    /// LLM provider id active when the conversation was saved (for display only; never a model id).
    pub provider_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

// `Eq` is intentionally NOT derived: the WU-7 reconstruction fields on `AgentMessage`
// (`citations`/`usage`) carry `Option<f32>` scores, which are not `Eq`. `PartialEq` covers every
// consumer (`assert_eq!` in tests); nothing keys a detail in a hash/btree set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
/// One conversation plus its full (bounded) message transcript.
pub struct AgentConversationDetail {
    #[serde(flatten)]
    pub summary: AgentConversationSummary,
    pub messages: Vec<AgentMessage>,
}

// `Eq` dropped to match `AgentMessage` (its WU-7 reconstruction fields carry `Option<f32>`). The
// save payload never carries those fields (they `skip_serializing_if`), so the request is unchanged
// on the wire; only the derive narrows from `Eq` to `PartialEq`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
/// Request payload for `save_ai_conversation`: upsert a conversation + replace its messages.
///
/// When `title` is `None`/blank the store derives one from the first user message. The full
/// message list is sent on every save (persist-on-finalize, not per chunk) and atomically
/// replaces the prior transcript for this conversation id.
pub struct SaveAgentConversationRequest {
    pub id: String,
    pub title: Option<String>,
    pub provider_id: Option<String>,
    pub messages: Vec<AgentMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Request payload for `list_ai_conversations`: a bounded, newest-first page cap.
pub struct ListAgentConversationsRequest {
    /// Maximum conversations to return (newest-first); the store clamps this to a safe ceiling.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Response for `list_ai_conversations`.
pub struct AgentConversationListResponse {
    pub conversations: Vec<AgentConversationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Request payload for `rename_ai_conversation`.
pub struct RenameAgentConversationRequest {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Result of `delete_ai_conversation`: whether a row with the id existed and was removed.
pub struct DeleteAgentConversationResult {
    pub deleted: bool,
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
    /// The `is:starred` facet (W-AI-6): restrict BOTH lexical AND semantic recall to starred pages.
    ///
    /// Mirrors the lexical Explorer `is:starred` facet but pushes the constraint into the semantic
    /// plane too via the [`crate::ai::VectorIndex`] allowlist seam, so meaning-based search over
    /// favorites is honest (today the lexical browse facet only constrains the keyword path). `None`
    /// or `Some(false)` is the unfiltered default; `Some(true)` activates the allowlist. Optional with
    /// `#[serde(default)]` so older shell payloads without the field still deserialize.
    #[serde(default)]
    pub starred_only: Option<bool>,
}

impl Default for AiSearchRequest {
    /// Returns the default search request used by empty shell forms.
    fn default() -> Self {
        Self {
            query: String::new(),
            profile_id: None,
            domain: None,
            limit: Some(8),
            cursor: None,
            starred_only: None,
        }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
/// Citation for one visit used in an assistant answer or insight.
///
/// `canonical_url` (W-AI-7, additive) is the W-STAR star key, resolved at the agent tool-call site
/// via `visit_taxonomy::normalize_visit_url` so the front end can star a cited page directly from a
/// streamed `Citations` chunk without re-normalizing. It is `#[serde(default, skip_serializing_if =
/// "Option::is_none")]` so every pre-W-AI-7 payload (insights, the old assistant response) still
/// serializes/deserializes byte-for-byte unchanged.
pub struct AiCitation {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub visited_at: String,
    pub score: Option<f32>,
    /// W-STAR star key (canonicalized URL); `None` on the legacy/insight paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_url: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// One stored site-content enrichment for a visit/URL, FE-facing (W-ENRICH-1 detail panel, 06 §6).
///
/// The capped `summary` + structured `metadata` are what the detail panel renders; the full body
/// stays in the content-addressed blob and is NOT shipped here (the FE shows the summary, not the
/// multi-KB body). `fetchStatus` is honest (success | empty | blocked | fetch-error | …) so the panel
/// can show a real failure state instead of pretending.
pub struct VisitEnrichmentRecord {
    pub content_source: String,
    pub fetch_status: String,
    pub fetched_at: String,
    pub readable_title: Option<String>,
    pub summary: Option<String>,
    pub extractor_version: Option<i64>,
    /// Structured extraction JSON (GitHub topics/desc/stars, video channel, …) as an opaque string the
    /// FE parses for chips. Kept as a string so this model stays decoupled from per-extractor schema.
    pub metadata_json: Option<String>,
    pub final_url: Option<String>,
    pub http_status: Option<i64>,
    pub refetch_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Settings-facing content-fetch consent + status surface (W-ENRICH-1, 06 §6).
///
/// Mirrors the persisted consent flags so the Settings panel can bind them, plus a small live status
/// (queued/running/failed counts) so the panel shows fetch progress without a separate query.
pub struct ContentFetchSettings {
    /// Master switch (hard-default-OFF). When false the whole content-fetch plane is inert.
    pub enabled: bool,
    /// Per-extractor toggles (GitHub repo metadata, generic readable summary).
    pub extractors: Vec<ContentFetchExtractorPreference>,
    /// Per-domain allow/block rules.
    pub domains: Vec<ContentFetchDomainRule>,
    /// Queued content-fetch jobs awaiting a drain.
    pub queued_jobs: usize,
    /// Currently-running content-fetch jobs.
    pub running_jobs: usize,
    /// Content-fetch jobs in a failed terminal state.
    pub failed_jobs: usize,
    /// Stored content-fetch enrichment rows (success + negative-cache markers).
    pub stored_records: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Request payload for the manual "fetch now" PME trigger (W-ENRICH-1, 06 §6).
pub struct ContentFetchNowRequest {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
/// Result of a manual "fetch now" enqueue (W-ENRICH-1).
pub struct ContentFetchNowResult {
    /// The enqueued (or refreshed) job id.
    pub job_id: i64,
    /// The job state after enqueue (`queued` / `running` / `disabled`).
    pub state: String,
    /// Localised-key-friendly note (the FE maps to copy; never raw prose committed to the contract).
    pub note: String,
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
        // W-AI-1 wire shape is PINNED: a `ToolCall` with no `callId` serializes exactly as before
        // (the additive `callId` is `skip_serializing_if = None`), proving W-AI-7 is additive.
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::ToolCall {
                name: "search".to_string(),
                arguments: "{}".to_string(),
                call_id: None,
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
    fn ai_chat_stream_chunk_w_ai_7_additive_variants_serialize_with_kind_tag() {
        // The agent harness emits a `callId` when the provider reports one.
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::ToolCall {
                name: "search".to_string(),
                arguments: "{}".to_string(),
                call_id: Some("call-1".to_string()),
            })
            .unwrap(),
            json!({ "kind": "toolCall", "name": "search", "arguments": "{}", "callId": "call-1" })
        );
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::ToolResult {
                call_id: "call-1".to_string(),
                name: "search".to_string(),
                result: "3 rows".to_string(),
                is_error: false,
            })
            .unwrap(),
            json!({ "kind": "toolResult", "callId": "call-1", "name": "search", "result": "3 rows", "isError": false })
        );
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::Usage {
                prompt_tokens: 12,
                completion_tokens: 7,
            })
            .unwrap(),
            json!({ "kind": "usage", "promptTokens": 12, "completionTokens": 7 })
        );
        // The agent emits the run's accumulated evidence once before Done; each citation carries its
        // canonicalUrl (the W-STAR star key) so the FE renders starrable evidence rows.
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::Citations {
                citations: vec![AiCitation {
                    history_id: 7,
                    profile_id: "p".to_string(),
                    url: "https://a.example/x".to_string(),
                    title: Some("X".to_string()),
                    visited_at: "2026-01-01T00:00:00Z".to_string(),
                    score: Some(0.5),
                    canonical_url: Some("https://a.example/x".to_string()),
                }],
            })
            .unwrap(),
            json!({
                "kind": "citations",
                "citations": [{
                    "historyId": 7,
                    "profileId": "p",
                    "url": "https://a.example/x",
                    "title": "X",
                    "visitedAt": "2026-01-01T00:00:00Z",
                    "score": 0.5,
                    "canonicalUrl": "https://a.example/x",
                }],
            })
        );
        // An empty Citations chunk (no tool surfaced a row) is still well-formed.
        assert_eq!(
            serde_json::to_value(AiChatStreamChunk::Citations { citations: vec![] }).unwrap(),
            json!({ "kind": "citations", "citations": [] })
        );
    }

    #[test]
    fn ai_citation_omits_canonical_url_on_the_legacy_path() {
        // The additive `canonicalUrl` is `skip_serializing_if = None`, so a legacy/insight citation
        // (resolved without a canonical url) serializes EXACTLY as it did before W-AI-7.
        let legacy = AiCitation {
            history_id: 3,
            profile_id: "p".to_string(),
            url: "https://b.example/".to_string(),
            title: None,
            visited_at: "2026-02-02T00:00:00Z".to_string(),
            score: None,
            canonical_url: None,
        };
        let value = serde_json::to_value(&legacy).unwrap();
        assert_eq!(
            value,
            json!({
                "historyId": 3,
                "profileId": "p",
                "url": "https://b.example/",
                "title": null,
                "visitedAt": "2026-02-02T00:00:00Z",
                "score": null,
            })
        );
        assert!(value.get("canonicalUrl").is_none());
        // And a payload without the key round-trips back to `None`.
        let parsed: AiCitation = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.canonical_url, None);
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
            ..AiChatSendRequest::default()
        })
        .unwrap();
        assert_eq!(value["providerId"], json!("p1"));
        assert_eq!(value["maxTokens"], json!(64));
        // The nested message also uses camelCase role/content keys.
        assert_eq!(value["messages"][0], json!({ "role": "user", "content": "hi" }));
        // W-AI-7 additive flag is camelCase and defaults to false (plain chat).
        assert_eq!(value["toolsEnabled"], json!(false));
    }

    #[test]
    fn ai_chat_send_request_deserializes_legacy_w_ai_1_payload() {
        // The frozen W-AI-1 payload (no toolsEnabled/conversationId/messageId) still deserializes,
        // defaulting to plain streaming chat — proving the W-AI-7 fields are additive.
        let legacy = json!({
            "providerId": "p1",
            "messages": [{ "role": "user", "content": "hi" }],
            "temperature": 0.5,
            "maxTokens": 64
        });
        let request: AiChatSendRequest = serde_json::from_value(legacy).expect("legacy payload");
        assert!(!request.tools_enabled);
        assert_eq!(request.conversation_id, None);
        assert_eq!(request.message_id, None);
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

    #[test]
    fn content_fetch_domain_blocked_matches_case_insensitively_and_ignores_blank() {
        let mut config = AppConfig::default();
        config.ai.content_fetch_domains = vec![
            ContentFetchDomainRule { domain: "Blocked.COM".to_string(), allowed: false },
            ContentFetchDomainRule { domain: "allowed.com".to_string(), allowed: true },
        ];
        // Case-insensitive block match.
        assert!(content_fetch_domain_blocked(&config, "blocked.com"));
        assert!(content_fetch_domain_blocked(&config, "  BLOCKED.com "));
        // An explicit-allow rule is not a block; an unlisted domain is not blocked.
        assert!(!content_fetch_domain_blocked(&config, "allowed.com"));
        assert!(!content_fetch_domain_blocked(&config, "other.com"));
        // A blank/whitespace domain is never blocked (covers the empty-domain early-out).
        assert!(!content_fetch_domain_blocked(&config, ""));
        assert!(!content_fetch_domain_blocked(&config, "   "));
    }

    #[test]
    fn content_extractor_enabled_defaults_on_and_honors_explicit_disable() {
        let mut config = AppConfig::default();
        // No explicit preference → enabled by default.
        assert!(content_extractor_enabled(&config, "github-repo"));
        // An explicit disable turns it off; an unrelated extractor stays on.
        config.ai.content_fetch_extractors = vec![ContentFetchExtractorPreference {
            extractor_id: "github-repo".to_string(),
            enabled: false,
        }];
        assert!(!content_extractor_enabled(&config, "github-repo"));
        assert!(content_extractor_enabled(&config, "generic-readable"));
    }

    #[test]
    fn ai_settings_search_knobs_default_to_conservative_values() {
        // W-AI-6: the hybrid-search knobs ship at safe, equal-weight, bounded defaults.
        let settings = AiSettings::default();
        assert_eq!(settings.hybrid_rrf_k, 60, "RRF k defaults to the canonical 60");
        assert_eq!(settings.lexical_weight, 1.0);
        assert_eq!(settings.semantic_weight, 1.0);
        assert_eq!(settings.starred_boost, 0.15, "the starred boost is conservative by default");
    }

    #[test]
    fn ai_settings_search_knobs_serialize_camel_case() {
        // The FE binds these later (W-AI-9), so the wire keys must be camelCase like the rest.
        let value = serde_json::to_value(AiSettings::default()).unwrap();
        assert_eq!(value["hybridRrfK"], json!(60));
        assert_eq!(value["lexicalWeight"].as_f64().unwrap(), 1.0);
        assert_eq!(value["semanticWeight"].as_f64().unwrap(), 1.0);
        // f32 0.15 widens to a non-exact f64; compare with tolerance rather than a literal.
        assert!((value["starredBoost"].as_f64().unwrap() - 0.15).abs() < 1e-6);
        // The camelCase keys must exist (not snake_case) so the FE binding contract holds.
        assert!(value.get("starredBoost").is_some());
    }

    #[test]
    fn ai_settings_search_knobs_deserialize_from_older_payload_without_them() {
        // Older configs lack the W-AI-6 knobs; `#[serde(default)]` must fill them with the defaults so
        // an upgrade never fails to parse and never silently zeroes the RRF denominator.
        let older = json!({});
        let settings: AiSettings = serde_json::from_value(older).unwrap();
        assert_eq!(settings.hybrid_rrf_k, 60);
        assert_eq!(settings.lexical_weight, 1.0);
        assert_eq!(settings.semantic_weight, 1.0);
        assert_eq!(settings.starred_boost, 0.15);
    }

    #[test]
    fn normalize_search_knobs_clamps_into_valid_ranges() {
        // A hand-edited / hostile config must be clamped: k >= 1 (non-zero denominator), weights into
        // [0, MAX_SEARCH_WEIGHT], boost into [0, MAX_STARRED_BOOST]. Negative + over-cap + zero-k all hit.
        let mut settings = AiSettings {
            hybrid_rrf_k: 0,
            lexical_weight: -5.0,
            semantic_weight: 1_000.0,
            starred_boost: 9.0,
            ..AiSettings::default()
        };
        settings.normalize_search_knobs();
        assert_eq!(
            settings.hybrid_rrf_k, 1,
            "k=0 is lifted to 1 so the RRF denominator is non-zero"
        );
        assert_eq!(settings.lexical_weight, 0.0, "a negative weight clamps up to 0");
        assert_eq!(settings.semantic_weight, MAX_SEARCH_WEIGHT, "an over-cap weight clamps down");
        assert_eq!(
            settings.starred_boost, MAX_STARRED_BOOST,
            "an over-cap boost clamps to the cap"
        );
    }

    #[test]
    fn normalize_search_knobs_resets_nan_weights_and_boost_to_defaults() {
        // NaN must never reach the score comparison; it resets to the conservative default.
        let mut settings = AiSettings {
            lexical_weight: f32::NAN,
            semantic_weight: f32::NAN,
            starred_boost: f32::NAN,
            ..AiSettings::default()
        };
        settings.normalize_search_knobs();
        assert_eq!(settings.lexical_weight, 1.0);
        assert_eq!(settings.semantic_weight, 1.0);
        assert_eq!(settings.starred_boost, 0.15);
    }

    #[test]
    fn normalize_search_knobs_is_idempotent_for_valid_values() {
        // Clamping an already-valid config is a no-op (covers the in-range branches).
        let mut settings = AiSettings {
            hybrid_rrf_k: 30,
            lexical_weight: 0.5,
            semantic_weight: 2.0,
            starred_boost: 0.25,
            ..AiSettings::default()
        };
        settings.normalize_search_knobs();
        assert_eq!(settings.hybrid_rrf_k, 30);
        assert_eq!(settings.lexical_weight, 0.5);
        assert_eq!(settings.semantic_weight, 2.0);
        assert_eq!(settings.starred_boost, 0.25);
    }

    #[test]
    fn ai_search_request_starred_only_round_trips_and_defaults_absent() {
        // The `is:starred` facet field serializes camelCase and tolerates an older payload omitting it.
        let value = serde_json::to_value(AiSearchRequest {
            starred_only: Some(true),
            ..AiSearchRequest::default()
        })
        .unwrap();
        assert_eq!(value["starredOnly"], json!(true));
        let older: AiSearchRequest = serde_json::from_value(json!({ "query": "rust" })).unwrap();
        assert_eq!(older.starred_only, None, "absent facet defaults to None (unfiltered)");
    }
}
