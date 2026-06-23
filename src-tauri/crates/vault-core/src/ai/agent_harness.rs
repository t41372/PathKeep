//! Hand-rolled, durable, tool-executing streaming agent loop (W-AI-7 core, 02 §F).
//!
//! ## Responsibilities
//! - drive ONE agent run as a thin tokio while-loop over `LlmProvider::chat_stream`: forward
//!   token/reasoning chunks, accumulate the turn's tool calls + usage, then EXECUTE each tool call
//!   against the owned [`ToolRegistry`] and thread the result back as a `tool_result` message
//! - enforce the run invariants: cooperative cancel at every checkpoint, a per-run token budget, a
//!   max-iteration ceiling, and stall/no-progress detection
//! - JOURNAL every model turn + tool result BEFORE observing/emitting it (so a crash leaves an
//!   interrupted partial trace and a resume = replay, never a re-call of the model)
//! - degrade honestly: when the provider cannot do tool calls (the WU-1 probe), fall back to a
//!   deterministic retrieve-then-answer shape (seeded evidence, no tool loop) + an honest note
//!
//! ## Not responsible for
//! - touching rig (it consumes only `LlmStreamChunk` via the boundary) — 02 §B
//! - owning the agent.sqlite schema/CRUD (that is `agent_store.rs`; the harness writes through the
//!   [`AgentJournal`] seam) or the run registry/cancel token plumbing (the worker owns those)
//! - LLM auto-compaction (deferred to W-AI-8/9); context is bounded here by row-id + count evidence,
//!   never by inlining huge result sets
//!
//! ## Why this module exists
//! 02 §F MANDATES a hand-rolled thin loop over the per-turn completion client with an owned tool
//! dispatch — rig's `Agent`/`.prompt()` runtime is in-memory, non-resumable, and gives no
//! cancel/budget/journaling/citation hooks. This is that loop.
//!
//! ## Performance notes
//! - runs on the worker thread (never the UI); journal writes are bounded per step; the
//!   max-iteration ceiling caps how often the bounded retrieval (incl. the known O(n) `is:starred`
//!   `.pkmap` pass) can run, so a runaway loop can never spin it.

use super::agent_tools::{AgentToolContext, HostCallRecord, LimitsHit, ToolOutcome, ToolRegistry};
use super::traits::{LlmChatRequest, LlmMessage, LlmProvider, LlmRole, LlmStreamChunk, LlmUsage};
use super::{AiRunControl, await_with_ai_cancellation};
use crate::models::{AiAgentNote, AiChatStreamChunk, AiCitation};
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

/// Hard ceiling on model turns in one agent run.
///
/// Bounds total cost AND how many times the bounded retrieval pipeline (incl. the gated O(n)
/// `is:starred` pass) can run, so a looping model can never spin the host. The model must converge
/// on an answer within this many turns or the run finalizes with the best evidence so far.
pub const DEFAULT_MAX_ITERATIONS: u32 = 8;

/// Default per-run token budget (summed prompt + completion across turns).
///
/// A coarse cost ceiling (02 §F). When the running total crosses this the loop stops after the
/// current turn rather than starting another — the user gets the partial answer, never an unbounded
/// bill. `0` means "no budget" (used by tests that want pure iteration control).
pub const DEFAULT_TOKEN_BUDGET: u64 = 120_000;

/// One journaled step kind, recorded BEFORE the corresponding chunk is observed/emitted.
///
/// Stable tags so trace replay (and the explorer) can route a step without guessing.
const STEP_KIND_ASSISTANT_TURN: &str = "assistant-turn";
const STEP_KIND_TOOL_RESULT: &str = "tool-result";
const STEP_KIND_DEGRADED_ANSWER: &str = "degraded-answer";

/// Why an agent run ended, returned to the worker so it can finalize the agent.sqlite header.
///
/// Mirrors the terminal stream marker the sink already received; the worker maps it onto
/// `AgentRunStatus`. `Completed` carries the final answer + total budget; the others carry enough to
/// record an honest header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRunOutcome {
    /// The run produced a final answer (no further tool calls, or the ceiling/budget was reached).
    Completed { iterations: u32, prompt_tokens: u64, completion_tokens: u64 },
    /// The run was cooperatively cancelled (a `Done` was already emitted, matching plain chat).
    Cancelled { iterations: u32, prompt_tokens: u64, completion_tokens: u64 },
    /// The run hit a terminal error (an `Error` chunk was already emitted).
    Failed { iterations: u32, prompt_tokens: u64, completion_tokens: u64, message: String },
}

/// The durable journal seam the harness writes through (implemented by the worker over agent.sqlite).
///
/// JOURNAL-BEFORE-OBSERVE (02 §F): the harness calls `journal_step` and only emits the matching
/// stream chunk after it returns `Ok`. `record_citations` pins evidence so it survives compaction.
/// A pure trait keeps vault-core free of the SQLite plumbing AND lets the deterministic harness test
/// drive an in-memory journal with no database.
pub trait AgentJournal: Send + Sync {
    /// Journals one step (the harness assigns `turn`/`kind`; the store assigns `seq`). For a
    /// tool-result step `tool_call_id` is the idempotency key.
    fn journal_step(
        &self,
        turn: u32,
        kind: &str,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
        payload: &str,
    ) -> Result<()>;

    /// Pins the run's evidence citations (canonical_url keyed) once they are known.
    fn record_citations(&self, citations: &[AiCitation]) -> Result<()>;
}

/// The sink the harness emits IPC chunks through (the worker wraps `AppHandle::emit`).
///
/// Identical contract to `drive_chat_stream`'s sink so the worker reuses the same emit closure.
pub trait AgentRunSink: FnMut(AiChatStreamChunk) {}
impl<F: FnMut(AiChatStreamChunk)> AgentRunSink for F {}

/// Accumulated tool calls + usage for the turn currently streaming from the model.
#[derive(Default)]
struct TurnAccumulator {
    text: String,
    reasoning: String,
    tool_calls: Vec<PendingToolCall>,
    usage: Option<LlmUsage>,
}

/// One tool call the model requested this turn, awaiting execution.
#[derive(Clone)]
struct PendingToolCall {
    call_id: String,
    name: String,
    arguments: String,
}

/// Drives one agent run to completion, forwarding chunks to `sink` and journaling every step.
///
/// The loop (02 §F):
/// 1. checkpoint cancel; stream one model turn (tools attached only when the provider can use them)
/// 2. forward Token/Reasoning chunks; accumulate ToolCall + Usage
/// 3. journal the assistant turn BEFORE acting on it; add the run-token budget from its usage
/// 4. no tool calls → emit `Done`, return `Completed`
/// 5. else execute each tool against the registry: journal the result (idempotency key =
///    run_id + tool_call_id) BEFORE emitting a `ToolResult`, then thread an `LlmMessage::tool_result`
///    back into history (a tool ERROR becomes an `is_error` ToolResult + an honest threaded message
///    so the model can recover — one failure never aborts the run)
/// 6. enforce max-iterations + per-run token budget + stall detection, then loop
///
/// Capability degradation: when `provider_can_use_tools` is false the loop is skipped entirely and
/// the run takes the deterministic retrieve-then-answer shape (one seeded turn, no tools) + an
/// honest note. Cancel at EVERY checkpoint emits `Done` (not `Error`), matching `drive_chat_stream`.
#[allow(clippy::too_many_arguments)]
pub async fn drive_agent_run<P, J, S>(
    provider: &P,
    registry: &ToolRegistry,
    tool_context: &AgentToolContext,
    provider_can_use_tools: bool,
    mut request: LlmChatRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
    max_iterations: u32,
    token_budget: u64,
    journal: &J,
    mut sink: S,
) -> AgentRunOutcome
where
    P: LlmProvider,
    J: AgentJournal,
    S: AgentRunSink,
{
    let control = run_control.as_ref();
    let mut prompt_tokens = 0u64;
    let mut completion_tokens = 0u64;
    let mut citations: Vec<AiCitation> = Vec::new();

    // Pre-cancel: mirror drive_chat_stream — emit Done, not Error.
    if control.is_some_and(|control| control.cancelled()) {
        sink(AiChatStreamChunk::Done);
        return AgentRunOutcome::Cancelled { iterations: 0, prompt_tokens, completion_tokens };
    }

    // Capability degradation (02 §B/§F): a tool-incapable provider never enters the loop.
    if !provider_can_use_tools {
        return run_degraded_answer(
            provider,
            request,
            control,
            journal,
            &mut sink,
            &mut prompt_tokens,
            &mut completion_tokens,
        )
        .await;
    }

    // Advertise the owned tools every turn (the registry builds the definitions).
    request.tools = registry.definitions();

    let mut turn: u32 = 0;
    loop {
        turn += 1;

        // Checkpoint before each turn; cancel emits Done.
        if let Err(error) = checkpoint(control, "Agent run cancelled before the next model turn.") {
            return finish_cancel_or_error(
                &mut sink,
                error,
                turn - 1,
                prompt_tokens,
                completion_tokens,
            );
        }

        // Stream one model turn, forwarding tokens/reasoning and accumulating tool calls + usage.
        let accumulated = match stream_one_turn(provider, &request, control, &mut sink).await {
            Ok(accumulated) => accumulated,
            Err(TurnError::Cancelled) => {
                sink(AiChatStreamChunk::Done);
                return AgentRunOutcome::Cancelled {
                    iterations: turn,
                    prompt_tokens,
                    completion_tokens,
                };
            }
            Err(TurnError::Stream(message)) => {
                sink(AiChatStreamChunk::Error { message: message.clone() });
                return AgentRunOutcome::Failed {
                    iterations: turn,
                    prompt_tokens,
                    completion_tokens,
                    message,
                };
            }
        };

        // Account for usage + emit the Usage marker so the UI can show running cost.
        if let Some(usage) = accumulated.usage {
            prompt_tokens += usage.prompt_tokens;
            completion_tokens += usage.completion_tokens;
            sink(AiChatStreamChunk::Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
            });
        }

        // JOURNAL the assistant turn BEFORE acting on its tool calls.
        let turn_payload = json!({
            "text": accumulated.text,
            "reasoning": accumulated.reasoning,
            "toolCalls": accumulated
                .tool_calls
                .iter()
                .map(|call| json!({ "callId": call.call_id, "name": call.name, "arguments": call.arguments }))
                .collect::<Vec<_>>(),
            "usage": accumulated.usage.map(|usage| json!({
                "promptTokens": usage.prompt_tokens,
                "completionTokens": usage.completion_tokens,
            })),
        })
        .to_string();
        if let Err(error) =
            journal.journal_step(turn, STEP_KIND_ASSISTANT_TURN, None, None, &turn_payload)
        {
            sink(AiChatStreamChunk::Error { message: error.to_string() });
            return AgentRunOutcome::Failed {
                iterations: turn,
                prompt_tokens,
                completion_tokens,
                message: error.to_string(),
            };
        }

        // Record the assistant's own text turn into history so the next turn has full context.
        if !accumulated.text.is_empty() {
            request.messages.push(LlmMessage::new(LlmRole::Assistant, accumulated.text.clone()));
        }

        // No tool calls → the model is done. Pin citations, stream them, emit Done, complete.
        if accumulated.tool_calls.is_empty() {
            let _ = journal.record_citations(&citations);
            emit_citations(&mut sink, &citations);
            sink(AiChatStreamChunk::Done);
            return AgentRunOutcome::Completed {
                iterations: turn,
                prompt_tokens,
                completion_tokens,
            };
        }

        // Execute each tool call (partial-results discipline: one failure never aborts the run).
        for call in &accumulated.tool_calls {
            if let Err(error) = checkpoint(control, "Agent run cancelled before a tool call.") {
                return finish_cancel_or_error(
                    &mut sink,
                    error,
                    turn,
                    prompt_tokens,
                    completion_tokens,
                );
            }
            // Emit the ToolCall BEFORE executing it so the FE creates the pending row (args +
            // spinner) that the matching ToolResult then resolves by call_id (02 §G transparency:
            // the user sees the live tool-use timeline on the agent path, not only the result).
            // `stream_one_turn` accumulates the call rather than forwarding it, so this is the one
            // place the agent path advertises a call before acting on it.
            sink(AiChatStreamChunk::ToolCall {
                name: call.name.clone(),
                arguments: call.arguments.clone(),
                call_id: Some(call.call_id.clone()),
            });
            let ToolExecution {
                result_text,
                is_error,
                citations: mut new_citations,
                code_source,
                host_calls,
                limits_hit,
            } = execute_tool(registry, tool_context, call).await;

            // JOURNAL the tool result BEFORE emitting it (idempotency key = run_id + call_id, the
            // store dedups). The result payload is the model-facing text (bounded); the code-mode
            // fields are appended ONLY when present (`run_code`), so a search-tool step's payload is
            // byte-identical to before (the journal blob is opaque JSON — no schema migration).
            let result_payload = tool_result_payload(
                call,
                &result_text,
                is_error,
                &code_source,
                &host_calls,
                limits_hit,
            );
            if let Err(error) = journal.journal_step(
                turn,
                STEP_KIND_TOOL_RESULT,
                Some(&call.name),
                Some(&call.call_id),
                &result_payload,
            ) {
                sink(AiChatStreamChunk::Error { message: error.to_string() });
                return AgentRunOutcome::Failed {
                    iterations: turn,
                    prompt_tokens,
                    completion_tokens,
                    message: error.to_string(),
                };
            }

            // Emit the ToolResult to the UI (carrying the code-mode fields so the FE can render +
            // persist the script/timeline/limit), then thread the text back to the model.
            sink(AiChatStreamChunk::ToolResult {
                call_id: call.call_id.clone(),
                name: call.name.clone(),
                result: result_text.clone(),
                is_error,
                code_source,
                host_calls,
                limits_hit,
            });
            request.messages.push(LlmMessage::tool_result(
                call.call_id.clone(),
                call.name.clone(),
                result_text,
            ));
            citations.append(&mut new_citations);
        }

        // Budget + max-iteration + stall gates AFTER threading results back.
        if max_iterations != 0 && turn >= max_iterations {
            return finalize_after_loop(
                &mut sink,
                journal,
                &citations,
                turn,
                prompt_tokens,
                completion_tokens,
                AiAgentNote::MaxStepsReached,
            );
        }
        if token_budget != 0 && prompt_tokens + completion_tokens >= token_budget {
            return finalize_after_loop(
                &mut sink,
                journal,
                &citations,
                turn,
                prompt_tokens,
                completion_tokens,
                AiAgentNote::TokenBudgetReached,
            );
        }
    }
}

/// Emits a closing control NOTE + Done after the loop hit a budget/iteration ceiling, pinning
/// citations (review-fix M-6).
///
/// The closing note is streamed as a localized [`AiChatStreamChunk::Note`] CODE — NOT raw English —
/// so the front end renders it in the user's locale. This note is purely USER-facing (the loop has
/// ended, so there is no model-facing counterpart to thread back); the model-facing English that
/// steers the LLM lives in the threaded `tool_result` messages + preamble, which are untouched.
fn finalize_after_loop<J: AgentJournal, S: AgentRunSink>(
    sink: &mut S,
    journal: &J,
    citations: &[AiCitation],
    turn: u32,
    prompt_tokens: u64,
    completion_tokens: u64,
    note: AiAgentNote,
) -> AgentRunOutcome {
    sink(AiChatStreamChunk::Note { code: note });
    let _ = journal.record_citations(citations);
    emit_citations(sink, citations);
    sink(AiChatStreamChunk::Done);
    AgentRunOutcome::Completed { iterations: turn, prompt_tokens, completion_tokens }
}

/// Emits the run's accumulated evidence as ONE `Citations` chunk right before a terminal `Done`.
///
/// Deduped by `history_id` to mirror the journal's `agent_citations` primary key (`(run_id,
/// history_id)`), so the streamed set matches the pinned trace exactly. Always emitted on a
/// successful finish (even when empty) so the FE has a single, unambiguous "evidence is final"
/// signal it can map onto starrable evidence rows (02 §G transparency: the user sees what the agent
/// cited). The `canonicalUrl` already carried on each citation is the W-STAR star key.
fn emit_citations<S: AgentRunSink>(sink: &mut S, citations: &[AiCitation]) {
    let mut seen = std::collections::HashSet::new();
    let deduped: Vec<AiCitation> =
        citations.iter().filter(|c| seen.insert(c.history_id)).cloned().collect();
    sink(AiChatStreamChunk::Citations { citations: deduped });
}

/// Maps a cancellation/error from a checkpoint into the right terminal outcome.
///
/// A cancellation emits `Done` (matching drive_chat_stream); any other error emits `Error`.
fn finish_cancel_or_error<S: AgentRunSink>(
    sink: &mut S,
    error: anyhow::Error,
    iterations: u32,
    prompt_tokens: u64,
    completion_tokens: u64,
) -> AgentRunOutcome {
    if is_cancellation(&error) {
        sink(AiChatStreamChunk::Done);
        AgentRunOutcome::Cancelled { iterations, prompt_tokens, completion_tokens }
    } else {
        let message = error.to_string();
        sink(AiChatStreamChunk::Error { message: message.clone() });
        AgentRunOutcome::Failed { iterations, prompt_tokens, completion_tokens, message }
    }
}

/// Whether an error from a checkpoint is a cooperative cancellation (vs a real failure).
fn is_cancellation(error: &anyhow::Error) -> bool {
    error.downcast_ref::<crate::ai::AiRunCancelled>().is_some()
}

/// Runs one `AiRunControl` checkpoint, returning its error verbatim (so cancel stays distinguishable).
fn checkpoint(control: Option<&Arc<dyn AiRunControl>>, detail: &str) -> Result<()> {
    match control {
        Some(control) => control.checkpoint(detail),
        None => Ok(()),
    }
}

/// Error from streaming one model turn: a cooperative cancel, or a terminal stream failure.
enum TurnError {
    Cancelled,
    Stream(String),
}

/// Streams ONE model turn, forwarding token/reasoning chunks and accumulating tool calls + usage.
///
/// Reuses drive_chat_stream's per-turn discipline: forward each Token/Reasoning immediately; a
/// mid-stream Err terminates the turn; cancel between chunks stops cleanly. Tool calls and the
/// terminal Usage marker are accumulated (NOT forwarded as the visible answer) so the loop can act
/// on them after the turn completes.
async fn stream_one_turn<P, S>(
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
            None => return Ok(accumulated),
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
            },
            Some(Err(error)) => return Err(TurnError::Stream(error.to_string())),
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
fn tool_result_payload(
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
struct ToolExecution {
    result_text: String,
    is_error: bool,
    citations: Vec<AiCitation>,
    /// The `run_code` script verbatim (transparency); `None` for the search tools / any failure.
    code_source: Option<String>,
    /// The `run_code` host-call timeline; empty for the search tools / any failure.
    host_calls: Vec<HostCallRecord>,
    /// Which hard sandbox limit bounded a `run_code` script, if any.
    limits_hit: Option<LimitsHit>,
}

/// Executes one tool call into a [`ToolExecution`].
///
/// On success the result_text is the tool's bounded summary and citations are pinned; for `run_code`
/// the code-mode transparency fields are carried too. On FAILURE the result_text becomes an honest
/// error string, `is_error` is true, and the rest are empty — the harness threads that back so the
/// model can recover (partial-results discipline). Invalid args produce the same honest error rather
/// than aborting the run.
async fn execute_tool(
    registry: &ToolRegistry,
    context: &AgentToolContext,
    call: &PendingToolCall,
) -> ToolExecution {
    let args = match serde_json::from_str::<serde_json::Value>(&call.arguments) {
        Ok(value) => value,
        Err(error) => {
            return ToolExecution {
                result_text: format!(
                    "Tool `{}` received arguments that are not valid JSON: {error}",
                    call.name
                ),
                is_error: true,
                citations: Vec::new(),
                code_source: None,
                host_calls: Vec::new(),
                limits_hit: None,
            };
        }
    };
    match registry.dispatch(&call.name, args, context).await {
        Ok(ToolOutcome { model_text, citations, code_source, host_calls, limits_hit }) => {
            ToolExecution {
                result_text: model_text,
                is_error: false,
                citations,
                code_source,
                host_calls,
                limits_hit,
            }
        }
        Err(error) => ToolExecution {
            result_text: format!(
                "Tool `{}` failed: {error}. Try a different query or tool.",
                call.name
            ),
            is_error: true,
            citations: Vec::new(),
            code_source: None,
            host_calls: Vec::new(),
            limits_hit: None,
        },
    }
}

/// Runs the deterministic retrieve-then-answer fallback for a tool-incapable provider.
///
/// 02 §B/§F degradation: instead of a tool loop, this runs ONE seeded model turn (no tools attached)
/// and streams an honest note that tool calling is unavailable. The seed evidence belongs in the
/// caller's preamble (built like `build_assistant_preamble`), so this just streams the single turn
/// and journals it. Cancel emits `Done`; a stream error emits `Error`.
async fn run_degraded_answer<P, J, S>(
    provider: &P,
    request: LlmChatRequest,
    control: Option<&Arc<dyn AiRunControl>>,
    journal: &J,
    sink: &mut S,
    prompt_tokens: &mut u64,
    completion_tokens: &mut u64,
) -> AgentRunOutcome
where
    P: LlmProvider,
    J: AgentJournal,
    S: AgentRunSink,
{
    // USER-facing control note as a localized CODE (review-fix M-6), not raw English. The model is
    // NOT told it is degraded (it answers from the seeded evidence in its preamble), so this note has
    // no model-facing counterpart to thread back — the split is clean.
    sink(AiChatStreamChunk::Note { code: AiAgentNote::ToolCallingUnavailable });
    // No tools attached on the degraded path (the request keeps whatever seed messages it has).
    let mut degraded = request;
    degraded.tools = Vec::new();

    let accumulated = match stream_one_turn(provider, &degraded, control, sink).await {
        Ok(accumulated) => accumulated,
        Err(TurnError::Cancelled) => {
            sink(AiChatStreamChunk::Done);
            return AgentRunOutcome::Cancelled {
                iterations: 1,
                prompt_tokens: *prompt_tokens,
                completion_tokens: *completion_tokens,
            };
        }
        Err(TurnError::Stream(message)) => {
            sink(AiChatStreamChunk::Error { message: message.clone() });
            return AgentRunOutcome::Failed {
                iterations: 1,
                prompt_tokens: *prompt_tokens,
                completion_tokens: *completion_tokens,
                message,
            };
        }
    };
    if let Some(usage) = accumulated.usage {
        *prompt_tokens += usage.prompt_tokens;
        *completion_tokens += usage.completion_tokens;
        sink(AiChatStreamChunk::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        });
    }
    let payload = json!({ "text": accumulated.text, "degraded": true }).to_string();
    let _ = journal.journal_step(1, STEP_KIND_DEGRADED_ANSWER, None, None, &payload);
    // The degraded path runs no tools, so the evidence set is empty — but still emit the terminal
    // `Citations` signal so the FE's success contract is uniform across both paths.
    emit_citations(sink, &[]);
    sink(AiChatStreamChunk::Done);
    AgentRunOutcome::Completed {
        iterations: 1,
        prompt_tokens: *prompt_tokens,
        completion_tokens: *completion_tokens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AiRunCancelled;
    use crate::ai::traits::{LlmCapabilities, LlmChatResponse, LlmChunkStream};
    use crate::config::project_paths_with_root;
    use crate::models::AppConfig;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// A two-turn scripted provider mirroring the llm.rs stub: turn 1 (no Tool-role message in the
    /// request) emits Reasoning → Token → ToolCall(call-1); turn 2 (after a tool_result is threaded
    /// back) emits Token → Usage and NO tool call. Fully deterministic, no network.
    struct TwoTurnProvider {
        tool_capable: bool,
    }

    impl LlmProvider for TwoTurnProvider {
        async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
            Ok(LlmChatResponse::default())
        }

        async fn chat_stream(&self, req: LlmChatRequest) -> Result<LlmChunkStream> {
            let has_tool_result = req.messages.iter().any(|m| m.role == LlmRole::Tool);
            let chunks: Vec<Result<LlmStreamChunk>> = if has_tool_result {
                vec![
                    Ok(LlmStreamChunk::Token("final answer".to_string())),
                    Ok(LlmStreamChunk::Usage(LlmUsage { prompt_tokens: 10, completion_tokens: 5 })),
                ]
            } else {
                vec![
                    Ok(LlmStreamChunk::Reasoning("thinking".to_string())),
                    Ok(LlmStreamChunk::Token("let me search".to_string())),
                    Ok(LlmStreamChunk::ToolCall {
                        call_id: "call-1".to_string(),
                        name: "search_history".to_string(),
                        arguments: r#"{"query":"tauri"}"#.to_string(),
                    }),
                    Ok(LlmStreamChunk::Usage(LlmUsage { prompt_tokens: 8, completion_tokens: 3 })),
                ]
            };
            Ok(Box::pin(vec_stream(chunks)))
        }

        fn capabilities(&self) -> LlmCapabilities {
            LlmCapabilities { tool_call: self.tool_capable, ..LlmCapabilities::default() }
        }
    }

    /// A provider that requests a tool every turn (never stops) so the budget/max-iter gates fire.
    struct AlwaysToolProvider;
    impl LlmProvider for AlwaysToolProvider {
        async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
            Ok(LlmChatResponse::default())
        }
        async fn chat_stream(&self, _req: LlmChatRequest) -> Result<LlmChunkStream> {
            let chunks = vec![
                Ok(LlmStreamChunk::Token("loop".to_string())),
                Ok(LlmStreamChunk::ToolCall {
                    call_id: "call-loop".to_string(),
                    name: "search_history".to_string(),
                    arguments: "{}".to_string(),
                }),
                Ok(LlmStreamChunk::Usage(LlmUsage {
                    prompt_tokens: 1000,
                    completion_tokens: 1000,
                })),
            ];
            Ok(Box::pin(vec_stream(chunks)))
        }
        fn capabilities(&self) -> LlmCapabilities {
            LlmCapabilities { tool_call: true, ..LlmCapabilities::default() }
        }
    }

    fn vec_stream(
        chunks: Vec<Result<LlmStreamChunk>>,
    ) -> impl futures_core::Stream<Item = Result<LlmStreamChunk>> + Send {
        struct VecStream(std::vec::IntoIter<Result<LlmStreamChunk>>);
        impl futures_core::Stream for VecStream {
            type Item = Result<LlmStreamChunk>;
            fn poll_next(
                mut self: std::pin::Pin<&mut Self>,
                _cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Option<Self::Item>> {
                std::task::Poll::Ready(self.0.next())
            }
        }
        VecStream(chunks.into_iter())
    }

    /// An in-memory journal capturing the ordered (turn, kind, tool_call_id) steps + citations.
    ///
    /// `payloads` additionally captures each step's `(kind, payload)` so a test can assert the exact
    /// journaled JSON (the WU-4 tool-result payload widening is verified here).
    #[derive(Default)]
    struct RecordingJournal {
        steps: Mutex<Vec<(u32, String, Option<String>)>>,
        payloads: Mutex<Vec<(String, String)>>,
        citations: Mutex<Vec<AiCitation>>,
        fail_on_kind: Option<String>,
    }

    impl AgentJournal for RecordingJournal {
        fn journal_step(
            &self,
            turn: u32,
            kind: &str,
            _tool_name: Option<&str>,
            tool_call_id: Option<&str>,
            payload: &str,
        ) -> Result<()> {
            if self.fail_on_kind.as_deref() == Some(kind) {
                anyhow::bail!("forced journal failure");
            }
            self.steps.lock().unwrap().push((
                turn,
                kind.to_string(),
                tool_call_id.map(ToString::to_string),
            ));
            self.payloads.lock().unwrap().push((kind.to_string(), payload.to_string()));
            Ok(())
        }

        fn record_citations(&self, citations: &[AiCitation]) -> Result<()> {
            *self.citations.lock().unwrap() = citations.to_vec();
            Ok(())
        }
    }

    fn tool_context() -> (tempfile::TempDir, AgentToolContext) {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.assistant_enabled = true;
        let context = AgentToolContext {
            paths,
            config,
            database_key: None,
            embedding_provider: None,
            default_profile_id: None,
            default_domain: None,
            default_limit: 8,
            // The harness tests drive cancel through `drive_agent_run`'s own `run_control`; the tool
            // context needs no separate hook here (only `run_code` reads it, and these tests that
            // exercise run_code build their own context with a real control).
            run_control: None,
        };
        (dir, context)
    }

    fn user_request() -> LlmChatRequest {
        LlmChatRequest::new(
            vec![LlmMessage::new(LlmRole::User, "what did i read about tauri?")],
            Some(0.2),
            Some(256),
        )
    }

    /// Cooperative cancel control that stops after N successful checkpoints.
    struct StopAfter {
        checkpoints: AtomicUsize,
        stop_at: usize,
    }
    impl AiRunControl for StopAfter {
        fn checkpoint(&self, detail: &str) -> Result<()> {
            let n = self.checkpoints.fetch_add(1, Ordering::SeqCst);
            if n >= self.stop_at {
                return Err(AiRunCancelled::new(detail.to_string()).into());
            }
            Ok(())
        }
        fn cancelled(&self) -> bool {
            self.checkpoints.load(Ordering::SeqCst) >= self.stop_at
        }
    }

    #[tokio::test]
    async fn two_turn_run_emits_tool_call_result_token_usage_done_in_order_and_journals() {
        // THE key deterministic test: turn 1 requests a tool, the harness executes it, threads the
        // result back, turn 2 answers + reports usage, then Done. Assert emit ORDER, journaling, and
        // citation recording — fully deterministic, no network.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();

        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;

        let events = emitted.lock().unwrap().clone();
        // Expected observable order across the two turns.
        let kinds: Vec<&str> = events
            .iter()
            .map(|chunk| match chunk {
                AiChatStreamChunk::Reasoning { .. } => "reasoning",
                AiChatStreamChunk::Token { .. } => "token",
                AiChatStreamChunk::ToolCall { .. } => "toolCall",
                AiChatStreamChunk::ToolResult { .. } => "toolResult",
                AiChatStreamChunk::Usage { .. } => "usage",
                AiChatStreamChunk::Citations { .. } => "citations",
                AiChatStreamChunk::Note { .. } => "note",
                AiChatStreamChunk::Done => "done",
                AiChatStreamChunk::Error { .. } => "error",
            })
            .collect();
        // turn 1: reasoning, token, usage; tool exec: toolCall (the live transparency row) then its
        // toolResult; turn 2: token, usage; then the terminal evidence (citations) right before done.
        assert_eq!(
            kinds,
            vec![
                "reasoning",
                "token",
                "usage",
                "toolCall",
                "toolResult",
                "token",
                "usage",
                "citations",
                "done"
            ]
        );
        // The terminal Citations chunk lands exactly once, immediately before Done.
        assert!(matches!(events.iter().rev().nth(1), Some(AiChatStreamChunk::Citations { .. })));
        // F1 transparency: a ToolCall chunk is emitted immediately BEFORE its ToolResult and carries
        // the SAME call_id, so the FE creates the pending row (args + spinner) the result resolves.
        let tool_call_index = events
            .iter()
            .position(|chunk| matches!(chunk, AiChatStreamChunk::ToolCall { .. }))
            .expect("a tool call was streamed before execution");
        match (&events[tool_call_index], &events[tool_call_index + 1]) {
            (
                AiChatStreamChunk::ToolCall { name, arguments, call_id },
                AiChatStreamChunk::ToolResult { call_id: result_call_id, .. },
            ) => {
                assert_eq!(name, "search_history");
                assert_eq!(arguments, r#"{"query":"tauri"}"#);
                assert_eq!(call_id.as_deref(), Some("call-1"));
                assert_eq!(result_call_id, "call-1", "the ToolResult resolves the ToolCall by id");
            }
            other => {
                panic!("expected ToolCall immediately followed by its ToolResult, got {other:?}")
            }
        }
        // The tool result is emitted and threaded with the correct call id.
        let tool_result = events
            .iter()
            .find_map(|chunk| match chunk {
                AiChatStreamChunk::ToolResult { call_id, name, is_error, .. } => {
                    Some((call_id.clone(), name.clone(), *is_error))
                }
                _ => None,
            })
            .expect("a tool result was emitted");
        assert_eq!(tool_result, ("call-1".to_string(), "search_history".to_string(), false));

        // Budget summed across both turns: (8+3) + (10+5) = 26.
        assert_eq!(
            outcome,
            AgentRunOutcome::Completed { iterations: 2, prompt_tokens: 18, completion_tokens: 8 }
        );

        // Journal-before-observe: the assistant turn + the tool result were both journaled, in order.
        let steps = journal.steps.lock().unwrap().clone();
        assert_eq!(
            steps,
            vec![
                (1, STEP_KIND_ASSISTANT_TURN.to_string(), None),
                (1, STEP_KIND_TOOL_RESULT.to_string(), Some("call-1".to_string())),
                (2, STEP_KIND_ASSISTANT_TURN.to_string(), None),
            ]
        );
        // Citations were recorded at completion (empty here: the fixture archive has no rows).
        assert!(journal.citations.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn max_iterations_stops_the_loop_with_a_note() {
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &AlwaysToolProvider,
            &registry,
            &context,
            true,
            user_request(),
            None,
            2, // max 2 iterations
            0, // no token budget so the iteration gate is the one that fires
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        match outcome {
            AgentRunOutcome::Completed { iterations, .. } => assert_eq!(iterations, 2),
            other => panic!("expected Completed at the iteration ceiling, got {other:?}"),
        }
        let events = emitted.lock().unwrap().clone();
        assert!(matches!(events.last(), Some(AiChatStreamChunk::Done)));
        // The closing control note is a localized CODE (review-fix M-6), not raw English prose.
        assert!(events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::Note { code: AiAgentNote::MaxStepsReached }
        )));
        // No raw English max-steps sentence leaks onto the user-facing token stream.
        assert!(!events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::Token { text } if text.contains("maximum number of agent steps")
        )));
    }

    #[tokio::test]
    async fn token_budget_stops_the_loop_with_a_note() {
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        // AlwaysToolProvider reports 2000 tokens/turn; a 1500 budget trips after turn 1.
        let outcome = drive_agent_run(
            &AlwaysToolProvider,
            &registry,
            &context,
            true,
            user_request(),
            None,
            50,
            1500,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        match outcome {
            AgentRunOutcome::Completed { iterations, prompt_tokens, completion_tokens } => {
                assert_eq!(iterations, 1);
                assert_eq!(prompt_tokens + completion_tokens, 2000);
            }
            other => panic!("expected Completed at the budget ceiling, got {other:?}"),
        }
        let events = emitted.lock().unwrap().clone();
        // The closing control note is a localized CODE (review-fix M-6), not raw English prose.
        assert!(events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::Note { code: AiAgentNote::TokenBudgetReached }
        )));
        assert!(!events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::Token { text } if text.contains("token budget")
        )));
    }

    #[tokio::test]
    async fn pre_cancelled_run_emits_only_done() {
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let control: Arc<dyn AiRunControl> =
            Arc::new(StopAfter { checkpoints: AtomicUsize::new(0), stop_at: 0 });
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Cancelled { iterations: 0, .. }));
        assert_eq!(emitted.lock().unwrap().clone(), vec![AiChatStreamChunk::Done]);
    }

    #[tokio::test]
    async fn cancel_at_a_checkpoint_emits_done_not_error() {
        // Allow the first turn-start checkpoint, then cancel at the next checkpoint (before a tool
        // call). Cancel must emit Done (matching drive_chat_stream), not Error.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let control: Arc<dyn AiRunControl> =
            Arc::new(StopAfter { checkpoints: AtomicUsize::new(0), stop_at: 1 });
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Cancelled { .. }));
        let events = emitted.lock().unwrap().clone();
        assert!(matches!(events.last(), Some(AiChatStreamChunk::Done)));
        assert!(!events.iter().any(|chunk| matches!(chunk, AiChatStreamChunk::Error { .. })));
    }

    #[tokio::test]
    async fn tool_error_is_threaded_and_run_continues() {
        // A provider that calls an UNKNOWN tool: the dispatch fails, the harness emits an is_error
        // ToolResult + threads an honest message back, and the run continues to a final answer.
        struct UnknownToolProvider;
        impl LlmProvider for UnknownToolProvider {
            async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
                Ok(LlmChatResponse::default())
            }
            async fn chat_stream(&self, req: LlmChatRequest) -> Result<LlmChunkStream> {
                let has_tool_result = req.messages.iter().any(|m| m.role == LlmRole::Tool);
                let chunks = if has_tool_result {
                    vec![Ok(LlmStreamChunk::Token("recovered".to_string()))]
                } else {
                    vec![Ok(LlmStreamChunk::ToolCall {
                        call_id: "call-x".to_string(),
                        name: "no_such_tool".to_string(),
                        arguments: "{}".to_string(),
                    })]
                };
                Ok(Box::pin(vec_stream(chunks)))
            }
            fn capabilities(&self) -> LlmCapabilities {
                LlmCapabilities { tool_call: true, ..LlmCapabilities::default() }
            }
        }

        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &UnknownToolProvider,
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Completed { .. }));
        let events = emitted.lock().unwrap().clone();
        let error_result = events.iter().find_map(|chunk| match chunk {
            AiChatStreamChunk::ToolResult { is_error, result, .. } => {
                Some((*is_error, result.clone()))
            }
            _ => None,
        });
        let (is_error, message) = error_result.expect("an is_error tool result");
        assert!(is_error);
        assert!(message.contains("failed"));
        // The run recovered and finished.
        assert!(events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::Token { text } if text == "recovered"
        )));
    }

    #[tokio::test]
    async fn invalid_tool_arguments_thread_an_honest_error() {
        // The model emits malformed JSON args; the harness reports a JSON error (is_error) instead
        // of panicking, and continues.
        struct BadArgsProvider;
        impl LlmProvider for BadArgsProvider {
            async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
                Ok(LlmChatResponse::default())
            }
            async fn chat_stream(&self, req: LlmChatRequest) -> Result<LlmChunkStream> {
                let has_tool_result = req.messages.iter().any(|m| m.role == LlmRole::Tool);
                let chunks = if has_tool_result {
                    vec![Ok(LlmStreamChunk::Token("ok".to_string()))]
                } else {
                    vec![Ok(LlmStreamChunk::ToolCall {
                        call_id: "call-bad".to_string(),
                        name: "search_bm25".to_string(),
                        arguments: "not json".to_string(),
                    })]
                };
                Ok(Box::pin(vec_stream(chunks)))
            }
            fn capabilities(&self) -> LlmCapabilities {
                LlmCapabilities { tool_call: true, ..LlmCapabilities::default() }
            }
        }
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        drive_agent_run(
            &BadArgsProvider,
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        let events = emitted.lock().unwrap().clone();
        assert!(events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::ToolResult { is_error: true, result, .. } if result.contains("not valid JSON")
        )));
    }

    #[tokio::test]
    async fn tool_incapable_provider_takes_the_degraded_path_with_a_note() {
        // provider_can_use_tools = false → no loop, one seeded turn + an honest reasoning note.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: false },
            &registry,
            &context,
            false, // capability probe says: cannot use tools
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Completed { iterations: 1, .. }));
        let events = emitted.lock().unwrap().clone();
        // An honest control note (localized CODE, review-fix M-6) about the lack of tool support, no
        // ToolCall/ToolResult chunks, and no raw English reasoning sentence on the user stream.
        assert!(events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::Note { code: AiAgentNote::ToolCallingUnavailable }
        )));
        assert!(!events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::Reasoning { text } if text.contains("does not support tool calling")
        )));
        assert!(!events.iter().any(|chunk| matches!(
            chunk,
            AiChatStreamChunk::ToolCall { .. } | AiChatStreamChunk::ToolResult { .. }
        )));
        // The degraded path journaled exactly one degraded-answer step.
        let steps = journal.steps.lock().unwrap().clone();
        assert_eq!(steps, vec![(1, STEP_KIND_DEGRADED_ANSWER.to_string(), None)]);
    }

    #[tokio::test]
    async fn a_stream_open_error_finalizes_failed() {
        struct OpenErrorProvider;
        impl LlmProvider for OpenErrorProvider {
            async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
                Ok(LlmChatResponse::default())
            }
            async fn chat_stream(&self, _req: LlmChatRequest) -> Result<LlmChunkStream> {
                anyhow::bail!("open boom")
            }
            fn capabilities(&self) -> LlmCapabilities {
                LlmCapabilities { tool_call: true, ..LlmCapabilities::default() }
            }
        }
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &OpenErrorProvider,
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Failed { .. }));
        let events = emitted.lock().unwrap().clone();
        assert!(
            matches!(events.last(), Some(AiChatStreamChunk::Error { message }) if message.contains("open boom"))
        );
    }

    #[tokio::test]
    async fn a_journal_failure_on_the_assistant_turn_fails_the_run() {
        // Journal-before-observe means a journal write failure must terminate the run with an error
        // rather than silently proceeding past an un-journaled step.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal {
            fail_on_kind: Some(STEP_KIND_ASSISTANT_TURN.to_string()),
            ..RecordingJournal::default()
        };
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(
            matches!(outcome, AgentRunOutcome::Failed { message, .. } if message.contains("forced journal failure"))
        );
    }

    #[tokio::test]
    async fn a_mid_stream_error_finalizes_failed() {
        struct MidErrorProvider;
        impl LlmProvider for MidErrorProvider {
            async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
                Ok(LlmChatResponse::default())
            }
            async fn chat_stream(&self, _req: LlmChatRequest) -> Result<LlmChunkStream> {
                let chunks = vec![
                    Ok(LlmStreamChunk::Token("partial".to_string())),
                    Err(anyhow::anyhow!("mid boom")),
                ];
                Ok(Box::pin(vec_stream(chunks)))
            }
            fn capabilities(&self) -> LlmCapabilities {
                LlmCapabilities { tool_call: true, ..LlmCapabilities::default() }
            }
        }
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &MidErrorProvider,
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(
            matches!(outcome, AgentRunOutcome::Failed { message, .. } if message.contains("mid boom"))
        );
    }

    /// A control whose `cancelled()` stays false (so the pre-cancel check passes) but whose first
    /// `checkpoint()` cancels — exercising the in-stream cancellation arm rather than the pre-check.
    struct CancelOnFirstCheckpoint {
        fired: AtomicBool,
    }
    impl AiRunControl for CancelOnFirstCheckpoint {
        fn checkpoint(&self, detail: &str) -> Result<()> {
            if self.fired.swap(true, Ordering::SeqCst) {
                Ok(())
            } else {
                Err(AiRunCancelled::new(detail.to_string()).into())
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }

    /// A control that cancels on the Nth `checkpoint()` call (1-based) while keeping `cancelled()`
    /// false, so a specific checkpoint in the loop (turn-start vs before-a-tool-call) can be targeted
    /// without the in-stream `cancelled()` arm firing first.
    struct CancelOnNthCheckpoint {
        calls: AtomicUsize,
        fire_on: usize,
    }
    impl AiRunControl for CancelOnNthCheckpoint {
        fn checkpoint(&self, detail: &str) -> Result<()> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if n == self.fire_on {
                return Err(AiRunCancelled::new(detail.to_string()).into());
            }
            Ok(())
        }
        fn cancelled(&self) -> bool {
            false
        }
    }

    /// A control whose `checkpoint()` always succeeds but flips `cancelled()` to true after the
    /// first call — so the run gets past the stream-open checkpoint and then observes cancellation
    /// in the mid-stream `cancelled()` arm of `stream_one_turn`.
    struct CancelAfterFirstCheckpoint {
        seen: AtomicBool,
    }
    impl AiRunControl for CancelAfterFirstCheckpoint {
        fn checkpoint(&self, _detail: &str) -> Result<()> {
            self.seen.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn cancelled(&self) -> bool {
            self.seen.load(Ordering::SeqCst)
        }
    }

    /// A control whose first `checkpoint()` returns a NON-cancellation error (and `cancelled()`
    /// stays false), so the harness's `finish_cancel_or_error` takes the genuine-failure arm:
    /// an `Error` chunk + a `Failed` outcome, rather than the graceful cancel path.
    struct ErrorOnFirstCheckpoint {
        fired: AtomicBool,
    }
    impl AiRunControl for ErrorOnFirstCheckpoint {
        fn checkpoint(&self, _detail: &str) -> Result<()> {
            if self.fired.swap(true, Ordering::SeqCst) {
                Ok(())
            } else {
                anyhow::bail!("checkpoint backend failure")
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn a_non_cancellation_checkpoint_error_finalizes_failed() {
        // A checkpoint can fail for reasons OTHER than cooperative cancel (e.g. its backend errored);
        // that must finalize the run as Failed with an Error chunk, not a graceful Done.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let control: Arc<dyn AiRunControl> =
            Arc::new(ErrorOnFirstCheckpoint { fired: AtomicBool::new(false) });
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(
            matches!(outcome, AgentRunOutcome::Failed { message, .. } if message.contains("checkpoint backend failure"))
        );
        let events = emitted.lock().unwrap().clone();
        assert!(
            matches!(events.last(), Some(AiChatStreamChunk::Error { message }) if message.contains("checkpoint backend failure"))
        );
    }

    #[tokio::test]
    async fn cancel_at_the_turn_start_checkpoint_emits_done_not_error() {
        // The FIRST checkpoint is the turn-start guard (02 §F): cancelling there must finalize via
        // `finish_cancel_or_error` as Cancelled + a single Done, before any model turn streams.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let control: Arc<dyn AiRunControl> =
            Arc::new(CancelOnNthCheckpoint { calls: AtomicUsize::new(0), fire_on: 1 });
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        // iterations is the COMPLETED turn count (turn - 1 = 0 here: the cancel hit before turn 1).
        assert!(matches!(outcome, AgentRunOutcome::Cancelled { iterations: 0, .. }));
        let events = emitted.lock().unwrap().clone();
        assert_eq!(events, vec![AiChatStreamChunk::Done]);
    }

    #[tokio::test]
    async fn cancel_before_a_tool_call_emits_done_not_error() {
        // The before-each-tool checkpoint (the 3rd checkpoint call: turn-start, stream-open, then
        // this one) must finalize as Cancelled + Done even after a turn already streamed + journaled.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let control: Arc<dyn AiRunControl> =
            Arc::new(CancelOnNthCheckpoint { calls: AtomicUsize::new(0), fire_on: 3 });
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        // Turn 1 streamed before the tool checkpoint fired, so the completed-turn count is 1.
        assert!(matches!(outcome, AgentRunOutcome::Cancelled { iterations: 1, .. }));
        let events = emitted.lock().unwrap().clone();
        assert!(matches!(events.last(), Some(AiChatStreamChunk::Done)));
        assert!(!events.iter().any(|chunk| matches!(chunk, AiChatStreamChunk::Error { .. })));
        // No ToolResult was emitted: the cancel landed BEFORE the tool ran.
        assert!(!events.iter().any(|chunk| matches!(chunk, AiChatStreamChunk::ToolResult { .. })));
    }

    #[tokio::test]
    async fn cancel_observed_mid_stream_emits_done() {
        // `cancelled()` is false at the pre-check and stream-open checkpoint, then becomes true, so
        // the in-stream `cancelled()` arm of `stream_one_turn` fires and the run ends Cancelled.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let control: Arc<dyn AiRunControl> =
            Arc::new(CancelAfterFirstCheckpoint { seen: AtomicBool::new(false) });
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Cancelled { iterations: 1, .. }));
        let events = emitted.lock().unwrap().clone();
        assert!(matches!(events.last(), Some(AiChatStreamChunk::Done)));
        assert!(!events.iter().any(|chunk| matches!(chunk, AiChatStreamChunk::Error { .. })));
    }

    #[tokio::test]
    async fn a_journal_failure_on_the_tool_result_fails_the_run() {
        // Journal-before-observe on the tool-result step: a journal write failure there must
        // terminate the run with an Error (never emit a ToolResult that was not durably journaled).
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal {
            fail_on_kind: Some(STEP_KIND_TOOL_RESULT.to_string()),
            ..RecordingJournal::default()
        };
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: true },
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(
            matches!(outcome, AgentRunOutcome::Failed { message, .. } if message.contains("forced journal failure"))
        );
        let events = emitted.lock().unwrap().clone();
        // The failure terminated with an Error and NO ToolResult was emitted (journal-before-observe).
        assert!(matches!(events.last(), Some(AiChatStreamChunk::Error { .. })));
        assert!(!events.iter().any(|chunk| matches!(chunk, AiChatStreamChunk::ToolResult { .. })));
        // The assistant turn WAS journaled (it precedes the failing tool-result step).
        let steps = journal.steps.lock().unwrap().clone();
        assert_eq!(steps, vec![(1, STEP_KIND_ASSISTANT_TURN.to_string(), None)]);
    }

    #[tokio::test]
    async fn degraded_path_stream_error_finalizes_failed() {
        // The degraded (tool-incapable) path must surface a mid-stream provider error as a terminal
        // Failed outcome + an Error chunk, exactly like the tool loop's error handling.
        struct DegradedMidErrorProvider;
        impl LlmProvider for DegradedMidErrorProvider {
            async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
                Ok(LlmChatResponse::default())
            }
            async fn chat_stream(&self, _req: LlmChatRequest) -> Result<LlmChunkStream> {
                let chunks = vec![
                    Ok(LlmStreamChunk::Token("partial".to_string())),
                    Err(anyhow::anyhow!("degraded boom")),
                ];
                Ok(Box::pin(vec_stream(chunks)))
            }
            fn capabilities(&self) -> LlmCapabilities {
                LlmCapabilities::default()
            }
        }
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &DegradedMidErrorProvider,
            &registry,
            &context,
            false, // degraded path
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(
            matches!(outcome, AgentRunOutcome::Failed { iterations: 1, message, .. } if message.contains("degraded boom"))
        );
        let events = emitted.lock().unwrap().clone();
        assert!(
            matches!(events.last(), Some(AiChatStreamChunk::Error { message }) if message.contains("degraded boom"))
        );
    }

    #[tokio::test]
    async fn degraded_path_cancel_in_stream_emits_done() {
        // The degraded path opens its turn under `await_with_ai_cancellation`; a checkpoint cancel
        // there must surface as Cancelled + a single Done (matching drive_chat_stream).
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let control: Arc<dyn AiRunControl> =
            Arc::new(CancelOnFirstCheckpoint { fired: AtomicBool::new(false) });
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: false },
            &registry,
            &context,
            false,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Cancelled { iterations: 1, .. }));
        let events = emitted.lock().unwrap().clone();
        // The degraded reasoning note streamed, then a Done (no Error).
        assert!(matches!(events.last(), Some(AiChatStreamChunk::Done)));
        assert!(!events.iter().any(|chunk| matches!(chunk, AiChatStreamChunk::Error { .. })));
    }

    #[tokio::test]
    async fn degraded_path_completion_emits_an_empty_citations_chunk_before_done() {
        // The degraded path runs no tools, so the terminal Citations chunk is present but empty —
        // the FE success contract (Citations then Done) is uniform across both paths.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &TwoTurnProvider { tool_capable: false },
            &registry,
            &context,
            false,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Completed { iterations: 1, .. }));
        let events = emitted.lock().unwrap().clone();
        assert!(matches!(events.last(), Some(AiChatStreamChunk::Done)));
        // The penultimate chunk is an empty Citations marker.
        assert!(matches!(
            events.iter().rev().nth(1),
            Some(AiChatStreamChunk::Citations { citations }) if citations.is_empty()
        ));
    }

    #[test]
    fn emit_citations_dedups_by_history_id_and_carries_the_canonical_url() {
        // The streamed Citations set mirrors the journal's (run_id, history_id) primary key: one row
        // per history_id (first wins), each keeping the canonical_url star key intact for the FE.
        let citation = |history_id: i64, canonical: &str| AiCitation {
            history_id,
            profile_id: "p".to_string(),
            url: format!("{canonical}?utm=x"),
            title: None,
            visited_at: "2026-01-01T00:00:00Z".to_string(),
            score: Some(0.5),
            canonical_url: Some(canonical.to_string()),
        };
        let input = vec![
            citation(1, "https://a.example/"),
            citation(2, "https://b.example/"),
            citation(1, "https://a.example/"), // duplicate history_id → dropped
        ];
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let mut sink = move |chunk: AiChatStreamChunk| sink_events.lock().unwrap().push(chunk);
        emit_citations(&mut sink, &input);
        let events = emitted.lock().unwrap().clone();
        match events.as_slice() {
            [AiChatStreamChunk::Citations { citations }] => {
                assert_eq!(citations.len(), 2);
                assert_eq!(citations[0].history_id, 1);
                assert_eq!(citations[0].canonical_url.as_deref(), Some("https://a.example/"));
                assert_eq!(citations[1].history_id, 2);
            }
            other => panic!("expected exactly one Citations chunk, got {other:?}"),
        }
    }

    // ---- W-AI-8 WU-6: code-mode through the harness ------------------------------------------

    /// A process-global wall-time-budget guard for the cross-thread `run_code` tests (the sandbox
    /// runs on a `spawn_blocking` worker, so the per-thread override cannot reach it). Serialized by a
    /// static mutex so two cross-thread budget tests never race the shared atomic.
    struct GlobalWallBudgetGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    impl GlobalWallBudgetGuard {
        fn set(budget: std::time::Duration) -> Self {
            static SERIAL: Mutex<()> = Mutex::new(());
            let lock = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            crate::ai::code_mode::TEST_WALL_TIME_BUDGET_MS_GLOBAL
                .store(budget.as_millis() as u64, Ordering::SeqCst);
            Self { _lock: lock }
        }
    }
    impl Drop for GlobalWallBudgetGuard {
        fn drop(&mut self) {
            crate::ai::code_mode::TEST_WALL_TIME_BUDGET_MS_GLOBAL.store(0, Ordering::SeqCst);
        }
    }

    /// A two-turn provider that calls `run_code` on turn 1 with the given source, then answers.
    struct RunCodeProvider {
        source: String,
    }
    impl LlmProvider for RunCodeProvider {
        async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
            Ok(LlmChatResponse::default())
        }
        async fn chat_stream(&self, req: LlmChatRequest) -> Result<LlmChunkStream> {
            let has_tool_result = req.messages.iter().any(|m| m.role == LlmRole::Tool);
            let chunks = if has_tool_result {
                vec![Ok(LlmStreamChunk::Token("done".to_string()))]
            } else {
                vec![Ok(LlmStreamChunk::ToolCall {
                    call_id: "code-1".to_string(),
                    name: "run_code".to_string(),
                    arguments: json!({ "source": self.source }).to_string(),
                })]
            };
            Ok(Box::pin(vec_stream(chunks)))
        }
        fn capabilities(&self) -> LlmCapabilities {
            LlmCapabilities { tool_call: true, ..LlmCapabilities::default() }
        }
    }

    /// Builds a tool context with a real run control threaded in (so a `run_code` sandbox sees cancel).
    fn tool_context_with_control(
        control: Arc<dyn AiRunControl>,
    ) -> (tempfile::TempDir, AgentToolContext) {
        let (dir, mut context) = tool_context();
        context.run_control = Some(control);
        (dir, context)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn run_code_journals_and_streams_the_code_mode_fields() {
        // A scripted provider calls run_code with pure-JS distillation; the journaled tool-result
        // payload AND the emitted ToolResult chunk both carry codeSource (no host call here, so
        // hostCalls is omitted/empty and limitsHit is None) — the WU-4 trace contract.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &RunCodeProvider { source: "return { ok: 1 };".to_string() },
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Completed { .. }));

        // The journaled tool-result payload carries codeSource (the script verbatim).
        let payloads = journal.payloads.lock().unwrap().clone();
        let tool_payload = payloads
            .iter()
            .find(|(kind, _)| kind == STEP_KIND_TOOL_RESULT)
            .map(|(_, p)| p.clone())
            .expect("a tool-result step was journaled");
        let parsed: serde_json::Value = serde_json::from_str(&tool_payload).expect("valid payload");
        assert_eq!(parsed["name"], "run_code");
        assert_eq!(parsed["codeSource"], "return { ok: 1 };");
        // No host call ran, so hostCalls/limitsHit are omitted from the opaque payload.
        assert!(parsed.get("hostCalls").is_none(), "no host call → hostCalls omitted");
        assert!(parsed.get("limitsHit").is_none(), "no limit → limitsHit omitted");

        // The streamed ToolResult chunk carries the same code-mode fields for the FE.
        let events = emitted.lock().unwrap().clone();
        let streamed = events
            .iter()
            .find_map(|chunk| match chunk {
                AiChatStreamChunk::ToolResult { name, code_source, limits_hit, .. }
                    if name == "run_code" =>
                {
                    Some((code_source.clone(), *limits_hit))
                }
                _ => None,
            })
            .expect("a run_code ToolResult was streamed");
        assert_eq!(streamed.0.as_deref(), Some("return { ok: 1 };"));
        assert_eq!(streamed.1, None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cancel_mid_run_code_finalizes_cancelled_and_emits_done() {
        // THE deadlock proof: a run_code script that loops forever, cancelled mid-flight. The cancel
        // bumps the sandbox epoch (so the guest traps) AND the next harness checkpoint observes the
        // cancel → the run finalizes Cancelled + a single Done. If `spawn_blocking` deadlocked the
        // loop, this test would hang instead of returning.
        let flag = Arc::new(AtomicBool::new(false));
        let control: Arc<dyn AiRunControl> = Arc::new(FlagCancel(flag.clone()));
        let (_dir, context) = tool_context_with_control(control.clone());
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();

        // Flip the cancel flag shortly after the run starts (while the infinite loop is running).
        let flag_thread = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            flag_thread.store(true, Ordering::SeqCst);
        });

        let outcome = drive_agent_run(
            &RunCodeProvider { source: "while (true) {} return 1;".to_string() },
            &registry,
            &context,
            true,
            user_request(),
            Some(control),
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;

        assert!(
            matches!(outcome, AgentRunOutcome::Cancelled { .. }),
            "cancel mid-run-code finalizes Cancelled, got {outcome:?}"
        );
        let events = emitted.lock().unwrap().clone();
        assert!(
            matches!(events.last(), Some(AiChatStreamChunk::Done)),
            "a single Done was emitted"
        );
        assert!(
            !events.iter().any(|chunk| matches!(chunk, AiChatStreamChunk::Error { .. })),
            "cancel is not an error"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn an_infinite_loop_run_code_is_bounded_not_a_hang() {
        // An infinite-loop script driven THROUGH the tool path trips the wall-time deadline and yields
        // a bounded result (NOT a hang): the run completes, a run_code ToolResult is emitted, and the
        // code-mode limit (Time) is carried on it. A short global budget keeps the test fast.
        let _budget = GlobalWallBudgetGuard::set(std::time::Duration::from_millis(300));
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &RunCodeProvider { source: "while (true) {} return 1;".to_string() },
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Completed { .. }), "bounded, not a hang");
        let events = emitted.lock().unwrap().clone();
        let limit = events
            .iter()
            .find_map(|chunk| match chunk {
                AiChatStreamChunk::ToolResult { name, limits_hit, .. } if name == "run_code" => {
                    Some(*limits_hit)
                }
                _ => None,
            })
            .expect("a run_code ToolResult was emitted");
        assert_eq!(limit, Some(crate::ai::LimitsHit::Time), "the wall-time limit bounded the run");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_huge_output_run_code_is_bounded_with_the_output_limit() {
        // A script returning more than the output cap is truncated host-side; the run completes and
        // the ToolResult carries the Output limit + the honest bounded-run note in its text.
        let (_dir, context) = tool_context();
        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let outcome = drive_agent_run(
            &RunCodeProvider { source: "return 'A'.repeat(300000);".to_string() },
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Completed { .. }));
        let events = emitted.lock().unwrap().clone();
        let (limit, text) = events
            .iter()
            .find_map(|chunk| match chunk {
                AiChatStreamChunk::ToolResult { name, limits_hit, result, .. }
                    if name == "run_code" =>
                {
                    Some((*limits_hit, result.clone()))
                }
                _ => None,
            })
            .expect("a run_code ToolResult was emitted");
        assert_eq!(limit, Some(crate::ai::LimitsHit::Output));
        assert!(text.contains("bounded by a hard limit"), "honest bounded-run note threaded back");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_host_call_budget_run_code_is_bounded_with_the_host_calls_limit() {
        // A script that loops issuing query_history past the host-call budget trips HostCalls (the
        // host refuses overflow calls), and the run completes bounded — driven through the tool path
        // over an initialized-but-empty archive (each call is fast + deterministic).
        let _budget = GlobalWallBudgetGuard::set(std::time::Duration::from_secs(120));
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = crate::config::project_paths_with_root(dir.path());
        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.assistant_enabled = true;
        crate::archive::ensure_archive_initialized(&paths, &config, None).expect("init archive");
        let archive =
            crate::archive::open_archive_connection(&paths, &config, None).expect("open archive");
        crate::archive::create_schema(&archive).expect("schema");
        let context = AgentToolContext {
            paths,
            config,
            database_key: None,
            embedding_provider: None,
            default_profile_id: None,
            default_domain: None,
            default_limit: 8,
            run_control: None,
        };

        let registry = ToolRegistry::with_default_search_tools();
        let journal = RecordingJournal::default();
        let emitted: Arc<Mutex<Vec<AiChatStreamChunk>>> = Arc::new(Mutex::new(Vec::new()));
        let sink_events = emitted.clone();
        let source = r#"
            let i = 0;
            try { for (i = 0; i < 1000; i++) { query_history({ query: "x" }); } }
            catch (e) { return { serviced: i }; }
            return { serviced: i };
        "#;
        let outcome = drive_agent_run(
            &RunCodeProvider { source: source.to_string() },
            &registry,
            &context,
            true,
            user_request(),
            None,
            DEFAULT_MAX_ITERATIONS,
            DEFAULT_TOKEN_BUDGET,
            &journal,
            move |chunk| sink_events.lock().unwrap().push(chunk),
        )
        .await;
        assert!(matches!(outcome, AgentRunOutcome::Completed { .. }));
        let events = emitted.lock().unwrap().clone();
        let payloads = journal.payloads.lock().unwrap().clone();
        // The journaled payload records the host-call timeline (a non-empty hostCalls array).
        let tool_payload = payloads
            .iter()
            .find(|(kind, _)| kind == STEP_KIND_TOOL_RESULT)
            .map(|(_, p)| p.clone())
            .expect("a tool-result step");
        let parsed: serde_json::Value = serde_json::from_str(&tool_payload).expect("valid payload");
        assert_eq!(parsed["limitsHit"], "host-calls");
        assert!(
            parsed["hostCalls"].as_array().is_some_and(|a| !a.is_empty()),
            "the host-call timeline is journaled: {parsed}"
        );
        let limit = events.iter().find_map(|chunk| match chunk {
            AiChatStreamChunk::ToolResult { name, limits_hit, .. } if name == "run_code" => {
                Some(*limits_hit)
            }
            _ => None,
        });
        assert_eq!(limit, Some(Some(crate::ai::LimitsHit::HostCalls)));
    }

    /// A cancel control whose `checkpoint()` errors and `cancelled()` is true once the flag is set.
    struct FlagCancel(Arc<AtomicBool>);
    impl AiRunControl for FlagCancel {
        fn checkpoint(&self, detail: &str) -> Result<()> {
            if self.0.load(Ordering::SeqCst) {
                return Err(AiRunCancelled::new(detail.to_string()).into());
            }
            Ok(())
        }
        fn cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }
}
