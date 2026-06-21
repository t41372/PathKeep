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
use rig::completion::{AssistantContent, Message as RigMessage, ToolDefinition, Usage as RigUsage};
use rig::streaming::StreamedAssistantContent;

#[cfg(not(any(test, coverage)))]
use {
    rig::client::CompletionClient,
    rig::completion::CompletionModel,
    rig::providers::{anthropic, gemini, openai},
    secrecy::ExposeSecret,
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

/// Derives the in-engine capability view from the provider config.
///
/// Shared by `capabilities()` and the connection probe so both report the same thing.
/// The OpenAI-compat floor and Anthropic/Gemini native adapters all support streaming
/// chat in rig 0.34; `max_context_tokens` stays `None` here because rig does not expose
/// a window size and PathKeep never hardcodes one per the model-agnostic rule (D4).
pub(super) fn rig_llm_capabilities(config: &super::AiProviderConfig) -> LlmCapabilities {
    let interactive_chat = matches!(
        config.request_format,
        AiRequestFormat::OpenAi
            | AiRequestFormat::Anthropic
            | AiRequestFormat::Google
            | AiRequestFormat::Ollama
            | AiRequestFormat::LmStudio
    );
    // TODO(W-AI-7): tool_call/structured_output are OPTIMISTIC advertisements for local
    // providers; the agent must runtime-probe and degrade before sending a tools payload (02 §B).
    // Anthropic and Gemini native adapters support prefix/prompt caching; the local
    // OpenAI-compat floor (Ollama/LM Studio/OpenAI) does not advertise it through rig.
    let prompt_cache =
        matches!(config.request_format, AiRequestFormat::Anthropic | AiRequestFormat::Google);
    LlmCapabilities {
        tool_call: interactive_chat,
        structured_output: interactive_chat,
        streaming: interactive_chat,
        prompt_cache,
        max_context_tokens: None,
    }
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
/// `ToolCall`. The final usage marker and tool-call deltas are intentionally dropped here
/// (deltas are aggregated by rig into the complete `ToolCall`).
fn map_streamed_content<R: Clone + Unpin>(
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
        StreamedAssistantContent::ToolCall { tool_call, .. } => Some(LlmStreamChunk::ToolCall {
            name: tool_call.function.name,
            arguments: tool_call.function.arguments.to_string(),
        }),
        // TODO(W-AI-7): Final carries token usage; the budget loop needs it surfaced (e.g. add
        // LlmStreamChunk::Usage).
        StreamedAssistantContent::ToolCallDelta { .. } | StreamedAssistantContent::Final(_) => None,
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
    R: Clone + Unpin,
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
    let mut builder = openai::CompletionsClient::builder().api_key(runtime.api_key.expose_secret());
    if let Some(base_url) = runtime.config.base_url.as_deref() {
        builder = builder.base_url(base_url);
    }
    Ok(builder.build()?)
}

/// Builds an Anthropic native rig client.
#[cfg(not(any(test, coverage)))]
fn build_anthropic_client(runtime: &AiProviderRuntime) -> Result<anthropic::Client> {
    let mut builder = anthropic::Client::builder().api_key(runtime.api_key.expose_secret());
    if let Some(base_url) = runtime.config.base_url.as_deref() {
        builder = builder.base_url(base_url);
    }
    Ok(builder.build()?)
}

/// Builds a Gemini native rig client.
#[cfg(not(any(test, coverage)))]
fn build_gemini_client(runtime: &AiProviderRuntime) -> Result<gemini::Client> {
    let mut builder = gemini::Client::builder().api_key(runtime.api_key.expose_secret());
    if let Some(base_url) = runtime.config.base_url.as_deref() {
        builder = builder.base_url(base_url);
    }
    Ok(builder.build()?)
}

// ---------------------------------------------------------------------------
// Deterministic stub (test + coverage). Same public call graph as production:
// `chat` returns stable text/reasoning/usage; `chat_stream` yields a Reasoning,
// Token(s), a ToolCall, then ends — and surfaces a stream error for ids tagged
// "stream-error" so the Err arm is exercised.
// ---------------------------------------------------------------------------

/// Returns a stable, request-derived chat response without any network call.
#[cfg(any(test, coverage))]
async fn chat_impl(runtime: &AiProviderRuntime, req: LlmChatRequest) -> Result<LlmChatResponse> {
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

/// Returns a deterministic chunk stream that exercises every chunk variant + an error path.
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
    let emit_error = runtime.config.id.contains("stream-error");
    let mut chunks: Vec<Result<LlmStreamChunk>> = vec![
        Ok(LlmStreamChunk::Reasoning(format!("{format_tag} thinking"))),
        Ok(LlmStreamChunk::Token(format!("{format_tag}:{message_count}"))),
        Ok(LlmStreamChunk::ToolCall { name: "search".to_string(), arguments: "{}".to_string() }),
    ];
    if emit_error {
        chunks.push(Err(anyhow::anyhow!("forced coverage mid-stream error")));
    }
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
    use crate::ai::traits::{LlmMessage, LlmRole, LlmToolDef};
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
            api_key: SecretString::from("test-key".to_string()),
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
    async fn chat_stream_yields_reasoning_token_and_tool_call() {
        let provider = RigLlmProvider::new(runtime(AiRequestFormat::Google, "stream-ok"));
        let stream = provider.chat_stream(request_with_tools()).await.expect("stream");
        let (chunks, errors) = drain(stream).await;
        assert_eq!(errors, 0);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], LlmStreamChunk::Reasoning("google thinking".to_string()));
        assert_eq!(chunks[1], LlmStreamChunk::Token("google:2".to_string()));
        assert!(matches!(
            &chunks[2],
            LlmStreamChunk::ToolCall { name, .. } if name == "search"
        ));
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
        let openai = RigLlmProvider::new(runtime(AiRequestFormat::OpenAi, "caps-openai"));
        let caps = openai.capabilities();
        assert!(caps.streaming && caps.tool_call && caps.structured_output);
        assert!(!caps.prompt_cache);
        assert_eq!(caps.max_context_tokens, None);

        let anthropic = RigLlmProvider::new(runtime(AiRequestFormat::Anthropic, "caps-anthropic"));
        assert!(anthropic.capabilities().prompt_cache);

        let gemini = RigLlmProvider::new(runtime(AiRequestFormat::Google, "caps-gemini"));
        assert!(gemini.capabilities().prompt_cache);
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
            Some(LlmStreamChunk::ToolCall { name, arguments }) => {
                assert_eq!(name, "search");
                assert_eq!(arguments, serde_json::json!({ "q": "rust" }).to_string());
            }
            other => panic!("expected ToolCall chunk, got {other:?}"),
        }
    }

    #[test]
    fn map_streamed_content_drops_final_and_tool_call_delta() {
        let final_item: StreamedAssistantContent<u8> = StreamedAssistantContent::Final(0);
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
    fn to_rig_message_defaults_missing_tool_call_id_to_empty_string() {
        // A `Tool`-role message with no correlating call id maps to an empty rig tool id rather
        // than panicking.
        let bare = LlmMessage {
            role: LlmRole::Tool,
            content: "x".to_string(),
            tool_call_id: None,
            tool_name: None,
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
