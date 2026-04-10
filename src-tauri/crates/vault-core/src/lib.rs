pub mod ai;
pub mod ai_queue;
pub mod ai_sidecar;
pub mod app_lock;
pub mod archive;
mod browser_retention;
pub mod chrome;
pub mod config;
pub mod deterministic;
pub mod git_audit;
pub mod insights;
pub mod intelligence_runtime;
pub mod models;
pub mod remote;
pub mod takeout;
pub mod utils;

pub use ai::{
    AiIntegrationPreview, AiProviderRuntime, ai_index_status, ai_queue_status,
    answer_history_question, build_ai_index, load_assistant_run_response, preview_ai_integrations,
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
    load_audit_run_detail, load_dashboard_snapshot, load_recent_runs, preview_retention,
    preview_snapshot_restore, rekey_archive, repair_health_issues, run_backup,
    run_backup_with_progress, run_retention_prune, run_snapshot_restore,
};
pub use chrome::discover_profiles;
pub use config::{ProjectPaths, load_config, project_paths, save_config};
pub use insights::{
    clear_derived_intelligence_state, explain_insight, insight_status, load_insight_thread_detail,
    load_insights, run_insights,
};
pub use intelligence_runtime::{
    cancel_intelligence_job, load_intelligence_runtime, retry_intelligence_job,
};
pub use models::*;
pub use remote::{preview_remote_backup, run_remote_backup, verify_remote_backup};
pub use takeout::{
    import_takeout, inspect_takeout, load_import_batches, preview_import_batch,
    restore_import_batch, revert_import_batch,
};
