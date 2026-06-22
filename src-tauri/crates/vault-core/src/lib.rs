//! Canonical backend domain crate for PathKeep.
//!
//! `vault-core` owns the source-of-truth backend rules: archive schema and
//! recoverability, browser/profile read models, optional AI and Core
//! Intelligence, remote backup verification, and the shared serde models that
//! the desktop shell consumes.
//!
//! What this crate does not own:
//!
//! - native OS integrations such as keyrings or schedulers
//! - Tauri command naming and transport concerns
//! - worker orchestration glue that combines multiple subsystems for one UI
//!   action

pub mod agent_store;
pub mod ai;
pub mod ai_queue;
pub mod ai_sidecar;
pub mod annotations;
pub mod app_lock;
pub mod archive;
mod browser_retention;
pub mod chrome;
pub mod config;
pub mod diagnostics;
mod enrichment;
pub mod git_audit;
pub mod intelligence;
mod intelligence_blobs;
mod intelligence_catalog;
pub mod intelligence_runtime;
mod intelligence_sections;
pub mod migration;
pub mod models;
pub mod stars;
pub mod takeout;
pub mod utils;
pub mod visit_taxonomy;

pub use agent_store::{
    AgentCitationRecord, AgentRunStatus, AgentRunTrace, AgentStepRecord, AppendAgentStep,
    BeginAgentRun, append_agent_step, begin_agent_run, delete_conversation, ensure_agent_schema,
    finalize_agent_run, list_conversations, load_agent_run, load_conversation,
    open_agent_connection, record_agent_citations, rename_conversation, save_conversation,
};
pub use ai::{
    ALLOWLIST_EXPANSION, AgentJournal, AgentRunOutcome, AgentToolContext, AiIntegrationPreview,
    AiProviderRuntime, AiRunCancelled, AiRunControl, AnyEmbeddingProvider, BinaryPlane,
    CANDLE_INAPP_BASE_URL, CandidateSignals, CandleEmbeddingProvider, DEFAULT_CANDLE_MODEL_FILES,
    DEFAULT_CANDLE_MODEL_REPO, DEFAULT_CANDLE_QUANT, DEFAULT_CANDLE_TOKENIZER_REPO,
    DEFAULT_MAX_ITERATIONS, DEFAULT_STATIC_MODEL_FILES, DEFAULT_STATIC_MODEL_REPO,
    DEFAULT_TOKEN_BUDGET, EMBEDDING_FINGERPRINT_VERSION, EmbeddingDescriptor, EmbeddingDtype,
    EmbeddingFingerprint, EmbeddingPooling, EmbeddingProvider, EmbeddingRole,
    ExternalEmbeddingProvider, FlatVectorIndex, IndexBackfillLedger, IndexBackfillProgress,
    Int8Plane, Int8PlaneReader, Int8Vector, LlmCapabilities, LlmChatRequest, LlmChatResponse,
    LlmChunkStream, LlmMessage, LlmProvider, LlmResponseFormat, LlmRole, LlmStreamChunk,
    LlmToolDef, LlmUsage, MAX_WORKING_SET, ModelDownloadProgress, ModelFile, NarrativeSummary,
    NoopDownloadProgress, PlaneBuildReport, PlaneHeader, QWEN3_QUERY_TASK, QueryFamilyFacts,
    RECALL_EXPANSION, RECALL_FLOOR, RigLlmProvider, STATIC_INAPP_BASE_URL, STATIC_MAX_INPUT_TOKENS,
    StaticEmbeddingMatrix, StaticEmbeddingProvider, ToolRegistry, TopicFacts, VectorIndex,
    VectorStore, VectorStoreHeader, VisitContentMap, WorkingSetCandidate, WorkingSetConfig,
    ai_index_status, ai_queue_status, answer_history_question,
    answer_history_question_with_control, apply_role_instruction, binarize, binary_bytes_for_dim,
    build_ai_index, build_ai_index_with_control, build_dedup_content_hash, build_planes_from_store,
    candle_repo_for_runtime, content_key_from_hash, degrade_candle_to_external,
    degrade_static_to_external, dequantize_int8, deregister_ai_chat_run, derived_plane_bytes,
    dot_product, drive_agent_run, drive_ai_chat_stream, ensure_model_downloaded, gguf_file_name,
    hamming_distance, load_assistant_run_response, model_dir_for_repo, model_is_loadable,
    model_is_present_and_verified, parse_static_config, planes_are_stale, prepare_query,
    preview_ai_integrations, probe_tool_capability, provider_capabilities,
    provider_connection_failure_report, quantize_int8, reconcile_ai_queue_controls,
    register_ai_chat_run, request_ai_chat_cancel, runtime_uses_candle, runtime_uses_static,
    score_candidate, select_embedding_provider, select_working_set, semantic_search_history,
    static_embed_ids, static_l2_normalize, static_repo_for_runtime, summarize_query_family,
    summarize_topic, test_provider_connection, vector_plane_bytes, visit_map_plane_bytes,
};
pub use annotations::{
    get_annotation, list_annotations, replace_tags, search_annotations, set_notes,
};
pub use app_lock::{
    app_lock_status, app_lock_status_with_biometric, clear_app_lock_passcode,
    ensure_app_lock_unlocked, hydrate_app_lock_config, initialize_app_lock_session,
    lock_app_session, set_app_lock_passcode, unlock_app_session, unlock_app_session_with_biometric,
    validate_app_lock_config, validate_app_lock_config_with_biometric,
};
pub use archive::{
    BrowseDayInsights, BrowseDayInsightsRequest, BrowseDaySearchQuery, BrowseDayTopDomain,
    BrowseDayTopUrl, archive_status, doctor, ensure_archive_initialized, export_history,
    get_browse_day_insights, list_history, load_audit_run_detail, load_dashboard_snapshot,
    load_history_favicons, load_recent_runs, og_images, og_images_fetch,
    open_source_evidence_connection, preview_retention, preview_snapshot_restore, rekey_archive,
    repair_health_issues, run_backup, run_backup_with_progress, run_retention_prune,
    run_snapshot_restore,
};
pub use chrome::discover_profiles;
pub use config::{ProjectPaths, load_config, project_paths, project_paths_with_root, save_config};
pub use diagnostics::{load_runtime_diagnostics, record_frontend_error, record_rust_panic};
pub use enrichment::content_fetch_api::{
    content_fetch_schedule_eta_secs, content_fetch_settings, drain_one_content_fetch_job,
    enqueue_content_fetch_now, enqueue_content_fetch_working_set, list_visit_enrichment,
};
pub use enrichment::execute_enrichment_job_by_id;
pub use intelligence::{
    build_intelligence_local_host, clear_derived_intelligence_state,
    delete_search_engine_rule_for_settings, explain_entity, get_activity_mix,
    get_activity_mix_trend, get_breadth_index, get_browsing_rhythm, get_compare_set_detail,
    get_compare_sets, get_day_insights, get_digest_summary, get_discovery_trend,
    get_domain_deep_dive, get_domain_trend, get_friction_signals, get_habit_patterns,
    get_hub_pages, get_intelligence_embed_cards, get_intelligence_public_snapshot,
    get_intelligence_widget_snapshot, get_interrupted_habits, get_multi_browser_diff,
    get_navigation_path, get_observed_interactions, get_on_this_day, get_path_flows,
    get_query_families, get_query_family_detail, get_refind_page_detail, get_refind_pages,
    get_reopened_investigations, get_search_effectiveness, get_search_engine_ranking,
    get_search_queries, get_search_trails, get_session_detail, get_sessions, get_stable_sources,
    get_top_search_concepts, get_top_sites, get_trail_detail, intelligence_status,
    list_search_engine_rules_for_settings, preview_intelligence_local_host, run_core_intelligence,
    run_core_intelligence_with_progress, upsert_search_engine_rule_for_settings,
};
pub use intelligence_runtime::{
    cancel_intelligence_job, intelligence_job_stop_requested, load_intelligence_runtime,
    retry_intelligence_job, update_intelligence_job_artifact,
};
pub use intelligence_sections::build_core_intelligence_section_meta;
pub use migration::{
    ApplyImportOptions, EXPORT_FORMAT_VERSION, ExportManifest, ExportManifestFile, ExportedBundle,
    ImportExclusionNote, ImportPreview, ImportResult, apply_import, export_app_data,
    preview_import,
};
pub use models::*;
/// Re-export of the two `secrecy` symbols callers need to construct/expose
/// [`AiProviderRuntime`] secrets without taking their own direct dependency on the crate.
pub use secrecy::{ExposeSecret, SecretString};
pub use stars::{
    StarredMatcher, is_starred_batch, list_stars, load_starred_matcher, set_star, star_counts,
    starred_history_ids, unset_star,
};
pub use takeout::{
    import_browser_history, import_browser_history_with_progress, import_takeout,
    import_takeout_with_progress, inspect_browser_history, inspect_takeout, load_import_batches,
    preview_import_batch, restore_import_batch, revert_import_batch,
};
/// Pure SHA-256 helpers (hex digest of bytes / of a file's contents). Re-exported so the worker's
/// model-download tests + the candle e2e can pin digests against the same impl the manifest uses.
pub use utils::{file_sha256_hex, sha256_hex};
