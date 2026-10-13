//! PathKeep-owned AI boundary traits (rig-free contract layer).
//!
//! ## Responsibilities
//! - declare the three boundary traits every AI engine implements: `EmbeddingProvider`,
//!   `LlmProvider`, and `VectorIndex`
//! - own the minimal request/response/descriptor types that cross those boundaries
//! - keep PathKeep semantics independent of any third-party LLM/vector crate, so the
//!   transport (rig today, anything tomorrow) can be swapped behind these signatures
//!
//! ## Not responsible for
//! - any concrete provider/engine implementation (rig adapters live in `provider.rs`;
//!   the vector engine arrives in W-AI-5)
//! - persistence, scheduling, or queue bookkeeping (those stay in the surrounding `ai`
//!   submodules)
//! - secret storage (API keys live in the keyring + `AiProviderRuntime`)
//!
//! ## Why this module exists
//! It is the ONE place that is intentionally free of `rig` (and every other vendor)
//! type. Downstream blocks implement these traits; nothing outside an adapter is allowed
//! to depend on a vendor type, which is what keeps the LLM/vector backend replaceable.

use anyhow::Result;
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Whether an embedding request is for a search query or an indexed document.
///
/// Asymmetric embedding models (instruction-tuned ones in particular) encode queries and
/// documents differently. The role threads from the call site all the
/// way to the provider so neither the index nor search silently mixes the two encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmbeddingRole {
    /// Text being embedded to search the index (the user's question).
    Query,
    /// Text being embedded to populate the index (a stored history row).
    Document,
}

impl EmbeddingRole {
    /// Returns the stable lowercase tag used in fingerprints, logs, and instruction lookup.
    ///
    /// A dedicated tag keeps role identity decoupled from `Debug` formatting so fingerprint
    /// hashes stay stable even if the enum's `Debug` representation ever changes.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Document => "document",
        }
    }
}

/// Numeric representation a provider emits for each embedding component.
///
/// Recorded in the fingerprint so an index built from `float32` vectors is correctly
/// treated as stale if the provider is later reconfigured to emit a quantized dtype.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmbeddingDtype {
    /// IEEE-754 single precision (the common default for cloud and local providers).
    Float32,
    /// Signed 8-bit integers (e.g. Cohere/Voyage `int8` output mode).
    Int8,
    /// Unsigned 8-bit integers.
    Uint8,
    /// 1-bit packed binary embeddings.
    Binary,
}

impl EmbeddingDtype {
    /// Returns the stable lowercase tag used when serializing the fingerprint.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Float32 => "float32",
            Self::Int8 => "int8",
            Self::Uint8 => "uint8",
            Self::Binary => "binary",
        }
    }
}

/// Pooling strategy a model applies to produce a single vector per input.
///
/// Like dtype and dim, pooling is a per-model property PathKeep never hardcodes (D4): it is
/// detected/declared at runtime and recorded so a model swap invalidates the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmbeddingPooling {
    /// The pooling strategy is opaque to PathKeep (typical for hosted `/v1/embeddings`).
    Unknown,
    /// Mean pooling over token embeddings.
    Mean,
    /// Last-token pooling (common for decoder-style embedding models).
    LastToken,
    /// A dedicated CLS/pooler token.
    Cls,
}

impl EmbeddingPooling {
    /// Returns the stable lowercase tag used when serializing the fingerprint.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Mean => "mean",
            Self::LastToken => "last-token",
            Self::Cls => "cls",
        }
    }
}

/// Runtime-detected description of one embedding model's encoding behaviour.
///
/// This is the model-agnostic capability descriptor (D4): every value is observed or
/// declared at runtime rather than assumed. `effective_dim` is `None` until at least one
/// vector has been returned, because the only trustworthy dimension is the length of a
/// real response (02 §C.3 rule a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingDescriptor {
    /// Stable provider identity that owns this model (e.g. an `AiProviderConfig` id).
    pub provider_id: String,
    /// Model identifier as configured by the user; never a hardcoded product string.
    pub model_id: String,
    /// Actual returned vector length once known; `None` before the first response.
    pub effective_dim: Option<usize>,
    /// Numeric type of each component.
    pub dtype: EmbeddingDtype,
    /// Whether vectors are already L2-normalized when the provider returns them.
    pub normalized: bool,
    /// Pooling strategy when known.
    pub pooling: EmbeddingPooling,
    /// Optional instruction template the model expects to be prefixed (role-dependent).
    pub instruction_template: Option<String>,
}

/// Boundary trait for any source of embedding vectors (cloud, local, in-app).
///
/// Implementations live behind this trait so the index and search layers depend only on
/// PathKeep types. The `role` parameter is mandatory so query/document asymmetry can never
/// be lost on the way to the model.
///
/// `embed` is intentionally RPITIT (`-> impl Future`) to keep the high-volume embedding hot
/// path (the 14.4M-row S1 throughput gate) monomorphized and inlinable — it is therefore NOT
/// `dyn`-compatible. The coexisting impls (external `/v1/embeddings` + the in-app static model)
/// are selected at runtime and MUST be dispatched via an `enum AnyEmbeddingProvider { .. }`
/// with a hand-rolled `impl EmbeddingProvider`, NOT `Box<dyn EmbeddingProvider>`. Do not box
/// `embed` for false symmetry with `LlmProvider::chat_stream` (whose only boxed part is the
/// stream item, on a far lower-volume path).
pub trait EmbeddingProvider: Send + Sync {
    /// Embeds a batch of texts under one role, returning one vector per input in order.
    fn embed(
        &self,
        texts: &[String],
        role: EmbeddingRole,
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>>> + Send;

    /// Returns the configured model identifier (a runtime string, never a product constant).
    fn model_id(&self) -> &str;

    /// Returns the current runtime descriptor for this model.
    fn descriptor(&self) -> EmbeddingDescriptor;
}

/// One message in a chat completion request.
///
/// `tool_call_id`/`tool_name` are populated only for the `Tool` role so a tool
/// result can be correlated back to the model's originating call. They are
/// additive (W-AI-1) and default to `None` for ordinary turns; the W-AI-7 agent
/// harness fills them when it threads executed tool results back into history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmMessage {
    /// Conversational role of the speaker.
    pub role: LlmRole,
    /// Plain-text content of the message.
    pub content: String,
    /// For the `Tool` role: the id of the tool call this message answers.
    pub tool_call_id: Option<String>,
    /// For the `Tool` role: the name of the tool that produced this result.
    pub tool_name: Option<String>,
    /// For the `Assistant` role: the tool calls this turn requested.
    ///
    /// An OpenAI-compatible transport REQUIRES that every `tool` result follows an `assistant`
    /// message carrying the matching `tool_calls` (correlated by call id). The agent harness must
    /// therefore thread back the assistant's tool-call turn BEFORE the tool results, or the model
    /// cannot see that it already called the tool and re-issues the same call every turn (the loop
    /// bug). Defaults to empty for ordinary turns (additive, W-AI-7).
    pub tool_calls: Vec<LlmToolCall>,
}

/// One tool/function call an assistant turn requested, threaded back so the transport can correlate
/// the following `tool` result(s) to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmToolCall {
    /// Provider call id (matches the `tool_call_id` on the answering `Tool` message).
    pub call_id: String,
    /// Tool name the assistant invoked.
    pub name: String,
    /// Raw JSON arguments string the assistant emitted.
    pub arguments: String,
}

impl LlmMessage {
    /// Builds an ordinary (non-tool) message with no tool correlation metadata.
    ///
    /// Most call sites only ever send System/User/Assistant turns; this keeps them
    /// from repeating the additive tool fields the W-AI-7 shape introduced.
    pub fn new(role: LlmRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_call_id: None,
            tool_name: None,
            tool_calls: Vec::new(),
        }
    }

    /// Builds a `Tool`-role message carrying the originating call id and tool name.
    pub fn tool_result(
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: LlmRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            tool_name: Some(tool_name.into()),
            tool_calls: Vec::new(),
        }
    }

    /// Builds an `Assistant`-role turn recording the tool calls it requested (plus any visible text).
    ///
    /// The agent harness pushes this AHEAD of the answering `Tool` results so the transport can
    /// correlate them; without it an OpenAI-compatible model never registers its own prior calls and
    /// loops on the same call (W-AI-7 tool-loop fix).
    pub fn assistant_tool_calls(content: impl Into<String>, tool_calls: Vec<LlmToolCall>) -> Self {
        Self {
            role: LlmRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            tool_name: None,
            tool_calls,
        }
    }
}

/// A tool/function definition exposed to the model for one chat turn.
///
/// W-AI-1 threads these DEFINITIONS through to the provider so the model can emit
/// `LlmStreamChunk::ToolCall`; tool *execution* is W-AI-7. `parameters` is a raw
/// JSON-Schema object (kept as a value, not a Rust type, because each tool's
/// schema is dynamic and provider-encoded by the adapter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmToolDef {
    /// Tool name the model uses to invoke the function.
    pub name: String,
    /// Human/model-facing description of what the tool does.
    pub description: String,
    /// JSON-Schema object describing the tool's arguments.
    pub parameters: serde_json::Value,
}

/// Structured-output request for one chat turn.
///
/// When `Some`, the adapter asks the provider to constrain the response to the
/// JSON schema. Per 02 §B, a serde validate-and-repair fallback still applies
/// above this boundary (constrained ≠ semantically correct); this only carries
/// the schema intent down to the transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmResponseFormat {
    /// A name for the schema (some providers require/echo one).
    pub schema_name: String,
    /// The JSON-Schema object the response must conform to.
    pub schema: serde_json::Value,
}

/// Token accounting for one completed chat turn, when the provider reports it.
///
/// The agent budget loop (02 §F) needs prompt/completion counts to enforce
/// per-run token ceilings. Providers that omit usage leave this `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LlmUsage {
    /// Input ("prompt") tokens consumed.
    pub prompt_tokens: u64,
    /// Output ("completion") tokens generated.
    pub completion_tokens: u64,
}

/// Conversational role of an `LlmMessage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LlmRole {
    /// System/preamble guidance.
    System,
    /// End-user input.
    User,
    /// Prior model output.
    Assistant,
    /// A tool/function result fed back to the model.
    Tool,
}

/// Request for one chat turn against an `LlmProvider`.
///
/// `tools` and `response_format` are additive (W-AI-1); they default to empty/`None`
/// so existing callers keep compiling. Use [`LlmChatRequest::new`] for a plain turn.
#[derive(Debug, Clone, PartialEq)]
pub struct LlmChatRequest {
    /// Ordered conversation, oldest first; the system message (if any) leads.
    pub messages: Vec<LlmMessage>,
    /// Sampling temperature; `None` lets the provider/default decide.
    pub temperature: Option<f32>,
    /// Hard cap on generated tokens; `None` lets the provider/default decide.
    pub max_tokens: Option<u32>,
    /// Tool DEFINITIONS exposed to the model this turn (execution is W-AI-7).
    pub tools: Vec<LlmToolDef>,
    /// Optional structured-output schema the response should conform to.
    pub response_format: Option<LlmResponseFormat>,
}

impl LlmChatRequest {
    /// Builds a plain chat request with no tools and no structured-output schema.
    ///
    /// Exists so the common case does not have to spell out the two additive fields.
    pub fn new(
        messages: Vec<LlmMessage>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Self {
        Self { messages, temperature, max_tokens, tools: Vec::new(), response_format: None }
    }
}

/// Response from one non-streaming chat turn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LlmChatResponse {
    /// The model's answer text.
    pub text: String,
    /// Separated reasoning/thinking content when the provider exposes it.
    pub reasoning: Option<String>,
    /// Token accounting when the provider reports it (`None` otherwise).
    pub usage: Option<LlmUsage>,
}

/// One incremental piece of a streaming chat response.
///
/// W-AI-1 maps these onto the `pathkeep://ai-stream` Tauri event. The variants are split by
/// kind up front so the UI can render tokens, reasoning, and tool calls in distinct lanes.
///
/// W-AI-7 is ADDITIVE here: `Usage` surfaces the per-turn token accounting the agent budget loop
/// needs (mapped from the provider's terminal marker), and `ToolCall` now carries the provider
/// `call_id` so an executed tool result can be correlated back to its originating call. Existing
/// `Token`/`Reasoning` are untouched, so plain streaming chat (no tools, budget ignored) keeps the
/// same observable behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmStreamChunk {
    /// A fragment of the visible answer.
    Token(String),
    /// A fragment of the reasoning/thinking stream.
    Reasoning(String),
    /// A tool/function call the model wants executed.
    ToolCall {
        /// Provider-assigned call id, when the transport reports one. The harness threads it back
        /// to the model via [`LlmMessage::tool_result`] so multi-call turns stay correlated; an
        /// empty string means the provider did not assign one (older / non-native transports).
        call_id: String,
        /// Tool name to invoke.
        name: String,
        /// JSON-encoded arguments.
        arguments: String,
    },
    /// The terminal token accounting for this turn, when the provider reports it.
    ///
    /// The agent harness sums these across turns to enforce a per-run token budget (02 §F). Plain
    /// streaming chat ignores it; it carries no visible text.
    Usage(LlmUsage),
}

/// Capabilities one configured LLM provider advertises.
///
/// Mirrors the semantics of [`crate::models::AiProviderCapabilityReport`] (the serialized,
/// UI-facing report) but adds the agent-relevant `prompt_cache` and `max_context_tokens`
/// the harness needs. The serialized report stays the contract for IPC; this is the
/// in-engine view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LlmCapabilities {
    /// Whether the provider supports tool/function calling.
    pub tool_call: bool,
    /// Whether the provider supports schema-constrained structured output.
    pub structured_output: bool,
    /// Whether the provider supports token streaming.
    pub streaming: bool,
    /// Whether the provider supports prompt/prefix caching.
    pub prompt_cache: bool,
    /// Maximum context window in tokens when known.
    pub max_context_tokens: Option<u32>,
}

/// Boxed stream of streaming chat chunks returned by [`LlmProvider::chat_stream`].
///
/// Boxed (rather than RPITIT) so the streaming method stays object-safe-friendly and the
/// concrete rig stream type never leaks across the boundary.
pub type LlmChunkStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>;

/// Boundary trait for chat completion providers (external today, per D1).
///
/// rig types are confined to the adapter that implements this trait; everything above the
/// boundary speaks only PathKeep types.
pub trait LlmProvider: Send + Sync {
    /// Runs one non-streaming chat turn.
    fn chat(
        &self,
        req: LlmChatRequest,
    ) -> impl std::future::Future<Output = Result<LlmChatResponse>> + Send;

    /// Runs one streaming chat turn, yielding chunks as they arrive.
    fn chat_stream(
        &self,
        req: LlmChatRequest,
    ) -> impl std::future::Future<Output = Result<LlmChunkStream>> + Send;

    /// Returns the provider's advertised capabilities.
    fn capabilities(&self) -> LlmCapabilities;
}

/// Boundary trait for the vector index engine.
///
/// `u64` external ids are the stable join key back to canonical history rows, so allowlist
/// filtering addresses vectors without depending on the engine's internal ordering.
pub trait VectorIndex: Send + Sync {
    /// Builds a fresh index from `(external_id, vector)` pairs.
    fn build(&mut self, items: &[(u64, Vec<f32>)]) -> Result<()>;

    /// Appends `(external_id, vector)` pairs to the existing index.
    fn append(&mut self, items: &[(u64, Vec<f32>)]) -> Result<()>;

    /// Returns the top-`k` `(external_id, score)` matches for `query`.
    ///
    /// When `allowlist` is `Some`, only those external ids are eligible (visibility/filter).
    fn search(&self, query: &[f32], k: usize, allowlist: Option<&[u64]>)
    -> Result<Vec<(u64, f32)>>;

    /// Loads the index from its backing store.
    fn load(&mut self) -> Result<()>;
}
