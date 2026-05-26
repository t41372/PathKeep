//! Command dispatcher for the dev-only IPC bridge.
//!
//! ## Responsibilities
//!
//! - Map mirrored desktop command strings to their worker bridge adapters.
//! - Decode command payload DTOs without changing the browser automation JSON shape.
//! - Keep updater and file-manager calls inside the desktop facade boundary.
//!
//! ## Not responsible for
//!
//! - Starting the localhost HTTP listener or configuring CORS.
//! - Defining payload DTO field names.
//! - Exposing a production remote-control API.
//!
//! ## Dependencies
//!
//! - Parent bridge state for the live app handle and session key.
//! - `worker_bridge` for the real desktop command implementations.
//! - `file_manager` / `updater` for desktop-layer helpers that do not belong in `vault-worker`.
//!
//! ## Performance notes
//!
//! Dispatch only decodes small command envelopes. Heavy archive, import, and
//! intelligence work must stay behind the existing worker bridge and
//! off-main-thread command contracts.

use crate::{file_manager, session::session_key, updater, worker_bridge};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use vault_core::{
    AiAssistantRequest, AiIndexRequest, AiProviderConnectionTestRequest, AiSearchRequest,
    CategoryFilteredDateRangeRequest, CompareSetDetailRequest, CoreIntelligenceRebuildRequest,
    DayInsightsRequest, DomainDeepDiveRequest, DomainTrendRequest, EntityExplanationRequest,
    ExplainRefindRequest, FrontendErrorReportRequest, GranularityDateRangeRequest,
    IntelligenceEmbedCardsRequest, IntelligenceLocalHostRequest, PagedDateRangeRequest,
    PathFlowRequest, ProfileScopedRequest, QueryFamilyDetailRequest, RefindPageDetailRequest,
    RefindPagesRequest, RetentionPruneRequest, ScopedDateRangeRequest, SearchEffectivenessRequest,
    SearchEngineRuleInput, SearchQueryListRequest, SearchTrailQueryRequest,
    SetAppLockPasscodeRequest, SnapshotRestoreRequest, TopSearchConceptsRequest, TopSitesRequest,
    UnlockAppSessionRequest,
};
use vault_worker::RekeyRequest;

use super::{DevIpcAppHandle, DevIpcBridgeState, payloads::*};

/// Calls the mirrored desktop command while preserving the production command names.
///
/// `command` must be one of the existing Tauri command strings exposed through
/// the devtools bridge. `payload` is decoded into the same request wrapper used
/// by that command; unknown commands and serde failures return the JSON error
/// text expected by the router-level envelope.
pub(in crate::dev_ipc_bridge) async fn dispatch_command(
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
            let key = session_key(&state.session);
            json_value!(backup_now_off_thread(payload.due_only, key).await?)
        }
        "query_history" => {
            let payload = parse_payload::<QueryHistoryPayload>(payload)?;
            json_value!(worker_bridge::query_history_impl(
                payload.query,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_history_favicons" => {
            let payload = parse_payload::<HistoryFaviconPayload>(payload)?;
            json_value!(worker_bridge::load_history_favicons_impl(
                payload.entries,
                session_key(&state.session).as_deref()
            )?)
        }
        "load_history_og_images" => {
            let payload = parse_payload::<HistoryOgImagePayload>(payload)?;
            json_value!(worker_bridge::load_history_og_images_impl(
                payload.entries,
                session_key(&state.session).as_deref()
            )?)
        }
        "mark_og_images_shown" => {
            let payload = parse_payload::<OgImageUrlsPayload>(payload)?;
            json_value!(worker_bridge::mark_og_images_shown_impl(
                payload.urls,
                session_key(&state.session).as_deref()
            )?)
        }
        "trigger_og_image_refetch" => {
            let payload = parse_payload::<OgImageUrlsPayload>(payload)?;
            let session_key = session_key(&state.session);
            // refetch builds a reqwest::blocking::Client whose internal tokio runtime
            // panics if dropped from inside another async runtime (Tauri commands wrap
            // in spawn_blocking via run_blocking_command; the dev bridge must mirror it).
            let result = tokio::task::spawn_blocking(move || {
                worker_bridge::refetch_og_images_impl(payload.urls, session_key.as_deref())
            })
            .await
            .map_err(|error| format!("trigger_og_image_refetch join failed: {error}"))??;
            json_value!(result)
        }
        "get_og_image_storage_stats" => {
            json_value!(worker_bridge::og_image_storage_stats_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "clear_og_image_cache" => {
            json_value!(worker_bridge::clear_og_image_cache_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "run_og_image_cleanup" => {
            json_value!(worker_bridge::run_og_image_cleanup_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "get_url_annotation" => {
            let payload = parse_payload::<UrlPayload>(payload)?;
            json_value!(worker_bridge::get_annotation_impl(
                session_key(&state.session).as_deref(),
                &payload.url,
            )?)
        }
        "set_url_notes" => {
            let payload = parse_payload::<SetNotesPayload>(payload)?;
            json_value!(worker_bridge::set_notes_impl(
                session_key(&state.session).as_deref(),
                payload.request,
            )?)
        }
        "replace_url_tags" => {
            let payload = parse_payload::<ReplaceTagsPayload>(payload)?;
            json_value!(worker_bridge::replace_tags_impl(
                session_key(&state.session).as_deref(),
                payload.request,
            )?)
        }
        "list_url_annotations" => {
            let payload = parse_payload::<AnnotationLimitPayload>(payload)?;
            json_value!(worker_bridge::list_annotations_impl(
                session_key(&state.session).as_deref(),
                payload.limit,
            )?)
        }
        "search_url_annotations" => {
            let payload = parse_payload::<AnnotationSearchPayload>(payload)?;
            json_value!(worker_bridge::search_annotations_impl(
                session_key(&state.session).as_deref(),
                &payload.query,
                payload.limit,
            )?)
        }
        "export_app_data" => {
            let payload = parse_payload::<ExportAppDataPayload>(payload)?;
            json_value!(worker_bridge::export_app_data_impl(
                session_key(&state.session).as_deref(),
                std::path::PathBuf::from(payload.target_path),
            )?)
        }
        "preview_app_data_import" => {
            let payload = parse_payload::<PreviewAppDataImportPayload>(payload)?;
            json_value!(worker_bridge::preview_app_data_import_impl(std::path::PathBuf::from(
                payload.bundle_path
            ),)?)
        }
        "apply_app_data_import" => {
            let payload = parse_payload::<ApplyAppDataImportPayload>(payload)?;
            json_value!(worker_bridge::apply_app_data_import_impl(
                session_key(&state.session).as_deref(),
                std::path::PathBuf::from(payload.bundle_path),
                payload.options,
            )?)
        }
        "load_dashboard_snapshot" => {
            json_value!(worker_bridge::dashboard_snapshot_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "get_browse_day_insights" => {
            let payload = parse_payload::<BrowseDayInsightsPayload>(payload)?;
            json_value!(worker_bridge::browse_day_insights_impl(
                session_key(&state.session).as_deref(),
                payload.request,
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
        "inspect_takeout" => {
            let payload = parse_payload::<TakeoutPayload>(payload)?;
            json_value!(worker_bridge::inspect_takeout_impl(payload.request)?)
        }
        "import_takeout" => {
            let payload = parse_payload::<TakeoutPayload>(payload)?;
            json_value!(worker_bridge::import_takeout_impl(
                payload.request,
                session_key(&state.session).as_deref(),
                std::mem::drop::<vault_core::ImportProgressEvent>
            )?)
        }
        "inspect_browser_history" => {
            let payload = parse_payload::<BrowserHistoryPayload>(payload)?;
            json_value!(worker_bridge::inspect_browser_history_impl(payload.request)?)
        }
        "import_browser_history" => {
            let payload = parse_payload::<BrowserHistoryPayload>(payload)?;
            json_value!(worker_bridge::import_browser_history_impl(
                payload.request,
                session_key(&state.session).as_deref(),
                std::mem::drop::<vault_core::ImportProgressEvent>
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
        "repair_schedule" => {
            let payload = parse_payload::<PlanPayload>(payload)?;
            json_value!(worker_bridge::repair_schedule_impl(payload.plan)?)
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
        "run_core_intelligence_now" => {
            let payload = parse_payload::<WrappedRequest<CoreIntelligenceRebuildRequest>>(payload)?;
            json_value!(worker_bridge::run_core_intelligence_now_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "queue_core_intelligence_rebuild" => {
            let payload = parse_payload::<WrappedRequest<CoreIntelligenceRebuildRequest>>(payload)?;
            json_value!(worker_bridge::queue_core_intelligence_rebuild_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_sessions" => {
            let payload = parse_payload::<WrappedRequest<PagedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_sessions_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_session_detail" => {
            let payload = parse_payload::<SessionIdPayload>(payload)?;
            json_value!(worker_bridge::get_session_detail_impl(
                payload.session_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_search_trails" => {
            let payload = parse_payload::<WrappedRequest<SearchTrailQueryRequest>>(payload)?;
            json_value!(worker_bridge::get_search_trails_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_trail_detail" => {
            let payload = parse_payload::<TrailIdPayload>(payload)?;
            json_value!(worker_bridge::get_trail_detail_impl(
                payload.trail_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_navigation_path" => {
            let payload = parse_payload::<VisitIdPayload>(payload)?;
            json_value!(worker_bridge::get_navigation_path_impl(
                payload.visit_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_hub_pages" => {
            let payload = parse_payload::<WrappedRequest<TopSitesRequest>>(payload)?;
            json_value!(worker_bridge::get_hub_pages_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_search_engine_ranking" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_search_engine_ranking_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "list_search_engine_rules" => {
            json_value!(worker_bridge::list_search_engine_rules_impl(
                session_key(&state.session).as_deref()
            )?)
        }
        "upsert_search_engine_rule" => {
            let payload = parse_payload::<InputPayload<SearchEngineRuleInput>>(payload)?;
            json_value!(worker_bridge::upsert_search_engine_rule_impl(
                payload.input,
                session_key(&state.session).as_deref()
            )?)
        }
        "delete_search_engine_rule" => {
            let payload = parse_payload::<RuleIdPayload>(payload)?;
            json_value!(worker_bridge::delete_search_engine_rule_impl(
                payload.rule_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_top_search_concepts" => {
            let payload = parse_payload::<WrappedRequest<TopSearchConceptsRequest>>(payload)?;
            json_value!(worker_bridge::get_top_search_concepts_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_search_queries" => {
            let payload = parse_payload::<WrappedRequest<SearchQueryListRequest>>(payload)?;
            json_value!(worker_bridge::get_search_queries_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_query_families" => {
            let payload = parse_payload::<WrappedRequest<PagedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_query_families_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_query_family_detail" => {
            let payload = parse_payload::<WrappedRequest<QueryFamilyDetailRequest>>(payload)?;
            json_value!(worker_bridge::get_query_family_detail_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_top_sites" => {
            let payload = parse_payload::<WrappedRequest<TopSitesRequest>>(payload)?;
            json_value!(worker_bridge::get_top_sites_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_domain_trend" => {
            let payload = parse_payload::<WrappedRequest<DomainTrendRequest>>(payload)?;
            json_value!(worker_bridge::get_domain_trend_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_refind_pages" => {
            let payload = parse_payload::<WrappedRequest<RefindPagesRequest>>(payload)?;
            json_value!(worker_bridge::get_refind_pages_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_refind_page_detail" => {
            let payload = parse_payload::<WrappedRequest<RefindPageDetailRequest>>(payload)?;
            json_value!(worker_bridge::get_refind_page_detail_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "explain_refind" => {
            let payload = parse_payload::<WrappedRequest<ExplainRefindRequest>>(payload)?;
            json_value!(worker_bridge::explain_refind_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "explain_entity" => {
            let payload = parse_payload::<WrappedRequest<EntityExplanationRequest>>(payload)?;
            json_value!(worker_bridge::explain_entity_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_activity_mix" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_activity_mix_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_activity_mix_trend" => {
            let payload = parse_payload::<WrappedRequest<GranularityDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_activity_mix_trend_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_digest_summary" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_digest_summary_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_intelligence_primary_overview" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_intelligence_primary_overview_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_intelligence_secondary_overview" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_intelligence_secondary_overview_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_stable_sources" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_stable_sources_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_search_effectiveness" => {
            let payload = parse_payload::<WrappedRequest<SearchEffectivenessRequest>>(payload)?;
            json_value!(worker_bridge::get_search_effectiveness_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_friction_signals" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_friction_signals_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_reopened_investigations" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_reopened_investigations_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_domain_deep_dive" => {
            let payload = parse_payload::<WrappedRequest<DomainDeepDiveRequest>>(payload)?;
            json_value!(worker_bridge::get_domain_deep_dive_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_day_insights" => {
            let payload = parse_payload::<WrappedRequest<DayInsightsRequest>>(payload)?;
            json_value!(worker_bridge::get_day_insights_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        // TODO: M13 - Revisit this dev-only intelligence mirror only if the
        // next transport parity audit proves a shared manifest or generation
        // layer would reduce real maintenance cost.
        "get_browsing_rhythm" => {
            let payload =
                parse_payload::<WrappedRequest<CategoryFilteredDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_browsing_rhythm_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_discovery_trend" => {
            let payload = parse_payload::<WrappedRequest<GranularityDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_discovery_trend_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_intelligence_embed_cards" => {
            let payload = parse_payload::<WrappedRequest<IntelligenceEmbedCardsRequest>>(payload)?;
            json_value!(worker_bridge::get_intelligence_embed_cards_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_intelligence_widget_snapshot" => {
            let payload = parse_payload::<WrappedRequest<IntelligenceEmbedCardsRequest>>(payload)?;
            json_value!(worker_bridge::get_intelligence_widget_snapshot_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_intelligence_public_snapshot" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_intelligence_public_snapshot_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "preview_intelligence_local_host" => {
            let payload = parse_payload::<WrappedRequest<IntelligenceLocalHostRequest>>(payload)?;
            json_value!(worker_bridge::preview_intelligence_local_host_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "build_intelligence_local_host" => {
            let payload = parse_payload::<WrappedRequest<IntelligenceLocalHostRequest>>(payload)?;
            json_value!(worker_bridge::build_intelligence_local_host_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_on_this_day" => {
            let payload = parse_payload::<ProfileIdPayload>(payload)?;
            json_value!(worker_bridge::get_on_this_day_impl(
                payload.profile_id,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_breadth_index" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_breadth_index_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_habit_patterns" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_habit_patterns_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_interrupted_habits" => {
            let payload = parse_payload::<WrappedRequest<ProfileScopedRequest>>(payload)?;
            json_value!(worker_bridge::get_interrupted_habits_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_path_flows" => {
            let payload = parse_payload::<WrappedRequest<PathFlowRequest>>(payload)?;
            json_value!(worker_bridge::get_path_flows_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_observed_interactions" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_observed_interactions_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_compare_sets" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_compare_sets_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_compare_set_detail" => {
            let payload = parse_payload::<WrappedRequest<CompareSetDetailRequest>>(payload)?;
            json_value!(worker_bridge::get_compare_set_detail_impl(
                payload.request,
                session_key(&state.session).as_deref()
            )?)
        }
        "get_multi_browser_diff" => {
            let payload = parse_payload::<WrappedRequest<ScopedDateRangeRequest>>(payload)?;
            json_value!(worker_bridge::get_multi_browser_diff_impl(
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

fn require_app_handle(state: &DevIpcBridgeState) -> Result<DevIpcAppHandle, String> {
    let guard = state
        .app
        .read()
        .map_err(|_| "PathKeep dev IPC bridge AppHandle slot is poisoned.".to_string())?;
    guard.clone().ok_or_else(|| {
        "PathKeep dev IPC bridge needs a live AppHandle for updater commands.".to_string()
    })
}

fn parse_payload<T: DeserializeOwned>(payload: Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|error| error.to_string())
}

fn to_json_value<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

/// Hops `run_backup_now_impl` onto the tokio blocking thread pool.
///
/// The post-backup tick inside `run_backup_now` fires the og:image refetch
/// and prefetch passes, which build a `reqwest::blocking::Client` whose
/// internal tokio runtime panics if dropped from inside another async
/// runtime. Production Tauri commands wrap synchronous worker calls in
/// `tauri::async_runtime::spawn_blocking` via `run_blocking_command`; the
/// dev IPC bridge must mirror it. Without this hop the dev-IPC HTTP
/// server thread crashes mid-response and the client sees a
/// `socket hang up` with no body.
async fn backup_now_off_thread(
    due_only: bool,
    key: Option<String>,
) -> Result<vault_core::BackupReport, String> {
    tokio::task::spawn_blocking(move || {
        worker_bridge::run_backup_now_impl(
            due_only,
            key.as_deref(),
            std::mem::drop::<vault_core::BackupProgressEvent>,
        )
    })
    .await
    .unwrap_or_else(|error| Err(format!("run_backup_now join failed: {error}")))
}

#[cfg(test)]
mod tests;
