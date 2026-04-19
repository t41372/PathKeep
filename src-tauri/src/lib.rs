//! Desktop application entrypoint and process-level wiring.
//!
//! This crate is the outermost backend boundary for the Tauri desktop app. It
//! decides whether the process should boot the GUI shell or run the worker CLI,
//! installs global plugins such as logging and updates, and wires the Tauri
//! command surface to the lower-level worker bridge.
//!
//! Important invariants from the accepted docs:
//!
//! - Tauri commands are transport glue, not the source of truth for archive or
//!   AI behavior.
//! - The worker CLI stays behaviorally aligned with the desktop command
//!   surface, so local automation and tests do not get a different backend.
//! - App Lock session setup happens before the renderer starts issuing archive
//!   reads.

mod commands;
#[cfg(feature = "devtools-bridge")]
mod dev_ipc_bridge;
mod file_manager;
mod session;
mod updater;
mod worker_bridge;

use anyhow::Result;
#[cfg(not(test))]
use commands::*;
#[cfg(not(test))]
use session::SessionState;
use std::io::Write;
#[cfg(not(test))]
use tauri_plugin_autostart::MacosLauncher;
#[cfg(not(test))]
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const PRODUCT_DISPLAY_NAME: &str = "PathKeep";

/// Launches the desktop process in either GUI or `--worker` mode.
pub fn entrypoint() -> Result<()> {
    let arguments = std::env::args().collect::<Vec<_>>();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_payload(&mut handle, run_with_arguments(&arguments)?)
}

/// Routes process arguments to the worker CLI or the GUI shell.
fn run_with_arguments(arguments: &[String]) -> Result<Option<String>> {
    if arguments.get(1).map(String::as_str) == Some("--worker") {
        return vault_worker::run_worker_cli(&arguments[2..]).map(Some);
    }
    run_app()?;
    Ok(None)
}

/// Writes a worker-mode JSON payload to stdout when one exists.
fn write_payload<W: Write>(writer: &mut W, payload: Option<String>) -> Result<()> {
    if let Some(payload) = payload {
        writeln!(writer, "{payload}")?;
    }
    Ok(())
}

#[cfg(not(test))]
/// Boots the desktop shell, installs plugins, and registers the command facade.
fn run_app() -> Result<()> {
    let session_state = SessionState::default();
    #[cfg(feature = "devtools-bridge")]
    let dev_bridge_state = session_state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--windowed"])))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let paths = vault_core::project_paths().map_err(tauri::Error::Anyhow)?;
            vault_core::config::ensure_paths(&paths).map_err(tauri::Error::Anyhow)?;
            install_panic_hook(&paths);
            app.handle().plugin(build_logging_plugin(&paths))?;
            let mut config = vault_core::load_config(&paths).map_err(tauri::Error::Anyhow)?;
            vault_core::hydrate_app_lock_config(&paths, &mut config)
                .map_err(tauri::Error::Anyhow)?;
            vault_core::initialize_app_lock_session(&paths, &config)
                .map_err(tauri::Error::Anyhow)?;
            app.handle().plugin(
                tauri_plugin_stronghold::Builder::with_argon2(&paths.stronghold_salt_path).build(),
            )?;
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            #[cfg(feature = "devtools-bridge")]
            dev_ipc_bridge::maybe_launch(app.handle().clone(), dev_bridge_state.clone())
                .map_err(tauri::Error::Anyhow)?;
            Ok(())
        })
        .manage(session_state)
        .invoke_handler(tauri::generate_handler![
            app_build_info,
            app_snapshot,
            app_lock_status,
            save_config,
            initialize_archive,
            preview_rekey_archive,
            rekey_archive,
            preview_snapshot_restore,
            run_snapshot_restore,
            preview_retention_prune,
            run_retention_prune,
            set_session_database_key,
            clear_session_database_key,
            set_app_lock_passcode,
            clear_app_lock_passcode,
            lock_app_session,
            unlock_app_session,
            run_backup_now,
            query_history,
            load_dashboard_snapshot,
            load_audit_run_detail,
            export_history,
            preview_remote_backup,
            run_remote_backup,
            verify_remote_backup,
            inspect_takeout,
            import_takeout,
            preview_import_batch,
            revert_import_batch,
            restore_import_batch,
            preview_schedule,
            apply_schedule,
            remove_schedule,
            schedule_status,
            doctor_report,
            repair_health,
            clear_derived_intelligence,
            keyring_status,
            security_status,
            keyring_get_database_key,
            keyring_store_database_key,
            keyring_clear_database_key,
            store_s3_credentials,
            clear_s3_credentials,
            store_ai_provider_api_key,
            clear_ai_provider_api_key,
            test_ai_provider_connection,
            load_ai_queue_status,
            run_ai_queue_jobs,
            replay_ai_job,
            cancel_ai_job,
            load_ai_assistant_job,
            build_ai_index,
            search_ai_history,
            ask_ai_assistant,
            run_core_intelligence_now,
            queue_core_intelligence_rebuild,
            get_sessions,
            get_session_detail,
            get_search_trails,
            get_trail_detail,
            get_navigation_path,
            get_hub_pages,
            get_search_engine_ranking,
            list_search_engine_rules,
            upsert_search_engine_rule,
            delete_search_engine_rule,
            get_top_search_concepts,
            get_search_queries,
            get_query_families,
            get_query_family_detail,
            get_top_sites,
            get_domain_trend,
            get_refind_pages,
            get_refind_page_detail,
            explain_refind,
            explain_entity,
            get_activity_mix,
            get_activity_mix_trend,
            get_digest_summary,
            get_intelligence_primary_overview,
            get_intelligence_secondary_overview,
            get_stable_sources,
            get_search_effectiveness,
            get_friction_signals,
            get_reopened_investigations,
            get_domain_deep_dive,
            get_day_insights,
            get_browsing_rhythm,
            get_discovery_trend,
            get_intelligence_embed_cards,
            get_intelligence_widget_snapshot,
            get_intelligence_public_snapshot,
            preview_intelligence_local_host,
            build_intelligence_local_host,
            get_on_this_day,
            get_breadth_index,
            get_habit_patterns,
            get_interrupted_habits,
            get_path_flows,
            get_observed_interactions,
            get_compare_sets,
            get_multi_browser_diff,
            load_intelligence_runtime,
            retry_intelligence_job,
            cancel_intelligence_job,
            preview_ai_integrations,
            record_frontend_error,
            reset_local_secret_vault,
            open_path_in_file_manager,
            open_external_url,
            check_for_app_update,
            download_and_install_app_update,
            relaunch_after_update
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

#[cfg(test)]
/// Test stub that avoids constructing a real Tauri runtime.
fn run_app() -> Result<()> {
    Ok(())
}

#[cfg(not(test))]
/// Builds the rotating frontend/Rust log sinks used by the desktop app.
fn build_logging_plugin<R: tauri::Runtime>(
    paths: &vault_core::ProjectPaths,
) -> tauri::plugin::TauriPlugin<R> {
    let frontend_logs = paths.logs_dir.clone();
    let rust_logs = paths.logs_dir.clone();

    tauri_plugin_log::Builder::new()
        .clear_targets()
        .level(tauri_plugin_log::log::LevelFilter::Info)
        .level_for("pathkeep_desktop", tauri_plugin_log::log::LevelFilter::Debug)
        .level_for("vault_core", tauri_plugin_log::log::LevelFilter::Debug)
        .level_for("vault_worker", tauri_plugin_log::log::LevelFilter::Debug)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .target(
            Target::new(TargetKind::Folder { path: rust_logs, file_name: Some("rust".into()) })
                .filter(|metadata| {
                    !metadata.target().starts_with(tauri_plugin_log::WEBVIEW_TARGET)
                }),
        )
        .target(
            Target::new(TargetKind::Folder {
                path: frontend_logs,
                file_name: Some("frontend".into()),
            })
            .filter(|metadata| metadata.target().starts_with(tauri_plugin_log::WEBVIEW_TARGET)),
        )
        .target(Target::new(TargetKind::Webview))
        .build()
}

#[cfg(not(test))]
/// Installs a one-time panic hook that persists Rust crash reports to disk.
fn install_panic_hook(paths: &vault_core::ProjectPaths) {
    use std::sync::OnceLock;

    static PANIC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();
    if PANIC_HOOK_INSTALLED.get().is_some() {
        return;
    }

    let panic_paths = paths.clone();
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        match vault_core::record_rust_panic(&panic_paths, panic_info) {
            Ok(summary) => {
                log::error!(
                    target: "crash",
                    "Captured rust panic at {}: {}",
                    summary.path,
                    summary.message
                );
            }
            Err(error) => {
                eprintln!("PathKeep failed to persist a panic report: {error:#}");
            }
        }
        previous_hook(panic_info);
    }));
    let _ = PANIC_HOOK_INSTALLED.set(());
}
