//! Intelligence, queue, and derived-state worker flows.
//!
//! ## Responsibilities
//! - expose the worker-facing AI queue, deterministic rebuild, and read-model helpers
//! - keep shared worker counters and section-meta helpers in one place
//! - re-export focused owner modules so `vault-worker` stays a thin facade
//!
//! ## Not responsible for
//! - canonical archive schema or deterministic rebuild algorithms
//! - Tauri command naming and desktop IPC payload design
//! - platform adapters, keyring plumbing, or archive ingest orchestration
//!
//! ## Dependencies
//! - `crate::context` for unlocked config access
//! - `vault_core::intelligence` for deterministic read-model and rebuild logic
//! - child modules `ai_queue` and `runtime` for heavy worker orchestration
//!
//! ## Performance notes
//! - section helpers only read enough runtime metadata to label surfaces honestly
//! - background worker counts stay in shared atomics so the worker never fans out
//!   unbounded concurrency on a 4-core host

mod agent_store;
mod ai_queue;
mod chat;
mod model_download;
mod route_queries;
mod runtime;
mod section_queries;

use crate::context::load_unlocked_config;
use anyhow::Result;
use chrono::Local;
use std::sync::atomic::AtomicUsize;
use vault_core::{
    ActivityMix, ActivityMixTrend, AppConfig, BreadthIndex, BrowserDiff,
    CategoryFilteredDateRangeRequest, CompareSet, CompareSetDetail, CompareSetDetailRequest,
    CoreIntelligencePrimaryOverview, CoreIntelligenceSecondaryOverview,
    CoreIntelligenceSectionResult, CoreIntelligenceSectionWindow, DayInsights, DayInsightsRequest,
    DigestSummary, DiscoveryTrend, DomainDeepDive, DomainDeepDiveRequest, DomainTrend,
    DomainTrendRequest, EngineRanking, EntityExplanationRequest, Explanation, FrictionSignal,
    GranularityDateRangeRequest, HabitPattern, HubPage, IntelligenceEmbedCardPayload,
    IntelligenceEmbedCardsRequest, IntelligenceLocalHostBuildResult, IntelligenceLocalHostPreview,
    IntelligenceLocalHostRequest, IntelligencePublicSnapshot, IntelligenceWidgetSnapshot,
    InterruptedHabit, NavigationPath, ObservedInteraction, OnThisDayEntry, PagedDateRangeRequest,
    PathFlow, PathFlowRequest, ProfileScopedRequest, QueryFamilyDetail, QueryFamilyDetailRequest,
    QueryFamilyResult, RefindExplanation, RefindPage, RefindPageDetail, RefindPageDetailRequest,
    RefindPagesRequest, ReopenedInvestigation, RhythmHeatmap, ScopedDateRangeRequest,
    SearchConcept, SearchEffectiveness, SearchEffectivenessRequest, SearchEngineRule,
    SearchEngineRuleInput, SearchQueryListRequest, SearchQueryListResult, SearchTrailQueryRequest,
    SessionDetail, SessionListResult, StableSource, TopSearchConceptsRequest, TopSite,
    TopSitesRequest, TrailDetail, TrailListResult, build_core_intelligence_section_meta,
    intelligence,
};

pub use self::agent_store::{
    delete_ai_conversation, list_ai_conversations, load_ai_conversation, rename_ai_conversation,
    save_ai_conversation,
};
pub(crate) use self::ai_queue::maybe_spawn_ai_queue_drain;
pub use self::ai_queue::{
    ask_ai_assistant, build_ai_index_now, cancel_ai_job, load_ai_assistant_job, load_ai_queue,
    preview_ai_integration_files, replay_ai_job, run_ai_queue_jobs, search_ai_history,
    test_ai_provider_connection_report,
};
#[cfg(all(test, coverage))]
pub(crate) use self::ai_queue::{
    complete_claimed_assistant_job, complete_claimed_index_job, drain_one_ai_queue_job,
    start_ai_job_control,
};
pub use self::chat::{ai_chat_cancel, ai_chat_send};
pub use self::model_download::{cancel_model_download, download_ai_embedding_model};
pub use self::route_queries::{
    delete_search_engine_rule, explain_entity, explain_refind, get_domain_trend, get_hub_pages,
    get_intelligence_primary_overview, get_navigation_path, get_query_families,
    get_query_family_detail, get_refind_page_detail, get_refind_pages, get_search_engine_ranking,
    get_search_queries, get_search_trails, get_session_detail, get_sessions,
    get_top_search_concepts, get_top_sites, get_trail_detail, list_search_engine_rules,
    upsert_search_engine_rule,
};
pub(crate) use self::runtime::maybe_spawn_intelligence_queue_drain;
pub use self::runtime::{
    cancel_intelligence_job_now, load_intelligence_runtime_snapshot,
    queue_core_intelligence_rebuild, retry_intelligence_job_now, run_core_intelligence_now,
};
#[cfg(all(test, coverage))]
pub(crate) use self::runtime::{
    drain_one_enrichment_intelligence_job, drain_one_priority_intelligence_job,
    execute_core_intelligence_job,
};
pub use self::section_queries::{
    build_intelligence_local_host, get_activity_mix, get_activity_mix_trend, get_breadth_index,
    get_browsing_rhythm, get_compare_set_detail, get_compare_sets, get_day_insights,
    get_digest_summary, get_discovery_trend, get_domain_deep_dive, get_friction_signals,
    get_habit_patterns, get_intelligence_embed_cards, get_intelligence_public_snapshot,
    get_intelligence_secondary_overview, get_intelligence_widget_snapshot, get_interrupted_habits,
    get_multi_browser_diff, get_observed_interactions, get_on_this_day, get_path_flows,
    get_reopened_investigations, get_search_effectiveness, get_stable_sources,
    preview_intelligence_local_host,
};

static AI_QUEUE_ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);
static INTELLIGENCE_PRIORITY_WORKERS: AtomicUsize = AtomicUsize::new(0);
static INTELLIGENCE_ENRICHMENT_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// Opens the unlocked project context required by deterministic intelligence reads.
///
/// The worker uses this helper so every read-model wrapper resolves paths and config
/// the same way, without each function reimplementing archive bootstrap logic.
fn with_core_intelligence<R>(
    _session_database_key: Option<&str>,
    f: impl FnOnce(&vault_core::ProjectPaths, &AppConfig) -> Result<R>,
) -> Result<R> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    f(&paths, &config)
}

/// Wraps one Core Intelligence section payload with the persisted runtime metadata.
///
/// The UI needs the section payload and its freshness/empty-state context together.
/// Centralizing that composition here prevents every worker wrapper from drifting on
/// `section_id`, window semantics, or what qualifies as an empty dataset.
fn with_core_intelligence_section<R>(
    session_database_key: Option<&str>,
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    fetch: impl FnOnce(&vault_core::ProjectPaths, &AppConfig) -> Result<R>,
    is_empty: impl FnOnce(&R) -> bool,
) -> Result<CoreIntelligenceSectionResult<R>> {
    with_core_intelligence(session_database_key, |paths, config| {
        let data = fetch(paths, config)?;
        let meta = build_core_intelligence_section_meta(
            paths,
            config,
            session_database_key,
            section_id,
            window,
            is_empty(&data),
        )?;
        Ok(CoreIntelligenceSectionResult { data, meta })
    })
}
