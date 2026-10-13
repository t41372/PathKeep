//! Hand-rolled, durable, tool-executing streaming agent loop (W-AI-7 core, 02 §F).
//!
//! ## Responsibilities
//! - drive ONE agent run as a thin tokio while-loop over `LlmProvider::chat_stream`: forward
//!   token/reasoning chunks, accumulate the turn's tool calls + usage, then EXECUTE each tool call
//!   against the owned [`ToolRegistry`] and thread the result back as a `tool_result` message
//! - enforce the run invariants: cooperative cancel at every checkpoint, a per-run token budget, a
//!   max-iteration ceiling, and stall/no-progress detection
//! - JOURNAL every model turn + tool result BEFORE observing/emitting it, and pin the citations.
//!   Nothing reads the step journal back: an interrupted answer is not resumed (decided
//!   2026-10-09), and a reopened chat reads only the citations and the run's token usage.
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

/// Runaway BACKSTOP on model turns in one agent run — NOT a functional step budget.
///
/// On a healthy, large-context run the binding ceiling is the live-context budget
/// ([`DEFAULT_TOKEN_BUDGET`]); a strong reasoning model converges on its own (a turn with no tool
/// calls ends the run). This cap is the FALLBACK bound: it stops a pathological model that re-issues
/// tool calls forever, and it becomes the *effective* ceiling whenever the live-context gate cannot
/// bind — e.g. a provider that reports no streaming usage (so the budget falls back to a coarse
/// estimate) or a context window much smaller than the budget. It is set FAR above realistic
/// convergence (a heavy map-reduce over a full day's history pages through a few dozen `run_code`
/// rounds at most). Reaching it does NOT end the run silently: it forces one final tool-free synthesis
/// turn ([`run_final_synthesis_turn`]) so the work already done still produces an answer. `0` disables
/// the backstop (tests that want pure token-budget control).
pub const DEFAULT_MAX_ITERATIONS: u32 = 64;

/// Default ceiling on a SINGLE turn's prompt size — the model's live context-window occupancy.
///
/// The primary ceiling for a large-context provider that reports usage. It is measured against the
/// LAST turn's prompt tokens (the full resent history = exactly what occupies the model's context that
/// turn), NOT the cumulative sum across turns: cumulative double-counts the history re-sent every
/// round, so it would fire long before the window is actually full ("context isn't even full, why
/// stop?"). Sized to leave headroom under a 128k-class window for the forced final answer. When a
/// turn's prompt crosses this we run the final synthesis turn instead of starting another tool round.
///
/// Caveats (see [`DEFAULT_MAX_ITERATIONS`], which backstops these): this is a fixed value, not the
/// provider's actual window, so a much smaller-context model can overflow (→ honest provider error)
/// before it binds; and a provider that omits streaming usage falls back to a coarse size estimate
/// ([`estimate_context_tokens`]). `0` means "no budget" (tests that want pure iteration control).
pub const DEFAULT_TOKEN_BUDGET: u64 = 110_000;

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

/// Host-computed facts the model cannot derive inside the run, threaded into the first-turn system
/// message so date/recency questions resolve correctly.
///
/// ## Why this exists
/// The model has NO clock and NO date sense inside a PathKeep agent run: the `run_code` sandbox has
/// no real clock (`Date.now()` is 0) and the model only knows its own training cutoff, so it would
/// otherwise GUESS "today" — typically a year behind a 2026 archive — search the wrong dates, find
/// nothing, and loop. It also cannot know the archive's date range without enumerating it. Both must
/// be computed on the HOST (the worker, which has the OS clock + timezone + a bounded archive read)
/// and handed to the model up front. This struct carries those host facts; [`build_agent_system_context`]
/// renders the concise, factual block. Kept separate from the rendering so the builder is a pure,
/// lethally-testable function (no clock / no I/O).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentSystemContext {
    /// Current local date in `YYYY-MM-DD` form (host clock).
    pub current_date: String,
    /// Current local time + UTC offset, e.g. `14:32 +08:00` (host clock + OS timezone).
    pub current_time: String,
    /// Weekday name for `current_date`, e.g. `Tuesday`, so relative dates ("last Friday") resolve.
    pub weekday: String,
    /// IANA/OS timezone name, e.g. `Asia/Taipei` (or `UTC` when the host lookup is unavailable).
    pub timezone: String,
    /// Earliest visit date (`YYYY-MM-DD`) in the archive, or `None` when the archive is empty.
    pub archive_earliest: Option<String>,
    /// Latest visit date (`YYYY-MM-DD`) in the archive, or `None` when the archive is empty.
    pub archive_latest: Option<String>,
    /// Total visible (non-reverted) visit count, so the model knows the data volume.
    pub archive_visit_count: usize,
}

/// Renders the concise, factual system-context block prepended to every agent run's first turn.
///
/// SHORT and factual by design (no fluff): the model needs (1) the real current date/time/timezone
/// — which it cannot get from the clockless sandbox or its training cutoff — so relative dates like
/// "last Friday" resolve against the right year; (2) the archive's date span + size so it searches
/// the right range and knows the data volume; (3) a one-line retrieval hint that an empty query
/// lists recent visits and that semantic search may be keyword-only without an embedding provider.
/// Pure: it formats already-resolved host facts, so it is deterministic and unit-testable.
pub fn build_agent_system_context(context: &AgentSystemContext) -> String {
    let mut block = String::new();
    block.push_str(&format!(
        "Current date: {date} ({weekday}), {time} {tz}.",
        date = context.current_date,
        weekday = context.weekday,
        time = context.current_time,
        tz = context.timezone,
    ));

    match (&context.archive_earliest, &context.archive_latest) {
        (Some(earliest), Some(latest)) => block.push_str(&format!(
            " The user's browser history spans {earliest} to {latest} (~{count} visits).",
            count = context.archive_visit_count,
        )),
        _ => block.push_str(" The user's browser history archive is currently empty."),
    }

    // NOTE: built from separate string literals via `concat!` rather than a backslash-continued
    // string. A `\`-at-end-of-line literal in production code (before the test module) desyncs the
    // coverage gate's line-masking, which counts braces over the masked source.
    block.push_str(concat!(
        " To enumerate recent visits or find the date range, call a search tool (or run_code's",
        " query_history) with an empty query. Semantic search may be keyword-only if no embedding",
        " provider is configured.",
    ));

    // Date resolution: teach the model to convert relative dates into concrete parameters.
    block.push_str(concat!(
        " Convert relative dates ('last Friday', 'this week', 'last month') to concrete",
        " start_date/end_date values based on the current date above. Never search for a date",
        " as text.",
    ));

    // Tool overview: concise guide for when to use each tool.
    block.push_str(concat!(
        " Tools: search_history — find visits by keyword, domain, date range, or starred status",
        " (empty query lists recent visits).",
        " run_code — compute/aggregate across visits (counts, grouping, joins) when a single",
        " search can't answer the question.",
        " intelligence_report — retrieve pre-computed analytics (top sites, sessions, search",
        " trails, trends, patterns, daily insights).",
        " list_stars / list_annotations — enumerate the user's favorites or notes/tags.",
    ));

    // Coverage strategy: intelligence_report gives the STATISTICAL shape of a period; the CONTENT of
    // what the user actually did must be read from the actual visits (it is not in the aggregates).
    block.push_str(concat!(
        " For the quantitative SHAPE of a time period ('top sites this week', 'how many visits Friday',",
        " 'how are my sessions bounded'), call intelligence_report for that range FIRST — it covers ALL",
        " visits in the period, but as STATISTICS ONLY: counts, top domains, and session structure. It",
        " does NOT tell you WHAT the user actually did or read. For the CONTENT or topics of a period or",
        " session ('what did I do last Friday', 'what was I researching'), read the ACTUAL visits instead",
        " — call intelligence_report report=\"session_detail\" for one session's real URLs/titles/queries,",
        " or write a run_code script that pages query_history over the range and reasons over the",
        " titles/URLs; NEVER describe content from a session's autoTitle or its counts. search_history",
        " returns only the TOP matches and sets has_more=true when more matches exist than it returned:",
        " when has_more is true you are seeing a sample, NOT the full set, so do NOT summarize it as if it",
        " were complete — use run_code to page query_history and aggregate the FULL set in code (rows stay",
        " in the sandbox, so only your small returned summary costs context — you can process hundreds of",
        " visits this way), and use search_history only to find or drill into specific pages. If the",
        " intelligence index is not built yet, fall back to search but say the summary is based on a",
        " partial sample.",
    ));

    // Time direction & completeness: search defaults to most-recent, relevance-ranked, so older
    // matches exist but are not returned by default — use sort + pagination for history-wide questions.
    block.push_str(concat!(
        " search_history defaults to the most-recent, relevance-ranked matches, so OLDER visits that",
        " match still exist but are NOT in the default results. For 'when did I FIRST/earliest do X',",
        " pass sort=\"oldest\" (the earliest matches return first). For 'ALL of X across all time',",
        " page through results using the returned next_cursor until has_more is false; if has_more stays",
        " true but NO next_cursor comes back, you have hit the retrieval cap — narrow the date range (or",
        " add filters) to reach the rest. NEVER conclude the first, last, or only occurrence of something",
        " from a single default-sorted page — if you have not paged to the end or sorted oldest, say your",
        " finding may be incomplete.",
    ));

    // Grounding gate: anti-hallucination rule.
    block.push_str(concat!(
        " IMPORTANT: Every page title, URL, time, or count in your answer MUST come from a row",
        " returned by a tool in this conversation. If no tool returned matching data, say you",
        " found no data — do NOT fabricate visits, titles, or URLs. Cite the visit ID, date,",
        " and URL for each claim.",
    ));

    // Local-time rule: the ENRICHED tool surfaces carry the user's LOCAL time so the clockless model
    // must NOT recompute dates from raw epoch ms (a real trace showed it burning hundreds of reasoning
    // lines doing exactly that). The claim is SCOPED to the enriched reports — overview/day_insights are
    // UTC, so promising "all rows are local" would make the model trust their UTC strings as local.
    // `concat!`, not a `\`-continued literal, to keep the coverage masking sane.
    block.push_str(concat!(
        " The search_history results, the run_code query_history rows, and the sessions / search_trails /",
        " session_detail reports already include the user's LOCAL time in the timezone above: search_history",
        " result lines LEAD with the local date-time; query_history rows carry visitedAtLocal + localDate;",
        " sessions and search trails carry firstVisitLocal/lastVisitLocal; session_detail visits carry",
        " visitTimeLocal. Read those LOCAL fields directly. When a row ALSO carries a raw epoch-millisecond",
        " field (e.g. firstVisitMs), IGNORE it — do not convert or cross-check it; never recompute a date",
        " from epoch milliseconds, and the run_code sandbox clock reads 0 so Date.now() there is meaningless.",
        " (Local times apply your CURRENT timezone offset to every visit, so a historical visit from a",
        " different daylight-saving period may read up to an hour off.) The overview and day_insights reports",
        " are NOT localized — their date buckets and *At timestamps are UTC; for a precise local-day question",
        " prefer the sessions / search_trails / session_detail reports, or a run_code script that groups by",
        " localDate.",
    ));

    // Stop rule: efficiency.
    block.push_str(concat!(
        " Once the tool results give you enough evidence, STOP calling tools and write the final",
        " answer with citations — do not keep searching.",
    ));

    block
}

/// Resolves the host facts for [`AgentSystemContext`] from the OS clock/timezone + a bounded archive
/// read, so the worker can prepend the first-turn system-context block (date/time/tz + archive span).
///
/// This is the HOST side that the clockless sandbox cannot do: the current local date, time, weekday,
/// and UTC offset come from `chrono::Local::now()`; the IANA timezone name reuses the same lookup the
/// backup scheduler uses (`current_timezone_name`); and the archive span (earliest/latest visit date
/// plus visible count) reuses the Dashboard "Span" read model (`load_dashboard_snapshot`), which is
/// cached and bounded so it never scans the corpus even at 14.4M visits. A read failure degrades to
/// an empty-archive context (the run still gets the date/time, never blocked on the span).
pub fn resolve_agent_system_context(vault: &crate::Vault) -> AgentSystemContext {
    let now = chrono::Local::now();
    // Archive span via the cached Dashboard read model; degrade to empty on any read error so the
    // run is never blocked from getting at least the date/time context.
    let (archive_earliest, archive_latest, archive_visit_count) =
        match crate::load_dashboard_snapshot(vault) {
            Ok(snapshot) => (
                snapshot.earliest_visit_at.as_deref().map(iso_date_only),
                snapshot.latest_visit_at.as_deref().map(iso_date_only),
                snapshot.total_visits,
            ),
            Err(_) => (None, None, 0),
        };
    AgentSystemContext {
        current_date: now.format("%Y-%m-%d").to_string(),
        current_time: now.format("%H:%M %:z").to_string(),
        weekday: now.format("%A").to_string(),
        timezone: crate::archive::current_timezone_name(),
        archive_earliest,
        archive_latest,
        archive_visit_count,
    }
}

/// Trims an RFC-3339 visit timestamp to its `YYYY-MM-DD` date prefix for the system-context span.
///
/// The archive stores `visit_time_iso` as RFC 3339 (e.g. `2026-05-02T00:00:00.000Z`); the model only
/// needs the date for "spans X to Y", so we keep the leading date and drop the time/zone. Robust to a
/// shorter-than-expected string (returns it unchanged) so a malformed bound never panics.
fn iso_date_only(iso: &str) -> String {
    iso.split('T').next().unwrap_or(iso).to_string()
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
/// 6. check the ceilings (runaway step backstop + live-context budget) AFTER threading results; when
///    one is reached, run ONE final tool-free synthesis turn so the evidence already gathered still
///    produces a real answer (never a silent bail), then finish — otherwise loop
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
    // Said once per run: some search behind this answer saw only part of the archive.
    let mut told_incomplete_index = false;

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

        // Account for usage + emit the Usage marker so the UI can show running cost. The live-context
        // gate reads `last_prompt_tokens` = THIS turn's prompt (the resent history), NOT the running
        // sum. When the provider reports no usage (some OpenAI-compatible servers omit it on streamed
        // completions), the gate would silently disable — so we fall back to a tokenizer-free size
        // estimate of the history. The estimate is coarse, so it is a FALLBACK only: accurate provider
        // usage is preferred whenever present.
        // The MOST RECENT turn's prompt size = the model's live context occupancy. Gated against
        // `token_budget` (NOT the cumulative `prompt_tokens`, which double-counts the resent history).
        // Bound fresh each turn (never carried across), so it is a per-iteration `let`.
        let last_prompt_tokens = match accumulated.usage {
            Some(usage) => {
                prompt_tokens += usage.prompt_tokens;
                completion_tokens += usage.completion_tokens;
                sink(AiChatStreamChunk::Usage {
                    prompt_tokens: usage.prompt_tokens,
                    completion_tokens: usage.completion_tokens,
                });
                usage.prompt_tokens
            }
            None => estimate_context_tokens(&request.messages),
        };

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

        // Record the assistant's own turn into history so the next turn has full context. When the
        // turn requested tool calls, the threaded-back assistant message MUST carry those calls (an
        // OpenAI-compatible transport correlates each following `tool` result to them by call id) —
        // otherwise the model never registers that it already called the tool and re-issues the same
        // call every turn (the loop bug this fixes). A text-only turn threads plain text.
        if !accumulated.tool_calls.is_empty() {
            let tool_calls = accumulated
                .tool_calls
                .iter()
                .map(|call| crate::ai::traits::LlmToolCall {
                    call_id: call.call_id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                })
                .collect();
            request
                .messages
                .push(LlmMessage::assistant_tool_calls(accumulated.text.clone(), tool_calls));
        } else if !accumulated.text.is_empty() {
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
                coverage_incomplete,
            } = execute_tool(registry, tool_context, call).await;
            if coverage_incomplete && !told_incomplete_index {
                told_incomplete_index = true;
                sink(AiChatStreamChunk::Note { code: AiAgentNote::SearchIndexIncomplete });
            }

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

        // Ceiling gates AFTER threading results back. Hitting a ceiling does NOT end the run
        // silently: it forces ONE final, tool-free synthesis turn so the evidence already gathered
        // produces a real answer (see `run_final_synthesis_turn`). The step cap is a runaway
        // backstop; the live-context budget — measured against THIS turn's prompt, not the
        // cumulative sum — is the real ceiling.
        let ceiling = if max_iterations != 0 && turn >= max_iterations {
            Some(AiAgentNote::MaxStepsReached)
        } else if token_budget != 0 && last_prompt_tokens >= token_budget {
            Some(AiAgentNote::TokenBudgetReached)
        } else {
            None
        };
        if let Some(note) = ceiling {
            return run_final_synthesis_turn(
                provider,
                request,
                control,
                journal,
                &mut sink,
                &citations,
                note,
                turn,
                &mut prompt_tokens,
                &mut completion_tokens,
            )
            .await;
        }
    }
}

/// Coarse, tokenizer-free estimate of how many tokens a message history occupies.
///
/// A FALLBACK for the live-context gate when the provider reports no streaming usage: without it the
/// gate would silently disable and only the step backstop would bound the run (risking a real context
/// overflow). Uses a ~4-bytes/token heuristic over message text + tool-call payloads — deliberately
/// approximate (it under/over-counts depending on language and JSON density), so accurate provider
/// usage is always preferred when present. It is NOT a substitute for the provider's true token count.
fn estimate_context_tokens(messages: &[LlmMessage]) -> u64 {
    let bytes: usize = messages
        .iter()
        .map(|message| {
            message.content.len()
                + message
                    .tool_calls
                    .iter()
                    .map(|call| call.name.len() + call.arguments.len())
                    .sum::<usize>()
        })
        .sum();
    (bytes / 4) as u64
}

/// MODEL-facing directive threaded into history for the forced final turn.
///
/// English on purpose: like the threaded `tool_result` messages + preamble, this STEERS the LLM and
/// is never shown to the user (its user-facing twin is the localized [`AiAgentNote`]). It forbids
/// further tool calls and demands an answer NOW from the evidence already gathered — the difference
/// between "the agent went silent after minutes of work" and "the agent wrapped up with what it
/// found".
const FINAL_SYNTHESIS_DIRECTIVE: &str = "You have reached this run's step/context budget, so tools \
are no longer available. Do not attempt any further tool calls. Answer the user's most recent \
question NOW, using only the evidence you have already gathered in this conversation. Be concrete \
and cite the specific records you found. If that evidence is not enough for a complete answer, give \
your best answer from what you have and state plainly what is still missing and how the user could \
narrow the request to retrieve the rest.";

/// Runs ONE final, tool-FREE synthesis turn after a ceiling was reached, so the run still produces a
/// real answer from the evidence already gathered instead of going silent (the old behavior streamed
/// only a control note + raw evidence and stopped, discarding the model's chance to actually answer).
///
/// Sequence: (1) stream the USER-facing localized [`AiAgentNote`] (why we are wrapping up); (2) thread
/// the MODEL-facing [`FINAL_SYNTHESIS_DIRECTIVE`] and STRIP the tool definitions so the model cannot
/// loop again; (3) stream the answer live; (4) journal the turn, then pin + emit the evidence.
///
/// Cancel emits `Done` (matching the rest of the harness); a stream error emits `Error`. The
/// tool-free turn adds no new citations, so it pins the set accumulated during the tool loop. The
/// journaled assistant-turn step carries `tool_call_id = None`, so `append_agent_step` appends it
/// (its idempotency dedup keys only on a present `tool_call_id`) — no collision with the loop's own
/// assistant-turn step at the same `turn`.
#[allow(clippy::too_many_arguments)]
async fn run_final_synthesis_turn<P, J, S>(
    provider: &P,
    mut request: LlmChatRequest,
    control: Option<&Arc<dyn AiRunControl>>,
    journal: &J,
    sink: &mut S,
    citations: &[AiCitation],
    note: AiAgentNote,
    turn: u32,
    prompt_tokens: &mut u64,
    completion_tokens: &mut u64,
) -> AgentRunOutcome
where
    P: LlmProvider,
    J: AgentJournal,
    S: AgentRunSink,
{
    // USER-facing localized note: why we are finishing up with what we have.
    sink(AiChatStreamChunk::Note { code: note });
    // MODEL-facing directive + strip tools so the final turn ANSWERS instead of calling more tools.
    request.messages.push(LlmMessage::new(LlmRole::User, FINAL_SYNTHESIS_DIRECTIVE.to_string()));
    request.tools = Vec::new();

    let accumulated = match stream_one_turn(provider, &request, control, sink).await {
        Ok(accumulated) => accumulated,
        Err(TurnError::Cancelled) => {
            sink(AiChatStreamChunk::Done);
            return AgentRunOutcome::Cancelled {
                iterations: turn,
                prompt_tokens: *prompt_tokens,
                completion_tokens: *completion_tokens,
            };
        }
        Err(TurnError::Stream(message)) => {
            sink(AiChatStreamChunk::Error { message: message.clone() });
            return AgentRunOutcome::Failed {
                iterations: turn,
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
    // The synthesis turn IS a model turn: journal it at `turn + 1` (a distinct ordinal from the
    // loop's own assistant-turn step for `turn`, so trace replay never sees two assistant turns at the
    // same turn) and count it in `iterations`. `final=true` marks it the post-ceiling synthesis.
    let synthesis_turn = turn + 1;
    let payload =
        json!({ "text": accumulated.text, "reasoning": accumulated.reasoning, "final": true })
            .to_string();
    let _ = journal.journal_step(synthesis_turn, STEP_KIND_ASSISTANT_TURN, None, None, &payload);
    let _ = journal.record_citations(citations);
    emit_citations(sink, citations);
    sink(AiChatStreamChunk::Done);
    // `iterations` counts model turns; the tokens above already include the synthesis turn, so it must
    // be counted here too (otherwise the cost shows N+1 turns' tokens against N iterations).
    AgentRunOutcome::Completed {
        iterations: synthesis_turn,
        prompt_tokens: *prompt_tokens,
        completion_tokens: *completion_tokens,
    }
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
    /// A search behind the result covered only part of the archive.
    coverage_incomplete: bool,
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
                coverage_incomplete: false,
            };
        }
    };
    match registry.dispatch(&call.name, args, context).await {
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
                "Tool `{}` failed: {error}. Try a different query or tool.",
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
    // USER-facing control note as a localized CODE, not raw English. The model is
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
