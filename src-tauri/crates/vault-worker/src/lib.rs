//! Worker orchestration crate for PathKeep.
//!
//! `vault-worker` is the glue layer between the desktop facade and the core
//! domain crates. It owns orchestration concerns such as:
//!
//! - hydrating config with native/keyring-backed state
//! - enforcing the App Lock session boundary before archive reads
//! - composing `vault-core` archive/intelligence flows with `vault-platform`
//!   adapters
//! - exposing a small CLI/MCP surface for desktop automation
//!
//! It does **not** define canonical schema rules, parser behavior, or Tauri
//! command naming. Those contracts still belong to the accepted architecture
//! docs and the lower-level crates.

mod annotations;
mod app;
mod archive_flows;
mod cli;
mod context;
mod intelligence;
mod job_runtime;
mod mcp;
mod migration;
mod schedule;
mod security;

#[cfg(all(test, coverage))]
pub(crate) use self::intelligence::{
    complete_claimed_assistant_job, complete_claimed_index_job, execute_core_intelligence_job,
    maybe_spawn_ai_queue_drain, maybe_spawn_intelligence_queue_drain, start_ai_job_control,
};
pub use self::migration::{apply_import, export_app_data, preview_import};
pub use self::{
    annotations::{get_annotation, list_annotations, replace_tags, search_annotations, set_notes},
    app::{
        RekeyRequest, app_snapshot, initialize_archive_database, rekey_archive_database,
        save_user_config,
    },
    archive_flows::{
        audit_run_detail, clear_derived_intelligence, clear_og_image_cache, dashboard_snapshot,
        doctor_report, export_query, import_browser_history_source,
        import_browser_history_source_with_progress, import_takeout_source,
        import_takeout_source_with_progress, inspect_browser_history_source,
        inspect_takeout_source, load_history_favicons, load_history_og_images,
        mark_og_images_shown, og_image_storage_stats, prefetch_og_images_on_demand,
        preview_import_batch_detail, preview_retention_plan, preview_snapshot_restore_plan,
        query_history, refetch_og_images, repair_health, restore_import_batch_detail,
        revert_import_batch_detail, run_backup_now, run_backup_now_with_progress,
        run_og_image_cleanup, run_retention_plan, run_snapshot_restore_plan,
    },
    cli::run_worker_cli,
    intelligence::{
        ask_ai_assistant, build_ai_index_now, build_intelligence_local_host, cancel_ai_job,
        cancel_intelligence_job_now, delete_search_engine_rule, explain_entity, explain_refind,
        get_activity_mix, get_activity_mix_trend, get_breadth_index, get_browsing_rhythm,
        get_compare_set_detail, get_compare_sets, get_day_insights, get_digest_summary,
        get_discovery_trend, get_domain_deep_dive, get_domain_trend, get_friction_signals,
        get_habit_patterns, get_hub_pages, get_intelligence_embed_cards,
        get_intelligence_primary_overview, get_intelligence_public_snapshot,
        get_intelligence_secondary_overview, get_intelligence_widget_snapshot,
        get_interrupted_habits, get_multi_browser_diff, get_navigation_path,
        get_observed_interactions, get_on_this_day, get_path_flows, get_query_families,
        get_query_family_detail, get_refind_page_detail, get_refind_pages,
        get_reopened_investigations, get_search_effectiveness, get_search_engine_ranking,
        get_search_queries, get_search_trails, get_session_detail, get_sessions,
        get_stable_sources, get_top_search_concepts, get_top_sites, get_trail_detail,
        list_search_engine_rules, load_ai_assistant_job, load_ai_queue,
        load_intelligence_runtime_snapshot, preview_ai_integration_files,
        preview_intelligence_local_host, queue_core_intelligence_rebuild, replay_ai_job,
        retry_intelligence_job_now, run_ai_queue_jobs, run_core_intelligence_now,
        search_ai_history, test_ai_provider_connection_report, upsert_search_engine_rule,
    },
    schedule::{
        apply_schedule_plan, preview_schedule_plan, remove_schedule_plan, repair_schedule_plan,
        schedule_status,
    },
    security::{
        clear_ai_provider_api_key, clear_database_key_from_keyring, configure_app_lock_passcode,
        keyring_report, load_app_lock_status, lock_app_ui_session, preview_rekey_archive,
        read_database_key_from_keyring, remove_app_lock_passcode, reset_local_secret_vault,
        security_status, store_ai_provider_api_key, unlock_app_ui_session,
        write_database_key_to_keyring,
    },
};
#[cfg(test)]
pub(crate) use self::{
    context::{
        derive_ai_status, hydrate_provider_collection, resolve_provider_runtime,
        search_response_with_resolution_note, selected_optional_embedding_runtime,
    },
    mcp::{
        BrowserHistoryMcpServer, McpSearchRequest, mcp_archive_status_result, mcp_search_result,
    },
};

#[cfg(test)]
pub(crate) mod tests;
