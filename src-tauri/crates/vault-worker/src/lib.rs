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

mod app;
mod archive_flows;
mod cli;
mod context;
mod intelligence;
mod job_runtime;
mod mcp;
mod schedule;
mod security;

pub use self::{
    app::{
        RekeyRequest, app_snapshot, initialize_archive_database, rekey_archive_database,
        save_user_config,
    },
    archive_flows::{
        audit_run_detail, clear_derived_intelligence, dashboard_snapshot, doctor_report,
        export_query, import_takeout_source, inspect_takeout_source, preview_import_batch_detail,
        preview_remote_backup_bundle, preview_retention_plan, preview_snapshot_restore_plan,
        query_history, repair_health, restore_import_batch_detail, revert_import_batch_detail,
        run_backup_now, run_backup_now_with_progress, run_retention_plan,
        run_snapshot_restore_plan, upload_remote_backup_bundle, verify_remote_backup_bundle,
    },
    cli::run_worker_cli,
    intelligence::{
        ask_ai_assistant, build_ai_index_now, cancel_ai_job, cancel_intelligence_job_now,
        explain_insight_now, load_ai_assistant_job, load_ai_queue, load_insight_thread,
        load_insights_snapshot, load_intelligence_runtime_snapshot, preview_ai_integration_files,
        queue_insights_rebuild, replay_ai_job, retry_intelligence_job_now, run_ai_queue_jobs,
        run_insights_now, search_ai_history, test_ai_provider_connection_report,
    },
    schedule::{apply_schedule_plan, preview_schedule_plan, remove_schedule_plan, schedule_status},
    security::{
        clear_ai_provider_api_key, clear_database_key_from_keyring, clear_s3_credentials,
        configure_app_lock_passcode, keyring_report, load_app_lock_status, lock_app_ui_session,
        preview_rekey_archive, read_database_key_from_keyring, remove_app_lock_passcode,
        reset_local_secret_vault, security_status, store_ai_provider_api_key, store_s3_credentials,
        unlock_app_ui_session, write_database_key_to_keyring,
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
mod tests;
