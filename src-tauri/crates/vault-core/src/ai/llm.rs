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
//! only file allowed to depend on rig for chat, so the transport stays swappable.
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
use rig::OneOrMany;
use rig::client::CompletionClient;
use rig::completion::{
    AssistantContent, CompletionModel, GetTokenUsage, Message as RigMessage, ToolDefinition,
    Usage as RigUsage,
};
use rig::providers::{anthropic, gemini, openai};
use rig::streaming::StreamedAssistantContent;
use std::pin::Pin;
use std::task::{Context as TaskContext, Poll};

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
/// Tool calls (W-AI-7, 02 §B): `tool_call` is [`request_format_self_certifies_tools`], the same
/// answer [`probe_tool_capability`] gives the agent harness. Only the native adapters (Anthropic,
/// Gemini) report `structured_output`.
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
        // The same answer the agent acts on when it decides to send tools, so the connection
        // test never reports "no tools" for a provider the agent then sends tools to.
        tool_call: request_format_self_certifies_tools(&config.request_format),
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
/// tool calls.
pub(super) fn request_format_self_certifies_tools(format: &AiRequestFormat) -> bool {
    // Exhaustive (no `matches!` wildcard) so a NEW request format must consciously declare its
    // tool-calling story rather than defaulting in.
    match format {
        AiRequestFormat::Anthropic
        | AiRequestFormat::Google
        | AiRequestFormat::OpenAi
        | AiRequestFormat::LmStudio
        | AiRequestFormat::Ollama => true,
    }
}

/// Resolves whether the provider can execute tool calls before the harness sends tools.
///
/// The agent harness calls this BEFORE attaching a tools payload (02 §B degradation rule). Every
/// supported request format self-certifies (see [`request_format_self_certifies_tools`]), so this
/// answers without a network call; the harness still degrades gracefully at runtime if a loop
/// genuinely yields no tool calls.
pub async fn probe_tool_capability(provider: &RigLlmProvider) -> Result<bool> {
    Ok(request_format_self_certifies_tools(&provider.runtime.config.request_format))
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
struct MappedRigStream<S> {
    inner: S,
}

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
fn build_openai_client(runtime: &AiProviderRuntime) -> Result<openai::CompletionsClient> {
    // OPTIONAL key: only a present, non-blank secret is forwarded as the bearer token. A keyless
    // local model (LM Studio / Ollama) carries an empty token — rig 0.34's openai client always
    // emits an `Authorization` header (see the module note on the upstream limitation), but a
    // keyless local endpoint ignores it, and we NEVER block the call on a missing key of our own
    // accord. A key-enforcing cloud server answers with its own 401, which surfaces verbatim.
    let mut builder = openai::CompletionsClient::builder()
        .api_key(runtime.api_key_for_transport().unwrap_or_default());
    if let Some(base_url) = runtime.config.base_url.as_deref() {
        builder = builder
            .base_url(super::provider::openai_api_root(base_url, &runtime.config.request_format));
    }
    Ok(builder.build()?)
}

/// Builds an Anthropic native rig client.
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
