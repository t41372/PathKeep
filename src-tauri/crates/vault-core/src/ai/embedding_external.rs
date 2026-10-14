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
//! HTTP path is `#[cfg(not(test))]`; under `cfg(test)` a deterministic same-signature stub
//! fabricates a wire-shaped reply and runs it through the same response decoder, so the index
//! and search tests get vectors without a live server.

use super::provider::l2_normalize;
use super::traits::{
    EmbeddingDescriptor, EmbeddingDtype, EmbeddingPooling, EmbeddingProvider, EmbeddingRole,
};
use super::{AiProviderRuntime, AiRequestFormat};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
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
#[cfg(not(test))]
const EMBEDDING_HTTP_TIMEOUT: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// Wire types.
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
/// Pure (no I/O): model id placement, the omit-when-`None` dimension hint and the float encoding
/// format. `requested_dim` is the resolver's output: `Some(n)`
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
/// Pure (no I/O). The wire contract:
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
/// hint. The real network path is gated behind `cfg(not(test))`; the stub path
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
    #[cfg(not(test))]
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
/// Always compiled (not behind the network cfg) so the OPTIONAL-key contract is a unit-tested
/// property of the BUILT request rather than a claim about the live socket: a
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
/// Always compiled (not behind the network cfg) so the test stub decodes through it too. A
/// NON-success status (the provider's
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
#[cfg(not(test))]
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

/// Deterministic, role-aware, normalized stub embed for tests.
///
/// Mirrors the production call graph (re-chunk → per-batch decode) without a live server, and
/// reuses the SAME pure encode/decode helpers as production so those paths are exercised: it
/// builds the request body, fabricates a wire-shaped response, then decodes it. Two different
/// texts (or roles) produce different vectors; the dim is the requested hint or a small synthetic
/// default with no relation to any real model.
#[cfg(test)]
async fn embed_impl(
    provider: &ExternalEmbeddingProvider,
    texts: &[String],
    role: EmbeddingRole,
) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let dim = provider.requested_dim.unwrap_or(STUB_EXTERNAL_DIM);
    let mut out = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(EMBEDDING_HTTP_BATCH) {
        // Run the production encoder so a body it cannot build fails here too.
        let _body = build_request_body(
            &provider.runtime.config.default_model,
            chunk,
            provider.requested_dim,
        )?;
        let response_bytes = stub_response_bytes(&provider.runtime.config.id, role, chunk, dim);
        out.extend(embeddings_response_to_vectors(200, true, &response_bytes, chunk.len())?);
    }
    Ok(out)
}

/// Synthetic dimension the stub emits when the user did not configure an explicit one.
///
/// An arbitrary small number with NO relationship to any real model's native dimension — picking
/// a real dim (1024/1536/768) here is exactly the D4 truth-assumption this block removes.
#[cfg(test)]
const STUB_EXTERNAL_DIM: usize = 8;

/// Builds wire-shaped response bytes for the stub so the real decoder runs in tests.
///
/// Deterministically derives each component from a per-(provider, role, text) digest so two
/// different texts or roles differ, and tags each datum with its `index` so the decoder's
/// reorder path is exercised. The vectors are intentionally NOT pre-normalized — the decoder's
/// defensive L2-normalize must do that, and the tests assert near-unit norm afterwards.
#[cfg(test)]
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
}
