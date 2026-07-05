//! rig adapter that implements PathKeep's `LlmProvider` boundary.
//!
//! ## Responsibilities
//! - own the ONE place where rig's completion client/types are touched for chat
//! - map PathKeep `LlmChatRequest`/`LlmMessage`/`LlmToolDef` into a rig request and
//!   map rig's (streaming and non-streaming) responses back into PathKeep types
//! - branch on `AiRequestFormat` to the right rig provider client
//!   (OpenAi/Ollama/LmStudio → openai, Anthropic → anthropic, Google → gemini)
//! - derive `LlmCapabilities` from the provider config
//!
//! ## Not responsible for
//! - resolving provider config or secrets (that is the worker's `AiProviderRuntime`)
//! - emitting Tauri events or owning the streaming run registry (worker/IPC layers)
//! - executing tool calls (W-AI-7); this only threads tool DEFINITIONS through and
//!   surfaces `ToolCall` chunks
//!
//! ## Why this module exists
//! The `LlmProvider` trait is rig-free by contract (`traits.rs`). This adapter is the
//! only file allowed to depend on rig for chat, so the transport stays swappable. The
//! real network paths are `#[cfg(not(any(test, coverage)))]`; a deterministic stub with
//! the SAME call graph backs `#[cfg(any(test, coverage))]` so the 100% coverage gate is
//! met without a live model.
//!
//! ## Optional API key (key is never a precondition)
//! The API key is OPTIONAL: a local/LAN self-hosted model (LM Studio / Ollama) needs none, so the
//! client builders forward a key ONLY when one is stored and non-blank
//! (`AiProviderRuntime::api_key_for_transport`). PathKeep never pre-empts a chat on a missing key;
//! a key-enforcing cloud server returns its OWN 401 instead. NOTE (upstream limitation): rig
//! 0.34's `openai`/`anthropic`/`gemini` `ClientBuilder::build` always inserts an auth header
//! (`Authorization: Bearer …` / `x-api-key` / `x-goog-api-key`) once `.api_key()` is called, with
//! no affordance to omit it for these concrete clients. A keyless local model ignores the empty
//! header, so chat still works; the embedding path (which we own end-to-end via reqwest in
//! `embedding_external.rs`) genuinely omits the header when keyless.

use super::traits::{
    LlmCapabilities, LlmChatRequest, LlmChatResponse, LlmChunkStream, LlmMessage, LlmProvider,
    LlmRole, LlmStreamChunk, LlmUsage,
};
use super::{AiProviderRuntime, AiRequestFormat};
use anyhow::Result;

// rig types used by the PURE mappers (below) are always compiled so the mappers — and their
// mutation-hardening unit tests — exist in test/coverage builds. The live client/model types stay
// behind `cfg(not(any(test, coverage)))` because they need a real network model.
use rig::OneOrMany;
use rig::completion::{
    AssistantContent, GetTokenUsage, Message as RigMessage, ToolDefinition, Usage as RigUsage,
};
use rig::streaming::StreamedAssistantContent;

#[cfg(not(any(test, coverage)))]
use {
    rig::client::CompletionClient,
    rig::completion::CompletionModel,
    rig::providers::{anthropic, gemini, openai},
    std::pin::Pin,
    std::task::{Context as TaskContext, Poll},
};

/// The single PathKeep `LlmProvider` implementation backed by rig.
///
/// Holds the resolved provider runtime (config + secret). Every method branches on
/// `config.request_format` internally so callers above the boundary never see rig.
pub struct RigLlmProvider {
    runtime: AiProviderRuntime,
}

impl RigLlmProvider {
    /// Wraps a resolved provider runtime as a chat provider.
    ///
    /// The runtime already carries the validated config and the in-memory secret, so the
    /// adapter never reaches back into Settings or the keyring.
    pub fn new(runtime: AiProviderRuntime) -> Self {
        Self { runtime }
    }

    /// Returns the configured model id (a runtime string, never a product constant).
    pub fn model_id(&self) -> &str {
        &self.runtime.config.default_model
    }
}

/// Derives the in-engine capability view from the provider config (honest, not optimistic).
///
/// Shared by `capabilities()` and the connection probe so both report the same thing. Every
/// supported request format streams chat in rig 0.34; `max_context_tokens` stays `None` because
/// rig exposes no window size and PathKeep never hardcodes one (D4).
///
/// Tool-call honesty (W-AI-7, 02 §B): the NATIVE adapters (Anthropic, Gemini) reliably encode
/// tool calling, so `tool_call`/`structured_output` are `true`. The OpenAI-compat FLOOR
/// (OpenAI/Ollama/LM Studio) is heterogeneous — many small local models behind the same shape do
/// NOT actually honor a tools payload — so capabilities are reported as `false` here and the agent
/// harness must call [`probe_tool_capability`] (a real, classified probe) before sending tools.
/// This replaces the previous optimistic advertisement that claimed tool support for every floor
/// provider unconditionally.
pub(super) fn rig_llm_capabilities(config: &super::AiProviderConfig) -> LlmCapabilities {
    let interactive_chat = matches!(
        config.request_format,
        AiRequestFormat::OpenAi
            | AiRequestFormat::Anthropic
            | AiRequestFormat::Google
            | AiRequestFormat::Ollama
            | AiRequestFormat::LmStudio
    );
    let native_tool_calling =
        matches!(config.request_format, AiRequestFormat::Anthropic | AiRequestFormat::Google);
    // Anthropic and Gemini native adapters support prefix/prompt caching; the local OpenAI-compat
    // floor does not advertise it through rig.
    let prompt_cache = native_tool_calling;
    LlmCapabilities {
        // Honest floor: only native adapters self-certify tool calling; the OpenAI-compat floor
        // must be probed (see `probe_tool_capability`) before the harness trusts it.
        tool_call: native_tool_calling,
        structured_output: native_tool_calling,
        streaming: interactive_chat,
        prompt_cache,
        max_context_tokens: None,
    }
}

/// Whether a request format self-certifies tool calling without a runtime probe.
///
/// The native Anthropic/Gemini adapters encode tool calls reliably (02 §B), and so does every
/// OpenAI-compat Chat Completions floor (OpenAI/Ollama/LM Studio): that wire shape RELIABLY accepts
/// a `tools` payload, and a budgeted model (e.g. gemma via LM Studio) does call tools — the live
/// agent e2e proves it. The earlier fragile 16-token probe false-negatived reasoning models (they
/// spend the budget thinking and return empty/length, which the catch-all mis-read as "no tool
/// support"), forcing gemma to run TOOL-LESS in production. Self-certifying the floor skips that
/// probe entirely; the harness still degrades gracefully at runtime if a loop genuinely yields no
/// tool calls. Pure + cheap so the harness can ask before deciding whether to probe.
pub(super) fn request_format_self_certifies_tools(format: &AiRequestFormat) -> bool {
    // Exhaustive (no `matches!` wildcard) so every arm is a reachable, covered region and a NEW
    // request format must consciously declare its tool-calling story rather than defaulting in.
    match format {
        AiRequestFormat::Anthropic
        | AiRequestFormat::Google
        | AiRequestFormat::OpenAi
        | AiRequestFormat::LmStudio
        | AiRequestFormat::Ollama => true,
    }
}

/// Resolves whether the provider can execute tool calls, probing the OpenAI-compat floor.
///
/// The agent harness calls this BEFORE attaching a tools payload (02 §B degradation rule). Every
/// supported request format now self-certifies (see [`request_format_self_certifies_tools`]), so in
/// practice this short-circuits to `true` with no network call — the floor's Chat Completions shape
/// reliably accepts a tools payload and a budgeted model calls tools (the live e2e proves it).
///
/// The probe below remains as a robust safety net (e.g. for a future probe-required format). It
/// issues one chat turn carrying a single trivial tool definition with a REAL token budget (512, not
/// the old 16 — a reasoning model truncated at 16 returns empty/length, which is NOT evidence of "no
/// tool support"). The verdict is biased toward CAPABLE: only a transport/auth failure is surfaced
/// as an error (so the caller never mistakes "the endpoint is down" for "no tool support"); a genuine
/// tools-REJECTION (the endpoint explicitly refuses the tools payload) degrades to `false`; and any
/// other failure — truncation/length/empty/generic provider-error — is treated as CAPABLE, letting
/// the harness degrade gracefully at runtime if the loop genuinely yields no tool calls. The outcome
/// is cached per provider config id so a multi-turn run probes at most once.
pub async fn probe_tool_capability(provider: &RigLlmProvider) -> Result<bool> {
    if request_format_self_certifies_tools(&provider.runtime.config.request_format)
        && !forces_a_probe_round_trip(&provider.runtime.config.id)
    {
        return Ok(true);
    }
    probe_floor_tool_capability(&provider.runtime).await
}

/// Whether a config id opts OUT of the self-cert short-circuit so the floor probe still runs.
///
/// Every shipping format self-certifies, so the public probe never reaches [`probe_floor_tool_capability`]
/// in production (a real config id is an opaque mint, never this marker). The worker's defensive
/// probe-error arm (a probe `Err` aborts the run + records a failed header) still needs an honest
/// path to exercise it; a test/coverage fixture sets a `probe-roundtrip` id to force the round-trip
/// and drive that arm. Always `false` in a real build, so production LM Studio config skips the probe
/// exactly as documented.
#[cfg(any(test, coverage))]
fn forces_a_probe_round_trip(provider_id: &str) -> bool {
    provider_id.contains("probe-roundtrip")
}

/// In a real build no id forces a probe — every shipping format self-certifies and skips it.
#[cfg(not(any(test, coverage)))]
fn forces_a_probe_round_trip(_provider_id: &str) -> bool {
    false
}

/// Runs the real, classified tool-capability probe for a non-self-certifying format.
///
/// Split out from [`probe_tool_capability`] so the network-bound probe body (and its classification
/// arms) stays exercisable: with every shipping format now self-certifying, the public entry never
/// reaches this — but a future probe-required format would, and the unit tests drive it directly.
/// Caches per provider config id so a multi-turn run probes at most once. See the caller's doc for
/// the capable-biased verdict rules.
async fn probe_floor_tool_capability(runtime: &AiProviderRuntime) -> Result<bool> {
    if let Some(cached) = tool_capability_cache_get(&runtime.config.id) {
        return Ok(cached);
    }
    let probe_request = LlmChatRequest {
        messages: vec![LlmMessage::new(LlmRole::User, "Reply with OK.")],
        temperature: Some(0.0),
        // A real budget so a reasoning model can finish thinking and actually respond instead of
        // truncating mid-thought (the old 16-token budget false-negatived reasoning models).
        max_tokens: Some(512),
        tools: vec![super::traits::LlmToolDef {
            name: "pathkeep_probe".to_string(),
            description: "Capability probe; do not call.".to_string(),
            parameters: serde_json::json!({ "type": "object", "properties": {} }),
        }],
        response_format: None,
    };
    let capable = match chat_impl(runtime, probe_request).await {
        Ok(_) => true,
        Err(error) => {
            let message = error.to_string();
            let (code, _, _) = super::provider::classify_provider_error(&message);
            // A reachability/auth failure is not evidence of "no tool support" — surface it so the
            // run fails honestly rather than silently degrading on a transient outage.
            if matches!(code.as_deref(), Some("network-error") | Some("secret-missing")) {
                return Err(error);
            }
            // Only a GENUINE tools-rejection (the endpoint refusing the tools payload) is evidence
            // of no tool support. Truncation/length/empty/generic provider-error is NOT, so default
            // to CAPABLE and let the harness degrade at runtime if the loop yields no tool calls.
            !error_rejects_tools(&message)
        }
    };
    tool_capability_cache_put(&runtime.config.id, capable);
    Ok(capable)
}

/// Whether a provider error message is a GENUINE tools-rejection (vs an unrelated failure).
///
/// An OpenAI-compat endpoint that does not honor tools rejects the `tools` payload with a message
/// naming tools/function calling explicitly. This is the only error class that proves "no tool
/// support" — everything else (a length/truncation/empty completion, a generic provider error) is
/// NOT, so the probe must NOT read those as incapable. Kept narrow on purpose: a false positive
/// here would wrongly drop a tool-capable model onto the degraded path.
fn error_rejects_tools(message: &str) -> bool {
    let normalized = message.to_lowercase();
    let mentions_tools = normalized.contains("tool") || normalized.contains("function call");
    let mentions_refusal = normalized.contains("not support")
        || normalized.contains("unsupported")
        || normalized.contains("does not support")
        || normalized.contains("not allowed")
        || normalized.contains("not enabled")
        || normalized.contains("no tools");
    mentions_tools && mentions_refusal
}

/// Process-global cache of probe outcomes, keyed by provider config id.
///
/// Tool capability is a stable property of one configured provider within a process run, so a
/// run-spanning probe should happen at most once. Cleared implicitly when the process restarts (a
/// config change mints a new provider id via Settings, so a stale entry never masks a real change).
fn tool_capability_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, bool>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, bool>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Returns a cached probe outcome for one provider config id, if any.
fn tool_capability_cache_get(provider_id: &str) -> Option<bool> {
    tool_capability_cache()
        .lock()
        .expect("tool capability cache poisoned")
        .get(provider_id)
        .copied()
}

/// Records a probe outcome for one provider config id.
fn tool_capability_cache_put(provider_id: &str, capable: bool) {
    tool_capability_cache()
        .lock()
        .expect("tool capability cache poisoned")
        .insert(provider_id.to_string(), capable);
}

impl LlmProvider for RigLlmProvider {
    fn chat(
        &self,
        req: LlmChatRequest,
    ) -> impl std::future::Future<Output = Result<LlmChatResponse>> + Send {
        chat_impl(&self.runtime, req)
    }

    fn chat_stream(
        &self,
        req: LlmChatRequest,
    ) -> impl std::future::Future<Output = Result<LlmChunkStream>> + Send {
        chat_stream_impl(&self.runtime, req)
    }

    fn capabilities(&self) -> LlmCapabilities {
        rig_llm_capabilities(&self.runtime.config)
    }
}

// ---------------------------------------------------------------------------
// Pure rig <-> PathKeep mappers.
//
// These are ALWAYS compiled (even in test/coverage) so a swap like
// `prompt_tokens`/`completion_tokens` in `to_llm_usage` or a `Reasoning`/`Token`
// flip in `map_streamed_content` is caught by the unit tests + mutation gate
// instead of shipping silently. The network-bound helpers (clients, models,
// request builder, streaming wrapper) remain `cfg(not(any(test, coverage)))`.
// ---------------------------------------------------------------------------

/// Converts one PathKeep message into a rig `Message`.
///
/// Tool results map onto rig's `tool_result`, preserving the call id correlation when the
/// caller supplied one. System/User/Assistant map onto the matching rig constructors.
fn to_rig_message(message: &LlmMessage) -> RigMessage {
    match message.role {
        LlmRole::System => RigMessage::system(message.content.clone()),
        LlmRole::User => RigMessage::user(message.content.clone()),
        LlmRole::Assistant if !message.tool_calls.is_empty() => {
            // An assistant turn that requested tool calls must be threaded back carrying those calls
            // so an OpenAI-compatible transport can correlate the following `tool` result(s) to it
            // (W-AI-7 tool-loop fix). Optional leading text is preserved; each call maps onto rig's
            // `AssistantContent::ToolCall`. Malformed arguments degrade to `null` (the call shape is
            // still threaded so the result correlates) rather than dropping the turn.
            let mut contents: Vec<AssistantContent> = Vec::new();
            if !message.content.is_empty() {
                contents.push(AssistantContent::text(message.content.clone()));
            }
            for call in &message.tool_calls {
                let arguments =
                    serde_json::from_str(&call.arguments).unwrap_or(serde_json::Value::Null);
                contents.push(AssistantContent::tool_call(
                    call.call_id.clone(),
                    call.name.clone(),
                    arguments,
                ));
            }
            // `contents` is non-empty (there is at least one tool call), so the OneOrMany builds.
            RigMessage::Assistant {
                id: None,
                content: OneOrMany::many(contents).expect("at least one tool call content"),
            }
        }
        LlmRole::Assistant => RigMessage::assistant(message.content.clone()),
        LlmRole::Tool => {
            let call_id = message.tool_call_id.clone().unwrap_or_default();
            RigMessage::tool_result(call_id, message.content.clone())
        }
    }
}

/// Maps PathKeep tool definitions onto rig `ToolDefinition`s.
fn to_rig_tools(req: &LlmChatRequest) -> Vec<ToolDefinition> {
    req.tools
        .iter()
        .map(|tool| ToolDefinition {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.parameters.clone(),
        })
        .collect()
}

/// Builds a rig completion request from a PathKeep chat request against one model.
///
/// The newest message is the prompt; everything before it is chat history. Temperature,
/// max_tokens, tools, and (optional) structured-output schema are all threaded through.
#[cfg(not(any(test, coverage)))]
fn build_rig_request<M: CompletionModel>(
    model: M,
    runtime: &AiProviderRuntime,
    req: &LlmChatRequest,
) -> Result<rig::completion::CompletionRequest> {
    let Some((last, history)) = req.messages.split_last() else {
        anyhow::bail!("Chat request must contain at least one message.");
    };
    let mut builder = model
        .completion_request(to_rig_message(last))
        .messages(history.iter().map(to_rig_message))
        .temperature(req.temperature.unwrap_or(runtime.config.temperature.unwrap_or(0.2)) as f64)
        .max_tokens(req.max_tokens.unwrap_or(runtime.config.max_tokens.unwrap_or(1200)) as u64)
        .tools(to_rig_tools(req));
    if let Some(format) = req.response_format.as_ref() {
        // `Schema` is the same `schemars` 1.x instance rig resolves to (see Cargo.toml note).
        // `TryFrom<Value>` rejects a non-object schema, which surfaces here as a hard error
        // rather than silently dropping the structured-output intent.
        let schema = schemars::Schema::try_from(format.schema.clone()).map_err(|error| {
            anyhow::anyhow!(
                "structured-output schema '{}' is not a valid JSON-Schema object: {error}",
                format.schema_name
            )
        })?;
        builder = builder.output_schema(schema);
    }
    Ok(builder.build())
}

/// Pulls the visible text + separated reasoning out of a finished rig completion choice.
fn collect_assistant_content(choice: OneOrMany<AssistantContent>) -> (String, Option<String>) {
    let mut text = String::new();
    let mut reasoning = String::new();
    for item in choice.into_iter() {
        match item {
            AssistantContent::Text(value) => text.push_str(value.text()),
            AssistantContent::Reasoning(value) => reasoning.push_str(&value.display_text()),
            AssistantContent::ToolCall(_) | AssistantContent::Image(_) => {}
        }
    }
    let reasoning = if reasoning.is_empty() { None } else { Some(reasoning) };
    (text, reasoning)
}

/// Converts rig token usage into PathKeep's prompt/completion accounting.
fn to_llm_usage(usage: RigUsage) -> Option<LlmUsage> {
    if usage.input_tokens == 0 && usage.output_tokens == 0 && usage.total_tokens == 0 {
        return None;
    }
    Some(LlmUsage { prompt_tokens: usage.input_tokens, completion_tokens: usage.output_tokens })
}

/// Runs one non-streaming completion against a concrete rig model and maps the result.
#[cfg(not(any(test, coverage)))]
async fn complete_with_model<M: CompletionModel>(
    model: M,
    runtime: &AiProviderRuntime,
    req: &LlmChatRequest,
) -> Result<LlmChatResponse> {
    let request = build_rig_request(model.clone(), runtime, req)?;
    let response = model.completion(request).await?;
    let (text, reasoning) = collect_assistant_content(response.choice);
    Ok(LlmChatResponse { text, reasoning, usage: to_llm_usage(response.usage) })
}

/// Maps one streaming rig item onto an optional PathKeep stream chunk.
///
/// Reasoning (full block and delta) maps to `Reasoning` so thinking-heavy models such as
/// gemma render in the reasoning lane; text maps to `Token`; complete tool calls map to
/// `ToolCall` carrying the provider call id (so the agent harness can correlate the executed
/// result back to its call). The terminal `Final` marker maps to `Usage` when the provider
/// reports token counts — the agent budget loop (W-AI-7, 02 §F) sums these. Tool-call deltas
/// are still dropped (rig aggregates them into the complete `ToolCall`); a `Final` with no usage
/// (or all-zero counts) yields `None` exactly as before so plain chat is unaffected.
///
/// The `GetTokenUsage` bound is what lets `Final` surface usage; it holds for every concrete rig
/// streaming response type (and `()`), so it does not constrain any real call site.
fn map_streamed_content<R: Clone + Unpin + GetTokenUsage>(
    item: StreamedAssistantContent<R>,
) -> Option<LlmStreamChunk> {
    match item {
        StreamedAssistantContent::Text(text) => Some(LlmStreamChunk::Token(text.text)),
        StreamedAssistantContent::Reasoning(reasoning) => {
            Some(LlmStreamChunk::Reasoning(reasoning.display_text()))
        }
        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
            Some(LlmStreamChunk::Reasoning(reasoning))
        }
        StreamedAssistantContent::ToolCall { tool_call, internal_call_id, .. } => {
            // Prefer the provider's own id; fall back to rig's internal correlation id so a
            // ToolResult can still be threaded back even on transports that omit one.
            let call_id =
                if tool_call.id.is_empty() { internal_call_id } else { tool_call.id.clone() };
            Some(LlmStreamChunk::ToolCall {
                call_id,
                name: tool_call.function.name,
                arguments: tool_call.function.arguments.to_string(),
            })
        }
        StreamedAssistantContent::Final(response) => {
            response.token_usage().and_then(|usage| to_llm_usage(usage).map(LlmStreamChunk::Usage))
        }
        StreamedAssistantContent::ToolCallDelta { .. } => None,
    }
}

/// A `Stream` that lifts rig's `StreamingCompletionResponse` into PathKeep chunks.
///
/// Hand-rolled on `futures_core::Stream` (already a dependency) so the adapter does not pull
/// the full `futures` crate just for a `filter_map`. Items rig emits that have no PathKeep
/// chunk (tool-call deltas, the final usage marker) are skipped by re-polling; a provider
/// error becomes a single `Err` item so the consumer terminates gracefully.
#[cfg(not(any(test, coverage)))]
struct MappedRigStream<S> {
    inner: S,
}

#[cfg(not(any(test, coverage)))]
impl<S, R> futures_core::Stream for MappedRigStream<S>
where
    S: futures_core::Stream<
            Item = Result<StreamedAssistantContent<R>, rig::completion::CompletionError>,
        > + Unpin,
    R: Clone + Unpin + GetTokenUsage,
{
    type Item = Result<LlmStreamChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(error))) => {
                    return Poll::Ready(Some(Err(anyhow::Error::new(error))));
                }
                Poll::Ready(Some(Ok(content))) => match map_streamed_content(content) {
                    Some(chunk) => return Poll::Ready(Some(Ok(chunk))),
                    // No PathKeep chunk for this rig item — keep draining.
                    None => continue,
                },
            }
        }
    }
}

/// Starts a streaming completion against a concrete rig model and boxes the mapped stream.
///
/// A terminal provider error is surfaced as a stream `Err` item (not a panic) so the IPC
/// layer can encode it as an `error` chunk.
#[cfg(not(any(test, coverage)))]
async fn stream_with_model<M>(
    model: M,
    runtime: &AiProviderRuntime,
    req: &LlmChatRequest,
) -> Result<LlmChunkStream>
where
    M: CompletionModel + 'static,
{
    let request = build_rig_request(model.clone(), runtime, req)?;
    let stream = model.stream(request).await?;
    Ok(Box::pin(MappedRigStream { inner: stream }))
}

/// Real non-streaming chat: branches to the right rig client by request format.
#[cfg(not(any(test, coverage)))]
async fn chat_impl(runtime: &AiProviderRuntime, req: LlmChatRequest) -> Result<LlmChatResponse> {
    match runtime.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            let client = build_openai_client(runtime)?;
            complete_with_model(
                client.completion_model(runtime.config.default_model.clone()),
                runtime,
                &req,
            )
            .await
        }
        AiRequestFormat::Anthropic => {
            let client = build_anthropic_client(runtime)?;
            complete_with_model(
                client.completion_model(runtime.config.default_model.clone()),
                runtime,
                &req,
            )
            .await
        }
        AiRequestFormat::Google => {
            let client = build_gemini_client(runtime)?;
            complete_with_model(
                client.completion_model(runtime.config.default_model.clone()),
                runtime,
                &req,
            )
            .await
        }
    }
}

/// Real streaming chat: branches to the right rig client by request format.
#[cfg(not(any(test, coverage)))]
async fn chat_stream_impl(
    runtime: &AiProviderRuntime,
    req: LlmChatRequest,
) -> Result<LlmChunkStream> {
    match runtime.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::Ollama | AiRequestFormat::LmStudio => {
            let client = build_openai_client(runtime)?;
            stream_with_model(
                client.completion_model(runtime.config.default_model.clone()),
                runtime,
                &req,
            )
            .await
        }
        AiRequestFormat::Anthropic => {
            let client = build_anthropic_client(runtime)?;
            stream_with_model(
                client.completion_model(runtime.config.default_model.clone()),
                runtime,
                &req,
            )
            .await
        }
        AiRequestFormat::Google => {
            let client = build_gemini_client(runtime)?;
            stream_with_model(
                client.completion_model(runtime.config.default_model.clone()),
                runtime,
                &req,
            )
            .await
        }
    }
}

/// Builds an OpenAI-compatible rig client (covers OpenAI/Ollama/LM Studio).
///
/// Uses the **Chat Completions** client (`CompletionsClient`), NOT rig's default `Client`
/// (which targets the Responses API). LM Studio, Ollama, and llama-server only speak Chat
/// Completions, and gemma's `reasoning_content` deltas only flow on that path — matching the
/// existing `run_llm_agent` choice and 02 §B (OpenAI-compat Chat Completions as the floor).
#[cfg(not(any(test, coverage)))]
fn build_openai_client(runtime: &AiProviderRuntime) -> Result<openai::CompletionsClient> {
    // OPTIONAL key: only a present, non-blank secret is forwarded as the bearer token. A keyless
    // local model (LM Studio / Ollama) carries an empty token — rig 0.34's openai client always
    // emits an `Authorization` header (see the module note on the upstream limitation), but a
    // keyless local endpoint ignores it, and we NEVER block the call on a missing key of our own
    // accord. A key-enforcing cloud server answers with its own 401, which surfaces verbatim.
    let mut builder = openai::CompletionsClient::builder()
        .api_key(runtime.api_key_for_transport().unwrap_or_default());
    if let Some(base_url) = runtime.config.base_url.as_deref() {
        builder = builder.base_url(super::provider::normalize_local_base_url(base_url));
    }
    Ok(builder.build()?)
}

/// Builds an Anthropic native rig client.
#[cfg(not(any(test, coverage)))]
fn build_anthropic_client(runtime: &AiProviderRuntime) -> Result<anthropic::Client> {
    // Anthropic is cloud-only and key-required; an absent key falls through to an empty token so
    // the provider returns its own 401 rather than PathKeep pre-empting the call (see module note).
    let mut builder =
        anthropic::Client::builder().api_key(runtime.api_key_for_transport().unwrap_or_default());
    if let Some(base_url) = runtime.config.base_url.as_deref() {
        builder = builder.base_url(super::provider::normalize_local_base_url(base_url));
    }
    Ok(builder.build()?)
}

/// Builds a Gemini native rig client.
#[cfg(not(any(test, coverage)))]
fn build_gemini_client(runtime: &AiProviderRuntime) -> Result<gemini::Client> {
    // Gemini is cloud-only and key-required; an absent key falls through to an empty token so the
    // provider returns its own 401 rather than PathKeep pre-empting the call (see module note).
    let mut builder =
        gemini::Client::builder().api_key(runtime.api_key_for_transport().unwrap_or_default());
    if let Some(base_url) = runtime.config.base_url.as_deref() {
        builder = builder.base_url(super::provider::normalize_local_base_url(base_url));
    }
    Ok(builder.build()?)
}

// ---------------------------------------------------------------------------
// Deterministic stub (test + coverage). Same public call graph as production:
// `chat` returns stable text/reasoning/usage; `chat_stream` is a TWO-TURN script
// keyed on whether the request already carries a Tool-role message (the harness
// threads a tool_result back as one). Turn 1 (no tool result yet) yields
// Reasoning → Token → ToolCall(call_id); turn 2 (after a tool_result) yields
// Token → Usage with NO tool call, so the agent harness loop terminates. Ids
// tagged "stream-error" surface a mid-stream Err on turn 1 so the Err arm is
// exercised. This is the fixture the W-AI-7 deterministic harness test drives.
// ---------------------------------------------------------------------------

/// Returns a stable, request-derived chat response without any network call.
#[cfg(any(test, coverage))]
async fn chat_impl(runtime: &AiProviderRuntime, req: LlmChatRequest) -> Result<LlmChatResponse> {
    if runtime.config.id.contains("network-error") {
        // A reachability failure (classified `network-error`) so the tool-capability probe
        // surfaces the error honestly instead of degrading to tool-incapable.
        anyhow::bail!("connection refused: the local model endpoint is unreachable");
    }
    if runtime.config.id.contains("tools-rejection") {
        // A GENUINE tools-rejection: the endpoint names tool/function calling and refuses it, the
        // only error class that proves "no tool support" (probe degrades to tool-incapable).
        anyhow::bail!("this model does not support tool calling / function calls");
    }
    if runtime.config.id.contains("chat-error") {
        anyhow::bail!("forced coverage chat error");
    }
    let format_tag = request_format_tag(&runtime.config.request_format);
    let last = req.messages.last().map(|message| message.content.as_str()).unwrap_or_default();
    let reasoning = if req.temperature.is_some() {
        Some(format!("{format_tag} considered {} message(s)", req.messages.len()))
    } else {
        None
    };
    Ok(LlmChatResponse {
        text: format!("{format_tag} answer to '{last}' with {} tool(s)", req.tools.len()),
        reasoning,
        usage: Some(LlmUsage {
            prompt_tokens: req.messages.len() as u64,
            completion_tokens: req.tools.len() as u64 + 1,
        }),
    })
}

/// Returns the deterministic two-turn chunk stream that the agent harness loop drives.
///
/// Turn detection is purely structural: if the request already contains a `Tool`-role message the
/// harness has threaded a tool result back, so this is the SECOND turn and the stub answers with
/// `Token → Usage` and NO tool call (the loop then emits `Done`). Otherwise it is the FIRST turn
/// and the stub yields `Reasoning → Token → ToolCall(call_id)`. Ids containing "stream-error"
/// surface a mid-stream Err on turn 1 (the Err arm); the eager open-error path is unchanged.
#[cfg(any(test, coverage))]
async fn chat_stream_impl(
    runtime: &AiProviderRuntime,
    req: LlmChatRequest,
) -> Result<LlmChunkStream> {
    if runtime.config.id.contains("stream-open-error") {
        anyhow::bail!("forced coverage stream open error");
    }
    let format_tag = request_format_tag(&runtime.config.request_format).to_string();
    let message_count = req.messages.len();
    let tool_result_threaded =
        req.messages.iter().any(|message| message.role == super::traits::LlmRole::Tool);
    let chunks: Vec<Result<LlmStreamChunk>> = if tool_result_threaded {
        // Second turn: the model has the tool evidence, so it answers and reports usage.
        vec![
            Ok(LlmStreamChunk::Token(format!("{format_tag} final:{message_count}"))),
            Ok(LlmStreamChunk::Usage(LlmUsage {
                prompt_tokens: message_count as u64,
                completion_tokens: 2,
            })),
        ]
    } else {
        // First turn: think, emit a token, then request a tool call (with a stable call id).
        let mut chunks = vec![
            Ok(LlmStreamChunk::Reasoning(format!("{format_tag} thinking"))),
            Ok(LlmStreamChunk::Token(format!("{format_tag}:{message_count}"))),
            Ok(LlmStreamChunk::ToolCall {
                call_id: "call-1".to_string(),
                name: "search_history".to_string(),
                arguments: r#"{"query":"example"}"#.to_string(),
            }),
        ];
        if runtime.config.id.contains("stream-error") {
            chunks.push(Err(anyhow::anyhow!("forced coverage mid-stream error")));
        }
        chunks
    };
    Ok(Box::pin(stub_stream(chunks)))
}

/// Stable lowercase label for one request format, shared by the stub paths.
#[cfg(any(test, coverage))]
fn request_format_tag(format: &AiRequestFormat) -> &'static str {
    match format {
        AiRequestFormat::OpenAi => "openai",
        AiRequestFormat::Ollama => "ollama",
        AiRequestFormat::LmStudio => "lmstudio",
        AiRequestFormat::Anthropic => "anthropic",
        AiRequestFormat::Google => "google",
    }
}

/// A minimal always-ready owned stream so the stub does not depend on futures-util.
#[cfg(any(test, coverage))]
fn stub_stream(
    chunks: Vec<Result<LlmStreamChunk>>,
) -> impl futures_core::Stream<Item = Result<LlmStreamChunk>> + Send {
    use std::pin::Pin;
    use std::task::{Context, Poll};
    struct StubStream(std::vec::IntoIter<Result<LlmStreamChunk>>);
    impl futures_core::Stream for StubStream {
        type Item = Result<LlmStreamChunk>;
        fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.0.next())
        }
    }
    StubStream(chunks.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::traits::{LlmMessage, LlmRole, LlmToolCall, LlmToolDef};
    use crate::models::{AiProviderConfig, AiProviderPurpose};
    use secrecy::SecretString;

    fn runtime(format: AiRequestFormat, id: &str) -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: id.to_string(),
                name: "Test Provider".to_string(),
                purpose: AiProviderPurpose::Llm,
                request_format: format,
                enabled: true,
                default_model: "test-model".to_string(),
                temperature: Some(0.4),
                max_tokens: Some(256),
                ..AiProviderConfig::default()
            },
            api_key: Some(SecretString::from("test-key".to_string())),
        }
    }

    fn request_with_tools() -> LlmChatRequest {
        LlmChatRequest {
            messages: vec![
                LlmMessage::new(LlmRole::System, "be helpful"),
                LlmMessage::new(LlmRole::User, "what did i read?"),
            ],
            temperature: Some(0.6),
            max_tokens: Some(128),
            tools: vec![LlmToolDef {
                name: "search".to_string(),
                description: "search history".to_string(),
                parameters: serde_json::json!({ "type": "object" }),
            }],
            response_format: None,
        }
    }

    async fn drain(mut stream: LlmChunkStream) -> (Vec<LlmStreamChunk>, usize) {
        use std::future::poll_fn;
        let mut chunks = Vec::new();
        let mut errors = 0usize;
        while let Some(slot) = poll_fn(|cx| stream.as_mut().poll_next(cx)).await {
            match slot {
                Ok(chunk) => chunks.push(chunk),
                Err(_) => errors += 1,
            }
        }
        (chunks, errors)
    }

    #[tokio::test]
    async fn chat_returns_format_tagged_text_reasoning_and_usage() {
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::LmStudio, "llm-ok"));
        assert_eq!(provider.model_id(), "test-model");
        let response = provider.chat(request_with_tools()).await.expect("chat");
        assert_eq!(response.text, "lmstudio answer to 'what did i read?' with 1 tool(s)");
        assert_eq!(response.reasoning.as_deref(), Some("lmstudio considered 2 message(s)"));
        assert_eq!(response.usage, Some(LlmUsage { prompt_tokens: 2, completion_tokens: 2 }));
    }

    #[tokio::test]
    async fn chat_omits_reasoning_when_temperature_absent() {
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::OpenAi, "llm-no-temp"));
        let request = LlmChatRequest::new(vec![LlmMessage::new(LlmRole::User, "hi")], None, None);
        let response = provider.chat(request).await.expect("chat");
        assert_eq!(response.reasoning, None);
        assert_eq!(response.text, "openai answer to 'hi' with 0 tool(s)");
    }

    #[tokio::test]
    async fn chat_surfaces_forced_error() {
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::Anthropic, "chat-error"));
        let error = provider
            .chat(LlmChatRequest::new(vec![LlmMessage::new(LlmRole::User, "x")], None, None))
            .await
            .expect_err("forced error");
        assert!(error.to_string().contains("forced coverage chat error"));
    }

    #[tokio::test]
    async fn chat_stream_turn_one_yields_reasoning_token_and_tool_call() {
        // Turn 1 of the two-turn stub: the request carries no Tool-role message, so the stub asks
        // for a tool call (with a stable call id) after thinking + a token.
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::Google, "stream-ok"));
        let stream = provider.chat_stream(request_with_tools()).await.expect("stream");
        let (chunks, errors) = drain(stream).await;
        assert_eq!(errors, 0);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], LlmStreamChunk::Reasoning("google thinking".to_string()));
        assert_eq!(chunks[1], LlmStreamChunk::Token("google:2".to_string()));
        assert!(matches!(
            &chunks[2],
            LlmStreamChunk::ToolCall { name, call_id, .. }
                if name == "search_history" && call_id == "call-1"
        ));
    }

    #[tokio::test]
    async fn chat_stream_turn_two_yields_token_and_usage_no_tool_call() {
        // Turn 2: once a Tool-role result is threaded back, the stub answers with a token + a Usage
        // marker and NO tool call, so the agent harness loop terminates.
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::Google, "stream-ok"));
        let request = LlmChatRequest {
            messages: vec![
                LlmMessage::new(LlmRole::User, "what did i read?"),
                LlmMessage::new(LlmRole::Assistant, "let me search"),
                LlmMessage::tool_result("call-1", "search_history", "3 rows"),
            ],
            temperature: Some(0.2),
            max_tokens: Some(128),
            tools: Vec::new(),
            response_format: None,
        };
        let stream = provider.chat_stream(request).await.expect("stream");
        let (chunks, errors) = drain(stream).await;
        assert_eq!(errors, 0);
        assert_eq!(chunks.len(), 2);
        assert!(matches!(&chunks[0], LlmStreamChunk::Token(text) if text == "google final:3"));
        assert!(matches!(
            chunks[1],
            LlmStreamChunk::Usage(LlmUsage { prompt_tokens: 3, completion_tokens: 2 })
        ));
        assert!(
            !chunks.iter().any(|chunk| matches!(chunk, LlmStreamChunk::ToolCall { .. })),
            "turn 2 must not request a tool call"
        );
    }

    #[tokio::test]
    async fn chat_stream_surfaces_terminal_error_as_stream_item() {
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::Ollama, "stream-error"));
        let stream = provider.chat_stream(request_with_tools()).await.expect("stream");
        let (chunks, errors) = drain(stream).await;
        assert_eq!(chunks.len(), 3);
        assert_eq!(errors, 1);
    }

    #[tokio::test]
    async fn chat_stream_open_error_is_returned_eagerly() {
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::OpenAi, "stream-open-error"));
        let result = provider
            .chat_stream(LlmChatRequest::new(vec![LlmMessage::new(LlmRole::User, "x")], None, None))
            .await;
        // The Ok variant (a boxed stream) is not `Debug`, so match instead of `expect_err`.
        match result {
            Ok(_) => panic!("expected eager open error"),
            Err(error) => {
                assert!(error.to_string().contains("forced coverage stream open error"));
            }
        }
    }

    #[test]
    fn capabilities_reflect_request_format() {
        // Honest floor: the OpenAI-compat floor streams but does NOT self-certify tool calling /
        // structured output — it is probe-required (W-AI-7).
        let openai = RigLlmProvider::new(runtime(AiRequestFormat::OpenAi, "caps-openai"));
        let caps = openai.capabilities();
        assert!(caps.streaming);
        assert!(!caps.tool_call, "floor must not optimistically claim tool calling");
        assert!(!caps.structured_output);
        assert!(!caps.prompt_cache);
        assert_eq!(caps.max_context_tokens, None);

        // Native adapters self-certify tool calling + structured output + prompt caching.
        let anthropic = RigLlmProvider::new(runtime(AiRequestFormat::Anthropic, "caps-anthropic"));
        let anthropic_caps = anthropic.capabilities();
        assert!(anthropic_caps.tool_call && anthropic_caps.structured_output);
        assert!(anthropic_caps.prompt_cache);

        let gemini = RigLlmProvider::new(runtime(AiRequestFormat::Google, "caps-gemini"));
        let gemini_caps = gemini.capabilities();
        assert!(gemini_caps.tool_call && gemini_caps.structured_output);
        assert!(gemini_caps.prompt_cache);
    }

    #[test]
    fn request_format_self_certification_covers_every_shipping_format() {
        // F2 layer 3: the native adapters AND the OpenAI-compat Chat Completions floor all reliably
        // ACCEPT a tools payload, so every shipping format self-certifies and skips the fragile probe.
        assert!(request_format_self_certifies_tools(&AiRequestFormat::Anthropic));
        assert!(request_format_self_certifies_tools(&AiRequestFormat::Google));
        assert!(request_format_self_certifies_tools(&AiRequestFormat::OpenAi));
        assert!(request_format_self_certifies_tools(&AiRequestFormat::Ollama));
        assert!(request_format_self_certifies_tools(&AiRequestFormat::LmStudio));
    }

    #[tokio::test]
    async fn probe_tool_capability_short_circuits_for_self_certifying_formats() {
        // Both a native adapter and an LM Studio floor provider self-certify, so the public probe
        // returns capable with no network round-trip and never caches (the floor skips the probe).
        let anthropic = RigLlmProvider::new(runtime(AiRequestFormat::Anthropic, "probe-native"));
        assert!(probe_tool_capability(&anthropic).await.expect("native probe"));
        let lmstudio = RigLlmProvider::new(runtime(AiRequestFormat::LmStudio, "probe-lmstudio"));
        assert!(probe_tool_capability(&lmstudio).await.expect("lmstudio self-certifies"));
        assert_eq!(
            tool_capability_cache_get("probe-lmstudio"),
            None,
            "a self-certifying format never runs (or caches) a probe"
        );
    }

    #[tokio::test]
    async fn floor_probe_treats_a_successful_turn_as_capable() {
        // The floor probe, driven through the PUBLIC entry via a `probe-roundtrip` id that opts out
        // of the self-cert short-circuit (every shipping format self-certifies in production): the
        // stub chat succeeds, so the probe reports capable and caches.
        let provider =
            RigLlmProvider::new(runtime(AiRequestFormat::OpenAi, "probe-roundtrip-floor-ok"));
        assert!(probe_tool_capability(&provider).await.expect("floor probe"));
        // Cached: a second call returns the same outcome without re-running.
        assert_eq!(tool_capability_cache_get("probe-roundtrip-floor-ok"), Some(true));
        assert!(probe_tool_capability(&provider).await.expect("cached floor probe"));
    }

    #[tokio::test]
    async fn floor_probe_treats_a_generic_failure_as_capable() {
        // F2 layer 2: a forced chat error that is NOT a tools-rejection / transport failure (a
        // generic provider-error, e.g. a truncated/length/empty completion) is NOT evidence of "no
        // tool support", so the probe defaults to CAPABLE and lets the harness degrade at runtime.
        let provider =
            RigLlmProvider::new(runtime(AiRequestFormat::OpenAi, "probe-roundtrip-chat-error"));
        assert!(probe_tool_capability(&provider).await.expect("capable-biased probe"));
        assert_eq!(tool_capability_cache_get("probe-roundtrip-chat-error"), Some(true));
    }

    #[tokio::test]
    async fn floor_probe_degrades_only_on_a_genuine_tools_rejection() {
        // A GENUINE tools-rejection (the endpoint naming + refusing tool/function calling) is the
        // only error class that proves "no tool support", so the probe degrades to incapable.
        let provider = RigLlmProvider::new(runtime(
            AiRequestFormat::OpenAi,
            "probe-roundtrip-tools-rejection",
        ));
        assert!(!probe_tool_capability(&provider).await.expect("tools-rejection probe"));
        assert_eq!(tool_capability_cache_get("probe-roundtrip-tools-rejection"), Some(false));
    }

    #[tokio::test]
    async fn floor_probe_surfaces_a_transport_failure_as_an_error() {
        // A reachability failure (classified `network-error`) is NOT evidence of "no tool support",
        // so the probe returns the error instead of silently degrading — and never caches a verdict.
        let provider =
            RigLlmProvider::new(runtime(AiRequestFormat::OpenAi, "probe-roundtrip-network-error"));
        let error = probe_tool_capability(&provider).await.expect_err("transport failure surfaces");
        assert!(error.to_string().contains("connection refused"));
        assert_eq!(
            tool_capability_cache_get("probe-roundtrip-network-error"),
            None,
            "a transport failure must not cache a capability verdict"
        );
    }

    #[test]
    fn error_rejects_tools_matches_only_a_genuine_tools_refusal() {
        // Pins the narrow tools-rejection classifier: a message naming tool/function calling AND a
        // refusal is a rejection; an unrelated failure (or a tools mention without a refusal) is NOT.
        assert!(error_rejects_tools("This model does not support tool calling"));
        assert!(error_rejects_tools("tools are not allowed for this endpoint"));
        assert!(error_rejects_tools("function call is unsupported here"));
        // A truncation/length/empty/generic error is not a tools-rejection (defaults to capable).
        assert!(!error_rejects_tools("finish_reason: length"));
        assert!(!error_rejects_tools("forced coverage chat error"));
        // A tools mention with no refusal, or a refusal with no tools mention, is not a rejection.
        assert!(!error_rejects_tools("the tool ran fine"));
        assert!(!error_rejects_tools("the model is not enabled"));
    }

    #[test]
    fn request_format_tags_are_stable() {
        assert_eq!(request_format_tag(&AiRequestFormat::OpenAi), "openai");
        assert_eq!(request_format_tag(&AiRequestFormat::Ollama), "ollama");
        assert_eq!(request_format_tag(&AiRequestFormat::LmStudio), "lmstudio");
        assert_eq!(request_format_tag(&AiRequestFormat::Anthropic), "anthropic");
        assert_eq!(request_format_tag(&AiRequestFormat::Google), "google");
    }

    // ---------------------------------------------------------------------------
    // Pure-mapper tests (always compiled). These pin the rig <-> PathKeep mapping
    // so a field/variant swap is caught by the unit + mutation gates.
    // ---------------------------------------------------------------------------

    #[test]
    fn to_llm_usage_maps_input_to_prompt_and_output_to_completion() {
        let usage = RigUsage {
            input_tokens: 11,
            output_tokens: 22,
            total_tokens: 33,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        // Pins the field correspondence so an input/output swap fails here.
        assert_eq!(
            to_llm_usage(usage),
            Some(LlmUsage { prompt_tokens: 11, completion_tokens: 22 })
        );
    }

    #[test]
    fn to_llm_usage_returns_none_when_all_counts_are_zero() {
        let usage = RigUsage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        assert_eq!(to_llm_usage(usage), None);
    }

    #[test]
    fn to_llm_usage_keeps_some_when_only_total_is_reported() {
        let usage = RigUsage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 7,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        // total>0 means the provider DID report usage even with split counts at 0.
        assert_eq!(to_llm_usage(usage), Some(LlmUsage { prompt_tokens: 0, completion_tokens: 0 }));
    }

    #[test]
    fn map_streamed_content_maps_text_to_token() {
        let item: StreamedAssistantContent<()> =
            StreamedAssistantContent::Text(rig::completion::message::Text::from("hello"));
        assert_eq!(map_streamed_content(item), Some(LlmStreamChunk::Token("hello".to_string())));
    }

    #[test]
    fn map_streamed_content_maps_reasoning_block_to_reasoning() {
        let item: StreamedAssistantContent<()> =
            StreamedAssistantContent::Reasoning(rig::completion::message::Reasoning::new("think"));
        assert_eq!(
            map_streamed_content(item),
            Some(LlmStreamChunk::Reasoning("think".to_string()))
        );
    }

    #[test]
    fn map_streamed_content_maps_reasoning_delta_to_reasoning() {
        let item: StreamedAssistantContent<()> =
            StreamedAssistantContent::ReasoningDelta { id: None, reasoning: "step".to_string() };
        assert_eq!(map_streamed_content(item), Some(LlmStreamChunk::Reasoning("step".to_string())));
    }

    #[test]
    fn map_streamed_content_maps_tool_call() {
        let tool_call = rig::completion::message::ToolCall::new(
            "call-1".to_string(),
            rig::completion::message::ToolFunction::new(
                "search".to_string(),
                serde_json::json!({ "q": "rust" }),
            ),
        );
        let item: StreamedAssistantContent<()> = StreamedAssistantContent::ToolCall {
            tool_call,
            internal_call_id: "internal".to_string(),
        };
        match map_streamed_content(item) {
            Some(LlmStreamChunk::ToolCall { call_id, name, arguments }) => {
                // The provider call id ("call-1") wins over rig's internal correlation id.
                assert_eq!(call_id, "call-1");
                assert_eq!(name, "search");
                assert_eq!(arguments, serde_json::json!({ "q": "rust" }).to_string());
            }
            other => panic!("expected ToolCall chunk, got {other:?}"),
        }
    }

    #[test]
    fn map_streamed_content_falls_back_to_internal_call_id_when_provider_id_is_empty() {
        // A transport that does not assign a provider call id leaves `tool_call.id` empty; the
        // mapper then uses rig's internal correlation id so the harness can still thread results.
        let tool_call = rig::completion::message::ToolCall::new(
            String::new(),
            rig::completion::message::ToolFunction::new(
                "search".to_string(),
                serde_json::json!({}),
            ),
        );
        let item: StreamedAssistantContent<()> = StreamedAssistantContent::ToolCall {
            tool_call,
            internal_call_id: "internal-7".to_string(),
        };
        match map_streamed_content(item) {
            Some(LlmStreamChunk::ToolCall { call_id, .. }) => assert_eq!(call_id, "internal-7"),
            other => panic!("expected ToolCall chunk, got {other:?}"),
        }
    }

    /// A minimal `GetTokenUsage` fixture so the `Final` → `Usage` mapping is exercised without a
    /// real rig streaming response type (those need a live model). It just reports a fixed usage.
    #[derive(Clone)]
    struct UsageFixture(Option<RigUsage>);
    impl GetTokenUsage for UsageFixture {
        fn token_usage(&self) -> Option<RigUsage> {
            self.0
        }
    }

    #[test]
    fn map_streamed_content_maps_final_with_usage_to_usage_chunk() {
        // A `Final` carrying token usage surfaces as a `Usage` chunk so the agent budget loop can
        // sum it; the input/output → prompt/completion mapping reuses `to_llm_usage`.
        let item: StreamedAssistantContent<UsageFixture> =
            StreamedAssistantContent::Final(UsageFixture(Some(RigUsage {
                input_tokens: 5,
                output_tokens: 9,
                total_tokens: 14,
                cached_input_tokens: 0,
                cache_creation_input_tokens: 0,
            })));
        assert_eq!(
            map_streamed_content(item),
            Some(LlmStreamChunk::Usage(LlmUsage { prompt_tokens: 5, completion_tokens: 9 }))
        );
        // A `Final` whose provider reported no usage yields nothing (plain-chat behaviour held).
        let no_usage: StreamedAssistantContent<UsageFixture> =
            StreamedAssistantContent::Final(UsageFixture(None));
        assert_eq!(map_streamed_content(no_usage), None);
    }

    #[test]
    fn map_streamed_content_drops_final_without_usage_and_tool_call_delta() {
        // `()` reports no usage, so a `Final(())` yields nothing (plain chat is unaffected).
        let final_item: StreamedAssistantContent<()> = StreamedAssistantContent::Final(());
        assert_eq!(map_streamed_content(final_item), None);
        let delta: StreamedAssistantContent<()> = StreamedAssistantContent::ToolCallDelta {
            id: "id".to_string(),
            internal_call_id: "internal".to_string(),
            content: rig::streaming::ToolCallDeltaContent::Delta("{".to_string()),
        };
        assert_eq!(map_streamed_content(delta), None);
    }

    #[test]
    fn to_rig_message_maps_each_role_to_the_matching_rig_variant() {
        // The rig `Message` enum is `#[serde(tag = "role")]`, so the role tag pins the mapping.
        let system = serde_json::to_value(to_rig_message(&LlmMessage::new(LlmRole::System, "s")))
            .expect("serialize system");
        assert_eq!(system["role"], "system");
        assert_eq!(system["content"], "s");

        let user = serde_json::to_value(to_rig_message(&LlmMessage::new(LlmRole::User, "u")))
            .expect("serialize user");
        assert_eq!(user["role"], "user");

        let assistant =
            serde_json::to_value(to_rig_message(&LlmMessage::new(LlmRole::Assistant, "a")))
                .expect("serialize assistant");
        assert_eq!(assistant["role"], "assistant");

        // A tool result becomes a rig user message carrying ToolResult content, with the
        // originating call id preserved.
        let tool = serde_json::to_value(to_rig_message(&LlmMessage::tool_result(
            "call-7", "search", "5 rows",
        )))
        .expect("serialize tool");
        assert_eq!(tool["role"], "user");
        assert_eq!(tool["content"][0]["id"], "call-7");
    }

    #[test]
    fn to_rig_message_threads_assistant_tool_calls_with_text_and_arguments() {
        // An assistant turn carrying tool calls maps to a rig assistant message whose content holds
        // BOTH the optional leading text and the tool call (id + name + parsed arguments), so an
        // OpenAI-compatible transport can correlate the following tool result (W-AI-7 loop fix).
        let message = LlmMessage::assistant_tool_calls(
            "let me look".to_string(),
            vec![LlmToolCall {
                call_id: "call-9".to_string(),
                name: "search_history".to_string(),
                arguments: r#"{"query":""}"#.to_string(),
            }],
        );
        let value = serde_json::to_value(to_rig_message(&message)).expect("serialize assistant");
        assert_eq!(value["role"], "assistant");
        let content = value["content"].as_array().expect("content array");
        assert_eq!(content.len(), 2, "leading text + the tool call: {value}");
        // The tool call carries the provider call id, the tool name, and the PARSED arguments object.
        let call = content
            .iter()
            .find(|c| c.get("function").is_some())
            .expect("a tool-call content entry");
        assert_eq!(call["id"], "call-9");
        assert_eq!(call["function"]["name"], "search_history");
        assert_eq!(call["function"]["arguments"]["query"], "");
    }

    #[test]
    fn to_rig_message_threads_a_tool_only_assistant_turn_without_text() {
        // A reasoning-only turn (no visible text) still threads the tool call — the gemma case that
        // looped before this fix. Malformed arguments degrade to null rather than dropping the call.
        let message = LlmMessage::assistant_tool_calls(
            String::new(),
            vec![LlmToolCall {
                call_id: "call-1".to_string(),
                name: "search_history".to_string(),
                arguments: "not json".to_string(),
            }],
        );
        let value = serde_json::to_value(to_rig_message(&message)).expect("serialize assistant");
        assert_eq!(value["role"], "assistant");
        let content = value["content"].as_array().expect("content array");
        assert_eq!(content.len(), 1, "no text → only the tool call: {value}");
        assert_eq!(content[0]["id"], "call-1");
        assert!(content[0]["function"]["arguments"].is_null(), "bad args degrade to null");
    }

    #[test]
    fn to_rig_message_defaults_missing_tool_call_id_to_empty_string() {
        // A `Tool`-role message with no correlating call id maps to an empty rig tool id rather
        // than panicking.
        let bare = LlmMessage {
            role: LlmRole::Tool,
            content: "x".to_string(),
            tool_call_id: None,
            tool_name: None,
            tool_calls: Vec::new(),
        };
        let value = serde_json::to_value(to_rig_message(&bare)).expect("serialize bare tool");
        assert_eq!(value["role"], "user");
        assert_eq!(value["content"][0]["id"], "");
    }

    #[test]
    fn to_rig_tools_threads_name_description_and_parameters() {
        let request = LlmChatRequest {
            messages: vec![LlmMessage::new(LlmRole::User, "hi")],
            temperature: None,
            max_tokens: None,
            tools: vec![LlmToolDef {
                name: "search".to_string(),
                description: "search history".to_string(),
                parameters: serde_json::json!({ "type": "object" }),
            }],
            response_format: None,
        };
        let tools = to_rig_tools(&request);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "search");
        assert_eq!(tools[0].description, "search history");
        assert_eq!(tools[0].parameters, serde_json::json!({ "type": "object" }));
    }

    #[test]
    fn collect_assistant_content_splits_text_reasoning_and_ignores_tool_call_and_image() {
        let choice = OneOrMany::many(vec![
            AssistantContent::text("answer "),
            AssistantContent::reasoning("because"),
            // A tool call and an image in the same choice are dropped from the (text, reasoning)
            // result — this pins the catch-all arm so it is not silently broadened.
            AssistantContent::tool_call("call-1", "search", serde_json::json!({})),
            AssistantContent::image_base64("aGk=", None, None),
            AssistantContent::text("body"),
        ])
        .expect("non-empty choice");
        let (text, reasoning) = collect_assistant_content(choice);
        assert_eq!(text, "answer body");
        assert_eq!(reasoning.as_deref(), Some("because"));
    }

    #[test]
    fn collect_assistant_content_returns_none_reasoning_when_absent() {
        let choice = OneOrMany::one(AssistantContent::text("only text"));
        let (text, reasoning) = collect_assistant_content(choice);
        assert_eq!(text, "only text");
        assert_eq!(reasoning, None);
    }
}
