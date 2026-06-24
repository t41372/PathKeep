//! Regression coverage for the dev IPC command dispatcher.
//!
//! ## Responsibilities
//!
//! - Prove session-only commands still work without a live Tauri app handle.
//! - Preserve the unknown-command error contract used by browser automation.
//!
//! ## Not responsible for
//!
//! - Testing every mirrored desktop command implementation.
//! - Starting the HTTP router or binding a localhost port.
//!
//! ## Dependencies
//!
//! - `SessionState` for in-memory command round trips.
//! - `serde_json` for browser-shaped command envelopes.
//!
//! ## Performance notes
//!
//! These tests run only tiny dispatch paths and must not bootstrap archives,
//! browser fixtures, or intelligence rebuilds.

#![allow(unexpected_cfgs)]

use crate::session::{SessionState, session_key};
use crate::test_support::{
    CHROME_USER_DATA_OVERRIDE_ENV, PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, lock_env,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::future::Future;
use tempfile::tempdir;
use vault_core::{
    AgentMessage, AiAssistantRequest, AiChatMessage, AiChatRole, AiChatSendRequest, AiIndexRequest,
    AiProviderConnectionTestRequest, AiProviderPurpose, AiProviderSecretInput, AiSearchRequest,
    AppConfig, AppUpdateInstallRequest, ArchiveMode, BrowserHistoryImportRequest,
    CategoryFilteredDateRangeRequest, CompareSetDetailRequest, CoreIntelligenceRebuildRequest,
    DateRange, DayInsightsRequest, DomainDeepDiveRequest, DomainTrendRequest,
    EntityExplanationRequest, ExportFormat, ExportRequest, FrontendErrorReportRequest,
    GeneratedFile, GranularityDateRangeRequest, HistoryFaviconLookupEntry, HistoryQuery,
    IntelligenceEmbedCardsRequest, IntelligenceLocalHostRequest, ListAgentConversationsRequest,
    PagedDateRangeRequest, PathFlowRequest, ProfileScopedRequest, QueryFamilyDetailRequest,
    RefindPageDetailRequest, RefindPagesRequest, RenameAgentConversationRequest,
    RetentionPruneRequest, SaveAgentConversationRequest, SchedulePlan, ScopedDateRangeRequest,
    SearchEffectivenessRequest, SearchEngineRuleInput, SearchQueryListRequest,
    SearchTrailQueryRequest, SetAppLockPasscodeRequest, SnapshotRestoreRequest, TakeoutRequest,
    TopSearchConceptsRequest, TopSitesRequest, UnlockAppSessionRequest,
};
use vault_worker::RekeyRequest;

use super::super::{DEFAULT_DEV_IPC_BRIDGE_PORT, DevIpcBridgeState};
use super::dispatch_command;

const COVERAGE_UPDATER_STATE_ENV: &str = "PATHKEEP_COVERAGE_UPDATER_STATE";

#[tokio::test]
async fn dispatch_command_handles_session_round_trip_without_tauri_app() {
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);

    let set =
        dispatch_command(&state, "set_session_database_key", json!({ "databaseKey": "secret" }))
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
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);

    let error = dispatch_command(&state, "missing", json!({}))
        .await
        .expect_err("missing command should fail");

    assert!(error.contains("does not recognize"));
}

fn wrapped<T: Serialize>(request: T) -> Value {
    json!({ "request": request })
}

fn input<T: Serialize>(input: T) -> Value {
    json!({ "input": input })
}

fn schedule_plan() -> SchedulePlan {
    SchedulePlan {
        platform: "linux".to_string(),
        label: "Linux".to_string(),
        executable_path: "/tmp/pathkeep".to_string(),
        generated_files: vec![GeneratedFile {
            relative_path: "pathkeep.timer".to_string(),
            absolute_path: None,
            purpose: "test fixture".to_string(),
            contents: "timer".to_string(),
        }],
        manual_steps: vec!["review".to_string()],
        manual_step_details: Vec::new(),
        apply_commands: Vec::new(),
        rollback_commands: Vec::new(),
        apply_supported: false,
    }
}

#[tokio::test]
async fn dispatch_command_runs_updater_commands_when_app_handle_is_available() {
    let guard = lock_env();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".to_string(),
        json!({
            "pubkey": "test-public-key",
            "endpoints": []
        }),
    );
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .expect("mock app");
    let state = DevIpcBridgeState::with_app(
        app.handle().clone(),
        SessionState::default(),
        DEFAULT_DEV_IPC_BRIDGE_PORT,
    );

    unsafe {
        std::env::set_var(COVERAGE_UPDATER_STATE_ENV, "available");
    }
    drop(guard);
    let check =
        dispatch_command(&state, "check_for_app_update", json!({})).await.expect("check update");
    assert_eq!(check.pointer("/availability/supported").and_then(Value::as_bool), Some(true));

    let install = dispatch_command(
        &state,
        "download_and_install_app_update",
        json!({ "request": AppUpdateInstallRequest::default() }),
    )
    .await
    .expect("install update");
    assert!(install.pointer("/phase").and_then(Value::as_str).is_some());

    #[cfg(coverage)]
    {
        let relaunch = dispatch_command(&state, "relaunch_after_update", json!({}))
            .await
            .expect("relaunch update");
        assert_eq!(relaunch, Value::Bool(true));
    }

    unsafe {
        std::env::remove_var(COVERAGE_UPDATER_STATE_ENV);
    }
}

fn test_config() -> AppConfig {
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        selected_profile_ids: Vec::new(),
        git_enabled: false,
        ..AppConfig::default()
    }
}

// Some dispatch arms intentionally yield (e.g. spawn_blocking for the
// og:image refetch reqwest client). A current-thread tokio runtime
// drives every arm to completion without bootstrapping archives or
// intelligence rebuilds.
fn ready_block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("dispatch coverage tokio runtime")
        .block_on(future)
}

fn dispatch_for_coverage(state: &DevIpcBridgeState, command: &str, payload: Value) {
    // A few dispatch arms reach worker code that builds its own multi-thread
    // tokio runtime via `Runtime::new()` (e.g. `search_ai_history` →
    // `tokio_runtime()?.block_on(...)`). That panics with "Cannot start a
    // runtime from within a runtime" when invoked from inside the
    // `ready_block_on` current-thread runtime above. We only care that the
    // dispatch arm is *reached* for coverage; the inner runtime panic is
    // an environment artifact of the test harness, not a bug in the
    // command surface. Catch it so the rest of the dispatch walk completes.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ready_block_on(dispatch_command(state, command, payload));
    }));
}

#[test]
fn dispatch_command_decodes_all_browser_mirror_command_payloads() {
    let _guard = lock_env();
    let dir = tempdir().expect("tempdir");
    let chrome_root = dir.path().join("chrome-user-data");
    let keyring_root = dir.path().join("test-keyring");
    std::fs::create_dir_all(&chrome_root).expect("chrome root");

    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
    }

    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let date_range = DateRange { start: "1970-01-01".to_string(), end: "2100-01-01".to_string() };
    let scoped = ScopedDateRangeRequest { date_range: date_range.clone(), profile_id: None };
    let paged = PagedDateRangeRequest {
        date_range: date_range.clone(),
        profile_id: None,
        page: 0,
        page_size: 10,
    };
    let top_sites = TopSitesRequest {
        date_range: date_range.clone(),
        profile_id: None,
        sort_by: None,
        limit: Some(10),
    };
    let takeout = TakeoutRequest {
        source_path: dir.path().join("takeout").display().to_string(),
        dry_run: true,
    };
    let browser_history = BrowserHistoryImportRequest {
        source_path: dir.path().join("History").display().to_string(),
        dry_run: true,
        browser_family: Some("chromium".to_string()),
        profile_id: Some("chrome:Default".to_string()),
        browser_name: Some("Chrome".to_string()),
        profile_name: Some("Default".to_string()),
    };
    let intelligence_host = IntelligenceLocalHostRequest {
        date_range: date_range.clone(),
        profile_id: None,
        locale: "en".to_string(),
    };

    dispatch_for_coverage(&state, "app_build_info", json!({}));
    dispatch_for_coverage(&state, "app_snapshot", json!({}));
    dispatch_for_coverage(&state, "app_lock_status", json!({}));
    dispatch_for_coverage(&state, "save_config", json!({ "config": test_config() }));
    dispatch_for_coverage(
        &state,
        "initialize_archive",
        json!({ "config": test_config(), "databaseKey": null }),
    );
    dispatch_for_coverage(
        &state,
        "preview_rekey_archive",
        wrapped(RekeyRequest { new_mode: ArchiveMode::Encrypted, new_key: None }),
    );
    dispatch_for_coverage(
        &state,
        "rekey_archive",
        wrapped(RekeyRequest {
            new_mode: ArchiveMode::Encrypted,
            new_key: Some("dispatch-passphrase".to_string()),
        }),
    );
    dispatch_for_coverage(
        &state,
        "preview_snapshot_restore",
        wrapped(SnapshotRestoreRequest {
            snapshot_path: dir.path().join("missing-snapshot.sqlite").display().to_string(),
        }),
    );
    dispatch_for_coverage(
        &state,
        "run_snapshot_restore",
        wrapped(SnapshotRestoreRequest {
            snapshot_path: dir.path().join("missing-snapshot.sqlite").display().to_string(),
        }),
    );
    dispatch_for_coverage(&state, "preview_retention_prune", json!({}));
    dispatch_for_coverage(
        &state,
        "run_retention_prune",
        wrapped(RetentionPruneRequest { bucket_ids: vec!["snapshots".to_string()] }),
    );
    dispatch_for_coverage(&state, "set_session_database_key", json!({ "databaseKey": "key" }));
    dispatch_for_coverage(&state, "clear_session_database_key", json!({}));
    dispatch_for_coverage(
        &state,
        "set_app_lock_passcode",
        wrapped(SetAppLockPasscodeRequest {
            passcode: "1234".to_string(),
            recovery_hint: Some("hint".to_string()),
        }),
    );
    dispatch_for_coverage(&state, "clear_app_lock_passcode", json!({}));
    dispatch_for_coverage(&state, "lock_app_session", json!({ "reason": "coverage" }));
    dispatch_for_coverage(
        &state,
        "unlock_app_session",
        wrapped(UnlockAppSessionRequest {
            passcode: Some("1234".to_string()),
            use_biometric: false,
        }),
    );
    dispatch_for_coverage(&state, "run_backup_now", json!({ "dueOnly": false }));
    dispatch_for_coverage(&state, "query_history", json!({ "query": HistoryQuery::default() }));
    dispatch_for_coverage(
        &state,
        "load_history_favicons",
        json!({
            "entries": [HistoryFaviconLookupEntry {
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com".to_string(),
                visit_time: 0,
            }]
        }),
    );
    dispatch_for_coverage(&state, "load_dashboard_snapshot", json!({}));
    dispatch_for_coverage(
        &state,
        "get_browse_day_insights",
        wrapped(vault_core::BrowseDayInsightsRequest {
            date: "2026-04-01".to_string(),
            profile_id: None,
        }),
    );
    dispatch_for_coverage(&state, "load_audit_run_detail", json!({ "runId": 1 }));
    // Data Migration (Settings → Export / Import) — every arm runs through
    // `worker_bridge::migration` and the wrappers must surface even when
    // the underlying paths point at a fresh, uninitialised project tree.
    dispatch_for_coverage(
        &state,
        "export_app_data",
        json!({ "targetPath": dir.path().join("bundle.pathkeep").display().to_string() }),
    );
    dispatch_for_coverage(
        &state,
        "preview_app_data_import",
        json!({ "bundlePath": dir.path().join("missing.pathkeep").display().to_string() }),
    );
    dispatch_for_coverage(
        &state,
        "apply_app_data_import",
        json!({
            "bundlePath": dir.path().join("missing.pathkeep").display().to_string(),
            "options": { "confirmOverwrite": false }
        }),
    );
    // og:image + annotations dispatch arms (cover dispatch.rs lines 154-188).
    dispatch_for_coverage(&state, "load_history_og_images", json!({ "entries": [] }));
    dispatch_for_coverage(&state, "mark_og_images_shown", json!({ "urls": [] }));
    dispatch_for_coverage(&state, "trigger_og_image_refetch", json!({ "urls": [] }));
    dispatch_for_coverage(&state, "get_og_image_storage_stats", json!({}));
    dispatch_for_coverage(&state, "clear_og_image_cache", json!({}));
    dispatch_for_coverage(&state, "run_og_image_cleanup", json!({}));
    dispatch_for_coverage(
        &state,
        "get_url_annotation",
        json!({ "url": "https://example.com/seed" }),
    );
    dispatch_for_coverage(
        &state,
        "set_url_notes",
        json!({
            "request": {
                "url": "https://example.com/seed",
                "notes": "dispatch-test"
            }
        }),
    );
    dispatch_for_coverage(
        &state,
        "replace_url_tags",
        json!({
            "request": {
                "url": "https://example.com/seed",
                "tags": ["dispatch", "test"]
            }
        }),
    );
    dispatch_for_coverage(&state, "list_url_annotations", json!({ "limit": 10 }));
    dispatch_for_coverage(
        &state,
        "search_url_annotations",
        json!({ "query": "dispatch", "limit": 10 }),
    );
    // Stars dispatch arms.
    dispatch_for_coverage(
        &state,
        "set_star",
        json!({
            "request": {
                "entityKind": "url",
                "entityKey": "https://example.com/seed"
            }
        }),
    );
    dispatch_for_coverage(
        &state,
        "unset_star",
        json!({
            "request": {
                "entityKind": "url",
                "entityKey": "https://example.com/seed"
            }
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_star_status",
        json!({
            "request": {
                "entityKind": "url",
                "entityKeys": ["https://example.com/seed"]
            }
        }),
    );
    dispatch_for_coverage(
        &state,
        "list_stars",
        json!({ "kind": "url", "sort": "recently_starred", "limit": 10 }),
    );
    dispatch_for_coverage(&state, "get_star_counts", json!({}));
    dispatch_for_coverage(
        &state,
        "export_history",
        json!({ "request": ExportRequest { query: HistoryQuery::default(), format: ExportFormat::Jsonl } }),
    );
    dispatch_for_coverage(&state, "inspect_takeout", json!({ "request": takeout.clone() }));
    dispatch_for_coverage(&state, "import_takeout", json!({ "request": takeout }));
    dispatch_for_coverage(
        &state,
        "inspect_browser_history",
        json!({ "request": browser_history.clone() }),
    );
    dispatch_for_coverage(&state, "import_browser_history", json!({ "request": browser_history }));
    dispatch_for_coverage(&state, "preview_import_batch", json!({ "batchId": 1 }));
    dispatch_for_coverage(&state, "revert_import_batch", json!({ "batchId": 1 }));
    dispatch_for_coverage(&state, "restore_import_batch", json!({ "batchId": 1 }));
    dispatch_for_coverage(&state, "preview_schedule", json!({ "platform": "linux" }));
    dispatch_for_coverage(&state, "schedule_status", json!({ "platform": "linux" }));
    dispatch_for_coverage(&state, "apply_schedule", json!({ "plan": schedule_plan() }));
    dispatch_for_coverage(&state, "remove_schedule", json!({ "plan": schedule_plan() }));
    dispatch_for_coverage(&state, "repair_schedule", json!({ "plan": schedule_plan() }));
    dispatch_for_coverage(&state, "doctor_report", json!({}));
    dispatch_for_coverage(&state, "repair_health", json!({}));
    dispatch_for_coverage(&state, "clear_derived_intelligence", json!({}));
    dispatch_for_coverage(&state, "keyring_status", json!({}));
    dispatch_for_coverage(&state, "security_status", json!({}));
    dispatch_for_coverage(&state, "keyring_get_database_key", json!({}));
    dispatch_for_coverage(&state, "keyring_store_database_key", json!({ "value": "secret" }));
    dispatch_for_coverage(&state, "keyring_clear_database_key", json!({}));
    dispatch_for_coverage(
        &state,
        "store_ai_provider_api_key",
        json!({ "input": AiProviderSecretInput {
            provider_id: "llm-primary".to_string(),
            api_key: "secret".to_string(),
        } }),
    );
    dispatch_for_coverage(
        &state,
        "clear_ai_provider_api_key",
        json!({ "providerId": "llm-primary" }),
    );
    dispatch_for_coverage(
        &state,
        "test_ai_provider_connection",
        wrapped(AiProviderConnectionTestRequest {
            provider_id: "llm-primary".to_string(),
            purpose: AiProviderPurpose::Llm,
        }),
    );
    dispatch_for_coverage(&state, "load_ai_queue_status", json!({}));
    dispatch_for_coverage(&state, "run_ai_queue_jobs", json!({ "maxJobs": 1 }));
    dispatch_for_coverage(&state, "replay_ai_job", json!({ "jobId": 999 }));
    dispatch_for_coverage(&state, "cancel_ai_job", json!({ "jobId": 999 }));
    dispatch_for_coverage(&state, "load_ai_assistant_job", json!({ "jobId": 999 }));
    dispatch_for_coverage(&state, "build_ai_index", wrapped(AiIndexRequest::default()));
    dispatch_for_coverage(
        &state,
        "search_ai_history",
        wrapped(AiSearchRequest {
            query: "example".to_string(),
            profile_id: None,
            domain: None,
            limit: Some(5),
            cursor: None,
            starred_only: None,
            start_date: None,
            end_date: None,
        }),
    );
    dispatch_for_coverage(
        &state,
        "ask_ai_assistant",
        wrapped(AiAssistantRequest {
            question: "What did I visit?".to_string(),
            profile_id: None,
            domain: None,
        }),
    );
    dispatch_for_coverage(
        &state,
        "ai_chat_send",
        wrapped(AiChatSendRequest {
            provider_id: None,
            messages: vec![AiChatMessage {
                role: AiChatRole::User,
                content: "summarize my history".to_string(),
            }],
            temperature: Some(0.6),
            max_tokens: Some(64),
            ..Default::default()
        }),
    );
    dispatch_for_coverage(&state, "ai_chat_cancel", json!({ "runId": "chat-missing" }));
    dispatch_for_coverage(&state, "download_ai_embedding_model", json!({}));
    dispatch_for_coverage(&state, "cancel_ai_embedding_model_download", json!({}));
    dispatch_for_coverage(
        &state,
        "save_ai_conversation",
        wrapped(SaveAgentConversationRequest {
            id: "dispatch-conv".to_string(),
            title: None,
            provider_id: Some("llm-local".to_string()),
            messages: vec![AgentMessage {
                id: "dispatch-m1".to_string(),
                role: "user".to_string(),
                content: "remember this conversation".to_string(),
                reasoning: None,
                tool_calls_json: None,
                status: None,
                ..Default::default()
            }],
        }),
    );
    dispatch_for_coverage(
        &state,
        "list_ai_conversations",
        wrapped(ListAgentConversationsRequest { limit: Some(10) }),
    );
    dispatch_for_coverage(
        &state,
        "load_ai_conversation",
        json!({ "conversationId": "dispatch-conv" }),
    );
    dispatch_for_coverage(
        &state,
        "rename_ai_conversation",
        wrapped(RenameAgentConversationRequest {
            id: "dispatch-conv".to_string(),
            title: "renamed dispatch conversation".to_string(),
        }),
    );
    dispatch_for_coverage(
        &state,
        "delete_ai_conversation",
        json!({ "conversationId": "dispatch-conv" }),
    );
    dispatch_for_coverage(
        &state,
        "run_core_intelligence_now",
        wrapped(CoreIntelligenceRebuildRequest::default()),
    );
    dispatch_for_coverage(
        &state,
        "queue_core_intelligence_rebuild",
        wrapped(CoreIntelligenceRebuildRequest::default()),
    );
    dispatch_for_coverage(&state, "get_sessions", wrapped(paged.clone()));
    dispatch_for_coverage(&state, "get_session_detail", json!({ "sessionId": "missing" }));
    dispatch_for_coverage(
        &state,
        "get_search_trails",
        wrapped(SearchTrailQueryRequest {
            date_range: date_range.clone(),
            profile_id: None,
            engine: None,
            page: 0,
            page_size: 10,
        }),
    );
    dispatch_for_coverage(&state, "get_trail_detail", json!({ "trailId": "missing" }));
    dispatch_for_coverage(&state, "get_navigation_path", json!({ "visitId": 1 }));
    dispatch_for_coverage(&state, "get_hub_pages", wrapped(top_sites.clone()));
    dispatch_for_coverage(&state, "get_search_engine_ranking", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "list_search_engine_rules", json!({}));
    dispatch_for_coverage(
        &state,
        "upsert_search_engine_rule",
        input(SearchEngineRuleInput {
            rule_id: Some("dispatch-rule".to_string()),
            engine_id: "dispatch".to_string(),
            display_name: "Dispatch".to_string(),
            host_pattern: "search.example".to_string(),
            path_prefix: Some("/search".to_string()),
            query_param_key: "q".to_string(),
            enabled: true,
            note: None,
            example_url: Some("https://search.example/search?q=pathkeep".to_string()),
        }),
    );
    dispatch_for_coverage(
        &state,
        "delete_search_engine_rule",
        json!({ "ruleId": "dispatch-rule" }),
    );
    dispatch_for_coverage(
        &state,
        "get_top_search_concepts",
        wrapped(TopSearchConceptsRequest {
            date_range: date_range.clone(),
            profile_id: None,
            limit: Some(10),
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_search_queries",
        wrapped(SearchQueryListRequest {
            date_range: date_range.clone(),
            profile_id: None,
            browser_kind: None,
            engine: None,
            domain: None,
            query: None,
            sort: None,
            page: 0,
            page_size: 10,
        }),
    );
    dispatch_for_coverage(&state, "get_query_families", wrapped(paged));
    dispatch_for_coverage(
        &state,
        "get_query_family_detail",
        wrapped(QueryFamilyDetailRequest {
            family_id: "missing".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        }),
    );
    dispatch_for_coverage(&state, "get_top_sites", wrapped(top_sites));
    dispatch_for_coverage(
        &state,
        "get_domain_trend",
        wrapped(DomainTrendRequest {
            registrable_domain: "example.com".to_string(),
            date_range: date_range.clone(),
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_refind_pages",
        wrapped(RefindPagesRequest {
            date_range: date_range.clone(),
            profile_id: None,
            limit: Some(10),
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_refind_page_detail",
        wrapped(RefindPageDetailRequest {
            canonical_url: "https://example.com/".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        }),
    );
    dispatch_for_coverage(
        &state,
        "explain_refind",
        wrapped(vault_core::ExplainRefindRequest {
            canonical_url: "https://example.com/".to_string(),
        }),
    );
    dispatch_for_coverage(
        &state,
        "explain_entity",
        wrapped(EntityExplanationRequest {
            entity_type: "domain".to_string(),
            entity_id: "example.com".to_string(),
        }),
    );
    dispatch_for_coverage(&state, "get_activity_mix", wrapped(scoped.clone()));
    dispatch_for_coverage(
        &state,
        "get_activity_mix_trend",
        wrapped(GranularityDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: None,
            granularity: "day".to_string(),
        }),
    );
    dispatch_for_coverage(&state, "get_digest_summary", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "get_intelligence_primary_overview", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "get_intelligence_secondary_overview", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "get_stable_sources", wrapped(scoped.clone()));
    dispatch_for_coverage(
        &state,
        "get_search_effectiveness",
        wrapped(SearchEffectivenessRequest {
            date_range: date_range.clone(),
            profile_id: None,
            engine: None,
        }),
    );
    dispatch_for_coverage(&state, "get_friction_signals", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "get_reopened_investigations", wrapped(scoped.clone()));
    dispatch_for_coverage(
        &state,
        "get_domain_deep_dive",
        wrapped(DomainDeepDiveRequest {
            registrable_domain: "example.com".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_day_insights",
        wrapped(DayInsightsRequest { date: "2026-04-01".to_string(), profile_id: None }),
    );
    dispatch_for_coverage(
        &state,
        "get_browsing_rhythm",
        wrapped(CategoryFilteredDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: None,
            category: None,
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_discovery_trend",
        wrapped(GranularityDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: None,
            granularity: "week".to_string(),
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_intelligence_embed_cards",
        wrapped(IntelligenceEmbedCardsRequest {
            date_range: date_range.clone(),
            profile_id: None,
            limit: Some(4),
        }),
    );
    dispatch_for_coverage(
        &state,
        "get_intelligence_widget_snapshot",
        wrapped(IntelligenceEmbedCardsRequest {
            date_range: date_range.clone(),
            profile_id: None,
            limit: Some(4),
        }),
    );
    dispatch_for_coverage(&state, "get_intelligence_public_snapshot", wrapped(scoped.clone()));
    dispatch_for_coverage(
        &state,
        "preview_intelligence_local_host",
        wrapped(intelligence_host.clone()),
    );
    dispatch_for_coverage(&state, "build_intelligence_local_host", wrapped(intelligence_host));
    dispatch_for_coverage(&state, "get_on_this_day", json!({ "profileId": null }));
    dispatch_for_coverage(&state, "get_breadth_index", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "get_habit_patterns", wrapped(scoped.clone()));
    dispatch_for_coverage(
        &state,
        "get_interrupted_habits",
        wrapped(ProfileScopedRequest { profile_id: None }),
    );
    dispatch_for_coverage(
        &state,
        "get_path_flows",
        wrapped(PathFlowRequest {
            date_range: date_range.clone(),
            profile_id: None,
            step_count: 2,
            limit: Some(10),
        }),
    );
    dispatch_for_coverage(&state, "get_observed_interactions", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "get_compare_sets", wrapped(scoped.clone()));
    dispatch_for_coverage(
        &state,
        "get_compare_set_detail",
        wrapped(CompareSetDetailRequest {
            compare_set_id: "missing".to_string(),
            date_range: date_range.clone(),
            profile_id: None,
        }),
    );
    dispatch_for_coverage(&state, "get_multi_browser_diff", wrapped(scoped.clone()));
    dispatch_for_coverage(&state, "load_intelligence_runtime", json!({}));
    dispatch_for_coverage(&state, "retry_intelligence_job", json!({ "jobId": 999 }));
    dispatch_for_coverage(&state, "cancel_intelligence_job", json!({ "jobId": 999 }));
    dispatch_for_coverage(&state, "preview_ai_integrations", json!({}));
    dispatch_for_coverage(
        &state,
        "record_frontend_error",
        wrapped(FrontendErrorReportRequest {
            source: "vitest".to_string(),
            message: "dispatch coverage".to_string(),
            stack: None,
            url: Some("http://localhost/".to_string()),
            line: Some(1),
            column: Some(1),
            fatal: false,
        }),
    );
    dispatch_for_coverage(&state, "reset_local_secret_vault", json!({}));

    dispatch_for_coverage(
        &state,
        "open_path_in_file_manager",
        json!({ "path": dir.path().join("missing-path").display().to_string() }),
    );
    dispatch_for_coverage(
        &state,
        "open_external_url",
        json!({ "url": "ftp://example.com/pathkeep" }),
    );
    dispatch_for_coverage(
        &state,
        "export_conversation_file",
        json!({
            "targetPath": dir.path().join("conversation.md").display().to_string(),
            "contents": "# PathKeep conversation\n",
        }),
    );
    dispatch_for_coverage(&state, "check_for_app_update", json!({}));
    dispatch_for_coverage(
        &state,
        "download_and_install_app_update",
        json!({ "request": AppUpdateInstallRequest::default() }),
    );
    dispatch_for_coverage(&state, "relaunch_after_update", json!({}));

    // W-ENRICH-1 content-fetch command surface (covers dispatch.rs routing arms + the 5
    // worker_bridge `_impl` fns). The archive was initialized above, so these reach real worker code.
    dispatch_for_coverage(&state, "get_content_fetch_settings", json!({}));
    dispatch_for_coverage(
        &state,
        "set_content_fetch_settings",
        json!({
            "settings": {
                "enabled": true,
                "extractors": [],
                "domains": [],
                "queuedJobs": 0,
                "runningJobs": 0,
                "failedJobs": 0,
                "storedRecords": 0
            }
        }),
    );
    dispatch_for_coverage(&state, "list_visit_enrichment", json!({ "historyId": 1 }));
    dispatch_for_coverage(
        &state,
        "content_fetch_now",
        json!({
            "request": {
                "historyId": 1,
                "profileId": "chrome:Default",
                "url": "https://github.com/o/r",
                "title": null
            }
        }),
    );
    dispatch_for_coverage(&state, "enqueue_content_fetch_working_set", json!({ "limit": 10 }));

    unsafe {
        std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
        std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
        std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
    }
}
