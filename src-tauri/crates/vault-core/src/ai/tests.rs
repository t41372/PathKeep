//! Regression tests for optional AI and semantic retrieval helpers.
use super::indexing::{clear_provider_embeddings, collect_stale_history_ids, upsert_embedding};
use super::provider::should_retry_embedding_error;
use super::*;
use crate::{
    archive::ensure_archive_initialized,
    config::project_paths_with_root,
    models::{AiSettings, ArchiveMode},
};
use browser_history_parser::chromium::chrome_time_to_unix_ms;
use rusqlite::{Connection, params};
use std::{
    fs,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
};
use tokio::runtime::Runtime;

fn test_paths() -> ProjectPaths {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "pathkeep-ai-test-{}-{}-{}",
        std::process::id(),
        unique,
        sequence
    ));
    fs::create_dir_all(&root).expect("create temp root");
    project_paths_with_root(&root)
}

fn base_config() -> AppConfig {
    let mut llm_config = llm_provider().config;
    llm_config.api_key_saved = true;
    let mut embedding_config = embedding_provider().config;
    embedding_config.api_key_saved = true;
    AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        git_enabled: false,
        ai: AiSettings {
            enabled: true,
            assistant_enabled: true,
            semantic_index_enabled: true,
            llm_provider_id: Some("llm".to_string()),
            embedding_provider_id: Some("embed".to_string()),
            llm_providers: vec![llm_config],
            embedding_providers: vec![embedding_config],
            ..AiSettings::default()
        },
        ..AppConfig::default()
    }
}

fn embedding_provider() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "embed".to_string(),
            name: "Embedding provider".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "text-embedding-3-small".to_string(),
            dimensions: Some(3),
            ..AiProviderConfig::default()
        },
        api_key: "secret".to_string(),
    }
}

#[cfg(coverage)]
fn embedding_provider_with_id(id: &str) -> AiProviderRuntime {
    let mut provider = embedding_provider();
    provider.config.id = id.to_string();
    provider.config.name = format!("Embedding provider {id}");
    provider
}

fn llm_provider() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: "llm".to_string(),
            name: "LLM provider".to_string(),
            purpose: AiProviderPurpose::Llm,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            default_model: "gpt-4.1-mini".to_string(),
            ..AiProviderConfig::default()
        },
        api_key: "secret".to_string(),
    }
}

#[derive(Debug)]
struct CountdownControl {
    checkpoints_before_cancel: AtomicUsize,
}

impl CountdownControl {
    fn new(checkpoints_before_cancel: usize) -> Self {
        Self { checkpoints_before_cancel: AtomicUsize::new(checkpoints_before_cancel) }
    }
}

impl AiRunControl for CountdownControl {
    fn checkpoint(&self, detail: &str) -> Result<()> {
        let remaining = self
            .checkpoints_before_cancel
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_sub(1))
            })
            .unwrap_or(0);
        if remaining == 0 {
            return Err(AiRunCancelled::new(detail).into());
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        self.checkpoints_before_cancel.load(Ordering::Relaxed) == 0
    }
}

struct DefaultCancelledControl;

impl AiRunControl for DefaultCancelledControl {
    fn checkpoint(&self, _detail: &str) -> Result<()> {
        Ok(())
    }
}

#[test]
fn run_control_default_cancelled_is_false() {
    let control = DefaultCancelledControl;
    control.checkpoint("not cancelled").expect("default checkpoint");
    assert!(!control.cancelled());
}

fn llm_provider_with_format(request_format: AiRequestFormat) -> AiProviderRuntime {
    let mut provider = llm_provider();
    provider.config.request_format = request_format;
    provider
}

fn expected_stub_embedding(provider_id: &str, query: &str, dimensions: usize) -> Vec<f32> {
    let fingerprint = sha256_hex(format!("{provider_id}::{query}").as_bytes());
    let bytes = fingerprint.as_bytes();
    (0..dimensions).map(|index| ((bytes[index % bytes.len()] % 13) as f32 + 1.0) / 13.0).collect()
}

fn seed_visit(
    connection: &Connection,
    history_id: i64,
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visit_time: i64,
) {
    let browser_kind = profile_id.split(':').next().unwrap_or("legacy");
    let profile_row_id = profile_id.bytes().fold(0_i64, |acc, value| acc + value as i64).max(1);
    connection
        .execute(
            "INSERT OR IGNORE INTO archive.runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', ?1, 'UTC', 'success', '[]', '[]', '{}', 0)",
            [now_rfc3339()],
        )
        .expect("insert canonical run");
    connection
        .execute(
            "INSERT OR IGNORE INTO archive.source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (?1, ?2, 'test', ?3, ?4, ?5, 1, ?6, ?5)",
            params![
                profile_row_id,
                browser_kind,
                profile_id,
                format!("/tmp/{profile_id}"),
                now_rfc3339(),
                profile_id,
            ],
        )
        .expect("insert canonical profile");
    connection
        .execute(
            "INSERT OR IGNORE INTO archive.urls
             (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, ?6, 1, ?1, 0, ?7, ?8)",
            params![
                history_id,
                url,
                title,
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time),
                profile_row_id,
                format!("payload-{history_id}"),
                now_rfc3339()
            ],
        )
        .expect("insert canonical url");
    connection
        .execute(
            "INSERT INTO archive.visits
             (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 805306368, 0, ?6, 1, NULL, 1, 0, NULL, NULL, ?7, ?8, ?9)",
            params![
                history_id,
                history_id,
                history_id.to_string(),
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time),
                profile_row_id,
                format!("fp-{history_id}"),
                format!("payload-{history_id}"),
                now_rfc3339(),
            ],
        )
        .expect("insert visit");
}

fn seed_embedding(
    connection: &Connection,
    history_id: i64,
    provider: &AiProviderRuntime,
    content_hash: &str,
) {
    connection
        .execute(
            "INSERT INTO ai_embeddings
             (history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at)
             VALUES (?1, 'chrome:Default', 'https://example.com', 'Example', 'example.com', '2026-04-04T00:00:00Z', ?2, 7, ?3, ?4, ?5)",
            params![
                history_id,
                content_hash,
                provider.config.id,
                provider.config.default_model,
                now_rfc3339()
            ],
        )
        .expect("insert embedding");
}

fn seed_embedding_with_vector(
    connection: &Connection,
    history_id: i64,
    provider: &AiProviderRuntime,
    _vector: &[f32],
) {
    connection
        .execute(
            "INSERT INTO ai_embeddings
             (history_id, profile_id, url, title, domain, visited_at, content_hash, content_bytes, provider_id, model, indexed_at)
             VALUES (?1, 'chrome:Default', 'https://example.com', 'Example', 'example.com', '2026-04-04T00:00:00Z', ?2, 7, ?3, ?4, ?5)",
            params![
                history_id,
                format!("hash-{history_id}"),
                provider.config.id,
                provider.config.default_model,
                now_rfc3339()
            ],
        )
        .expect("insert embedding with vector");
}

fn sync_sidecar_vectors(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    rows: &[(i64, Vec<f32>)],
) {
    let runtime = Runtime::new().expect("runtime");
    let sidecar_rows = rows
        .iter()
        .map(|(history_id, vector)| SidecarEmbeddingRow {
            history_id: *history_id,
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com".to_string(),
            title: Some("Example".to_string()),
            domain: "example.com".to_string(),
            visited_at: "2026-04-04T00:00:00Z".to_string(),
            provider_id: provider.config.id.clone(),
            model: provider.config.default_model.clone(),
            content_hash: format!("hash-{history_id}"),
            indexed_at: now_rfc3339(),
            vector: vector.clone(),
        })
        .collect::<Vec<_>>();
    runtime
        .block_on(ai_sidecar::sync_provider_embeddings(
            paths,
            &provider.config.id,
            &provider.config.default_model,
            &sidecar_rows,
            true,
            false,
            &[],
        ))
        .expect("sync sidecar vectors");
}

fn prepared_archive() -> (ProjectPaths, AppConfig, Connection) {
    let paths = test_paths();
    let config = base_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let archive = open_archive_connection(&paths, &config, None).expect("open archive");
    create_schema(&archive).expect("create schema");
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    ensure_ai_schema(&connection).expect("ensure ai schema");
    (paths, config, connection)
}

fn seed_failed_index_ledger(
    connection: &Connection,
    provider: &AiProviderRuntime,
    failure_reason: &str,
) {
    connection
        .execute(
            "INSERT OR REPLACE INTO ai_index_ledger (
               provider_id,
               model,
               sidecar_table,
               index_version,
               state,
               source_watermark,
               last_run_id,
               build_started_at,
               build_finished_at,
               last_indexed_at,
               last_cleared_at,
               last_failure_at,
               failure_reason
             )
             VALUES (?1, ?2, 'ai_embeddings', 'test-v1', 'failed', NULL, NULL, NULL, NULL, NULL, NULL, ?3, ?4)",
            params![
                provider.config.id,
                provider.config.default_model,
                now_rfc3339(),
                failure_reason,
            ],
        )
        .expect("insert failed ledger");
}

fn seed_successful_index_ledger(
    connection: &Connection,
    provider: &AiProviderRuntime,
    source_watermark: i64,
) {
    connection
        .execute(
            "INSERT OR REPLACE INTO ai_index_ledger (
               provider_id,
               model,
               sidecar_table,
               index_version,
               state,
               source_watermark,
               last_run_id,
               build_started_at,
               build_finished_at,
               last_indexed_at,
               last_cleared_at,
               last_failure_at,
               failure_reason
             )
             VALUES (?1, ?2, 'ai_embeddings', 'test-v1', 'succeeded', ?3, 1, ?4, ?4, ?4, NULL, NULL, NULL)",
            params![
                provider.config.id,
                provider.config.default_model,
                source_watermark,
                now_rfc3339(),
            ],
        )
        .expect("insert successful ledger");
}

#[test]
fn cosine_similarity_handles_empty_vectors() {
    assert_eq!(cosine_similarity(&[], &[]), 0.0);
    assert_eq!(cosine_similarity(&[1.0], &[0.0]), 0.0);
}

#[test]
fn build_embedding_content_stays_stable() {
    let rendered = build_embedding_content(
        "chrome:Default",
        "https://example.com/docs",
        Some("Docs"),
        "2026-04-04T00:00:00Z",
    );
    assert!(rendered.contains("chrome:Default"));
    assert!(rendered.contains("example.com"));
    assert!(rendered.contains("Docs"));
}

#[test]
fn preview_ai_integrations_returns_mcp_and_skill_artifacts() {
    let paths = test_paths();
    let preview = preview_ai_integrations(&paths, &AppConfig::default()).expect("preview");
    assert_eq!(preview.generated_files.len(), 2);
    assert!(preview.mcp_command.contains("--worker mcp-server"));
    assert!(!preview.manual_steps.is_empty());
    assert_eq!(
        preview.warnings,
        vec!["MCP and skill integration are both disabled in Settings right now.".to_string()]
    );

    let mut partially_enabled = AppConfig::default();
    partially_enabled.ai.mcp_enabled = true;
    let enabled_preview =
        preview_ai_integrations(&paths, &partially_enabled).expect("enabled preview");
    assert!(enabled_preview.warnings.is_empty());
}

#[test]
fn validate_provider_rejects_anthropic_embeddings() {
    let error = validate_provider(
        &AiProviderRuntime {
            config: AiProviderConfig {
                id: "embed".to_string(),
                name: "Anthropic embeddings".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::Anthropic,
                enabled: true,
                default_model: "claude-3-7-sonnet".to_string(),
                ..AiProviderConfig::default()
            },
            api_key: "secret".to_string(),
        },
        AiProviderPurpose::Embedding,
    )
    .expect_err("anthropic embeddings should fail");
    assert!(error.to_string().contains("Anthropic"));
}

#[test]
fn validate_provider_rejects_disabled_wrong_purpose_and_missing_model() {
    let disabled = validate_provider(
        &AiProviderRuntime {
            config: AiProviderConfig {
                id: "embed".to_string(),
                name: "Disabled".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::OpenAi,
                enabled: false,
                default_model: "text-embedding-3-small".to_string(),
                ..AiProviderConfig::default()
            },
            api_key: "secret".to_string(),
        },
        AiProviderPurpose::Embedding,
    )
    .expect_err("disabled provider should fail");
    assert!(disabled.to_string().contains("Enable provider"));

    let wrong_purpose = validate_provider(&embedding_provider(), AiProviderPurpose::Llm)
        .expect_err("purpose mismatch should fail");
    assert!(wrong_purpose.to_string().contains("configured for"));

    let missing_model = validate_provider(
        &AiProviderRuntime {
            config: AiProviderConfig {
                id: "llm".to_string(),
                name: "Missing model".to_string(),
                purpose: AiProviderPurpose::Llm,
                request_format: AiRequestFormat::OpenAi,
                enabled: true,
                default_model: String::new(),
                ..AiProviderConfig::default()
            },
            api_key: "secret".to_string(),
        },
        AiProviderPurpose::Llm,
    )
    .expect_err("missing model should fail");
    assert!(missing_model.to_string().contains("default model"));
}

#[test]
fn ai_index_status_warns_when_archive_is_missing() {
    let paths = test_paths();
    let mut config = base_config();
    config.ai.mcp_enabled = true;
    config.ai.skill_enabled = true;
    config.ai.job_queue_paused = true;
    config.ai.job_queue_concurrency = 7;

    let status = ai_index_status(&paths, &config, None).expect("status");
    assert!(status.enabled);
    assert!(status.assistant_enabled);
    assert!(status.mcp_enabled);
    assert!(status.skill_enabled);
    assert_eq!(status.state, "blocked");
    assert_eq!(status.llm_provider_id.as_deref(), Some("llm"));
    assert_eq!(status.embedding_provider_id.as_deref(), Some("embed"));
    assert!(status.queue_paused);
    assert_eq!(status.queue_concurrency, 7);
    assert_eq!(status.queued_jobs, 0);
    assert_eq!(status.running_jobs, 0);
    assert_eq!(status.failed_jobs, 0);
    assert!(status.recent_jobs.is_empty());
    assert_eq!(
        status.warning.as_deref(),
        Some("Initialize the archive before using AI analysis features.")
    );
    assert!(!status.ready);
    assert_eq!(status.indexed_items, 0);
    assert!(status.last_indexed_at.is_none());
}

#[test]
fn ai_index_status_reports_ready_with_existing_embeddings() {
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ready");

    let status = ai_index_status(&paths, &config, None).expect("status");
    assert!(status.ready);
    assert_eq!(status.state, "ready");
    assert_eq!(status.indexed_items, 1);
    assert!(status.last_indexed_at.is_some());
}

#[test]
fn ai_index_status_requires_initialized_archive_even_if_embeddings_exist() {
    let (paths, mut config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ready");
    config.initialized = false;

    let status = ai_index_status(&paths, &config, None).expect("status");
    assert!(!status.ready);
    assert_eq!(status.indexed_items, 0);
    assert!(status.last_indexed_at.is_none());
    assert_eq!(
        status.warning.as_deref(),
        Some("Initialize the archive before using AI analysis features.")
    );
}

#[test]
fn ai_index_status_requires_indexed_rows_and_respects_warning_gate() {
    let (paths, config, _connection) = prepared_archive();

    let status = ai_index_status(&paths, &config, None).expect("status");
    assert!(!status.ready);
    assert_eq!(status.state, "empty");
    assert_eq!(status.indexed_items, 0);
    assert!(status.last_indexed_at.is_none());
    assert_eq!(
        status.warning.as_deref(),
        Some("Run Build index after configuring an embedding provider to enable semantic search.")
    );

    let mut disabled = config.clone();
    disabled.ai.enabled = false;
    let disabled_status = ai_index_status(&paths, &disabled, None).expect("disabled status");
    assert!(!disabled_status.ready);
    assert_eq!(disabled_status.state, "disabled");
    assert_eq!(disabled_status.warning, None);
}

#[test]
fn ai_index_status_treats_selected_model_without_embeddings_as_empty() {
    let (paths, mut config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ready");
    config.ai.embedding_providers[0].default_model = "text-embedding-3-large".to_string();

    let status = ai_index_status(&paths, &config, None).expect("status");
    assert!(!status.ready);
    assert_eq!(status.state, "empty");
    assert_eq!(status.indexed_items, 0);
    assert!(status.last_indexed_at.is_none());
}

#[test]
fn ai_index_status_covers_degraded_queued_paused_rebuilding_and_failed_states() {
    let provider = embedding_provider();

    let (paths, mut degraded_config, _connection) = prepared_archive();
    degraded_config.ai.embedding_provider_id = None;
    let degraded = ai_index_status(&paths, &degraded_config, None).expect("degraded status");
    assert_eq!(degraded.state, "degraded");
    assert!(!degraded.ready);
    assert_eq!(degraded.indexed_items, 0);
    assert!(degraded.warning.is_some());

    let (paths, config, connection) = prepared_archive();
    ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    let queued = ai_index_status(&paths, &config, None).expect("queued status");
    assert_eq!(queued.state, "queued");
    assert_eq!(queued.queued_jobs, 1);
    assert_eq!(queued.running_jobs, 0);
    assert_eq!(queued.failed_jobs, 0);
    assert_eq!(queued.recent_jobs.len(), 1);

    let (paths, mut paused_config, connection) = prepared_archive();
    paused_config.ai.job_queue_paused = true;
    ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), true)
        .expect("enqueue paused job");
    let paused = ai_index_status(&paths, &paused_config, None).expect("paused status");
    assert_eq!(paused.state, "paused");
    assert!(paused.queue_paused);
    assert_eq!(paused.queued_jobs, 1);

    let (paths, mut paused_empty_config, _connection) = prepared_archive();
    paused_empty_config.ai.job_queue_paused = true;
    let paused_empty =
        ai_index_status(&paths, &paused_empty_config, None).expect("paused empty status");
    assert_eq!(paused_empty.state, "empty");

    let (paths, config, connection) = prepared_archive();
    let queued_job = ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
        .expect("enqueue job for rebuild");
    let running_job = ai_queue::claim_ai_job_by_id(&connection, queued_job.id, 300)
        .expect("claim job")
        .expect("running job");
    let rebuilding = ai_index_status(&paths, &config, None).expect("rebuilding status");
    assert_eq!(rebuilding.state, "rebuilding");
    assert_eq!(rebuilding.running_jobs, 1);
    assert!(rebuilding.recent_jobs.iter().any(|job| job.id == running_job.id));

    let (paths, config, connection) = prepared_archive();
    seed_failed_index_ledger(&connection, &provider, "embedding run failed");
    let failed = ai_index_status(&paths, &config, None).expect("failed status");
    assert_eq!(failed.state, "failed");
    assert_eq!(failed.warning.as_deref(), Some("embedding run failed"));
}

#[test]
fn ai_queue_status_reflects_config_and_recent_jobs() {
    let paths = test_paths();
    let mut config = base_config();
    config.ai.job_queue_paused = true;
    config.ai.job_queue_concurrency = 3;
    let missing_status = ai_queue_status(&paths, &config, None).expect("missing queue status");
    assert!(missing_status.paused);
    assert_eq!(missing_status.concurrency, 3);
    assert_eq!(missing_status.queued, 0);
    assert!(missing_status.recent_jobs.is_empty());

    let (paths, mut config, connection) = prepared_archive();
    config.ai.job_queue_paused = false;
    config.ai.job_queue_concurrency = 2;
    ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    ai_queue::enqueue_assistant_job(
        &connection,
        &AiAssistantRequest {
            question: "What changed?".to_string(),
            profile_id: None,
            domain: None,
        },
        "llm",
        Some("embed"),
        true,
    )
    .expect("enqueue paused assistant job");
    let status = ai_queue_status(&paths, &config, None).expect("queue status");
    assert!(!status.paused);
    assert_eq!(status.concurrency, 2);
    assert_eq!(status.queued, 2);
    assert_eq!(status.running, 0);
    assert_eq!(status.failed, 0);
    assert_eq!(status.recent_jobs.len(), 2);
    assert!(status.recent_jobs.iter().any(|job| job.state == "queued"));
    assert!(status.recent_jobs.iter().any(|job| job.state == "paused"));

    let (paths, mut uninitialized, connection) = prepared_archive();
    uninitialized.initialized = false;
    uninitialized.ai.job_queue_paused = true;
    ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
        .expect("enqueue job for uninitialized queue");
    let still_default =
        ai_queue_status(&paths, &uninitialized, None).expect("uninitialized queue status");
    assert!(still_default.paused);
    assert_eq!(still_default.queued, 0);
    assert!(still_default.recent_jobs.is_empty());
}

#[test]
fn reconcile_ai_queue_controls_pauses_resumes_and_noops() {
    let (paths, config, connection) = prepared_archive();
    let queued_job = ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");

    let mut paused = config.clone();
    paused.ai.job_queue_paused = true;
    reconcile_ai_queue_controls(&paths, &paused, &paused, None).expect("no-op reconcile");
    let unchanged = ai_queue::load_ai_queue_status(&connection, false, 1, 5).expect("load jobs");
    assert!(
        unchanged.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "queued")
    );

    reconcile_ai_queue_controls(&paths, &config, &paused, None).expect("pause reconcile");
    let paused_status =
        ai_queue::load_ai_queue_status(&connection, true, 1, 5).expect("load paused jobs");
    assert!(
        paused_status
            .recent_jobs
            .iter()
            .any(|job| job.id == queued_job.id && job.state == "paused")
    );

    reconcile_ai_queue_controls(&paths, &paused, &config, None).expect("resume reconcile");
    let resumed =
        ai_queue::load_ai_queue_status(&connection, false, 1, 5).expect("load resumed jobs");
    assert!(resumed.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "queued"));

    let (paths, config, connection) = prepared_archive();
    let queued_job = ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    let mut not_initialized = config.clone();
    not_initialized.initialized = false;
    let mut paused_next = config.clone();
    paused_next.initialized = false;
    paused_next.ai.job_queue_paused = true;
    reconcile_ai_queue_controls(&paths, &not_initialized, &paused_next, None)
        .expect("skip reconcile for uninitialized archive");
    let skipped =
        ai_queue::load_ai_queue_status(&connection, false, 1, 5).expect("load skipped jobs");
    assert!(skipped.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "queued"));
}

#[test]
fn provider_helpers_report_capabilities_and_failure_metadata() {
    let embedding_config = embedding_provider().config;
    let capabilities = provider_capabilities(&embedding_config);
    assert!(capabilities.supports_embeddings);
    assert!(!capabilities.supports_chat);
    assert!(!capabilities.supports_streaming);
    assert!(!capabilities.supports_tool_use);
    assert!(!capabilities.supports_structured_output);

    let failure = provider_connection_failure_report(&embedding_config, "unauthorized");
    assert_eq!(failure.provider_id, "embed");
    assert_eq!(failure.purpose, "embedding");
    assert_eq!(failure.model, "text-embedding-3-small");
    assert!(!failure.ok);
    assert_eq!(failure.latency_ms, 0);
    assert!(failure.capabilities.supports_embeddings);
    assert_eq!(failure.error_code.as_deref(), Some("secret-missing"));
    assert!(failure.action_hint.is_some());
    assert!(failure.retry_hint.is_some());
    assert!(failure.warnings.is_empty());
    assert_eq!(failure.message, "unauthorized");

    for request_format in
        [AiRequestFormat::Google, AiRequestFormat::Ollama, AiRequestFormat::LmStudio]
    {
        let mut provider = embedding_config.clone();
        provider.request_format = request_format;
        assert!(provider_capabilities(&provider).supports_embeddings);
    }

    for request_format in [
        AiRequestFormat::OpenAi,
        AiRequestFormat::Anthropic,
        AiRequestFormat::Google,
        AiRequestFormat::Ollama,
        AiRequestFormat::LmStudio,
    ] {
        let mut provider = llm_provider().config;
        provider.request_format = request_format;
        let capabilities = provider_capabilities(&provider);
        assert!(capabilities.supports_chat);
        assert!(capabilities.supports_streaming);
        assert!(capabilities.supports_tool_use);
        assert!(capabilities.supports_structured_output);
    }
}

#[test]
fn provider_validation_readiness_and_error_branches_stay_actionable() {
    let mut disabled = embedding_provider();
    disabled.config.enabled = false;
    assert!(
        validate_provider(&disabled, AiProviderPurpose::Embedding)
            .expect_err("disabled provider")
            .to_string()
            .contains("Enable provider")
    );

    let mut wrong_purpose = embedding_provider();
    wrong_purpose.config.purpose = AiProviderPurpose::Llm;
    assert!(
        validate_provider(&wrong_purpose, AiProviderPurpose::Embedding)
            .expect_err("purpose mismatch")
            .to_string()
            .contains("configured for")
    );

    let mut missing_model = embedding_provider();
    missing_model.config.default_model = "  ".to_string();
    assert!(
        validate_provider(&missing_model, AiProviderPurpose::Embedding)
            .expect_err("missing model")
            .to_string()
            .contains("Select a default model")
    );

    let mut anthropic_embedding = embedding_provider();
    anthropic_embedding.config.request_format = AiRequestFormat::Anthropic;
    assert!(
        validate_provider(&anthropic_embedding, AiProviderPurpose::Embedding)
            .expect_err("unsupported embedding format")
            .to_string()
            .contains("Anthropic request format")
    );

    let cases = [
        ("enable provider first", "provider-disabled", false),
        ("rate limit 429 quota", "rate-limited", true),
        ("provider is not configured for embeddings", "unsupported-capability", false),
        ("model not found", "bad-model", true),
        ("dns refused network timed out", "network-error", true),
        ("unexpected provider failure", "provider-error", true),
    ];
    for (message, code, retry_expected) in cases {
        let (error_code, action_hint, retry_hint) = classify_provider_error(message);
        assert_eq!(error_code.as_deref(), Some(code));
        if code == "provider-error" {
            assert!(action_hint.is_none());
        } else {
            assert!(action_hint.is_some());
        }
        assert_eq!(retry_hint.is_some(), retry_expected);
    }

    let mut config = base_config();
    config.ai.embedding_provider_id = None;
    let readiness = embedding_provider_readiness(&config);
    assert!(!readiness.available);
    assert!(readiness.warning.as_deref().expect("warning").contains("Select"));

    config.ai.embedding_provider_id = Some("missing".to_string());
    let readiness = embedding_provider_readiness(&config);
    assert!(!readiness.available);
    assert!(readiness.warning.as_deref().expect("warning").contains("no longer available"));

    config.ai.embedding_provider_id = Some("embed".to_string());
    config.ai.embedding_providers[0].enabled = false;
    let readiness = embedding_provider_readiness(&config);
    assert!(!readiness.available);
    assert_eq!(readiness.selected_model.as_deref(), Some("text-embedding-3-small"));

    config.ai.embedding_providers[0].enabled = true;
    config.ai.embedding_providers[0].api_key_saved = false;
    let readiness = embedding_provider_readiness(&config);
    assert!(!readiness.available);
    assert!(readiness.warning.as_deref().expect("warning").contains("API key"));

    config.ai.embedding_providers[0].api_key_saved = true;
    config.ai.embedding_providers[0].default_model = " ".to_string();
    let readiness = embedding_provider_readiness(&config);
    assert!(!readiness.available);
    assert!(readiness.selected_model.is_none());

    config.ai.embedding_providers[0].default_model = "text-embedding-3-small".to_string();
    let readiness = embedding_provider_readiness(&config);
    assert!(readiness.available);
    assert!(readiness.warning.is_none());
}

#[test]
fn persisted_assistant_run_loader_reports_malformed_json_columns() {
    let (paths, config, connection) = prepared_archive();
    connection
        .execute(
            "INSERT INTO ai_assistant_runs
             (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at)
             VALUES (1, 'question', 'answer', 'llm', 'embed', '{not-json', '[]', 'now')",
            [],
        )
        .expect("insert malformed citations");
    let citation_error =
        load_assistant_run_response(&paths, &config, None, 1).expect_err("citations json error");
    assert!(format!("{citation_error:#}").contains("invalid"));

    connection
        .execute(
            "INSERT INTO ai_assistant_runs
             (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at)
             VALUES (2, 'question', 'answer', 'llm', 'embed', '[]', '{not-json', 'now')",
            [],
        )
        .expect("insert malformed notes");
    let notes_error =
        load_assistant_run_response(&paths, &config, None, 2).expect_err("notes json error");
    assert!(format!("{notes_error:#}").contains("invalid"));
}

#[test]
fn persisted_assistant_run_loader_marks_cited_answers_completed() {
    let (paths, config, connection) = prepared_archive();
    let citations_json = serde_json::to_string(&vec![AiCitation {
        history_id: 42,
        profile_id: "chrome:Default".to_string(),
        url: "https://example.com/report".to_string(),
        title: Some("Example report".to_string()),
        visited_at: "2026-04-27T00:00:00Z".to_string(),
        score: Some(0.91),
    }])
    .expect("serialize citation");
    connection
        .execute(
            "INSERT INTO ai_assistant_runs
             (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at)
             VALUES (3, 'question', 'answer', 'llm', 'embed', ?1, ?2, 'now')",
            params![
                citations_json,
                serde_json::to_string(&vec!["grounded by one visit"]).expect("serialize notes")
            ],
        )
        .expect("insert cited assistant run");

    let response = load_assistant_run_response(&paths, &config, None, 3).expect("load run");

    assert_eq!(response.state, "completed");
    assert_eq!(response.run_id, Some(3));
    assert_eq!(response.citations.len(), 1);
    assert_eq!(response.citations[0].history_id, 42);
    assert_eq!(response.notes, vec!["grounded by one visit"]);
}

#[test]
fn provider_reports_cover_llm_capabilities_and_failure_purpose() {
    let provider = llm_provider_with_format(AiRequestFormat::Anthropic).config;

    let capabilities = provider_capabilities(&provider);
    assert!(capabilities.supports_chat);
    assert!(capabilities.supports_streaming);
    assert!(capabilities.supports_tool_use);
    assert!(capabilities.supports_structured_output);
    assert!(!capabilities.supports_embeddings);

    let failure = provider_connection_failure_report(&provider, "provider timeout");
    assert_eq!(failure.purpose, "llm");
    assert_eq!(failure.provider_id, "llm");
    assert!(!failure.ok);
    assert!(failure.retry_hint.is_some());
    assert!(failure.capabilities.supports_tool_use);
}

#[test]
fn test_provider_connection_reports_success_fields_and_anthropic_warning() {
    let runtime = Runtime::new().expect("runtime");
    let embedding_report = runtime
        .block_on(test_provider_connection(&embedding_provider()))
        .expect("embedding connection report");
    assert_eq!(embedding_report.provider_id, "embed");
    assert_eq!(embedding_report.purpose, "embedding");
    assert!(embedding_report.ok);
    assert!(embedding_report.message.contains("3-dimension"));

    let report = runtime
        .block_on(test_provider_connection(&llm_provider_with_format(AiRequestFormat::Anthropic)))
        .expect("connection report");
    assert_eq!(report.provider_id, "llm");
    assert_eq!(report.purpose, "llm");
    assert_eq!(report.model, "gpt-4.1-mini");
    assert!(report.ok);
    assert!(report.latency_ms >= 1);
    assert!(report.capabilities.supports_chat);
    assert!(!report.capabilities.supports_embeddings);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("chat-only"));
    assert!(report.message.contains("successfully"));

    let openai = runtime
        .block_on(test_provider_connection(&llm_provider_with_format(AiRequestFormat::OpenAi)))
        .expect("openai connection report");
    assert!(openai.ok);
    assert!(openai.warnings.is_empty());

    let capabilities = provider_capabilities(&embedding_provider().config);
    let failure = provider_connection_report_from_probe(
        &embedding_provider(),
        capabilities,
        7,
        Err(anyhow::anyhow!("unauthorized")),
    )
    .expect("failure report");
    assert!(!failure.ok);
    assert_eq!(failure.latency_ms, 7);
    assert_eq!(failure.error_code.as_deref(), Some("secret-missing"));

    let llm_failure = provider_connection_report_from_probe(
        &llm_provider_with_format(AiRequestFormat::OpenAi),
        provider_capabilities(&llm_provider_with_format(AiRequestFormat::OpenAi).config),
        8,
        Err(anyhow::anyhow!("provider unavailable")),
    )
    .expect("llm failure report");
    assert_eq!(llm_failure.purpose, "llm");
    assert_eq!(llm_failure.error_code.as_deref(), Some("provider-error"));
}

#[test]
fn ensure_ai_schema_adds_tables() {
    let empty = Connection::open_in_memory().expect("empty connection");
    assert_eq!(ai_embeddings_storage_bytes(&empty).expect("empty storage bytes"), 0);
    assert_eq!(ai_embedding_token_estimate(&empty).expect("empty token estimate"), 0);

    let paths = test_paths();
    let config = base_config();
    ensure_archive_initialized(&paths, &config, None).expect("init archive");
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    ensure_ai_schema(&connection).expect("schema");
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ai_embeddings'",
            [],
            |row: &Row<'_>| row.get(0),
        )
        .expect("count");
    assert_eq!(count, 1);
}

#[test]
fn ensure_ai_schema_creates_metadata_only_embedding_rows() {
    let paths = test_paths();
    std::fs::create_dir_all(
        paths.intelligence_database_path.parent().expect("intelligence database parent"),
    )
    .expect("create archive dir");
    let connection =
        Connection::open(&paths.intelligence_database_path).expect("open intelligence");
    ensure_ai_schema(&connection).expect("ensure schema");

    let columns = connection
        .prepare("PRAGMA table_info(ai_embeddings)")
        .expect("prepare pragma")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query pragma")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect columns");
    assert!(columns.iter().any(|column| column == "content_bytes"));
    assert!(!columns.iter().any(|column| column == "content"));
    assert!(!columns.iter().any(|column| column == "embedding_blob"));
    assert!(!columns.iter().any(|column| column == "embedding_json"));
}

#[test]
fn build_assistant_preamble_covers_empty_and_seeded_context() {
    let config = base_config();
    let empty = build_assistant_preamble(&config, &AiSearchResponse::default());
    assert!(empty.contains("No indexed evidence was found"));

    let with_context = build_assistant_preamble(
        &config,
        &AiSearchResponse {
            total: 1,
            provider_id: "embed".to_string(),
            model: "text-embedding-3-small".to_string(),
            items: vec![AiSearchEntry {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/docs".to_string(),
                title: Some("Docs".to_string()),
                domain: "example.com".to_string(),
                visited_at: "2026-04-04T00:00:00Z".to_string(),
                score: 0.91,
                match_reason: "Semantic match".to_string(),
            }],
            notes: Vec::new(),
            next_cursor: None,
        },
    );
    assert!(with_context.contains("Semantic match"));
    assert!(with_context.contains("https://example.com/docs"));
}

#[test]
fn collect_visits_to_index_skips_already_indexed_rows_and_cleanup_removes_stale_rows() {
    let (paths, _config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);

    let visit_time = connection
        .query_row(
            "SELECT (visit_time_ms * 1000 + 11644473600000000)
             FROM archive.visits
             WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("load visit time");
    let first_content = build_embedding_content(
        "chrome:Default",
        "https://example.com/docs",
        Some("Docs"),
        &crate::utils::chrome_time_to_rfc3339(visit_time),
    );
    seed_embedding(&connection, 1, &provider, &sha256_hex(first_content.as_bytes()));
    seed_embedding(&connection, 999, &provider, "orphan-hash");

    let candidates =
        collect_visits_to_index(&paths, &connection, &provider, Some(10)).expect("collect");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].history_id, 2);

    let removed = cleanup_stale_embeddings(&connection, &provider).expect("cleanup");
    assert_eq!(removed, 1);
}

#[test]
fn cleanup_stale_embeddings_returns_zero_when_nothing_is_removed() {
    let (_paths, _config, connection) = prepared_archive();
    let removed = cleanup_stale_embeddings(&connection, &embedding_provider()).expect("cleanup");
    assert_eq!(removed, 0);
}

#[test]
fn index_clear_only_and_sqlite_mirror_helpers_cover_stale_rows() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let candidates =
        collect_visits_to_index(&paths, &connection, &provider, Some(1)).expect("collect visit");
    assert_eq!(candidates.len(), 1);
    upsert_embedding(&connection, &provider, &candidates[0], "2026-04-14T00:00:00Z")
        .expect("upsert embedding mirror");
    assert_eq!(
        provider_embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("provider count"),
        1
    );
    assert!(collect_stale_history_ids(&connection, &provider).expect("fresh stale ids").is_empty());

    connection
        .execute("UPDATE archive.visits SET reverted_at = '2026-04-14T00:00:00Z' WHERE id = 1", [])
        .expect("revert indexed visit");
    assert_eq!(collect_stale_history_ids(&connection, &provider).expect("stale ids"), vec![1]);
    clear_provider_embeddings(&connection, &provider).expect("clear provider mirror");
    assert_eq!(
        provider_embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("provider count after clear"),
        0
    );

    seed_embedding(&connection, 1, &provider, "old-hash");
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &provider,
            &AiIndexRequest {
                provider_id: None,
                full_rebuild: false,
                clear_only: true,
                limit: None,
            },
        ))
        .expect("clear semantic index");
    assert_eq!(report.indexed_items, 0);
    assert!(report.notes[0].contains("Cleared the semantic index"));
    let cleared = open_intelligence_connection(&paths, &config, None).expect("reload intelligence");
    assert_eq!(
        provider_embedding_count(&cleared, &provider.config.id, &provider.config.default_model)
            .expect("cleared provider count"),
        0
    );
}

#[test]
fn search_history_internal_requires_query_and_supports_lexical_fallback() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let empty_error = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "   ".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect_err("empty query should fail");
    assert!(empty_error.to_string().contains("Enter a question"));

    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("lexical search");
    assert_eq!(response.total, 1);
    assert_eq!(response.provider_id, "lexical-fallback");
    assert_eq!(response.items[0].score, 0.6);
    assert!(response.notes.iter().any(|note| note.contains("lexical retrieval")));
}

#[test]
fn semantic_search_history_uses_public_wrapper_for_search_results() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let response = runtime
        .block_on(semantic_search_history(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("public search wrapper");
    assert_eq!(response.total, 1);
    assert_eq!(response.provider_id, "lexical-fallback");
    assert!(response.items.iter().any(|item| item.url.contains("/docs")));
}

#[test]
fn build_ai_index_returns_without_network_when_no_candidates_exist() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    drop(connection);
    let report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &embedding_provider(),
            &AiIndexRequest {
                provider_id: None,
                full_rebuild: true,
                clear_only: false,
                limit: Some(5),
            },
        ))
        .expect("empty build");
    assert_eq!(report.indexed_items, 0);
    assert!(report.notes.iter().any(|note| note.contains("No new or changed history rows")));
}

#[test]
fn build_ai_index_with_control_stops_at_batch_boundaries() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    let control: Arc<dyn AiRunControl> = Arc::new(CountdownControl::new(1));

    let error = runtime
        .block_on(build_ai_index_with_control(
            &paths,
            &config,
            None,
            &embedding_provider(),
            &AiIndexRequest::default(),
            Some(control),
        ))
        .expect_err("controlled build should cancel");
    assert!(error.to_string().contains("cancelled"));
}

#[cfg(coverage)]
#[test]
fn build_ai_index_falls_back_to_per_row_embeddings_and_records_skips() {
    let runtime = Runtime::new().expect("runtime");

    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    drop(connection);
    let fallback_report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &embedding_provider_with_id("embed-batch-short"),
            &AiIndexRequest::default(),
        ))
        .expect("fallback build");
    assert_eq!(fallback_report.indexed_items, 1);
    assert_eq!(fallback_report.skipped_items, 0);
    assert!(fallback_report.notes.iter().any(|note| note.contains("Synced 1 row")));
    let connection =
        open_intelligence_connection(&paths, &config, None).expect("reopen intelligence");
    connection
        .execute("UPDATE archive.urls SET title = 'Docs updated' WHERE id = 1", [])
        .expect("change indexed content");
    drop(connection);
    let updated_report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &embedding_provider_with_id("embed-batch-short"),
            &AiIndexRequest::default(),
        ))
        .expect("updated build");
    assert_eq!(updated_report.updated_items, 1);

    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    drop(connection);
    let skipped_report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &embedding_provider_with_id("embed-batch-error-single-error"),
            &AiIndexRequest::default(),
        ))
        .expect("skipped build");
    assert_eq!(skipped_report.indexed_items, 0);
    assert_eq!(skipped_report.skipped_items, 1);
    assert!(
        skipped_report
            .notes
            .iter()
            .any(|note| note.contains("Skipped 1 row(s) after retrying failed embedding batches"))
    );
}

#[test]
fn answer_history_question_checks_feature_gates_before_network() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, mut config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    config.ai.assistant_enabled = false;
    let error = runtime
        .block_on(answer_history_question(
            &paths,
            &config,
            None,
            &llm_provider(),
            None,
            &AiAssistantRequest {
                question: "What did I read?".to_string(),
                profile_id: None,
                domain: None,
            },
        ))
        .expect_err("assistant should require feature gate");
    assert!(error.to_string().contains("assistant"));
}

#[test]
fn answer_history_question_with_control_can_cancel_before_model_response() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let control: Arc<dyn AiRunControl> = Arc::new(CountdownControl::new(1));

    let error = runtime
        .block_on(answer_history_question_with_control(
            &paths,
            &config,
            None,
            &llm_provider(),
            None,
            &AiAssistantRequest {
                question: "What did I read?".to_string(),
                profile_id: None,
                domain: None,
            },
            Some(control),
        ))
        .expect_err("assistant should observe cancellation");
    assert!(error.to_string().contains("cancelled"));
}

#[test]
fn search_history_tool_definition_and_call_collect_citations() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/history", Some("History"), 1);
    let citations = Arc::new(Mutex::new(Vec::new()));
    let tool = SearchHistoryTool {
        context: SearchContext {
            paths,
            config,
            database_key: None,
            embedding_provider: None,
            default_profile_id: None,
            default_domain: None,
            default_limit: 3,
            citations: Arc::clone(&citations),
            run_control: None,
        },
    };

    let definition = runtime.block_on(rig::tool::Tool::definition(&tool, String::new()));
    assert_eq!(definition.name, "search_history");

    let output = runtime
        .block_on(rig::tool::Tool::call(
            &tool,
            SearchHistoryArgs {
                query: "history".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(3),
            },
        ))
        .expect("tool call");
    assert_eq!(output.items.len(), 1);
    let stored = runtime.block_on(async { citations.lock().await.clone() });
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].history_id, 1);

    for (checkpoints_before_cancel, expected) in [
        (0, "Assistant run was cancelled before an additional history search."),
        (1, "Assistant run was cancelled after the latest history search."),
    ] {
        let cancellation_tool = SearchHistoryTool {
            context: SearchContext {
                paths: tool.context.paths.clone(),
                config: tool.context.config.clone(),
                database_key: None,
                embedding_provider: None,
                default_profile_id: None,
                default_domain: None,
                default_limit: 3,
                citations: Arc::new(Mutex::new(Vec::new())),
                run_control: Some(Arc::new(CountdownControl::new(checkpoints_before_cancel))),
            },
        };
        let error = runtime
            .block_on(rig::tool::Tool::call(
                &cancellation_tool,
                SearchHistoryArgs {
                    query: "history".to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(3),
                },
            ))
            .expect_err("tool cancellation should surface");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn ai_status_and_search_cover_non_ready_and_semantic_empty_branches() {
    let runtime = Runtime::new().expect("runtime");
    let mut disabled = base_config();
    disabled.ai.enabled = false;
    let missing_paths = test_paths();
    let disabled_status =
        ai_index_status(&missing_paths, &disabled, None).expect("disabled status");
    assert!(disabled_status.warning.is_none());

    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let mut no_provider = config.clone();
    no_provider.ai.embedding_provider_id = None;
    let no_provider_status = ai_index_status(&paths, &no_provider, None).expect("no provider");
    assert_eq!(no_provider_status.indexed_items, 0);
    assert!(no_provider_status.warning.is_some());

    let collected = collect_visits_to_index(&paths, &connection, &embedding_provider(), None)
        .expect("collect all");
    assert_eq!(collected.len(), 1);
    seed_embedding(&connection, 1, &embedding_provider(), "sqlite-only-hash");

    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding_provider()),
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("semantic empty fallback");
    assert_eq!(response.provider_id, "embed");
    assert!(
        response.notes.iter().any(|note| note.contains("No indexed semantic matches were found"))
    );
    assert!(response.notes.iter().any(|note| note.contains("sidecar is missing or empty")));
}

#[test]
fn build_index_search_and_assistant_cover_semantic_and_persistence_flows() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    seed_embedding(&connection, 1, &embedding, "stale-hash");
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(&paths, &config, None, &embedding, &AiIndexRequest::default()))
        .expect("build index");
    assert_eq!(report.indexed_items, 1);
    assert_eq!(report.updated_items, 1);
    assert!(report.notes[0].contains("Indexed 2 history rows"));

    let rebuilt = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &embedding,
            &AiIndexRequest {
                provider_id: None,
                full_rebuild: true,
                clear_only: false,
                limit: Some(1),
            },
        ))
        .expect("full rebuild");
    assert_eq!(rebuilt.indexed_items, 1);

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("semantic search");
    assert_eq!(search.provider_id, "embed");
    assert!(search.items.iter().any(|item| item.match_reason.contains("Semantic")));

    let assistant = runtime
        .block_on(answer_history_question(
            &paths,
            &config,
            None,
            &llm_provider(),
            Some(&embedding),
            &AiAssistantRequest {
                question: "Summarize my docs reading".to_string(),
                profile_id: None,
                domain: None,
            },
        ))
        .expect("assistant answer");
    assert!(assistant.answer.contains("Summarize my docs reading"));
    assert_eq!(assistant.provider_id, "llm");
    assert_eq!(assistant.embedding_provider_id, "embed");
    assert!(!assistant.citations.is_empty());

    let connection =
        open_intelligence_connection(&paths, &config, None).expect("open intelligence");
    let runs: i64 = connection
        .query_row("SELECT COUNT(*) FROM ai_assistant_runs", [], |row: &Row<'_>| row.get(0))
        .expect("assistant run count");
    assert_eq!(runs, 1);
}

#[test]
fn semantic_matches_returns_sidecar_results_when_available() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/hidden", Some("Hidden"), 2);
    let query_vector = runtime.block_on(embed_query(&embedding, "docs")).expect("query vector");
    seed_embedding_with_vector(&connection, 1, &embedding, &query_vector);
    seed_embedding_with_vector(&connection, 2, &embedding, &query_vector);

    let missing_sidecar = runtime
        .block_on(semantic_matches(
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("semantic matches without sidecar");
    assert!(missing_sidecar.items.is_empty());
    assert!(
        missing_sidecar
            .notes
            .iter()
            .any(|note| note.contains("semantic sidecar is missing or empty"))
    );

    connection
        .execute("UPDATE archive.visits SET reverted_at = ?1 WHERE id = 2", [now_rfc3339()])
        .expect("hide semantic row");
    sync_sidecar_vectors(
        &paths,
        &embedding,
        &[(1, query_vector.clone()), (2, query_vector.clone())],
    );

    let matches = runtime
        .block_on(semantic_matches(
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("semantic matches");
    assert_eq!(matches.items.len(), 1);
    assert_eq!(matches.items[0].history_id, 1);

    let stale_watermark = semantic_index_staleness_reason(
        &connection,
        &embedding.config.id,
        &embedding.config.default_model,
        1,
        Some("2026-01-01T00:00:00Z"),
    )
    .expect("stale watermark");
    assert!(
        stale_watermark
            .as_deref()
            .is_some_and(|reason| reason.contains("visibility or import watermark"))
    );

    connection
        .execute(
            "CREATE TABLE IF NOT EXISTS visit_content_enrichments (
                history_id INTEGER PRIMARY KEY,
                fetch_status TEXT NOT NULL,
                fetched_at TEXT NOT NULL
            )",
            [],
        )
        .expect("enrichment table");
    connection
        .execute(
            "INSERT OR REPLACE INTO visit_content_enrichments
             (history_id, content_source, fetch_status, fetched_at, snippet_json, extraction_json, pipeline_version)
             VALUES (1, 'readable-content', 'success', '2026-04-05T00:00:00Z', '[]', '{}', 'test')",
            [],
        )
        .expect("fresh enrichment");
    let stale_enrichment = semantic_index_staleness_reason(
        &connection,
        &embedding.config.id,
        &embedding.config.default_model,
        current_source_watermark(&connection).expect("current watermark"),
        Some("2026-04-01T00:00:00Z"),
    )
    .expect("stale enrichment");
    assert!(
        stale_enrichment
            .as_deref()
            .is_some_and(|reason| reason.contains("Readable-content enrichment changed"))
    );
}

#[test]
fn semantic_matches_reports_stale_ledger_and_sidecar_errors() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let query_vector = runtime.block_on(embed_query(&embedding, "docs")).expect("query vector");
    seed_embedding_with_vector(&connection, 1, &embedding, &query_vector);
    seed_successful_index_ledger(&connection, &embedding, 1);

    let stale_without_sidecar = runtime
        .block_on(semantic_matches(
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("semantic matches with stale ledger");
    assert!(
        stale_without_sidecar
            .notes
            .iter()
            .any(|note| note.contains("visibility or import watermark"))
    );
    assert!(
        stale_without_sidecar
            .notes
            .iter()
            .any(|note| note.contains("semantic sidecar is missing or empty"))
    );

    runtime
        .block_on(ai_sidecar::sync_provider_embeddings(
            &paths,
            &embedding.config.id,
            &embedding.config.default_model,
            &[SidecarEmbeddingRow {
                history_id: 1,
                profile_id: "chrome:Default".to_string(),
                url: "https://example.com/docs".to_string(),
                title: Some("Docs".to_string()),
                domain: "example.com".to_string(),
                visited_at: "2026-04-04T00:00:00Z".to_string(),
                provider_id: embedding.config.id.clone(),
                model: embedding.config.default_model.clone(),
                content_hash: "hash-1".to_string(),
                indexed_at: now_rfc3339(),
                vector: vec![0.1, 0.2],
            }],
            true,
            false,
            &[],
        ))
        .expect("sync incompatible sidecar vector");
    let sidecar_error = runtime
        .block_on(semantic_matches(
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("semantic matches with sidecar error");
    assert!(
        sidecar_error.notes.iter().any(|note| note.contains("semantic sidecar could not answer"))
    );
}

#[test]
fn search_history_internal_blends_semantic_and_lexical_scores() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let query_vector = runtime.block_on(embed_query(&embedding, "docs")).expect("query vector");
    seed_embedding_with_vector(&connection, 1, &embedding, &query_vector);
    sync_sidecar_vectors(&paths, &embedding, &[(1, query_vector.clone())]);

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
            },
        ))
        .expect("semantic + lexical search");

    assert_eq!(search.items.len(), 1);
    assert_eq!(search.items[0].history_id, 1);
    assert_eq!(search.items[0].match_reason, "Semantic + lexical match");
    assert!((search.items[0].score - 1.08).abs() < 1e-6);
}

#[test]
fn lexical_scoring_helpers_return_expected_values() {
    assert!((lexical_score(0, 5) - 0.6).abs() < 1e-6);
    assert!((lexical_score(4, 5) - 0.456).abs() < 1e-6);
    assert!((lexical_boost(0, 5) - 0.08).abs() < 1e-6);
    assert!((lexical_boost(4, 5) - 0.016).abs() < 1e-6);
}

#[test]
fn semantic_sort_helper_treats_nan_scores_as_equal() {
    let mut rows = [
        StoredEmbedding {
            history_id: 1,
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/low".to_string(),
            title: Some("Low".to_string()),
            domain: "example.com".to_string(),
            visited_at: "2026-04-25T00:00:00Z".to_string(),
            score: 0.1,
        },
        StoredEmbedding {
            history_id: 2,
            profile_id: "chrome:Default".to_string(),
            url: "https://example.com/nan".to_string(),
            title: Some("NaN".to_string()),
            domain: "example.com".to_string(),
            visited_at: "2026-04-25T00:00:01Z".to_string(),
            score: f32::NAN,
        },
    ];

    assert_eq!(sort_stored_embeddings_desc(&rows[0], &rows[1]), std::cmp::Ordering::Equal);
    rows.sort_by(sort_stored_embeddings_desc);
    assert_eq!(rows.len(), 2);
}

#[test]
fn stubbed_llm_and_embedding_helpers_cover_supported_formats() {
    let runtime = Runtime::new().expect("runtime");
    let openai_answer = runtime
        .block_on(run_llm_agent(
            &llm_provider_with_format(AiRequestFormat::OpenAi),
            "system preamble",
            Vec::new(),
            "hello",
        ))
        .expect("openai answer");
    assert!(openai_answer.contains("openai"));

    let ollama_answer = runtime
        .block_on(run_llm_agent(
            &llm_provider_with_format(AiRequestFormat::Ollama),
            "system preamble",
            Vec::new(),
            "hello",
        ))
        .expect("ollama answer");
    assert!(ollama_answer.contains("ollama"));

    let lmstudio_answer = runtime
        .block_on(run_llm_agent(
            &llm_provider_with_format(AiRequestFormat::LmStudio),
            "system preamble",
            Vec::new(),
            "hello",
        ))
        .expect("lmstudio answer");
    assert!(lmstudio_answer.contains("lmstudio"));

    let google_answer = runtime
        .block_on(run_llm_agent(
            &llm_provider_with_format(AiRequestFormat::Google),
            "system preamble",
            Vec::new(),
            "hello",
        ))
        .expect("google answer");
    assert!(google_answer.contains("google"));

    let anthropic_answer = runtime
        .block_on(run_llm_agent(
            &llm_provider_with_format(AiRequestFormat::Anthropic),
            "system preamble",
            Vec::new(),
            "hello",
        ))
        .expect("anthropic answer");
    assert!(anthropic_answer.contains("anthropic"));

    let google_embedding_provider = AiProviderRuntime {
        config: AiProviderConfig {
            id: "google-embed".to_string(),
            name: "Google embeddings".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::Google,
            enabled: true,
            default_model: "text-embedding-004".to_string(),
            dimensions: Some(4),
            ..AiProviderConfig::default()
        },
        api_key: "secret".to_string(),
    };
    let embedding = runtime
        .block_on(embed_query(&google_embedding_provider, "hello"))
        .expect("google embedding");
    assert_eq!(embedding.len(), 4);
    assert_eq!(
        embedding,
        expected_stub_embedding(&google_embedding_provider.config.id, "hello", 4)
    );

    let anthropic_error = runtime
        .block_on(embed_query(
            &AiProviderRuntime {
                config: AiProviderConfig {
                    id: "anthropic-embed".to_string(),
                    name: "Anthropic embeddings".to_string(),
                    purpose: AiProviderPurpose::Embedding,
                    request_format: AiRequestFormat::Anthropic,
                    enabled: true,
                    default_model: "claude-embedding".to_string(),
                    ..AiProviderConfig::default()
                },
                api_key: "secret".to_string(),
            },
            "hello",
        ))
        .expect_err("anthropic embeddings should fail");
    assert!(anthropic_error.to_string().contains("does not support embeddings"));

    let openai_embedding =
        runtime.block_on(embed_query(&embedding_provider(), "docs")).expect("openai embedding");
    assert_eq!(openai_embedding.len(), 3);
    assert_eq!(openai_embedding, expected_stub_embedding("embed", "docs", 3));
    assert_ne!(openai_embedding, embedding);
}

#[test]
fn embedding_retry_helpers_cover_success_error_and_rate_limit_detection() {
    let runtime = Runtime::new().expect("runtime");
    let provider = embedding_provider();
    let texts = vec!["first".to_string(), "second".to_string()];
    let batch =
        runtime.block_on(embed_batch_with_retry(&provider, &texts)).expect("batch embeddings");
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0], expected_stub_embedding("embed", "first", 3));

    let single =
        runtime.block_on(embed_single_with_retry(&provider, "single")).expect("single embedding");
    assert_eq!(single, expected_stub_embedding("embed", "single", 3));

    let mut anthropic = provider.clone();
    anthropic.config.request_format = AiRequestFormat::Anthropic;
    let batch_error = runtime
        .block_on(embed_batch_with_retry(&anthropic, &texts))
        .expect_err("anthropic batch embedding");
    assert!(batch_error.to_string().contains("does not support embeddings"));

    let single_error = runtime
        .block_on(embed_single_with_retry(&anthropic, "single"))
        .expect_err("anthropic single embedding");
    assert!(single_error.to_string().contains("does not support embeddings"));

    let mut attempts = 0;
    assert!(!should_retry_embedding_error(&anyhow::anyhow!("rate limit 429 quota"), &mut attempts));
    assert_eq!(attempts, 1);
    assert!(should_retry_embedding_error(&anyhow::anyhow!("connection refused"), &mut attempts));
    attempts = 2;
    assert!(!should_retry_embedding_error(&anyhow::anyhow!("connection refused"), &mut attempts));

    assert!(embedding_error_is_rate_limited(&anyhow::anyhow!("rate limit 429 quota")));
    assert!(!embedding_error_is_rate_limited(&anyhow::anyhow!("connection refused")));
}

#[test]
fn await_with_ai_cancellation_polls_control_while_provider_future_is_pending() {
    let runtime = Runtime::new().expect("runtime");
    let control: Arc<dyn AiRunControl> = Arc::new(CountdownControl::new(2));

    let value = runtime
        .block_on(await_with_ai_cancellation(Some(&control), "still waiting", async {
            tokio::time::sleep(Duration::from_millis(310)).await;
            Ok::<_, anyhow::Error>(7)
        }))
        .expect("pending provider future");

    assert_eq!(value, 7);
    assert!(control.cancelled());
}
