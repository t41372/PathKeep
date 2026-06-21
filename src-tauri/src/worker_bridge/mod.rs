//! Bridge from the Tauri command facade into `vault-worker`.
//!
//! The command modules are intentionally transport-shaped, while
//! `vault-worker` is task-shaped. This layer adapts between them:
//! session-state lookups, transient key updates, progress callbacks, and
//! uniform string error shaping.

mod annotations;
mod app;
mod archive;
mod import;
mod intelligence;
mod migration;
mod schedule;
mod security;
mod stars;

// `annotations::*` is exercised only via the production command facade
// (#[cfg(not(test))]); the worker_bridge test block doesn't drive notes/tags
// yet, so the glob is unused in test builds. Splitting it out keeps the noise
// localized — every sibling `_impl` already carries its own
// #[cfg_attr(test, allow(dead_code))] on the definition.
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use self::annotations::*;
// `migration::*` follows the same cfg-cleanliness pattern as annotations:
// the worker_bridge test block does not yet drive Export/Import (the
// integration tests live in `vault_core::migration::tests`), so each
// `_impl` already carries its own #[cfg_attr(test, allow(dead_code))]
// and the glob is only loaded for production builds.
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use self::migration::*;
// `stars::*` follows the same cfg-cleanliness pattern as annotations: the
// worker_bridge test block drives the star impls (see the stars coverage test),
// but each `_impl` still carries its own #[cfg_attr(test, allow(dead_code))] so
// the glob stays quiet in any build configuration.
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use self::stars::*;
pub(crate) use self::{app::*, archive::*, import::*, intelligence::*, schedule::*, security::*};

/// Normalizes worker/core errors into the string transport contract used by Tauri commands.
///
/// Uses the `{:#}` alternate Display formatter so `anyhow::Error` chains
/// surface as `"top: cause: root"` instead of just the top-level summary.
/// PathKeep is a local-only app and the user is *also* the bug reporter —
/// hiding the cause behind a generic frontend fallback ("…failed for an
/// unknown reason.") strips the one piece of information that would let
/// them file an actionable bug. Non-anyhow types fall back to plain
/// Display, which `{:#}` reduces to for any type that does not honour the
/// alternate flag.
fn worker_result<T, E: std::fmt::Display>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| {
        let message = format!("{error:#}");
        log::warn!(target: "pathkeep::worker_bridge", "{message}");
        message
    })
}

#[cfg(test)]
use crate::PRODUCT_DISPLAY_NAME;
#[cfg(test)]
use crate::session::{SessionState, session_key};
#[cfg(test)]
use crate::{entrypoint, run_with_arguments, write_payload};
#[cfg(test)]
use rusqlite::Connection;
#[cfg(test)]
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(test)]
use tempfile::tempdir;
#[cfg(test)]
use vault_core::*;
#[cfg(test)]
use vault_worker::RekeyRequest;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        CHROME_USER_DATA_OVERRIDE_ENV, PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV,
        lock_env,
    };

    fn initialized_config() -> AppConfig {
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            due_after_hours: 72.0,
            checkpoint_days: 1,
            selected_profile_ids: vec!["chrome:Default".to_string()],
            ..AppConfig::default()
        };
        config.ai.enabled = true;
        config.ai.assistant_enabled = true;
        config.ai.semantic_index_enabled = true;
        config.ai.mcp_enabled = true;
        config.ai.skill_enabled = true;
        config.ai.llm_provider_id = Some("llm-primary".to_string());
        config.ai.embedding_provider_id = Some("embed-primary".to_string());
        config.ai.llm_providers = vec![AiProviderConfig {
            id: "llm-primary".to_string(),
            name: "Primary LLM".to_string(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "gpt-4.1-mini".to_string(),
            ..AiProviderConfig::default()
        }];
        config.ai.embedding_providers = vec![AiProviderConfig {
            id: "embed-primary".to_string(),
            name: "Primary embedding".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "text-embedding-3-large".to_string(),
            dimensions: Some(1536),
            ..AiProviderConfig::default()
        }];
        config
    }

    fn chrome_user_data_fixture(root: &Path) -> PathBuf {
        let chrome_root = root.join("chrome-user-data");
        let profile_dir = chrome_root.join("Default");
        let now_chrome_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time")
            .as_micros() as i64
            + 11_644_473_600_000_000i64;
        fs::create_dir_all(&profile_dir).expect("create chrome profile dir");
        fs::write(chrome_root.join("Last Version"), "135.0.0.0").expect("write version");
        fs::write(
            chrome_root.join("Local State"),
            r#"{"profile":{"info_cache":{"Default":{"name":"Default","user_name":"fixture@example.test"}}}}"#,
        )
        .expect("write local state");

        let history = Connection::open(profile_dir.join("History")).expect("open source history");
        history
            .execute_batch(
                "
                CREATE TABLE urls (
                  id INTEGER PRIMARY KEY,
                  url TEXT NOT NULL,
                  title TEXT,
                  visit_count INTEGER NOT NULL,
                  typed_count INTEGER NOT NULL,
                  last_visit_time INTEGER NOT NULL,
                  hidden INTEGER NOT NULL
                );
                CREATE TABLE visits (
                  id INTEGER PRIMARY KEY,
                  url INTEGER NOT NULL,
                  visit_time INTEGER NOT NULL,
                  from_visit INTEGER,
                  transition INTEGER,
                  visit_duration INTEGER,
                  is_known_to_sync INTEGER,
                  visited_link_id INTEGER,
                  external_referrer_url TEXT,
                  app_id TEXT
                );
                CREATE TABLE downloads (
                  id INTEGER PRIMARY KEY,
                  guid TEXT,
                  current_path TEXT,
                  target_path TEXT,
                  start_time INTEGER,
                  received_bytes INTEGER,
                  total_bytes INTEGER,
                  state INTEGER,
                  mime_type TEXT,
                  original_mime_type TEXT
                );
                CREATE TABLE keyword_search_terms (
                  keyword_id INTEGER,
                  url_id INTEGER,
                  term TEXT,
                  normalized_term TEXT
                );",
            )
            .expect("create history schema");
        history
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, last_visit_time, hidden)
                 VALUES (1, 'https://www.google.com/search?q=example', 'example - Google Search', 1, 1, ?1, 0)",
                [now_chrome_micros],
            )
            .expect("insert url");
        history
            .execute(
                "INSERT INTO visits (id, url, visit_time, from_visit, transition, visit_duration, is_known_to_sync, visited_link_id, external_referrer_url, app_id)
                 VALUES (1, 1, ?1, NULL, 805306368, 24000, 1, 3, 'https://ref.example', 'com.example.app')",
                [now_chrome_micros],
            )
            .expect("insert visit");
        history
            .execute(
                "INSERT INTO downloads (id, guid, current_path, target_path, start_time, received_bytes, total_bytes, state, mime_type, original_mime_type)
                 VALUES (1, 'guid-1', '/tmp/current', '/tmp/target', 1, 1, 2, 3, 'text/html', 'text/plain')",
                [],
            )
            .expect("insert download");
        history
            .execute(
                "INSERT INTO keyword_search_terms (keyword_id, url_id, term, normalized_term)
                 VALUES (1, 1, 'browser history backup', 'browser history backup')",
                [],
            )
            .expect("insert search term");

        chrome_root
    }

    fn takeout_fixture(root: &Path) -> String {
        let source_dir = root.join("takeout-source");
        fs::create_dir_all(&source_dir).expect("takeout dir");
        fs::write(
            source_dir.join("takeout.jsonl"),
            r#"{"url":"https://example.com/takeout","title":"Takeout","visitedAt":"2026-04-01T10:00:00+00:00"}"#,
        )
        .expect("write takeout");
        source_dir.display().to_string()
    }

    #[test]
    fn command_helpers_cover_local_desktop_flows() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");
        let takeout_source = takeout_fixture(dir.path());

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let config = initialized_config();

        set_session_database_key_impl("transient-session-key".to_string(), &session)
            .expect("set transient session key");
        assert_eq!(session_key(&session), Some("transient-session-key".to_string()));
        clear_session_database_key_impl(&session).expect("clear transient session key");
        assert_eq!(session_key(&session), None);

        let snapshot =
            initialize_archive_impl(config.clone(), None, &session).expect("initialize archive");
        assert!(snapshot.archive_status.initialized);
        assert_eq!(
            save_config_impl(config.clone(), session_key(&session).as_deref())
                .expect("save config")
                .config
                .selected_profile_ids,
            config.selected_profile_ids
        );

        let backup =
            run_backup_now_impl(false, session_key(&session).as_deref(), |_| {}).expect("backup");
        assert_eq!(backup.run.expect("run").new_visits, 1);
        let backup_run_id = app_snapshot_impl(session_key(&session).as_deref())
            .expect("snapshot after backup")
            .recent_runs[0]
            .id;
        let backup_detail = audit_run_detail_impl(backup_run_id, session_key(&session).as_deref())
            .expect("backup audit detail");
        let snapshot_path = backup_detail.artifacts[0].path.clone();
        let restore_preview = preview_snapshot_restore_impl(
            vault_core::SnapshotRestoreRequest { snapshot_path: snapshot_path.clone() },
            &session,
        )
        .expect("preview snapshot restore");
        assert!(restore_preview.execute_supported);
        let restore_run = run_snapshot_restore_impl(
            vault_core::SnapshotRestoreRequest { snapshot_path },
            &session,
        )
        .expect("run snapshot restore");
        assert_eq!(restore_run.run.expect("snapshot restore run").run_type, "snapshot_restore");
        let retention_preview =
            preview_retention_prune_impl(&session).expect("preview retention prune");
        assert!(retention_preview.buckets.iter().any(|bucket| bucket.id == "snapshots"));
        let retention_result = run_retention_prune_impl(
            vault_core::RetentionPruneRequest { bucket_ids: vec!["snapshots".to_string()] },
            &session,
        )
        .expect("run retention prune");
        assert!(retention_result.deleted_files > 0);

        let history = query_history_impl(
            HistoryQuery {
                q: Some("example".to_string()),
                limit: Some(10),
                ..HistoryQuery::default()
            },
            session_key(&session).as_deref(),
        )
        .expect("query history");
        assert_eq!(history.total, 1);

        let export = export_history_impl(
            ExportRequest {
                query: HistoryQuery::default(),
                format: vault_core::ExportFormat::Jsonl,
            },
            session_key(&session).as_deref(),
        )
        .expect("export history");
        assert_eq!(export.count, 1);

        let inspected = inspect_takeout_impl(TakeoutRequest {
            source_path: takeout_source.clone(),
            dry_run: true,
        })
        .expect("inspect takeout");
        assert_eq!(inspected.candidate_items, 1);

        let imported = import_takeout_impl(
            TakeoutRequest { source_path: takeout_source, dry_run: false },
            session_key(&session).as_deref(),
            |_| {},
        )
        .expect("import takeout");
        let batch_id = imported.import_batch.expect("import batch").id;
        assert_eq!(imported.imported_items, 1);
        let import_preview = preview_import_batch_impl(batch_id, session_key(&session).as_deref())
            .expect("preview import batch");
        assert_eq!(import_preview.batch.status, "imported");
        assert_eq!(import_preview.preview_entries.len(), 1);
        let reverted_batch = revert_import_batch_impl(batch_id, session_key(&session).as_deref())
            .expect("revert import batch");
        assert_eq!(reverted_batch.batch.status, "reverted");
        assert_eq!(reverted_batch.batch.visible_items, 0);
        let restored_batch = restore_import_batch_impl(batch_id, session_key(&session).as_deref())
            .expect("restore import batch");
        assert_eq!(restored_batch.batch.status, "imported");
        assert_eq!(restored_batch.batch.visible_items, 1);

        let keyring = keyring_store_database_key_impl("session-secret".to_string())
            .expect("store database key");
        assert!(keyring.stored_secret);
        assert_eq!(keyring_status_impl().stored_secret, keyring.stored_secret);
        assert_eq!(
            keyring_get_database_key_impl().expect("read database key"),
            Some("session-secret".to_string())
        );
        assert!(!keyring_clear_database_key_impl().expect("clear keyring key").stored_secret);

        let provider_snapshot = store_ai_provider_api_key_impl(
            AiProviderSecretInput {
                provider_id: "llm-primary".to_string(),
                api_key: "secret".to_string(),
            },
            session_key(&session).as_deref(),
        )
        .expect("store provider key");
        assert!(provider_snapshot.config.ai.llm_providers[0].api_key_saved);
        let cleared_provider_snapshot = clear_ai_provider_api_key_impl(
            "llm-primary".to_string(),
            session_key(&session).as_deref(),
        )
        .expect("clear provider key");
        assert!(!cleared_provider_snapshot.config.ai.llm_providers[0].api_key_saved);
        let ai_index_report = build_ai_index_impl(
            AiIndexRequest {
                provider_id: None,
                full_rebuild: false,
                clear_only: false,
                limit: Some(5),
            },
            session_key(&session).as_deref(),
        )
        .expect("index build should queue a background job report");
        assert!(ai_index_report.job_id.is_some());
        assert!(ai_index_report.run_id.is_none());
        assert!(
            ai_index_report
                .notes
                .iter()
                .any(|note| note.contains("processing it in the background"))
        );
        let _ = run_ai_queue_jobs_impl(Some(1), session_key(&session).as_deref())
            .expect("drain queued index job before immediate assistant claim");

        let ai_search = search_ai_history_impl(
            AiSearchRequest {
                query: "example".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
            session_key(&session).as_deref(),
        )
        .expect("ai search");
        assert_eq!(ai_search.total, 2);

        let assistant_error = ask_ai_assistant_impl(
            AiAssistantRequest {
                question: "What did I visit?".to_string(),
                profile_id: None,
                domain: None,
            },
            session_key(&session).as_deref(),
        )
        .expect_err("assistant should require saved provider key");
        assert!(assistant_error.contains("API key"), "assistant error: {assistant_error}");

        let preview = preview_ai_integrations_impl().expect("preview ai integrations");
        assert!(preview.mcp_command.contains("mcp-server"));

        let plan = preview_schedule_impl(Some("linux".to_string())).expect("schedule preview");
        assert_eq!(plan.platform, "linux");
        let applied = apply_schedule_impl(plan).expect("apply schedule");
        assert!(!applied.applied);
        let removed = remove_schedule_impl(
            preview_schedule_impl(Some("linux".to_string())).expect("schedule preview for remove"),
        )
        .expect("remove schedule");
        assert!(!removed.applied);
        let schedule_status =
            schedule_status_impl(Some("linux".to_string()), session_key(&session).as_deref())
                .expect("schedule status");
        assert_eq!(schedule_status.install_state, "manual-review");

        let doctor = doctor_report_impl(session_key(&session).as_deref()).expect("doctor");
        assert!(!doctor.checks.is_empty());
        let runtime_before_rekey = (0..50)
            .find_map(|attempt| {
                let runtime = load_intelligence_runtime_impl(session_key(&session).as_deref())
                    .expect("load intelligence runtime before rekey");
                let only_deferred_network_backlog = runtime.queue.running == 0
                    && runtime.recent_jobs.iter().all(|job| {
                        job.state != "queued"
                            || job.plugin_id.as_deref() == Some("readable-content-refetch")
                    });
                if only_deferred_network_backlog {
                    Some(runtime)
                } else if attempt == 49 {
                    panic!("intelligence runtime did not go idle before rekey: {runtime:?}");
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    None
                }
            })
            .expect("idle intelligence runtime before rekey");
        assert_eq!(runtime_before_rekey.queue.running, 0);
        assert!(
            runtime_before_rekey.recent_jobs.iter().all(|job| {
                job.state != "queued"
                    || job.plugin_id.as_deref() == Some("readable-content-refetch")
            }),
            "expected only deferred network backlog before rekey: {runtime_before_rekey:?}"
        );

        let date_range =
            DateRange { start: "1970-01-01".to_string(), end: "2100-01-01".to_string() };
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
        let embed_cards = IntelligenceEmbedCardsRequest {
            date_range: date_range.clone(),
            profile_id: None,
            limit: Some(4),
        };

        let _ = run_core_intelligence_now_impl(
            CoreIntelligenceRebuildRequest::default(),
            session_key(&session).as_deref(),
        )
        .expect("run core intelligence through bridge");
        let queued_rebuild = queue_core_intelligence_rebuild_impl(
            CoreIntelligenceRebuildRequest::default(),
            session_key(&session).as_deref(),
        )
        .expect("queue core intelligence through bridge");
        assert!(queued_rebuild.job_id > 0);
        let _ = retry_intelligence_job_impl(999_999, session_key(&session).as_deref())
            .expect_err("missing intelligence job retry should fail");
        let _ = cancel_intelligence_job_impl(999_999, session_key(&session).as_deref())
            .expect_err("missing intelligence job cancel should fail");

        let _ = get_sessions_impl(paged.clone(), session_key(&session).as_deref())
            .expect("bridge sessions");
        let _ = get_session_detail_impl(
            "missing-session".to_string(),
            session_key(&session).as_deref(),
        );
        let _ = get_search_trails_impl(
            SearchTrailQueryRequest {
                date_range: date_range.clone(),
                profile_id: None,
                engine: None,
                page: 0,
                page_size: 10,
            },
            session_key(&session).as_deref(),
        );
        let _ =
            get_trail_detail_impl("missing-trail".to_string(), session_key(&session).as_deref());
        let _ = get_navigation_path_impl(1, session_key(&session).as_deref());
        let _ = get_hub_pages_impl(top_sites.clone(), session_key(&session).as_deref());
        let _ = get_search_engine_ranking_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = list_search_engine_rules_impl(session_key(&session).as_deref());
        let rules = upsert_search_engine_rule_impl(
            SearchEngineRuleInput {
                rule_id: Some("bridge-fixture-search".to_string()),
                engine_id: "bridge-fixture".to_string(),
                display_name: "Bridge Fixture".to_string(),
                host_pattern: "bridge-search.example".to_string(),
                path_prefix: Some("/search".to_string()),
                query_param_key: "q".to_string(),
                enabled: true,
                note: Some("bridge coverage fixture".to_string()),
                example_url: Some("https://bridge-search.example/search?q=pathkeep".to_string()),
            },
            session_key(&session).as_deref(),
        )
        .expect("bridge upsert search rule");
        assert!(rules.iter().any(|rule| rule.rule_id == "bridge-fixture-search"));
        let _ = delete_search_engine_rule_impl(
            "bridge-fixture-search".to_string(),
            session_key(&session).as_deref(),
        )
        .expect("bridge delete search rule");
        let _ = get_top_search_concepts_impl(
            TopSearchConceptsRequest {
                date_range: date_range.clone(),
                profile_id: None,
                limit: Some(10),
            },
            session_key(&session).as_deref(),
        );
        let _ = get_search_queries_impl(
            SearchQueryListRequest {
                date_range: date_range.clone(),
                profile_id: None,
                browser_kind: None,
                engine: None,
                domain: None,
                query: None,
                sort: None,
                page: 0,
                page_size: 10,
            },
            session_key(&session).as_deref(),
        );
        let _ = get_query_families_impl(paged, session_key(&session).as_deref());
        let _ = get_query_family_detail_impl(
            QueryFamilyDetailRequest {
                family_id: "missing-family".to_string(),
                date_range: date_range.clone(),
                profile_id: None,
            },
            session_key(&session).as_deref(),
        );
        let _ = get_top_sites_impl(top_sites, session_key(&session).as_deref());
        let _ = get_domain_trend_impl(
            DomainTrendRequest {
                registrable_domain: "example.com".to_string(),
                date_range: date_range.clone(),
            },
            session_key(&session).as_deref(),
        );
        let _ = get_refind_pages_impl(
            RefindPagesRequest {
                date_range: date_range.clone(),
                profile_id: None,
                limit: Some(10),
            },
            session_key(&session).as_deref(),
        );
        let _ = get_refind_page_detail_impl(
            RefindPageDetailRequest {
                canonical_url: "https://example.com/".to_string(),
                date_range: date_range.clone(),
                profile_id: None,
            },
            session_key(&session).as_deref(),
        );
        let _ = explain_refind_impl(
            ExplainRefindRequest { canonical_url: "https://example.com/".to_string() },
            session_key(&session).as_deref(),
        );
        let _ = explain_entity_impl(
            EntityExplanationRequest {
                entity_type: "domain".to_string(),
                entity_id: "example.com".to_string(),
            },
            session_key(&session).as_deref(),
        );
        let _ = get_activity_mix_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_activity_mix_trend_impl(
            GranularityDateRangeRequest {
                date_range: date_range.clone(),
                profile_id: None,
                granularity: "day".to_string(),
            },
            session_key(&session).as_deref(),
        );
        let _ = get_digest_summary_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_intelligence_primary_overview_impl(
            scoped.clone(),
            session_key(&session).as_deref(),
        );
        let _ = get_intelligence_secondary_overview_impl(
            scoped.clone(),
            session_key(&session).as_deref(),
        );
        let _ = get_stable_sources_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_search_effectiveness_impl(
            SearchEffectivenessRequest {
                date_range: date_range.clone(),
                profile_id: None,
                engine: None,
            },
            session_key(&session).as_deref(),
        );
        let _ = get_friction_signals_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_reopened_investigations_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_domain_deep_dive_impl(
            DomainDeepDiveRequest {
                registrable_domain: "example.com".to_string(),
                date_range: date_range.clone(),
                profile_id: None,
            },
            session_key(&session).as_deref(),
        );
        let _ = get_day_insights_impl(
            DayInsightsRequest { date: "2026-04-01".to_string(), profile_id: None },
            session_key(&session).as_deref(),
        );
        let _ = get_browsing_rhythm_impl(
            CategoryFilteredDateRangeRequest {
                date_range: date_range.clone(),
                profile_id: None,
                category: None,
            },
            session_key(&session).as_deref(),
        );
        let _ = get_discovery_trend_impl(
            GranularityDateRangeRequest {
                date_range: date_range.clone(),
                profile_id: None,
                granularity: "week".to_string(),
            },
            session_key(&session).as_deref(),
        );
        let _ = get_on_this_day_impl(None, session_key(&session).as_deref());
        let _ = get_intelligence_embed_cards_impl(
            embed_cards.clone(),
            session_key(&session).as_deref(),
        );
        let _ =
            get_intelligence_widget_snapshot_impl(embed_cards, session_key(&session).as_deref());
        let _ =
            get_intelligence_public_snapshot_impl(scoped.clone(), session_key(&session).as_deref());
        let local_host_request = IntelligenceLocalHostRequest {
            date_range: date_range.clone(),
            profile_id: None,
            locale: "en".to_string(),
        };
        let _ = preview_intelligence_local_host_impl(
            local_host_request.clone(),
            session_key(&session).as_deref(),
        );
        let _ = build_intelligence_local_host_impl(
            local_host_request,
            session_key(&session).as_deref(),
        );
        let _ = get_breadth_index_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_habit_patterns_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_interrupted_habits_impl(
            ProfileScopedRequest { profile_id: None },
            session_key(&session).as_deref(),
        );
        let _ = get_path_flows_impl(
            PathFlowRequest {
                date_range: date_range.clone(),
                profile_id: None,
                step_count: 2,
                limit: Some(10),
            },
            session_key(&session).as_deref(),
        );
        let _ = get_observed_interactions_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_compare_sets_impl(scoped.clone(), session_key(&session).as_deref());
        let _ = get_compare_set_detail_impl(
            CompareSetDetailRequest {
                compare_set_id: "missing-compare-set".to_string(),
                date_range,
                profile_id: None,
            },
            session_key(&session).as_deref(),
        );
        let _ = get_multi_browser_diff_impl(scoped, session_key(&session).as_deref());

        let rekey_preview = preview_rekey_archive_impl(
            RekeyRequest { new_mode: ArchiveMode::Encrypted, new_key: None },
            &session,
        )
        .expect("preview rekey archive");
        assert!(rekey_preview.requires_new_key);
        let rekeyed_snapshot = rekey_archive_impl(
            RekeyRequest {
                new_mode: ArchiveMode::Encrypted,
                new_key: Some("vault-passphrase".to_string()),
            },
            &session,
        )
        .expect("rekey archive");
        assert!(rekeyed_snapshot.archive_status.encrypted);
        let cleared_derived = clear_derived_intelligence_impl(session_key(&session).as_deref())
            .expect("clear derived intelligence");
        assert!(!cleared_derived.notes.is_empty());

        let snapshot_again =
            app_snapshot_impl(session_key(&session).as_deref()).expect("app snapshot");
        assert_eq!(snapshot_again.browser_profiles.len(), 1);
        assert!(snapshot_again.archive_status.encrypted);
        let security_status =
            security_status_impl(session_key(&session).as_deref()).expect("security status");
        assert_eq!(security_status.mode, "encrypted");
        fs::write(dir.path().join("vault.hold"), "secret vault").expect("write stronghold fixture");
        reset_local_secret_vault_impl().expect("reset local secret vault");
        assert!(!dir.path().join("vault.hold").exists());
        let worker_payload = run_with_arguments(&[
            "pathkeep".to_string(),
            "--worker".to_string(),
            "doctor".to_string(),
        ])
        .expect("run worker doctor")
        .expect("worker payload");
        assert!(worker_payload.contains("checks"));
        assert!(run_with_arguments(&["pathkeep".to_string()]).is_ok());

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn worker_bridge_covers_dashboard_audit_and_ai_wrapper_edges() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let config = initialized_config();
        initialize_archive_impl(config.clone(), Some("vault-passphrase".to_string()), &session)
            .expect("initialize archive");
        save_config_impl(config, session_key(&session).as_deref()).expect("save config");
        let report = run_backup_now_impl(false, session_key(&session).as_deref(), |_| {})
            .expect("run backup");
        let run_id = report.run.expect("backup run").id;

        let dashboard =
            dashboard_snapshot_impl(session_key(&session).as_deref()).expect("dashboard snapshot");
        assert_eq!(dashboard.total_visits, 1);
        assert!(!dashboard.recent_runs.is_empty());

        let detail = audit_run_detail_impl(run_id, session_key(&session).as_deref())
            .expect("audit run detail");
        assert_eq!(detail.run.id, run_id);

        let repair =
            repair_health_impl(session_key(&session).as_deref()).expect("repair health report");
        assert!(repair.run_id.is_none() || repair.run_id > Some(run_id));

        let connection_probe = test_ai_provider_connection_impl(
            AiProviderConnectionTestRequest {
                provider_id: "embed-primary".to_string(),
                purpose: AiProviderPurpose::Embedding,
            },
            session_key(&session).as_deref(),
        )
        .expect("missing provider key should surface in the report");
        assert!(!connection_probe.ok);
        assert_eq!(connection_probe.error_code.as_deref(), Some("secret-missing"));

        let queue = load_ai_queue_status_impl(session_key(&session).as_deref())
            .expect("load ai queue status");
        assert!(queue.recent_jobs.is_empty());
        let drained = run_ai_queue_jobs_impl(None, session_key(&session).as_deref())
            .expect("run empty ai queue");
        assert!(drained.recent_jobs.is_empty());

        let replay = replay_ai_job_impl(999, session_key(&session).as_deref())
            .expect_err("missing ai job should not replay");
        assert!(replay.contains("999"));
        let cancel = cancel_ai_job_impl(999, session_key(&session).as_deref())
            .expect_err("missing ai job should not cancel");
        assert!(cancel.contains("999"));
        let assistant_job = load_ai_assistant_job_impl(999, session_key(&session).as_deref())
            .expect_err("missing assistant job should not load");
        assert!(assistant_job.contains("999"));

        let intelligence_run = run_core_intelligence_now_impl(
            vault_core::CoreIntelligenceRebuildRequest::default(),
            session_key(&session).as_deref(),
        )
        .expect("core intelligence run should complete");
        assert!(!intelligence_run.last_run_at.is_empty());
        let sessions = get_sessions_impl(
            vault_core::PagedDateRangeRequest {
                date_range: vault_core::DateRange {
                    start: "1970-01-01".to_string(),
                    end: "2100-01-01".to_string(),
                },
                profile_id: None,
                page: 0,
                page_size: 10,
            },
            session_key(&session).as_deref(),
        )
        .expect("sessions should load after the rebuild");
        assert!(sessions.total >= 1);
        let refind_pages = get_refind_pages_impl(
            vault_core::RefindPagesRequest {
                date_range: vault_core::DateRange {
                    start: "1970-01-01".to_string(),
                    end: "2100-01-01".to_string(),
                },
                profile_id: None,
                limit: Some(10),
            },
            session_key(&session).as_deref(),
        )
        .expect("refind pages should load");
        if let Some(page) = refind_pages.data.first() {
            let explanation = explain_refind_impl(
                vault_core::ExplainRefindRequest { canonical_url: page.canonical_url.clone() },
                session_key(&session).as_deref(),
            )
            .expect("refind explanation should load");
            assert!(!explanation.factors.is_empty());
        }

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn build_info_exposes_version_and_git_metadata() {
        let info = app_build_info_impl();
        assert_eq!(info.product_name, PRODUCT_DISPLAY_NAME);
        assert!(!info.version.is_empty());
        assert!(!info.git_commit_short.is_empty());
        assert!(!info.git_commit_full.is_empty());
    }

    #[test]
    fn annotations_and_og_image_impls_cover_local_desktop_flows() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let config = initialized_config();
        initialize_archive_impl(config.clone(), None, &session).expect("initialize archive");
        save_config_impl(config, session_key(&session).as_deref()).expect("save config");
        run_backup_now_impl(false, session_key(&session).as_deref(), |_| {}).expect("backup");

        // Annotations PME loop: set notes, replace tags, read back, list, search.
        let url = "https://example.com/seed";
        assert!(
            super::get_annotation_impl(session_key(&session).as_deref(), url)
                .expect("read missing annotation")
                .is_none(),
            "fresh archive must not have annotations",
        );

        super::set_notes_impl(
            session_key(&session).as_deref(),
            vault_core::SetNotesRequest {
                url: url.to_string(),
                notes: "rust internals research".to_string(),
                source_profile: None,
            },
        )
        .expect("set notes");

        super::replace_tags_impl(
            session_key(&session).as_deref(),
            vault_core::ReplaceTagsRequest {
                url: url.to_string(),
                tags: vec!["rust".to_string(), "tokio".to_string()],
                source_profile: None,
            },
        )
        .expect("replace tags");

        let after = super::get_annotation_impl(session_key(&session).as_deref(), url)
            .expect("read after writes")
            .expect("annotation present");
        assert_eq!(after.notes, "rust internals research");
        assert!(after.tags.iter().any(|tag| tag == "rust"));

        let listed = super::list_annotations_impl(session_key(&session).as_deref(), Some(10))
            .expect("list annotations");
        assert!(listed.iter().any(|record| record.url == url));

        let searched =
            super::search_annotations_impl(session_key(&session).as_deref(), "internals", Some(10))
                .expect("search annotations");
        assert!(searched.iter().any(|record| record.url == url));

        // Stars PME loop — toggle, batched status, list, and counts across
        // both the worker_bridge `_impl` surface AND the vault-worker fns it
        // delegates to (cross-crate coverage runs under `--workspace`).
        super::set_star_impl(
            session_key(&session).as_deref(),
            vault_core::SetStarRequest {
                entity_kind: vault_core::StarEntityKind::Url,
                entity_key: url.to_string(),
                source_profile: Some("chrome:Default".to_string()),
            },
        )
        .expect("set url star");
        super::set_star_impl(
            session_key(&session).as_deref(),
            vault_core::SetStarRequest {
                entity_kind: vault_core::StarEntityKind::Domain,
                entity_key: "example.com".to_string(),
                source_profile: None,
            },
        )
        .expect("set domain star");

        let status = super::is_starred_batch_impl(
            session_key(&session).as_deref(),
            vault_core::StarEntityKind::Url,
            &[url.to_string()],
        )
        .expect("read star status");
        assert_eq!(status.get(url), Some(&true));

        let listed_stars = super::list_stars_impl(
            session_key(&session).as_deref(),
            None,
            vault_core::StarSort::MostRevisited,
            Some(50),
        )
        .expect("list stars");
        assert!(listed_stars.iter().any(|item| item.entity_key == url));

        let counts =
            super::star_counts_impl(session_key(&session).as_deref()).expect("star counts");
        assert_eq!(counts.urls, 1);
        assert_eq!(counts.domains, 1);

        super::unset_star_impl(
            session_key(&session).as_deref(),
            vault_core::SetStarRequest {
                entity_kind: vault_core::StarEntityKind::Url,
                entity_key: url.to_string(),
                source_profile: None,
            },
        )
        .expect("unset url star");
        let after_unstar = super::star_counts_impl(session_key(&session).as_deref())
            .expect("star counts after unstar");
        assert_eq!(after_unstar.urls, 0);

        // Og:image impls — every PME boundary except the network refetch
        // (covered by og_images_fetch unit tests). Empty + no-blob inputs
        // exercise the bulk path; stats / cleanup / clear close the loop.
        let empty_lookup =
            super::load_history_og_images_impl(Vec::new(), session_key(&session).as_deref())
                .expect("empty og:image lookup");
        assert!(empty_lookup.is_empty());

        super::mark_og_images_shown_impl(vec![url.to_string()], session_key(&session).as_deref())
            .expect("mark og:image shown for URL with no cached blob");

        let _initial_stats = super::og_image_storage_stats_impl(session_key(&session).as_deref())
            .expect("og:image storage stats");

        let _cleanup = super::run_og_image_cleanup_impl(session_key(&session).as_deref())
            .expect("og:image cleanup pass");

        let _cleared = super::clear_og_image_cache_impl(session_key(&session).as_deref())
            .expect("clear og:image cache");

        // Refetch impl: empty URL list + blocked-host path + fetch_enabled=false fast-path.
        let zero = super::refetch_og_images_impl(Vec::new(), session_key(&session).as_deref())
            .expect("refetch with empty url list");
        assert_eq!(zero, 0);

        // Block a host in the user's config so the refetch worker takes
        // the blocked_outcome branch instead of hitting the network.
        let mut blocked_config = initialized_config();
        blocked_config.og_image.blocked_hosts = vec!["blocked.example.test".to_string()];
        save_config_impl(blocked_config, session_key(&session).as_deref())
            .expect("save blocked-hosts config");
        let blocked_count = super::refetch_og_images_impl(
            vec!["https://blocked.example.test/post".to_string()],
            session_key(&session).as_deref(),
        )
        .expect("refetch with a blocked host");
        assert_eq!(blocked_count, 0, "blocked host should not count as success");

        let mut disabled_config = initialized_config();
        disabled_config.og_image.fetch_enabled = false;
        save_config_impl(disabled_config, session_key(&session).as_deref())
            .expect("save og-image-disabled config");
        let disabled = super::refetch_og_images_impl(
            vec!["https://example.com/some-page".to_string()],
            session_key(&session).as_deref(),
        )
        .expect("refetch with fetch_enabled=false");
        assert_eq!(disabled, 0);

        // fetch_mode = Off with fetch_enabled = true is the modern policy
        // for "no network fetches anywhere". The implicit on-demand IPC
        // path (this fn) must short-circuit even though the legacy kill
        // switch is on. Settings copy promises Off = "No fetching anywhere",
        // and that promise lives here.
        let mut off_mode_config = initialized_config();
        off_mode_config.og_image.fetch_enabled = true;
        off_mode_config.og_image.fetch_mode = vault_core::OgImageFetchMode::Off;
        save_config_impl(off_mode_config, session_key(&session).as_deref())
            .expect("save fetch_mode=Off config");
        let off_mode = super::refetch_og_images_impl(
            vec!["https://example.com/another-page".to_string()],
            session_key(&session).as_deref(),
        )
        .expect("refetch with fetch_mode=Off");
        assert_eq!(off_mode, 0);

        // Drive the `Err(error) => return Err(error.to_string())` arm
        // of `refetch_og_images_impl`. The outer `effective_og_image_fetch_mode`
        // helper hydrates config through `load_unlocked_config`, which
        // refuses to return a hydrated `AppConfig` when an enabled App
        // Lock is currently locked. Configure that state, lock the
        // session, and the bridge surface refuses the implicit on-
        // demand fetch with a typed error string instead of silently
        // initiating outbound HTTP.
        let mut locked_config = initialized_config();
        locked_config.app_lock.enabled = true;
        set_app_lock_passcode_impl(SetAppLockPasscodeRequest {
            passcode: "1357".to_string(),
            recovery_hint: None,
        })
        .expect("set app lock passcode for refetch err-arm coverage");
        save_config_impl(locked_config, session_key(&session).as_deref())
            .expect("save app-lock-enabled config");
        lock_app_session_impl(Some("manual".to_string()))
            .expect("lock app session for refetch err-arm coverage");
        let err = super::refetch_og_images_impl(
            vec!["https://example.com/while-locked".to_string()],
            session_key(&session).as_deref(),
        )
        .expect_err("refetch must surface lock error from effective_mode helper");
        assert!(
            err.contains("currently locked"),
            "expected locked-session error context, got {err:?}",
        );
        // Restore the unlocked-baseline so subsequent assertions in this
        // test (and other tests sharing the env override) see a normal
        // unlocked state. Clearing the passcode now requires an unlocked
        // session (server-side lock enforcement), so unlock with the
        // configured passcode first.
        unlock_app_session_impl(UnlockAppSessionRequest {
            passcode: Some("1357".to_string()),
            use_biometric: false,
        })
        .expect("unlock before clearing app lock for follow-up tests");
        clear_app_lock_passcode_impl().expect("clear app lock for follow-up tests");

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn app_lock_bridge_guards_desktop_read_models_until_unlock() {
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let mut config = initialized_config();
        initialize_archive_impl(config.clone(), None, &session).expect("initialize archive");
        run_backup_now_impl(false, session_key(&session).as_deref(), |_| {}).expect("backup");

        let passcode_status = set_app_lock_passcode_impl(SetAppLockPasscodeRequest {
            passcode: "2468".to_string(),
            recovery_hint: Some("desk drawer".to_string()),
        })
        .expect("set app lock passcode");
        assert!(passcode_status.passcode_configured);
        let app_lock_status = app_lock_status_impl().expect("load app lock status");
        assert!(app_lock_status.passcode_configured);

        config.app_lock.enabled = true;
        let saved_snapshot =
            save_config_impl(config, session_key(&session).as_deref()).expect("enable app lock");
        assert!(saved_snapshot.config.app_lock.enabled);

        let locked = lock_app_session_impl(Some("manual".to_string())).expect("lock app session");
        assert!(locked.locked);
        assert_eq!(locked.lock_reason.as_deref(), Some("manual"));

        let snapshot_error =
            app_snapshot_impl(session_key(&session).as_deref()).expect_err("snapshot should block");
        assert!(snapshot_error.contains("currently locked"));

        let dashboard_error = dashboard_snapshot_impl(session_key(&session).as_deref())
            .expect_err("dashboard should block");
        assert!(dashboard_error.contains("currently locked"));

        let unlock_error = unlock_app_session_impl(UnlockAppSessionRequest {
            passcode: Some("9999".to_string()),
            use_biometric: false,
        })
        .expect_err("wrong passcode should fail");
        assert!(unlock_error.contains("did not match"));

        let unlocked = unlock_app_session_impl(UnlockAppSessionRequest {
            passcode: Some("2468".to_string()),
            use_biometric: false,
        })
        .expect("unlock app session");
        assert!(!unlocked.locked);

        let snapshot =
            app_snapshot_impl(session_key(&session).as_deref()).expect("snapshot after unlock");
        assert!(snapshot.archive_status.initialized);
        let dashboard =
            dashboard_snapshot_impl(session_key(&session).as_deref()).expect("dashboard");
        assert!(!dashboard.recent_runs.is_empty());
        let cleared = clear_app_lock_passcode_impl().expect("clear app lock passcode");
        assert!(!cleared.enabled);
        assert!(!cleared.passcode_configured);

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn migration_and_browse_insights_impls_cover_local_desktop_flows() {
        // Drives the Settings → Data Migration wrappers (export / preview /
        // apply) and the Browse-day insights / og:image prefetch wrappers
        // through their `_impl` entry points. These are thin adapters over
        // `vault_worker::*` whose only desktop-layer behaviour is the
        // string-error transport shape (`worker_result`). We need a real
        // initialised project root so the worker's `load_unlocked_config`
        // call succeeds — otherwise the wrappers short-circuit before
        // touching the bundle / aggregation paths we care about for
        // coverage.
        let _guard = lock_env();
        let dir = tempdir().expect("tempdir");
        let chrome_root = chrome_user_data_fixture(dir.path());
        let keyring_root = dir.path().join("test-keyring");

        unsafe {
            std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, dir.path());
            std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, &chrome_root);
            std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, &keyring_root);
        }

        let session = SessionState::default();
        let config = initialized_config();
        initialize_archive_impl(config.clone(), None, &session).expect("initialize archive");
        save_config_impl(config, session_key(&session).as_deref()).expect("save config");
        run_backup_now_impl(false, session_key(&session).as_deref(), |_| {}).expect("backup");

        // Browse-day insights: aggregates one local calendar day from the
        // archive. The seeded Chrome fixture emits a single visit dated
        // "today" in the host's local zone, so the request only needs to
        // succeed — we don't depend on a specific count here.
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let insights = super::browse_day_insights_impl(
            session_key(&session).as_deref(),
            BrowseDayInsightsRequest { date: today.clone(), profile_id: None },
        )
        .expect("browse day insights wrapper");
        assert_eq!(insights.date, today);
        assert_eq!(insights.hour_buckets.len(), 24);

        // og:image on-demand prefetch: budget=0 takes the fast-path inside
        // the worker (no network, no archive scan) so this is safe inside
        // the test harness even with the seed fixture present. The wrapper
        // still exercises `worker_result` around the `(u32, u32)` return.
        let prefetch = super::prefetch_og_images_impl(0, session_key(&session).as_deref())
            .expect("prefetch og images wrapper");
        assert_eq!(prefetch, (0, 0));

        // Export / Import round-trip. Re-uses the just-initialised project
        // as the export source; importing it back onto the same tree
        // exercises the `confirm_overwrite=true` overwrite branch.
        let bundle_target = dir.path().join("bundle.pathkeep");
        let exported =
            super::export_app_data_impl(session_key(&session).as_deref(), bundle_target.clone())
                .expect("export app data wrapper");
        assert_eq!(exported.bundle_path, bundle_target);
        assert!(bundle_target.exists(), "bundle file should land at the chosen target path");

        let preview = super::preview_app_data_import_impl(bundle_target.clone())
            .expect("preview app data import wrapper");
        assert!(
            preview.will_overwrite_existing,
            "preview should flag overwrite because the source tree is already initialised",
        );

        let applied = super::apply_app_data_import_impl(
            session_key(&session).as_deref(),
            bundle_target.clone(),
            vault_core::ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        )
        .expect("apply app data import wrapper");
        assert_eq!(applied.final_schema_version, vault_core::archive::max_schema_version());

        // Error transport: a missing bundle should surface through
        // `worker_result` as a non-empty string carrying the cause chain.
        let bogus = dir.path().join("does-not-exist.pathkeep");
        let preview_err = super::preview_app_data_import_impl(bogus)
            .expect_err("missing bundle should surface as a string error");
        assert!(!preview_err.is_empty());

        unsafe {
            std::env::remove_var(PROJECT_ROOT_OVERRIDE_ENV);
            std::env::remove_var(CHROME_USER_DATA_OVERRIDE_ENV);
            std::env::remove_var(TEST_KEYRING_OVERRIDE_ENV);
        }
    }

    #[test]
    fn entrypoint_and_payload_writer_cover_stdout_paths() {
        entrypoint().expect("entrypoint");

        let mut output = Vec::new();
        write_payload(&mut output, Some("worker-payload".to_string())).expect("write payload");
        write_payload(&mut output, None).expect("write empty payload");
        assert_eq!(String::from_utf8(output).expect("utf8"), "worker-payload\n");
    }
}
