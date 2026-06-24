//! AI provider validation, readiness, and embedding/LLM runtime bridges.
//!
//! ## Responsibilities
//! - validate configured providers before semantic or assistant work starts
//! - classify provider errors into actionable user-facing categories
//! - run lightweight health probes against embedding and LLM providers
//! - bridge request-format-specific embedding and chat calls into rig.rs clients
//!
//! ## Not responsible for
//! - semantic index ledger persistence or archive run bookkeeping
//! - assistant retrieval composition or semantic search result ranking
//! - persisted read models such as `AiIndexStatus` or assistant run history
//!
//! ## Dependencies
//! - `super::read_model::provider_capabilities` for capability reporting
//! - shared provider/runtime types and rig.rs imports from the parent `ai` module
//!
//! ## Performance notes
//! - embedding helpers batch requests where possible and retry only a bounded number
//!   of times
//! - rate-limited failures do not spin through retries, which avoids making quota
//!   pressure worse on a constrained host or provider account

use super::llm::rig_llm_capabilities;
use super::read_model::provider_capabilities;
use super::traits::{EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling, EmbeddingRole};
use super::*;
use crate::models::LlmProviderCapabilityReport;

/// Test-only synthetic dimension used by the deterministic embedding stubs.
///
/// Coverage builds never reach a live provider, so the stub must pick a length when the
/// config does not request a specific dim. This is an arbitrary small number with NO
/// relationship to any real model's native dimension — picking a real dim (1536/768) here
/// is exactly the D4 truth-assumption this block removes.
#[cfg(any(test, coverage))]
const STUB_SYNTHETIC_DIM: usize = 8;

/// L2-normalizes a vector in place, guarding against degenerate (zero / non-finite) inputs.
///
/// Returned vectors are normalized defensively (02 §C.3 rule b): MRL truncation and some
/// providers leave vectors un-normalized, which silently corrupts cosine similarity. The norm
/// and the per-component scale are computed in f64 (so an f32-overflowing magnitude does not
/// saturate the reciprocal to `inf`), and a zero or non-finite norm leaves the vector exactly
/// as-is rather than writing NaNs/infs.
pub(super) fn l2_normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| f64::from(*value) * f64::from(*value)).sum::<f64>().sqrt();
    if !norm.is_finite() || norm <= 0.0 {
        return;
    }
    let inv = 1.0 / norm;
    for value in vector.iter_mut() {
        *value = (f64::from(*value) * inv) as f32;
    }
}

/// Rewrites a `localhost` host label to `127.0.0.1` in a provider base URL, leaving everything
/// else byte-for-byte.
///
/// ## Why this exists
/// reqwest on macOS resolves `localhost` to the dual-stack `[::1]`/`127.0.0.1` set and, against a
/// server that binds IPv4 ONLY (LM Studio's default `*:1234`, where `[::1]:1234` is
/// connection-refused), returns a spurious 503 even though `127.0.0.1:1234` streams perfectly.
/// `curl` masks the bug because it prefers the IPv4 address. Every LM Studio / Ollama preset ships
/// a `localhost` base URL, so pinning the connection to `127.0.0.1` at the URL-construction layer
/// fixes EXISTING saved configs without anyone re-typing the field, and without touching the
/// transport (rig/reqwest stay as-is — this is purely an address-selection fix).
///
/// ## Exact-label contract
/// Only the bare host token `localhost` is rewritten, matched case-insensitively (`LOCALHOST`),
/// with or without a scheme (`http://localhost:1234/v1`, a bare `localhost:1234`). Scheme,
/// userinfo, port, path, query, and fragment are preserved verbatim. A host that merely CONTAINS
/// the substring (`localhost.example.com`, `mylocalhost`) is left untouched — it is a different,
/// real host. IPv6 literals (`[::1]`), already-numeric hosts (`127.0.0.1`), cloud hosts, and the
/// candle in-app sentinel (`candle:in-app`, which has no `://` and whose "host" token is `candle`)
/// all fall through unchanged. Pure and total: any input that does not present a `localhost` host
/// label is returned as an owned copy of the original.
pub(super) fn normalize_local_base_url(base_url: &str) -> String {
    // Locate the authority. With a scheme, the authority begins right after `://`; without one
    // (a bare `localhost:1234`), the whole string is authority-then-path. We never invent a
    // scheme — a scheme-less input is returned scheme-less.
    let authority_start = match base_url.find("://") {
        Some(scheme_end) => scheme_end + 3,
        None => 0,
    };
    let rest = &base_url[authority_start..];

    // The authority ends at the first path/query/fragment delimiter.
    let authority_len = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_len];

    // Drop any `userinfo@` prefix; the host begins after the LAST `@` in the authority.
    let host_offset = authority.rfind('@').map_or(0, |at| at + 1);
    let host_and_port = &authority[host_offset..];

    // An IPv6 literal is bracketed (`[::1]`) and never equals the `localhost` label, so skip it
    // wholesale — splitting on `:` would otherwise mangle the inner colons.
    if host_and_port.starts_with('[') {
        return base_url.to_string();
    }

    // The host is everything up to the `:port` separator (if any).
    let host_len = host_and_port.find(':').unwrap_or(host_and_port.len());
    let host = &host_and_port[..host_len];

    if !host.eq_ignore_ascii_case("localhost") {
        return base_url.to_string();
    }

    // Splice `127.0.0.1` in place of exactly the host span, preserving every surrounding byte.
    let host_start = authority_start + host_offset;
    let host_end = host_start + host_len;
    let mut out = String::with_capacity(base_url.len() + 4);
    out.push_str(&base_url[..host_start]);
    out.push_str("127.0.0.1");
    out.push_str(&base_url[host_end..]);
    out
}

/// Whether the transport should request the model's native embedding dimension, or a
/// specific MRL-truncated dimension the user explicitly configured.
///
/// `None` = ask for the model's native dimension (no PathKeep-imposed size). `Some(n)` = the
/// user configured an explicit dimension, passed to the transport only as an MRL request hint.
type RequestedEmbeddingDim = Option<usize>;

/// Resolves the dimension to REQUEST from the transport without imposing a hidden model
/// assumption (D4), or errors if the provider cannot embed without one.
///
/// Per-format reality (rig 0.34, verified against the vendored adapter source):
/// - OpenAI / Ollama / LM Studio: `embedding_model(model)` omits the `dimensions` field when no
///   dim is configured, so the server returns the model's native size → `None` is safe.
/// - Gemini (`Google`): the rig adapter resolves an unrecognized model to `unwrap_or(768)` and
///   ALWAYS sends `output_dimensionality`, so it cannot request a model's native size. Letting
///   that through would silently force 768 — exactly the D4 truth-assumption this layer removes.
///   We therefore REQUIRE an explicit dimension for Gemini embedding rather than accept a hidden
///   default.
/// - Anthropic: no embedding API.
pub(super) fn resolve_embed_request_dim(
    provider: &AiProviderRuntime,
) -> Result<RequestedEmbeddingDim> {
    match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            Ok(provider.config.dimensions.map(|dim| dim as usize))
        }
        AiRequestFormat::Google => match provider.config.dimensions {
            Some(dim) => Ok(Some(dim as usize)),
            None => anyhow::bail!(
                "Set an explicit embedding dimension for Gemini provider {}: the Gemini transport cannot request the model's native dimension and would otherwise silently impose a default size.",
                provider.config.name
            ),
        },
        AiRequestFormat::Anthropic => {
            anyhow::bail!("Anthropic request format does not support embeddings in rig.rs.")
        }
    }
}

/// Builds the runtime descriptor for an embedding provider once `effective_dim` is known.
///
/// `effective_dim` must be the actual returned vector length (never config `dimensions`),
/// so the fingerprint downstream reflects reality rather than a requested hint.
pub(super) fn embedding_descriptor_for(
    provider: &AiProviderRuntime,
    effective_dim: Option<usize>,
) -> EmbeddingDescriptor {
    EmbeddingDescriptor {
        provider_id: provider.config.id.clone(),
        model_id: provider.config.default_model.clone(),
        effective_dim,
        // TODO(W-AI-4): these two are transport-defaults valid ONLY for the current float32 +
        // L2-normalized rig path. Per-provider adapters (candle, Voyage/Cohere int8) MUST set
        // dtype/normalized from the real transport before any fingerprint built from this
        // descriptor is persisted — otherwise two distinct encodings could share a fingerprint
        // (A-S2). `EmbeddingFingerprint::from_descriptor` has no production caller until then.
        dtype: EmbeddingDtype::Float32,
        // Vectors leave this layer L2-normalized via `l2_normalize`.
        normalized: true,
        // rig's hosted `/v1/embeddings` path does not expose pooling; it stays unknown until
        // the in-app candle engine (which owns its pooling) lands.
        pooling: EmbeddingPooling::Unknown,
        instruction_template: None,
    }
}

/// Captures whether the selected embedding provider is usable right now.
///
/// `AiIndexStatus` needs more than a boolean. It must explain whether the provider is
/// missing, disabled, missing a secret, or missing a model so the UI can tell the user
/// exactly what to fix before semantic retrieval can work.
#[derive(Debug, Clone, Default)]
pub(super) struct ProviderReadiness {
    pub available: bool,
    pub warning: Option<String>,
    /// Stable index-health warning CODE for the unavailable reason (review-fix M-7). `Some` whenever
    /// `available` is false; the read model carries it onto `AiIndexStatus.warning_code` so the FE
    /// localizes the exact missing prerequisite (provider missing / disabled / no key / no model)
    /// instead of matching the English `warning` sentence.
    pub warning_code: Option<AiIndexWarning>,
    pub selected_model: Option<String>,
}

/// Executes a lightweight health probe against one configured AI provider.
///
/// This keeps provider testing separate from the heavier queue/index/assistant flows,
/// so Settings can answer "can I reach this provider?" without enqueuing durable work.
pub async fn test_provider_connection(
    provider: &AiProviderRuntime,
) -> Result<AiProviderConnectionTestReport> {
    validate_provider(provider, provider.config.purpose.clone())?;
    let capabilities = provider_capabilities(&provider.config);
    let started = Instant::now();
    let probe_result = match provider.config.purpose {
        AiProviderPurpose::Embedding => {
            embed_query(provider, "PathKeep provider health check", EmbeddingRole::Query).await.map(
                |vector| {
                    // The effective dim is the actual returned length (D4), captured in the
                    // runtime descriptor rather than read back from config `dimensions`.
                    let descriptor = embedding_descriptor_for(provider, Some(vector.len()));
                    format!(
                        "Generated a {}-dimension probe embedding successfully.",
                        descriptor.effective_dim.unwrap_or_default()
                    )
                },
            )
        }
        AiProviderPurpose::Llm => run_llm_agent(
            provider,
            "You are a PathKeep connection test. Reply with a short OK message.",
            Vec::new(),
            "Reply with OK.",
        )
        .await
        .map(|_| "Provider completed a short chat probe successfully.".to_string()),
    };
    let latency_ms = (started.elapsed().as_millis() as u64).max(1);
    provider_connection_report_from_probe(provider, capabilities, latency_ms, probe_result)
}

/// Derives the serialized LLM capability detail for the probe report.
///
/// Only LLM providers get one (embedding providers have no chat surface). The detail mirrors
/// the in-engine [`LlmCapabilities`] the streaming transport exposes, so Settings can show the
/// exact same streaming/tool/structured/cache facts the chat path will rely on.
pub(super) fn llm_capability_report_for(
    provider: &AiProviderRuntime,
) -> Option<LlmProviderCapabilityReport> {
    if provider.config.purpose != AiProviderPurpose::Llm {
        return None;
    }
    let caps = rig_llm_capabilities(&provider.config);
    Some(LlmProviderCapabilityReport {
        tool_call: caps.tool_call,
        structured_output: caps.structured_output,
        streaming: caps.streaming,
        prompt_cache: caps.prompt_cache,
        max_context_tokens: caps.max_context_tokens,
    })
}

pub(super) fn provider_connection_report_from_probe(
    provider: &AiProviderRuntime,
    capabilities: AiProviderCapabilityReport,
    latency_ms: u64,
    probe_result: Result<String>,
) -> Result<AiProviderConnectionTestReport> {
    let llm_capabilities = llm_capability_report_for(provider);
    match probe_result {
        Ok(message) => Ok(AiProviderConnectionTestReport {
            provider_id: provider.config.id.clone(),
            purpose: match provider.config.purpose {
                AiProviderPurpose::Embedding => "embedding".to_string(),
                AiProviderPurpose::Llm => "llm".to_string(),
            },
            model: provider.config.default_model.clone(),
            ok: true,
            latency_ms,
            capabilities,
            llm_capabilities,
            warnings: if provider.config.request_format == AiRequestFormat::Anthropic
                && provider.config.purpose == AiProviderPurpose::Llm
            {
                vec!["Anthropic remains day-one chat-only in the current rig.rs integration; embedding selection should use a separate provider.".to_string()]
            } else {
                Vec::new()
            },
            message,
            ..AiProviderConnectionTestReport::default()
        }),
        Err(error) => {
            let (error_code, action_hint, retry_hint) = classify_provider_error(&error.to_string());
            Ok(AiProviderConnectionTestReport {
                provider_id: provider.config.id.clone(),
                purpose: match provider.config.purpose {
                    AiProviderPurpose::Embedding => "embedding".to_string(),
                    AiProviderPurpose::Llm => "llm".to_string(),
                },
                model: provider.config.default_model.clone(),
                ok: false,
                latency_ms,
                capabilities,
                llm_capabilities,
                error_code,
                action_hint,
                retry_hint,
                warnings: Vec::new(),
                message: error.to_string(),
            })
        }
    }
}

/// Validates that one provider can legally serve the requested PathKeep capability.
///
/// The product keeps provider misconfiguration explicit. Failing here is preferable to
/// starting a long-lived queue run that was doomed by a disabled provider, missing model,
/// or unsupported request format.
pub(super) fn validate_provider(
    provider: &AiProviderRuntime,
    expected_purpose: AiProviderPurpose,
) -> Result<()> {
    if !provider.config.enabled {
        anyhow::bail!("Enable provider {} before using it.", provider.config.name)
    }
    if provider.config.purpose != expected_purpose {
        anyhow::bail!(
            "Provider {} is configured for {:?}, not {:?}.",
            provider.config.name,
            provider.config.purpose,
            expected_purpose
        )
    }
    if provider.config.default_model.trim().is_empty() {
        anyhow::bail!("Select a default model for provider {}.", provider.config.name)
    }
    if matches!(
        (provider.config.purpose.clone(), provider.config.request_format.clone()),
        (AiProviderPurpose::Embedding, AiRequestFormat::Anthropic)
    ) {
        anyhow::bail!("Anthropic request format is not available for embeddings in rig.rs.")
    }
    Ok(())
}

/// Maps raw provider failures into the stable action/retry hints shown in the UI.
///
/// Settings and worker surfaces need consistent failure language regardless of which
/// provider produced the original transport/model error.
pub(super) fn classify_provider_error(
    message: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let normalized = message.to_lowercase();
    if normalized.contains("enable provider") {
        return (
            Some("provider-disabled".to_string()),
            Some("Enable the provider in Settings before testing it again.".to_string()),
            None,
        );
    }
    if normalized.contains("api key")
        || normalized.contains("store an api key")
        || normalized.contains("unauthorized")
        || normalized.contains("forbidden")
    {
        return (
            Some("secret-missing".to_string()),
            Some("Store a valid API key in the native keyring for this provider.".to_string()),
            Some("After updating the key, run Test connection again.".to_string()),
        );
    }
    if normalized.contains("rate limit")
        || normalized.contains("quota")
        || normalized.contains("429")
    {
        return (
            Some("rate-limited".to_string()),
            Some(
                "Wait for the provider quota window to reset or reduce this model's usage."
                    .to_string(),
            ),
            Some("Retry after the provider cooldown ends.".to_string()),
        );
    }
    if normalized.contains("does not support embeddings")
        || normalized.contains("not configured for")
    {
        return (
            Some("unsupported-capability".to_string()),
            Some("Pick a provider whose day-one capabilities match this purpose.".to_string()),
            None,
        );
    }
    if normalized.contains("model") && normalized.contains("not found") {
        return (
            Some("bad-model".to_string()),
            Some("Select a valid default model for this provider.".to_string()),
            Some("Save the model selection and test again.".to_string()),
        );
    }
    if normalized.contains("timed out")
        || normalized.contains("dns")
        || normalized.contains("refused")
        || normalized.contains("network")
    {
        return (
            Some("network-error".to_string()),
            Some(
                "Check the base URL, local daemon, or network path for this provider.".to_string(),
            ),
            Some("Retry after the endpoint is reachable.".to_string()),
        );
    }
    (
        Some("provider-error".to_string()),
        None,
        Some("Review the provider error and retry after fixing it.".to_string()),
    )
}

/// Resolves whether the configured embedding provider is actually ready for semantic retrieval.
///
/// Semantic search should degrade honestly before any build/search call starts. This
/// helper lets the status read model explain the exact missing prerequisite.
pub(super) fn embedding_provider_readiness(config: &AppConfig) -> ProviderReadiness {
    // Build each unavailable readiness from its stable warning CODE (review-fix M-7), deriving the
    // legacy English `warning` from the same code so the wire carries a localizable code and the
    // FE never falls back to English-sentence matching.
    fn unavailable(code: AiIndexWarning, selected_model: Option<String>) -> ProviderReadiness {
        ProviderReadiness {
            available: false,
            warning: Some(super::read_model::ai_index_warning_text(&code)),
            warning_code: Some(code),
            selected_model,
        }
    }

    let Some(provider_id) = config.ai.embedding_provider_id.as_deref() else {
        return unavailable(AiIndexWarning::NoEmbeddingProvider, None);
    };
    let Some(provider) =
        config.ai.embedding_providers.iter().find(|provider| provider.id == provider_id)
    else {
        return unavailable(
            AiIndexWarning::EmbeddingProviderMissing { provider_id: provider_id.to_string() },
            None,
        );
    };
    if !provider.enabled {
        return unavailable(
            AiIndexWarning::EmbeddingProviderDisabled { provider_name: provider.name.clone() },
            Some(provider.default_model.clone()),
        );
    }
    // OPTIONAL key: a missing API key does NOT make the embedding provider unavailable. A
    // local/LAN endpoint (LM Studio / Ollama) needs none, so pre-empting the index build on a
    // missing key here would re-introduce the very block this fix removes. If a cloud provider
    // truly needs a key, the build/probe surfaces its OWN 401 instead. The
    // `EmbeddingProviderNoApiKey` warning code is retained for the read-model/FE localization
    // contract but is no longer emitted as a readiness blocker.
    if provider.default_model.trim().is_empty() {
        return unavailable(
            AiIndexWarning::EmbeddingProviderNoModel { provider_name: provider.name.clone() },
            None,
        );
    }
    ProviderReadiness {
        available: true,
        warning: None,
        warning_code: None,
        selected_model: Some(provider.default_model.clone()),
    }
}

/// Runs one LLM prompt through the configured provider request format.
///
/// Assistant runs centralize model dispatch here so request-format-specific rig.rs wiring
/// does not leak into retrieval or run-ledger code.
#[cfg(not(any(test, coverage)))]
pub(super) async fn run_llm_agent(
    provider: &AiProviderRuntime,
    preamble: &str,
    tools: Vec<Box<dyn ToolDyn>>,
    question: &str,
) -> Result<String> {
    // OPTIONAL key (see `AiProviderRuntime::api_key_for_transport`): only a present, non-blank
    // secret is forwarded as the token; an absent key sends an empty one. rig 0.34 always emits an
    // auth header for these clients, but a keyless local endpoint (LM Studio / Ollama) ignores it
    // and a key-enforcing cloud server returns its own 401 — we never pre-empt the call ourselves.
    let api_key = provider.api_key_for_transport().unwrap_or_default();
    match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            let mut builder = openai::CompletionsClient::builder().api_key(api_key);
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(normalize_local_base_url(base_url));
            }
            let client = builder.build()?;
            let agent = client
                .agent(provider.config.default_model.clone())
                .preamble(preamble)
                .temperature(provider.config.temperature.unwrap_or(0.2) as f64)
                .max_tokens(provider.config.max_tokens.unwrap_or(1200).into())
                .tools(tools)
                .build();
            Ok(agent.prompt(question).await?)
        }
        AiRequestFormat::Anthropic => {
            let mut builder = anthropic::Client::builder().api_key(api_key);
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(normalize_local_base_url(base_url));
            }
            let client = builder.build()?;
            let agent = client
                .agent(provider.config.default_model.clone())
                .preamble(preamble)
                .temperature(provider.config.temperature.unwrap_or(0.2) as f64)
                .max_tokens(provider.config.max_tokens.unwrap_or(1200).into())
                .tools(tools)
                .build();
            Ok(agent.prompt(question).await?)
        }
        AiRequestFormat::Google => {
            let mut builder = gemini::Client::builder().api_key(api_key);
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(normalize_local_base_url(base_url));
            }
            let client = builder.build()?;
            let agent = client
                .agent(provider.config.default_model.clone())
                .preamble(preamble)
                .temperature(provider.config.temperature.unwrap_or(0.2) as f64)
                .max_tokens(provider.config.max_tokens.unwrap_or(1200).into())
                .tools(tools)
                .build();
            Ok(agent.prompt(question).await?)
        }
    }
}

/// Returns a deterministic stubbed LLM response for tests and coverage builds.
///
/// Tests need stable outputs without hitting a live provider. Keeping the stub here
/// preserves the same public call graph as production without adding network flakiness.
#[cfg(any(test, coverage))]
pub(super) async fn run_llm_agent(
    provider: &AiProviderRuntime,
    preamble: &str,
    tools: Vec<Box<dyn ToolDyn>>,
    question: &str,
) -> Result<String> {
    if provider.config.id.contains("llm-error") {
        anyhow::bail!("forced coverage LLM error");
    }
    let provider_label = match provider.config.request_format {
        AiRequestFormat::OpenAi => "openai",
        AiRequestFormat::Ollama => "ollama",
        AiRequestFormat::LmStudio => "lmstudio",
        AiRequestFormat::Anthropic => "anthropic",
        AiRequestFormat::Google => "google",
    };
    let preamble_summary =
        preamble.lines().next().unwrap_or_default().trim().chars().take(24).collect::<String>();
    Ok(format!(
        "{provider_label} stub answer to '{question}' with {} tools [{preamble_summary}]",
        tools.len()
    ))
}

/// Retries one embedding batch a bounded number of times before surfacing failure.
///
/// Batch retries amortize transient provider errors without exploding total request
/// volume. Rate-limit responses deliberately skip retries so PathKeep does not make
/// the provider situation worse.
#[cfg(test)]
pub(super) async fn embed_batch_with_retry(
    provider: &AiProviderRuntime,
    texts: &[String],
    role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    let mut attempts = 0usize;
    loop {
        match embed_text_batch(provider, texts, role).await {
            Ok(vectors) => return Ok(vectors),
            Err(error) if should_retry_embedding_error(&error, &mut attempts) => {}
            Err(error) => return Err(error),
        }
    }
}

/// Retries one single-text embedding request when the batch path fell back to per-row mode.
#[cfg(test)]
pub(super) async fn embed_single_with_retry(
    provider: &AiProviderRuntime,
    text: &str,
    role: EmbeddingRole,
) -> Result<Vec<f32>> {
    let mut attempts = 0usize;
    loop {
        match embed_query(provider, text, role).await {
            Ok(vector) => return Ok(vector),
            Err(error) if should_retry_embedding_error(&error, &mut attempts) => {}
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
pub(super) fn should_retry_embedding_error(error: &anyhow::Error, attempts: &mut usize) -> bool {
    if *attempts >= EMBEDDING_RETRY_ATTEMPTS {
        return false;
    }
    *attempts += 1;
    !embedding_error_is_rate_limited(error)
}

/// Detects whether an embedding failure should bypass retries and surface immediately.
#[cfg(test)]
pub(super) fn embedding_error_is_rate_limited(error: &anyhow::Error) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("rate limit") || message.contains("quota") || message.contains("429")
}

/// Returns deterministic embedding vectors for tests and coverage builds.
#[cfg(test)]
pub(super) async fn embed_text_batch(
    provider: &AiProviderRuntime,
    texts: &[String],
    role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    #[cfg(coverage)]
    {
        if provider.config.id.contains("batch-error") {
            anyhow::bail!("forced coverage batch embedding error");
        }
        if provider.config.id.contains("batch-short") {
            return Ok(Vec::new());
        }
    }

    let dimensions = stub_embedding_dimensions(provider)?;
    Ok(texts.iter().map(|text| stub_embedding_vector(provider, text, role, dimensions)).collect())
}

/// Executes one single-text embedding request against the configured provider.
///
/// D4 correctness rules (02 §C.3):
/// - the effective dimension is ALWAYS the actual returned vector length, never config
///   `dimensions`. The requested dimension is resolved by [`resolve_embed_request_dim`]:
///   `Some(n)` → an explicit MRL request hint via `embedding_model_with_ndims`; `None` → the
///   model's native dimension via `embedding_model(model)` (no PathKeep-imposed fallback). Note
///   `None` is only reachable for OpenAI-shaped transports — Gemini-without-an-explicit-dim is
///   rejected by the resolver because its adapter cannot request a native size.
/// - the returned vector is defensively L2-normalized.
/// - the `role` threads to the call (instruction injection arrives with per-provider
///   descriptors in W-AI-4; rig's hosted `/v1/embeddings` path has no role hook today).
#[cfg(not(any(test, coverage)))]
pub(super) async fn embed_query(
    provider: &AiProviderRuntime,
    query: &str,
    _role: EmbeddingRole,
) -> Result<Vec<f32>> {
    let requested_dim = resolve_embed_request_dim(provider)?;
    // OPTIONAL key (see `AiProviderRuntime::api_key_for_transport`): forward a present, non-blank
    // secret as the token; an absent key sends an empty one. rig 0.34's openai/gemini clients still
    // emit an auth header (upstream limitation, see `llm.rs` module note), but a keyless local model
    // ignores it. The DOMINANT embedding flow — the index backfill — runs through the reqwest
    // `ExternalEmbeddingProvider`, which genuinely omits the header when keyless. We keep rig here
    // (rather than rerouting) so an OpenAI provider WITHOUT an explicit base URL still resolves to
    // rig's hosted default instead of erroring on the external adapter's required base URL.
    let api_key = provider.api_key_for_transport().unwrap_or_default();
    match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            let mut builder = openai::Client::builder().api_key(api_key);
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(normalize_local_base_url(base_url));
            }
            let client = builder.build()?;
            let embedding = match requested_dim {
                Some(ndims) => {
                    client
                        .embedding_model_with_ndims(provider.config.default_model.clone(), ndims)
                        .embed_text(query)
                        .await?
                }
                None => {
                    client
                        .embedding_model(provider.config.default_model.clone())
                        .embed_text(query)
                        .await?
                }
            };
            Ok(finalize_embedding_vector(embedding.vec))
        }
        AiRequestFormat::Google => {
            let mut builder = gemini::Client::builder().api_key(api_key);
            if let Some(base_url) = provider.config.base_url.as_deref() {
                builder = builder.base_url(normalize_local_base_url(base_url));
            }
            let client = builder.build()?;
            // `resolve_embed_request_dim` guarantees an explicit dim for Gemini (it rejects the
            // native-dim request the adapter cannot honor), so this is always the hint path.
            let ndims = requested_dim.context(
                "internal: Gemini embedding reached the request path without an explicit dimension",
            )?;
            let embedding = client
                .embedding_model_with_ndims(provider.config.default_model.clone(), ndims)
                .embed_text(query)
                .await?;
            Ok(finalize_embedding_vector(embedding.vec))
        }
        AiRequestFormat::Anthropic => {
            anyhow::bail!("Anthropic request format does not support embeddings in rig.rs.")
        }
    }
}

/// Converts a rig embedding (f64) into PathKeep's f32 vector and L2-normalizes it.
///
/// The returned length is the truth for the effective dimension; callers must read
/// `vec.len()` rather than trusting config `dimensions`.
#[cfg(not(any(test, coverage)))]
fn finalize_embedding_vector(vec: Vec<f64>) -> Vec<f32> {
    let mut out: Vec<f32> = vec.into_iter().map(|value| value as f32).collect();
    l2_normalize(&mut out);
    out
}

/// Returns deterministic single-text embedding vectors for tests and coverage builds.
#[cfg(any(test, coverage))]
pub(super) async fn embed_query(
    provider: &AiProviderRuntime,
    query: &str,
    role: EmbeddingRole,
) -> Result<Vec<f32>> {
    #[cfg(coverage)]
    {
        if provider.config.id.contains("single-error") {
            anyhow::bail!("forced coverage single embedding error");
        }
    }

    let dimensions = stub_embedding_dimensions(provider)?;
    Ok(stub_embedding_vector(provider, query, role, dimensions))
}

/// Resolves the dimension the deterministic stub should emit WITHOUT assuming a real model.
///
/// Routes through the same [`resolve_embed_request_dim`] the production path uses, so the stub
/// enforces identical semantics (incl. the Gemini-requires-explicit-dim and Anthropic-no-embed
/// rules). A native-dim request (`None`) maps to [`STUB_SYNTHETIC_DIM`], a value with no
/// relation to any real model's native size.
#[cfg(any(test, coverage))]
pub(super) fn stub_embedding_dimensions(provider: &AiProviderRuntime) -> Result<usize> {
    Ok(resolve_embed_request_dim(provider)?.unwrap_or(STUB_SYNTHETIC_DIM))
}

/// Produces one deterministic, role-aware, L2-normalized stub embedding vector.
#[cfg(any(test, coverage))]
pub(super) fn stub_embedding_vector(
    provider: &AiProviderRuntime,
    text: &str,
    role: EmbeddingRole,
    dimensions: usize,
) -> Vec<f32> {
    let fingerprint = crate::utils::sha256_hex(
        format!("{}::{}::{text}", provider.config.id, role.as_str()).as_bytes(),
    );
    let bytes = fingerprint.as_bytes();
    let mut vector: Vec<f32> = (0..dimensions)
        .map(|index| ((bytes[index % bytes.len()] % 13) as f32 + 1.0) / 13.0)
        .collect();
    l2_normalize(&mut vector);
    vector
}
