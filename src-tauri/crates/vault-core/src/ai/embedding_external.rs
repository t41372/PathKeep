//! External OpenAI-compatible `/v1/embeddings` provider.
//!
//! ## Responsibilities
//! - own the ONE place reqwest/serde touch the OpenAI-compatible embeddings wire shape
//!   (covers LM Studio / Ollama / vLLM / llama-server / OpenAI / OpenRouter)
//! - implement the rig-free [`EmbeddingProvider`] boundary against that wire shape, honoring
//!   the 02 §C.3 correctness 鐵律: read the ACTUAL returned vector length as `effective_dim`,
//!   defensively L2-normalize every returned vector, and thread [`EmbeddingRole`] through
//!
//! ## Not responsible for
//! - resolving provider config or secrets (the worker's `AiProviderRuntime` carries both)
//! - vector persistence, indexing, queue bookkeeping (those live in `vector_store`/`indexing`)
//! - choosing between this and the in-app static engine (`embedding_engine`)
//!
//! ## Why this module exists
//! The `EmbeddingProvider` trait is vendor-free by contract (`traits.rs`). This adapter is the
//! only embedding file allowed to depend on reqwest, so the transport stays swappable. The real
//! HTTP path is `#[cfg(not(any(test, coverage)))]`; a deterministic same-signature stub backs
//! `#[cfg(any(test, coverage))]` (the established `provider.rs`/`llm.rs` pattern) so the 100%
//! coverage gate is met without a live server. The PURE helpers (request-body building,
//! response decoding, descriptor/dtype derivation, normalize) are un-gated and unit-tested so a
//! field/label swap is caught by the unit + mutation gates rather than shipping silently.

use super::provider::l2_normalize;
use super::traits::{
    EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling, EmbeddingProvider, EmbeddingRole,
};
use super::{AiProviderRuntime, AiRequestFormat};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[cfg(not(any(test, coverage)))]
use std::time::Duration;

/// Max inputs sent per `/v1/embeddings` HTTP request.
///
/// The OpenAI-compatible API accepts an array of inputs and returns one vector per input. A
/// bounded batch amortizes request overhead during the 14.4M backfill without building a body
/// so large it stresses memory or trips a server's per-request limit. The backfill chunk size
/// upstream may exceed this; [`ExternalEmbeddingProvider::embed`] re-chunks internally so the
/// caller never has to know the wire batch limit.
pub(super) const EMBEDDING_HTTP_BATCH: usize = 64;

/// HTTP request timeout for one embeddings call.
///
/// Embedding a batch on a constrained local server (CPU-only LM Studio/Ollama) can take a while
/// per batch; the timeout is generous so a legitimately slow batch is not mistaken for a hang,
/// while still bounding a truly dead endpoint.
#[cfg(not(any(test, coverage)))]
const EMBEDDING_HTTP_TIMEOUT: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// Wire types (always compiled so the PURE encode/decode helpers + their
// mutation-hardening tests exist in every build).
// ---------------------------------------------------------------------------

/// Request body for `POST {base_url}/embeddings` (OpenAI-compatible shape).
///
/// `dimensions` is sent ONLY when the user configured an explicit MRL dimension; otherwise it is
/// omitted so the server returns the model's native size (D4: never impose a hidden dim). The
/// returned vector length — not this hint — is the truth for `effective_dim`.
#[derive(Debug, Serialize)]
struct EmbeddingsRequestBody<'a> {
    model: &'a str,
    input: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
    /// Some servers (OpenAI) require/accept `encoding_format`; we always request float so the
    /// decoder never has to handle base64 packing. Documented no-op on servers that ignore it.
    encoding_format: &'static str,
}

/// One element of the `data` array in an OpenAI-compatible embeddings response.
#[derive(Debug, Deserialize)]
struct EmbeddingsResponseDatum {
    embedding: Vec<f32>,
    /// Position the server assigns; used to restore input order if the server reorders.
    #[serde(default)]
    index: usize,
}

/// Top-level OpenAI-compatible embeddings response body.
#[derive(Debug, Deserialize)]
struct EmbeddingsResponseBody {
    data: Vec<EmbeddingsResponseDatum>,
}

/// Builds the JSON request body bytes for one batch under one model.
///
/// Pure (no I/O) so the encoding — model id placement, the omit-when-`None` dimension hint, the
/// float encoding format — is unit-tested. `requested_dim` is the resolver's output: `Some(n)`
/// is an explicit MRL hint, `None` asks for the native size.
pub(super) fn build_request_body(
    model: &str,
    inputs: &[String],
    requested_dim: Option<usize>,
) -> Result<Vec<u8>> {
    let body = EmbeddingsRequestBody {
        model,
        input: inputs,
        dimensions: requested_dim,
        encoding_format: "float",
    };
    serde_json::to_vec(&body).context("serializing embeddings request body")
}

/// Decodes a `/v1/embeddings` response body into ordered, defensively-normalized vectors.
///
/// Pure (no I/O) so the wire contract is unit-tested:
/// - reads the ACTUAL returned vectors (02 §C.3 rule a) — the dim is whatever the server sent.
/// - L2-normalizes every vector defensively (rule b), so MRL-truncated or un-normalized
///   transports still produce cosine-correct vectors.
/// - restores the model's `index` ordering so the Nth output maps to the Nth input even if the
///   server reorders `data`.
/// - errors when the count or a dim is wrong rather than silently returning a short/ragged set
///   (a partial batch would corrupt the id↔vector join during backfill).
pub(super) fn decode_response_body(body: &[u8], expected_count: usize) -> Result<Vec<Vec<f32>>> {
    let parsed: EmbeddingsResponseBody = serde_json::from_slice(body).with_context(|| {
        format!("the embedding service's reply has no vectors: {}", reply_excerpt(body))
    })?;
    if parsed.data.len() != expected_count {
        anyhow::bail!(
            "embeddings response returned {} vector(s) for {} input(s)",
            parsed.data.len(),
            expected_count
        );
    }
    // Restore request order by `index`; default-0 servers (single input) stay correct.
    let mut ordered: Vec<Option<Vec<f32>>> = (0..expected_count).map(|_| None).collect();
    for datum in parsed.data {
        let slot = ordered
            .get_mut(datum.index)
            .with_context(|| format!("embeddings response index {} out of range", datum.index))?;
        let mut vector = datum.embedding;
        if vector.is_empty() {
            anyhow::bail!("embeddings response contained an empty vector at index {}", datum.index);
        }
        l2_normalize(&mut vector);
        *slot = Some(vector);
    }
    ordered
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            slot.with_context(|| format!("embeddings response missing index {index}"))
        })
        .collect()
}

/// Resolves the dimension to REQUEST from an OpenAI-compatible server without imposing a hidden
/// model assumption (D4).
///
/// `None` = ask for the model's native dimension (no PathKeep-imposed size). `Some(n)` = the user
/// configured an explicit dimension, sent as an MRL request hint. Anthropic has no embedding API;
/// Gemini is not OpenAI-shaped (it would need its own adapter), so both are rejected here rather
/// than silently routed through the OpenAI-compatible body.
pub(super) fn resolve_external_request_dim(provider: &AiProviderRuntime) -> Result<Option<usize>> {
    match provider.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            Ok(provider.config.dimensions.map(|dim| dim as usize))
        }
        AiRequestFormat::Google => anyhow::bail!(
            "Gemini embedding is not OpenAI-compatible; configure it through a dedicated adapter for provider {}.",
            provider.config.name
        ),
        AiRequestFormat::Anthropic => anyhow::bail!(
            "Anthropic request format does not support embeddings for provider {}.",
            provider.config.name
        ),
    }
}

/// Builds this adapter's runtime descriptor once `effective_dim` is known.
///
/// Fixes the W-AI-0 A-S2 carryover for the external path: dtype/normalized are set from THIS
/// adapter's reality, not a transport-wide constant. The OpenAI-compatible path always returns
/// float32 component values and we ALWAYS L2-normalize them on the way out, so the persisted
/// fingerprint reflects exactly what was stored. `effective_dim` must be the actual returned
/// length (never config `dimensions`), so a fingerprint built from this descriptor is true.
pub(super) fn external_descriptor(
    provider: &AiProviderRuntime,
    effective_dim: Option<usize>,
) -> EmbeddingDescriptor {
    EmbeddingDescriptor {
        provider_id: provider.config.id.clone(),
        model_id: provider.config.default_model.clone(),
        effective_dim,
        // This adapter emits float32 components and unconditionally L2-normalizes them below.
        dtype: EmbeddingDtype::Float32,
        normalized: true,
        // A hosted `/v1/embeddings` server does not expose its pooling.
        pooling: EmbeddingPooling::Unknown,
        // OpenAI-compatible servers have no role/instruction field, so no template is applied
        // (see `embed`'s role note).
        instruction_template: None,
    }
}

/// External embedding provider implementing [`EmbeddingProvider`] over `/v1/embeddings`.
///
/// Holds the resolved provider runtime (config + in-memory secret) and the resolved request-dim
/// hint. The real network path is gated behind `cfg(not(any(test, coverage)))`; the stub path
/// returns deterministic, role-aware, normalized vectors with the same public call graph.
pub struct ExternalEmbeddingProvider {
    runtime: AiProviderRuntime,
    requested_dim: Option<usize>,
}

impl ExternalEmbeddingProvider {
    /// Wraps a resolved embedding-provider runtime as an OpenAI-compatible embedding source.
    ///
    /// Validates up front that the request format actually supports the OpenAI-compatible
    /// embeddings shape (rejecting Anthropic/Gemini) so misconfiguration fails before any embed
    /// call rather than mid-backfill.
    pub fn new(runtime: AiProviderRuntime) -> Result<Self> {
        let requested_dim = resolve_external_request_dim(&runtime)?;
        Ok(Self { runtime, requested_dim })
    }

    /// The embeddings endpoint under the configured address (`openai_api_root`: `localhost`
    /// pinned to IPv4, `/v1` added for a bare LM Studio or Ollama address, a pasted endpoint
    /// trimmed back to its root).
    #[cfg(not(any(test, coverage)))]
    fn endpoint(&self) -> Result<String> {
        let base = self
            .runtime
            .config
            .base_url
            .as_deref()
            .context("external embedding provider requires a base URL")?;
        let root = super::provider::openai_api_root(base, &self.runtime.config.request_format);
        Ok(format!("{root}/embeddings"))
    }
}

impl EmbeddingProvider for ExternalEmbeddingProvider {
    /// Embeds a batch of texts under one role, re-chunking to [`EMBEDDING_HTTP_BATCH`] per call.
    ///
    /// `role` is threaded per the trait contract but is a DOCUMENTED NO-OP for OpenAI-compatible
    /// servers, which have no query/document instruction field. The parameter stays wired because
    /// the trait carries it for engines that prefix a role instruction.
    fn embed(
        &self,
        texts: &[String],
        role: EmbeddingRole,
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>>> + Send {
        embed_impl(self, texts, role)
    }

    fn model_id(&self) -> &str {
        &self.runtime.config.default_model
    }

    fn descriptor(&self) -> EmbeddingDescriptor {
        // `effective_dim` is None here: the descriptor is only authoritative AFTER a real vector
        // has been observed. Callers that need the fingerprint pass the observed length to
        // `external_descriptor` (the indexing loop does exactly this).
        external_descriptor(&self.runtime, None)
    }
}

/// Builds ONE `/v1/embeddings` POST, attaching `Authorization: Bearer …` ONLY for a present key.
///
/// Always compiled (not behind the network cfg) so the OPTIONAL-key contract is a unit-tested,
/// coverage-counted property of the BUILT request rather than a claim about the live socket: a
/// `None` key (no key, or a whitespace-only one — see [`AiProviderRuntime::api_key_for_transport`])
/// attaches NO auth header at all, so a keyless local server (LM Studio / Ollama) is reached
/// untouched; we never send a hollow `Bearer ` a key-enforcing server would reject. `Some(key)`
/// attaches the bearer token verbatim. PathKeep never pre-empts the call on a missing key — a
/// cloud server that needs one answers with its OWN 401, surfaced from the `embed_impl` send path.
pub(super) fn build_embeddings_request(
    client: &reqwest::Client,
    endpoint: &str,
    body: Vec<u8>,
    api_key: Option<&str>,
) -> reqwest::RequestBuilder {
    let request = client.post(endpoint).header("content-type", "application/json").body(body);
    match api_key {
        Some(key) => request.bearer_auth(key),
        None => request,
    }
}

/// Turns one `/v1/embeddings` HTTP outcome into vectors, or surfaces the PROVIDER's OWN error.
///
/// Always compiled (not behind the network cfg) so the "only a provider-returned error fails the
/// call" contract is a unit-tested, coverage-counted property. A NON-success status (the provider's
/// real 401/403/429/5xx — e.g. a key-enforcing cloud server rejecting a missing/invalid key)
/// surfaces verbatim, carrying the server's own response body, NOT a synthetic PathKeep
/// precondition. A success status decodes + normalizes the vectors. `status_code` is the numeric
/// HTTP status (e.g. `401`); `success` mirrors `StatusCode::is_success` so the pure helper does not
/// need the reqwest status type.
pub(super) fn embeddings_response_to_vectors(
    status_code: u16,
    success: bool,
    body: &[u8],
    expected_count: usize,
) -> Result<Vec<Vec<f32>>> {
    if !success {
        let detail = String::from_utf8_lossy(body);
        anyhow::bail!("embeddings request failed with status {status_code}: {detail}");
    }
    if let Some(said) = service_error(body) {
        anyhow::bail!("the embedding service answered with an error: {said}");
    }
    decode_response_body(body, expected_count)
}

/// The error a service put in a successful (2xx) reply instead of vectors.
///
/// LM Studio answers a path it does not serve with HTTP 200 and
/// `{"error":"Unexpected endpoint or method. (POST /embeddings)"}`; OpenAI-style servers put
/// `{"error":{"message":…}}`. Only a reply with an `error` and no `data` counts, so a server
/// that sends `"error": null` beside its vectors still works.
fn service_error(body: &[u8]) -> Option<String> {
    let reply: serde_json::Value = serde_json::from_slice(body).ok()?;
    let reply = reply.as_object()?;
    if reply.contains_key("data") {
        return None;
    }
    match reply.get("error")? {
        serde_json::Value::Null => None,
        serde_json::Value::String(message) => Some(message.clone()),
        error => Some(
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map_or_else(|| error.to_string(), str::to_owned),
        ),
    }
}

/// The start of a reply that could not be read as vectors, for the error that says so.
fn reply_excerpt(body: &[u8]) -> String {
    const SHOWN: usize = 300;
    let text = String::from_utf8_lossy(body);
    let text = text.trim();
    match text.char_indices().nth(SHOWN) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None if text.is_empty() => "an empty reply".to_string(),
        None => text.to_string(),
    }
}

/// Real `/v1/embeddings` embed: re-chunks, POSTs JSON, decodes + normalizes each batch.
#[cfg(not(any(test, coverage)))]
async fn embed_impl(
    provider: &ExternalEmbeddingProvider,
    texts: &[String],
    _role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let endpoint = provider.endpoint()?;
    let client = reqwest::Client::builder()
        .timeout(EMBEDDING_HTTP_TIMEOUT)
        .build()
        .context("building embeddings HTTP client")?;
    // OPTIONAL key resolved ONCE here; `build_embeddings_request` then omits the auth header when
    // it is absent. A keyless local server works untouched; a key-enforcing cloud server 401s.
    let api_key = provider.runtime.api_key_for_transport().map(str::to_owned);

    let mut out = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(EMBEDDING_HTTP_BATCH) {
        let body = build_request_body(
            &provider.runtime.config.default_model,
            chunk,
            provider.requested_dim,
        )?;
        let response = build_embeddings_request(&client, &endpoint, body, api_key.as_deref())
            .send()
            .await
            .context("sending embeddings request")?;
        let status = response.status();
        let bytes = response.bytes().await.context("reading embeddings response body")?;
        out.extend(embeddings_response_to_vectors(
            status.as_u16(),
            status.is_success(),
            &bytes,
            chunk.len(),
        )?);
    }
    Ok(out)
}

/// Deterministic, role-aware, normalized stub embed for tests and coverage builds.
///
/// Mirrors the production call graph (re-chunk → per-batch decode) without a live server, and
/// reuses the SAME pure encode/decode helpers as production so those paths are exercised: it
/// builds the request body, fabricates a wire-shaped response, then decodes it. Two different
/// texts (or roles) produce different vectors; the dim is the requested hint or a small synthetic
/// default with no relation to any real model.
#[cfg(any(test, coverage))]
async fn embed_impl(
    provider: &ExternalEmbeddingProvider,
    texts: &[String],
    role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    #[cfg(coverage)]
    {
        if provider.runtime.config.id.contains("embed-http-error") {
            anyhow::bail!("forced coverage external embedding error");
        }
    }
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let dim = provider.requested_dim.unwrap_or(STUB_EXTERNAL_DIM);
    let mut out = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(EMBEDDING_HTTP_BATCH) {
        // Exercise the production encoder so a body-shape regression is caught in coverage too.
        let _body = build_request_body(
            &provider.runtime.config.default_model,
            chunk,
            provider.requested_dim,
        )?;
        let response_bytes = stub_response_bytes(&provider.runtime.config.id, role, chunk, dim);
        out.extend(decode_response_body(&response_bytes, chunk.len())?);
    }
    Ok(out)
}

/// Synthetic dimension the stub emits when the user did not configure an explicit one.
///
/// An arbitrary small number with NO relationship to any real model's native dimension — picking
/// a real dim (1024/1536/768) here is exactly the D4 truth-assumption this block removes.
#[cfg(any(test, coverage))]
const STUB_EXTERNAL_DIM: usize = 8;

/// Builds wire-shaped response bytes for the stub so the real decoder runs in tests/coverage.
///
/// Deterministically derives each component from a per-(provider, role, text) digest so two
/// different texts or roles differ, and tags each datum with its `index` so the decoder's
/// reorder path is exercised. The vectors are intentionally NOT pre-normalized — the decoder's
/// defensive L2-normalize must do that, and the tests assert near-unit norm afterwards.
#[cfg(any(test, coverage))]
fn stub_response_bytes(
    provider_id: &str,
    role: EmbeddingRole,
    chunk: &[String],
    dim: usize,
) -> Vec<u8> {
    use crate::utils::sha256_hex;
    let data: Vec<serde_json::Value> = chunk
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let digest = sha256_hex(format!("{provider_id}::{}::{text}", role.as_str()).as_bytes());
            let bytes = digest.as_bytes();
            let embedding: Vec<f32> = (0..dim)
                .map(|component| (bytes[component % bytes.len()] % 13) as f32 + 1.0)
                .collect();
            serde_json::json!({ "index": index, "embedding": embedding })
        })
        .collect();
    serde_json::to_vec(&serde_json::json!({ "data": data })).expect("stub response is serializable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding_engine::AnyEmbeddingProvider;
    use crate::ai::embedding_static::StaticEmbeddingProvider;
    use crate::ai::provider::l2_normalize;
    use crate::models::{AiProviderConfig, AiProviderPurpose};
    use secrecy::SecretString;

    fn runtime(format: AiRequestFormat, id: &str, dim: Option<u32>) -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: id.to_string(),
                name: "Embed Provider".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: format,
                enabled: true,
                base_url: Some("http://localhost:1234/v1".to_string()),
                default_model: "text-embedding-test".to_string(),
                dimensions: dim,
                ..AiProviderConfig::default()
            },
            api_key: Some(SecretString::from("test-key".to_string())),
        }
    }

    fn near_unit_norm(vector: &[f32]) -> f32 {
        vector.iter().map(|value| value * value).sum::<f32>().sqrt()
    }

    // ── OPTIONAL-key transport contract (assert on the BUILT request, not a mock) ─────────────────

    #[test]
    fn build_embeddings_request_attaches_bearer_header_only_when_key_present() {
        let client = reqwest::Client::new();
        let endpoint = "http://localhost:1234/v1/embeddings";

        // PRESENT key → the built request carries `Authorization: Bearer <key>` verbatim.
        let with_key =
            build_embeddings_request(&client, endpoint, b"{}".to_vec(), Some("sk-secret"))
                .build()
                .expect("build request with key");
        let auth = with_key
            .headers()
            .get(reqwest::header::AUTHORIZATION)
            .expect("present key must attach an Authorization header");
        assert_eq!(auth, "Bearer sk-secret");

        // ABSENT key → NO Authorization header at all (NOT a hollow `Bearer `). This is the live
        // bug's core: a keyless local server (LM Studio) must be reached untouched.
        let without_key = build_embeddings_request(&client, endpoint, b"{}".to_vec(), None)
            .build()
            .expect("build request without key");
        assert!(
            without_key.headers().get(reqwest::header::AUTHORIZATION).is_none(),
            "absent key must send NO Authorization header"
        );
        // The non-auth headers (and method/url) are identical either way — only auth differs.
        assert_eq!(without_key.method(), reqwest::Method::POST);
        assert_eq!(
            without_key.headers().get(reqwest::header::CONTENT_TYPE).map(|value| value.as_bytes()),
            Some(b"application/json".as_slice())
        );
    }

    #[test]
    fn embeddings_response_surfaces_provider_401_verbatim_and_decodes_success() {
        // A PROVIDER-returned 401 (e.g. a key-enforcing cloud server rejecting a missing/invalid
        // key) surfaces as the provider's OWN error carrying its response body — NOT a synthetic
        // PathKeep precondition. This is the "only the provider may say no" half of the principle.
        let unauthorized = embeddings_response_to_vectors(
            401,
            false,
            br#"{"error":{"message":"Incorrect API key provided"}}"#,
            1,
        )
        .expect_err("a 401 must surface as an error");
        let message = unauthorized.to_string();
        assert!(message.contains("status 401"), "carries the provider status: {message}");
        assert!(
            message.contains("Incorrect API key provided"),
            "carries the provider's own body verbatim: {message}"
        );

        // A 403 is likewise surfaced (any non-success provider verdict blocks, nothing else does).
        let forbidden =
            embeddings_response_to_vectors(403, false, b"forbidden", 1).expect_err("403 surfaces");
        assert!(forbidden.to_string().contains("status 403"));

        // A success status decodes + L2-normalizes the returned vectors.
        let ok_body = serde_json::to_vec(&serde_json::json!({
            "data": [ { "index": 0, "embedding": [3.0, 0.0, 4.0] } ]
        }))
        .expect("body");
        let vectors =
            embeddings_response_to_vectors(200, true, &ok_body, 1).expect("success decodes");
        assert_eq!(vectors.len(), 1);
        assert!((near_unit_norm(&vectors[0]) - 1.0).abs() < 1e-6);
    }

    /// What a service may send back with a 2xx status, and what the user must then see. Ways it
    /// could go wrong: LM Studio's 200 + `{"error": "..."}` decoded as "no vectors" with its
    /// words dropped; an OpenAI-style error object shown as raw JSON; a reply with vectors and
    /// `"error": null` rejected; an HTML page reported without a hint of what it was.
    #[test]
    fn a_successful_status_with_an_error_says_what_the_service_said() {
        let lm_studio = embeddings_response_to_vectors(
            200,
            true,
            br#"{"error":"Unexpected endpoint or method. (POST /embeddings)"}"#,
            1,
        )
        .expect_err("an error body fails");
        assert_eq!(
            lm_studio.to_string(),
            "the embedding service answered with an error: Unexpected endpoint or method. (POST /embeddings)"
        );

        let openai_style = embeddings_response_to_vectors(
            200,
            true,
            br#"{"error":{"message":"model 'chat-model' is not an embedding model","type":"invalid_request_error"}}"#,
            1,
        )
        .expect_err("an error object fails");
        assert!(
            openai_style.to_string().ends_with("model 'chat-model' is not an embedding model"),
            "{openai_style}"
        );

        let bare_object = embeddings_response_to_vectors(200, true, br#"{"error":{"code":7}}"#, 1)
            .expect_err("an error without a message fails");
        assert!(bare_object.to_string().ends_with(r#"{"code":7}"#), "{bare_object}");

        let with_null_error = serde_json::to_vec(&serde_json::json!({
            "error": null,
            "data": [ { "index": 0, "embedding": [1.0, 0.0] } ]
        }))
        .expect("body");
        assert_eq!(
            embeddings_response_to_vectors(200, true, &with_null_error, 1).expect("vectors").len(),
            1
        );

        let page = format!("<html><body>{}</body></html>", "x".repeat(400));
        let html = embeddings_response_to_vectors(200, true, page.as_bytes(), 1)
            .expect_err("a web page fails");
        let shown = format!("{html:#}");
        assert!(shown.contains("reply has no vectors: <html><body>x"), "{shown}");
        assert!(shown.contains('…') && shown.len() < 600, "only the start is shown: {shown}");

        let empty = embeddings_response_to_vectors(200, true, b"", 1).expect_err("empty fails");
        assert!(format!("{empty:#}").contains("an empty reply"));
    }

    #[test]
    fn api_key_for_transport_treats_absent_and_blank_keys_as_no_header() {
        // None → no header.
        let mut absent = runtime(AiRequestFormat::LmStudio, "p", None);
        absent.api_key = None;
        assert_eq!(absent.api_key_for_transport(), None);

        // A whitespace-only key is treated exactly like an absent one (no hollow `Bearer `).
        let mut blank = runtime(AiRequestFormat::LmStudio, "p", None);
        blank.api_key = Some(SecretString::from("   ".to_string()));
        assert_eq!(blank.api_key_for_transport(), None);

        // A real key flows through verbatim.
        let mut present = runtime(AiRequestFormat::LmStudio, "p", None);
        present.api_key = Some(SecretString::from("sk-real".to_string()));
        assert_eq!(present.api_key_for_transport(), Some("sk-real"));
    }

    #[test]
    fn build_request_body_omits_dimension_when_native() {
        let body = build_request_body("model-x", &["hi".to_string()], None).expect("body");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["model"], "model-x");
        assert_eq!(value["input"][0], "hi");
        assert_eq!(value["encoding_format"], "float");
        // Native request must NOT carry a dimensions hint (D4: no imposed size).
        assert!(value.get("dimensions").is_none());
    }

    #[test]
    fn build_request_body_includes_explicit_dimension_hint() {
        let body = build_request_body("model-x", &["a".to_string(), "b".to_string()], Some(256))
            .expect("body");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(value["dimensions"], 256);
        assert_eq!(value["input"].as_array().expect("array").len(), 2);
    }

    #[test]
    fn decode_response_reads_actual_length_and_normalizes() {
        // Two un-normalized vectors of length 3; the decoder must return them L2-normalized and in
        // request order.
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [
                { "index": 0, "embedding": [3.0, 0.0, 4.0] },
                { "index": 1, "embedding": [0.0, 6.0, 8.0] },
            ]
        }))
        .expect("body");
        let vectors = decode_response_body(&body, 2).expect("decode");
        assert_eq!(vectors.len(), 2);
        // effective dim is whatever the server returned (3), not any config value.
        assert_eq!(vectors[0].len(), 3);
        assert!((near_unit_norm(&vectors[0]) - 1.0).abs() < 1e-6);
        assert!((near_unit_norm(&vectors[1]) - 1.0).abs() < 1e-6);
        // [3,0,4] normalizes to [0.6, 0, 0.8].
        assert!((vectors[0][0] - 0.6).abs() < 1e-6);
        assert!((vectors[0][2] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn decode_response_restores_index_order() {
        // Server returns data out of order; the decoder must put index 0 first.
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [
                { "index": 1, "embedding": [0.0, 1.0] },
                { "index": 0, "embedding": [1.0, 0.0] },
            ]
        }))
        .expect("body");
        let vectors = decode_response_body(&body, 2).expect("decode");
        assert_eq!(vectors[0], vec![1.0, 0.0]);
        assert_eq!(vectors[1], vec![0.0, 1.0]);
    }

    #[test]
    fn decode_response_rejects_count_mismatch() {
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [ { "index": 0, "embedding": [1.0] } ]
        }))
        .expect("body");
        let error = decode_response_body(&body, 2).expect_err("count mismatch");
        assert!(error.to_string().contains("1 vector(s) for 2 input(s)"));
    }

    #[test]
    fn decode_response_rejects_empty_vector() {
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [ { "index": 0, "embedding": [] } ]
        }))
        .expect("body");
        let error = decode_response_body(&body, 1).expect_err("empty vector");
        assert!(error.to_string().contains("empty vector"));
    }

    #[test]
    fn decode_response_rejects_out_of_range_index() {
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [ { "index": 5, "embedding": [1.0] } ]
        }))
        .expect("body");
        let error = decode_response_body(&body, 1).expect_err("oob index");
        assert!(error.to_string().contains("out of range"));
    }

    #[test]
    fn decode_response_rejects_missing_index() {
        // Two slots expected but both data point at index 0 → slot 1 is never filled.
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [
                { "index": 0, "embedding": [1.0] },
                { "index": 0, "embedding": [2.0] },
            ]
        }))
        .expect("body");
        let error = decode_response_body(&body, 2).expect_err("missing index");
        assert!(error.to_string().contains("missing index 1"));
    }

    #[test]
    fn decode_response_rejects_unparseable_body() {
        let error = decode_response_body(b"{not json", 1).expect_err("bad json");
        assert_eq!(error.to_string(), "the embedding service's reply has no vectors: {not json");
    }

    #[test]
    fn resolve_external_request_dim_maps_formats() {
        assert_eq!(
            resolve_external_request_dim(&runtime(AiRequestFormat::OpenAi, "p", None))
                .expect("openai"),
            None
        );
        assert_eq!(
            resolve_external_request_dim(&runtime(AiRequestFormat::LmStudio, "p", Some(512)))
                .expect("lmstudio"),
            Some(512)
        );
        assert_eq!(
            resolve_external_request_dim(&runtime(AiRequestFormat::Ollama, "p", Some(256)))
                .expect("ollama"),
            Some(256)
        );
        assert!(
            resolve_external_request_dim(&runtime(AiRequestFormat::Google, "p", None)).is_err()
        );
        assert!(
            resolve_external_request_dim(&runtime(AiRequestFormat::Anthropic, "p", None)).is_err()
        );
    }

    #[test]
    fn external_descriptor_sets_float32_normalized_truth() {
        let descriptor = external_descriptor(&runtime(AiRequestFormat::OpenAi, "p", None), Some(8));
        assert_eq!(descriptor.dtype, EmbeddingDtype::Float32);
        assert!(descriptor.normalized);
        assert_eq!(descriptor.pooling, EmbeddingPooling::Unknown);
        assert_eq!(descriptor.instruction_template, None);
        assert_eq!(descriptor.effective_dim, Some(8));
        assert_eq!(descriptor.model_id, "text-embedding-test");
        assert_eq!(descriptor.provider_id, "p");
    }

    #[test]
    fn new_rejects_non_openai_formats() {
        assert!(
            ExternalEmbeddingProvider::new(runtime(AiRequestFormat::Anthropic, "p", None)).is_err()
        );
        assert!(
            ExternalEmbeddingProvider::new(runtime(AiRequestFormat::Google, "p", None)).is_err()
        );
        assert!(
            ExternalEmbeddingProvider::new(runtime(AiRequestFormat::OpenAi, "p", None)).is_ok()
        );
    }

    #[tokio::test]
    async fn embed_returns_role_aware_normalized_vectors() {
        let provider =
            ExternalEmbeddingProvider::new(runtime(AiRequestFormat::LmStudio, "p", None))
                .expect("provider");
        let docs = provider
            .embed(&["alpha".to_string(), "beta".to_string()], EmbeddingRole::Document)
            .await
            .expect("embed docs");
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].len(), STUB_EXTERNAL_DIM);
        assert!((near_unit_norm(&docs[0]) - 1.0).abs() < 1e-6);
        // Different texts differ.
        assert_ne!(docs[0], docs[1]);

        // Same text under a different role differs (role is threaded into the stub digest).
        let query = provider
            .embed(&["alpha".to_string()], EmbeddingRole::Query)
            .await
            .expect("embed query");
        assert_ne!(query[0], docs[0]);
    }

    #[tokio::test]
    async fn embed_empty_input_short_circuits() {
        let provider = ExternalEmbeddingProvider::new(runtime(AiRequestFormat::OpenAi, "p", None))
            .expect("provider");
        assert!(provider.embed(&[], EmbeddingRole::Document).await.expect("empty").is_empty());
    }

    #[tokio::test]
    async fn embed_honors_explicit_dimension() {
        let provider =
            ExternalEmbeddingProvider::new(runtime(AiRequestFormat::OpenAi, "p", Some(16)))
                .expect("provider");
        let vectors =
            provider.embed(&["x".to_string()], EmbeddingRole::Document).await.expect("embed");
        assert_eq!(vectors[0].len(), 16);
    }

    #[tokio::test]
    async fn embed_rechunks_beyond_http_batch() {
        let provider = ExternalEmbeddingProvider::new(runtime(AiRequestFormat::OpenAi, "p", None))
            .expect("provider");
        let inputs: Vec<String> =
            (0..(EMBEDDING_HTTP_BATCH + 5)).map(|index| format!("text-{index}")).collect();
        let vectors = provider.embed(&inputs, EmbeddingRole::Document).await.expect("embed");
        assert_eq!(vectors.len(), inputs.len());
    }

    #[test]
    fn model_id_and_descriptor_passthrough() {
        let provider = ExternalEmbeddingProvider::new(runtime(AiRequestFormat::OpenAi, "p", None))
            .expect("provider");
        assert_eq!(provider.model_id(), "text-embedding-test");
        // The trait-level descriptor reports no observed dim yet.
        assert_eq!(provider.descriptor().effective_dim, None);
    }

    #[tokio::test]
    async fn any_provider_dispatches_external_variant() {
        let provider = ExternalEmbeddingProvider::new(runtime(AiRequestFormat::OpenAi, "p", None))
            .expect("provider");
        let any = AnyEmbeddingProvider::External(provider);
        assert_eq!(any.model_id(), "text-embedding-test");
        assert_eq!(any.descriptor().dtype, EmbeddingDtype::Float32);
        let vectors = any.embed(&["hi".to_string()], EmbeddingRole::Document).await.expect("embed");
        assert_eq!(vectors.len(), 1);
        assert!((near_unit_norm(&vectors[0]) - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn any_provider_dispatches_static_variant() {
        // The W-AI-4c Static arm must be reachable through the enum so the index/search loops can
        // run the in-app static engine without a call-site change. Uses the deterministic static stub.
        let provider = StaticEmbeddingProvider::new_stub(
            "static:potion",
            "minishlab/potion-multilingual-128M",
        );
        let any = AnyEmbeddingProvider::Static(Box::new(provider));
        assert_eq!(any.model_id(), "minishlab/potion-multilingual-128M");
        // Static is symmetric: Mean pooling, no instruction template.
        assert_eq!(any.descriptor().pooling, EmbeddingPooling::Mean);
        assert_eq!(any.descriptor().instruction_template, None);
        let vectors =
            any.embed(&["hi".to_string()], EmbeddingRole::Document).await.expect("static embed");
        assert_eq!(vectors.len(), 1);
        assert!((near_unit_norm(&vectors[0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_helper_is_reused_from_provider() {
        // Guards that this module shares the ONE normalize impl rather than forking it.
        let mut vector = vec![3.0f32, 4.0];
        l2_normalize(&mut vector);
        assert!((near_unit_norm(&vector) - 1.0).abs() < 1e-6);
    }

    // Triggers the coverage-only forced-error branch in `embed_impl` so it is not dead code under
    // the coverage gate. The branch fires when the provider id contains `embed-http-error`,
    // standing in for a transport failure the real `#[cfg(not(any(test, coverage)))]` HTTP path
    // would surface — the stub cannot otherwise produce one. Under plain `test` (no `coverage`) the
    // branch is compiled out, so this test only exists in coverage builds.
    #[cfg(coverage)]
    #[tokio::test]
    async fn embed_surfaces_forced_coverage_transport_error() {
        let provider = ExternalEmbeddingProvider::new(runtime(
            AiRequestFormat::OpenAi,
            "embed-http-error",
            None,
        ))
        .expect("provider");
        let error = provider
            .embed(&["x".to_string()], EmbeddingRole::Document)
            .await
            .expect_err("forced transport error");
        assert!(error.to_string().contains("forced coverage external embedding error"));
    }
}
