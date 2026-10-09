//! Regression tests for optional AI and semantic retrieval helpers.
use super::indexing::{
    IndexedVisit, chunk_size, clear_provider_embeddings, collect_stale_history_ids,
    collect_visit_chunk, select_embed_targets, validate_embedding_batch_for_keys,
};
use super::provider::{
    embedding_descriptor_for, l2_normalize, normalize_local_base_url, resolve_embed_request_dim,
    should_retry_embedding_error, stub_embedding_dimensions, stub_embedding_vector,
};
use super::*;
// The match-reason CODE vocabulary the search pipeline stamps (see `ai::search`): these tests
// assert the wire CODES, never English prose, because the FE localizes from the code.
use super::search::{
    MATCH_REASON_LEXICAL, MATCH_REASON_LEXICAL_SEMANTIC, MATCH_REASON_RECENT_VISIT,
    MATCH_REASON_SEMANTIC, MATCH_REASON_STARRED_SUFFIX,
};
use super::search::{search_history_internal, semantic_matches};
use crate::utils::sha256_hex;
use crate::{
    ai_sidecar::SidecarEmbeddingRow,
    config::project_paths_with_root,
    models::{AiSettings, ArchiveMode},
};
use native::fixtures::{
    SeedVisit, prepared_archive, prepared_archive_with_key, seed_embedding, seed_enrichment_text,
    seed_extra_visit, seed_keyword_visits_bulk, seed_successful_index_ledger, seed_visit,
    seed_visit_on_date, seed_visits,
};
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
fn native_ai_schema_has_empty_metadata_totals() {
    for key in [None, Some([33; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        vault.read(vault_store::Db::Intelligence,|s| {
            assert_eq!(super::native_ledger::ai_embeddings_storage_bytes(s)?,0);
            assert_eq!(super::native_ledger::ai_embedding_token_estimate(s)?,0);
            let count=s.query_one("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='ai_embeddings'",(),|r|Ok(r.get::<i64>(0)?))?;
            assert_eq!(count,1);Ok(())
        }).unwrap();
    }
}
#[test]
fn native_ai_schema_keeps_vectors_out_of_metadata_rows() {
    let vault = crate::Vault::open_temp().unwrap();
    let sql = vault
        .read(vault_store::Db::Intelligence, |s| {
            Ok(s.query_one(
                "SELECT sql FROM sqlite_master WHERE type='table' AND name='ai_embeddings'",
                (),
                |r| Ok(r.get::<String>(0)?),
            )?)
        })
        .unwrap();
    let columns = sql
        .split_once('(')
        .unwrap()
        .1
        .split(',')
        .map(str::trim)
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();
    assert!(columns.contains(&"content_bytes"));
    for excluded in ["content", "embedding_blob", "embedding_json"] {
        assert!(!columns.contains(&excluded));
    }
}

#[test]
fn build_assistant_preamble_covers_empty_and_seeded_context() {
    let config = base_config();
    let empty = build_assistant_preamble(&config, &AiSearchResponse::default());
    assert!(empty.contains("No indexed evidence was found"));

    let with_context = build_assistant_preamble(
        &config,
        &AiSearchResponse {
            search_state: None,
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
                match_reason: MATCH_REASON_SEMANTIC.to_string(),
                enrichment_excerpt: None,
            }],
            notes: Vec::new(),
            note_codes: Vec::new(),
            next_cursor: None,
            applied_limit: None,
            has_more: false,
        },
    );
    assert!(with_context.contains(MATCH_REASON_SEMANTIC));
    assert!(with_context.contains("https://example.com/docs"));
}

#[test]
fn search_history_internal_empty_query_returns_recent_visits() {
    // BROWSE-BY-RECENCY: a blank query is no longer an error — it returns the most recent visits
    // (newest first), the entry point the agent needs to enumerate history / find the date range.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/old", Some("Old"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/new", Some("New"), 99);

    for blank in ["", "   "] {
        let response = runtime
            .block_on(search_history_internal(
                &vault,
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
                    browser_kind: None,
                },
            ))
            .expect("empty query returns recent visits");
        assert_eq!(response.total, 2, "both visits surface for `{blank:?}`");
        assert_eq!(response.provider_id, "recent-visits");
        // Blank search uses canonical chronology; both lexical scores are zero.
        assert_eq!(response.items[0].history_id, 2);
        assert_eq!(response.items[0].score, 0.0);
        assert_eq!(response.items[0].match_reason, MATCH_REASON_RECENT_VISIT);
        assert_eq!(response.items[1].history_id, 1, "older row ranks lower");
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/plain", Some("Plain"), 1);
    seed_visit(&connection, 2, "chrome:Default", "https://starred.com/page", Some("Star"), 2);
    star_url(&vault, "https://starred.com/page");

    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("starred-only recency");
    assert_eq!(response.total, 1, "only the starred page survives the facet");
    assert_eq!(response.items[0].url, "https://starred.com/page");
    // The recency path still applies the starred boost, so the code is the base recency code with
    // the `+starred` suffix appended — the exact composed value the FE resolves.
    assert_eq!(
        response.items[0].match_reason,
        format!("{MATCH_REASON_RECENT_VISIT}{MATCH_REASON_STARRED_SUFFIX}")
    );
}

#[test]
fn semantic_search_history_uses_public_wrapper_for_search_results() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let response = runtime
        .block_on(semantic_search_history(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    seed_keyword_visits_bulk(&connection, "mlx", 1, 120);

    let response = runtime
        .block_on(semantic_search_history(
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("public search wrapper");
    assert!(
        response.items.len() <= 50,
        "the FE wrapper caps returned rows at the model-facing cap even when 200 are requested: got {}",
        response.items.len()
    );
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    config.ai.assistant_enabled = false;
    let error = runtime
        .block_on(answer_history_question(
            &vault,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let control: Arc<dyn AiRunControl> = Arc::new(CountdownControl::new(1));

    let error = runtime
        .block_on(answer_history_question_with_control(
            &vault,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let response = runtime
        .block_on(answer_history_question(
            &vault,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/history", Some("History"), 1);
    let citations = Arc::new(Mutex::new(Vec::new()));
    let tool = SearchHistoryTool {
        context: SearchContext {
            vault: vault.clone(),
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
    assert_eq!(definition.parameters["properties"]["cursor"]["type"], "string");

    let output = runtime
        .block_on(rig::tool::Tool::call(
            &tool,
            SearchHistoryArgs {
                cursor: None,
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
                vault: vault.clone(),
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
                    cursor: None,
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
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let mut disabled = config.clone();
    disabled.ai.enabled = false;
    let disabled_status =
        ai_index_status(&vault.with_config(disabled.clone())).expect("disabled status");
    assert!(disabled_status.warning.is_none());

    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);

    let mut no_provider = config.clone();
    no_provider.ai.embedding_provider_id = None;
    let no_provider_status =
        ai_index_status(&vault.with_config(no_provider.clone())).expect("no provider");
    assert_eq!(no_provider_status.indexed_items, 0);
    assert!(no_provider_status.warning.is_some());

    let collected =
        collect_visits_to_index(&vault, &embedding_provider(), None).expect("collect all");
    assert_eq!(collected.len(), 1);
    seed_embedding(&connection, 1, &embedding_provider(), "metadata-only-hash");

    // No vector planes were ever built, so the flat index loads empty and search degrades to lexical
    // with an honest "no vectors yet" note (W-AI-5 replaced the old "tracked for v0.3.0" placeholder).
    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("semantic empty fallback");
    assert_eq!(response.provider_id, "embed");
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].match_reason, MATCH_REASON_LEXICAL);
    assert!(
        response.notes.iter().any(|note| note.contains("has no vectors yet")),
        "empty index must surface an honest note: {:?}",
        response.notes
    );
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // An embedding metadata row exists (the staleness reasons gate on it) but NO planes were built.
    seed_embedding(&connection, 1, &embedding, "metadata-only-hash");

    let report = runtime
        .block_on(semantic_matches(
            &vault,
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
                browser_kind: None,
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
}

#[test]
fn semantic_matches_returns_real_hits_and_surfaces_stale_ledger_note() {
    // Built planes → real two-stage recall returns the page whose stored vector matches the query
    // embedding; a stale ledger watermark still surfaces its rebuild note alongside the hits.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
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
    // Embedding metadata rows (the staleness reason gates on them) + a stale ledger watermark so the
    // staleness note is exercised alongside real hits.
    seed_embedding(&connection, 1, &embedding, "hash-1");
    seed_successful_index_ledger(&connection, &embedding, 1);

    let report = runtime
        .block_on(semantic_matches(
            &vault,
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
                browser_kind: None,
            },
            None,
        ))
        .expect("semantic matches with built planes");
    assert!(!report.hits.is_empty(), "a built index must return real hits");
    // Page 1 (the exact match) must rank first.
    assert_eq!(report.hits[0].history_id, 1);
    assert_eq!(report.hits[0].match_reason, MATCH_REASON_SEMANTIC);
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
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider(); // dimensions: Some(3) → query embeds at dim 3
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // Build the planes at dim 4 (a 4-component stored vector) — a different dim than the live query.
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, vec![0.5, 0.5, 0.5, 0.5])]);

    let report = runtime
        .block_on(semantic_matches(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
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
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // Enrichment text well over the 180-char budget → must come back capped + ellipsised.
    let long_summary = "Reusable workflow runner. ".repeat(20);
    seed_enrichment_text(&connection, 1, &long_summary);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, docs_vector)]);

    let report = runtime
        .block_on(semantic_matches(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    // No `seed_enrichment_text`, so the page's search document carries no enrichment text. The row
    // must still come back, just without an excerpt.

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, docs_vector)]);

    let report = runtime
        .block_on(semantic_matches(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    let earlier = 13_300_000_000_000_000_i64;
    let later = 13_400_000_000_000_000_i64;
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), earlier);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/docs", Some("Docs"), later);
    // The two visits are url_ids 1 and 2 on one page, and enrichment belongs to the page. Seeding it
    // through the older visit proves the surviving newer visit still carries the page's excerpt.
    seed_enrichment_text(&connection, 1, "Shared page summary");

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
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Work", "https://example.com/docs", Some("Docs"), 1);

    let docs_vector = runtime
        .block_on(embed_query(&embedding, "docs", EmbeddingRole::Query))
        .expect("docs vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, docs_vector)]);

    let report = runtime
        .block_on(semantic_matches(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    seed_embedding(&connection, 1, &embedding, "metadata-only-hash");

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("lexical fallback search");

    assert_eq!(search.items.len(), 1);
    assert_eq!(search.items[0].history_id, 1);
    assert_eq!(search.items[0].match_reason, MATCH_REASON_LEXICAL);
    assert!(search.notes.iter().any(|note| note.contains("has no vectors yet")));
}

#[test]
fn search_history_internal_merges_real_semantic_hits_with_lexical() {
    // With built planes, a page found by BOTH lexical and semantic recall reconciles on its visit
    // and is stamped the combined `lexical+semantic` code; a semantic-only page joins as a fresh hit.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("merged search");

    let page_one = search.items.iter().find(|item| item.history_id == 1).expect("page 1");
    assert_eq!(
        page_one.match_reason, MATCH_REASON_LEXICAL_SEMANTIC,
        "a dual-recall page reads as a combined match"
    );
    let page_two = search.items.iter().find(|item| item.history_id == 2).expect("page 2");
    assert_eq!(
        page_two.match_reason, MATCH_REASON_SEMANTIC,
        "a semantic-only page joins as a fresh hit"
    );
}

/// Stars a URL in the archive the AI test harness booted (`base_config` is plaintext).
fn star_url(vault: &crate::Vault, url: &str) {
    crate::stars::set_star(
        vault,
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
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("hybrid search");
    assert_eq!(
        search.items[0].history_id, 1,
        "the dual-list page wins on summed RRF contributions"
    );
    assert_eq!(search.items[0].match_reason, MATCH_REASON_LEXICAL_SEMANTIC);
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
    // hit, so page A is ONE `lexical+semantic` row that out-ranks a single-list page.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
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
        page_a.match_reason, MATCH_REASON_LEXICAL_SEMANTIC,
        "the fused multi-visit page earns the dual-list reason (and the RRF dual-list credit)"
    );
    // The dual-list page out-ranks the single-list page B.
    assert_eq!(search.items[0].url, "https://example.com/docs", "the dual-list page ranks first");
    let page_b = search.items.iter().find(|item| item.history_id == 3).expect("page B");
    assert_eq!(page_b.match_reason, MATCH_REASON_SEMANTIC, "page B is single-list (semantic only)");
    assert!(page_a.score > page_b.score, "the fused dual-list page out-ranks the single-list page");
}

#[test]
fn rrf_semantic_weight_zero_degrades_to_lexical_order() {
    // Tunable weights (W-AI-6): zeroing the semantic weight drops its RRF contribution, so ranking
    // collapses to the lexical order even though semantic recall still ran. Page 2 (semantic-only)
    // contributes nothing and falls to the bottom; page 1 (lexical) ranks first.
    let runtime = Runtime::new().expect("runtime");
    let (paths, mut config, connection) = prepared_archive();
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("page 1");
    assert_eq!(first.total, 2, "the fused pool is the lexical page ∪ the distinct semantic page");
    assert_eq!(first.items.len(), 1);
    let cursor = first.next_cursor.clone().expect("a next cursor exists");

    let second = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("lexical-only");
    assert_eq!(response.items.len(), 1);
    assert_eq!(response.items[0].match_reason, MATCH_REASON_LEXICAL);
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
    let vault = connection.vault.clone();
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
    star_url(&vault, "https://example.com/star-mid");
    star_url(&vault, "https://example.com/star-off");

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    // The starred pages carry the `+starred` code suffix the FE resolves to the favorite affordance;
    // the unstarred ones do not.
    let page_two = &search.items[rank_of(2)];
    assert!(
        page_two.match_reason.ends_with(MATCH_REASON_STARRED_SUFFIX),
        "a boosted result is marked Starred"
    );
    let page_three = &search.items[rank_of(3)];
    assert!(
        !page_three.match_reason.contains(MATCH_REASON_STARRED_SUFFIX),
        "an unstarred result is not marked"
    );
}

#[test]
fn starred_boost_off_leaves_favorites_unpromoted() {
    // With the boost set to 0 the starred status is inert: the identical pages 2 (starred) and 3
    // (unstarred) tie on fusion and neither carries the `+starred` code suffix.
    let runtime = Runtime::new().expect("runtime");
    let (paths, mut config, connection) = prepared_archive();
    let vault = connection.vault.clone();
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
    star_url(&vault, "https://example.com/star-mid");

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("boost-off search");
    assert!(
        search.items.iter().all(|item| !item.match_reason.contains(MATCH_REASON_STARRED_SUFFIX)),
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
    let vault = connection.vault.clone();
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
    star_url(&vault, "https://example.com/alpha");

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
    star_url(&vault, "https://example.com/alpha");

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/alpha", Some("Alpha"), 1);
    star_url(&vault, "https://example.com/alpha");
    // No `seed_vector_planes` → no `.pkvec`/`.pkmap`/sidecars at all.

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    let embedding = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/alpha", Some("Alpha"), 1);
    let needle = runtime
        .block_on(embed_query(&embedding, "needle", EmbeddingRole::Query))
        .expect("needle vector");
    seed_vector_planes(&paths, &embedding, &[(1, 0x1111, needle.clone())]);
    // Nothing is starred.

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/rust-a", Some("Rust A"), 2);
    seed_visit(&connection, 2, "chrome:Default", "https://example.com/rust-b", Some("Rust B"), 1);
    star_url(&vault, "https://example.com/rust-a");

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();

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
    star_url(&vault, "https://example.com/rust-old");

    let search = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
            },
        ))
        .expect("lexical-fallback search");
    assert_eq!(response.note_codes, vec![AiSearchNote::LexicalFallbackNoProvider]);
    // The English note is DERIVED from the code, in code order — never raw prose pushed ad hoc.
    assert_eq!(response.notes, vec![AiSearchNote::LexicalFallbackNoProvider.model_facing_text()]);
}

#[test]
fn search_history_internal_recency_path_excludes_out_of_range_visits() {
    // Test 1 (recency path) — an empty query + start_date == end_date returns ONLY the in-range visit;
    // the out-of-range visit (a different day) is excluded by the date filter threaded into the lexical
    // recency reader. A regression dropping the date predicate would return BOTH.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    seed_visit_on_date(&connection, 1, "https://in.example/page", Some("In"), "2026-06-19");
    seed_visit_on_date(&connection, 2, "https://out.example/page", Some("Out"), "2026-06-20");

    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
    seed_visit_on_date(&connection, 1, "https://in.example/docs", Some("Docs in"), "2026-06-19");
    seed_visit_on_date(&connection, 2, "https://out.example/docs", Some("Docs out"), "2026-06-25");

    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
        browser_kind: None,
    };

    let limited = runtime
        .block_on(search_history_internal(&vault, &paths, &config, None, None, &request(2)))
        .expect("limit 2 recency");
    assert_eq!(limited.items.len(), 2, "exactly `limit` items returned");
    assert!(limited.has_more, "3 visits with limit 2 → has_more must be true (H3)");
    assert_eq!(limited.applied_limit, Some(2));

    let unlimited = runtime
        .block_on(search_history_internal(&vault, &paths, &config, None, None, &request(5)))
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
    let vault = connection.vault.clone();
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
        browser_kind: None,
    };

    let limited = runtime
        .block_on(search_history_internal(&vault, &paths, &config, None, None, &request(2)))
        .expect("limit 2 keyword");
    assert_eq!(limited.items.len(), 2, "exactly `limit` items returned");
    assert!(limited.has_more, "3 keyword matches with limit 2 → has_more must be true (M1)");
    assert_eq!(limited.applied_limit, Some(2));

    let unlimited = runtime
        .block_on(search_history_internal(&vault, &paths, &config, None, None, &request(5)))
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
    let vault = connection.vault.clone();
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
            &vault,
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
    assert!(oldest.next_cursor.as_deref().is_some_and(|cursor| cursor.contains(':')));
}

#[test]
fn sort_newest_returns_the_latest_occurrence_across_years() {
    // The mirror of the oldest test: `sort:"newest", limit:1` returns the LATEST (2026-06) match.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
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
            &vault,
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
    let vault = connection.vault.clone();
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
                &vault,
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
    let vault = connection.vault.clone();
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
            &vault,
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
    let vault = connection.vault.clone();
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
            &vault,
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
    let vault = connection.vault.clone();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/mlx", Some("mlx"), 1);
    for sort in [None, Some("relevance".to_string()), Some("garbage".to_string())] {
        let response = runtime
            .block_on(search_history_internal(
                &vault,
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

#[test]
fn date_ordered_pagination_reaches_every_match_without_overlap() {
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    seed_keyword_visits_bulk(&connection, "mlx", 1, 1001);
    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let response = runtime
            .block_on(search_history_internal(
                &vault,
                &paths,
                &config,
                None,
                None,
                &AiSearchRequest {
                    query: "mlx".into(),
                    limit: Some(10),
                    sort: Some("oldest".into()),
                    cursor,
                    ..Default::default()
                },
            ))
            .unwrap();
        assert_eq!(response.total, 1001);
        assert!(!response.notes.iter().any(|note| note.contains("retrieval cap")));
        assert_eq!(response.has_more, response.next_cursor.is_some());
        ids.extend(response.items.into_iter().map(|item| item.history_id));
        assert!(ids.len() <= 1001, "paging repeated matches");
        cursor = response.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids, (1..=1001).collect::<Vec<_>>());
}

#[test]
fn sort_oldest_with_date_range_returns_first_in_that_range() {
    // The date sort composes with the date filter: with the keyword present in 2024, 2025, and 2026 and
    // a [2025-01-01, 2025-12-31] range, `sort:"oldest"` returns the 2025 row first — the 2024 match is
    // excluded below the range and the 2026 match is excluded above it. Without the date predicate the
    // 2024 row would lead; without the oldest sort the order would not be chronological.
    let runtime = Runtime::new().expect("runtime");
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    seed_visit_on_date(&connection, 1, "https://a.example/mlx", Some("mlx 2024"), "2024-05-10");
    seed_visit_on_date(&connection, 2, "https://b.example/mlx", Some("mlx 2025"), "2025-07-15");
    seed_visit_on_date(&connection, 3, "https://c.example/mlx", Some("mlx 2026"), "2026-02-20");

    let response = runtime
        .block_on(search_history_internal(
            &vault,
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
                browser_kind: None,
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
    let vault = connection.vault.clone();
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
    star_url(&vault, "https://two.example/mlx");
    star_url(&vault, "https://four.example/mlx");

    let response = runtime
        .block_on(search_history_internal(
            &vault,
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

#[test]
fn lexical_ai_cursors_reach_beyond_every_former_pool_and_date_window() {
    let (paths, config, connection) = prepared_archive_with_key(Some([94; 32]));
    let vault = connection.vault.clone();
    seed_visits(
        &vault,
        (1..=1107)
            .map(|id| {
                SeedVisit::chrome(
                    id,
                    "chrome:Default",
                    &format!("https://all.test/{id}"),
                    Some("completepool"),
                    13_300_000_000_000_000 + id * 1000,
                )
            })
            .collect(),
    );
    let runtime = Runtime::new().unwrap();
    for sort in [None, Some("oldest".to_owned()), Some("newest".to_owned())] {
        let mut request = AiSearchRequest {
            query: "completepool".into(),
            limit: Some(73),
            sort,
            ..Default::default()
        };
        let mut ids = Vec::new();
        loop {
            let response = runtime
                .block_on(search_history_internal(&vault, &paths, &config, None, None, &request))
                .unwrap();
            assert_eq!(response.total, 1107);
            ids.extend(response.items.iter().map(|item| item.history_id));
            if response.next_cursor.is_none() {
                break;
            }
            request.cursor = response.next_cursor;
        }
        ids.sort_unstable();
        assert_eq!(ids, (1..=1107).collect::<Vec<_>>());
    }
}

#[test]
fn semantic_facets_find_eligible_pages_beyond_every_former_recall_pool() {
    let (paths, config, connection) = prepared_archive_with_key(Some([108; 32]));
    let vault = connection.vault.clone();
    let provider = embedding_provider();
    let runtime = Runtime::new().unwrap();
    let vector =
        runtime.block_on(embed_query(&provider, "candidatephrase", EmbeddingRole::Query)).unwrap();
    let mut visits = Vec::new();
    let mut vectors = Vec::new();
    for id in 1..=1207 {
        visits.push(SeedVisit::chrome(
            id,
            if id == 1207 { "firefox:target" } else { "chrome:Default" },
            &format!("https://scope.test/page/{id}"),
            Some("Unrelated"),
            13_300_000_000_000_000 + id * 1000,
        ));
        vectors.push((
            id,
            id as u64,
            if id == 1207 { vector.iter().map(|value| -value).collect() } else { vector.clone() },
        ));
    }
    seed_visits(&vault, visits);
    seed_vector_planes(&paths, &provider, &vectors);
    let response = runtime
        .block_on(search_history_internal(
            &vault,
            &paths,
            &config,
            None,
            Some(&provider),
            &AiSearchRequest {
                query: "candidatephrase".into(),
                profile_id: Some("firefox:target".into()),
                limit: Some(1),
                ..Default::default()
            },
        ))
        .unwrap();
    assert_eq!(response.total, 1);
    assert_eq!(response.items[0].history_id, 1207);
}

#[path = "tests_native/mod.rs"]
pub(in crate::ai) mod native;
