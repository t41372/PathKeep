#![cfg(feature = "devtools-bridge")]
#![cfg_attr(test, allow(dead_code))]

//! Dev-only HTTP bridge for browser automation and local tooling.
//!
//! This module mirrors a subset of the desktop command surface over localhost
//! HTTP so Playwright/Chrome automation can drive the backend during local
//! development. It is intentionally feature-gated and environment-gated; it is
//! not part of the production transport contract.

use crate::{
    PRODUCT_DISPLAY_NAME, file_manager,
    session::{SessionState, session_key},
    updater, worker_bridge,
};
use anyhow::{Context, Result};
use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::net::{Ipv4Addr, SocketAddr};
use tauri::AppHandle;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConnectionTestRequest, AiProviderSecretInput,
    AiSearchRequest, AppConfig, AppUpdateInstallRequest, ExplainInsightRequest, ExportRequest,
    FrontendErrorReportRequest, HistoryQuery, RetentionPruneRequest, RunInsightsRequest,
    S3CredentialInput, SchedulePlan, SetAppLockPasscodeRequest, SnapshotRestoreRequest,
    TakeoutRequest, UnlockAppSessionRequest,
};
use vault_worker::RekeyRequest;

pub(crate) const DEV_IPC_BRIDGE_ENABLED_ENV: &str = "PATHKEEP_ENABLE_DEV_IPC_BRIDGE";
pub(crate) const DEV_IPC_BRIDGE_PORT_ENV: &str = "PATHKEEP_DEV_IPC_PORT";
pub(crate) const DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV: &str = "PATHKEEP_DEV_IPC_ALLOWED_ORIGINS";
pub(crate) const DEFAULT_DEV_IPC_BRIDGE_PORT: u16 = 43_117;

#[derive(Clone)]
struct DevIpcBridgeState {
    app: Option<AppHandle>,
    session: SessionState,
    port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DevIpcBridgeConfig {
    port: u16,
    allowed_origins: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeArchivePayload {
    config: AppConfig,
    database_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WrappedRequest<T> {
    request: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseKeyPayload {
    database_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunBackupPayload {
    #[serde(default)]
    due_only: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryHistoryPayload {
    query: HistoryQuery,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunIdPayload {
    run_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportPayload {
    request: ExportRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundlePathPayload {
    bundle_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TakeoutPayload {
    request: TakeoutRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchIdPayload {
    batch_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformPayload {
    platform: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanPayload {
    plan: SchedulePlan,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsPayload {
    credentials: S3CredentialInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiProviderSecretPayload {
    input: AiProviderSecretInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderIdPayload {
    provider_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaxJobsPayload {
    max_jobs: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobIdPayload {
    job_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadIdPayload {
    thread_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathPayload {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UrlPayload {
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LockReasonPayload {
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValuePayload {
    value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateInstallPayload {
    request: Option<AppUpdateInstallRequest>,
}

/// Starts the localhost dev bridge when the feature flag and env vars allow it.
pub(crate) fn maybe_launch(app: AppHandle, session: SessionState) -> Result<()> {
    if !bridge_enabled() {
        return Ok(());
    }

    let config = resolve_bridge_config_from_env()?;
    let listener =
        std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, config.port)))
            .with_context(|| format!("binding PathKeep dev IPC bridge on port {}", config.port))?;
    listener
        .set_nonblocking(true)
        .context("marking PathKeep dev IPC bridge listener as non-blocking")?;

    let app_state = DevIpcBridgeState { app: Some(app), session, port: config.port };
    let app_router = build_router(app_state, &config)?;

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                log::error!("PathKeep dev IPC bridge failed to adopt TCP listener: {error:#}");
                return;
            }
        };

        if let Err(error) = axum::serve(listener, app_router).await {
            log::error!("PathKeep dev IPC bridge crashed: {error:#}");
        }
    });

    log::info!(
        "PathKeep dev IPC bridge listening on http://127.0.0.1:{} for Chrome/Playwright automation.",
        config.port
    );
    Ok(())
}

async fn bridge_health(
    State(state): State<DevIpcBridgeState>,
) -> Result<axum::Json<Value>, (StatusCode, axum::Json<Value>)> {
    Ok(axum::Json(json!({
        "ok": true,
        "productName": PRODUCT_DISPLAY_NAME,
        "runtime": "browser-desktop-bridge",
        "port": state.port,
    })))
}

async fn bridge_invoke(
    State(state): State<DevIpcBridgeState>,
    Path(command): Path<String>,
    body: Bytes,
) -> Result<axum::Json<Value>, (StatusCode, axum::Json<Value>)> {
    let payload = if body.is_empty() {
        Value::Object(Default::default())
    } else {
        serde_json::from_slice(&body)
            .map_err(|error| bad_request(format!("Invalid JSON payload: {error}")))?
    };

    dispatch_command(&state, &command, payload).await.map(axum::Json).map_err(internal_error)
}

fn build_router(state: DevIpcBridgeState, config: &DevIpcBridgeConfig) -> Result<Router> {
    let allowed_origins = config
        .allowed_origins
        .iter()
        .map(|origin| HeaderValue::from_str(origin))
        .collect::<Result<Vec<_>, _>>()
        .context("parsing PathKeep dev IPC allowed origins")?;

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(AllowOrigin::list(allowed_origins));

    Ok(Router::new()
        .route("/health", get(bridge_health))
        .route("/commands/{command}", post(bridge_invoke))
        .layer(cors)
        .with_state(state))
}

async fn dispatch_command(
    state: &DevIpcBridgeState,
    command: &str,
    payload: Value,
) -> Result<Value, String> {
    macro_rules! json_value {
        ($value:expr) => {
            to_json_value($value)
        };
    }

    match command {
        "app_build_info" => json_value!(worker_bridge::app_build_info_impl()),
        "app_snapshot" => {
            json_value!(worker_bridge::app_snapshot_impl(session_key(&state.session).as_deref())?)
        }
        "app_lock_status" => json_value!(worker_bridge::app_lock_status_impl()?),
        "save_config" => {
            let payload = parse_payload::<WrappedConfigPayload>(payload)?;
            json_value!(worker_bridge::save_config_impl(
                payload.config,
                session_key(&state.session).as_deref()
            )?)
        }
        "initialize_archive" => {
            let payload = parse_payload::<InitializeArchivePayload>(payload)?;
            json_value!(worker_bridge::initialize_archive_impl(
                payload.config,
                payload.database_key,
                &state.session
            )?)
        }
        "preview_rekey_archive" => {
            let payload = parse_payload::<WrappedRequest<RekeyRequest>>(payload)?;
            json_value!(worker_bridge::preview_rekey_archive_impl(payload.request, &state.session)?)
        }
        "rekey_archive" => {
            let payload = parse_payload::<WrappedRequest<RekeyRequest>>(payload)?;
            json_value!(worker_bridge::rekey_archive_impl(payload.request, &state.session)?)
        }
        "preview_snapshot_restore" => {
            let payload = parse_payload::<WrappedRequest<SnapshotRestoreRequest>>(payload)?;
            json_value!(worker_bridge::preview_snapshot_restore_impl(
                payload.request,
                &state.session
            )?)
        }
        "run_snapshot_restore" => {
            let payload = parse_payload::<WrappedRequest<SnapshotRestoreRequest>>(payload)?;
            json_value!(worker_bridge::run_snapshot_restore_impl(payload.request, &state.session)?)
        }
        "preview_retention_prune" => {
            json_value!(worker_bridge::preview_retention_prune_impl(&state.session)?)
        }
        "run_retention_prune" => {
            let payload = parse_payload::<WrappedRequest<RetentionPruneRequest>>(payload)?;
            json_value!(worker_bridge::run_retention_prune_impl(payload.request, &state.session)?)
        }
        "set_session_database_key" => {
            let payload = parse_payload::<DatabaseKeyPayload>(payload)?;
            worker_bridge::set_session_database_key_impl(payload.database_key, &state.session)?;
            Ok(Value::Null)
        }
        "clear_session_database_key" => {
            worker_bridge::clear_session_database_key_impl(&state.session)?;
            Ok(Value::Null)
        }
        "set_app_lock_passcode" => {
            let payload = parse_payload::<WrappedRequest<SetAppLockPasscodeRequest>>(payload)?;
            json_value!(worker_bridge::set_app_lock_passcode_impl(payload.request)?)
        }
        "clear_app_lock_passcode" => json_value!(worker_bridge::clear_app_lock_passcode_impl()?),
        "lock_app_session" => {
            let payload = parse_payload::<LockReasonPayload>(payload)?;
            json_value!(worker_bridge::lock_app_session_impl(payload.reason)?)
        }
        "unlock_app_session" => {
            let payload = parse_payload::<WrappedRequest<UnlockAppSessionRequest>>(payload)?;
            json_value!(worker_bridge::unlock_app_session_impl(payload.request)?)
        }
        "run_backup_now" => {
            let payload = parse_payload::<RunBackupPayload>(payload)?;
            json_value!(worker_bridge::run_backup_now_impl(
                payload.due_only,
                session_key(&state.session).as_deref(),
                |_| {},
            )?)
        }
        "query_history" => {
            let payload = parse_payload::<QueryHistoryPayload>(payload)?;
            json_value!(worker_bridge::query_history_impl(
                payload.query,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_dashboard_snapshot" => {
            json_value!(worker_bridge::dashboard_snapshot_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "load_audit_run_detail" => {
            let payload = parse_payload::<RunIdPayload>(payload)?;
            json_value!(worker_bridge::audit_run_detail_impl(
                payload.run_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "export_history" => {
            let payload = parse_payload::<ExportPayload>(payload)?;
            json_value!(worker_bridge::export_history_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "preview_remote_backup" => json_value!(worker_bridge::preview_remote_backup_impl()?),
        "run_remote_backup" => {
            json_value!(worker_bridge::run_remote_backup_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "verify_remote_backup" => {
            let payload = parse_payload::<BundlePathPayload>(payload)?;
            json_value!(worker_bridge::verify_remote_backup_impl(
                payload.bundle_path,
                session_key(&state.session).as_deref()
            )?)
        }
        "inspect_takeout" => {
            let payload = parse_payload::<TakeoutPayload>(payload)?;
            json_value!(worker_bridge::inspect_takeout_impl(payload.request)?)
        }
        "import_takeout" => {
            let payload = parse_payload::<TakeoutPayload>(payload)?;
            json_value!(worker_bridge::import_takeout_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "preview_import_batch" => {
            let payload = parse_payload::<BatchIdPayload>(payload)?;
            json_value!(worker_bridge::preview_import_batch_impl(
                payload.batch_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "revert_import_batch" => {
            let payload = parse_payload::<BatchIdPayload>(payload)?;
            json_value!(worker_bridge::revert_import_batch_impl(
                payload.batch_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "restore_import_batch" => {
            let payload = parse_payload::<BatchIdPayload>(payload)?;
            json_value!(worker_bridge::restore_import_batch_impl(
                payload.batch_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "preview_schedule" => {
            let payload = parse_payload::<PlatformPayload>(payload)?;
            json_value!(worker_bridge::preview_schedule_impl(payload.platform)?)
        }
        "schedule_status" => {
            let payload = parse_payload::<PlatformPayload>(payload)?;
            json_value!(worker_bridge::schedule_status_impl(
                payload.platform,
                session_key(&state.session).as_deref()
            )?)
        }
        "apply_schedule" => {
            let payload = parse_payload::<PlanPayload>(payload)?;
            json_value!(worker_bridge::apply_schedule_impl(payload.plan)?)
        }
        "remove_schedule" => {
            let payload = parse_payload::<PlanPayload>(payload)?;
            json_value!(worker_bridge::remove_schedule_impl(payload.plan)?)
        }
        "doctor_report" => {
            json_value!(worker_bridge::doctor_report_impl(session_key(&state.session).as_deref())?)
        }
        "repair_health" => {
            json_value!(worker_bridge::repair_health_impl(session_key(&state.session).as_deref())?)
        }
        "clear_derived_intelligence" => {
            json_value!(worker_bridge::clear_derived_intelligence_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "keyring_status" => json_value!(worker_bridge::keyring_status_impl()),
        "security_status" => json_value!(worker_bridge::security_status_impl(
            session_key(&state.session).as_deref()
        )?),
        "keyring_get_database_key" => json_value!(worker_bridge::keyring_get_database_key_impl()?),
        "keyring_store_database_key" => {
            let payload = parse_payload::<ValuePayload>(payload)?;
            json_value!(worker_bridge::keyring_store_database_key_impl(payload.value)?)
        }
        "keyring_clear_database_key" => {
            json_value!(worker_bridge::keyring_clear_database_key_impl()?)
        }
        "store_s3_credentials" => {
            let payload = parse_payload::<CredentialsPayload>(payload)?;
            worker_bridge::store_s3_credentials_impl(payload.credentials)?;
            Ok(Value::Null)
        }
        "clear_s3_credentials" => {
            worker_bridge::clear_s3_credentials_impl()?;
            Ok(Value::Null)
        }
        "store_ai_provider_api_key" => {
            let payload = parse_payload::<AiProviderSecretPayload>(payload)?;
            json_value!(worker_bridge::store_ai_provider_api_key_impl(
                payload.input,
                session_key(&state.session).as_deref()
            )?)
        }
        "clear_ai_provider_api_key" => {
            let payload = parse_payload::<ProviderIdPayload>(payload)?;
            json_value!(worker_bridge::clear_ai_provider_api_key_impl(
                payload.provider_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "test_ai_provider_connection" => {
            let payload =
                parse_payload::<WrappedRequest<AiProviderConnectionTestRequest>>(payload)?;
            json_value!(worker_bridge::test_ai_provider_connection_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_ai_queue_status" => json_value!(worker_bridge::load_ai_queue_status_impl(
            session_key(&state.session).as_deref()
        )?),
        "run_ai_queue_jobs" => {
            let payload = parse_payload::<MaxJobsPayload>(payload)?;
            json_value!(worker_bridge::run_ai_queue_jobs_impl(
                payload.max_jobs,
                session_key(&state.session).as_deref()
            )?)
        }
        "replay_ai_job" => {
            let payload = parse_payload::<JobIdPayload>(payload)?;
            json_value!(worker_bridge::replay_ai_job_impl(
                payload.job_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "cancel_ai_job" => {
            let payload = parse_payload::<JobIdPayload>(payload)?;
            json_value!(worker_bridge::cancel_ai_job_impl(
                payload.job_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_ai_assistant_job" => {
            let payload = parse_payload::<JobIdPayload>(payload)?;
            json_value!(worker_bridge::load_ai_assistant_job_impl(
                payload.job_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "build_ai_index" => {
            let payload = parse_payload::<WrappedRequest<AiIndexRequest>>(payload)?;
            json_value!(worker_bridge::build_ai_index_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "search_ai_history" => {
            let payload = parse_payload::<WrappedRequest<AiSearchRequest>>(payload)?;
            json_value!(worker_bridge::search_ai_history_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "ask_ai_assistant" => {
            let payload = parse_payload::<WrappedRequest<AiAssistantRequest>>(payload)?;
            json_value!(worker_bridge::ask_ai_assistant_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "run_insights_now" => {
            let payload = parse_payload::<WrappedRequest<RunInsightsRequest>>(payload)?;
            json_value!(worker_bridge::run_insights_now_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_insights" => {
            let payload = parse_payload::<WrappedRequest<RunInsightsRequest>>(payload)?;
            json_value!(worker_bridge::load_insights_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_thread_detail" => {
            let payload = parse_payload::<ThreadIdPayload>(payload)?;
            json_value!(worker_bridge::load_thread_detail_impl(
                payload.thread_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "explain_insight" => {
            let payload = parse_payload::<WrappedRequest<ExplainInsightRequest>>(payload)?;
            json_value!(worker_bridge::explain_insight_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_intelligence_runtime" => {
            json_value!(worker_bridge::load_intelligence_runtime_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "retry_intelligence_job" => {
            let payload = parse_payload::<JobIdPayload>(payload)?;
            json_value!(worker_bridge::retry_intelligence_job_impl(
                payload.job_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "cancel_intelligence_job" => {
            let payload = parse_payload::<JobIdPayload>(payload)?;
            json_value!(worker_bridge::cancel_intelligence_job_impl(
                payload.job_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "preview_ai_integrations" => json_value!(worker_bridge::preview_ai_integrations_impl()?),
        "record_frontend_error" => {
            let payload = parse_payload::<WrappedRequest<FrontendErrorReportRequest>>(payload)?;
            let paths = vault_core::project_paths().map_err(|error| error.to_string())?;
            json_value!(
                vault_core::record_frontend_error(&paths, &payload.request)
                    .map_err(|error| error.to_string())?
            )
        }
        "reset_local_secret_vault" => {
            worker_bridge::reset_local_secret_vault_impl()?;
            Ok(Value::Null)
        }
        "open_path_in_file_manager" => {
            let payload = parse_payload::<PathPayload>(payload)?;
            json_value!(file_manager::open_path_in_file_manager_impl(payload.path)?)
        }
        "open_external_url" => {
            let payload = parse_payload::<UrlPayload>(payload)?;
            json_value!(file_manager::open_external_url_impl(payload.url)?)
        }
        "check_for_app_update" => {
            let app = require_app_handle(state)?;
            json_value!(updater::check_for_app_update(app).await)
        }
        "download_and_install_app_update" => {
            let app = require_app_handle(state)?;
            let payload = parse_payload::<AppUpdateInstallPayload>(payload)?;
            json_value!(updater::download_and_install_app_update(app, payload.request).await)
        }
        "relaunch_after_update" => {
            let app = require_app_handle(state)?;
            json_value!(updater::relaunch_after_update(app))
        }
        other => {
            Err(format!("PathKeep dev IPC bridge does not recognize desktop command \"{other}\"."))
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WrappedConfigPayload {
    config: AppConfig,
}

fn require_app_handle(state: &DevIpcBridgeState) -> Result<AppHandle, String> {
    state.app.clone().ok_or_else(|| {
        "PathKeep dev IPC bridge needs a live AppHandle for updater commands.".to_string()
    })
}

fn parse_payload<T: DeserializeOwned>(payload: Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|error| error.to_string())
}

fn to_json_value<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

fn bad_request(message: String) -> (StatusCode, axum::Json<Value>) {
    (StatusCode::BAD_REQUEST, axum::Json(json!({ "error": message })))
}

fn internal_error(message: String) -> (StatusCode, axum::Json<Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(json!({ "error": message })))
}

fn bridge_enabled() -> bool {
    matches!(
        std::env::var(DEV_IPC_BRIDGE_ENABLED_ENV),
        Ok(value)
            if matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
    )
}

fn resolve_bridge_config_from_env() -> Result<DevIpcBridgeConfig> {
    let port = match std::env::var(DEV_IPC_BRIDGE_PORT_ENV) {
        Ok(value) => value
            .trim()
            .parse::<u16>()
            .with_context(|| format!("parsing {DEV_IPC_BRIDGE_PORT_ENV} as u16"))?,
        Err(_) => DEFAULT_DEV_IPC_BRIDGE_PORT,
    };

    let allowed_origins = std::env::var(DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV)
        .unwrap_or_else(|_| "http://127.0.0.1:1420,http://localhost:1420".to_string())
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    Ok(DevIpcBridgeConfig { port, allowed_origins })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bridge_config_from_env_with_defaults() {
        unsafe {
            std::env::remove_var(DEV_IPC_BRIDGE_PORT_ENV);
            std::env::remove_var(DEV_IPC_BRIDGE_ALLOWED_ORIGINS_ENV);
        }

        let config = resolve_bridge_config_from_env().expect("resolve config");

        assert_eq!(config.port, DEFAULT_DEV_IPC_BRIDGE_PORT);
        assert_eq!(
            config.allowed_origins,
            vec!["http://127.0.0.1:1420".to_string(), "http://localhost:1420".to_string()]
        );
    }

    #[tokio::test]
    async fn dispatch_command_handles_session_round_trip_without_tauri_app() {
        let state = DevIpcBridgeState {
            app: None,
            session: SessionState::default(),
            port: DEFAULT_DEV_IPC_BRIDGE_PORT,
        };

        let set = dispatch_command(
            &state,
            "set_session_database_key",
            json!({ "databaseKey": "secret" }),
        )
        .await
        .expect("set session key");
        assert_eq!(set, Value::Null);
        assert_eq!(session_key(&state.session), Some("secret".to_string()));

        let clear = dispatch_command(&state, "clear_session_database_key", json!({}))
            .await
            .expect("clear session key");
        assert_eq!(clear, Value::Null);
        assert_eq!(session_key(&state.session), None);
    }

    #[tokio::test]
    async fn dispatch_command_rejects_unknown_commands() {
        let state = DevIpcBridgeState {
            app: None,
            session: SessionState::default(),
            port: DEFAULT_DEV_IPC_BRIDGE_PORT,
        };

        let error = dispatch_command(&state, "missing", json!({}))
            .await
            .expect_err("missing command should fail");

        assert!(error.contains("does not recognize"));
    }
}
