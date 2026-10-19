//! Per-turn rig transport behind the rig-free LlmProvider boundary.
//!
//! Responsibilities: encode chat, decode streams, and retain native reasoning for one run.
//! Not responsible for: tool execution, conversation storage, budgets or a multi-turn runtime.
//! Why: provider wire differences belong here, while the agent loop remains ours.

use super::traits::{
    LlmCapabilities, LlmChatRequest, LlmChatResponse, LlmChunkStream, LlmFinishReason, LlmMessage,
    LlmProvider, LlmReasoning, LlmRole, LlmStreamChunk, LlmStreamError, LlmUsage,
};
use super::{AiProviderRuntime, AiRequestFormat};
use anyhow::Result;
use rig_core::completion::{
    AssistantContent, AssistantMessage, CompletionRequest, CompletionResponse, FinishReason,
    Message, ToolDefinition, Usage,
};
use rig_core::error::ProviderError;
use rig_core::message::{CallId, ToolName, ToolResultContent, UserContent};
use rig_core::providers::openai::wire::{Auth, Dialect, OLLAMA, OPENAI, Quirks, Route};
use rig_core::providers::{anthropic, gemini, openai};
use rig_core::streaming::{CompletionStream, Item, StreamEvent};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// One resolved provider; all concrete rig types stay inside this adapter.
pub struct RigLlmProvider {
    runtime: AiProviderRuntime,
}
impl RigLlmProvider {
    /// Keeps config and its resolved secret together without consulting storage during a turn.
    pub fn new(runtime: AiProviderRuntime) -> Self {
        Self { runtime }
    }
    /// Identifies the selected model without hardcoding provider model names.
    pub fn model_id(&self) -> &str {
        &self.runtime.config.default_model
    }
}

/// Derives the in-engine capability view from the provider config (honest, not optimistic).
///
/// Shared by `capabilities()` and the connection probe so both report the same thing. Every
/// supported request format streams chat in rig 0.44; `max_context_tokens` stays `None` because
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
    async fn chat(&self, req: LlmChatRequest) -> Result<LlmChatResponse> {
        let response = completion_model(&self.runtime).call(build_request(&req)?).await?;
        if let Some(error) = &response.error {
            anyhow::bail!("{error}");
        }
        let text = response
            .choice
            .iter()
            .filter_map(|item| match item {
                AssistantContent::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<String>();
        let reasoning = response.reasoning();
        Ok(LlmChatResponse {
            text,
            reasoning: (!reasoning.is_empty()).then_some(reasoning),
            usage: usage(&response.usage),
        })
    }
    async fn chat_stream(&self, req: LlmChatRequest) -> Result<LlmChunkStream> {
        let stream = completion_model(&self.runtime).stream(build_request(&req)?)?;
        Ok(Box::pin(MappedRigStream {
            inner: Some(stream),
            finishing: None,
            pending: VecDeque::new(),
            arguments: std::collections::HashMap::new(),
            ended: false,
        }))
    }
    fn capabilities(&self) -> LlmCapabilities {
        rig_llm_capabilities(&self.runtime.config)
    }
}

// Chat Completions is the floor even for hosted OpenAI. A keyless gateway gets no header.
// Keep rig's message encoding (including system content arrays) untouched.
const COMPAT: Dialect = {
    let mut dialect = OPENAI;
    dialect.quirks = Quirks::openai();
    dialect.quirks.completion_route = Route::Chat;
    dialect.quirks.auth = Auth::OptionalBearer;
    dialect.quirks.stream_include_usage = true;
    dialect
};

fn completion_model(
    runtime: &AiProviderRuntime,
) -> rig_core::DynModel<rig_core::operation::Completion> {
    let model = runtime.config.default_model.clone();
    let key = runtime.api_key_for_transport().unwrap_or_default();
    match runtime.config.request_format {
        AiRequestFormat::OpenAi | AiRequestFormat::LmStudio | AiRequestFormat::Ollama => {
            let dialect = if runtime.config.request_format == AiRequestFormat::Ollama {
                &OLLAMA
            } else {
                &COMPAT
            };
            let mut config = openai::OpenAIConfig::with_key(dialect, key);
            if let Some(url) = &runtime.config.base_url {
                config = config.with_base_url(super::provider::openai_api_root(
                    url,
                    &runtime.config.request_format,
                ));
            }
            config.client().chat(model).erase()
        }
        AiRequestFormat::Anthropic => {
            let mut config = anthropic::AnthropicConfig::new(key);
            if let Some(url) = &runtime.config.base_url {
                config = config.with_base_url(super::provider::normalize_local_base_url(url));
            }
            // rig supplies the catalog's maximum output for known Anthropic models.
            // An unknown model has no honest limit to invent, so rig returns an explicit error.
            config.client().completion(model).erase()
        }
        AiRequestFormat::Google => {
            let mut config = gemini::GeminiConfig::new(key);
            if let Some(url) = &runtime.config.base_url {
                config = config.with_base_url(super::provider::normalize_local_base_url(url));
            }
            config.client().completion(model).erase()
        }
    }
}

fn to_rig_message(message: &LlmMessage) -> Result<Message> {
    Ok(match message.role {
        LlmRole::System => Message::system(message.content.clone()),
        LlmRole::User => Message::user(message.content.clone()),
        LlmRole::Tool => Message::User {
            content: vec![UserContent::tool_result(
                CallId::from_wire(message.tool_call_id.clone().unwrap_or_default()),
                ToolName::new(message.tool_name.clone().unwrap_or_default())?,
                vec![ToolResultContent::text(message.content.clone())],
            )],
        },
        LlmRole::Assistant => {
            let mut content = Vec::new();
            if let Some(reasoning) = &message.reasoning {
                for block in &reasoning.blocks {
                    content
                        .push(AssistantContent::Reasoning(serde_json::from_value(block.clone())?));
                }
            }
            if !message.content.is_empty() {
                content.push(AssistantContent::text(&message.content));
            }
            for call in &message.tool_calls {
                // This is a last boundary guard. Malformed or non-object arguments never reach
                // a template's arguments|items, even if a caller bypassed the harness.
                let args = serde_json::from_str::<serde_json::Value>(&call.arguments)
                    .ok()
                    .filter(serde_json::Value::is_object)
                    .unwrap_or_else(|| serde_json::json!({}));
                content.push(AssistantContent::tool_call(
                    &call.call_id,
                    ToolName::new(&call.name)?,
                    args,
                ));
            }
            let mut assistant = AssistantMessage::new(content);
            if let Some(reasoning) = &message.reasoning {
                assistant =
                    assistant.with_origin(Some(serde_json::from_value(reasoning.origin.clone())?));
            }
            Message::Assistant(assistant)
        }
    })
}

fn build_request(req: &LlmChatRequest) -> Result<CompletionRequest> {
    let mut messages = req.messages.iter().map(to_rig_message).collect::<Result<Vec<_>>>()?;
    let prompt = messages
        .pop()
        .ok_or_else(|| anyhow::anyhow!("Chat request must contain at least one message."))?;
    let mut request = CompletionRequest::new(prompt).messages(messages).tools(
        req.tools
            .iter()
            .map(|tool| {
                Ok(ToolDefinition {
                    name: ToolName::new(&tool.name)?,
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?,
    );
    if let Some(format) = &req.response_format {
        request = request.output_schema(schemars::Schema::try_from(format.schema.clone())?);
    }
    // No sampling parameters and no output cap. Native Anthropic alone supplies its required cap.
    Ok(request)
}

fn usage(value: &Usage) -> Option<LlmUsage> {
    Some(LlmUsage { prompt_tokens: value.input_tokens?, completion_tokens: value.output_tokens? })
}

fn terminal(response: CompletionResponse) -> Result<LlmStreamChunk> {
    if let Some(error) = &response.error {
        return Err(anyhow::Error::new(LlmStreamError::Generation(error.clone())));
    }
    let reason = match response.finish_reason() {
        Some(FinishReason::Stop) => LlmFinishReason::Stop,
        Some(FinishReason::ToolCalls) => LlmFinishReason::ToolCalls,
        Some(FinishReason::Length) => LlmFinishReason::Length,
        Some(reason) => LlmFinishReason::Other(format!("{reason:?}")),
        None => {
            return Err(anyhow::Error::new(LlmStreamError::Generation(
                "The provider ended without a finish reason.".into(),
            )));
        }
    };
    let blocks = response
        .choice
        .iter()
        .filter_map(|content| match content {
            AssistantContent::Reasoning(reasoning) => Some(serde_json::to_value(reasoning)),
            _ => None,
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let reasoning = if blocks.is_empty() {
        None
    } else {
        Some(LlmReasoning { blocks, origin: serde_json::to_value(response.origin)? })
    };
    Ok(LlmStreamChunk::Finished { reason, reasoning })
}

fn stream_error(error: ProviderError) -> anyhow::Error {
    // The network opens lazily on polling. HTTP/auth/request failures remain terminal, even
    // though rig now delivers them as stream items. Only a failed generation may be repaired.
    let authentication_error = error.provider_response().is_some_and(|response| {
        response.machine_code().is_some_and(|code| {
            matches!(
                code.to_ascii_lowercase().as_str(),
                "401"
                    | "403"
                    | "invalid_api_key"
                    | "authentication_error"
                    | "unauthenticated"
                    | "unauthorized"
                    | "permission_denied"
            )
        }) || error.provider_response_json().ok().flatten().is_some_and(|body| {
            ["/error/type", "/error/status"]
                .iter()
                .filter_map(|path| body.pointer(path).and_then(serde_json::Value::as_str))
                .any(|code| {
                    matches!(
                        code.to_ascii_lowercase().as_str(),
                        "authentication_error" | "unauthenticated" | "permission_denied"
                    )
                })
        })
    });
    let recoverable = !authentication_error
        && match &error {
            ProviderError::Truncated
            | ProviderError::Json(_)
            | ProviderError::Response(_)
            | ProviderError::Provider(_) => true,
            ProviderError::ProviderResponse(_) => {
                error.provider_response_status().is_none_or(|status| status.is_success())
            }
            _ => false,
        };
    if recoverable {
        anyhow::Error::new(LlmStreamError::Generation(error.to_string()))
    } else {
        anyhow::Error::new(LlmStreamError::Transport(error.to_string()))
    }
}

type FinishFuture =
    Pin<Box<dyn Future<Output = std::result::Result<CompletionResponse, ProviderError>> + Send>>;
struct MappedRigStream {
    inner: Option<CompletionStream>,
    finishing: Option<FinishFuture>,
    pending: VecDeque<LlmStreamChunk>,
    arguments: std::collections::HashMap<usize, String>,
    ended: bool,
}
impl futures_core::Stream for MappedRigStream {
    type Item = Result<LlmStreamChunk>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(chunk) = self.pending.pop_front() {
                return Poll::Ready(Some(Ok(chunk)));
            }
            if self.ended {
                return Poll::Ready(None);
            }
            if let Some(future) = &mut self.finishing {
                let response = match future.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(response) => response,
                };
                self.finishing = None;
                self.ended = true;
                return match response {
                    Err(error) => Poll::Ready(Some(Err(stream_error(error)))),
                    Ok(response) => {
                        if let Some(value) = usage(&response.usage) {
                            self.pending.push_back(LlmStreamChunk::Usage(value));
                        }
                        let terminal = terminal(response);
                        if let Some(chunk) = self.pending.pop_front() {
                            match terminal {
                                Ok(finish) => self.pending.push_back(finish),
                                Err(error) => return Poll::Ready(Some(Err(error))),
                            }
                            Poll::Ready(Some(Ok(chunk)))
                        } else {
                            Poll::Ready(Some(terminal))
                        }
                    }
                };
            }
            let next = Pin::new(self.inner.as_mut().expect("active stream")).poll_next(cx);
            match next {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    self.finishing =
                        Some(Box::pin(self.inner.take().expect("active stream").finish()));
                }
                Poll::Ready(Some(Err(error))) => {
                    self.ended = true;
                    self.inner = None;
                    return Poll::Ready(Some(Err(stream_error(error))));
                }
                Poll::Ready(Some(Ok(Item::Event(event)))) => match event {
                    StreamEvent::Text { text, .. } => {
                        return Poll::Ready(Some(Ok(LlmStreamChunk::Token(text))));
                    }
                    StreamEvent::Reasoning { text, .. } => {
                        return Poll::Ready(Some(Ok(LlmStreamChunk::Reasoning(text))));
                    }
                    StreamEvent::Arguments { part, json } => {
                        self.arguments.entry(part.index()).or_default().push_str(&json);
                    }
                    StreamEvent::End { part, content: AssistantContent::ToolCall(call) } => {
                        return Poll::Ready(Some(Ok(LlmStreamChunk::ToolCall {
                            call_id: call.id.to_string(),
                            name: call.function.name.to_string(),
                            arguments: self
                                .arguments
                                .remove(&part.index())
                                .filter(|raw| !raw.is_empty())
                                .or(call.function.invalid_arguments)
                                .unwrap_or_else(|| {
                                    serde_json::Value::Object(call.function.arguments).to_string()
                                }),
                        })));
                    }
                    _ => {}
                },
                Poll::Ready(Some(Ok(Item::Unknown(_)))) => {}
            }
        }
    }
}
