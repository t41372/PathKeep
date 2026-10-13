//! Failure modes: wrappers bypass the session Vault or queue workers outlive shutdown accounting.
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
mod content_fetch;
mod model_download;
mod route_queries;
mod runtime;
mod section_queries;

use crate::job_runtime::WorkerPool;
use anyhow::Result;
use vault_core::{
    ActivityMix, ActivityMixTrend, BreadthIndex, BrowserDiff, CategoryFilteredDateRangeRequest,
    CompareSet, CompareSetDetail, CompareSetDetailRequest, CoreIntelligencePrimaryOverview,
    CoreIntelligenceSecondaryOverview, CoreIntelligenceSectionResult,
    CoreIntelligenceSectionWindow, DayInsights, DayInsightsRequest, DigestSummary, DiscoveryTrend,
    DomainDeepDive, DomainDeepDiveRequest, DomainTrend, DomainTrendRequest, EngineRanking,
    EntityExplanationRequest, Explanation, FrequentSearch, FrequentSearchesRequest, FrictionSignal,
    GranularityDateRangeRequest, HabitPattern, HubPage, IntelligenceEmbedCardPayload,
    IntelligenceEmbedCardsRequest, IntelligenceLocalHostBuildResult, IntelligenceLocalHostPreview,
    IntelligenceLocalHostRequest, IntelligencePublicSnapshot, IntelligenceWidgetSnapshot,
    InterruptedHabit, NavigationPath, ObservedInteraction, OnThisDayEntry, PagedDateRangeRequest,
    PathFlow, PathFlowRequest, ProfileScopedRequest, QueryFamilyDetail, QueryFamilyDetailRequest,
    QueryFamilyResult, RefindExplanation, RefindPage, RefindPageDetail, RefindPageDetailRequest,
    RefindPagesRequest, ReopenedInvestigation, RhythmHeatmap, ScopedDateRangeRequest,
    SearchConcept, SearchEffectiveness, SearchEffectivenessRequest, SearchEngineRule,
    SearchEngineRuleInput, SearchFamiliesRequest, SearchQueryListRequest, SearchQueryListResult,
    SearchTrailQueryRequest, SessionDetail, SessionListResult, StableSource,
    TopSearchConceptsRequest, TopSite, TopSitesRequest, TrailDetail, TrailListResult,
    build_core_intelligence_section_meta, intelligence,
};

pub use self::agent_store::{
    delete_ai_conversation, list_ai_conversations, list_ai_messages, load_ai_conversation,
    rename_ai_conversation, save_ai_conversation,
};
pub(crate) use self::ai_queue::maybe_spawn_ai_queue_drain;
pub use self::ai_queue::{
    ask_ai_assistant, build_ai_index_now, cancel_ai_job, estimate_reembed_now,
    list_ai_provider_models, load_ai_assistant_job, load_ai_queue, preview_ai_integration_files,
    replay_ai_job, reset_ai_index_build, run_ai_queue_jobs, search_ai_history,
    test_ai_provider_connection_report,
};
#[cfg(all(test, coverage))]
pub(crate) use self::ai_queue::{
    complete_claimed_assistant_job, complete_claimed_index_job, drain_one_ai_queue_job,
    start_ai_job_control,
};
pub use self::chat::{ai_chat_cancel, ai_chat_send};
pub(crate) use self::content_fetch::maybe_spawn_content_fetch_drain;
pub use self::content_fetch::{
    content_fetch_now, content_fetch_settings, enqueue_content_fetch_working_set,
    list_visit_enrichment, set_content_fetch_settings,
};
pub use self::model_download::{cancel_model_download, download_static_embedding_model};
pub use self::route_queries::{
    delete_search_engine_rule, explain_entity, explain_refind, get_domain_trend,
    get_frequent_searches, get_hub_pages, get_insights_time_zone,
    get_intelligence_primary_overview, get_navigation_path, get_query_families,
    get_query_family_detail, get_refind_page_detail, get_refind_pages, get_search_engine_ranking,
    get_search_families, get_search_queries, get_search_trails, get_session_detail, get_sessions,
    get_sites_in_category, get_top_search_concepts, get_top_sites, get_trail_detail,
    list_search_engine_rules, upsert_search_engine_rule,
};
pub use self::runtime::{
    cancel_intelligence_job_now, load_intelligence_runtime_snapshot,
    queue_core_intelligence_rebuild, retry_intelligence_job_now, run_core_intelligence_now,
    set_site_category,
};
#[cfg(all(test, coverage))]
pub(crate) use self::runtime::{
    drain_one_enrichment_intelligence_job, drain_one_priority_intelligence_job,
    execute_core_intelligence_job,
};
pub(crate) use self::runtime::{
    maybe_spawn_intelligence_queue_drain, rebuild_in_chosen_zone, resume_insights,
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

static AI_QUEUE_ACTIVE_WORKERS: WorkerPool = WorkerPool::new();
static INTELLIGENCE_PRIORITY_WORKERS: WorkerPool = WorkerPool::new();
static INTELLIGENCE_ENRICHMENT_WORKERS: WorkerPool = WorkerPool::new();

/// Queue workers running in this process: AI queue, both intelligence lanes and content fetch.
/// "Delete all data" waits for this to reach zero before removing the databases they write.
pub(crate) fn running_background_workers() -> usize {
    [
        &AI_QUEUE_ACTIVE_WORKERS,
        &INTELLIGENCE_PRIORITY_WORKERS,
        &INTELLIGENCE_ENRICHMENT_WORKERS,
        &content_fetch::CONTENT_FETCH_WORKERS,
    ]
    .iter()
    .map(|workers| workers.active())
    .sum()
}

/// Keeps route wrappers independent of storage details.
fn with_core_intelligence<R>(
    vault: &vault_core::Vault,
    f: impl FnOnce(&vault_core::Vault) -> Result<R>,
) -> Result<R> {
    f(vault)
}

/// Reads section data and its runtime metadata through the same owner, with the visits or
/// searches the card rests on in `profile`'s part of the window.
fn with_core_intelligence_section<R>(
    vault: &vault_core::Vault,
    section_id: &str,
    window: CoreIntelligenceSectionWindow,
    profile: Option<&str>,
    fetch: impl FnOnce(&vault_core::Vault) -> Result<R>,
    is_empty: impl FnOnce(&R) -> bool,
) -> Result<CoreIntelligenceSectionResult<R>> {
    let data = fetch(vault)?;
    let mut meta =
        build_core_intelligence_section_meta(vault, section_id, window, is_empty(&data))?;
    intelligence::fill_section_evidence(vault, &mut meta, profile)?;
    Ok(CoreIntelligenceSectionResult { data, meta })
}

/// Continues AI history beyond the status preview without starting a worker.
pub fn list_ai_jobs(
    vault: &vault_core::Vault,
    request: &vault_core::ListAiJobsRequest,
) -> Result<vault_core::ai_queue::AiJobPage> {
    vault_core::ai_queue::list_ai_jobs(vault, request.cursor, request.limit.unwrap_or(20) as usize)
}
/// Continues deterministic and enrichment job history on the same owner.
pub fn list_intelligence_jobs(
    vault: &vault_core::Vault,
    request: &vault_core::ListIntelligenceJobsRequest,
) -> Result<vault_core::IntelligenceJobPage> {
    vault_core::list_intelligence_jobs(
        vault,
        request.cursor.clone(),
        request.limit.unwrap_or(20) as usize,
    )
}
