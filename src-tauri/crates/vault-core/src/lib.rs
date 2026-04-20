//! Canonical backend domain crate for PathKeep.
//!
//! `vault-core` owns the source-of-truth backend rules: archive schema and
//! recoverability, browser/profile read models, optional AI and deterministic
//! intelligence, remote backup verification, and the shared serde models that
//! the desktop shell consumes.
//!
//! What this crate does not own:
//!
//! - native OS integrations such as keyrings or schedulers
//! - Tauri command naming and transport concerns
//! - worker orchestration glue that combines multiple subsystems for one UI
//!   action

pub mod ai;
pub mod ai_queue;
pub mod ai_sidecar;
pub mod app_lock;
pub mod archive;
mod browser_retention;
pub mod chrome;
pub mod config;
pub mod deterministic;
pub mod diagnostics;
mod enrichment;
pub mod git_audit;
pub mod intelligence;
mod intelligence_blobs;
mod intelligence_catalog;
pub mod intelligence_runtime;
mod intelligence_sections;
pub mod models;
pub mod remote;
pub mod takeout;
pub mod utils;

pub use ai::{
    AiIntegrationPreview, AiProviderRuntime, AiRunCancelled, AiRunControl, ai_index_status,
    ai_queue_status, answer_history_question, answer_history_question_with_control, build_ai_index,
    build_ai_index_with_control, load_assistant_run_response, preview_ai_integrations,
    provider_capabilities, provider_connection_failure_report, reconcile_ai_queue_controls,
    semantic_search_history, test_provider_connection,
};
pub use app_lock::{
    app_lock_status, app_lock_status_with_biometric, clear_app_lock_passcode,
    ensure_app_lock_unlocked, hydrate_app_lock_config, initialize_app_lock_session,
    lock_app_session, set_app_lock_passcode, unlock_app_session, unlock_app_session_with_biometric,
    validate_app_lock_config, validate_app_lock_config_with_biometric,
};
pub use archive::{
    archive_status, doctor, ensure_archive_initialized, export_history, list_history,
    load_audit_run_detail, load_dashboard_snapshot, load_recent_runs,
    open_source_evidence_connection, preview_retention, preview_snapshot_restore, rekey_archive,
    repair_health_issues, run_backup, run_backup_with_progress, run_retention_prune,
    run_snapshot_restore,
};
pub use chrome::discover_profiles;
pub use config::{ProjectPaths, load_config, project_paths, save_config};
pub use diagnostics::{load_runtime_diagnostics, record_frontend_error, record_rust_panic};
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
pub use models::*;
pub use remote::{preview_remote_backup, run_remote_backup, verify_remote_backup};
pub use takeout::{
    import_takeout, import_takeout_with_progress, inspect_takeout, load_import_batches,
    preview_import_batch, restore_import_batch, revert_import_batch,
};
