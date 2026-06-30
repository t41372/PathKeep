//! Regression tests for optional AI and semantic retrieval helpers.
use super::indexing::{
    IndexedVisit, chunk_size, clear_provider_embeddings, collect_stale_history_ids,
    collect_visit_chunk, select_embed_targets, upsert_embedding, validate_embedding_batch_for_keys,
};
use super::provider::{
    embedding_descriptor_for, l2_normalize, normalize_local_base_url, resolve_embed_request_dim,
    should_retry_embedding_error, stub_embedding_dimensions, stub_embedding_vector,
};
use super::*;
use crate::utils::sha256_hex;
use crate::{
    ai_sidecar::SidecarEmbeddingRow,
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
        api_key: Some("secret".into()),
    }
}

/// The built-in in-app STATIC embedding provider runtime (F1) — routes the embed loop to the real
/// in-memory static engine (`AnyEmbeddingProvider::Static`) via the `static:in-app` sentinel.
fn static_embedding_provider() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig {
            id: crate::models::BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID.to_string(),
            name: "In-app static (local)".to_string(),
            purpose: AiProviderPurpose::Embedding,
            request_format: AiRequestFormat::OpenAi,
            enabled: true,
            base_url: Some(STATIC_INAPP_BASE_URL.to_string()),
            default_model: DEFAULT_STATIC_MODEL_REPO.to_string(),
            dimensions: None,
            ..AiProviderConfig::default()
        },
        api_key: None,
    }
}

/// An embedding provider whose configured `dimensions = 0` makes the (stub) transport return empty
/// vectors — used to exercise the F3 0-byte honesty path (the build must FAIL, never count rows).
fn zero_dim_embedding_provider() -> AiProviderRuntime {
    AiProviderRuntime {
        config: AiProviderConfig { dimensions: Some(0), ..embedding_provider().config },
        api_key: Some("secret".into()),
    }
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
        api_key: Some("secret".into()),
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

/// Captures every backfill watermark the embed loop reports, so a test can assert resumption
/// progress is monotone and ends with the full embedded count past the last id.
#[derive(Default)]
struct RecordingLedger {
    progresses: std::sync::Mutex<Vec<IndexBackfillProgress>>,
}

impl RecordingLedger {
    fn snapshot(&self) -> Vec<IndexBackfillProgress> {
        self.progresses.lock().expect("ledger lock").clone()
    }
}

impl IndexBackfillLedger for RecordingLedger {
    fn record(&self, progress: IndexBackfillProgress) -> Result<()> {
        self.progresses.lock().expect("ledger lock").push(progress);
        Ok(())
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

fn expected_stub_embedding(
    provider_id: &str,
    query: &str,
    role: EmbeddingRole,
    dimensions: usize,
) -> Vec<f32> {
    let fingerprint = sha256_hex(format!("{provider_id}::{}::{query}", role.as_str()).as_bytes());
    let bytes = fingerprint.as_bytes();
    let mut vector: Vec<f32> = (0..dimensions)
        .map(|index| ((bytes[index % bytes.len()] % 13) as f32 + 1.0) / 13.0)
        .collect();
    l2_normalize(&mut vector);
    vector
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

/// Adds a SECOND visit row on an EXISTING `urls` row (`url_id`), so one page can have multiple visits
/// with distinct visit ids — the multi-visit-page shape M-12's page-stable fusion must collapse. The
/// new visit id is `visit_id`; `visit_time` controls recency (higher = more recent → the representative).
fn seed_extra_visit(connection: &Connection, visit_id: i64, url_id: i64, visit_time: i64) {
    let profile_row_id =
        "chrome:Default".bytes().fold(0_i64, |acc, value| acc + value as i64).max(1);
    connection
        .execute(
            "INSERT INTO archive.visits
             (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 805306368, 0, ?6, 1, NULL, 1, 0, NULL, NULL, ?7, ?8, ?9)",
            params![
                visit_id,
                url_id,
                visit_id.to_string(),
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time),
                profile_row_id,
                format!("fp-{visit_id}"),
                format!("payload-{visit_id}"),
                now_rfc3339(),
            ],
        )
        .expect("insert extra visit");
    // Keep the url's last_visit bookkeeping consistent with the newer visit so the hydration picks the
    // most-recent visible visit as the representative.
    connection
        .execute(
            "UPDATE archive.urls SET visit_count = visit_count + 1,
               last_visit_ms = MAX(last_visit_ms, ?2), last_visit_iso = ?3
             WHERE id = ?1",
            params![
                url_id,
                chrome_time_to_unix_ms(visit_time),
                crate::utils::chrome_time_to_rfc3339(visit_time)
            ],
        )
        .expect("bump url last_visit");
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

/// Seeds the derived vector planes (W-AI-5) for a provider from `(history_id, content_key, vector)`.
///
/// Writes a fingerprint-stamped `.pkvec` store (one vector per content_key, last-writer-wins), a
/// `.pkmap` fanning each history_id to its content_key, then projects the binary + int8 planes — the
/// exact on-disk state `build_ai_index` leaves, so semantic search resolves real hits in tests.
fn seed_vector_planes(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    rows: &[(i64, u64, Vec<f32>)],
) {
    let dim = rows.first().map(|(_, _, vector)| vector.len()).unwrap_or(1);
    let fingerprint = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        dim,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let store = VectorStore::create_stamped(paths, &fingerprint).expect("stamp store");
    // Dedup to one vector per content_key for the store (its read_all is a set), keep full fan-out for
    // the map.
    let mut seen = std::collections::HashSet::new();
    let mut store_records = Vec::new();
    let mut map_records = Vec::new();
    for (history_id, content_key, vector) in rows {
        if seen.insert(*content_key) {
            store_records.push((*content_key, vector.clone()));
        }
        map_records.push((*history_id, *content_key));
    }
    store.append_vectors(&store_records).expect("append vectors");
    let map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    map.ensure_created(paths).expect("create map");
    map.append(&map_records).expect("append map");
    super::vector_planes::build_planes_from_store(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    )
    .expect("build planes");
    // Project the M-11 keyed reverse/forward sidecars from the `.pkmap`, stamped with the same store
    // fingerprint hash — the exact on-disk state `build_ai_index` now leaves, so the search hydration +
    // `is:starred` paths exercise the KEYED binary-search lookups (the steady-state path), not just the
    // `.pkmap`-scan fallback. (`seed_vector_planes_without_reverse_sidecars` omits this to cover the
    // older-index fallback.)
    super::reverse_visit_map::ReverseVisitMap::for_provider(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    )
    .build_from_visit_map(paths, &map, &fingerprint.hash())
    .expect("build reverse sidecars");
}

/// Seeds the vector planes + `.pkmap` but NOT the M-11 keyed sidecars (the pre-M-11 / torn-sidecar
/// on-disk state), so the search read path exercises the authoritative `.pkmap`-scan FALLBACK.
fn seed_vector_planes_without_reverse_sidecars(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    rows: &[(i64, u64, Vec<f32>)],
) {
    seed_vector_planes(paths, provider, rows);
    // Drop the just-built sidecars so the read path falls back to the `.pkmap` scan (an older index
    // built before the sidecar existed has the planes + map but no `.pkrev`/`.pkfwd`).
    super::reverse_visit_map::ReverseVisitMap::for_provider(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    )
    .delete()
    .expect("drop reverse sidecars");
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

/// Seeds one `search.search_documents` row carrying `enrichment_text` for a URL (REACH-C3 tests).
///
/// `seed_visit` sets `urls.id == history_id`, and `search_documents` is keyed on that same `url_id`, so
/// `url_id == history_id` here. Writes directly to the derived SEARCH plane (the same DB the lexical
/// path projects into), so the semantic hydration JOIN finds an excerpt to cap. The other lexical
/// columns are stubbed `''` because this path only reads `enrichment_text`.
fn seed_enrichment_text(paths: &ProjectPaths, url_id: i64, url: &str, enrichment_text: &str) {
    let search = Connection::open(&paths.search_database_path).expect("open search projection");
    search
        .execute(
            "INSERT INTO search_documents
               (url_id, url, title, search_terms, normalized_url, normalized_title,
                normalized_search_terms, compact_text, cjk_grams, enrichment_text, updated_at)
             VALUES (?1, ?2, '', '', '', '', '', '', '', ?3, ?4)
             ON CONFLICT(url_id) DO UPDATE SET enrichment_text = excluded.enrichment_text",
            params![url_id, url, enrichment_text, now_rfc3339()],
        )
        .expect("seed enrichment_text");
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

    // Skill OFF capability note is honest about the disabled state.
    assert!(
        preview
            .capability_notes
            .iter()
            .any(|note| note.contains("Usage guide is disabled in saved Settings")),
        "disabled skill capability note must read honestly"
    );

    let mut partially_enabled = AppConfig::default();
    partially_enabled.ai.mcp_enabled = true;
    let enabled_preview =
        preview_ai_integrations(&paths, &partially_enabled).expect("enabled preview");
    assert!(enabled_preview.warnings.is_empty());

    // Skill ON but the MCP server OFF: the capability note must disclose that the
    // guide is enabled yet unreachable, so the user is never misled.
    let mut skill_only = AppConfig::default();
    skill_only.ai.skill_enabled = true;
    let skill_only_preview =
        preview_ai_integrations(&paths, &skill_only).expect("skill-only preview");
    assert!(
        skill_only_preview
            .capability_notes
            .iter()
            .any(|note| note.contains("enabled but unreachable")),
        "an enabled-but-unreachable guide must say so"
    );
    // The warning clears once either outward toggle is on (skill counts).
    assert!(skill_only_preview.warnings.is_empty());

    // Skill ON and the MCP server ON: the guide is reachable and read-only.
    let mut both_on = AppConfig::default();
    both_on.ai.mcp_enabled = true;
    both_on.ai.skill_enabled = true;
    let both_preview = preview_ai_integrations(&paths, &both_on).expect("both-on preview");
    assert!(
        both_preview
            .capability_notes
            .iter()
            .any(|note| note.contains("read-only guide teaching connected tools")),
        "a reachable guide note must describe the read-only guide"
    );
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
            api_key: Some("secret".into()),
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
            api_key: Some("secret".into()),
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
            api_key: Some("secret".into()),
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
    // F3: readiness now requires REAL vectors on the `.pkvec` plane, not just SQLite metadata rows.
    seed_vector_planes(&paths, &provider, &[(1, 0xA1, vec![1.0, 0.0, 0.0])]);

    let status = ai_index_status(&paths, &config, None).expect("status");
    assert!(status.ready);
    assert_eq!(status.state, "ready");
    assert_eq!(status.indexed_items, 1);
    assert_eq!(status.semantic_vector_count, 1, "the real .pkvec vector count is surfaced");
    assert!(status.last_indexed_at.is_some());
}

#[test]
fn ai_index_status_flags_metadata_rows_with_an_empty_vector_store() {
    // F3 (0-byte honesty): SQLite `ai_embeddings` rows exist (a build counted rows) but NO vectors
    // landed on the `.pkvec` plane. The status must NOT report "ready, N indexed" — it surfaces the
    // honest `IndexVectorsMissing` warning, a degraded state, and the real (zero) vector count.
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ghost", Some("Ghost"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ghost");
    // Deliberately do NOT seed vector planes — the dishonest "indexed N with an empty sidecar" case.

    let status = ai_index_status(&paths, &config, None).expect("status");
    assert!(!status.ready, "metadata rows without vectors are NOT ready");
    assert_eq!(status.state, "degraded");
    assert_eq!(status.indexed_items, 1, "the SQLite metadata count is still reported honestly");
    assert_eq!(status.semantic_vector_count, 0, "the REAL vector count is zero");
    assert_eq!(status.warning_code, Some(AiIndexWarning::IndexVectorsMissing));
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

/// `recoverable_ai_jobs` is the drain-kick signal that fixes the wedged-build bug: it must see
/// queued work AND a job orphaned in `running` by a dead worker, while reporting 0 for a missing /
/// uninitialized archive (no work to recover) and 0 for a job a live worker is actively heartbeating.
#[test]
fn recoverable_ai_jobs_sees_queued_and_orphaned_running_work() {
    // No archive on disk yet → nothing to recover (the early guard).
    let missing_paths = test_paths();
    let config = base_config();
    assert_eq!(
        recoverable_ai_jobs(&missing_paths, &config, None).expect("missing archive recoverable"),
        0
    );

    let (paths, config, connection) = prepared_archive();

    // An empty queue has nothing to recover.
    assert_eq!(recoverable_ai_jobs(&paths, &config, None).expect("empty recoverable"), 0);

    // A queued job is recoverable work (the drain should be kicked to run it).
    let queued = ai_queue::enqueue_index_job(&connection, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    assert_eq!(recoverable_ai_jobs(&paths, &config, None).expect("queued recoverable"), 1);

    // Once claimed and actively heartbeating, it is NOT recoverable (a live worker owns it) — so a
    // status poll won't spuriously kick a second drain at it.
    let claimed = ai_queue::claim_next_ai_job(&connection, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);
    assert_eq!(recoverable_ai_jobs(&paths, &config, None).expect("running recoverable"), 0);

    // Orphan it (the worker died → stale heartbeat). Now it is recoverable again, so the next poll
    // re-kicks the drain and the build resumes from its cursor.
    connection
        .execute(
            "UPDATE ai_jobs SET heartbeat_at = ?1 WHERE id = ?2",
            params!["2000-01-01T00:00:00+00:00", claimed.id],
        )
        .expect("age heartbeat");
    assert_eq!(recoverable_ai_jobs(&paths, &config, None).expect("orphan recoverable"), 1);

    // An uninitialized config short-circuits to 0 even with a job present (the second guard branch).
    let mut uninitialized = config.clone();
    uninitialized.initialized = false;
    assert_eq!(
        recoverable_ai_jobs(&paths, &uninitialized, None).expect("uninitialized recoverable"),
        0
    );
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

    // OPTIONAL key: a missing API key MUST NOT make the embedding provider unavailable — a keyless
    // local endpoint (LM Studio / Ollama) needs none, so the index build proceeds. (Before the fix
    // this returned `available == false` with an "API key" warning, blocking keyless providers.)
    config.ai.embedding_providers[0].enabled = true;
    config.ai.embedding_providers[0].api_key_saved = false;
    let readiness = embedding_provider_readiness(&config);
    assert!(readiness.available, "a keyless provider stays available; key is optional");
    assert!(readiness.warning.is_none(), "no blocking warning for a missing optional key");
    assert!(readiness.warning_code.is_none());

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
        canonical_url: None,
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
                enrichment_excerpt: None,
            }],
            notes: Vec::new(),
            note_codes: Vec::new(),
            next_cursor: None,
            applied_limit: None,
            has_more: false,
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

    // History id 1 is "already indexed" iff its stored hash equals the VISIT-INDEPENDENT dedup hash
    // (canonical URL + title + reserved enrichment summary), so it is skipped on the next collect.
    let first_dedup_hash = build_dedup_content_hash("https://example.com/docs", Some("Docs"), None);
    seed_embedding(&connection, 1, &provider, &first_dedup_hash);
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
fn enrichment_summary_re_embed_blast_radius_is_bounded_to_the_enriched_url() {
    // W-ENRICH-1 / 06 §3: filling a URL's enrichment_summary re-hashes (and thus marks for re-embed)
    // ONLY that URL, never the rest of the corpus. We seed two visits, store both as "already embedded"
    // under their NO-summary dedup hash, then enrich ONLY visit 1 — and assert only visit 1 flips to
    // `needs_embedding` while visit 2 stays unchanged. This proves `enrichment_summary_for` resolves the
    // stored summary into the dedup hash with a bounded blast radius.
    let (paths, _config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://github.com/o/r", Some("o/r"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);

    // Both visits are "already embedded" under their summary-less dedup hash.
    seed_embedding(
        &connection,
        1,
        &provider,
        &build_dedup_content_hash("https://github.com/o/r", Some("o/r"), None),
    );
    seed_embedding(
        &connection,
        2,
        &provider,
        &build_dedup_content_hash("https://example.com/blog", Some("Blog"), None),
    );

    // Nothing changed yet → both are skipped (needs_embedding = false).
    let before = collect_visit_chunk(&paths, &connection, &provider, 0, 10).expect("before");
    assert!(
        before.iter().all(|visit| !visit.needs_embedding),
        "no enrichment → nothing to re-embed"
    );

    // Enrich ONLY visit 1 with a capped summary (the content-fetch job would do this).
    crate::enrichment::store_enrichment_with_cache(
        &paths,
        &connection,
        1,
        "github-repo",
        &crate::enrichment::EnrichmentResult {
            status: "success".to_string(),
            readable_title: Some("o/r".to_string()),
            enrichment_summary: Some("A repo about wasm".to_string()),
            extractor_version: Some(1),
            ..crate::enrichment::EnrichmentResult::default()
        },
        None,
    )
    .expect("store enrichment for visit 1");

    let after = collect_visit_chunk(&paths, &connection, &provider, 0, 10).expect("after");
    let visit1 = after.iter().find(|visit| visit.history_id == 1).expect("visit 1");
    let visit2 = after.iter().find(|visit| visit.history_id == 2).expect("visit 2");
    assert!(visit1.needs_embedding, "the enriched URL must re-embed (its dedup hash changed)");
    assert!(
        !visit2.needs_embedding,
        "the un-enriched URL must NOT re-embed (bounded blast radius)"
    );
    // The enriched visit's embedded TEXT now carries the summary (the 06 §4 funnel).
    assert!(visit1.content.contains("Summary: A repo about wasm"));
}

#[test]
fn upsert_embedding_reports_prior_existence() {
    let (paths, _config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let candidates = collect_visit_chunk(&paths, &connection, &provider, 0, 10).expect("collect");
    let visit = &candidates[0];

    // First write: no prior row.
    assert!(
        !upsert_embedding(&connection, &provider, visit, "2026-04-14T00:00:00Z").expect("first")
    );
    // Second write of the same (history_id, provider, model): prior row existed.
    assert!(
        upsert_embedding(&connection, &provider, visit, "2026-04-14T00:00:01Z").expect("second")
    );
    // Still exactly one row (the prior one was replaced, not duplicated).
    assert_eq!(
        provider_embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("count"),
        1
    );
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
            &AiIndexRequest { clear_only: true, ..AiIndexRequest::default() },
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
fn search_history_internal_empty_query_returns_recent_visits() {
    // BROWSE-BY-RECENCY: a blank query is no longer an error — it returns the most recent visits
    // (newest first), the entry point the agent needs to enumerate history / find the date range.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/old", Some("Old"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/new", Some("New"), 99);

    for blank in ["", "   "] {
        let response = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: blank.to_string(),
                    profile_id: None,
                    domain: None,
                    limit: Some(5),
                    cursor: None,
                    sort: None,
                    starred_only: None,
                    start_date: None,
                    end_date: None,
                },
            ))
            .expect("empty query returns recent visits");
        assert_eq!(response.total, 2, "both visits surface for `{blank:?}`");
        assert_eq!(response.provider_id, "recent-visits");
        // Newest visit (history_id 2, visit_time 99) sorts first; its recency score is the max (1.0).
        assert_eq!(response.items[0].history_id, 2);
        assert_eq!(response.items[0].score, 1.0);
        assert_eq!(response.items[0].match_reason, "Most recent visit");
        assert!(response.items[1].score < response.items[0].score, "older row ranks lower");
        // The recency path is pure recall — no degradation notes, no semantic plane.
        assert!(response.notes.is_empty());
        assert!(response.note_codes.is_empty());
        assert!(response.next_cursor.is_none());
    }
}

#[test]
fn search_history_internal_supports_lexical_fallback() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("lexical search");
    assert_eq!(response.total, 1);
    assert_eq!(response.provider_id, "lexical-fallback");
    // RRF over a single lexical-only list, normalized to [0, 1]: the lone result is the max so it
    // normalizes to 1.0 (W-AI-6 replaced the old fixed lexical_score with rank-fusion + normalization).
    assert_eq!(response.items[0].score, 1.0);
    assert!(response.notes.iter().any(|note| note.contains("lexical retrieval")));
}

#[test]
fn search_history_internal_empty_query_honors_starred_facet() {
    // The starred facet still applies on the recency path: with `starred_only`, only starred pages
    // surface, and the starred boost tags them — keeping behavior consistent with the keyword path.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/plain", Some("Plain"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://starred.com/page", Some("Star"), 2);
    connection
        .execute(
            "INSERT INTO archive.star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://starred.com/page', '2026-01-01')",
            [],
        )
        .expect("star a page");

    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: String::new(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: Some(true),
                start_date: None,
                end_date: None,
            },
        ))
        .expect("starred-only recency");
    assert_eq!(response.total, 1, "only the starred page survives the facet");
    assert_eq!(response.items[0].url, "https://starred.com/page");
    assert!(response.items[0].match_reason.contains("Starred"));
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("public search wrapper");
    assert_eq!(response.total, 1);
    assert_eq!(response.provider_id, "lexical-fallback");
    assert!(response.items.iter().any(|item| item.url.contains("/docs")));
}

#[test]
fn semantic_search_history_caps_the_fe_wrapper_at_the_model_facing_row_cap() {
    // The shared internal ceiling was raised to MAX_SEARCH_ROWS (1000) so the run_code SANDBOX can
    // fetch large pages, but the FE/worker `semantic_search_history` wrapper must stay capped at 50
    // (its pre-change behavior) — a UI/model-facing page never needs 1000 rows. Lethal: reverting the
    // wrapper cap lets the raised ceiling through and returns up to 120 here.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_keyword_visits_bulk(&connection, "mlx", 1, 120);

    let response = runtime
        .block_on(semantic_search_history(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "mlx".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(200),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("public search wrapper");
    assert!(
        response.items.len() <= 50,
        "the FE wrapper caps returned rows at the model-facing cap even when 200 are requested: got {}",
        response.items.len()
    );
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
            &AiIndexRequest { full_rebuild: true, limit: Some(5), ..AiIndexRequest::default() },
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
            0,
            0,
            None,
        ))
        .expect_err("controlled build should cancel");
    assert!(error.to_string().contains("cancelled"));
}

#[test]
fn build_ai_index_embeds_rows_into_vector_plane() {
    // The embed loop (W-AI-4a) now writes real vectors to `derived/vectors/` instead of bailing.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/news", Some("News"), 3);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("embed build");
    assert_eq!(report.indexed_items, 3);
    assert_eq!(report.updated_items, 0);
    assert_eq!(report.skipped_items, 0);
    assert!(report.notes[0].contains("vector plane"));

    // Vectors landed on the dedicated plane, keyed by content_key (deduped), fingerprint-stamped.
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert!(store.exists());
    // Three distinct pages → three deduped vectors (one per content_key).
    assert_eq!(store.count().expect("count"), 3);
    let records = store.read_all().expect("read all");
    let header = store.read_header().expect("header").expect("present");
    assert_eq!(header.dim, records[0].1.len());

    // The visit→content map fans each visit to its content_key (the dedup join), all three visits.
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let mut mapped: Vec<i64> = map.mapped_history_ids().expect("mapped").into_iter().collect();
    mapped.sort_unstable();
    assert_eq!(mapped, vec![1, 2, 3]);
    // Each visit's content_key resolves to a vector actually on the store (no orphan map entry).
    let store_keys = store.existing_ids().expect("store keys");
    for content_key in map.referenced_content_keys().expect("referenced") {
        assert!(store_keys.contains(&content_key), "every mapped content_key has a vector");
    }

    // SQLite metadata rows mirror the embedded count; vectors themselves are NOT in SQLite.
    let connection = open_intelligence_connection(&paths, &config, None).expect("reload");
    assert_eq!(
        provider_embedding_count(&connection, &provider.config.id, &provider.config.default_model)
            .expect("count"),
        3
    );

    // A second build with unchanged content re-scans but skips every row (no re-embed).
    let second = runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("idempotent rebuild");
    assert_eq!(second.indexed_items, 0);
    assert_eq!(second.skipped_items, 3);
    assert_eq!(store.count().expect("count after rebuild"), 3);
}

#[test]
fn static_tier_index_writes_real_vectors_and_semantic_search_finds_the_seeded_page() {
    // HEADLINE (F1 + F4 + F3): build the index with the BUILT-IN STATIC tier over a few seeded rows
    // through the REAL in-memory static engine → assert `.pkvec` is NON-ZERO and the vector count
    // matches `indexed_items` → semantic search resolves the seeded page. This exercises the real
    // static embed (tokenize → matrix lookup → mean-pool → L2-norm), not a digest stub.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = static_embedding_provider();
    // Each page carries a DISTINCTIVE vocab word in its title so the bag-of-words static vectors are
    // separable: only the "kestrel" page contains "kestrel".
    seed_visit(
        &connection,
        1,
        "chrome:Default",
        "https://example.com/a",
        Some("kestrel falcon"),
        1,
    );
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/b", Some("banana zebra"), 2);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/c", Some("quantum otter"), 3);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("static build");
    assert_eq!(report.indexed_items, 3);
    assert!(
        report.notes[0].contains("page vector"),
        "honest note reports vectors: {:?}",
        report.notes
    );

    // The `.pkvec` plane is NON-ZERO and its vector count matches the indexed rows (F3 honesty).
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert!(store.exists());
    let vector_count = store.count().expect("count");
    assert_eq!(vector_count, 3, "one vector per distinct page");
    assert_eq!(vector_count, report.indexed_items as u64, "vector count == indexed_items");
    assert!(
        store.path().metadata().expect("meta").len() > 0,
        "the .pkvec store holds non-zero bytes"
    );
    // The stored vectors are the static engine's real dim (not the candle/external stub dim).
    let header = store.read_header().expect("header").expect("present");
    assert!(header.dim > 1, "static engine produced a real multi-dim vector (dim {})", header.dim);

    // Semantic search through the SAME static engine resolves the page carrying the query word.
    let response = runtime
        .block_on(semantic_search_history(
            &paths,
            &config,
            None,
            Some(&provider),
            &AiSearchRequest {
                query: "kestrel".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("semantic search");
    assert!(
        response.items.iter().any(|item| item.url == "https://example.com/a"),
        "semantic search over the real static index returns the seeded 'kestrel' page; got {:?}",
        response.items.iter().map(|item| &item.url).collect::<Vec<_>>()
    );
}

#[test]
fn empty_vector_provider_fails_the_build_without_counting_or_writing_a_store() {
    // HEADLINE (F3): a provider/path that yields empty (zero-dim) vectors must surface a real failure
    // and NEVER report success with an empty store — `indexed_items` stays 0 and no `.pkvec` is left.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = zero_dim_embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/x", Some("Page"), 1);
    drop(connection);

    let error = runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect_err("a zero-dim provider must fail the build, not report success");
    assert!(
        error.to_string().contains("empty vector"),
        "the failure names the empty-vector cause: {error}"
    );

    // No vector store was created (nothing was honestly indexed).
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert!(!store.exists(), "no .pkvec store is left behind by a failed empty-vector build");

    // The status read model reports zero vectors (and is NOT ready) for this provider.
    let mut status_config = config.clone();
    status_config.ai.embedding_provider_id = Some(provider.config.id.clone());
    status_config.ai.embedding_providers = vec![provider.config.clone()];
    let status = ai_index_status(&paths, &status_config, None).expect("status");
    assert_eq!(status.semantic_vector_count, 0);
    assert!(!status.ready);
}

#[test]
fn incremental_revisit_build_maps_without_embedding_and_reports_it_honestly() {
    // F3 honesty note: a NEW visit of an ALREADY-embedded page is MAPPED but produces NO new vector,
    // so the build reports "Mapped N row(s) to existing vectors" rather than implying a fresh embed.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/p", Some("Page"), 1);
    let first = runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("first build");
    assert_eq!(first.indexed_items, 1);
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 1);

    // A NEW visit (history_id 2) of the SAME page (same url_id → same content_key).
    seed_extra_visit(&connection, 2, 1, 2);
    drop(connection);
    let second = runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("second build");
    assert_eq!(second.indexed_items, 1, "the new visit is processed (mapped)");
    assert_eq!(
        store.count().expect("count after revisit"),
        1,
        "no new vector — page already embedded"
    );
    assert!(second.notes[0].contains("Mapped"), "honest mapped note: {:?}", second.notes);
}

#[test]
fn build_with_ledger_persists_last_durable_checkpoint_on_failure() {
    // F2.1: when a chunk fails, the LAST-DURABLE cursor is persisted via the ledger BEFORE the error
    // propagates, so a reclaim resumes from real progress (here the origin, since the first chunk
    // itself fails). Without the failure-path checkpoint the ledger would never see this position.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/x", Some("Page"), 1);
    drop(connection);
    let ledger = Arc::new(RecordingLedger::default());
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    let error = runtime
        .block_on(build_ai_index_with_control(
            &paths,
            &config,
            None,
            &zero_dim_embedding_provider(),
            &AiIndexRequest::default(),
            None,
            0,
            0,
            Some(dyn_ledger),
        ))
        .expect_err("zero-dim build fails");
    assert!(error.to_string().contains("empty vector"));
    let progresses = ledger.snapshot();
    assert_eq!(
        progresses.last().copied(),
        // The watermark is still the origin (the first chunk itself failed), but the denominators are
        // captured at the build's TRUE start: `scan_target` = max candidate history id (1) and
        // `embed_target` = candidate page COUNT (1, the single seeded visit) — proving both determinate
        // denominators are recorded even when the build fails before any chunk completes.
        Some(IndexBackfillProgress {
            next_history_id: 0,
            embedded_so_far: 0,
            scan_target: 1,
            embed_target: 1,
        }),
        "the failure path persisted the last-durable checkpoint (origin + captured denominators)"
    );
}

#[test]
fn ai_index_status_surfaces_the_builtin_static_provider_for_the_ui() {
    // F1 surfacing: the status read model carries the built-in static provider's id, model repo,
    // download state, selection, and default flag so the Settings selector can render them.
    let (paths, mut config, _connection) = prepared_archive();
    let static_provider = crate::models::built_in_static_embedding_provider();
    config.ai.embedding_providers = vec![static_provider.clone()];
    config.ai.embedding_provider_id = Some(static_provider.id.clone());

    let status = ai_index_status(&paths, &config, None).expect("status");
    let surfaced = status.static_embedding.expect("static embedding surfaced");
    assert_eq!(surfaced.provider_id, crate::models::BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID);
    assert_eq!(surfaced.model_repo, DEFAULT_STATIC_MODEL_REPO);
    assert!(!surfaced.model_downloaded, "no static model files on disk in tests");
    assert!(surfaced.selected, "the built-in static provider is the active selection");
    assert!(surfaced.is_default);

    // The empty-default-model branch resolves the repo to the canonical default.
    config.ai.embedding_providers[0].default_model = String::new();
    let status_empty = ai_index_status(&paths, &config, None).expect("status empty model");
    assert_eq!(
        status_empty.static_embedding.expect("static").model_repo,
        DEFAULT_STATIC_MODEL_REPO
    );
}

#[test]
fn select_working_set_ranks_starred_first_then_other_signals() {
    // W-AI-4c heavy-working-set selector (05 §4/§8): the shared hook ranks unique-content candidates
    // by starred (top) ∪ recent ∪ annotated ∪ frequency, bounded + indexed off the archive.
    let (paths, config, _intelligence) = prepared_archive();
    let archive = open_archive_connection(&paths, &config, None).expect("archive");

    // Seed four distinct pages with different signal profiles.
    let now_ms = 1_000_000_000_000_i64;
    let recent_ms = now_ms - 1; // inside any window
    let old_ms = now_ms - 5 * 365 * 86_400_000; // ~5 years ago, outside an 18-month window
    let seed_url = |id: i64, url: &str, visit_count: i64, last_visit_ms: i64| {
        archive
            .execute(
                "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, '', ?5, '', 1, 1)",
                params![id, url, format!("Title {id}"), visit_count, last_visit_ms],
            )
            .expect("seed url");
    };
    // A run + profile so the FK constraints hold.
    archive
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES (1, 'backup', 'test', '2026-01-01', 'UTC', 'success', '[]', '[]', '{}', 0)",
            [],
        )
        .expect("run");
    archive
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at)
             VALUES (1, 'chrome', 'x', 'p', '/p', '2026-01-01', 1, 'chrome:Default', '2026-01-01')",
            [],
        )
        .expect("profile");

    seed_url(1, "https://starred.com/page", 1, old_ms); // STARRED (top weight) but old
    seed_url(2, "https://recent.com/page", 1, recent_ms); // recent only
    seed_url(3, "https://frequent.com/page", 9000, old_ms); // very frequent but old
    seed_url(4, "https://noted.com/page", 1, old_ms); // annotated only
    seed_url(5, "https://cold.com/page", 0, old_ms); // NO active signal → excluded
    // A SECOND raw variant of the recent page, with a HIGHER visit count and old last-visit. It
    // collapses onto the recent page's canonical URL, so the frequency gather must lift that
    // candidate's visit_count to this higher value (the max-across-variants branch).
    seed_url(6, "https://recent.com/page?utm_source=ad", 42, old_ms);

    // Star page 1 (canonical key), annotate page 4 (note), TAG page 2 (the tags gather), and add an
    // unknown star kind that must be ignored (a future query_family star, not errored).
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://starred.com/page', '2026-01-01')",
            [],
        )
        .expect("star");
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('query_family', 'some-future-key', '2026-01-01')",
            [],
        )
        .expect("future star kind");
    archive
        .execute(
            "INSERT INTO url_annotations (url, notes, created_at, updated_at) VALUES ('https://noted.com/page', 'keep this', '2026-01-01', '2026-01-01')",
            [],
        )
        .expect("note");
    archive
        .execute(
            "INSERT INTO url_tags (url, tag, created_at) VALUES ('https://recent.com/page', 'work', '2026-01-01')",
            [],
        )
        .expect("tag");

    let config = crate::WorkingSetConfig::default();
    let candidates = crate::select_working_set(&archive, &config, now_ms, 10).expect("select");

    // The cold page (no signal) is excluded.
    assert!(!candidates.iter().any(|c| c.canonical_url.contains("cold.com")));
    // Starred ranks first (top weight beats frequency/recency/annotation alone).
    assert!(candidates[0].canonical_url.contains("starred.com"));
    assert!(candidates[0].signals.starred);
    // Four active pages are present (the second recent.com variant collapsed onto the first).
    assert_eq!(candidates.len(), 4);
    // The frequent page carries its visit count signal; the annotated page is flagged annotated.
    let frequent = candidates.iter().find(|c| c.canonical_url.contains("frequent.com")).unwrap();
    assert_eq!(frequent.signals.visit_count, 9000);
    let noted = candidates.iter().find(|c| c.canonical_url.contains("noted.com")).unwrap();
    assert!(noted.signals.annotated);
    // The recent page is flagged recent AND annotated (via the tag) AND carries the MAX visit count
    // across its two raw variants (the higher 42, not the original 1) — the max-across-variants path.
    let recent = candidates.iter().find(|c| c.canonical_url.contains("recent.com")).unwrap();
    assert!(recent.signals.recent);
    assert!(recent.signals.annotated, "the tag marks the recent page annotated");
    assert_eq!(recent.signals.visit_count, 42, "max visit count across raw variants");

    // A zero limit returns nothing.
    assert!(crate::select_working_set(&archive, &config, now_ms, 0).expect("zero").is_empty());
}

#[test]
fn select_working_set_marks_domain_starred_pages() {
    // A DOMAIN star marks every page on that registrable domain (the user starred the source).
    let (paths, config, _intelligence) = prepared_archive();
    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    archive
        .execute(
            "INSERT OR IGNORE INTO runs (id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only) VALUES (1, 'backup', 't', '2026', 'UTC', 'success', '[]', '[]', '{}', 0)",
            [],
        )
        .expect("run");
    archive
        .execute(
            "INSERT OR IGNORE INTO source_profiles (id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at) VALUES (1, 'chrome', 'x', 'p', '/p', '2026', 1, 'chrome:Default', '2026')",
            [],
        )
        .expect("profile");
    archive
        .execute(
            "INSERT INTO urls (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id) VALUES (1, 'https://github.com/a/repo', 'Repo', 3, 0, 1, '', 1, '', 1, 1)",
            [],
        )
        .expect("url");
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('domain', 'github.com', '2026')",
            [],
        )
        .expect("domain star");

    let candidates = crate::select_working_set(
        &archive,
        &crate::WorkingSetConfig::default(),
        1_000_000_000_000,
        10,
    )
    .expect("select");
    let github = candidates.iter().find(|c| c.canonical_url.contains("github.com")).unwrap();
    assert!(github.signals.starred, "a page on a starred domain inherits the starred signal");
}

#[test]
fn estimate_reembed_full_counts_unique_pages_and_sizes_cost() {
    // W-AI-9-D: a Full re-embed estimate counts unique pages (`COUNT(*) FROM urls`) and applies the
    // S1 throughput constants. Three seeded pages → 3 / 1.25 / 60 CPU minutes, GPU = /10.
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/a", Some("A"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/b", Some("B"), 2);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/c", Some("C"), 3);
    drop(connection);

    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    let estimate = crate::estimate_reembed(
        &archive,
        &crate::WorkingSetConfig::default(),
        1_000_000_000_000,
        crate::ReembedScope::Full,
    )
    .expect("estimate full");
    assert_eq!(estimate.scope, crate::ReembedScope::Full);
    assert_eq!(estimate.page_count, 3);
    assert!((estimate.est_minutes_cpu - 3.0 / 1.25 / 60.0).abs() < 1e-9);
    assert!((estimate.est_minutes_gpu - estimate.est_minutes_cpu / 10.0).abs() < 1e-9);
    // Honest: in this CPU-only coverage build the GPU path is unavailable.
    assert_eq!(estimate.gpu_available, cfg!(feature = "metal"));
}

#[test]
fn estimate_reembed_working_set_counts_only_the_bounded_set() {
    // The WorkingSet estimate counts only the bounded working set: here one starred page is a member,
    // a cold page (no signal) is not — so the estimate's page_count is 1, not the whole archive.
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://starred.com/page", Some("Star"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://cold.com/page", Some("Cold"), 2);
    // `seed_visit` gives every page visit_count=1 (a frequency signal). Zero the cold page's count so
    // it has NO active signal and is genuinely outside the working set (the starred page is the lone
    // member). An old last_visit keeps it out of the recency window too.
    connection
        .execute("UPDATE archive.urls SET visit_count = 0, last_visit_ms = 1 WHERE id = 2", [])
        .expect("zero cold page");
    drop(connection);

    let archive = open_archive_connection(&paths, &config, None).expect("archive");
    archive
        .execute(
            "INSERT INTO star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://starred.com/page', '2026-01-01')",
            [],
        )
        .expect("star");

    let estimate = crate::estimate_reembed(
        &archive,
        &crate::WorkingSetConfig::default(),
        1_000_000_000_000,
        crate::ReembedScope::WorkingSet,
    )
    .expect("estimate working set");
    assert_eq!(estimate.scope, crate::ReembedScope::WorkingSet);
    assert_eq!(estimate.page_count, 1, "only the starred page is a working-set member");
}

#[test]
fn build_ai_index_working_set_scope_embeds_only_working_set_pages() {
    // W-AI-9-D: a WorkingSet re-embed touches ONLY the bounded heavy-tier set. The starred page is
    // embedded; the cold (no-signal) page is skipped even though it is a fresh candidate.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://starred.com/page", Some("Star"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://cold.com/page", Some("Cold"), 2);
    // Zero the cold page's signals (see the estimate test) so it is genuinely outside the working set.
    connection
        .execute("UPDATE archive.urls SET visit_count = 0, last_visit_ms = 1 WHERE id = 2", [])
        .expect("zero cold page");
    // Star page 1 so it (and only it) is in the working set.
    connection
        .execute(
            "INSERT INTO archive.star (entity_kind, entity_key, starred_at) VALUES ('url', 'https://starred.com/page', '2026-01-01')",
            [],
        )
        .expect("star");
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &provider,
            &AiIndexRequest { scope: crate::ReembedScope::WorkingSet, ..AiIndexRequest::default() },
        ))
        .expect("working-set build");
    // Only the starred page embedded; the cold page is counted as skipped (out of scope).
    assert_eq!(report.indexed_items, 1);
    assert!(report.skipped_items >= 1, "the cold page is skipped (out of working set)");
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 1, "one vector — only the working-set page");
}

#[test]
fn build_ai_index_full_scope_wipes_and_rebuilds() {
    // W-AI-9-D: `ReembedScope::Full` carries full-rebuild semantics — it wipes the plane then embeds
    // every unique page, exactly like the legacy `full_rebuild` flag.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/a", Some("A"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/b", Some("B"), 2);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &provider,
            &AiIndexRequest { scope: crate::ReembedScope::Full, ..AiIndexRequest::default() },
        ))
        .expect("full-scope build");
    assert_eq!(report.indexed_items, 2);
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 2);
}

#[test]
fn build_ai_index_dedupes_repeated_page_into_one_vector_and_maps_every_visit() {
    // W-AI-4c content-hash dedup (05 §1, "the biggest near-free lever"): many visits of the SAME
    // page (same canonical URL + title, tracking-param variants collapsed) must produce ONE vector
    // yet map EVERY visit to it — "5000 gmail visits → 1 embedding" in miniature.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    // Five visits of the SAME page (different tracking params + host casing → same canonical URL),
    // plus one DIFFERENT page → 2 unique contents across 6 visits.
    seed_visit(&connection, 1, "chrome:Default", "https://mail.google.com/inbox", Some("Inbox"), 1);
    seed_visit(
        &connection,
        2,
        "chrome:Default",
        "https://mail.google.com/inbox?utm_source=a",
        Some("Inbox"),
        2,
    );
    seed_visit(&connection, 3, "chrome:Default", "https://Mail.Google.com/inbox", Some("Inbox"), 3);
    seed_visit(
        &connection,
        4,
        "chrome:Default",
        "https://mail.google.com/inbox?ref=x",
        Some("Inbox"),
        4,
    );
    seed_visit(
        &connection,
        5,
        "chrome:Default",
        "https://mail.google.com/inbox?fbclid=z",
        Some("Inbox"),
        5,
    );
    seed_visit(&connection, 6, "chrome:Default", "https://example.com/other", Some("Other"), 6);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("dedup build");
    // All 6 visits are processed (metadata rows), but only 2 UNIQUE pages are embedded.
    assert_eq!(report.indexed_items, 6);

    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 2, "6 visits of 2 pages → 2 deduped vectors");

    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let all = map.read_all().expect("map");
    // Every visit is mapped.
    let mut mapped_ids: Vec<i64> = all.keys().copied().collect();
    mapped_ids.sort_unstable();
    assert_eq!(mapped_ids, vec![1, 2, 3, 4, 5, 6]);
    // The five gmail visits all share ONE content_key; the sixth has its own.
    let gmail_key = all[&1];
    for id in 2..=5 {
        assert_eq!(all[&id], gmail_key, "every gmail visit maps to the same content_key");
    }
    assert_ne!(all[&6], gmail_key, "the different page has its own content_key");
    // Exactly 2 distinct content_keys referenced, both with a vector on the store.
    let referenced = map.referenced_content_keys().expect("referenced");
    assert_eq!(referenced.len(), 2);
    let store_keys = store.existing_ids().expect("store keys");
    assert_eq!(referenced, store_keys, "every referenced content_key has a vector, no orphans");
}

#[test]
fn build_ai_index_resumes_from_cursor_and_records_progress() {
    // The resumable backfill: starting from a cursor only embeds rows at or beyond it, and the
    // ledger receives the advancing watermark.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    let ledger = Arc::new(RecordingLedger::default());
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    // Resume from history_id 3 → only rows 3,4,5 are embedded.
    let report = runtime
        .block_on(build_ai_index_with_control(
            &paths,
            &config,
            None,
            &provider,
            &AiIndexRequest::default(),
            None,
            3,
            0,
            Some(dyn_ledger),
        ))
        .expect("resumed build");
    assert_eq!(report.indexed_items, 3);

    // Visits 3,4,5 were mapped (the per-visit no-miss contract), and 3 distinct pages → 3 vectors.
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let mut mapped: Vec<i64> = map.mapped_history_ids().expect("mapped").into_iter().collect();
    mapped.sort_unstable();
    assert_eq!(mapped, vec![3, 4, 5]);
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 3, "3 distinct pages → 3 deduped vectors");

    // The ledger saw a monotone, advancing watermark ending past the last id with the full count.
    let progresses = ledger.snapshot();
    assert!(!progresses.is_empty());
    let last = progresses.last().expect("final progress");
    assert_eq!(last.embedded_so_far, 3);
    assert!(last.next_history_id >= 6);
    for window in progresses.windows(2) {
        assert!(window[1].next_history_id >= window[0].next_history_id);
        assert!(window[1].embedded_so_far >= window[0].embedded_so_far);
    }
}

#[test]
fn build_ai_index_resume_carries_cumulative_embedded_so_far() {
    // HIGH regression: the persisted `embedded_so_far` (and the embedded/embed_target progress bar)
    // must be CUMULATIVE across a reclaim. A worker that died after embedding some pages resumes with a
    // non-zero `start_embedded_so_far`; the count must continue from there, not restart at this
    // session's partial count — which would collapse the bar to ~0% on the very recovery path the
    // orphan-recovery fix enables.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    let ledger = Arc::new(RecordingLedger::default());
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    // Simulate a reclaim: resume from history_id 3 (rows 3,4,5 remain) with 2 pages already embedded in
    // the prior session.
    runtime
        .block_on(build_ai_index_with_control(
            &paths,
            &config,
            None,
            &provider,
            &AiIndexRequest::default(),
            None,
            3,
            2,
            Some(dyn_ledger),
        ))
        .expect("resumed build");

    // 2 (carried from the prior session) + 3 (embedded this session) = 5 cumulative — NOT 3.
    let last = ledger.snapshot().last().copied().expect("final progress");
    assert_eq!(
        last.embedded_so_far, 5,
        "embedded_so_far must carry the resume seed, not reset to 3"
    );
}

/// Embeds rows [start, …] capped by `limit` and returns the resulting MAPPED VISIT id set + the last
/// reported watermark, so the resume tests can drive deterministic chunks regardless of the
/// build-time `EMBEDDING_BACKFILL_CHUNK`.
///
/// W-AI-4c: the vector store is now keyed by content_key (deduped), so the per-VISIT no-dup/no-miss
/// guarantee lives on the visit→content map. These resume tests assert that visit-level contract via
/// the map's mapped history ids (sorted ascending), preserving the original W-AI-4a intent.
fn run_backfill_chunk(
    runtime: &Runtime,
    paths: &ProjectPaths,
    config: &AppConfig,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    start_history_id: i64,
) -> (Vec<i64>, IndexBackfillProgress) {
    let ledger = Arc::new(RecordingLedger::default());
    let dyn_ledger: Arc<dyn IndexBackfillLedger> = ledger.clone();
    runtime
        .block_on(build_ai_index_with_control(
            paths,
            config,
            None,
            provider,
            request,
            None,
            start_history_id,
            0,
            Some(dyn_ledger),
        ))
        .expect("backfill chunk");
    let map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    let mut ids: Vec<i64> = map.mapped_history_ids().expect("mapped ids").into_iter().collect();
    ids.sort_unstable();
    let last = ledger.snapshot().last().copied().unwrap_or_default();
    (ids, last)
}

#[test]
fn full_rebuild_resume_keeps_rows_below_cursor_and_completes_id_set() {
    // CRITICAL-1: a crashed full_rebuild that resumes with the persisted cursor (and the ORIGINAL
    // full_rebuild request still true) must NOT re-run the destructive wipe — rows already embedded
    // below the cursor MUST survive, and the final store must hold the complete id set exactly once.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    // Chunk 1 of a FULL REBUILD: embed ids 1,2 (limit caps the embed count), cursor advances to 3.
    let full_rebuild =
        AiIndexRequest { full_rebuild: true, limit: Some(2), ..AiIndexRequest::default() };
    let (ids_after_chunk1, progress) =
        run_backfill_chunk(&runtime, &paths, &config, &provider, &full_rebuild, 0);
    assert_eq!(ids_after_chunk1, vec![1, 2], "chunk 1 embedded ids 1,2");
    assert_eq!(progress.next_history_id, 3, "cursor advances to last + 1");

    // Simulate a worker restart that re-claims the job with the ORIGINAL full_rebuild request still
    // set, resuming from the persisted cursor. The wipe MUST be skipped (start_history_id > 0).
    let resume = AiIndexRequest { full_rebuild: true, limit: None, ..AiIndexRequest::default() };
    let (final_ids, _) =
        run_backfill_chunk(&runtime, &paths, &config, &provider, &resume, progress.next_history_id);

    // Rows below the cursor SURVIVED and the full id set is present exactly once.
    assert_eq!(final_ids, vec![1, 2, 3, 4, 5], "rows below the cursor must not be wiped on resume");
    assert_unique(&final_ids);
}

#[test]
fn resume_after_dropped_cursor_does_not_duplicate_vectors() {
    // CRITICAL-2: a crash AFTER a chunk's vectors are appended but BEFORE its SQLite hash rows /
    // cursor are written leaves the vectors on disk while `needs_embedding` is true again. A resume
    // must re-embed those rows WITHOUT appending a second copy. We reproduce the window by deleting
    // the boundary chunk's SQLite metadata, then resuming from before it.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    for id in 1..=5 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }

    // Chunk 1: embed ids 1,2 → vectors on disk, SQLite rows present.
    let chunk = AiIndexRequest { limit: Some(2), ..AiIndexRequest::default() };
    let (ids, _) = run_backfill_chunk(&runtime, &paths, &config, &provider, &chunk, 0);
    assert_eq!(ids, vec![1, 2]);

    // Simulate the crash window: the vectors for 1,2 are on disk but their SQLite hash rows were
    // never written (so they look un-embedded to the next pass).
    clear_provider_embeddings(&connection, &provider).expect("drop metadata");
    drop(connection);

    // Resume re-scanning from id 1 (cursor lost back to the start of the boundary chunk): rows 1,2
    // are re-embedded (needs_embedding true) but their vectors must NOT be appended a second time.
    let resume = AiIndexRequest::default();
    let (final_ids, _) = run_backfill_chunk(&runtime, &paths, &config, &provider, &resume, 1);

    assert_unique(&final_ids);
    let mut sorted = final_ids.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![1, 2, 3, 4, 5], "exact id set, no dup, no miss");
}

#[test]
fn multi_chunk_resume_yields_exact_contiguous_id_set() {
    // HIGH-7: a real multi-chunk run resumed from each persisted cursor must end with EXACTLY the
    // ids [1..=N] — no duplicate, no miss — and each cursor advance is exactly last + 1.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    const N: i64 = 5;
    for id in 1..=N {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    // Walk the backfill two rows at a time, always resuming from the last reported watermark.
    let mut start = 0_i64;
    let mut guard = 0;
    loop {
        let chunk = AiIndexRequest { limit: Some(2), ..AiIndexRequest::default() };
        let (ids, progress) =
            run_backfill_chunk(&runtime, &paths, &config, &provider, &chunk, start);
        assert_unique(&ids);
        if progress.next_history_id == 0 {
            // Nothing was embedded this pass (no candidates at/after `start`): the run is complete.
            break;
        }
        // The cursor advances to exactly last embedded id + 1.
        assert!(progress.next_history_id > start);
        if ids.len() as i64 >= N {
            assert_eq!(progress.next_history_id, N + 1, "final cursor is last id + 1");
            break;
        }
        start = progress.next_history_id;
        guard += 1;
        assert!(guard < 10, "multi-chunk resume should converge");
    }

    // The visit→content map holds EXACTLY the visit id set [1..=5] — no dup, no miss — after the
    // multi-chunk resume (the per-visit contract the store's content_key keying moved here).
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let mut final_ids: Vec<i64> = map.mapped_history_ids().expect("mapped").into_iter().collect();
    final_ids.sort_unstable();
    assert_eq!(final_ids, vec![1, 2, 3, 4, 5], "exact contiguous id set after multi-chunk resume");
    assert_unique(&final_ids);
    // And 5 distinct pages → 5 deduped vectors, also no-dup.
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 5);
}

/// Asserts the id list is a SET (the visit→content map / `.pkvec` read_all no-dup contract).
fn assert_unique(ids: &[i64]) {
    let unique: std::collections::HashSet<i64> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "ids must be unique (a set): {ids:?}");
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
fn answer_history_question_completes_with_lexical_citations_when_evidence_exists() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let response = runtime
        .block_on(answer_history_question(
            &paths,
            &config,
            None,
            &llm_provider(),
            None,
            &AiAssistantRequest { question: "docs".to_string(), profile_id: None, domain: None },
        ))
        .expect("assistant response");

    assert_eq!(response.state, "completed");
    assert_eq!(response.citations.len(), 1);
    assert_eq!(response.citations[0].history_id, 1);
    assert!(response.answer.contains("stub answer"));
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

    // No vector planes were ever built, so the flat index loads empty and search degrades to lexical
    // with an honest "no vectors yet" note (W-AI-5 replaced the old "tracked for v0.3.0" placeholder).
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("semantic empty fallback");
    assert_eq!(response.provider_id, "embed");
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].match_reason, "Lexical match");
    assert!(
        response.notes.iter().any(|note| note.contains("has no vectors yet")),
        "empty index must surface an honest note: {:?}",
        response.notes
    );
}

#[test]
fn build_index_re_embeds_changed_rows_and_counts_updates() {
    // Row 1 already has a (stale) metadata row → re-embedded as an update; row 2 is new.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    seed_embedding(&connection, 1, &embedding, "stale-hash");
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(&paths, &config, None, &embedding, &AiIndexRequest::default()))
        .expect("embed build");
    assert_eq!(report.indexed_items, 2);
    assert_eq!(report.updated_items, 1);

    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 2);
}

#[test]
fn build_index_full_rebuild_resets_the_vector_plane() {
    // A full rebuild deletes any prior store and re-stamps from the freshly observed dim.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    drop(connection);

    runtime
        .block_on(build_ai_index(&paths, &config, None, &embedding, &AiIndexRequest::default()))
        .expect("initial build");
    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 1);

    let report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &embedding,
            &AiIndexRequest { full_rebuild: true, ..AiIndexRequest::default() },
        ))
        .expect("full rebuild");
    assert_eq!(report.indexed_items, 1);
    // Still exactly one record (rebuilt, not duplicated) — the prior store was reset.
    assert_eq!(store.count().expect("count after rebuild"), 1);
}

#[test]
fn build_index_honors_limit_at_a_chunk_boundary() {
    // A `limit` smaller than the candidate count stops after exactly `limit` embeds.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    for id in 1..=4 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("Page"),
            id,
        );
    }
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(
            &paths,
            &config,
            None,
            &embedding,
            &AiIndexRequest { limit: Some(2), ..AiIndexRequest::default() },
        ))
        .expect("limited build");
    assert_eq!(report.indexed_items, 2);
    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 2);
}

#[test]
fn build_index_incremental_rejects_a_stale_store() {
    // An incremental build over a store stamped with a DIFFERENT fingerprint must refuse to append
    // dimension-incompatible vectors — the clear seam W-AI-5 wires re-embed migration into.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    drop(connection);

    // Stamp a store at the SAME (provider, model) — so it shares the store file — but under a
    // different DIM, so the live config is stale against it.
    let stale_fp = EmbeddingFingerprint::new(
        embedding.config.id.clone(),
        embedding.config.default_model.clone(),
        99,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    VectorStore::create_stamped(&paths, &stale_fp).expect("stamp stale store");

    let error = runtime
        .block_on(build_ai_index(&paths, &config, None, &embedding, &AiIndexRequest::default()))
        .expect_err("stale store should block an incremental append");
    assert!(error.to_string().contains("different embedding configuration"));
}

#[test]
fn build_index_incremental_appends_to_a_matching_store() {
    // A second (incremental) build with a NEW row appends to the prior matching store rather than
    // recreating it — exercising the "reuse existing store" lazy-resolution branch.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    drop(connection);

    runtime
        .block_on(build_ai_index(&paths, &config, None, &embedding, &AiIndexRequest::default()))
        .expect("first build");
    let store =
        VectorStore::for_provider(&paths, &embedding.config.id, &embedding.config.default_model);
    assert_eq!(store.count().expect("count"), 1);
    let header_before = store.read_header().expect("header").expect("present");

    // Add a new visit (via the intelligence connection's attached `archive` schema), then run an
    // incremental build (no full_rebuild) so it appends to the existing matching store.
    let connection = open_intelligence_connection(&paths, &config, None).expect("reopen intel");
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    drop(connection);

    let report = runtime
        .block_on(build_ai_index(&paths, &config, None, &embedding, &AiIndexRequest::default()))
        .expect("incremental build");
    assert_eq!(report.indexed_items, 1);
    assert_eq!(store.count().expect("count after incremental"), 2);
    // Same store (same fingerprint header), appended in place.
    assert_eq!(store.read_header().expect("header2").expect("present"), header_before);
}

#[test]
fn incremental_build_does_not_re_embed_a_new_visit_of_an_already_embedded_page() {
    // MEDIUM-5: a PLAIN incremental job starts at cursor 0 with empty dedup sets. Without loading the
    // persisted keys/map when a store already exists, a NEW visit of an already-embedded page would
    // re-embed + append a DUPLICATE `.pkvec` record (bloat that `read_all` last-writer-wins masks).
    // Assert the RAW record count (store.count()), not read_all().len(), so the duplicate is visible.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let provider = embedding_provider();
    // First build embeds ONE page (one canonical URL, one visit).
    seed_visit(&connection, 1, "chrome:Default", "https://mail.google.com/inbox", Some("Inbox"), 1);
    drop(connection);
    runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("first build");
    let store =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    assert_eq!(store.count().expect("count"), 1, "one page → one vector after first build");

    // A NEW visit (history_id 2) of the SAME page (tracking-param variant → same canonical URL/hash).
    let connection = open_intelligence_connection(&paths, &config, None).expect("reopen intel");
    seed_visit(
        &connection,
        2,
        "chrome:Default",
        "https://mail.google.com/inbox?utm_source=x",
        Some("Inbox"),
        2,
    );
    drop(connection);

    // A PLAIN incremental build (cursor 0, no full_rebuild). The new visit must be MAPPED but the page
    // must NOT be re-embedded — the RAW record count stays 1 (no duplicate `.pkvec` record appended).
    runtime
        .block_on(build_ai_index(&paths, &config, None, &provider, &AiIndexRequest::default()))
        .expect("incremental build");
    assert_eq!(
        store.count().expect("raw count after incremental"),
        1,
        "a new visit of an already-embedded page must NOT append a duplicate vector record",
    );

    // The new visit IS mapped to the same content_key (the per-visit no-miss contract holds).
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    let all = map.read_all().expect("map");
    let mut mapped: Vec<i64> = all.keys().copied().collect();
    mapped.sort_unstable();
    assert_eq!(mapped, vec![1, 2], "both visits are mapped");
    assert_eq!(all[&1], all[&2], "the new visit maps to the SAME content_key (one page)");
}

#[test]
fn validate_embedding_batch_pairs_keys_and_guards_shape() {
    // Happy path: content_keys paired with vectors, effective dim = first vector length.
    let (dim, records) =
        validate_embedding_batch_for_keys(&[10, 20], &[vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]])
            .expect("valid batch");
    assert_eq!(dim, 3);
    assert_eq!(records, vec![(10u64, vec![0.1, 0.2, 0.3]), (20u64, vec![0.4, 0.5, 0.6])]);

    // Count mismatch.
    let count_error =
        validate_embedding_batch_for_keys(&[1, 2], &[vec![0.1]]).expect_err("count mismatch");
    assert!(count_error.to_string().contains("1 vector(s) for 2 input(s)"));

    // Empty vector.
    let empty_error = validate_embedding_batch_for_keys(&[1], &[vec![]]).expect_err("empty vector");
    assert!(empty_error.to_string().contains("empty vector"));

    // Ragged batch (second vector a different dim).
    let ragged_error = validate_embedding_batch_for_keys(&[1, 2], &[vec![0.1, 0.2], vec![0.3]])
        .expect_err("ragged batch");
    assert!(ragged_error.to_string().contains("ragged batch"));

    // F3 (0-byte honesty): an all-zero batch (right dim, no signal) is rejected, not counted.
    let zero_error = validate_embedding_batch_for_keys(&[1, 2], &[vec![0.0, 0.0], vec![0.0, 0.0]])
        .expect_err("all-zero batch");
    assert!(zero_error.to_string().contains("only zero vectors"));
    // A batch with at least one real vector still passes (a single degenerate row is tolerated).
    let (_, mixed) = validate_embedding_batch_for_keys(&[1, 2], &[vec![0.0, 0.0], vec![0.0, 1.0]])
        .expect("mixed");
    assert_eq!(mixed.len(), 2);
}

/// Builds a synthetic `IndexedVisit` for the pure dedup-selection tests (no archive needed).
fn indexed_visit(history_id: i64, content_hash: &str) -> IndexedVisit {
    IndexedVisit {
        history_id,
        profile_id: "chrome:Default".to_string(),
        url: format!("https://example.com/{history_id}"),
        title: Some("Page".to_string()),
        domain: "example.com".to_string(),
        visited_at: "2026-04-04T00:00:00Z".to_string(),
        content: format!("content-{history_id}"),
        content_key: super::dedup::content_key_from_hash(content_hash),
        content_hash: content_hash.to_string(),
        needs_embedding: true,
    }
}

#[test]
fn select_embed_targets_keeps_distinct_pages_that_collide_on_truncated_u64() {
    // MEDIUM-4: two DISTINCT pages whose content_hashes truncate to the SAME u64 content_key must BOTH
    // be embedded — the work-dedup keys on the FULL hash, never the truncated u64. Construct two hashes
    // that share the first 16 hex chars (so `content_key_from_hash` collides) but differ after.
    let hash_a = "0102030405060708aaaaaaaaaaaaaaaa";
    let hash_b = "0102030405060708bbbbbbbbbbbbbbbb";
    let a = indexed_visit(1, hash_a);
    let b = indexed_visit(2, hash_b);
    // Precondition: the two distinct hashes DO collide on the u64 key (the bug's trigger).
    assert_eq!(a.content_key, b.content_key, "test setup must produce a real u64 collision");
    assert_ne!(a.content_hash, b.content_hash);

    let changed: Vec<&IndexedVisit> = vec![&a, &b];
    let targets = select_embed_targets(
        &changed,
        &std::collections::HashSet::new(),
        &std::collections::HashSet::new(),
    );
    // BOTH pages are selected for embedding (the second is NOT collapsed onto the first's vector).
    assert_eq!(targets.len(), 2, "a u64 collision must NOT drop the second distinct page's embed");
    let target_hashes: Vec<&str> = targets.iter().map(|v| v.content_hash.as_str()).collect();
    assert!(target_hashes.contains(&hash_a));
    assert!(target_hashes.contains(&hash_b));

    // The SAME page seen twice (same full hash) IS deduped to one embed (the intended dedup).
    let a_again = indexed_visit(3, hash_a);
    let same_page: Vec<&IndexedVisit> = vec![&a, &a_again];
    let one = select_embed_targets(
        &same_page,
        &std::collections::HashSet::new(),
        &std::collections::HashSet::new(),
    );
    assert_eq!(one.len(), 1, "two visits of one page (same full hash) → one embed");

    // A page whose FULL hash is already persisted this run is skipped (no re-embed).
    let persisted_hashes = std::collections::HashSet::from([hash_a.to_string()]);
    let skip = select_embed_targets(&changed, &persisted_hashes, &std::collections::HashSet::new());
    assert_eq!(skip.len(), 1, "the already-embedded full hash is skipped, the other still embeds");
    assert_eq!(skip[0].content_hash, hash_b);

    // The u64 storage-boundary backstop still skips a key already on the `.pkvec` plane (resume).
    let persisted_keys = std::collections::HashSet::from([a.content_key]);
    let backstop = select_embed_targets(&[&a], &std::collections::HashSet::new(), &persisted_keys);
    assert!(backstop.is_empty(), "a u64 key already on disk is the resume backstop skip");
}

#[test]
fn chunk_size_clamps_to_remaining_and_constant() {
    // No remaining cap → the full constant.
    assert_eq!(chunk_size(None), super::EMBEDDING_BACKFILL_CHUNK);
    // A small remaining cap shrinks the chunk; zero floors to 1 (the loop checks `Some(0)` first).
    assert_eq!(chunk_size(Some(1)), 1);
    assert_eq!(chunk_size(Some(0)), 1);
    // A cap larger than the constant is capped at the constant.
    assert_eq!(chunk_size(Some(100_000)), super::EMBEDDING_BACKFILL_CHUNK);
}

#[test]
fn semantic_matches_returns_no_hits_with_honest_note_when_index_is_empty() {
    // No vector planes built → the flat index loads empty → an honest "no vectors yet" note and no
    // hits (W-AI-5 replaced the old "tracked for v0.3.0" placeholder). The legacy SQLite sidecar
    // write path is STILL deferred, which we assert too.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // A SQLite compatibility row exists (the staleness reasons gate on it) but NO planes were built.
    seed_embedding(&connection, 1, &embedding, "sqlite-only-hash");

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches on empty index");
    assert!(report.hits.is_empty());
    assert!(
        report.notes.iter().any(|note| matches!(note, AiSearchNote::EmptySemanticIndex)),
        "empty index must surface an honest note CODE: {:?}",
        report.notes
    );

    // The deferred legacy sidecar WRITE path still errors (vectors live on the derived planes now).
    let sidecar_error = runtime
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
                vector: vec![0.1, 0.2, 0.3],
            }],
            true,
            false,
            &[],
        ))
        .expect_err("semantic sidecar writes are deferred");
    assert!(sidecar_error.to_string().contains("tracked for PathKeep v0.3.0"));

    // Staleness reasons still surface (watermark + enrichment change) for a built-but-stale index.
    let stale_watermark = semantic_index_staleness_reason(
        &connection,
        &embedding.config.id,
        &embedding.config.default_model,
        1,
        Some("2026-01-01T00:00:00Z"),
    )
    .expect("stale watermark");
    assert_eq!(stale_watermark, Some(AiSemanticStaleness::Watermark));

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
    assert_eq!(stale_enrichment, Some(AiSemanticStaleness::Enrichment));
}

#[test]
fn semantic_matches_returns_real_hits_and_surfaces_stale_ledger_note() {
    // Built planes → real two-stage recall returns the page whose stored vector matches the query
    // embedding; a stale ledger watermark still surfaces its rebuild note alongside the hits.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);

    // The deterministic stub embeds "docs" to a known query vector; seed page 1's stored vector to
    // that same vector (an exact match) and page 2 to an orthogonal one.
    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    let other_vector: Vec<f32> = docs_vector.iter().rev().map(|value| -value).collect();
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x2222, other_vector)],
    );
    // SQLite compatibility rows (the staleness reason gates on them) + a stale ledger watermark so the
    // staleness note is exercised alongside real hits.
    seed_embedding(&connection, 1, &embedding, "hash-1");
    seed_successful_index_ledger(&connection, &embedding, 1);

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches with built planes");
    assert!(!report.hits.is_empty(), "a built index must return real hits");
    // Page 1 (the exact match) must rank first.
    assert_eq!(report.hits[0].history_id, 1);
    assert_eq!(report.hits[0].match_reason, "Semantic match");
    assert!(report.hits[0].url.contains("/docs"));
    assert!(
        report.notes.iter().any(|note| matches!(
            note,
            AiSearchNote::Stale { reason: AiSemanticStaleness::Watermark }
        )),
        "the stale ledger note CODE must still surface: {:?}",
        report.notes
    );
}

#[test]
fn semantic_matches_falls_back_to_pkmap_scan_when_reverse_sidecars_absent() {
    // M-11 read-path guard: an OLDER index (planes + `.pkmap` present, but no `.pkrev`/`.pkfwd`) must
    // still resolve the SAME hits via the authoritative `.pkmap` scan fallback — no wrong/empty result.
    // This exercises the `reverse_sidecars_usable == false` branch + `VisitContentMap` scan path.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/blog", Some("Blog"), 2);
    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    let other_vector: Vec<f32> = docs_vector.iter().rev().map(|value| -value).collect();
    // Seed the planes + `.pkmap` but DROP the keyed sidecars (the pre-M-11 on-disk shape).
    seed_vector_planes_without_reverse_sidecars(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x2222, other_vector)],
    );
    seed_embedding(&connection, 1, &embedding, "hash-1");
    seed_successful_index_ledger(&connection, &embedding, 1);

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches via fallback");
    assert!(!report.hits.is_empty(), "the `.pkmap`-scan fallback must still return the real hit");
    assert_eq!(report.hits[0].history_id, 1, "fallback resolves the same most-recent visit");
    assert!(report.hits[0].url.contains("/docs"));
}

#[test]
fn semantic_matches_rejects_a_dim_mismatch_with_an_honest_note() {
    // D1: the planes were built at dim 4, but the live embedding config produces a dim-3 query. A
    // search would binarize the query to a narrower byte width and prefix-compare garbage, so the
    // config-drift guard must return NO semantic hits + an honest "different embedding configuration
    // (vector dimension changed)" note, BEFORE any meaningless score can reach the merge.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider(); // dimensions: Some(3) → query embeds at dim 3
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // Build the planes at dim 4 (a 4-component stored vector) — a different dim than the live query.
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, vec![0.5, 0.5, 0.5, 0.5])]);

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches under a dim mismatch");
    assert!(report.hits.is_empty(), "a dim mismatch must yield NO semantic hits");
    assert!(
        report.notes.iter().any(|note| matches!(note, AiSearchNote::ConfigDriftDimension)),
        "the dim-mismatch note CODE must surface: {:?}",
        report.notes
    );
}

#[test]
fn semantic_matches_rejects_same_dim_fingerprint_drift_with_an_honest_note() {
    // D1: the planes were built at the SAME dim as the live query (3) but under a DIFFERENT fingerprint
    // (Mean pooling), while the live External engine reports Unknown pooling. The bytes line up but the
    // geometry differs, so scores would still be meaningless. `planes_are_stale` must catch the drift
    // and the guard must degrade to lexical-only with a rebuild note.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    // Stamp the source store (and thus the planes) with a Mean-pooled fingerprint — same dim, different
    // hash than the live External (Unknown-pooled) descriptor the guard rebuilds.
    let drifted = EmbeddingFingerprint::new(
        embedding.config.id.clone(),
        embedding.config.default_model.clone(),
        docs_vector.len(),
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Mean,
        None,
    );
    let store = VectorStore::create_stamped(&paths, &drifted).expect("stamp drifted store");
    store.append_vectors(&[(0x1111, docs_vector.clone())]).expect("append");
    let map = VisitContentMap::for_provider(
        &paths,
        &embedding.config.id,
        &embedding.config.default_model,
    );
    map.ensure_created(&paths).expect("create map");
    map.append(&[(1, 0x1111)]).expect("append map");
    super::vector_planes::build_planes_from_store(
        &paths,
        &embedding.config.id,
        &embedding.config.default_model,
    )
    .expect("build planes");

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches under fingerprint drift");
    assert!(report.hits.is_empty(), "a fingerprint drift must yield NO semantic hits");
    assert!(
        report.notes.iter().any(|note| matches!(note, AiSearchNote::ConfigDriftFingerprint)),
        "the fingerprint-drift note CODE must surface: {:?}",
        report.notes
    );
}

#[test]
fn semantic_matches_filters_by_profile_and_domain_facets() {
    // Two profiles share the SAME page content (one content_key); a profile facet must keep only the
    // matching profile's visit, and a domain facet only the matching domain.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Work", "https://example.com/docs", Some("Docs"), 5);
    seed_visit(&connection, 2, "chrome:Home", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 3, "chrome:Work", "https://other.com/page", Some("Other"), 3);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    let other_vector: Vec<f32> = docs_vector.iter().rev().map(|value| -value).collect();
    // Visits 1 + 2 share content_key 0x1111 (same page, two profiles); visit 3 is a different page.
    seed_vector_planes(
        &paths,
        &embedding,
        &[
            (1, 0x1111, docs_vector.clone()),
            (2, 0x1111, docs_vector.clone()),
            (3, 0x3333, other_vector),
        ],
    );

    // Profile facet: only chrome:Work's visit of the shared page survives.
    let by_profile = runtime
        .block_on(semantic_matches(
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: Some("chrome:Work".to_string()),
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("profile facet");
    let docs_hit = by_profile.hits.iter().find(|hit| hit.url.contains("/docs")).expect("docs hit");
    assert_eq!(docs_hit.profile_id, "chrome:Work");
    assert_eq!(docs_hit.history_id, 1, "the representative visit is chrome:Work's");

    // Domain facet: only other.com survives.
    let by_domain = runtime
        .block_on(semantic_matches(
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: Some("other.com".to_string()),
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("domain facet");
    assert!(by_domain.hits.iter().all(|hit| hit.domain == "other.com"));
    assert!(by_domain.hits.iter().any(|hit| hit.history_id == 3));
}

#[test]
fn semantic_matches_picks_most_recent_visit_among_a_pages_visits() {
    // One page (content_key) with TWO visible visits: hydration must collapse them to the SINGLE
    // most-recent visit (the dedup fan-out), exercising the keep-newer / replace-older arms.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    // Distinct VALID Chrome microsecond timestamps (above the epoch offset) so the two visits resolve
    // to distinct unix-ms (tiny values clamp to the same floor and would not be orderable).
    let earlier = 13_300_000_000_000_000_i64; // ~2022
    let later = 13_400_000_000_000_000_i64; // ~2025, more recent
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), earlier);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/docs", Some("Docs"), later);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    // Both visits share ONE content_key (same page), so the index has one vector but two visits map.
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x1111, docs_vector)],
    );

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches");
    let docs_hits: Vec<_> = report.hits.iter().filter(|hit| hit.url.contains("/docs")).collect();
    assert_eq!(docs_hits.len(), 1, "the page collapses to ONE representative hit");
    assert_eq!(docs_hits[0].history_id, 2, "the most-recent visit (time 9) represents the page");
}

#[test]
fn semantic_matches_attaches_capped_enrichment_excerpt_for_an_enriched_hit() {
    // REACH-C3: an enriched page's semantic hit carries the SAME honest excerpt the lexical reader does,
    // capped on a CHAR boundary. Over-budget enrichment text proves the shared cap is applied (not the
    // raw summary), so the search payload stays bounded at the 14.4M tail.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // Enrichment text well over the 180-char budget → must come back capped + ellipsised.
    let long_summary = "Reusable workflow runner. ".repeat(20);
    seed_enrichment_text(&paths, 1, "https://example.com/docs", &long_summary);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, docs_vector)]);

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches with an enriched hit");
    assert_eq!(report.hits.len(), 1, "the enriched page returns one hit");
    let excerpt = report.hits[0]
        .enrichment_excerpt
        .as_deref()
        .expect("an enriched semantic hit must carry an excerpt");
    assert!(excerpt.starts_with("Reusable workflow runner"), "the excerpt is the enrichment text");
    assert!(excerpt.ends_with('…'), "over-budget text is capped with a trailing ellipsis");
    // The shared cap takes ≤180 CONTENT chars, then appends one ellipsis glyph (so a truncated excerpt
    // is at most 181 chars — the cap is on the content, not the rendered string).
    assert!(excerpt.chars().count() <= 181, "the excerpt honors the shared char cap");
    assert!(
        excerpt.chars().filter(|character| *character != '…').count() <= 180,
        "at most 180 content chars survive the cap"
    );
}

#[test]
fn semantic_matches_leaves_excerpt_none_for_a_non_enriched_hit() {
    // The honest non-enriched outcome: a page with NO enrichment text gets NO snippet (the band + reason
    // carry the "why"). The LEFT JOIN still returns the row — it is not dropped for lacking an excerpt.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // No `seed_enrichment_text` → there is no `search_documents` row for this URL at all, so the LEFT
    // JOIN yields a NULL `enrichment_text`. The row must still come back, just without an excerpt.

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, docs_vector)]);

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches with a non-enriched hit");
    assert_eq!(report.hits.len(), 1, "a non-enriched page is still returned (LEFT JOIN)");
    assert_eq!(
        report.hits[0].enrichment_excerpt, None,
        "a non-enriched page gets NO snippet — never a fabricated one"
    );
}

#[test]
fn semantic_matches_enrichment_join_keeps_one_hit_for_a_multi_visit_enriched_page() {
    // The hydration JOIN must not drop or DUPLICATE results: ONE enriched URL with TWO visits (one
    // `search_documents` row) still collapses to ONE hit carrying the excerpt — the LEFT JOIN does not
    // fan a result out per visit, and the dedup keeps the most-recent visit.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    let earlier = 13_300_000_000_000_000_i64;
    let later = 13_400_000_000_000_000_i64;
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), earlier);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/docs", Some("Docs"), later);
    // `seed_visit` keys `urls.id` off `history_id`, so the two visits are url_ids 1 and 2; both share
    // ONE content_key (one page) for the dedup. Seed the excerpt on both url_ids so whichever visit wins
    // the dedup carries it (the surviving visit's url_id drives the JOIN).
    seed_enrichment_text(&paths, 1, "https://example.com/docs", "Shared page summary");
    seed_enrichment_text(&paths, 2, "https://example.com/docs", "Shared page summary");

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    // Both visits map to the SAME content_key (one page) so the index returns one vector, two visits.
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x1111, docs_vector)],
    );

    let report = runtime
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches over a multi-visit enriched page");
    let docs_hits: Vec<_> = report.hits.iter().filter(|hit| hit.url.contains("/docs")).collect();
    assert_eq!(docs_hits.len(), 1, "the JOIN must not duplicate a multi-visit page into many hits");
    assert_eq!(docs_hits[0].history_id, 2, "the most-recent visit still represents the page");
    assert_eq!(
        docs_hits[0].enrichment_excerpt.as_deref(),
        Some("Shared page summary"),
        "the surviving hit carries the page's excerpt"
    );
}

#[test]
fn semantic_matches_reports_no_visible_hits_when_all_are_facet_filtered_out() {
    // The index returns matches but the facet filter drops every one → an honest "none visible under
    // the active filters" note and no hits.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Work", "https://example.com/docs", Some("Docs"), 1);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, docs_vector)]);

    let report = runtime
        .block_on(semantic_matches(
            &paths,
            &config,
            None,
            &embedding,
            &AiSearchRequest {
                query: "docs".to_string(),
                // No visit lives under this profile, so every semantic match is filtered out.
                profile_id: Some("chrome:DoesNotExist".to_string()),
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
            None,
        ))
        .expect("semantic matches");
    assert!(report.hits.is_empty());
    assert!(
        report.notes.iter().any(|note| matches!(note, AiSearchNote::SemanticMatchesFilteredOut)),
        "must surface the filtered-out note CODE: {:?}",
        report.notes
    );
}

#[test]
fn search_history_internal_uses_lexical_results_when_index_is_empty() {
    // An embedding provider is selected but no vector planes were built → the index loads empty, so
    // the merged result is lexical-only with an honest "no vectors yet" note (W-AI-5).
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_embedding(&connection, 1, &embedding, "sqlite-only-hash");

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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("lexical fallback search");

    assert_eq!(search.items.len(), 1);
    assert_eq!(search.items[0].history_id, 1);
    assert_eq!(search.items[0].match_reason, "Lexical match");
    assert!(search.notes.iter().any(|note| note.contains("has no vectors yet")));
}

#[test]
fn search_history_internal_merges_real_semantic_hits_with_lexical() {
    // With built planes, a page found by BOTH lexical and semantic recall reconciles on its visit
    // and reads as a combined "Lexical + semantic match"; a semantic-only page joins as a fresh hit.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    // Page 1 is found lexically (title/url match "docs") AND semantically (vector match).
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // Page 2 matches the query vector but NOT the lexical query "docs" (title/url differ).
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/api", Some("Reference"), 2);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x2222, docs_vector.clone())],
    );

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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("merged search");

    let page_one = search.items.iter().find(|item| item.history_id == 1).expect("page 1");
    assert_eq!(
        page_one.match_reason, "Lexical + semantic match",
        "a dual-recall page reads as a combined match"
    );
    let page_two = search.items.iter().find(|item| item.history_id == 2).expect("page 2");
    assert_eq!(
        page_two.match_reason, "Semantic match",
        "a semantic-only page joins as a fresh hit"
    );
}

/// Stars a URL in the archive the AI test harness booted (`base_config` is plaintext).
fn star_url(paths: &ProjectPaths, config: &AppConfig, url: &str) {
    crate::stars::set_star(
        paths,
        config,
        None,
        crate::models::SetStarRequest {
            entity_kind: crate::models::StarEntityKind::Url,
            entity_key: url.to_string(),
            source_profile: None,
        },
    )
    .expect("star url");
}

#[test]
fn rrf_ranks_a_dual_list_page_above_a_single_list_page() {
    // RRF core property (W-AI-6): a page ranked high in BOTH the lexical and semantic lists must beat a
    // page ranked high in only one list, because its score sums both lists' contributions. Page 1 is the
    // top lexical hit AND the top semantic hit; page 2 is only a semantic hit. Page 1 must rank first.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    // Page 1: lexical match for "docs" AND the exact semantic vector → in both lists at rank 0.
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // Page 2: NOT a lexical match for "docs" (title/url differ) but the same strong semantic vector.
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/api", Some("Reference"), 2);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    // Page 1's vector is the exact query; page 2's is slightly less aligned so page 1 also tops semantic.
    let near: Vec<f32> = {
        let mut v = docs_vector.clone();
        if let Some(first) = v.first_mut() {
            *first *= 0.9;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / norm).collect()
    };
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, docs_vector.clone()), (2, 0x2222, near)]);

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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("hybrid search");
    assert_eq!(
        search.items[0].history_id, 1,
        "the dual-list page wins on summed RRF contributions"
    );
    assert_eq!(search.items[0].match_reason, "Lexical + semantic match");
    let page_two = search.items.iter().find(|item| item.history_id == 2).expect("page 2");
    assert!(
        search.items[0].score > page_two.score,
        "page 1 (both lists) scores strictly above page 2 (one list)"
    );
}

#[test]
fn rrf_fuses_a_multi_visit_page_into_one_dual_list_row() {
    // M-12 regression: a frequently-visited page whose TWO newest matching visits BOTH land in the lexical
    // window must fuse into ONE row with the dual-list boost — not duplicate into a fused row PLUS a
    // separate lexical-only row, with only the newest visit earning the RRF dual-list credit. Page A has
    // visits 1 (older) and 2 (newer) on ONE url; the semantic representative is the most-recent visible
    // visit (id 2), while the lexical list ALSO surfaces visit 1 (a DIFFERENT visit id). Page-stable
    // keying (canonical url) collapses both lexical visits onto page A and fuses them with the semantic
    // hit, so page A is ONE "Lexical + semantic match" that out-ranks a single-list page.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();

    // Page A: one url (url_id 1), TWO visits — visit 1 (older) + visit 2 (newer). Both match the lexical
    // query "docs" (shared title/url); the semantic vector matches too. Real chrome-microsecond times
    // (tiny values clamp to unix-ms 0 and would TIE) so visit 2 is strictly newer → the most-recent
    // visible visit (id 2) is the semantic representative, distinct from the older lexical visit (id 1).
    let older = 13_358_534_400_000_000_i64;
    let newer = older + 86_400_000_000; // +1 day in chrome microseconds.
    seed_visit(
        &connection,
        1,
        "chrome:Default",
        "https://example.com/docs",
        Some("Docs Guide"),
        older,
    );
    seed_extra_visit(&connection, 2, 1, newer);
    // Page B: a DISTINCT semantic-only page (no lexical match for "docs") — the single-list comparison.
    seed_visit(
        &connection,
        3,
        "chrome:Default",
        "https://example.com/api",
        Some("Reference"),
        older,
    );

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    // A slightly-less-aligned vector for page B so page A also tops the semantic list.
    let near: Vec<f32> = {
        let mut v = docs_vector.clone();
        if let Some(first) = v.first_mut() {
            *first *= 0.9;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / norm).collect()
    };
    // Map BOTH of page A's visits (1, 2) to its single content_key 0x1111, so the .pkmap fan-out resolves
    // the page to its most-recent visible visit (id 2) as the semantic representative. Page B → 0x2222.
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x1111, docs_vector.clone()), (3, 0x2222, near)],
    );

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
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("hybrid search");

    // Page A appears EXACTLY once (no duplicate lexical-only row for the older visit 1).
    let page_a_rows: Vec<&AiSearchEntry> =
        search.items.iter().filter(|item| item.url == "https://example.com/docs").collect();
    assert_eq!(
        page_a_rows.len(),
        1,
        "the multi-visit page must fuse into ONE row, not duplicate per visit: {:?}",
        search.items.iter().map(|i| (i.history_id, &i.match_reason)).collect::<Vec<_>>()
    );
    // Older visit 1 must NOT survive as a separate row — the page collapsed onto its representative.
    assert!(
        !search.items.iter().any(|item| item.history_id == 1),
        "the older visit (id 1) must not appear as its own lexical-only row"
    );
    let page_a = page_a_rows[0];
    assert_eq!(
        page_a.history_id, 2,
        "the surviving row is the most-recent visible visit (the rep)"
    );
    assert_eq!(
        page_a.match_reason, "Lexical + semantic match",
        "the fused multi-visit page earns the dual-list reason (and the RRF dual-list credit)"
    );
    // The dual-list page out-ranks the single-list page B.
    assert_eq!(search.items[0].url, "https://example.com/docs", "the dual-list page ranks first");
    let page_b = search.items.iter().find(|item| item.history_id == 3).expect("page B");
    assert_eq!(page_b.match_reason, "Semantic match", "page B is single-list (semantic only)");
    assert!(page_a.score > page_b.score, "the fused dual-list page out-ranks the single-list page");
}

#[test]
fn rrf_semantic_weight_zero_degrades_to_lexical_order() {
    // Tunable weights (W-AI-6): zeroing the semantic weight drops its RRF contribution, so ranking
    // collapses to the lexical order even though semantic recall still ran. Page 2 (semantic-only)
    // contributes nothing and falls to the bottom; page 1 (lexical) ranks first.
    let runtime = Runtime::new().expect("runtime");
    let (paths, mut config, connection) = prepared_archive();
    config.ai.semantic_weight = 0.0; // normalize_search_knobs leaves 0.0 valid (disable, not invert).
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/api", Some("Reference"), 2);
    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, docs_vector.clone()), (2, 0x2222, docs_vector.clone())],
    );

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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("lexical-weighted search");
    assert_eq!(search.items[0].history_id, 1, "the lexical hit tops when semantic weight is 0");
    let page_two = search.items.iter().find(|item| item.history_id == 2).expect("page 2");
    // Page 2 has only a semantic rank, whose weight is 0, so its fused score is 0 → bottom.
    assert_eq!(page_two.score, 0.0, "a semantic-only page scores 0 when semantic weight is 0");
}

#[test]
fn rrf_pagination_cursor_walks_the_fused_pool() {
    // RRF must not break pagination (W-AI-6): with a limit-1 page over a fused pool of TWO distinct
    // pages (one lexical-only, one semantic-only), page 1 returns a next_cursor and following it returns
    // the second result without overlap. (The lexical fetch + the per-page semantic hit are each bounded
    // by `limit`, so the pool grows past `limit` only when the semantic plane adds a DISTINCT page — what
    // this seeds: lexical page 1 plus a distinct semantic page 2.) A heavier lexical weight makes the
    // lexical page outrank the semantic page deterministically (no normalized tie) so the page order is
    // stable under parallel runs.
    let runtime = Runtime::new().expect("runtime");
    let (paths, mut config, connection) = prepared_archive();
    config.ai.lexical_weight = 2.0; // lexical page (2/60) outranks the semantic page (1/60) without a tie.
    let embedding = embedding_provider();
    // Page 1: lexical match for "docs", no seeded vector → lexical-only.
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 2);
    // Page 2: NOT a lexical match for "docs" but the exact semantic vector → semantic-only (distinct).
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/api", Some("Reference"), 1);
    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(&paths, &embedding, &[(2, 0x2222, docs_vector.clone())]);

    let first = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(1),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("page 1");
    assert_eq!(first.total, 2, "the fused pool is the lexical page ∪ the distinct semantic page");
    assert_eq!(first.items.len(), 1);
    let cursor = first.next_cursor.clone().expect("a next cursor exists");

    let second = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(1),
                cursor: Some(cursor),
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("page 2");
    assert_eq!(second.items.len(), 1);
    assert!(second.next_cursor.is_none(), "the second page is the last");
    assert_ne!(
        first.items[0].history_id, second.items[0].history_id,
        "the cursor advances to a different result"
    );
}

#[test]
fn ai_off_search_is_lexical_only() {
    // Honest degradation (W-AI-6): no embedding provider → lexical-only, RRF degrades to the lexical
    // list, and the honest "lexical retrieval only" note is present.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
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
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("lexical-only");
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].match_reason, "Lexical match");
    assert!(response.notes.iter().any(|note| note.contains("lexical retrieval only")));
}

#[test]
fn starred_boost_promotes_a_relevant_favorite_without_dominating() {
    // THE bounded-boost property (W-AI-6, 05 §10): the starred boost must promote a *relevant* starred
    // page but must NOT let an *irrelevant* starred page leapfrog a strongly-relevant unstarred page —
    // otherwise semantic search degenerates into a bookmark list.
    //
    // Corpus (query "rust"):
    //  - page 1 RELEVANT + UNSTARRED: exact semantic vector + lexical "rust" match → normalizes near 1.0
    //  - page 2 MODEST + STARRED: a weak-but-positive semantic vector, no lexical match → low fusion,
    //    lifted by the boost so it sits ABOVE an equally-modest unstarred page…
    //  - page 3 MODEST + UNSTARRED: the SAME weak vector as page 2 but no star → the boost baseline
    //  - page 4 IRRELEVANT + STARRED: a near-orthogonal vector → very low fusion; even +boost must stay
    //    BELOW the relevant unstarred page 1.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/rust-guide", Some("Rust"), 4);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/star-mid", Some("Mid"), 3);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/plain-mid", Some("Plain"), 2);
    seed_visit(&connection, 4, "chrome:Default", "https://example.com/star-off", Some("Off"), 1);

    let rust_vector = runtime
        .block_on(embed_query(&embedding, "rust", EmbeddingRole::Query))
        .expect("rust vector");
    // A weak-but-positive variant (blend the query with an orthogonal direction) and a near-orthogonal
    // one, both L2-normalized so dot == cosine.
    let normalize = |mut v: Vec<f32>| {
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        v
    };
    let orthogonal: Vec<f32> = rust_vector.iter().rev().copied().collect();
    let weak: Vec<f32> = normalize(
        rust_vector.iter().zip(orthogonal.iter()).map(|(a, b)| 0.5 * a + 0.5 * b).collect(),
    );
    let near_orthogonal: Vec<f32> = normalize(
        rust_vector.iter().zip(orthogonal.iter()).map(|(a, b)| 0.05 * a + 0.95 * b).collect(),
    );
    seed_vector_planes(
        &paths,
        &embedding,
        &[
            (1, 0x1111, rust_vector.clone()),
            (2, 0x2222, weak.clone()),
            (3, 0x3333, weak.clone()),
            (4, 0x4444, near_orthogonal),
        ],
    );
    // Star the modestly-relevant page 2 and the irrelevant page 4.
    star_url(&paths, &config, "https://example.com/star-mid");
    star_url(&paths, &config, "https://example.com/star-off");

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "rust".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("boosted hybrid search");

    let rank_of = |id: i64| search.items.iter().position(|item| item.history_id == id).unwrap();
    // BOUNDED: the relevant unstarred page 1 stays on top — an irrelevant favorite can't leapfrog it.
    assert_eq!(search.items[0].history_id, 1, "a strongly-relevant unstarred page stays #1");
    assert!(
        rank_of(4) > rank_of(1),
        "an IRRELEVANT starred page does NOT leapfrog the relevant one"
    );
    // PROMOTING: the modestly-relevant STARRED page 2 ranks above the identical UNSTARRED page 3.
    assert!(rank_of(2) < rank_of(3), "a relevant favorite is promoted over its unstarred twin");
    // The starred pages carry the "(Starred)" affordance the FE shows; the unstarred ones do not.
    let page_two = &search.items[rank_of(2)];
    assert!(page_two.match_reason.contains("(Starred)"), "a boosted result is marked Starred");
    let page_three = &search.items[rank_of(3)];
    assert!(!page_three.match_reason.contains("Starred"), "an unstarred result is not marked");
}

#[test]
fn starred_boost_off_leaves_favorites_unpromoted() {
    // With the boost set to 0 the starred status is inert: the identical pages 2 (starred) and 3
    // (unstarred) tie on fusion and neither carries the "(Starred)" affordance.
    let runtime = Runtime::new().expect("runtime");
    let (paths, mut config, connection) = prepared_archive();
    config.ai.starred_boost = 0.0;
    let embedding = embedding_provider();
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/star-mid", Some("Mid"), 2);
    seed_visit(&connection, 3, "chrome:Default", "https://example.com/plain-mid", Some("Plain"), 1);
    let rust_vector = runtime
        .block_on(embed_query(&embedding, "rust", EmbeddingRole::Query))
        .expect("rust vector");
    seed_vector_planes(
        &paths,
        &embedding,
        &[(2, 0x2222, rust_vector.clone()), (3, 0x3333, rust_vector.clone())],
    );
    star_url(&paths, &config, "https://example.com/star-mid");

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "rust".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("boost-off search");
    assert!(
        search.items.iter().all(|item| !item.match_reason.contains("Starred")),
        "no result is marked Starred when the boost is 0"
    );
}

#[test]
fn is_starred_facet_restricts_semantic_recall_to_starred_pages() {
    // The `is:starred` facet (W-AI-6) pushes the starred allowlist into the SEMANTIC plane: only starred
    // pages may be returned, even though both pages match the query vector equally. Page 1 is starred,
    // page 2 is not; with the facet on, only page 1 survives semantic recall.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    // Both pages share the EXACT query vector; only the star distinguishes them. Neither title/url
    // matches the query text, so this isolates the SEMANTIC allowlist (no lexical contribution).
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/alpha", Some("Alpha"), 2);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/beta", Some("Beta"), 1);
    let needle = runtime
        .block_on(embed_query(&embedding, "needle", EmbeddingRole::Query))
        .expect("needle vector");
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, needle.clone()), (2, 0x2222, needle.clone())],
    );
    star_url(&paths, &config, "https://example.com/alpha");

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "needle".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: Some(true),
                start_date: None,
                end_date: None,
            },
        ))
        .expect("starred-facet search");
    assert_eq!(search.items.len(), 1, "only the starred page survives the facet");
    assert_eq!(search.items[0].history_id, 1);
    assert!(
        search.items.iter().all(|item| item.url.contains("/alpha")),
        "the unstarred page is excluded from semantic recall by the allowlist"
    );
}

#[test]
fn is_starred_facet_resolves_via_pkmap_scan_when_reverse_sidecars_absent() {
    // M-11 read-path guard (forward / `is:starred` direction): an older index (no `.pkrev`/`.pkfwd`)
    // must still resolve the starred content_key allowlist via the authoritative `.pkmap` scan fallback
    // — same outcome as the keyed path. Exercises `resolve_content_keys_for_history_ids`'s fallback.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/alpha", Some("Alpha"), 2);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/beta", Some("Beta"), 1);
    let needle = runtime
        .block_on(embed_query(&embedding, "needle", EmbeddingRole::Query))
        .expect("needle vector");
    // Planes + `.pkmap` present, keyed sidecars DROPPED (the pre-M-11 shape) → forward scan fallback.
    seed_vector_planes_without_reverse_sidecars(
        &paths,
        &embedding,
        &[(1, 0x1111, needle.clone()), (2, 0x2222, needle.clone())],
    );
    star_url(&paths, &config, "https://example.com/alpha");

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "needle".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: Some(true),
                start_date: None,
                end_date: None,
            },
        ))
        .expect("starred-facet search via fallback");
    assert_eq!(search.items.len(), 1, "the fallback resolves the same starred allowlist");
    assert_eq!(search.items[0].history_id, 1);
    assert!(search.items[0].url.contains("/alpha"));
}

#[test]
fn is_starred_facet_before_any_index_build_resolves_empty_without_a_store() {
    // M-11 guard, no-`.pkvec` branch: an `is:starred` query with a provider configured but NOTHING ever
    // embedded (no `.pkvec` store) must not error — `reverse_sidecars_usable` returns false (no store to
    // key off), the `.pkmap` fallback is empty, the allowlist is empty, and semantic recall honestly
    // yields no hits. Exercises the `store.read_header()? is None` branch of `reverse_sidecars_usable`.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/alpha", Some("Alpha"), 1);
    star_url(&paths, &config, "https://example.com/alpha");
    // No `seed_vector_planes` → no `.pkvec`/`.pkmap`/sidecars at all.

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "needle".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: Some(true),
                start_date: None,
                end_date: None,
            },
        ))
        .expect("starred-facet search with no store");
    assert!(search.items.is_empty(), "no store ⇒ empty allowlist ⇒ no semantic hits (no error)");
}

#[test]
fn is_starred_facet_with_nothing_starred_returns_no_semantic_hits() {
    // The facet is honest: when nothing starred is indexed the allowlist is EMPTY, so semantic recall
    // returns nothing rather than silently ignoring the facet. With no lexical match either, the result
    // set is empty.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/alpha", Some("Alpha"), 1);
    let needle = runtime
        .block_on(embed_query(&embedding, "needle", EmbeddingRole::Query))
        .expect("needle vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, needle.clone())]);
    // Nothing is starred.

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "needle".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: Some(true),
                start_date: None,
                end_date: None,
            },
        ))
        .expect("empty-starred-facet search");
    assert!(search.items.is_empty(), "an empty starred allowlist yields no semantic hits");
}

#[test]
fn is_starred_facet_constrains_the_lexical_plane_too() {
    // The facet must also constrain the LEXICAL plane (today the browse facet is FE-only). Both pages
    // are lexical matches for "rust"; with the facet on, only the starred one survives — even with no
    // embedding provider (pure lexical path).
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/rust-a", Some("Rust A"), 2);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/rust-b", Some("Rust B"), 1);
    star_url(&paths, &config, "https://example.com/rust-a");

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None, // lexical-only path
            &AiSearchRequest {
                query: "rust".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: Some(true),
                start_date: None,
                end_date: None,
            },
        ))
        .expect("lexical starred-facet search");
    assert_eq!(search.items.len(), 1, "only the starred lexical row survives the facet");
    assert_eq!(search.items[0].history_id, 1);
}

#[test]
fn is_starred_facet_recalls_an_older_starred_match_on_the_lexical_plane() {
    // MEDIUM regression (Bug 2): the `is:starred` lexical plane must EXPAND recall so a starred page
    // that matches the query text but was visited OLDER than the newest `limit` text matches is still
    // returned. The old code fetched only `limit` of the NEWEST matches then post-filtered to starred,
    // hard-truncating away older starred hits — for AI-off / no-provider users the lexical plane is the
    // ONLY recall path, so they'd silently lose the favorite. Seed MANY recent NON-starred text matches
    // + one OLDER STARRED text match, run lexical-only with the facet on, and assert the old starred row
    // IS returned.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();

    // The starred page is the OLDEST "rust" match (visit_time 1). Then 20 newer non-starred "rust"
    // matches (visit_time 2..=21) so a limit of 5 would, under the old code, fetch only the newest 5
    // (all non-starred) and post-filter to nothing.
    seed_visit(
        &connection,
        1,
        "chrome:Default",
        "https://example.com/rust-old",
        Some("Rust Old"),
        1,
    );
    for visit_id in 2..=21 {
        seed_visit(
            &connection,
            visit_id,
            "chrome:Default",
            &format!("https://example.com/rust-new-{visit_id}"),
            Some("Rust New"),
            visit_id,
        );
    }
    star_url(&paths, &config, "https://example.com/rust-old");

    let search = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None, // lexical-only path (AI-off / no provider)
            &AiSearchRequest {
                query: "rust".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: Some(true),
                start_date: None,
                end_date: None,
            },
        ))
        .expect("lexical starred-facet search with an older starred match");
    // Only the starred page survives the facet, AND it is found despite being older than the newest 5.
    assert_eq!(
        search.items.len(),
        1,
        "only the starred row survives, and it is not truncated away"
    );
    assert_eq!(
        search.items[0].history_id, 1,
        "the OLDER starred match is recalled by the expanded lexical pool"
    );
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
        api_key: Some("secret".into()),
    };
    let embedding = runtime
        .block_on(embed_query(&google_embedding_provider, "hello", EmbeddingRole::Query))
        .expect("google embedding");
    assert_eq!(embedding.len(), 4);
    assert_eq!(
        embedding,
        expected_stub_embedding(
            &google_embedding_provider.config.id,
            "hello",
            EmbeddingRole::Query,
            4
        )
    );

    // The same text under a different role must produce a distinct vector, proving the role
    // threads all the way to the encoder.
    let embedding_document = runtime
        .block_on(embed_query(&google_embedding_provider, "hello", EmbeddingRole::Document))
        .expect("google embedding document role");
    assert_ne!(embedding, embedding_document);

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
                api_key: Some("secret".into()),
            },
            "hello",
            EmbeddingRole::Query,
        ))
        .expect_err("anthropic embeddings should fail");
    assert!(anthropic_error.to_string().contains("does not support embeddings"));

    let openai_embedding = runtime
        .block_on(embed_query(&embedding_provider(), "docs", EmbeddingRole::Document))
        .expect("openai embedding");
    assert_eq!(openai_embedding.len(), 3);
    assert_eq!(
        openai_embedding,
        expected_stub_embedding("embed", "docs", EmbeddingRole::Document, 3)
    );
    assert_ne!(openai_embedding, embedding);
}

#[test]
fn embedding_retry_helpers_cover_success_error_and_rate_limit_detection() {
    let runtime = Runtime::new().expect("runtime");
    let provider = embedding_provider();
    let texts = vec!["first".to_string(), "second".to_string()];
    let batch = runtime
        .block_on(embed_batch_with_retry(&provider, &texts, EmbeddingRole::Document))
        .expect("batch embeddings");
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0], expected_stub_embedding("embed", "first", EmbeddingRole::Document, 3));

    let single = runtime
        .block_on(embed_single_with_retry(&provider, "single", EmbeddingRole::Query))
        .expect("single embedding");
    assert_eq!(single, expected_stub_embedding("embed", "single", EmbeddingRole::Query, 3));

    let mut anthropic = provider.clone();
    anthropic.config.request_format = AiRequestFormat::Anthropic;
    let batch_error = runtime
        .block_on(embed_batch_with_retry(&anthropic, &texts, EmbeddingRole::Document))
        .expect_err("anthropic batch embedding");
    assert!(batch_error.to_string().contains("does not support embeddings"));

    let single_error = runtime
        .block_on(embed_single_with_retry(&anthropic, "single", EmbeddingRole::Query))
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
fn l2_normalize_unit_scales_non_zero_and_leaves_degenerate_vectors_untouched() {
    let mut vector = vec![3.0_f32, 4.0];
    l2_normalize(&mut vector);
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-6);
    assert!((vector[0] - 0.6).abs() < 1e-6);
    assert!((vector[1] - 0.8).abs() < 1e-6);

    // Zero vector: the `norm <= 0.0` guard must leave it exactly as-is (no NaNs).
    let mut zero = vec![0.0_f32, 0.0, 0.0];
    l2_normalize(&mut zero);
    assert_eq!(zero, vec![0.0, 0.0, 0.0]);

    // Non-finite input: the `!norm.is_finite()` guard must leave it untouched. This exercises
    // the is_finite() false outcome distinctly from the zero case, so a mutant that deletes
    // `!norm.is_finite() ||` (which would let 1/inf = 0 turn every component into NaN) is killed.
    let mut non_finite = vec![f32::INFINITY, 1.0];
    l2_normalize(&mut non_finite);
    assert_eq!(non_finite, vec![f32::INFINITY, 1.0]);

    // Empty slice is a no-op.
    let mut empty: Vec<f32> = Vec::new();
    l2_normalize(&mut empty);
    assert!(empty.is_empty());
}

#[test]
fn normalize_local_base_url_rewrites_only_the_localhost_host_label() {
    // The live-bug case: a `localhost` host pins to 127.0.0.1 so reqwest reaches the IPv4-only
    // local server, while scheme, port, and path are preserved byte-for-byte.
    assert_eq!(normalize_local_base_url("http://localhost:1234/v1"), "http://127.0.0.1:1234/v1");
    // The Ollama default (no path) rewrites the host and keeps the port.
    assert_eq!(normalize_local_base_url("http://localhost:11434"), "http://127.0.0.1:11434");
    // No port, no path — just the host.
    assert_eq!(normalize_local_base_url("http://localhost"), "http://127.0.0.1");
    // A query string is preserved.
    assert_eq!(
        normalize_local_base_url("http://localhost:1234/v1?k=v"),
        "http://127.0.0.1:1234/v1?k=v"
    );
    // A fragment is preserved (the authority ends at `#`).
    assert_eq!(
        normalize_local_base_url("http://localhost:8080#frag"),
        "http://127.0.0.1:8080#frag"
    );
    // https scheme is preserved.
    assert_eq!(normalize_local_base_url("https://localhost/"), "https://127.0.0.1/");
    // The host match is case-insensitive but the replacement is always lowercase numeric.
    assert_eq!(normalize_local_base_url("http://LOCALHOST:1234/v1"), "http://127.0.0.1:1234/v1");
    assert_eq!(normalize_local_base_url("http://LocalHost"), "http://127.0.0.1");
    // A bare authority with NO scheme still rewrites (the whole string is authority-then-path).
    assert_eq!(normalize_local_base_url("localhost:1234"), "127.0.0.1:1234");
    assert_eq!(normalize_local_base_url("localhost"), "127.0.0.1");
    // Userinfo before the host is preserved and the host after the LAST `@` is what matches.
    assert_eq!(
        normalize_local_base_url("http://user:pass@localhost:1234/v1"),
        "http://user:pass@127.0.0.1:1234/v1"
    );

    // ── Must NOT rewrite ──────────────────────────────────────────────────────────────────────
    // Already 127.0.0.1: unchanged (idempotent — re-normalizing a normalized URL is a no-op).
    assert_eq!(normalize_local_base_url("http://127.0.0.1:1234/v1"), "http://127.0.0.1:1234/v1");
    // A cloud host is never touched.
    assert_eq!(normalize_local_base_url("https://api.openai.com/v1"), "https://api.openai.com/v1");
    // A host that merely CONTAINS "localhost" is a different, real host — left alone.
    assert_eq!(
        normalize_local_base_url("http://localhost.example.com:1234/v1"),
        "http://localhost.example.com:1234/v1"
    );
    // A "localhost" appearing only in the PATH (not the host) must not be rewritten.
    assert_eq!(
        normalize_local_base_url("https://api.example.com/localhost"),
        "https://api.example.com/localhost"
    );
    // A leading-substring host (`localhosts`) is not the exact label.
    assert_eq!(normalize_local_base_url("http://localhosts:1234"), "http://localhosts:1234");
    // The candle in-app sentinel has no `://`; its host token is `candle`, never `localhost`.
    assert_eq!(normalize_local_base_url(CANDLE_INAPP_BASE_URL), CANDLE_INAPP_BASE_URL);
    // An IPv6 loopback literal is left exactly as-is (its inner colons are not a port separator,
    // and it is never the `localhost` label).
    assert_eq!(normalize_local_base_url("http://[::1]:1234/v1"), "http://[::1]:1234/v1");
}

#[test]
fn embedding_descriptor_reads_effective_dim_from_argument_not_config() {
    // config.dimensions = Some(3) but the descriptor must reflect the ARGUMENT (the real
    // returned length), proving config is never the truth source for effective dim (D4).
    let provider = embedding_provider();
    assert_eq!(provider.config.dimensions, Some(3));
    let descriptor = embedding_descriptor_for(&provider, Some(7));
    assert_eq!(descriptor.effective_dim, Some(7));
    assert_eq!(descriptor.provider_id, "embed");
    assert_eq!(descriptor.model_id, "text-embedding-3-small");
    assert_eq!(descriptor.dtype, EmbeddingDtype::Float32);
    assert!(descriptor.normalized);
    assert_eq!(descriptor.pooling, EmbeddingPooling::Unknown);
    assert!(descriptor.instruction_template.is_none());

    let unknown = embedding_descriptor_for(&provider, None);
    assert_eq!(unknown.effective_dim, None);
}

#[test]
fn resolve_embed_request_dim_requests_native_or_hint_and_rejects_unsupported() {
    // OpenAI-shaped: explicit dim → MRL hint; no dim → native (None, never a hardcoded default).
    let mut openai = embedding_provider();
    assert_eq!(resolve_embed_request_dim(&openai).expect("openai hint"), Some(3));
    openai.config.dimensions = None;
    assert_eq!(resolve_embed_request_dim(&openai).expect("openai native"), None);

    // Gemini: an explicit dim is required because the transport cannot request a native size;
    // an explicit dim is honored, but no dim is a hard error (the Finding-1 D4 fix) rather than
    // a silent 768 fallback.
    let mut gemini = embedding_provider();
    gemini.config.request_format = AiRequestFormat::Google;
    gemini.config.dimensions = Some(256);
    assert_eq!(resolve_embed_request_dim(&gemini).expect("gemini hint"), Some(256));
    gemini.config.dimensions = None;
    let gemini_error = resolve_embed_request_dim(&gemini).expect_err("gemini native must error");
    assert!(gemini_error.to_string().contains("explicit embedding dimension"));

    // Anthropic has no embedding API at all.
    let mut anthropic = embedding_provider();
    anthropic.config.request_format = AiRequestFormat::Anthropic;
    assert!(resolve_embed_request_dim(&anthropic).is_err());
}

#[test]
fn stub_embedding_dimensions_honours_config_or_falls_back_to_synthetic_dim() {
    // Config dim is honoured (no real-model assumption).
    let provider = embedding_provider();
    assert_eq!(stub_embedding_dimensions(&provider).expect("configured dim"), 3);

    // No config dim → synthetic test dim (NOT 1536/768).
    let mut no_dim = embedding_provider();
    no_dim.config.dimensions = None;
    let fallback = stub_embedding_dimensions(&no_dim).expect("fallback dim");
    assert_ne!(fallback, 1536);
    assert_ne!(fallback, 768);
    assert!(fallback > 0);

    // Anthropic still has no embedding path.
    let mut anthropic = embedding_provider();
    anthropic.config.request_format = AiRequestFormat::Anthropic;
    assert!(stub_embedding_dimensions(&anthropic).is_err());
}

#[test]
fn stub_embedding_vector_is_role_sensitive_and_normalized() {
    let provider = embedding_provider();
    let query = stub_embedding_vector(&provider, "same text", EmbeddingRole::Query, 5);
    let document = stub_embedding_vector(&provider, "same text", EmbeddingRole::Document, 5);
    assert_eq!(query.len(), 5);
    assert_ne!(query, document);
    let norm = query.iter().map(|value| value * value).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-6);
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

// ---------------------------------------------------------------------------
// Review-fix M-6 / M-7: stable note/warning CODES replace raw English prose.
// These exhaustively exercise every code variant's MODEL-facing English rendering
// so the wire codes and their English twins can never drift per call site, and
// confirm the search response carries a code per English note in lockstep.
// ---------------------------------------------------------------------------

#[test]
fn ai_search_note_text_covers_every_search_note_code() {
    // Every variant of the closed set maps to a non-empty, distinct English sentence (model-facing /
    // persisted trace), and the staleness variant defers to the shared staleness vocabulary.
    let cases = [
        AiSearchNote::LexicalFallbackNoProvider,
        AiSearchNote::EmptySemanticIndex,
        AiSearchNote::SemanticMatchesFilteredOut,
        AiSearchNote::ConfigDriftDimension,
        AiSearchNote::ConfigDriftFingerprint,
        AiSearchNote::Stale { reason: AiSemanticStaleness::Watermark },
        AiSearchNote::Stale { reason: AiSemanticStaleness::Enrichment },
        AiSearchNote::ProviderResolutionFailed { reason: "connection refused".to_string() },
    ];
    let mut seen = std::collections::HashSet::new();
    for code in &cases {
        let text = code.model_facing_text();
        assert!(!text.trim().is_empty(), "note {code:?} must render English text");
        assert!(seen.insert(text), "note {code:?} must render a DISTINCT sentence");
    }
    // The provider-resolution-failed note weaves its opaque reason.
    assert!(
        AiSearchNote::ProviderResolutionFailed { reason: "connection refused".to_string() }
            .model_facing_text()
            .contains("connection refused")
    );
    // The two staleness reasons render their shared sentences.
    assert!(AiSemanticStaleness::Watermark.model_facing_text().contains("import watermark"));
    assert!(
        AiSemanticStaleness::Enrichment.model_facing_text().contains("Readable-content enrichment")
    );
}

#[test]
fn ai_index_warning_text_covers_every_warning_code() {
    // Every index-health warning variant renders a non-empty English sentence; interpolated variants
    // weave their structural params; the staleness variant defers to the shared vocabulary.
    let cases = [
        AiIndexWarning::ArchiveNotInitialized,
        AiIndexWarning::NoEmbeddingProvider,
        AiIndexWarning::EmbeddingProviderMissing { provider_id: "emb-9".to_string() },
        AiIndexWarning::EmbeddingProviderDisabled { provider_name: "My Embed".to_string() },
        AiIndexWarning::EmbeddingProviderNoApiKey { provider_name: "My Embed".to_string() },
        AiIndexWarning::EmbeddingProviderNoModel { provider_name: "My Embed".to_string() },
        AiIndexWarning::IndexNotBuilt,
        AiIndexWarning::IndexStale { reason: AiSemanticStaleness::Watermark },
        AiIndexWarning::IndexStale { reason: AiSemanticStaleness::Enrichment },
        AiIndexWarning::BuildFailed { reason: "boom from provider".to_string() },
    ];
    for code in &cases {
        let text = super::read_model::ai_index_warning_text(code);
        assert!(!text.trim().is_empty(), "warning {code:?} must render English text");
    }
    // Interpolation params are woven in (structural carry, not pre-baked English).
    assert!(
        super::read_model::ai_index_warning_text(&AiIndexWarning::EmbeddingProviderMissing {
            provider_id: "emb-9".to_string()
        })
        .contains("emb-9")
    );
    assert!(
        super::read_model::ai_index_warning_text(&AiIndexWarning::EmbeddingProviderDisabled {
            provider_name: "My Embed".to_string()
        })
        .contains("My Embed")
    );
    // BuildFailed carries its opaque transport text verbatim (no fixed vocabulary).
    assert_eq!(
        super::read_model::ai_index_warning_text(&AiIndexWarning::BuildFailed {
            reason: "boom from provider".to_string()
        }),
        "boom from provider"
    );
}

#[test]
fn search_response_carries_a_note_code_per_english_note_for_lexical_fallback() {
    // No embedding provider → the lexical-fallback degradation must surface BOTH the stable CODE
    // (for FE localization) and the English note (model-facing), in lockstep and same order.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None, // no embedding provider → lexical fallback
            &AiSearchRequest {
                query: "docs".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(5),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: None,
                end_date: None,
            },
        ))
        .expect("lexical-fallback search");
    assert_eq!(response.note_codes, vec![AiSearchNote::LexicalFallbackNoProvider]);
    // The English note is DERIVED from the code, in code order — never raw prose pushed ad hoc.
    assert_eq!(response.notes, vec![AiSearchNote::LexicalFallbackNoProvider.model_facing_text()]);
}

/// Seeds one visit DATED on a real `"YYYY-MM-DD"` day (at 12:00:00 UTC of that day).
///
/// The existing `seed_visit` timestamps (1 / 2 / 99) all collapse to ~1601 (the Chrome epoch), so they
/// cannot exercise a date filter. This converts the date's midday Unix-ms to a Chrome timestamp (the
/// format `seed_visit` expects) so `visits.visit_time_ms` lands on that real day. Midday avoids any
/// boundary ambiguity with the inclusive [00:00, 23:59.999] day range.
fn seed_visit_on_date(
    connection: &Connection,
    history_id: i64,
    url: &str,
    title: Option<&str>,
    date: &str,
) {
    let (start_ms, _end_ms) =
        crate::utils::date_str_to_unix_ms_range(date).expect("valid YYYY-MM-DD date");
    let midday_unix_ms = start_ms + 12 * 60 * 60 * 1000; // +12h, comfortably inside the day.
    let chrome_time = crate::utils::unix_micros_to_chrome_time(midday_unix_ms * 1000);
    seed_visit(connection, history_id, "chrome:Default", url, title, chrome_time);
}

#[test]
fn search_history_internal_recency_path_excludes_out_of_range_visits() {
    // Test 1 (recency path) — an empty query + start_date == end_date returns ONLY the in-range visit;
    // the out-of-range visit (a different day) is excluded by the date filter threaded into the lexical
    // recency reader. A regression dropping the date predicate would return BOTH.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit_on_date(&connection, 1, "https://in.example/page", Some("In"), "2026-06-19");
    seed_visit_on_date(&connection, 2, "https://out.example/page", Some("Out"), "2026-06-20");

    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: String::new(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: Some("2026-06-19".to_string()),
                end_date: Some("2026-06-19".to_string()),
            },
        ))
        .expect("recency date-filtered search");
    let ids: Vec<i64> = response.items.iter().map(|item| item.history_id).collect();
    assert_eq!(ids, vec![1], "only the in-range visit returns on the recency path: {ids:?}");
}

#[test]
fn search_history_internal_lexical_path_excludes_out_of_range_visits() {
    // Test 1 (lexical path) — a keyword query + a date range returns ONLY the in-range matching visit.
    // Both pages match the keyword "docs"; only the in-range one is within [2026-06-19, 2026-06-19].
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit_on_date(&connection, 1, "https://in.example/docs", Some("Docs in"), "2026-06-19");
    seed_visit_on_date(&connection, 2, "https://out.example/docs", Some("Docs out"), "2026-06-25");

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
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: Some("2026-06-19".to_string()),
                end_date: Some("2026-06-19".to_string()),
            },
        ))
        .expect("lexical date-filtered search");
    let ids: Vec<i64> = response.items.iter().map(|item| item.history_id).collect();
    assert_eq!(ids, vec![1], "only the in-range lexical match returns: {ids:?}");
}

#[test]
fn search_history_internal_semantic_path_excludes_out_of_range_visits() {
    // Test 1 (semantic/hybrid path, H1) — WITH an embedding provider, an out-of-range page that the
    // VECTOR index would otherwise surface must be excluded by the date filter. Both pages carry the
    // SAME query vector (both are strong semantic hits), but only page 1 is inside the date range. Before
    // H1's fix, `load_visit_rows` had no date predicate, so the out-of-range semantic hit (page 2)
    // LEAKED through. This is the regression-proving assertion: revert the SQL date predicate and page 2
    // reappears.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    // Page 1 is in-range; page 2 is out-of-range. Neither matches the keyword (titles/urls differ) so
    // ONLY the semantic plane can surface them — isolating the semantic date filter.
    seed_visit_on_date(&connection, 1, "https://in.example/alpha", Some("Alpha"), "2026-06-19");
    seed_visit_on_date(&connection, 2, "https://out.example/beta", Some("Beta"), "2026-06-25");

    let query_vector = runtime
        .block_on(embed_query(&embedding, "semantic", EmbeddingRole::Query))
        .expect("query vector");
    // Both pages share the exact query vector → both are top semantic hits absent any date filter.
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, query_vector.clone()), (2, 0x2222, query_vector.clone())],
    );

    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "semantic".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(10),
                cursor: None,
                sort: None,
                starred_only: None,
                start_date: Some("2026-06-19".to_string()),
                end_date: Some("2026-06-19".to_string()),
            },
        ))
        .expect("semantic date-filtered search");
    let ids: Vec<i64> = response.items.iter().map(|item| item.history_id).collect();
    assert!(ids.contains(&1), "the in-range semantic hit must be present: {ids:?}");
    assert!(!ids.contains(&2), "the OUT-OF-RANGE semantic hit must be excluded (H1): {ids:?}");
}

#[test]
fn search_history_internal_recency_has_more_and_applied_limit() {
    // Test 2 (recency path) — seed 3 visits; `limit: 2` → exactly 2 items, has_more == true,
    // applied_limit == Some(2). `limit: 5` → has_more == false. Before H3 the recency probe was a no-op
    // so has_more was ALWAYS false; this fails if H3 is reverted.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit_on_date(&connection, 1, "https://a.example/1", Some("One"), "2026-06-17");
    seed_visit_on_date(&connection, 2, "https://a.example/2", Some("Two"), "2026-06-18");
    seed_visit_on_date(&connection, 3, "https://a.example/3", Some("Three"), "2026-06-19");

    let request = |limit: u32| AiSearchRequest {
        query: String::new(),
        profile_id: None,
        domain: None,
        limit: Some(limit),
        cursor: None,
        sort: None,
        starred_only: None,
        start_date: None,
        end_date: None,
    };

    let limited = runtime
        .block_on(search_history_internal(&paths, &config, None, None, &request(2)))
        .expect("limit 2 recency");
    assert_eq!(limited.items.len(), 2, "exactly `limit` items returned");
    assert!(limited.has_more, "3 visits with limit 2 → has_more must be true (H3)");
    assert_eq!(limited.applied_limit, Some(2));

    let unlimited = runtime
        .block_on(search_history_internal(&paths, &config, None, None, &request(5)))
        .expect("limit 5 recency");
    assert_eq!(unlimited.items.len(), 3);
    assert!(!unlimited.has_more, "3 visits with limit 5 → no more rows");
    assert_eq!(unlimited.applied_limit, Some(5));
}

#[test]
fn search_history_internal_lexical_has_more_and_applied_limit() {
    // Test 2 (keyword path) — seed 3 keyword-matching visits; `limit: 2` → 2 items, has_more == true.
    // Before M1 the lexical pool was hard-capped at `limit`, so the fused `total` could never exceed
    // `limit` and has_more was always false. This fails if M1 is reverted.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit_on_date(&connection, 1, "https://a.example/docs1", Some("Docs One"), "2026-06-17");
    seed_visit_on_date(&connection, 2, "https://a.example/docs2", Some("Docs Two"), "2026-06-18");
    seed_visit_on_date(&connection, 3, "https://a.example/docs3", Some("Docs Three"), "2026-06-19");

    let request = |limit: u32| AiSearchRequest {
        query: "docs".to_string(),
        profile_id: None,
        domain: None,
        limit: Some(limit),
        cursor: None,
        sort: None,
        starred_only: None,
        start_date: None,
        end_date: None,
    };

    let limited = runtime
        .block_on(search_history_internal(&paths, &config, None, None, &request(2)))
        .expect("limit 2 keyword");
    assert_eq!(limited.items.len(), 2, "exactly `limit` items returned");
    assert!(limited.has_more, "3 keyword matches with limit 2 → has_more must be true (M1)");
    assert_eq!(limited.applied_limit, Some(2));

    let unlimited = runtime
        .block_on(search_history_internal(&paths, &config, None, None, &request(5)))
        .expect("limit 5 keyword");
    assert_eq!(unlimited.items.len(), 3);
    assert!(!unlimited.has_more, "3 matches with limit 5 → no more rows");
    assert_eq!(unlimited.applied_limit, Some(5));
}

#[test]
fn ai_search_note_and_warning_codes_serialize_camelcase_for_the_fe() {
    // The FE resolves codes by their camelCase wire token; pin a representative of each enum so a
    // rename can't silently break the FE localizer contract.
    let note = serde_json::to_string(&AiSearchNote::LexicalFallbackNoProvider).expect("note json");
    assert_eq!(note, r#"{"code":"lexicalFallbackNoProvider"}"#);
    let stale =
        serde_json::to_string(&AiSearchNote::Stale { reason: AiSemanticStaleness::Watermark })
            .expect("stale json");
    assert_eq!(stale, r#"{"code":"stale","reason":"watermark"}"#);
    let warning = serde_json::to_string(&AiIndexWarning::EmbeddingProviderDisabled {
        provider_name: "Embed".to_string(),
    })
    .expect("warning json");
    assert_eq!(warning, r#"{"code":"embeddingProviderDisabled","providerName":"Embed"}"#);
    let agent = serde_json::to_string(&crate::models::AiAgentNote::MaxStepsReached)
        .expect("agent note json");
    assert_eq!(agent, r#"{"code":"maxStepsReached"}"#);
}

// ============================================================================================
// DATE-ORDERED SEARCH (`sort: "oldest" | "newest"`): the FIX for "when did I FIRST browse X?"
// wrongly answering the most-recent month when older records exist. The relevance/hybrid path
// returns a recency-ranked top-K and then re-sorts by SCORE, so an older match in the index is
// never surfaced. These tests prove the date-ordered enumeration surfaces the EARLIEST/latest
// occurrence and pages through every match in date order — reverting the sort threading fails them.
// ============================================================================================

#[test]
fn sort_oldest_returns_the_first_occurrence_across_years_not_the_recent_month() {
    // THE regression-proving test. The user has "mlx" visits spanning years; `sort:"oldest", limit:1`
    // must return the EARLIEST (2025-03), never the most-recent (2026-06). Reverting the hardcoded
    // `"newest"` in `lexical_history_results` (or re-sorting the date path by score) returns June and
    // fails this — exactly the false "first record is June 18" the agent produced.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    // Seed across years; insert OUT of date order so a stable result cannot come from insertion order.
    seed_visit_on_date(
        &connection,
        1,
        "https://recent.example/mlx",
        Some("mlx recent"),
        "2026-06-18",
    );
    seed_visit_on_date(
        &connection,
        2,
        "https://first.example/mlx",
        Some("mlx first"),
        "2025-03-04",
    );
    seed_visit_on_date(&connection, 3, "https://mid.example/mlx", Some("mlx mid"), "2025-11-20");

    let oldest = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "mlx".to_string(),
                limit: Some(1),
                sort: Some("oldest".to_string()),
                ..AiSearchRequest::default()
            },
        ))
        .expect("oldest-sorted search");
    assert_eq!(oldest.items.len(), 1);
    assert_eq!(
        oldest.items[0].history_id, 2,
        "sort:oldest must return the EARLIEST (2025-03) match, not the recent month: got {:?}",
        oldest.items[0]
    );
    assert_eq!(oldest.provider_id, "date-ordered");
    assert!(oldest.has_more, "two more matches exist beyond the first page");
    assert_eq!(oldest.next_cursor.as_deref(), Some("1"), "the cursor advances by the page size");
}

#[test]
fn sort_newest_returns_the_latest_occurrence_across_years() {
    // The mirror of the oldest test: `sort:"newest", limit:1` returns the LATEST (2026-06) match.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit_on_date(
        &connection,
        1,
        "https://recent.example/mlx",
        Some("mlx recent"),
        "2026-06-18",
    );
    seed_visit_on_date(
        &connection,
        2,
        "https://first.example/mlx",
        Some("mlx first"),
        "2025-03-04",
    );
    seed_visit_on_date(&connection, 3, "https://mid.example/mlx", Some("mlx mid"), "2025-11-20");

    let newest = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "mlx".to_string(),
                limit: Some(1),
                sort: Some("newest".to_string()),
                ..AiSearchRequest::default()
            },
        ))
        .expect("newest-sorted search");
    assert_eq!(newest.items.len(), 1);
    assert_eq!(
        newest.items[0].history_id, 1,
        "sort:newest must return the LATEST (2026-06) match: got {:?}",
        newest.items[0]
    );
    assert_eq!(newest.provider_id, "date-ordered");
}

#[test]
fn sort_oldest_pagination_covers_every_match_in_date_order_without_overlap() {
    // Pagination over the date-ordered list: with >limit matches, walking the cursor returns each page
    // in oldest-first order with NO overlap and FULL coverage. Five matches, page size 2 → pages of
    // [oldest..], [..], [last]; the concatenation is the matches sorted ascending by visit date.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    // Insert deliberately scrambled; the chronological order is 2024-01 < 2024-06 < 2025-01 < 2025-09 < 2026-03.
    seed_visit_on_date(&connection, 10, "https://e.example/mlx", Some("mlx e"), "2026-03-02");
    seed_visit_on_date(&connection, 11, "https://a.example/mlx", Some("mlx a"), "2024-01-05");
    seed_visit_on_date(&connection, 12, "https://d.example/mlx", Some("mlx d"), "2025-09-10");
    seed_visit_on_date(&connection, 13, "https://b.example/mlx", Some("mlx b"), "2024-06-15");
    seed_visit_on_date(&connection, 14, "https://c.example/mlx", Some("mlx c"), "2025-01-20");
    let chronological = vec![11_i64, 13, 14, 12, 10]; // ids sorted ascending by visit date.

    let mut collected: Vec<i64> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let response = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: "mlx".to_string(),
                    limit: Some(2),
                    sort: Some("oldest".to_string()),
                    cursor: cursor.clone(),
                    ..AiSearchRequest::default()
                },
            ))
            .expect("a date-ordered page");
        for item in &response.items {
            collected.push(item.history_id);
        }
        pages += 1;
        assert!(pages <= 5, "pagination must terminate (no infinite loop)");
        match response.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(
        collected, chronological,
        "paging the cursor yields every match exactly once, in oldest-first date order"
    );
    // No overlap is implied by the equality with the deduped chronological list, but assert it plainly.
    let mut sorted_unique = collected.clone();
    sorted_unique.sort_unstable();
    sorted_unique.dedup();
    assert_eq!(sorted_unique.len(), collected.len(), "no page overlap (every id appears once)");
}

#[test]
fn sort_oldest_preserves_date_order_even_when_score_order_differs() {
    // The date-ordered path must NOT be re-sorted by score (unlike the relevance path). We seed rows
    // whose RELEVANCE order is the OPPOSITE of their DATE order: the newer page is the stronger lexical
    // match (its title repeats the term), the older page is a weaker match. With `sort:"oldest"` the
    // OLDER (weaker-scoring) page must still come first — proving date order wins. A regression that
    // re-applied the score sort would put the newer, higher-scoring page first and fail here.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    // Older page: a single "mlx" mention (weaker lexical score).
    seed_visit_on_date(&connection, 1, "https://old.example/page", Some("mlx"), "2025-02-01");
    // Newer page: "mlx" repeated in url + title (stronger lexical score under the relevance path).
    seed_visit_on_date(
        &connection,
        2,
        "https://new.example/mlx/mlx",
        Some("mlx mlx mlx tutorial"),
        "2026-05-01",
    );

    let oldest = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "mlx".to_string(),
                limit: Some(10),
                sort: Some("oldest".to_string()),
                ..AiSearchRequest::default()
            },
        ))
        .expect("oldest-sorted search");
    let ids: Vec<i64> = oldest.items.iter().map(|item| item.history_id).collect();
    assert_eq!(
        ids,
        vec![1, 2],
        "date order (oldest first) must hold even though page 2 is the stronger relevance match: {ids:?}"
    );
}

#[test]
fn sort_oldest_bypasses_the_semantic_rerank() {
    // With an embedding provider configured AND vectors seeded so the semantic plane would, on the
    // relevance path, re-rank the NEWER page to the top, `sort:"oldest"` must STILL return the older
    // page first — proving the date path skips the semantic plane entirely (a meaning re-rank would
    // scramble the chronological order the caller asked for).
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let embedding = embedding_provider();
    seed_visit_on_date(&connection, 1, "https://old.example/mlx", Some("mlx old"), "2025-02-01");
    seed_visit_on_date(&connection, 2, "https://new.example/mlx", Some("mlx new"), "2026-05-01");
    // Seed the exact query vector on BOTH so the semantic plane treats them as equally strong hits;
    // on the relevance path this participates in fusion. The date path must ignore it entirely.
    let query_vector = runtime
        .block_on(embed_query(&embedding, "mlx", EmbeddingRole::Query))
        .expect("query vector");
    seed_vector_planes(
        &paths,
        &embedding,
        &[(1, 0x1111, query_vector.clone()), (2, 0x2222, query_vector.clone())],
    );

    let oldest = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            Some(&embedding),
            &AiSearchRequest {
                query: "mlx".to_string(),
                limit: Some(10),
                sort: Some("oldest".to_string()),
                ..AiSearchRequest::default()
            },
        ))
        .expect("oldest-sorted search with a provider");
    assert_eq!(oldest.items[0].history_id, 1, "the older page leads even with a semantic provider");
    assert_eq!(
        oldest.provider_id, "date-ordered",
        "the date path reports its own provider, not the hybrid one (semantic was skipped)"
    );
    // No semantic degradation/notes are produced on the date path (it never touched the plane).
    assert!(oldest.notes.is_empty(), "the date path emits no semantic notes: {:?}", oldest.notes);
}

#[test]
fn sort_relevance_and_none_keep_the_hybrid_path() {
    // `sort:"relevance"` and an absent sort BOTH route to the hybrid pipeline (unchanged behavior):
    // the provider id is the lexical-fallback hybrid id, NOT "date-ordered".
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/mlx", Some("mlx"), 1);
    for sort in [None, Some("relevance".to_string()), Some("garbage".to_string())] {
        let response = runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: "mlx".to_string(),
                    limit: Some(5),
                    sort: sort.clone(),
                    ..AiSearchRequest::default()
                },
            ))
            .expect("relevance-path search");
        assert_ne!(
            response.provider_id, "date-ordered",
            "sort={sort:?} must keep the hybrid/relevance path, not the date path"
        );
    }
}

/// Bulk-inserts `count` keyword-matching visits on DISTINCT, strictly-ascending timestamps in ONE
/// transaction, mirroring `seed_visit`'s exact `runs`/`source_profiles`/`urls`/`visits` writes.
///
/// Takes the `prepared_archive` connection (the intelligence connection with the archive ATTACHed as
/// `archive`, exactly the connection `seed_visit` writes through), so the `archive.` prefixes resolve.
///
/// Why a bespoke bulk insert: test #1 needs 1001 matchable rows to exceed the date-ordered window cap,
/// and 1001 individual `seed_visit_on_date` calls (each its own auto-committed statement) is too slow.
/// A single transaction with prepared statements keeps it well under a second while producing the EXACT
/// same on-disk shape `seed_visit` does — so `open_archive_connection`'s `seed_search_projection_if_missing`
/// rebuilds the FTS projection from these `urls` on the first `list_history` read and the keyword matches.
/// `history_id` is the visit id AND the url id (as `seed_visit` does), and `visit_time` increases with
/// `history_id` so the EARLIEST-dated visit is `first_id` and the latest is `first_id + count - 1`.
fn seed_keyword_visits_bulk(connection: &Connection, keyword: &str, first_id: i64, count: i64) {
    let profile_id = "chrome:Default";
    let browser_kind = "chrome";
    let profile_row_id = profile_id.bytes().fold(0_i64, |acc, value| acc + value as i64).max(1);
    // Base timestamp far enough in the past that +count seconds stays comfortably valid; the unit is
    // Unix microseconds fed to `unix_micros_to_chrome_time`, matching `seed_visit`'s expected format.
    let base_unix_ms: i64 = 1_700_000_000_000; // 2023-11-14, well-formed and distinct per row.

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

    connection.execute_batch("BEGIN").expect("begin bulk seed");
    {
        let mut url_stmt = connection
            .prepare(
                "INSERT OR IGNORE INTO archive.urls
                 (id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at)
                 VALUES (?1, ?2, ?3, 1, 0, ?4, ?5, ?4, ?5, ?6, 1, ?1, 0, ?7, ?8)",
            )
            .expect("prepare url insert");
        let mut visit_stmt = connection
            .prepare(
                "INSERT INTO archive.visits
                 (id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms, source_profile_id, created_by_run_id, from_visit, is_known_to_sync, visited_link_id, external_referrer_url, app_id, event_fingerprint, payload_hash, recorded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 805306368, 0, ?6, 1, NULL, 1, 0, NULL, NULL, ?7, ?8, ?9)",
            )
            .expect("prepare visit insert");
        let now = now_rfc3339();
        for offset in 0..count {
            let history_id = first_id + offset;
            // Strictly ascending so the EARLIEST-dated row is `first_id` (1 second apart per row).
            let visit_unix_ms = base_unix_ms + offset * 1000;
            let chrome_time = crate::utils::unix_micros_to_chrome_time(visit_unix_ms * 1000);
            let visit_time_ms = chrome_time_to_unix_ms(chrome_time);
            let visit_time_iso = crate::utils::chrome_time_to_rfc3339(chrome_time);
            // Put the keyword in BOTH url path and title so the FTS projection matches it.
            let url = format!("https://bulk.example/{keyword}/{history_id}");
            let title = format!("{keyword} entry {history_id}");
            url_stmt
                .execute(params![
                    history_id,
                    url,
                    title,
                    visit_time_ms,
                    visit_time_iso,
                    profile_row_id,
                    format!("payload-{history_id}"),
                    now,
                ])
                .expect("bulk insert url");
            visit_stmt
                .execute(params![
                    history_id,
                    history_id,
                    history_id.to_string(),
                    visit_time_ms,
                    visit_time_iso,
                    profile_row_id,
                    format!("fp-{history_id}"),
                    format!("payload-{history_id}"),
                    now,
                ])
                .expect("bulk insert visit");
        }
    }
    connection.execute_batch("COMMIT").expect("commit bulk seed");
}

#[test]
fn date_ordered_pagination_caps_honestly_at_the_window_without_looping() {
    // THE BLOCKER regression test. The fixed contract: the date-ordered path reports the TRUE uncapped
    // match count as `total`, stops the cursor at the 1000-row retrievable window (so the agent never
    // loops on empty pages past the cap), and keeps `has_more == true` with NO `next_cursor` plus a
    // "retrieval cap" note when matches exist beyond the window. The pre-fix code set `total =
    // ordered.len()` (capped at 1000) and could hand back a cursor past the window — reverting either
    // makes the assertions below fail.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    // 1001 matching visits on distinct ascending timestamps — ONE more than the 1000-row window cap.
    seed_keyword_visits_bulk(&connection, "mlx", 1, 1001);

    let page = |cursor: Option<String>, limit: u32| {
        runtime
            .block_on(search_history_internal(
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: "mlx".to_string(),
                    limit: Some(limit),
                    sort: Some("oldest".to_string()),
                    cursor,
                    ..AiSearchRequest::default()
                },
            ))
            .expect("a date-ordered page")
    };

    // THE LETHAL ASSERTION: `total` is the TRUE uncapped match count (1001), NOT the 1000-row pool size.
    let first = page(None, 10);
    assert_eq!(
        first.total, 1001,
        "total must be the TRUE uncapped match count (1001), not the 1000-row window cap: got {}",
        first.total
    );
    assert_eq!(first.provider_id, "date-ordered");
    // sort=oldest correctness at scale: the first page's first row is the EARLIEST-dated seeded visit.
    assert_eq!(
        first.items[0].history_id, 1,
        "sort:oldest must lead with the earliest-dated visit (history_id 1): got {}",
        first.items[0].history_id
    );

    // Walk the cursor up to the window edge with a small page size and assert no overlap + termination.
    let limit = 10_u32;
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut cursor: Option<String> = first.next_cursor.clone();
    for item in &first.items {
        assert!(seen.insert(item.history_id), "first page must not repeat ids");
    }
    let mut reached_edge = false;
    let mut iterations = 0;
    loop {
        iterations += 1;
        assert!(iterations <= 200, "pagination must terminate (no infinite loop)");
        let Some(next) = cursor.clone() else { break };
        let response = page(Some(next), limit);
        for item in &response.items {
            assert!(
                seen.insert(item.history_id),
                "paging must never re-yield an already-seen id (no overlap): {}",
                item.history_id
            );
        }
        // At/near the window edge (offset 990 → 1000) the cursor must stop with `has_more` still true
        // and a "retrieval cap" note, NEVER handing back a cursor that pages past the 1000-row clamp.
        match response.next_cursor.clone() {
            Some(next) => {
                let offset: usize = next.parse().expect("numeric cursor offset");
                assert!(
                    offset <= 1000,
                    "the cursor must never advance past the 1000-row window: {offset}"
                );
                cursor = Some(next);
            }
            None => {
                // The window edge: more matches exist (1001 > 1000) but the cursor stops to avoid an
                // infinite loop on empty pages beyond the cap.
                reached_edge = true;
                assert!(
                    response.has_more,
                    "at the window edge has_more must STAY true (matches exist beyond the cap)"
                );
                assert!(
                    response.notes.iter().any(|note| note.contains("retrieval cap")),
                    "the window-edge page must carry a 'retrieval cap' note: {:?}",
                    response.notes
                );
                break;
            }
        }
    }
    assert!(
        reached_edge,
        "the walk must reach the window edge and stop (cursor None) without looping"
    );
    // The retrievable window is exactly the 1000-row cap; the 1001st match is unreachable by paging.
    assert_eq!(
        seen.len(),
        1000,
        "paging covers exactly the 1000-row retrievable window: {}",
        seen.len()
    );
}

#[test]
fn sort_oldest_with_date_range_returns_first_in_that_range() {
    // The date sort composes with the date filter: with the keyword present in 2024, 2025, and 2026 and
    // a [2025-01-01, 2025-12-31] range, `sort:"oldest"` returns the 2025 row first — the 2024 match is
    // excluded below the range and the 2026 match is excluded above it. Without the date predicate the
    // 2024 row would lead; without the oldest sort the order would not be chronological.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    seed_visit_on_date(&connection, 1, "https://a.example/mlx", Some("mlx 2024"), "2024-05-10");
    seed_visit_on_date(&connection, 2, "https://b.example/mlx", Some("mlx 2025"), "2025-07-15");
    seed_visit_on_date(&connection, 3, "https://c.example/mlx", Some("mlx 2026"), "2026-02-20");

    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "mlx".to_string(),
                limit: Some(10),
                sort: Some("oldest".to_string()),
                start_date: Some("2025-01-01".to_string()),
                end_date: Some("2025-12-31".to_string()),
                ..AiSearchRequest::default()
            },
        ))
        .expect("oldest + date-range search");
    let ids: Vec<i64> = response.items.iter().map(|item| item.history_id).collect();
    assert_eq!(
        ids,
        vec![2],
        "only the in-range 2025 match returns (2024 below, 2026 above the range): {ids:?}"
    );
    assert_eq!(response.provider_id, "date-ordered");
}

#[test]
fn sort_oldest_with_starred_only_returns_oldest_starred_match() {
    // The `is:starred` facet composes with the date sort: with several keyword matches across dates and
    // only a SUBSET starred, `sort:"oldest"` + `starred_only` returns ONLY the starred matches, oldest
    // first, and `total` reflects the IN-POOL starred count (not the pre-filter count of all matches).
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    // Five matches; star pages 2 and 4 (so the starred set, oldest-first, is [2024-06 → id2, 2025-09 → id4]).
    seed_visit_on_date(&connection, 1, "https://one.example/mlx", Some("mlx one"), "2024-01-05");
    seed_visit_on_date(&connection, 2, "https://two.example/mlx", Some("mlx two"), "2024-06-15");
    seed_visit_on_date(
        &connection,
        3,
        "https://three.example/mlx",
        Some("mlx three"),
        "2025-03-20",
    );
    seed_visit_on_date(&connection, 4, "https://four.example/mlx", Some("mlx four"), "2025-09-10");
    seed_visit_on_date(&connection, 5, "https://five.example/mlx", Some("mlx five"), "2026-04-01");
    star_url(&paths, &config, "https://two.example/mlx");
    star_url(&paths, &config, "https://four.example/mlx");

    let response = runtime
        .block_on(search_history_internal(
            &paths,
            &config,
            None,
            None,
            &AiSearchRequest {
                query: "mlx".to_string(),
                limit: Some(10),
                sort: Some("oldest".to_string()),
                starred_only: Some(true),
                ..AiSearchRequest::default()
            },
        ))
        .expect("oldest + starred-only search");
    let ids: Vec<i64> = response.items.iter().map(|item| item.history_id).collect();
    assert_eq!(
        ids,
        vec![2, 4],
        "only the starred matches return, oldest first (id2 2024-06 then id4 2025-09): {ids:?}"
    );
    assert_eq!(
        response.total, 2,
        "total reflects the in-pool starred count (2), not the 5 pre-filter matches: got {}",
        response.total
    );
    assert_eq!(response.provider_id, "date-ordered");
}
