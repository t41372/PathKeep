//! Responsibilities: collect a complete model turn and execute prevalidated tool calls.
//! Not responsible for: retry policy, budgets, conversation persistence or provider encoding.
//! Why: a rejected turn must never expose executable partial calls to the agent loop.
use super::*;

/// Accumulated tool calls + usage for the turn currently streaming from the model.
#[derive(Default)]
pub(super) struct TurnAccumulator {
    pub(super) text: String,
    pub(super) reasoning: String,
    pub(super) tool_calls: Vec<PendingToolCall>,
    pub(super) usage: Option<LlmUsage>,
    pub(super) replay_reasoning: Option<LlmReasoning>,
    pub(super) finish_reason: Option<LlmFinishReason>,
}

/// One tool call the model requested this turn, awaiting execution.
#[derive(Clone)]
pub(super) struct PendingToolCall {
    pub(super) call_id: String,
    pub(super) name: String,
    pub(super) arguments: String,
}

/// Error from streaming one model turn: a cooperative cancel, or a terminal stream failure.
pub(super) enum TurnError {
    Cancelled,
    Stream(String),
    Recoverable { message: String, usage: Option<LlmUsage> },
}

/// Streams ONE model turn, forwarding token/reasoning chunks and accumulating tool calls + usage.
///
/// Reuses drive_chat_stream's per-turn discipline: forward each Token/Reasoning immediately; a
/// mid-stream Err terminates the turn; cancel between chunks stops cleanly. Tool calls and the
/// terminal Usage marker are accumulated (NOT forwarded as the visible answer) so the loop can act
/// on them after the turn completes.
pub(super) async fn stream_one_turn<P, S>(
    provider: &P,
    request: &LlmChatRequest,
    control: Option<&Arc<dyn AiRunControl>>,
    sink: &mut S,
) -> std::result::Result<TurnAccumulator, TurnError>
where
    P: LlmProvider,
    S: AgentRunSink,
{
    // Open the stream under cancellation (a cancel here is observed as a cancellation error).
    let open = await_with_ai_cancellation(
        control,
        "Agent run cancelled before the model responded.",
        provider.chat_stream(request.clone()),
    )
    .await;
    let mut stream = match open {
        Ok(stream) => stream,
        Err(error) if is_cancellation(&error) => return Err(TurnError::Cancelled),
        Err(error) => return Err(TurnError::Stream(error.to_string())),
    };

    let mut accumulated = TurnAccumulator::default();
    use std::future::poll_fn;
    loop {
        if control.is_some_and(|control| control.cancelled()) {
            return Err(TurnError::Cancelled);
        }
        let next = poll_fn(|cx| stream.as_mut().poll_next(cx)).await;
        match next {
            None => {
                return match accumulated.finish_reason {
                    Some(LlmFinishReason::Stop | LlmFinishReason::ToolCalls) => Ok(accumulated),
                    Some(LlmFinishReason::Length) => Err(TurnError::Recoverable {
                        message: "The model turn was truncated (finish reason length).".into(),
                        usage: accumulated.usage,
                    }),
                    Some(LlmFinishReason::Other(reason)) => {
                        Err(TurnError::Stream(format!("The model stopped with {reason}.")))
                    }
                    None => Err(TurnError::Recoverable {
                        message: "The stream ended without a terminal finish reason.".into(),
                        usage: accumulated.usage,
                    }),
                };
            }
            Some(Ok(chunk)) => match chunk {
                LlmStreamChunk::Token(text) => {
                    accumulated.text.push_str(&text);
                    sink(AiChatStreamChunk::Token { text });
                }
                LlmStreamChunk::Reasoning(text) => {
                    accumulated.reasoning.push_str(&text);
                    sink(AiChatStreamChunk::Reasoning { text });
                }
                LlmStreamChunk::ToolCall { call_id, name, arguments } => {
                    accumulated.tool_calls.push(PendingToolCall { call_id, name, arguments });
                }
                LlmStreamChunk::Usage(usage) => accumulated.usage = Some(usage),
                LlmStreamChunk::Finished { reason, reasoning } => {
                    accumulated.finish_reason = Some(reason);
                    accumulated.replay_reasoning = reasoning;
                }
            },
            Some(Err(error)) => {
                return Err(
                    if matches!(
                        error.downcast_ref::<LlmStreamError>(),
                        Some(LlmStreamError::Transport(_))
                    ) {
                        TurnError::Stream(error.to_string())
                    } else {
                        TurnError::Recoverable {
                            message: error.to_string(),
                            usage: accumulated.usage,
                        }
                    },
                );
            }
        }
    }
}

/// Builds the journaled `tool-result` step payload, widening it with the code-mode transparency
/// fields ONLY when present (`run_code`).
///
/// The journal blob is opaque JSON (no schema migration), so this stays additive: a search-tool step
/// serializes exactly `{ callId, name, result, isError }` as before; a `run_code` step adds
/// `codeSource` (the script verbatim), `hostCalls` (the timeline), and `limitsHit` (the hard limit),
/// each omitted when None/empty so trace replay + the explorer never see a spurious empty field.
pub(super) fn tool_result_payload(
    call: &PendingToolCall,
    result_text: &str,
    is_error: bool,
    code_source: &Option<String>,
    host_calls: &[HostCallRecord],
    limits_hit: Option<LimitsHit>,
) -> String {
    let mut payload = json!({
        "callId": call.call_id,
        "name": call.name,
        "result": result_text,
        "isError": is_error,
    });
    let object = payload.as_object_mut().expect("payload is a JSON object");
    if let Some(source) = code_source {
        object.insert("codeSource".to_string(), json!(source));
    }
    if !host_calls.is_empty() {
        object.insert("hostCalls".to_string(), json!(host_calls));
    }
    if let Some(limit) = limits_hit {
        object.insert("limitsHit".to_string(), json!(limit));
    }
    payload.to_string()
}

/// The result of executing one tool call: the model-facing text + error flag + pinned citations,
/// plus the optional code-mode transparency fields (`run_code` only) the harness journals + streams.
///
/// The code-mode fields default to empty/None on the search tools and on any failure, so the journal
/// payload + the streamed ToolResult chunk stay byte-identical for the W-AI-7 path (W-AI-8 additive).
pub(super) struct ToolExecution {
    pub(super) result_text: String,
    pub(super) is_error: bool,
    pub(super) citations: Vec<AiCitation>,
    /// The `run_code` script verbatim (transparency); `None` for the search tools / any failure.
    pub(super) code_source: Option<String>,
    /// The `run_code` host-call timeline; empty for the search tools / any failure.
    pub(super) host_calls: Vec<HostCallRecord>,
    /// Which hard sandbox limit bounded a `run_code` script, if any.
    pub(super) limits_hit: Option<LimitsHit>,
    /// A search behind the result covered only part of the archive.
    pub(super) coverage_incomplete: bool,
}

/// Executes one tool call into a [`ToolExecution`].
///
/// On success the result_text is the tool's bounded summary and citations are pinned; for `run_code`
/// the code-mode transparency fields are carried too. On FAILURE the result_text becomes an honest
/// error string, `is_error` is true, and the rest are empty — the harness threads that back so the
/// model can recover (partial-results discipline). Invalid args produce the same honest error rather
/// than aborting the run.
pub(super) async fn execute_tool(
    registry: &ToolRegistry,
    context: &AgentToolContext,
    call: &PendingToolCall,
    prepared: &std::result::Result<serde_json::Value, String>,
) -> ToolExecution {
    let args = match prepared {
        Ok(value) => value,
        Err(error) => {
            return ToolExecution {
                result_text: format!("Tool `{}` received invalid arguments: {error}", call.name),
                is_error: true,
                citations: Vec::new(),
                code_source: None,
                host_calls: Vec::new(),
                limits_hit: None,
                coverage_incomplete: false,
            };
        }
    };
    match registry.dispatch(&call.name, args.clone(), context).await {
        Ok(ToolOutcome {
            model_text,
            citations,
            code_source,
            host_calls,
            limits_hit,
            coverage_incomplete,
        }) => ToolExecution {
            result_text: model_text,
            is_error: false,
            citations,
            code_source,
            host_calls,
            limits_hit,
            coverage_incomplete,
        },
        Err(error) => ToolExecution {
            result_text: format!(
                "Tool `{}` failed: {error}. Correct the call and try again, or use another tool.",
                call.name
            ),
            is_error: true,
            citations: Vec::new(),
            code_source: None,
            host_calls: Vec::new(),
            limits_hit: None,
            coverage_incomplete: false,
        },
    }
}
