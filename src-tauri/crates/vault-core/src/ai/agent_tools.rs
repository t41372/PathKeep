//! PathKeep-owned agent tool registry (plain Rust, NOT rig's `Tool`).
//!
//! ## Responsibilities
//! - declare the `AgentTool` boundary trait the W-AI-7 harness dispatches against, plus the
//!   `ToolOutcome` it returns (model-facing text + structured citations) and the `ToolRegistry`
//!   (name → tool) that builds the `LlmChatRequest.tools` definitions
//! - own the single `search_history` retrieval tool that wraps the W-AI-5/6 hybrid pipeline
//!   (`search_history_internal`) with date-range support; the three former escalation planes
//!   (`search_bm25` / `search_vector` / `search_hybrid`) were collapsed into this one tool (W-PKG-A)
//! - resolve each citation's `canonical_url` (the W-STAR star key) so a cited page can be starred
//!
//! ## Not responsible for
//! - the agent loop, journaling, cancellation, or budget enforcement (that is `agent_harness.rs`)
//! - touching rig: tool DEFINITIONS are PathKeep `LlmToolDef`s and execution is plain Rust, so no
//!   vendor type crosses this boundary (02 §B "in-app tools = plain Rust trait")
//! - opening/owning provider runtimes or secrets (the worker resolves and passes those in)
//!
//! ## Why this module exists
//! 02 §F mandates an OWNED tool registry the hand-rolled loop dispatches against — rig's `Tool`
//! runtime gives us no cancellation/journaling/budget, so the harness must execute tools itself.
//! Keeping the tools here (over the verbatim `search_history_internal`) means the agent and the
//! plain assistant share one retrieval implementation with no second code path to drift.
//!
//! ## Performance notes
//! - every tool delegates to the bounded `search_history_internal` (RRF over the top-k pools, never
//!   the corpus); the harness caps tool iterations so the known O(n) `.pkmap` `is:starred` pass is
//!   never run in a tight loop

use super::AiProviderRuntime;
use super::AiRunControl;
use super::code_mode::run_code_in_sandbox;
// Re-export the code-mode transparency types so a `run_code` outcome (and everything threading it —
// the harness, the journal, the streamed chunk) names ONE owned shape, not a `code_mode::` path.
pub use super::code_mode::{HostCallRecord, LimitsHit};
use super::search::search_history_internal;
use super::traits::LlmToolDef;
use crate::config::ProjectPaths;
use crate::models::{AiCitation, AiSearchRequest, AppConfig};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

// Intelligence / stars / annotations imports for the W-AI-9 tools.
use crate::models::{
    CategoryFilteredDateRangeRequest, DateRange, DayInsightsRequest, DomainTrendRequest,
    PagedDateRangeRequest, ScopedDateRangeRequest, SearchTrailQueryRequest, StarSort,
    TopSitesRequest,
};

/// The result of executing one agent tool: text the model sees, plus structured citations.
///
/// `model_text` is the compact, row-id-bearing evidence threaded back into the conversation (never
/// a huge inlined result set — 02 §F bounded context); `citations` is the structured provenance the
/// harness journals + pins so an answer's evidence survives later compaction. Each citation carries
/// its `canonical_url` so WU-6 can star the cited page.
///
/// The three code-mode fields (`code_source`/`host_calls`/`limits_hit`) are ADDITIVE (W-AI-8 WU-2):
/// the search tools leave them at their defaults, while `run_code` populates them so the harness can
/// journal + stream the script verbatim, its transparent host-call timeline, and any hard limit hit
/// (02 §G: the user SEES exactly what ran and what it queried). A search-tool outcome serializes
/// identically to before because nothing here is wired — these fields are only ever read in Rust.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolOutcome {
    /// Compact, model-facing summary of the tool result (bounded; ids + count, not full rows).
    pub model_text: String,
    /// Structured citations resolved from the result, each keyed by canonical_url for W-STAR.
    pub citations: Vec<AiCitation>,
    /// The code-mode script verbatim (`Some` only for `run_code`; `None` for the search tools).
    pub code_source: Option<String>,
    /// The transparent host-call timeline a `run_code` script made (empty for the search tools).
    pub host_calls: Vec<HostCallRecord>,
    /// Which hard sandbox limit (if any) bounded a `run_code` script (`None` for the search tools).
    pub limits_hit: Option<LimitsHit>,
}

/// Read-only execution context shared by every retrieval tool.
///
/// Carries exactly what `search_history_internal` needs to rerun retrieval under the current unlock
/// state + scope defaults, without reaching back into higher-level orchestration. `embedding_provider`
/// is `None` when no embedding provider is configured — the lexical (`search_bm25`) plane still works
/// (honest degradation), while the semantic planes degrade to lexical-only with a note.
#[derive(Clone)]
pub struct AgentToolContext {
    pub paths: ProjectPaths,
    pub config: AppConfig,
    pub database_key: Option<String>,
    pub embedding_provider: Option<AiProviderRuntime>,
    pub default_profile_id: Option<String>,
    pub default_domain: Option<String>,
    pub default_limit: u32,
    /// The run's cooperative cancel hook, threaded into the `run_code` sandbox so a user cancel
    /// promptly traps the guest (the sandbox bumps the engine epoch on cancel). `None` outside an
    /// agent run (the harness tests, and any retrieval-only caller) — the search tools never read it.
    pub run_control: Option<Arc<dyn AiRunControl>>,
}

/// JSON arguments accepted by every retrieval tool (one shared shape across the planes).
#[derive(Debug, Default, Deserialize)]
struct SearchToolArgs {
    /// Optional: an empty/omitted query returns the most recent visits (browse-by-recency).
    #[serde(default)]
    query: String,
    profile_id: Option<String>,
    domain: Option<String>,
    limit: Option<u32>,
    /// Result ordering: `"relevance"` (default) | `"newest"` | `"oldest"`. `"oldest"` enumerates the
    /// keyword's matches earliest-first so the model can find the FIRST occurrence of a term across ALL
    /// history (the relevance default only ever returns the most-relevant sample, never the earliest).
    #[serde(default)]
    sort: Option<String>,
    /// Opaque pagination cursor from a prior response's `next_cursor`; fetches the NEXT page in the
    /// chosen sort order so the model can enumerate every match across time, not just the first page.
    #[serde(default)]
    cursor: Option<String>,
    /// The `is:starred` facet (W-AI-6); restricts recall to starred pages on both planes.
    #[serde(default)]
    starred_only: Option<bool>,
    /// Inclusive start date in `"YYYY-MM-DD"` format (W-PKG-A). Only visits on/after this day.
    #[serde(default)]
    start_date: Option<String>,
    /// Inclusive end date in `"YYYY-MM-DD"` format (W-PKG-A). Only visits on/before this day.
    #[serde(default)]
    end_date: Option<String>,
}

/// Which recall plane a retrieval tool drives, mapped onto `search_history_internal`.
///
/// The internal does lexical + (optional) semantic RRF in one place; the plane controls whether the
/// embedding provider is threaded, so the three escalation tools are honest about what they use
/// WITHOUT a second retrieval implementation:
/// - `Bm25` never threads a provider, so it is pure lexical recall and works with NO embedding
///   provider configured (the honest floor escalation start).
/// - `Vector` / `Hybrid` thread the provider so semantic recall participates; with no provider they
///   degrade to lexical-only with the internal's honest note (never an error).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Bm25/Vector retained for the internal code_mode plane; only Hybrid is exposed as a tool.
enum SearchPlane {
    /// Lexical BM25/trigram recall only (no embedding provider used).
    Bm25,
    /// Semantic recall (embedding provider threaded; fused with lexical by the internal).
    Vector,
    /// Full hybrid: lexical + semantic RRF (the default `search_history` behaviour).
    Hybrid,
}

impl SearchPlane {
    /// Returns the embedding provider to thread for this plane, given the context's provider.
    ///
    /// `Bm25` deliberately drops the provider (lexical only); the semantic planes pass it through.
    fn provider_for(self, context: &AgentToolContext) -> Option<&AiProviderRuntime> {
        match self {
            SearchPlane::Bm25 => None,
            SearchPlane::Vector | SearchPlane::Hybrid => context.embedding_provider.as_ref(),
        }
    }
}

/// The boundary trait every agent tool implements (plain Rust — no rig `Tool`).
///
/// `definition` is the PathKeep `LlmToolDef` advertised to the model; `call` executes the tool with
/// the model-supplied JSON args and returns a [`ToolOutcome`]. `call` is async (retrieval awaits the
/// embedding/index path) and returns `Result` so the harness can journal a failure as an error
/// ToolResult and thread an honest message back WITHOUT aborting the whole run.
pub trait AgentTool: Send + Sync {
    /// The tool name the model invokes (stable; also the registry key).
    fn name(&self) -> &str;

    /// The JSON-schema tool definition advertised to the model this turn.
    fn definition(&self) -> LlmToolDef;

    /// Executes the tool with the model-supplied JSON arguments.
    fn call(
        &self,
        args: Value,
        context: &AgentToolContext,
    ) -> impl std::future::Future<Output = Result<ToolOutcome>> + Send;
}

/// A retrieval tool over `search_history_internal` for one recall plane.
///
/// One struct backs all four tools — only the name, description, and plane differ — so the verbatim
/// reuse of the W-AI-5/6 pipeline is literal (no per-tool retrieval code to drift).
pub struct HistorySearchTool {
    name: &'static str,
    description: &'static str,
    plane: SearchPlane,
}

impl HistorySearchTool {
    /// The single `search_history` tool: hybrid lexical + semantic RRF with date-range support
    /// (W-PKG-A). Auto-degrades to lexical when no embedding provider is configured.
    pub fn search_history() -> Self {
        Self {
            name: "search_history",
            description: "Search browser history by meaning, URL, title, profile, domain, or date range (hybrid lexical + semantic). Returns the best matching visits with their ids. The `query` is OPTIONAL: call it with an empty or omitted query to list the MOST RECENT visits (browse by recency). Use `start_date` and `end_date` (YYYY-MM-DD) to narrow results to a date range — for example \"last Friday\" or \"this week\". Use `sort` to control ordering: 'oldest' finds the FIRST/earliest occurrence of a term across ALL history (essential for \"when did I first browse X?\" — the default relevance ranking only returns the most-relevant sample, never the earliest); 'newest' the latest; the default ranks by relevance. The response includes `applied_limit` (the actual limit used after clamping), `has_more` (whether more rows exist beyond the limit), and `next_cursor` — pass `next_cursor` back as `cursor` to page through the matches in the chosen sort order. If `has_more` is true but NO `next_cursor` is returned, you have reached the retrieval cap (the deepest page is retrievable for this query) — narrow the date range or add filters to see more, do NOT keep paging. When no embedding provider is configured, results use lexical retrieval only (an honest degradation, not an error).",
            plane: SearchPlane::Hybrid,
        }
    }

    /// Shared JSON-schema parameter object for the search tool.
    fn parameters() -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "What to search for. OPTIONAL: an empty or omitted query returns the most recent visits (browse by recency), so you can enumerate recent history or find the date range." },
                "profile_id": { "type": "string", "description": "Optional browser profile identifier filter." },
                "domain": { "type": "string", "description": "Optional domain filter." },
                "limit": { "type": "integer", "description": "Maximum number of visits to return. The actual limit used (after clamping to [1, 50]) is reported as applied_limit in the response." },
                "sort": { "type": "string", "enum": ["relevance", "newest", "oldest"], "description": "Result ordering. 'relevance' (default) ranks by best match. 'oldest' enumerates the matches earliest-first — use it to find the FIRST/earliest time a term appears across ALL history (the relevance default only returns the most-relevant sample, NOT the earliest, so it cannot answer 'when did I first browse X?'). 'newest' enumerates latest-first." },
                "cursor": { "type": "string", "description": "Pagination cursor from a prior response's next_cursor. Pass it to fetch the NEXT page in the same sort order so you can enumerate every match across time, not just the first page." },
                "starred_only": { "type": "boolean", "description": "Restrict to starred (favorited) pages only." },
                "start_date": { "type": "string", "description": "Inclusive start date filter in YYYY-MM-DD format. Only visits on or after this day are returned." },
                "end_date": { "type": "string", "description": "Inclusive end date filter in YYYY-MM-DD format. Only visits on or before this day are returned." }
            }
        })
    }
}

impl AgentTool for HistorySearchTool {
    fn name(&self) -> &str {
        self.name
    }

    fn definition(&self) -> LlmToolDef {
        LlmToolDef {
            name: self.name.to_string(),
            description: self.description.to_string(),
            parameters: Self::parameters(),
        }
    }

    async fn call(&self, args: Value, context: &AgentToolContext) -> Result<ToolOutcome> {
        let parsed: SearchToolArgs = serde_json::from_value(args)
            .map_err(|error| anyhow::anyhow!("invalid search arguments: {error}"))?;
        // An empty/omitted `query` is allowed: it returns the most recent visits (browse-by-recency),
        // so the model can ENUMERATE history / find the date range rather than only keyword-search.
        let request = AiSearchRequest {
            query: parsed.query,
            profile_id: parsed.profile_id.or_else(|| context.default_profile_id.clone()),
            domain: parsed.domain.or_else(|| context.default_domain.clone()),
            limit: parsed.limit.or(Some(context.default_limit)),
            cursor: parsed.cursor,
            sort: parsed.sort,
            starred_only: parsed.starred_only,
            start_date: parsed.start_date,
            end_date: parsed.end_date,
        };
        let provider = self.plane.provider_for(context);
        let response = search_history_internal(
            &context.paths,
            &context.config,
            context.database_key.as_deref(),
            provider,
            &request,
        )
        .await?;

        let citations = response
            .items
            .iter()
            .map(|item| AiCitation {
                history_id: item.history_id,
                profile_id: item.profile_id.clone(),
                url: item.url.clone(),
                title: item.title.clone(),
                visited_at: item.visited_at.clone(),
                score: Some(item.score),
                // Resolve the W-STAR star key here (the same canonicalization stars/refind use), so a
                // streamed `Citations` chunk can render a starrable evidence row without the FE — or
                // the worker journal — re-normalizing the raw url.
                canonical_url: crate::visit_taxonomy::normalize_visit_url(&item.url)
                    .map(|normalized| normalized.canonical_url),
            })
            .collect::<Vec<_>>();
        let model_text = summarize_search_for_model(self.name, &response);
        // The search tools carry no code-mode fields (those default); only `run_code` populates them.
        Ok(ToolOutcome { model_text, citations, ..ToolOutcome::default() })
    }
}

/// Builds the compact, bounded, model-facing summary of one search result.
///
/// 02 §F bounded-context rule: thread evidence by row id + count + a short line per hit, NEVER the
/// full result set. Each line is `[id] visited | url — title (reason, score)` so the model can cite
/// a real history row id; any degradation notes are appended so the model knows recall was limited.
fn summarize_search_for_model(
    tool_name: &str,
    response: &crate::models::AiSearchResponse,
) -> String {
    if response.items.is_empty() {
        let limit_info =
            response.applied_limit.map(|l| format!(" (applied_limit: {l})")).unwrap_or_default();
        let mut text = format!("{tool_name}: no matching history rows{limit_info}.");
        for note in &response.notes {
            text.push_str("\nNote: ");
            text.push_str(note);
        }
        return text;
    }
    let limit_info =
        response.applied_limit.map(|l| format!(", applied_limit: {l}")).unwrap_or_default();
    let more_info = if response.has_more { ", has_more: true" } else { "" };
    // Surface the pagination cursor so the model knows the next page exists AND the exact token to pass
    // back as `cursor` to fetch it — without this it can see `has_more` but has no handle to continue.
    let cursor_info = response
        .next_cursor
        .as_deref()
        .map(|cursor| format!(", next_cursor: {cursor}"))
        .unwrap_or_default();
    let mut text = format!(
        "{tool_name}: {} match(es) (provider: {}{limit_info}{more_info}{cursor_info}).",
        response.items.len(),
        response.provider_id
    );
    for item in &response.items {
        text.push_str(&format!(
            "\n[{id}] {visited} | {url} — {title} ({reason}, {score:.3})",
            id = item.history_id,
            visited = item.visited_at,
            url = item.url,
            title = item.title.clone().unwrap_or_else(|| "(untitled)".to_string()),
            reason = item.match_reason,
            score = item.score,
        ));
    }
    for note in &response.notes {
        text.push_str("\nNote: ");
        text.push_str(note);
    }
    text
}

/// The honest, host-API-teaching description for the `run_code` tool (W-AI-8 WU-2).
///
/// Kept as a const so the registry-order test can reference the same text the model sees. It tells
/// the model exactly what the sandbox grants (read-only history retrieval) and DENIES (network, fs,
/// clock, randomness), the two available globals + their argument shapes (including the `notes`
/// degradation signal each call returns), and that it must ALWAYS `return` a small distilled value
/// (even a negative result like `{ found: false }`) — the result is bounded host-side. An empty
/// return is honestly reported as "no value", so there is no reason to return nothing. No promise the
/// sandbox cannot keep.
const RUN_CODE_DESCRIPTION: &str = "Run a short JavaScript program in a locked-down sandbox over the user's browser history when a question needs computation/aggregation across many visits (counts, grouping, joins, dedup) that a single search cannot express. The program is READ-ONLY: there is NO network, NO filesystem, NO real clock (Date.now() is 0), and NO randomness; it is bounded by hard time/memory/host-call/output limits, so loop and aggregate freely. Two globals are available, both synchronous and returning { rows: [...], notes: [...], hasMore: boolean, nextCursor: string|null }: query_history({ query?: string, plane?: \"hybrid\"|\"vector\"|\"bm25\", sort?: \"relevance\"|\"newest\"|\"oldest\", limit?: number, cursor?: string, profileId?: string, domain?: string, starredOnly?: boolean, startDate?: string, endDate?: string }) runs the same hybrid/lexical/semantic retrieval as the search tools (each row has id, url, title, domain, visitedAt, score, matchReason, canonicalUrl); `query` is OPTIONAL — call query_history({}) or query_history({ query: \"\" }) to enumerate the MOST RECENT visits (browse by recency) when you need to list recent history or find the date range for a date/recency question; `sort` controls ordering — \"oldest\" enumerates the matches EARLIEST-first so you can find the FIRST time a term appears across ALL history (the default \"relevance\" only returns the most-relevant sample, NEVER the earliest, so it cannot answer \"when did I first browse X?\"), \"newest\" is latest-first; to page through the matches pass the reply's `nextCursor` back as `cursor` (when `hasMore` is true) and keep the same `sort`; if `hasMore` is true but `nextCursor` is null you have reached the retrieval cap (the deepest retrievable page for this query) — narrow `startDate`/`endDate` or add filters to see more rather than paging further; `startDate` and `endDate` are optional YYYY-MM-DD strings that restrict results to visits within that date range (inclusive); fetch_visits(ids: number[]) resolves specific visit ids to rows. The `notes` array on each result holds honest warnings about retrieval scope or degradation (for example, semantic/hybrid search fell back to keyword-only because no embedding provider is configured) — read them and reflect any limitation in your answer; do not ignore them. Always end the program with `return <value>` to hand back a SMALL distilled result (a summary object/array, not the raw rows) — that returned value is the ONLY thing you will see, and it is truncated if too large. Even when there is no answer, return a small explicit result such as { found: false } rather than returning nothing. Example: const result = query_history({ query: \"\", startDate: \"2026-06-19\", endDate: \"2026-06-19\" }); const domains = {}; result.rows.forEach(r => { domains[r.domain] = (domains[r.domain]||0)+1; }); return { date: \"2026-06-19\", domainCounts: domains, total: result.rows.length }; Example: const result = query_history({ query: \"machine learning\" }); if (result.rows.length === 0) return { found: false }; return { count: result.rows.length, topDomains: [...new Set(result.rows.map(r=>r.domain))].slice(0,5) };";

/// The `run_code` agent tool (W-AI-8 WU-2): wraps the WU-1 sandbox as a registered [`AgentTool`].
///
/// Default-enabled on every agent run (2026-06 user decision: the Wasmtime sandbox IS the safety
/// boundary, so there is NO model-capability gating — the LLM is swappable and the sandbox bounds
/// whatever it writes). `call` bridges the async harness to the SYNC CPU-bound wasmtime run on a
/// blocking thread (see below), so the agent loop stays responsive to cancel while a script runs.
pub struct RunCodeTool;

/// JSON arguments for `run_code`: the JavaScript program to run (the only field).
#[derive(Debug, Default, Deserialize)]
struct RunCodeArgs {
    /// The JS program. Must be a non-empty string; `return <value>` distills the result.
    source: String,
}

impl RunCodeTool {
    /// The tool name the model invokes + the registry key.
    pub fn new() -> Self {
        Self
    }

    /// The single-required-`source` JSON schema advertised to the model.
    fn parameters() -> Value {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "The JavaScript program to run in the sandbox. End with `return <value>` to distill a small result."
                }
            },
            "required": ["source"]
        })
    }
}

impl Default for RunCodeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentTool for RunCodeTool {
    fn name(&self) -> &str {
        "run_code"
    }

    fn definition(&self) -> LlmToolDef {
        LlmToolDef {
            name: "run_code".to_string(),
            description: RUN_CODE_DESCRIPTION.to_string(),
            parameters: Self::parameters(),
        }
    }

    async fn call(&self, args: Value, context: &AgentToolContext) -> Result<ToolOutcome> {
        let parsed: RunCodeArgs = serde_json::from_value(args)
            .map_err(|error| anyhow::anyhow!("invalid run_code arguments: {error}"))?;
        let source = parsed.source.trim().to_string();
        if source.is_empty() {
            anyhow::bail!("the `source` argument must not be empty");
        }
        let context = context.clone();
        let control = context.run_control.clone();
        // The wasmtime run is SYNC + CPU-bound (it does `block_in_place` + `block_on` internally for
        // its retrieval host calls). Calling it directly in this async fn would fight the tokio worker
        // it runs on, so hand it to a blocking thread: `spawn_blocking` gives it a dedicated thread
        // and `Handle::current()` is the multi-thread runtime handle the host fn drives retrieval on.
        // The agent loop keeps polling while the script runs, so a cancel (which bumps the engine
        // epoch from the ticker thread) traps the guest promptly instead of deadlocking the loop.
        let handle = tokio::runtime::Handle::current();
        let outcome = tokio::task::spawn_blocking(move || {
            run_code_in_sandbox(&source, &context, handle, control)
        })
        .await?;
        code_outcome_to_tool_outcome(outcome)
    }
}

/// Maps a [`CodeOutcome`] onto a [`ToolOutcome`] (or an `Err` the harness threads as a recoverable
/// `is_error` tool result), preserving the WU-1 "never panics, always honest" contract.
///
/// - A HARD failure (the sandbox reported an error AND produced no distilled output) is an `Err`, so
///   the harness threads an honest `is_error` message back (same discipline as the search tools — one
///   failure never aborts the run; the model can retry with different code).
/// - Otherwise `Ok`, carrying the distilled text + citations + the script source + the transparent
///   host-call timeline. When the script returned nothing (no `return`, or only whitespace) the empty
///   text is normalized to an honest sentinel so the model gets a clear "nothing was returned" signal
///   instead of an ambiguous empty string. When a hard limit bounded the run, a short honest note is
///   appended to the model_text so the model knows the result was truncated (never silently fabricated).
fn code_outcome_to_tool_outcome(outcome: super::code_mode::CodeOutcome) -> Result<ToolOutcome> {
    let super::code_mode::CodeOutcome {
        mut model_text,
        citations,
        host_calls,
        source,
        error,
        limits_hit,
    } = outcome;

    if let Some(message) = &error {
        if model_text.is_empty() {
            anyhow::bail!("run_code failed: {message}");
        }
    }
    // No hard error, but the script ran without `return`ing a value (or returned only whitespace).
    // An empty result is silently ambiguous — the model cannot tell "ran but found nothing" from
    // "ran and said nothing", so it may hallucinate or retry. Normalize to an HONEST sentinel (NOT a
    // fabricated answer): plainly state nothing was returned. This is not an error (`is_error` stays
    // false), so the limit note below can still append if a bounded run also returned nothing.
    if model_text.trim().is_empty() {
        model_text = "[run_code completed but returned no value]".to_string();
    }
    // The run produced output but was bounded by a hard limit — tell the model so it does not treat a
    // truncated/partial result as complete (02 §F bounded-context honesty).
    if let Some(limit) = limits_hit {
        model_text.push_str(&format!("\n\n[note: the script was bounded by a hard limit ({}); the result above may be partial]", limits_hit_label(limit)));
    }

    Ok(ToolOutcome { model_text, citations, code_source: Some(source), host_calls, limits_hit })
}

/// Returns the byte length of the longest prefix of `value` that is at most `max_bytes` long AND ends
/// on a UTF-8 char boundary (H2 — never split a multibyte CJK/emoji codepoint).
///
/// `&value[..n]` panics when byte `n` lands inside a multibyte char, which is a HARD crash on the
/// project's mandatory i18n (CJK) content. This walks back from `max_bytes` to the nearest char
/// boundary (`is_char_boundary` is O(1), and a UTF-8 char is at most 4 bytes, so this is at most a
/// 3-step walk), then returns a slice that is always valid. A prefix that already ends on a boundary
/// is returned unchanged; `max_bytes >= value.len()` returns the whole string.
fn truncate_str_on_char_boundary(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut cut = max_bytes;
    while cut > 0 && !value.is_char_boundary(cut) {
        cut -= 1;
    }
    &value[..cut]
}

/// A short, human-readable label for a hard sandbox limit (for the model-facing bounded-run note).
fn limits_hit_label(limit: LimitsHit) -> &'static str {
    match limit {
        LimitsHit::Time => "wall-time limit",
        LimitsHit::Memory => "memory limit",
        LimitsHit::HostCalls => "host-call budget",
        LimitsHit::Output => "output size cap",
        LimitsHit::Cancelled => "cancelled",
    }
}

// ---------------------------------------------------------------------------
// W-AI-9 Work Package B: intelligence_report, list_stars, list_annotations
// ---------------------------------------------------------------------------

/// JSON arguments for `intelligence_report`.
#[derive(Debug, Default, Deserialize)]
struct IntelligenceReportArgs {
    report: String,
    start_date: Option<String>,
    end_date: Option<String>,
    domain: Option<String>,
    limit: Option<u32>,
}

/// The report variants `intelligence_report` dispatches to.
///
/// Parsing the model-supplied `report` string into this enum ONCE ([`IntelReport::parse`]) gives a
/// single place an unknown name is rejected, and lets the required-arg and dispatch matches below be
/// exhaustive (compiler-checked) — no unreachable catch-all arm to drift out of sync or go untested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntelReport {
    TopSites,
    Sessions,
    SearchTrails,
    ActivityMix,
    BrowsingRhythm,
    DomainTrend,
    DayInsights,
    Overview,
}

impl IntelReport {
    /// Parses the model-supplied report name. An unknown name is an input error rejected here — the
    /// SINGLE validation point, run before the readiness gate so the model gets a clear signal even
    /// when the plane is also unbuilt.
    fn parse(name: &str) -> Result<Self> {
        Ok(match name {
            "top_sites" => Self::TopSites,
            "sessions" => Self::Sessions,
            "search_trails" => Self::SearchTrails,
            "activity_mix" => Self::ActivityMix,
            "browsing_rhythm" => Self::BrowsingRhythm,
            "domain_trend" => Self::DomainTrend,
            "day_insights" => Self::DayInsights,
            "overview" => Self::Overview,
            other => anyhow::bail!(
                "unknown report variant `{other}`; expected one of: top_sites, sessions, search_trails, activity_mix, browsing_rhythm, domain_trend, day_insights, overview"
            ),
        })
    }
}

/// Pre-computed intelligence report tool (W-AI-9 WPB-1).
///
/// Dispatches to deterministic Core Intelligence read models via a `report` enum parameter. Each
/// variant maps to one intelligence query (sessions, top sites, search trails, activity mix,
/// browsing rhythm, domain trend, day insights, overview). Faster and more complete than running
/// code over raw search results because it reads from the materialized intelligence plane.
pub struct IntelligenceReportTool;

impl IntelligenceReportTool {
    fn parameters() -> Value {
        json!({
            "type": "object",
            "properties": {
                "report": {
                    "type": "string",
                    "enum": ["top_sites", "sessions", "search_trails", "activity_mix",
                             "browsing_rhythm", "domain_trend", "day_insights", "overview"],
                    "description": "Which pre-computed report to retrieve."
                },
                "start_date": { "type": "string", "description": "Start of date range (YYYY-MM-DD). Omit for all-time." },
                "end_date": { "type": "string", "description": "End of date range (YYYY-MM-DD). Omit for all-time." },
                "domain": { "type": "string", "description": "Optional domain filter (e.g. 'youtube.com')." },
                "limit": { "type": "integer", "description": "Maximum items to return." }
            },
            "required": ["report"]
        })
    }
}

const INTELLIGENCE_REPORT_DESCRIPTION: &str = "Retrieve a pre-computed intelligence report about the user's browsing patterns. Reports are built from the full history archive and cover sessions, top sites, search trails, activity patterns, domain trends, and daily insights. Use this when the user asks about patterns, trends, summaries, or habits across time — it is faster and more complete than running code over raw search results.";

impl AgentTool for IntelligenceReportTool {
    fn name(&self) -> &str {
        "intelligence_report"
    }

    fn definition(&self) -> LlmToolDef {
        LlmToolDef {
            name: "intelligence_report".to_string(),
            description: INTELLIGENCE_REPORT_DESCRIPTION.to_string(),
            parameters: Self::parameters(),
        }
    }

    async fn call(&self, args: Value, context: &AgentToolContext) -> Result<ToolOutcome> {
        let parsed: IntelligenceReportArgs = serde_json::from_value(args)
            .map_err(|error| anyhow::anyhow!("invalid intelligence_report arguments: {error}"))?;
        if parsed.report.is_empty() {
            anyhow::bail!("the `report` argument must not be empty");
        }

        let paths = &context.paths;
        let config = &context.config;
        let key = context.database_key.as_deref();

        // B1 — resolve the date window with an ALL-TIME sentinel for omitted/blank dates.
        //
        // An OMITTED date used to become `""`, which broke every report: `NaiveDate::parse_from_str`
        // errors on `""` (sessions/search_trails/browsing_rhythm), `date_key <= ""` matches NO rows
        // (top_sites/activity_mix/domain_trend), and the overview needs the literal all-time sentinel
        // `"1900-01-01"` to load its all-time snapshot. So a blank/omitted `start_date` substitutes the
        // all-time sentinel and a blank/omitted `end_date` substitutes today (local), giving a true
        // "all of history up to now" window when the model omits the dates.
        let start = parsed
            .start_date
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| ALL_TIME_SCOPE_START.to_string());
        let end = parsed
            .end_date
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
        let date_range = DateRange { start, end };

        // L2 — parse + validate the report variant and its REQUIRED arg BEFORE the readiness gate, so
        // the model gets an actionable input error ("unknown report" / "domain required" /
        // "start_date required") rather than the generic "intelligence not built" note when it both
        // supplies bad input AND the plane is unbuilt. `parse` rejects an unknown variant up front
        // (an input error regardless of plane state); the match below adds the per-report arg checks.
        let report = IntelReport::parse(&parsed.report)?;
        match report {
            IntelReport::DomainTrend => {
                if parsed.domain.as_deref().unwrap_or("").trim().is_empty() {
                    anyhow::bail!("the `domain` argument is required for the domain_trend report");
                }
            }
            IntelReport::DayInsights => {
                if parsed.start_date.as_deref().unwrap_or("").trim().is_empty() {
                    anyhow::bail!(
                        "the `start_date` argument is required for the day_insights report (use it as the target date)"
                    );
                }
            }
            _ => {}
        }

        // M3 — readiness handling. The single `intelligence_status().ready` signal (sessions ||
        // search_trails || refind_pages) does NOT match what top_sites/activity_mix/domain_trend read
        // (the daily-rollup tables), so a partial build could misreport per report. The cleanest honest
        // approach is to LET EACH REPORT RUN: the read models query their own tables (created by
        // `ensure_core_intelligence_schema`), returning an empty-but-valid result when that report's
        // specific data is absent — never a "no such table" error. We keep the `ready` signal only as a
        // FAST PATH: when the whole plane is empty we short-circuit to one honest "not built" note
        // instead of N empty results, but we never use it to falsely block a report whose data exists.
        let ready = crate::intelligence::intelligence_status(paths, config, key)
            .map(|status| status.ready)
            .unwrap_or(false);
        if !ready {
            return Ok(intelligence_not_ready_outcome(&parsed.report));
        }

        // L1 — clamp a model-supplied limit to at least 1: `limit: 0` would become `page_size: 0` →
        // `LIMIT 0` → zero rows for sessions/search_trails even on a built plane. Keep the per-report
        // upper defaults (`get_top_sites`/`get_domain_trend` clamp their own caps internally).
        let paged_limit = parsed.limit.map(|value| value.max(1)).unwrap_or(20);

        // Exhaustive over `IntelReport` — the compiler guarantees every variant is handled, so there
        // is no catch-all arm (the unknown case was already rejected by `IntelReport::parse` above).
        let result_json: Value = match report {
            IntelReport::TopSites => {
                let request = TopSitesRequest {
                    date_range,
                    profile_id: None,
                    sort_by: Some("visit_count".to_string()),
                    limit: parsed.limit.map(|value| value.max(1)).or(Some(20)),
                };
                let result = crate::intelligence::get_top_sites(paths, config, key, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::Sessions => {
                let request = PagedDateRangeRequest {
                    date_range,
                    profile_id: None,
                    page: 0,
                    page_size: paged_limit,
                };
                let result = crate::intelligence::get_sessions(paths, config, key, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::SearchTrails => {
                let request = SearchTrailQueryRequest {
                    date_range,
                    profile_id: None,
                    engine: None,
                    page: 0,
                    page_size: paged_limit,
                };
                let result = crate::intelligence::get_search_trails(paths, config, key, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::ActivityMix => {
                let request = ScopedDateRangeRequest { date_range, profile_id: None };
                let result = crate::intelligence::get_activity_mix(paths, config, key, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::BrowsingRhythm => {
                let request = CategoryFilteredDateRangeRequest {
                    date_range,
                    profile_id: None,
                    category: None,
                };
                let result =
                    crate::intelligence::get_browsing_rhythm(paths, config, key, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::DomainTrend => {
                // The `domain` presence was validated above the readiness gate (L2).
                let domain = parsed.domain.clone().unwrap_or_default();
                let request = DomainTrendRequest { registrable_domain: domain, date_range };
                let result = crate::intelligence::get_domain_trend(paths, config, key, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::DayInsights => {
                // `start_date` presence was validated above (L2); use the RAW supplied date as the
                // target day (not the all-time sentinel substitution, which is only for ranges).
                let date = parsed.start_date.clone().unwrap_or_default();
                let request = DayInsightsRequest { date, profile_id: None };
                let result = crate::intelligence::get_day_insights(paths, config, key, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::Overview => {
                let request = ScopedDateRangeRequest { date_range, profile_id: None };
                let result = crate::intelligence::get_intelligence_primary_overview(
                    paths, config, key, &request,
                )?;
                serde_json::to_value(result)?
            }
        };

        // Build a compact model-facing summary. We serialize the JSON but truncate to avoid
        // blowing up the model context with the full payload. H2 — truncate on a CHAR boundary so a
        // multibyte (CJK / emoji) codepoint straddling the cut never panics on the project's mandatory
        // i18n content.
        let raw = serde_json::to_string_pretty(&result_json).unwrap_or_default();
        let truncated = if raw.len() > INTELLIGENCE_REPORT_MAX_BYTES {
            format!(
                "{}...\n[truncated; {} bytes total]",
                truncate_str_on_char_boundary(&raw, INTELLIGENCE_REPORT_MAX_BYTES),
                raw.len()
            )
        } else {
            raw
        };
        let model_text =
            format!("intelligence_report({}): result follows.\n{}", parsed.report, truncated);
        Ok(ToolOutcome { model_text, ..ToolOutcome::default() })
    }
}

/// The all-time sentinel start date for an OMITTED intelligence `start_date` (B1).
///
/// Mirrors the authoritative `crate::intelligence::intelligence_overview_snapshot::ALL_TIME_SCOPE_START`
/// (`"1900-01-01"`), which is `pub(crate)` inside a PRIVATE module so it cannot be named here. The
/// overview snapshot loader recognizes EXACTLY this literal as "load the all-time snapshot", and the
/// rollup reports compare `date_key >= start`, so any date at/before the earliest history works for them
/// while the overview specifically needs this exact value. Kept in sync by the
/// `all_time_sentinel_matches_intelligence_source_of_truth`-style coverage on the snapshot module.
const ALL_TIME_SCOPE_START: &str = "1900-01-01";

/// Byte budget for the model-facing intelligence_report JSON before truncation (02 §F bounded context).
const INTELLIGENCE_REPORT_MAX_BYTES: usize = 8_000;

/// Returns an honest "intelligence not ready" outcome for a given report variant.
fn intelligence_not_ready_outcome(report: &str) -> ToolOutcome {
    ToolOutcome {
        model_text: format!(
            "intelligence_report({report}): the intelligence index has not been built yet for this archive. \
             The user needs to run a Core Intelligence rebuild before this report is available. \
             You can still use search_history or run_code to answer questions from raw history data."
        ),
        ..ToolOutcome::default()
    }
}

// ---------------------------------------------------------------------------
// Tool 2: list_stars
// ---------------------------------------------------------------------------

/// JSON arguments for `list_stars`.
#[derive(Debug, Default, Deserialize)]
struct ListStarsArgs {
    sort: Option<String>,
    limit: Option<usize>,
}

/// Lists the user's starred (favorited) pages (W-AI-9 WPB-2).
pub struct ListStarsTool;

const LIST_STARS_DESCRIPTION: &str = "List the user's starred (favorited) pages. Returns entity keys, domains, titles, visit counts, and when each was starred. Use this to answer questions about the user's favorites or bookmarks.";

impl AgentTool for ListStarsTool {
    fn name(&self) -> &str {
        "list_stars"
    }

    fn definition(&self) -> LlmToolDef {
        LlmToolDef {
            name: "list_stars".to_string(),
            description: LIST_STARS_DESCRIPTION.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sort": {
                        "type": "string",
                        "enum": ["recent", "most_revisited"],
                        "description": "Sort order."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum items to return (default 50, max 500)."
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Value, context: &AgentToolContext) -> Result<ToolOutcome> {
        let parsed: ListStarsArgs = serde_json::from_value(args)
            .map_err(|error| anyhow::anyhow!("invalid list_stars arguments: {error}"))?;
        let sort = match parsed.sort.as_deref() {
            Some("most_revisited") => StarSort::MostRevisited,
            _ => StarSort::RecentlyStarred,
        };
        let limit = parsed.limit.unwrap_or(50).clamp(1, 500);
        let items = crate::stars::list_stars(
            &context.paths,
            &context.config,
            context.database_key.as_deref(),
            None,
            sort,
            Some(limit),
        )?;
        if items.is_empty() {
            return Ok(ToolOutcome {
                model_text: "list_stars: no starred pages found.".to_string(),
                ..ToolOutcome::default()
            });
        }
        let mut text = format!("list_stars: {} starred item(s).", items.len());
        for item in &items {
            text.push_str(&format!(
                "\n- {key} | {domain} — {title} ({visits}x, starred {at})",
                key = item.entity_key,
                domain = item.domain,
                title = if item.title.is_empty() { "(untitled)" } else { &item.title },
                visits = item.visit_count,
                at = item.starred_at,
            ));
        }
        Ok(ToolOutcome { model_text: text, ..ToolOutcome::default() })
    }
}

// ---------------------------------------------------------------------------
// Tool 3: list_annotations
// ---------------------------------------------------------------------------

/// JSON arguments for `list_annotations`.
#[derive(Debug, Default, Deserialize)]
struct ListAnnotationsArgs {
    query: Option<String>,
    limit: Option<usize>,
}

/// Byte budget for one annotation's note preview in the model-facing list before truncation.
const ANNOTATION_NOTE_PREVIEW_MAX_BYTES: usize = 120;

/// Lists or searches the user's per-URL annotations (W-AI-9 WPB-3).
pub struct ListAnnotationsTool;

const LIST_ANNOTATIONS_DESCRIPTION: &str = "List or search the user's annotations (notes and tags on URLs). Use this to find pages the user has tagged or noted.";

impl AgentTool for ListAnnotationsTool {
    fn name(&self) -> &str {
        "list_annotations"
    }

    fn definition(&self) -> LlmToolDef {
        LlmToolDef {
            name: "list_annotations".to_string(),
            description: LIST_ANNOTATIONS_DESCRIPTION.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Optional text to search within notes."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum items to return (default 50, max 500)."
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Value, context: &AgentToolContext) -> Result<ToolOutcome> {
        let parsed: ListAnnotationsArgs = serde_json::from_value(args)
            .map_err(|error| anyhow::anyhow!("invalid list_annotations arguments: {error}"))?;
        let limit = Some(parsed.limit.unwrap_or(50).clamp(1, 500));
        let paths = &context.paths;
        let config = &context.config;
        let key = context.database_key.as_deref();
        let query_str = parsed.query.as_deref().unwrap_or("").trim();
        let items = if query_str.is_empty() {
            crate::annotations::list_annotations(paths, config, key, limit)?
        } else {
            crate::annotations::search_annotations(paths, config, key, query_str, limit)?
        };
        if items.is_empty() {
            let qualifier = if query_str.is_empty() {
                String::new()
            } else {
                format!(" matching \"{query_str}\"")
            };
            return Ok(ToolOutcome {
                model_text: format!("list_annotations: no annotations found{qualifier}."),
                ..ToolOutcome::default()
            });
        }
        let mut text = format!("list_annotations: {} annotation(s).", items.len());
        for item in &items {
            let tags_str = if item.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", item.tags.join(", "))
            };
            // H2 — slice the note preview on a CHAR boundary, never a raw byte index: a CJK note whose
            // 120th byte lands mid-codepoint would otherwise panic (a hard crash on i18n content).
            let notes_preview = if item.notes.len() > ANNOTATION_NOTE_PREVIEW_MAX_BYTES {
                format!(
                    "{}...",
                    truncate_str_on_char_boundary(&item.notes, ANNOTATION_NOTE_PREVIEW_MAX_BYTES)
                )
            } else {
                item.notes.clone()
            };
            text.push_str(&format!(
                "\n- {url}{tags} — {notes} (updated {at})",
                url = item.url,
                tags = tags_str,
                notes = notes_preview,
                at = item.updated_at,
            ));
        }
        Ok(ToolOutcome { model_text: text, ..ToolOutcome::default() })
    }
}

/// A name → tool table the harness dispatches against; also builds the request's tool definitions.
///
/// `BTreeMap` keeps `definitions()` deterministically ordered (stable prompt across runs). The
/// registry owns boxed `dyn AgentTool`s because the harness dispatches dynamically by name; `call`
/// stays a real async fn on each concrete tool (the trait's RPITIT), boxed only at the registry edge
/// via a small dispatch wrapper.
#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Box<dyn DynAgentTool>>,
}

impl ToolRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self { tools: BTreeMap::new() }
    }

    /// Registers one tool, keyed by its name (a later registration with the same name replaces it).
    pub fn register<T: AgentTool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Box::new(DynTool(tool)));
    }

    /// Builds the full default registry: `search_history` (with date-range, W-PKG-A) + `run_code`
    /// + the W-AI-9 WPB read tools (intelligence reports, stars, annotations).
    ///
    /// The single `search_history` auto-degrades to lexical when no embedding provider is
    /// configured — honest degradation, never an empty toolset.
    pub fn with_default_search_tools() -> Self {
        let mut registry = Self::new();
        registry.register(HistorySearchTool::search_history());
        // `run_code` is DEFAULT-ENABLED on every agent run (2026-06 decision): the Wasmtime sandbox
        // is the safety boundary, so there is no model-capability gate. The model gets it alongside
        // the search tools and chooses code-mode only when computation/aggregation needs it.
        registry.register(RunCodeTool::new());
        // W-AI-9 WPB tools: intelligence reports, stars, and annotations.
        registry.register(IntelligenceReportTool);
        registry.register(ListStarsTool);
        registry.register(ListAnnotationsTool);
        registry
    }

    /// Returns the tool definitions to advertise to the model this turn (deterministic order).
    pub fn definitions(&self) -> Vec<LlmToolDef> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    /// Whether the registry has any tools.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Dispatches one tool call by name, returning its outcome.
    ///
    /// An unknown name is an `Err` the harness journals + threads back as a recoverable tool error
    /// (the model can choose a different tool) rather than a panic.
    pub async fn dispatch(
        &self,
        name: &str,
        args: Value,
        context: &AgentToolContext,
    ) -> Result<ToolOutcome> {
        match self.tools.get(name) {
            Some(tool) => tool.call(args, context).await,
            None => anyhow::bail!("unknown tool `{name}`"),
        }
    }
}

/// Object-safe shim over [`AgentTool`] so the registry can hold heterogeneous boxed tools.
///
/// `AgentTool::call` is RPITIT (not dyn-compatible), so the registry stores this boxed variant whose
/// `call` returns a boxed future. The shim exists only to bridge the (monomorphized, inlinable)
/// trait to dynamic dispatch at the single registry lookup point.
trait DynAgentTool: Send + Sync {
    fn definition(&self) -> LlmToolDef;
    fn call<'a>(
        &'a self,
        args: Value,
        context: &'a AgentToolContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolOutcome>> + Send + 'a>>;
}

/// Wraps a concrete [`AgentTool`] as a [`DynAgentTool`].
struct DynTool<T: AgentTool>(T);

impl<T: AgentTool> DynAgentTool for DynTool<T> {
    fn definition(&self) -> LlmToolDef {
        self.0.definition()
    }

    fn call<'a>(
        &'a self,
        args: Value,
        context: &'a AgentToolContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolOutcome>> + Send + 'a>> {
        Box::pin(self.0.call(args, context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use crate::models::{AiProviderConfig, AiProviderPurpose, AiRequestFormat};
    use rusqlite::params;
    use secrecy::SecretString;

    fn embedding_runtime() -> AiProviderRuntime {
        AiProviderRuntime {
            config: AiProviderConfig {
                id: "embed-1".to_string(),
                name: "Embed".to_string(),
                purpose: AiProviderPurpose::Embedding,
                request_format: AiRequestFormat::OpenAi,
                enabled: true,
                default_model: "embed-model".to_string(),
                dimensions: Some(8),
                ..AiProviderConfig::default()
            },
            api_key: Some(SecretString::from("k".to_string())),
        }
    }

    fn context_with(embedding: Option<AiProviderRuntime>) -> (tempfile::TempDir, AgentToolContext) {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.assistant_enabled = true;
        let context = AgentToolContext {
            paths,
            config,
            database_key: None,
            embedding_provider: embedding,
            default_profile_id: None,
            default_domain: None,
            default_limit: 8,
            // The harness tests build a context with no run control; the search tools never read it
            // and `run_code` threads `None` into the sandbox (no cancel hook) as a safe default.
            run_control: None,
        };
        (dir, context)
    }

    #[test]
    fn registry_advertises_all_default_tools_in_stable_order() {
        let registry = ToolRegistry::with_default_search_tools();
        assert!(!registry.is_empty());
        let names: Vec<String> = registry.definitions().into_iter().map(|def| def.name).collect();
        // BTreeMap keeps the order deterministic (alphabetical), so the prompt is stable. W-PKG-A
        // collapsed the three escalation planes into one `search_history`; W-AI-9 WPB adds the
        // intelligence, stars, and annotations tools.
        assert_eq!(
            names,
            vec![
                "intelligence_report",
                "list_annotations",
                "list_stars",
                "run_code",
                "search_history",
            ]
        );
    }

    #[test]
    fn empty_registry_reports_empty_and_has_no_definitions() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert!(registry.definitions().is_empty());
    }

    #[tokio::test]
    async fn dispatch_unknown_tool_is_a_recoverable_error() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let error = registry
            .dispatch("does_not_exist", json!({ "query": "x" }), &context)
            .await
            .expect_err("unknown tool");
        assert!(error.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn search_history_works_with_no_embedding_provider() {
        // Hybrid degrades to lexical-only when no embedding provider is configured (honest degradation).
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("search_history", json!({ "query": "tauri" }), &context)
            .await
            .expect("search_history runs without an embedding provider (degrades to lexical)");
        // No rows in an empty fixture archive, but the call succeeds and summarizes honestly.
        assert!(outcome.model_text.contains("search_history"));
    }

    #[tokio::test]
    async fn search_history_date_filter_args_are_deserialized() {
        // W-PKG-A: start_date and end_date must be accepted and threaded through to the request.
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch(
                "search_history",
                json!({ "query": "test", "start_date": "2026-06-01", "end_date": "2026-06-30" }),
                &context,
            )
            .await
            .expect("date-filtered search_history must succeed");
        // Empty archive → no rows, but the call succeeds (no deserialization error, no panic).
        assert!(outcome.model_text.contains("search_history"));
    }

    #[tokio::test]
    async fn search_history_sort_and_cursor_args_are_deserialized_and_threaded() {
        // The `sort` + `cursor` fields must parse and flow into the request without a deserialization
        // error (the FIX for "when did I first browse X?" — the agent needs an oldest-first enumeration
        // + a way to page it). An empty archive yields no rows, but the call succeeds (no parse error).
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch(
                "search_history",
                json!({ "query": "mlx", "sort": "oldest", "cursor": "0", "limit": 5 }),
                &context,
            )
            .await
            .expect("sort + cursor args must deserialize and run");
        assert!(outcome.model_text.contains("search_history"));
    }

    #[test]
    fn search_history_schema_and_description_document_sort_and_pagination() {
        // The model can only USE sort/pagination if the schema advertises them and the description
        // teaches the "oldest = first occurrence" + cursor-paging contract (this is how the agent learns
        // to answer "when did I first browse X?" instead of sampling the recent month).
        let def = HistorySearchTool::search_history().definition();
        let props = &def.parameters["properties"];
        assert_eq!(props["sort"]["type"], "string");
        let sort_enum = props["sort"]["enum"].as_array().expect("sort enum");
        assert!(sort_enum.contains(&json!("oldest")));
        assert!(sort_enum.contains(&json!("newest")));
        assert!(sort_enum.contains(&json!("relevance")));
        assert_eq!(props["cursor"]["type"], "string");
        assert!(
            def.description.contains("oldest") && def.description.contains("first"),
            "the description teaches oldest=first occurrence: {}",
            def.description
        );
        assert!(
            def.description.contains("next_cursor") && def.description.contains("cursor"),
            "the description teaches cursor pagination: {}",
            def.description
        );
    }

    #[test]
    fn search_summary_surfaces_next_cursor_when_present() {
        use crate::models::{AiSearchEntry, AiSearchResponse};
        let response = AiSearchResponse {
            total: 3,
            provider_id: "date-ordered".to_string(),
            model: "none".to_string(),
            items: vec![AiSearchEntry {
                history_id: 1,
                profile_id: "p".to_string(),
                url: "https://a.example/".to_string(),
                title: Some("A".to_string()),
                domain: "a.example".to_string(),
                visited_at: "2025-03-04T12:00:00Z".to_string(),
                score: 0.0,
                match_reason: "Lexical match (date-ordered)".to_string(),
                enrichment_excerpt: None,
            }],
            notes: Vec::new(),
            note_codes: Vec::new(),
            next_cursor: Some("1".to_string()),
            applied_limit: Some(1),
            has_more: true,
        };
        let text = summarize_search_for_model("search_history", &response);
        assert!(text.contains("has_more: true"), "has_more surfaced: {text}");
        assert!(
            text.contains("next_cursor: 1"),
            "next_cursor surfaced so the model can page: {text}"
        );

        // No cursor → no next_cursor segment.
        let last = AiSearchResponse { next_cursor: None, has_more: false, ..response };
        let last_text = summarize_search_for_model("search_history", &last);
        assert!(
            !last_text.contains("next_cursor"),
            "no cursor segment on the last page: {last_text}"
        );
    }

    #[tokio::test]
    async fn search_history_with_embedding_provider_runs_hybrid() {
        // With a provider configured the search tool runs the full hybrid pipeline; the empty fixture
        // archive yields no rows, but the call succeeds (no panic, no error).
        let (_dir, context) = context_with(Some(embedding_runtime()));
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("search_history", json!({ "query": "rust", "limit": 5 }), &context)
            .await
            .expect("search_history runs hybrid with a provider");
        assert!(outcome.model_text.contains("search_history"));
    }

    #[tokio::test]
    async fn empty_or_omitted_query_returns_recent_visits() {
        // Browse-by-recency: a blank query (or an omitted one) is no longer rejected — it returns
        // the most recent visits so the model can enumerate history. The empty fixture archive holds
        // no visits, so the call succeeds with zero rows (no error, no panic).
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        for args in [json!({ "query": "   " }), json!({})] {
            let outcome = registry
                .dispatch("search_history", args.clone(), &context)
                .await
                .unwrap_or_else(|error| panic!("empty query {args} must succeed, got {error}"));
            assert!(outcome.model_text.contains("search_history"));
            assert!(outcome.citations.is_empty());
        }
    }

    #[tokio::test]
    async fn invalid_arguments_are_rejected_without_panicking() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        // `query` is required and must be a string; a number is a parse error, not a panic.
        let error = registry
            .dispatch("search_history", json!({ "query": 42 }), &context)
            .await
            .expect_err("invalid args");
        assert!(error.to_string().contains("invalid search arguments"));
    }

    #[test]
    fn search_plane_provider_selection_drops_provider_for_bm25() {
        let (_dir, context) = context_with(Some(embedding_runtime()));
        // Bm25 never threads the provider; the semantic planes do.
        assert!(SearchPlane::Bm25.provider_for(&context).is_none());
        assert!(SearchPlane::Vector.provider_for(&context).is_some());
        assert!(SearchPlane::Hybrid.provider_for(&context).is_some());
    }

    #[test]
    fn summarize_search_lists_rows_and_notes() {
        use crate::models::{AiSearchEntry, AiSearchResponse};
        let response = AiSearchResponse {
            total: 1,
            provider_id: "p1".to_string(),
            model: "m1".to_string(),
            items: vec![AiSearchEntry {
                history_id: 7,
                profile_id: "default".to_string(),
                url: "https://example.com/".to_string(),
                title: Some("Example".to_string()),
                domain: "example.com".to_string(),
                visited_at: "2026-06-21T00:00:00Z".to_string(),
                score: 0.42,
                match_reason: "Lexical match".to_string(),
                enrichment_excerpt: None,
            }],
            notes: vec!["lexical only".to_string()],
            note_codes: Vec::new(),
            next_cursor: None,
            applied_limit: Some(50),
            has_more: false,
        };
        let text = summarize_search_for_model("search_hybrid", &response);
        assert!(text.contains("[7]"));
        assert!(text.contains("https://example.com/"));
        assert!(text.contains("Note: lexical only"));

        // The empty case still names the tool and surfaces notes.
        let empty = AiSearchResponse {
            total: 0,
            provider_id: "p1".to_string(),
            model: "m1".to_string(),
            items: Vec::new(),
            notes: vec!["nothing".to_string()],
            note_codes: Vec::new(),
            next_cursor: None,
            applied_limit: None,
            has_more: false,
        };
        let empty_text = summarize_search_for_model("search_bm25", &empty);
        assert!(empty_text.contains("no matching history rows"));
        assert!(empty_text.contains("Note: nothing"));
    }

    #[test]
    fn search_summary_includes_applied_limit_and_has_more() {
        use crate::models::{AiSearchEntry, AiSearchResponse};
        let response = AiSearchResponse {
            total: 2,
            provider_id: "hybrid".to_string(),
            model: "m1".to_string(),
            items: vec![AiSearchEntry {
                history_id: 1,
                profile_id: "p".to_string(),
                url: "https://a.example/".to_string(),
                title: Some("A".to_string()),
                domain: "a.example".to_string(),
                visited_at: "2026-06-21T00:00:00Z".to_string(),
                score: 0.9,
                match_reason: "Lexical match".to_string(),
                enrichment_excerpt: None,
            }],
            notes: Vec::new(),
            note_codes: Vec::new(),
            next_cursor: None,
            applied_limit: Some(50),
            has_more: true,
        };
        let text = summarize_search_for_model("search_history", &response);
        assert!(text.contains("applied_limit: 50"), "applied_limit in summary: {text}");
        assert!(text.contains("has_more: true"), "has_more in summary: {text}");

        // When has_more is false it should not appear.
        let response_no_more =
            AiSearchResponse { applied_limit: Some(8), has_more: false, ..response.clone() };
        let text2 = summarize_search_for_model("search_history", &response_no_more);
        assert!(text2.contains("applied_limit: 8"), "applied_limit in summary: {text2}");
        assert!(!text2.contains("has_more"), "has_more absent when false: {text2}");
    }

    #[test]
    fn run_code_description_includes_start_end_date_and_examples() {
        let def = RunCodeTool::new().definition();
        assert!(def.description.contains("startDate"), "run_code description includes startDate");
        assert!(def.description.contains("endDate"), "run_code description includes endDate");
        assert!(
            def.description.contains("2026-06-19"),
            "run_code description includes date-range example"
        );
        assert!(
            def.description.contains("machine learning"),
            "run_code description includes no-answer example"
        );
        assert!(
            def.description.contains("{ found: false }"),
            "run_code description includes explicit no-answer shape"
        );
    }

    // ---- W-AI-8 WU-2: the `run_code` tool ----------------------------------------------------

    use super::super::code_mode::CodeOutcome;

    #[test]
    fn run_code_is_registered_by_default() {
        // Default-enabled (2026-06 decision): every agent run advertises `run_code` alongside search.
        let registry = ToolRegistry::with_default_search_tools();
        let names: Vec<String> = registry.definitions().into_iter().map(|def| def.name).collect();
        assert!(names.contains(&"run_code".to_string()), "run_code is a default tool: {names:?}");
    }

    #[test]
    fn run_code_default_matches_new() {
        // `RunCodeTool` is a unit tool; `Default` and `new` are interchangeable (both name `run_code`).
        // The `Default` impl is exercised via the fully-qualified call so the `default_constructed_
        // unit_structs` lint stays clean (`RunCodeTool::default()` on a unit struct trips it).
        let default_tool = <RunCodeTool as Default>::default();
        assert_eq!(default_tool.name(), RunCodeTool::new().name());
        assert_eq!(default_tool.name(), "run_code");
    }

    #[test]
    fn run_code_definition_teaches_the_host_api_and_requires_source() {
        // The definition advertised to the model must be honest about the sandbox (read-only, no
        // net/fs/clock/random) and name the two globals; the schema requires exactly `source`.
        let def = RunCodeTool::new().definition();
        assert_eq!(def.name, "run_code");
        assert!(def.description.contains("query_history"));
        assert!(def.description.contains("fetch_visits"));
        assert!(def.description.contains("READ-ONLY"));
        assert!(def.description.contains("NO network"));
        // F2: the description teaches the `notes` degradation signal and tells the model to honor it.
        assert!(def.description.contains("notes"), "the notes degradation signal is documented");
        assert!(
            def.description.contains("fell back to keyword"),
            "the description gives the concrete semantic→lexical degradation example"
        );
        // F1: the description steers the model to ALWAYS return a value (even a negative one), and no
        // longer tells it to return nothing.
        assert!(
            def.description.contains("Always end the program with `return"),
            "the description steers the model to always return a value"
        );
        assert!(
            def.description.contains("{ found: false }"),
            "the description shows a negative return shape for the no-answer case"
        );
        assert!(
            !def.description.to_lowercase().contains("return nothing if there is no answer"),
            "the misleading 'return nothing' guidance is dropped"
        );
        assert_eq!(def.parameters["required"], json!(["source"]));
        assert_eq!(def.parameters["properties"]["source"]["type"], "string");
    }

    #[test]
    fn maps_a_clean_code_outcome_to_a_populated_tool_outcome() {
        // A clean CodeOutcome (distilled text + one host call + a citation, no error/limit) maps onto
        // a ToolOutcome carrying the code-mode transparency fields verbatim.
        let outcome = CodeOutcome {
            model_text: "{\"count\":1}".to_string(),
            citations: vec![AiCitation {
                history_id: 101,
                profile_id: "p".to_string(),
                url: "https://rust-lang.org/".to_string(),
                title: Some("Rust".to_string()),
                visited_at: "2026-01-01T00:00:00Z".to_string(),
                score: Some(0.9),
                canonical_url: Some("https://rust-lang.org/".to_string()),
            }],
            host_calls: vec![HostCallRecord {
                function: "query_history".to_string(),
                query: Some("rust".to_string()),
                plane: Some("hybrid".to_string()),
                limit: Some(8),
                requested_ids: None,
                args_summary: "query=\"rust\" plane=hybrid limit=8".to_string(),
                row_count: 1,
            }],
            source: "return query_history({query:'rust'});".to_string(),
            error: None,
            limits_hit: None,
        };
        let tool = code_outcome_to_tool_outcome(outcome).expect("clean outcome maps to Ok");
        assert_eq!(tool.model_text, "{\"count\":1}");
        assert_eq!(tool.code_source.as_deref(), Some("return query_history({query:'rust'});"));
        assert_eq!(tool.host_calls.len(), 1);
        assert_eq!(tool.host_calls[0].function, "query_history");
        assert_eq!(tool.host_calls[0].query.as_deref(), Some("rust"));
        assert_eq!(tool.host_calls[0].plane.as_deref(), Some("hybrid"));
        assert_eq!(tool.host_calls[0].limit, Some(8));
        assert_eq!(tool.citations.len(), 1);
        assert_eq!(tool.limits_hit, None);
    }

    #[test]
    fn maps_a_hard_failure_with_no_output_to_a_recoverable_error() {
        // A thrown-JS-error CodeOutcome (error set, empty output) becomes an Err so the harness
        // threads a recoverable is_error tool result (same discipline as the search tools).
        let outcome = CodeOutcome {
            model_text: String::new(),
            source: "return notDefined.boom;".to_string(),
            error: Some("ReferenceError: notDefined is not defined".to_string()),
            ..CodeOutcome::default()
        };
        let error = code_outcome_to_tool_outcome(outcome).expect_err("a hard failure is an Err");
        assert!(error.to_string().contains("run_code failed"));
        assert!(error.to_string().contains("notDefined"));
    }

    #[test]
    fn an_empty_no_error_outcome_is_normalized_to_an_honest_sentinel() {
        // F1: a script that returned nothing AND did not throw is NOT a hard error, but an empty
        // result is silently ambiguous to the model. It is normalized to an honest sentinel (NOT a
        // fabricated answer) and stays a non-error Ok the harness threads back.
        let outcome = CodeOutcome {
            model_text: String::new(),
            source: "let x = 1;".to_string(),
            error: None,
            ..CodeOutcome::default()
        };
        let tool = code_outcome_to_tool_outcome(outcome).expect("an empty no-error run is Ok");
        assert_eq!(tool.model_text, "[run_code completed but returned no value]");

        // Whitespace-only output is treated the same (an honest "no value", not a blank string).
        let whitespace = CodeOutcome {
            model_text: "  \n\t ".to_string(),
            source: "return '   ';".to_string(),
            error: None,
            ..CodeOutcome::default()
        };
        let tool = code_outcome_to_tool_outcome(whitespace).expect("whitespace output is Ok");
        assert_eq!(tool.model_text, "[run_code completed but returned no value]");
    }

    #[test]
    fn an_empty_bounded_outcome_keeps_the_sentinel_and_appends_the_limit_note() {
        // F1 + the limit note compose: a bounded run that ALSO returned nothing reports the honest
        // "no value" sentinel AND the bounded-run note (so the model is never handed a bare string).
        let outcome = CodeOutcome {
            model_text: String::new(),
            source: "while (true) {}".to_string(),
            error: None,
            limits_hit: Some(LimitsHit::Time),
            ..CodeOutcome::default()
        };
        let tool = code_outcome_to_tool_outcome(outcome).expect("a bounded empty run is Ok");
        assert!(tool.model_text.starts_with("[run_code completed but returned no value]"));
        assert!(tool.model_text.contains("bounded by a hard limit"));
        assert!(tool.model_text.contains("wall-time limit"));
    }

    #[test]
    fn an_error_with_partial_output_is_still_returned_not_failed() {
        // If the script produced SOME distilled output, a trailing error does not discard it — the
        // model gets the partial result (Ok), not a recoverable error (only an empty+error is Err).
        let outcome = CodeOutcome {
            model_text: "{\"partial\":true}".to_string(),
            source: "...".to_string(),
            error: Some("late error".to_string()),
            ..CodeOutcome::default()
        };
        let tool = code_outcome_to_tool_outcome(outcome).expect("partial output is kept (Ok)");
        assert_eq!(tool.model_text, "{\"partial\":true}");
    }

    #[test]
    fn a_limited_outcome_appends_an_honest_note_and_is_ok() {
        // A bounded run (output cap hit) returns Ok, carries the limit, and appends an honest note so
        // the model does not treat the truncated result as complete (never fabricated).
        let outcome = CodeOutcome {
            model_text: "truncated...".to_string(),
            source: "return big;".to_string(),
            limits_hit: Some(LimitsHit::Output),
            ..CodeOutcome::default()
        };
        let tool = code_outcome_to_tool_outcome(outcome).expect("a limited run is Ok");
        assert_eq!(tool.limits_hit, Some(LimitsHit::Output));
        assert!(tool.model_text.contains("truncated..."));
        assert!(
            tool.model_text.contains("bounded by a hard limit")
                && tool.model_text.contains("output size cap"),
            "an honest bounded-run note is appended: {}",
            tool.model_text
        );
    }

    #[test]
    fn limits_hit_label_covers_every_variant() {
        // The model-facing label maps every hard-limit variant (keeps the note honest + non-empty).
        assert_eq!(limits_hit_label(LimitsHit::Time), "wall-time limit");
        assert_eq!(limits_hit_label(LimitsHit::Memory), "memory limit");
        assert_eq!(limits_hit_label(LimitsHit::HostCalls), "host-call budget");
        assert_eq!(limits_hit_label(LimitsHit::Output), "output size cap");
        assert_eq!(limits_hit_label(LimitsHit::Cancelled), "cancelled");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn run_code_dispatch_runs_real_js_and_returns_a_distilled_result() {
        // End-to-end through the tool path (the async↔sync `spawn_blocking` bridge): real JS runs in
        // the sandbox and returns a distilled value, carrying the script source on the outcome.
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch(
                "run_code",
                json!({ "source": "return { sum: [1,2,3].reduce((a,b)=>a+b,0) };" }),
                &context,
            )
            .await
            .expect("run_code runs through the tool path");
        let distilled: Value =
            serde_json::from_str(&outcome.model_text).expect("valid JSON output");
        assert_eq!(distilled["sum"], 6);
        assert!(outcome.code_source.is_some(), "the script source is carried for transparency");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn run_code_dispatch_threads_a_thrown_js_error_as_recoverable() {
        // A thrown JS error in the sandbox becomes an Err the harness threads as a recoverable
        // is_error tool result — never a panic, never a hang.
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let error = registry
            .dispatch("run_code", json!({ "source": "return notDefinedAnywhere.boom;" }), &context)
            .await
            .expect_err("a thrown JS error is a recoverable tool error");
        assert!(error.to_string().contains("run_code failed"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn run_code_rejects_empty_or_invalid_source_without_panicking() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        // An empty/whitespace source is rejected before any sandbox run.
        let empty = registry
            .dispatch("run_code", json!({ "source": "   " }), &context)
            .await
            .expect_err("empty source rejected");
        assert!(empty.to_string().contains("must not be empty"));
        // A non-string `source` is a parse error, not a panic.
        let invalid = registry
            .dispatch("run_code", json!({ "source": 42 }), &context)
            .await
            .expect_err("invalid source rejected");
        assert!(invalid.to_string().contains("invalid run_code arguments"));
        // A missing `source` is likewise rejected (the field is required).
        let missing = registry
            .dispatch("run_code", json!({}), &context)
            .await
            .expect_err("missing source rejected");
        assert!(missing.to_string().contains("invalid run_code arguments"));
    }

    // ---- W-AI-9 WPB: intelligence_report, list_stars, list_annotations ---------------------

    #[test]
    fn intelligence_report_definition_names_report_enum_and_key_phrases() {
        let tool = IntelligenceReportTool;
        let def = tool.definition();
        assert_eq!(def.name, "intelligence_report");
        assert!(def.description.contains("pre-computed intelligence report"));
        assert!(def.description.contains("patterns"));
        assert!(def.description.contains("trends"));
        // The schema requires `report` and lists the enum.
        assert_eq!(def.parameters["required"], json!(["report"]));
        let report_prop = &def.parameters["properties"]["report"];
        assert_eq!(report_prop["type"], "string");
        let enum_values = report_prop["enum"].as_array().expect("enum array");
        assert!(enum_values.len() >= 8, "all 8 report variants: {enum_values:?}");
        assert!(enum_values.contains(&json!("top_sites")));
        assert!(enum_values.contains(&json!("overview")));
    }

    #[tokio::test]
    async fn intelligence_report_top_sites_on_empty_archive_returns_not_ready() {
        // Against a fresh tempdir with no intelligence data, the tool returns the honest
        // "not ready" outcome rather than panicking.
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "top_sites" }), &context)
            .await
            .expect("should not panic on empty archive");
        assert!(outcome.model_text.contains("intelligence index has not been built"));
    }

    #[tokio::test]
    async fn intelligence_report_overview_on_empty_archive_returns_not_ready() {
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "overview" }), &context)
            .await
            .expect("should not panic on empty archive");
        assert!(outcome.model_text.contains("intelligence index has not been built"));
    }

    #[tokio::test]
    async fn intelligence_report_sessions_on_empty_archive_returns_not_ready() {
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "sessions" }), &context)
            .await
            .expect("should not panic on empty archive");
        assert!(outcome.model_text.contains("intelligence index has not been built"));
    }

    #[tokio::test]
    async fn intelligence_report_unknown_variant_is_an_error() {
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let error = tool
            .call(json!({ "report": "unicorn" }), &context)
            .await
            .expect_err("unknown variant should error");
        assert!(error.to_string().contains("unknown report variant"));
    }

    #[tokio::test]
    async fn intelligence_report_empty_report_is_rejected() {
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let error = tool
            .call(json!({ "report": "" }), &context)
            .await
            .expect_err("empty report should be rejected");
        assert!(error.to_string().contains("must not be empty"));
    }

    #[tokio::test]
    async fn intelligence_report_invalid_args_are_rejected() {
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let error = tool
            .call(json!({ "report": 42 }), &context)
            .await
            .expect_err("numeric report should fail parse");
        assert!(error.to_string().contains("invalid intelligence_report arguments"));
    }

    #[tokio::test]
    async fn intelligence_report_domain_trend_requires_domain() {
        // L2 — the `domain` required-arg check must run BEFORE the readiness gate, so an omitted
        // domain is an actionable arg ERROR (not the generic "not built" note) even on an empty plane.
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let error = tool
            .call(json!({ "report": "domain_trend" }), &context)
            .await
            .expect_err("domain_trend without `domain` must bail with a required-arg error");
        let message = error.to_string();
        assert!(message.contains("domain"), "got: {message}");
        assert!(
            !message.contains("not been built"),
            "the arg error must pre-empt the readiness note: {message}"
        );
    }

    #[tokio::test]
    async fn intelligence_report_day_insights_requires_start_date() {
        // L2 — the `start_date` required-arg check must run BEFORE the readiness gate (same as above).
        let (_dir, context) = context_with(None);
        let tool = IntelligenceReportTool;
        let error = tool
            .call(json!({ "report": "day_insights" }), &context)
            .await
            .expect_err("day_insights without `start_date` must bail with a required-arg error");
        let message = error.to_string();
        assert!(message.contains("start_date"), "got: {message}");
        assert!(
            !message.contains("not been built"),
            "the arg error must pre-empt the readiness note: {message}"
        );
    }

    // ---- list_stars tool tests ----

    #[test]
    fn list_stars_definition_has_correct_name_and_key_phrases() {
        let tool = ListStarsTool;
        let def = tool.definition();
        assert_eq!(def.name, "list_stars");
        assert!(def.description.contains("starred"));
        assert!(def.description.contains("favorited"));
        assert!(def.description.contains("bookmarks"));
        let sort_prop = &def.parameters["properties"]["sort"];
        assert_eq!(sort_prop["type"], "string");
        let enum_values = sort_prop["enum"].as_array().expect("enum array");
        assert!(enum_values.contains(&json!("recent")));
        assert!(enum_values.contains(&json!("most_revisited")));
    }

    #[tokio::test]
    async fn list_stars_on_empty_archive_returns_no_stars() {
        let (_dir, context) = context_with(None);
        let tool = ListStarsTool;
        let outcome =
            tool.call(json!({}), &context).await.expect("should not panic on empty archive");
        assert!(outcome.model_text.contains("no starred pages found"));
    }

    #[tokio::test]
    async fn list_stars_with_recent_sort_on_empty_archive() {
        let (_dir, context) = context_with(None);
        let tool = ListStarsTool;
        let outcome =
            tool.call(json!({ "sort": "recent" }), &context).await.expect("should succeed");
        assert!(outcome.model_text.contains("no starred pages found"));
    }

    #[tokio::test]
    async fn list_stars_with_most_revisited_sort_on_empty_archive() {
        let (_dir, context) = context_with(None);
        let tool = ListStarsTool;
        let outcome =
            tool.call(json!({ "sort": "most_revisited" }), &context).await.expect("should succeed");
        assert!(outcome.model_text.contains("no starred pages found"));
    }

    #[tokio::test]
    async fn list_stars_invalid_args_are_rejected() {
        let (_dir, context) = context_with(None);
        let tool = ListStarsTool;
        let error = tool
            .call(json!({ "sort": 42 }), &context)
            .await
            .expect_err("numeric sort should fail parse");
        assert!(error.to_string().contains("invalid list_stars arguments"));
    }

    // ---- list_annotations tool tests ----

    #[test]
    fn list_annotations_definition_has_correct_name_and_key_phrases() {
        let tool = ListAnnotationsTool;
        let def = tool.definition();
        assert_eq!(def.name, "list_annotations");
        assert!(def.description.contains("annotations"));
        assert!(def.description.contains("notes"));
        assert!(def.description.contains("tags"));
        let query_prop = &def.parameters["properties"]["query"];
        assert_eq!(query_prop["type"], "string");
    }

    #[tokio::test]
    async fn list_annotations_on_empty_archive_returns_no_annotations() {
        let (_dir, context) = context_with(None);
        let tool = ListAnnotationsTool;
        let outcome =
            tool.call(json!({}), &context).await.expect("should not panic on empty archive");
        assert!(outcome.model_text.contains("no annotations found"));
    }

    #[tokio::test]
    async fn list_annotations_with_query_on_empty_archive() {
        let (_dir, context) = context_with(None);
        let tool = ListAnnotationsTool;
        let outcome =
            tool.call(json!({ "query": "rust" }), &context).await.expect("should succeed");
        assert!(outcome.model_text.contains("no annotations found"));
        assert!(outcome.model_text.contains("rust"));
    }

    #[tokio::test]
    async fn list_annotations_without_query_on_empty_archive() {
        let (_dir, context) = context_with(None);
        let tool = ListAnnotationsTool;
        let outcome = tool.call(json!({ "limit": 10 }), &context).await.expect("should succeed");
        assert!(outcome.model_text.contains("no annotations found"));
    }

    #[tokio::test]
    async fn list_annotations_invalid_args_are_rejected() {
        let (_dir, context) = context_with(None);
        let tool = ListAnnotationsTool;
        let error = tool
            .call(json!({ "query": 42 }), &context)
            .await
            .expect_err("numeric query should fail parse");
        assert!(error.to_string().contains("invalid list_annotations arguments"));
    }

    // ---- W-AI-9 WPB registry integration ----

    #[test]
    fn new_tools_are_registered_by_default() {
        let registry = ToolRegistry::with_default_search_tools();
        let names: Vec<String> = registry.definitions().into_iter().map(|def| def.name).collect();
        assert!(
            names.contains(&"intelligence_report".to_string()),
            "intelligence_report is a default tool: {names:?}"
        );
        assert!(
            names.contains(&"list_stars".to_string()),
            "list_stars is a default tool: {names:?}"
        );
        assert!(
            names.contains(&"list_annotations".to_string()),
            "list_annotations is a default tool: {names:?}"
        );
    }

    #[tokio::test]
    async fn dispatch_intelligence_report_via_registry() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("intelligence_report", json!({ "report": "top_sites" }), &context)
            .await
            .expect("dispatch should succeed");
        // On an empty archive, intelligence is not ready.
        assert!(outcome.model_text.contains("intelligence"));
    }

    #[tokio::test]
    async fn dispatch_list_stars_via_registry() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("list_stars", json!({}), &context)
            .await
            .expect("dispatch should succeed");
        assert!(outcome.model_text.contains("list_stars"));
    }

    #[tokio::test]
    async fn dispatch_list_annotations_via_registry() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("list_annotations", json!({}), &context)
            .await
            .expect("dispatch should succeed");
        assert!(outcome.model_text.contains("list_annotations"));
    }

    // ============================================================================================
    // LETHAL tests: char-boundary truncation (H2), intelligence_report success arms (B1/M3/L1/L2),
    // and list_stars / list_annotations over SEEDED data (H2 note truncation). Each asserts a real
    // result that a regression of the fix would break.
    // ============================================================================================

    #[test]
    fn truncate_str_on_char_boundary_never_splits_a_multibyte_char() {
        // Test 5 — the helper must NEVER return a slice that ends mid-codepoint. We feed a string whose
        // Nth byte lands inside a 3-byte CJK char and assert the returned prefix is valid UTF-8 (it
        // already is, by type), is <= the cap, and is a true prefix on a char boundary (so a raw
        // `&s[..N]` would have panicked at this exact cut). A regression to a byte slice panics here.
        let cjk = "你好世界"; // 4 CJK chars, 3 bytes each = 12 bytes; byte index 4 is mid-char-2.
        assert_eq!(cjk.len(), 12);
        assert!(
            !cjk.is_char_boundary(4),
            "byte 4 must be mid-codepoint for this test to be meaningful"
        );
        // Cut at byte 4 (mid-char): the helper walks back to byte 3 (after the first whole char).
        let cut = truncate_str_on_char_boundary(cjk, 4);
        assert_eq!(cut, "你", "walks back to the nearest boundary, never splitting");
        assert!(cut.len() <= 4);

        // A boundary-aligned cap is honored exactly.
        assert_eq!(truncate_str_on_char_boundary(cjk, 6), "你好");
        // A cap at/over the length returns the whole string.
        assert_eq!(truncate_str_on_char_boundary(cjk, 12), cjk);
        assert_eq!(truncate_str_on_char_boundary(cjk, 99), cjk);
        // Emoji (4-byte) is also never split.
        let emoji = "🦀rust"; // crab is 4 bytes; "rust" is 4 ascii bytes; total 8.
        assert_eq!(
            truncate_str_on_char_boundary(emoji, 2),
            "",
            "a cap inside the first glyph yields the empty prefix"
        );
        assert_eq!(truncate_str_on_char_boundary(emoji, 4), "🦀");
    }

    /// Initializes the archive + Core Intelligence schema and seeds enough rows to make
    /// `intelligence_status().ready == true` and the read models return the seeded data.
    ///
    /// Replicates the relevant INSERTs from `intelligence::tests::fixtures::seed_core_intelligence_fixture`
    /// (`pub(super)` there, so not nameable here): one `sessions` row (so `ready` is true and `sessions`
    /// returns data) and one `domain_daily_rollups` row for `example.com` (so `top_sites` / `domain_trend`
    /// return the seeded domain). Dates are in 2024 so they fall inside any all-time window.
    fn seed_ready_intelligence(context: &AgentToolContext) {
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        let connection =
            crate::archive::open_intelligence_connection(&context.paths, &context.config, None)
                .expect("open intelligence");
        crate::intelligence::ensure_core_intelligence_schema(&connection)
            .expect("ensure core intelligence schema");
        connection
            .execute(
                "INSERT INTO sessions
                   (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES ('s-1', 'chrome:Default', 1711929600000, 1711929660000, 3, 1, 2, 1, 'Researching SQLite', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("seed session");
        connection
            .execute(
                "INSERT INTO domain_daily_rollups
                   (date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls)
                 VALUES ('2024-04-01', 'chrome:Default', 'example.com', 'reference', 9, 0, 1, 4)",
                [],
            )
            .expect("seed domain rollup");
    }

    #[tokio::test]
    async fn intelligence_report_top_sites_returns_seeded_domain_all_time() {
        // Test 3 — top_sites over a READY plane returns the seeded domain. Omitted dates exercise the
        // B1 all-time path (the rollup date_key 2024-04-01 must fall inside the substituted
        // [1900-01-01, today] window; before B1 the `""` start made `date_key <= ""` match nothing).
        let (_dir, context) = context_with(None);
        seed_ready_intelligence(&context);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "top_sites" }), &context)
            .await
            .expect("top_sites over a ready plane returns data");
        assert!(
            !outcome.model_text.contains("not been built"),
            "a ready plane must NOT return the not-ready note: {}",
            outcome.model_text
        );
        assert!(
            outcome.model_text.contains("example.com"),
            "the seeded domain must appear in the all-time top_sites: {}",
            outcome.model_text
        );
    }

    #[tokio::test]
    async fn intelligence_report_sessions_returns_seeded_session_all_time() {
        // Test 3 — sessions over a READY plane returns the seeded session, all-time (omitted dates).
        let (_dir, context) = context_with(None);
        seed_ready_intelligence(&context);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "sessions" }), &context)
            .await
            .expect("sessions over a ready plane returns data");
        assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
        assert!(
            outcome.model_text.contains("Researching SQLite"),
            "the seeded session auto_title must appear: {}",
            outcome.model_text
        );
    }

    #[tokio::test]
    async fn intelligence_report_overview_returns_data_all_time() {
        // Test 3 — overview over a READY plane returns its all-time snapshot WITHOUT error (B1: the
        // all-time path needs start == "1900-01-01"; the omitted dates substitute exactly that, and
        // `is_all_time_range` recognizes it to load/build the snapshot rather than erroring).
        let (_dir, context) = context_with(None);
        seed_ready_intelligence(&context);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "overview" }), &context)
            .await
            .expect("overview over a ready plane (all-time) returns data, not an error");
        assert!(outcome.model_text.contains("intelligence_report(overview)"));
        assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
    }

    #[tokio::test]
    async fn intelligence_report_day_insights_returns_data_for_a_seeded_day() {
        // Test 3 — day_insights for the seeded day returns data (the rollup is on 2024-04-01). The RAW
        // start_date is the target day (not the all-time sentinel substitution).
        let (_dir, context) = context_with(None);
        seed_ready_intelligence(&context);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "day_insights", "start_date": "2024-04-01" }), &context)
            .await
            .expect("day_insights for a seeded day returns data");
        assert!(outcome.model_text.contains("intelligence_report(day_insights)"));
        assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
        // The day's top_sites (built from the rollup) name the seeded domain.
        assert!(
            outcome.model_text.contains("example.com"),
            "the seeded day must surface the domain: {}",
            outcome.model_text
        );
    }

    #[tokio::test]
    async fn intelligence_report_domain_trend_returns_seeded_points() {
        // Test 3 — domain_trend (which reads domain_daily_rollups) returns the seeded point, all-time.
        let (_dir, context) = context_with(None);
        seed_ready_intelligence(&context);
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "domain_trend", "domain": "example.com" }), &context)
            .await
            .expect("domain_trend over a ready plane returns data");
        assert!(!outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
        assert!(
            outcome.model_text.contains("2024-04-01"),
            "the seeded rollup date_key must appear in the trend: {}",
            outcome.model_text
        );
    }

    #[tokio::test]
    async fn intelligence_report_remaining_arms_run_on_a_ready_plane() {
        // Test 3 — the arms NOT covered by the per-report success tests above (search_trails /
        // activity_mix / browsing_rhythm) must each dispatch over a READY plane without error,
        // returning their (possibly empty) result labeled by report — exercising every remaining
        // dispatch arm. `ensure_core_intelligence_schema` (via the seed) creates their tables, so a
        // sparse plane yields empty-but-valid results, never a "no such table" error.
        let (_dir, context) = context_with(None);
        seed_ready_intelligence(&context);
        let tool = IntelligenceReportTool;
        for report in ["search_trails", "activity_mix", "browsing_rhythm"] {
            let outcome =
                tool.call(json!({ "report": report }), &context).await.unwrap_or_else(|error| {
                    panic!("{report} over a ready plane must succeed: {error}")
                });
            assert!(
                outcome.model_text.contains(&format!("intelligence_report({report})")),
                "the {report} arm ran and labeled its result: {}",
                outcome.model_text
            );
            assert!(
                !outcome.model_text.contains("not been built"),
                "a ready plane must not return the not-ready note for {report}: {}",
                outcome.model_text
            );
        }
    }

    #[tokio::test]
    async fn intelligence_report_not_ready_when_plane_is_empty() {
        // Test 3 (keep a not-ready arm) — an initialized-but-empty plane returns the honest note.
        let (_dir, context) = context_with(None);
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "top_sites" }), &context)
            .await
            .expect("empty plane returns the honest not-ready note, never an error");
        assert!(outcome.model_text.contains("not been built"), "got: {}", outcome.model_text);
    }

    #[tokio::test]
    async fn intelligence_report_truncates_over_budget_json_on_a_char_boundary() {
        // Test 3 (>8KB truncation wiring) — seed MANY domains with CJK text so the serialized
        // top_sites JSON exceeds the 8000-byte budget. This proves the over-budget branch is WIRED
        // at the intelligence_report site: it truncates and carries the marker, producing valid
        // output. The char-boundary PANIC-safety of the cut itself is proven deterministically by
        // `truncate_str_on_char_boundary_never_splits_a_multibyte_char` (the helper, cut forced
        // mid-codepoint) and `list_annotations_truncates_long_cjk_note_on_a_char_boundary` (asserts
        // `!is_char_boundary` before the cut); we do NOT rely on byte 8000 of this serialized blob
        // happening to land mid-codepoint (it need not, and depending on that would be brittle).
        let (_dir, context) = context_with(None);
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        let connection =
            crate::archive::open_intelligence_connection(&context.paths, &context.config, None)
                .expect("open intelligence");
        crate::intelligence::ensure_core_intelligence_schema(&connection).expect("ensure schema");
        // One session so the plane is ready; then 300 domains, each carrying a long CJK category so the
        // pretty-printed JSON blows past 8000 bytes and a multibyte char is highly likely to straddle
        // the cut.
        connection
            .execute(
                "INSERT INTO sessions
                   (session_id, profile_id, first_visit_ms, last_visit_ms, visit_count, search_count, domain_count, is_deep_dive, auto_title, computed_at)
                 VALUES ('s-1', 'chrome:Default', 1, 2, 1, 0, 1, 0, NULL, '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("seed session");
        let cjk_category = "参考资料分类".repeat(4); // multibyte, repeated so it shows up across many rows.
        for index in 0..300 {
            connection
                .execute(
                    "INSERT INTO domain_daily_rollups
                       (date_key, profile_id, registrable_domain, domain_category, visit_count, search_count, new_domain_visits, unique_urls)
                     VALUES ('2024-04-01', 'chrome:Default', ?1, ?2, ?3, 0, 0, 1)",
                    params![format!("例子{index}.example.com"), cjk_category, index + 1],
                )
                .expect("seed many domains");
        }
        let tool = IntelligenceReportTool;
        let outcome = tool
            .call(json!({ "report": "top_sites", "limit": 300 }), &context)
            .await
            .expect("a huge CJK result truncates without panicking");
        // Read the length unconditionally: it proves truncation actually BOUNDED the output (the raw
        // 300-domain blob is far larger than the budget) and exercises the size path without relying on
        // an assert-failure message to evaluate it. Headroom covers the label prefix + the marker.
        let out_len = outcome.model_text.len();
        assert!(
            out_len <= INTELLIGENCE_REPORT_MAX_BYTES + 512,
            "truncation bounds model_text near the budget (label + marker headroom): len={out_len}"
        );
        assert!(
            outcome.model_text.contains("[truncated;"),
            "an over-budget result carries the truncation marker"
        );
        // The whole model_text is valid UTF-8 by type; the load-bearing claim is "no panic at the cut".
        assert!(outcome.model_text.contains("intelligence_report(top_sites)"));
    }

    #[tokio::test]
    async fn list_stars_returns_seeded_star_details() {
        // Test 4 — list_stars over a SEEDED archive returns the starred page's domain/title/visit-count.
        let (_dir, context) = context_with(None);
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        // Star a URL through the public API (the same canonicalization the tool resolves).
        crate::stars::set_star(
            &context.paths,
            &context.config,
            None,
            crate::models::SetStarRequest {
                entity_kind: crate::models::StarEntityKind::Url,
                entity_key: "https://example.com/page".to_string(),
                source_profile: None,
            },
        )
        .expect("star a url");
        let tool = ListStarsTool;
        let outcome =
            tool.call(json!({}), &context).await.expect("list_stars returns the seeded star");
        assert!(outcome.model_text.contains("1 starred item"), "got: {}", outcome.model_text);
        assert!(
            outcome.model_text.contains("example.com"),
            "the starred domain must appear: {}",
            outcome.model_text
        );
    }

    #[tokio::test]
    async fn list_stars_renders_untitled_branch() {
        // Test 4 — the "(untitled)" formatting branch: a starred URL with no title row renders the
        // sentinel rather than an empty title. A regression dropping the branch fails this.
        let (_dir, context) = context_with(None);
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        crate::stars::set_star(
            &context.paths,
            &context.config,
            None,
            crate::models::SetStarRequest {
                entity_kind: crate::models::StarEntityKind::Url,
                entity_key: "https://untitled.example/x".to_string(),
                source_profile: None,
            },
        )
        .expect("star a url");
        let tool = ListStarsTool;
        let outcome = tool.call(json!({}), &context).await.expect("list_stars succeeds");
        assert!(
            outcome.model_text.contains("(untitled)"),
            "a star with no title row renders the untitled sentinel: {}",
            outcome.model_text
        );
    }

    #[tokio::test]
    async fn list_annotations_returns_notes_and_tags_branch() {
        // Test 4 — list_annotations over SEEDED annotations returns the note + the tags-present branch.
        let (_dir, context) = context_with(None);
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        let url = "https://example.com/tagged";
        crate::annotations::set_notes(
            &context.paths,
            &context.config,
            None,
            crate::models::SetNotesRequest {
                url: url.to_string(),
                notes: "Short note about Rust".to_string(),
                source_profile: None,
            },
        )
        .expect("set notes");
        crate::annotations::replace_tags(
            &context.paths,
            &context.config,
            None,
            crate::models::ReplaceTagsRequest {
                url: url.to_string(),
                tags: vec!["rust".to_string(), "reference".to_string()],
                source_profile: None,
            },
        )
        .expect("set tags");
        let tool = ListAnnotationsTool;
        let outcome = tool.call(json!({}), &context).await.expect("list_annotations returns data");
        assert!(outcome.model_text.contains("1 annotation"), "got: {}", outcome.model_text);
        assert!(
            outcome.model_text.contains("Short note about Rust"),
            "note: {}",
            outcome.model_text
        );
        // The tags-present branch renders the bracketed list (tags are normalized + sorted, so
        // alphabetical order, not insertion order).
        assert!(outcome.model_text.contains("[reference, rust]"), "tags: {}", outcome.model_text);
    }

    #[tokio::test]
    async fn list_annotations_renders_no_tags_branch() {
        // Test 4 — the no-tags branch: a note with no tags renders without a "[...]" segment.
        let (_dir, context) = context_with(None);
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        crate::annotations::set_notes(
            &context.paths,
            &context.config,
            None,
            crate::models::SetNotesRequest {
                url: "https://example.com/plain".to_string(),
                notes: "A note with no tags".to_string(),
                source_profile: None,
            },
        )
        .expect("set notes");
        let tool = ListAnnotationsTool;
        let outcome = tool.call(json!({}), &context).await.expect("list_annotations returns data");
        assert!(outcome.model_text.contains("A note with no tags"), "got: {}", outcome.model_text);
        assert!(
            !outcome.model_text.contains('['),
            "no-tag note must not render a bracket: {}",
            outcome.model_text
        );
    }

    #[tokio::test]
    async fn list_annotations_truncates_long_cjk_note_on_a_char_boundary() {
        // Test 4 (H2) — a CJK note LONGER than 120 bytes must truncate WITHOUT panicking. CJK chars are
        // 3 bytes each, so the 120-byte cut lands mid-codepoint; a regression to `&item.notes[..120]`
        // panics here. We assert the preview is valid (it is, by type), is truncated (the "..." marker
        // appears), and the full note is NOT present (it was cut).
        let (_dir, context) = context_with(None);
        crate::archive::ensure_archive_initialized(&context.paths, &context.config, None)
            .expect("init archive");
        // 60 CJK chars = 180 bytes; byte 120 sits inside char #41 (40*3=120 is a boundary, so use 41
        // chars of a different glyph to force a mid-codepoint cut). Build a note whose 120th byte is
        // mid-char: prefix 1 ASCII char then CJK so 120 is NOT a multiple of 3 from a char start.
        let long_cjk_note = format!("x{}", "记录笔记内容".repeat(20)); // 1 + 6*3*20 = 361 bytes.
        assert!(long_cjk_note.len() > ANNOTATION_NOTE_PREVIEW_MAX_BYTES);
        assert!(
            !long_cjk_note.is_char_boundary(ANNOTATION_NOTE_PREVIEW_MAX_BYTES),
            "byte 120 must be mid-codepoint so a raw slice would panic"
        );
        crate::annotations::set_notes(
            &context.paths,
            &context.config,
            None,
            crate::models::SetNotesRequest {
                url: "https://example.com/cjk".to_string(),
                notes: long_cjk_note.clone(),
                source_profile: None,
            },
        )
        .expect("set notes");
        let tool = ListAnnotationsTool;
        let outcome = tool
            .call(json!({}), &context)
            .await
            .expect("a long CJK note truncates without panicking");
        assert!(
            outcome.model_text.contains("..."),
            "the preview is truncated: {}",
            outcome.model_text
        );
        assert!(
            !outcome.model_text.contains(&long_cjk_note),
            "the FULL over-budget note must not appear; only the capped preview does"
        );
    }
}
