//! Failure modes: tools use the wrong owner, output caps lose notes or reports, or cursors splice revisions.
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
//! - Search delegates to the area-3 entry point. Intelligence and user records use
//!   native cursor pages, with byte continuation for oversized reports and annotations.

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

/// Max rows the MODEL-FACING `search_history` tool returns in one call — the CONTEXT cap.
///
/// This is intentionally far below the retrieval ceiling (`search::MAX_SEARCH_ROWS` /
/// `code_mode::MAX_ROWS_PER_CALL`, both 1000). The two paths differ because of WHERE the rows go:
/// every row this tool returns is rendered into the model's CONTEXT, so a large page would bloat the
/// prompt and crowd out reasoning — 50 is the bounded evidence sample (02 §F). The `run_code` sandbox
/// is the opposite case: its `query_history` rows stay INSIDE the wasm sandbox and never touch the
/// model's context (only the script's small `return` value does), so it uses the full 1000-row page to
/// paginate + aggregate the WHOLE result set. We clamp the tool's `limit` to this cap HERE, before
/// calling the shared `search_history_internal` (whose own ceiling is the larger retrieval bound), so
/// the context protection lives at the model-facing call site and the sandbox path is unthrottled.
const MAX_TOOL_ROWS: u32 = 50;

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
    /// A search behind this result ran against an index that didn't cover the whole archive, so
    /// the harness tells the user the answer can miss pages.
    pub coverage_incomplete: bool,
}

/// Read-only execution context shared by every retrieval tool.
///
/// Carries exactly what `search_history_internal` needs to rerun retrieval under the current unlock
/// state + scope defaults, without reaching back into higher-level orchestration. `embedding_provider`
/// is `None` when no embedding provider is configured — the lexical (`search_bm25`) plane still works
/// (honest degradation), while the semantic planes degrade to lexical-only with a note.
#[derive(Clone)]
pub struct AgentToolContext {
    pub vault: crate::Vault,
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
    /// The computer's time zone, in which model-facing tool output renders visit timestamps as
    /// LOCAL date/time so the clockless model never hand-computes epoch/timezone math. A named
    /// zone, so each instant gets the offset of its own date (daylight saving included).
    pub time_zone: chrono_tz::Tz,
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
            description: "Search browser history by meaning, URL, title, profile, domain, or date range (hybrid lexical + semantic). Returns the best matching visits with their ids. The `query` is OPTIONAL: call it with an empty or omitted query to list the MOST RECENT visits (browse by recency). Use `start_date` and `end_date` (YYYY-MM-DD) to narrow results to a date range — for example \"last Friday\" or \"this week\". Use `sort` to control ordering: 'oldest' finds the FIRST/earliest occurrence of a term across ALL history (essential for \"when did I first browse X?\" — the default relevance ranking returns the first ranked page, not chronological order); 'newest' the latest; the default ranks by relevance. The response includes `applied_limit` (the actual limit used after clamping), `has_more` (whether more rows exist beyond the limit), and `next_cursor` — pass `next_cursor` back as `cursor` to page through the matches in the chosen sort order. Every match is reachable by cursor; keep query, filters, sort and limit unchanged. `snapshot_total` is exact for the pinned reader. When `search_state.coverageComplete` is false, indexing is incomplete: an empty result is not evidence of absence; start a new search to include later indexed data. When no embedding provider is configured, results use lexical retrieval only (an honest degradation, not an error).",
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
        // CONTEXT CAP: clamp the model-facing page to [`MAX_TOOL_ROWS`] (50) BEFORE retrieval — every
        // returned row enters the model's context, so the tool stays a bounded evidence sample even
        // though `search_history_internal`'s own ceiling is the larger retrieval bound (which only the
        // `run_code` sandbox uses, since its rows never reach the model). Clamping here (not in the
        // internal) is what lets the two paths differ.
        let request = AiSearchRequest {
            query: parsed.query,
            profile_id: parsed.profile_id.or_else(|| context.default_profile_id.clone()),
            domain: parsed.domain.or_else(|| context.default_domain.clone()),
            limit: Some(parsed.limit.unwrap_or(context.default_limit).min(MAX_TOOL_ROWS)),
            cursor: parsed.cursor,
            sort: parsed.sort,
            starred_only: parsed.starred_only,
            start_date: parsed.start_date,
            end_date: parsed.end_date,
            browser_kind: None,
        };
        let provider = self.plane.provider_for(context);
        let response = search_history_internal(
            &context.vault,
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
        let model_text = summarize_search_for_model(self.name, &response, context.time_zone);
        let coverage_incomplete =
            response.search_state.as_ref().is_some_and(|state| !state.coverage_complete);
        // The search tools carry no code-mode fields (those default); only `run_code` populates them.
        Ok(ToolOutcome { model_text, citations, coverage_incomplete, ..ToolOutcome::default() })
    }
}

/// Renders a UTC epoch-millis instant as a LOCAL `YYYY-MM-DD HH:MM` string in `zone`.
/// Model-facing output uses this so the clockless model never converts epoch ms or applies a
/// timezone by hand (a real trace showed it burning hundreds of reasoning lines doing exactly that).
pub(super) fn format_local_ms(ms: i64, zone: chrono_tz::Tz) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.with_timezone(&zone).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

/// Renders an RFC-3339 timestamp as a LOCAL `YYYY-MM-DD HH:MM` string in `zone`; returns the input
/// unchanged if it cannot be parsed (never panics on unexpected data).
pub(super) fn format_local_rfc3339(value: &str, zone: chrono_tz::Tz) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&zone).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| value.to_string())
}

/// The LOCAL `YYYY-MM-DD` calendar date for an RFC-3339 timestamp in `zone` (empty on parse failure).
pub(super) fn local_date_rfc3339(value: &str, zone: chrono_tz::Tz) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&zone).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Builds the compact, bounded, model-facing summary of one search result.
///
/// 02 §F bounded-context rule: thread evidence by row id + count + a short line per hit, NEVER the
/// full result set. Each line is `[id] visited | url — title (reason, score)` so the model can cite
/// a real history row id; any degradation notes are appended so the model knows recall was limited.
/// `zone` renders each row's `visited_at` as LOCAL time so the clockless model never converts epoch
/// ms or a timezone by hand.
fn summarize_search_for_model(
    tool_name: &str,
    response: &crate::models::AiSearchResponse,
    zone: chrono_tz::Tz,
) -> String {
    if response.items.is_empty() {
        let limit_info =
            response.applied_limit.map(|l| format!(" (applied_limit: {l})")).unwrap_or_default();
        let mut text = if response
            .search_state
            .as_ref()
            .is_some_and(|state| !state.coverage_complete)
        {
            format!(
                "{tool_name}: no matches in this indexed snapshot{limit_info}; archive coverage is incomplete."
            )
        } else {
            format!("{tool_name}: no matching history rows{limit_info}.")
        };
        for note in &response.notes {
            text.push_str("\nNote: ");
            text.push_str(note);
        }
        append_search_state(&mut text, response);
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
            visited = format_local_rfc3339(&item.visited_at, zone),
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
    append_search_state(&mut text, response);
    text
}

fn append_search_state(text: &mut String, response: &crate::models::AiSearchResponse) {
    if let Some(state) = &response.search_state {
        text.push_str(&format!(
            "\nsnapshot_total: {}; search_state: {}",
            response.total,
            serde_json::to_string(state).expect("serializable search coverage")
        ));
    }
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
const RUN_CODE_DESCRIPTION: &str = "Run a short JavaScript program in a locked-down sandbox over the user's browser history when a question needs computation/aggregation across many visits (counts, grouping, joins, dedup) that a single search cannot express. The program is READ-ONLY: there is NO network, NO filesystem, NO real clock (Date.now() is 0), and NO randomness; it is bounded by hard time/memory/host-call/output limits, so loop and aggregate freely. Two globals are available, both synchronous; query_history returns { rows: [...], notes: [...], total: number, hasMore: boolean, nextCursor: string|null, searchState: { coverageComplete: boolean, indexer: {...}, snapshotSeq: number } }: query_history({ query?: string, plane?: \"hybrid\"|\"vector\"|\"bm25\", sort?: \"relevance\"|\"newest\"|\"oldest\", limit?: number, cursor?: string, profileId?: string, domain?: string, starredOnly?: boolean, startDate?: string, endDate?: string }) runs the same hybrid/lexical/semantic retrieval as the search tools (each row has id, url, title, domain, visitedAtLocal (LOCAL wall-clock time, the user's timezone), localDate (YYYY-MM-DD in the user's timezone — GROUP BY THIS for per-day buckets), score, matchReason, canonicalUrl, plus visitedAt which is the raw UTC instant — use the LOCAL fields for any date/time/day grouping and avoid visitedAt for display). `query` is OPTIONAL — call query_history({}) or query_history({ query: \"\" }) to enumerate the MOST RECENT visits (browse by recency) when you need to list recent history or find the date range for a date/recency question; `sort` controls ordering — \"oldest\" enumerates the matches EARLIEST-first so you can find the FIRST time a term appears across ALL history (the default \"relevance\" returns a ranked page rather than chronological order, so it cannot answer \"when did I first browse X?\"), \"newest\" is latest-first; to page through the matches pass the reply's `nextCursor` back as `cursor` (when `hasMore` is true) and keep the same `sort`; there is no retrieval window; keep query, sort, filters and limit unchanged when reusing the cursor; `startDate` and `endDate` are optional YYYY-MM-DD strings that restrict results to visits within that date range (inclusive); fetch_visits(ids: number[]) resolves specific visit ids to rows. The `notes` array on each result holds honest warnings about retrieval scope or degradation (for example, semantic/hybrid search fell back to keyword-only because no embedding provider is configured) — read them and reflect any limitation in your answer; do not ignore them. Always end the program with `return <value>` to hand back a SMALL distilled result (a summary object/array, not the raw rows) — that returned value is the ONLY thing you will see, and it is truncated if too large. Even when there is no answer, return a small explicit result such as { found: false } rather than returning nothing. PROCESS THE FULL SET, NOT A SAMPLE: the rows query_history returns stay INSIDE this sandbox — they never enter your context, so only the small value you `return` costs context. That means you can fetch hundreds or thousands of visits, loop over ALL of them, and hand back a tiny summary. So do not answer from a single 50-row sample: when a question is about ALL matching visits (\"how many\", \"group by domain\", \"every site I visited that week\"), PAGINATE and AGGREGATE. Use a LARGE page (limit up to 1000 per call) so you cover the set in few calls — you have a budget of 64 query_history/fetch_visits calls total, so big pages + stopping as soon as you are done is the efficient pattern. Loop: pass the reply's `nextCursor` back as `cursor` (keeping the SAME query/sort/filters) and keep going while `hasMore` is true AND `nextCursor` is not null; stop when `hasMore` is false (you have the whole set) or you reach a sandbox execution limit. Carry the cursor into another run when work remains. Check `searchState.coverageComplete`: while indexing is incomplete, neither an empty page nor its count proves archive-wide absence. Accumulate into JS structures as you go, then `return` only the distilled summary. Example: const result = query_history({ query: \"\", startDate: \"2026-06-19\", endDate: \"2026-06-19\" }); const domains = {}; result.rows.forEach(r => { domains[r.domain] = (domains[r.domain]||0)+1; }); return { date: \"2026-06-19\", domainCounts: domains, returned: result.rows.length, total: result.total, nextCursor: result.nextCursor, searchState: result.searchState }; Example: const result = query_history({ query: \"machine learning\" }); if (result.rows.length === 0) return { found: false, searchState: result.searchState }; return { count: result.rows.length, nextCursor: result.nextCursor, searchState: result.searchState, pageDomains: [...new Set(result.rows.map(r=>r.domain))].slice(0,5) }; Example (paginate + aggregate the FULL set — count every visit by domain across a week, not just the first page): const byDomain = {}; const byDay = {}; let total = 0; let cursor = null; let truncated = false; let searchState = null; for (let i = 0; i < 64; i++) { const page = query_history({ query: \"\", sort: \"oldest\", startDate: \"2026-06-15\", endDate: \"2026-06-21\", limit: 1000, cursor }); searchState = page.searchState; for (const r of page.rows) { total++; byDomain[r.domain] = (byDomain[r.domain] || 0) + 1; const day = r.localDate; byDay[day] = (byDay[day] || 0) + 1; } if (!page.hasMore) { cursor = null; break; } if (!page.nextCursor) { truncated = true; break; } cursor = page.nextCursor; } return { total, byDomain, byDay, truncated: truncated || cursor !== null || searchState?.coverageComplete !== true, nextCursor: cursor, searchState };";

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
        coverage_incomplete,
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

    Ok(ToolOutcome {
        model_text,
        citations,
        code_source: Some(source),
        host_calls,
        limits_hit,
        coverage_incomplete,
    })
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
    cursor: Option<String>,
    trails_cursor: Option<String>,
    output_cursor: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    domain: Option<String>,
    limit: Option<u32>,
    /// Required for the `session_detail` report: the `sessionId` from a `sessions` report row, so the
    /// model can drill from a session into its actual visits (Defect C).
    session_id: Option<String>,
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
    SessionDetail,
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
            "session_detail" => Self::SessionDetail,
            other => anyhow::bail!(
                "unknown report variant `{other}`; expected one of: top_sites, sessions, search_trails, activity_mix, browsing_rhythm, domain_trend, day_insights, overview, session_detail"
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
                             "browsing_rhythm", "domain_trend", "day_insights", "overview",
                             "session_detail"],
                    "description": "Which pre-computed report to retrieve."
                },
                "start_date": { "type": "string", "description": "Start of date range (YYYY-MM-DD). Omit for all-time." },
                "end_date": { "type": "string", "description": "End of date range (YYYY-MM-DD). Omit for all-time." },
                "domain": { "type": "string", "description": "Optional domain filter (e.g. 'youtube.com')." },
                "limit": { "type": "integer", "description": "Items per data page; follow nextCursor to reach all records." },
                "cursor": {"type":"string","description":"nextCursor from the prior data page, or visitsNextCursor for session_detail; keep the same report and scope."},
                "trails_cursor": {"type":"string","description":"trailsNextCursor from session_detail; advances its trail list independently of visits."},
                "output_cursor": {"type":"string","description":"nextOutputCursor from an oversized report. Join jsonFragment values until null. If restartRequired is true, discard prior fragments and restart without output_cursor."},
                "session_id": { "type": "string", "description": "Required for the session_detail report: the sessionId from a sessions report row." }
            },
            "required": ["report"]
        })
    }
}

const INTELLIGENCE_REPORT_DESCRIPTION: &str = "Retrieve a pre-computed intelligence report about the user's browsing patterns. This tool returns ONLY high-level STATISTICAL and STRUCTURAL aggregates — visit/search/domain counts, top sites, session boundaries (each with a heuristic label), search-trail structure, activity mix, browsing rhythm, domain trends, and daily rollups. These are mechanical counts and structure; they carry NO semantic understanding of the content. This tool CANNOT tell you WHAT the user actually did, read, researched, or cared about inside a session or time window, and it cannot do any topic- or meaning-level analysis. Use it for the quantitative SHAPE of a period (how much, when, which domains, how sessions are bounded) — for those numbers it is faster and more complete than search. But to understand WHAT the user actually did or read (content, topics, intent), do NOT infer it from a session's autoTitle or its counts — read the ACTUAL visits: call this tool with report=\"session_detail\" and a sessionId to get the real URLs/titles/queries inside one session, or use search_history / run_code over the visit rows, and reason over those. In the sessions report, autoTitle is a HEURISTIC label naming a session by its most salient domain/query (NOT the subject of every visit); visitCount is the TOTAL visits in the session across all domains (NOT visits to the autoTitle term).";

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

        let _paths = &context.paths;
        let _config = &context.config;
        let _key = context.database_key.as_deref();

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
            IntelReport::DayInsights
                if parsed.start_date.as_deref().unwrap_or("").trim().is_empty() =>
            {
                anyhow::bail!(
                    "the `start_date` argument is required for the day_insights report (use it as the target date)"
                );
            }
            IntelReport::SessionDetail
                if parsed.session_id.as_deref().unwrap_or("").trim().is_empty() =>
            {
                anyhow::bail!(
                    "the `session_id` argument is required for the session_detail report (use the sessionId from a sessions report row)"
                );
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
        let ready = crate::intelligence::intelligence_status(&context.vault)?.ready;
        if !ready {
            return Ok(intelligence_not_ready_outcome(&parsed.report));
        }

        // L1 — clamp a model-supplied limit to at least 1: `limit: 0` would become `page_size: 0` →
        // `LIMIT 0` → zero rows for sessions/search_trails even on a built plane. Keep the per-report
        // upper defaults (`get_top_sites`/`get_domain_trend` clamp their own caps internally).
        let paged_limit = parsed.limit.map(|value| value.max(1)).unwrap_or(20);

        // Exhaustive over `IntelReport` — the compiler guarantees every variant is handled, so there
        // is no catch-all arm (the unknown case was already rejected by `IntelReport::parse` above).
        let mut result_json: Value = match report {
            IntelReport::TopSites => {
                let request = TopSitesRequest {
                    cursor: parsed.cursor.clone(),
                    date_range,
                    profile_id: None,
                    profile_ids: Vec::new(),
                    sort_by: Some("visit_count".to_string()),
                    limit: parsed.limit.map(|value| value.max(1)).or(Some(20)),
                };
                let result = crate::intelligence::get_top_sites(&context.vault, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::Sessions => {
                let request = PagedDateRangeRequest {
                    cursor: parsed.cursor.clone(),
                    date_range,
                    profile_id: None,

                    page_size: paged_limit,
                };
                let result = crate::intelligence::get_sessions(&context.vault, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::SearchTrails => {
                let request = SearchTrailQueryRequest {
                    cursor: parsed.cursor.clone(),
                    date_range,
                    profile_id: None,
                    engine: None,

                    page_size: paged_limit,
                };
                let result = crate::intelligence::get_search_trails(&context.vault, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::ActivityMix => {
                let request = ScopedDateRangeRequest { date_range, profile_id: None };
                let result = crate::intelligence::get_activity_mix(&context.vault, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::BrowsingRhythm => {
                let request = CategoryFilteredDateRangeRequest {
                    date_range,
                    profile_id: None,
                    category: None,
                };
                let result = crate::intelligence::get_browsing_rhythm(&context.vault, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::DomainTrend => {
                // The `domain` presence was validated above the readiness gate (L2).
                let domain = parsed.domain.clone().unwrap_or_default();
                let request = DomainTrendRequest { registrable_domain: domain, date_range };
                let result = crate::intelligence::get_domain_trend(&context.vault, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::DayInsights => {
                // `start_date` presence was validated above (L2); use the RAW supplied date as the
                // target day (not the all-time sentinel substitution, which is only for ranges).
                let date = parsed.start_date.clone().unwrap_or_default();
                let request = DayInsightsRequest { date, profile_id: None };
                let result = crate::intelligence::get_day_insights(&context.vault, &request)?;
                serde_json::to_value(result)?
            }
            IntelReport::Overview => {
                let request = ScopedDateRangeRequest { date_range, profile_id: None };
                let result = crate::intelligence::get_intelligence_primary_overview(
                    &context.vault,
                    &request,
                )?;
                serde_json::to_value(result)?
            }
            IntelReport::SessionDetail => {
                // `session_id` presence was validated above (L2). A bad id is `Err(not found)` from
                // the read model, which propagates as a recoverable tool error (never a panic).
                let session_id = parsed.session_id.clone().unwrap_or_default();
                let result = crate::intelligence::get_session_detail(
                    &context.vault,
                    &crate::SessionDetailRequest {
                        session_id: session_id.to_owned(),
                        visits_cursor: parsed.cursor.clone(),
                        trails_cursor: parsed.trails_cursor.clone(),
                        page_size: parsed.limit,
                    },
                )?;
                serde_json::to_value(result)?
            }
        };

        // Localize the model-facing timestamps at the AGENT-TOOL boundary (NOT in the shared read-model
        // structs, which are a FRONTEND contract). The model is clockless and the sandbox clock reads 0,
        // so a session/trail's epoch-ms `firstVisitMs`/`lastVisitMs` (and a session_detail visit's
        // `visitTimeMs`) get sibling LOCAL string fields it can use directly without epoch/timezone math.
        match report {
            IntelReport::Sessions => {
                enrich_session_like_local(&mut result_json["sessions"], context.time_zone);
            }
            IntelReport::SearchTrails => {
                enrich_session_like_local(&mut result_json["trails"], context.time_zone);
            }
            IntelReport::SessionDetail => {
                enrich_first_last_visit_local(&mut result_json["session"], context.time_zone);
                if let Some(visits) = result_json["visits"].as_array_mut() {
                    for visit in visits {
                        if let Some(ms) = visit.get("visitTimeMs").and_then(Value::as_i64) {
                            visit["visitTimeLocal"] = json!(format_local_ms(ms, context.time_zone));
                        }
                    }
                }
                // The session's trails carry firstVisitMs/lastVisitMs too — localize them so the
                // drill-down the fix added doesn't reintroduce raw epoch ms inside its own payload.
                enrich_session_like_local(&mut result_json["trails"], context.time_zone);
            }
            _ => {}
        }

        // Timing diagnostics are not report data and would change a text-page fingerprint.
        super::report_pages::remove_volatile_diagnostics(&mut result_json);
        let raw = serde_json::to_string_pretty(&result_json)?;
        let report_text = super::report_pages::output(&raw, parsed.output_cursor.as_deref())?;
        // Co-locate the sessions caveats WITH the data the model reasons over at answer-composition
        // time (the tool DESCRIPTION is only read at tool-selection time, so a buried caveat there is
        // forgotten by the time the model is reading rows). The preamble kills the "autoTitle == the
        // thing visited N times" misread (Defect B), and the footer makes the session_detail drill-down
        // discoverable from the output itself (Defect C) rather than only the description.
        //
        // The preamble/footer are `concat!` of separate literals, NOT a `\`-continued string: a
        // backslash-continued literal here desyncs the coverage gate's brace-counting line-mask and
        // falsely reports this arm as uncovered.
        let sessions_preamble = concat!(
            "intelligence_report(sessions): result follows. NOTE: autoTitle is a heuristic label ",
            "naming each session by its most salient domain/query — NOT the subject of every visit; ",
            "visitCount is the TOTAL visits in the session across all domains, NOT visits to the ",
            "autoTitle term.\n",
        );
        let sessions_footer = concat!(
            "\nTo see the actual URLs inside a session, call intelligence_report with ",
            "report=\"session_detail\" and session_id=<the sessionId of any row above>.",
        );
        let model_text = match report {
            IntelReport::Sessions => {
                format!("{sessions_preamble}{report_text}{sessions_footer}")
            }
            _ => {
                format!("intelligence_report({}): result follows.\n{}", parsed.report, report_text)
            }
        };
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
#[cfg(test)]
const INTELLIGENCE_REPORT_MAX_BYTES: usize = 8_000;

/// Adds sibling LOCAL `firstVisitLocal`/`lastVisitLocal` strings to each element of a
/// session-or-trail JSON array (sessions and search-trail rows both carry `firstVisitMs`/`lastVisitMs`).
///
/// A no-op when `items` is not an array (a torn/empty report serializes to something else) — so the
/// enrichment is best-effort and never panics on an unexpected shape. The raw ms fields are KEPT.
fn enrich_session_like_local(items: &mut Value, zone: chrono_tz::Tz) {
    if let Some(array) = items.as_array_mut() {
        for item in array {
            enrich_first_last_visit_local(item, zone);
        }
    }
}

/// Adds sibling LOCAL `firstVisitLocal`/`lastVisitLocal` strings to ONE object carrying integer
/// `firstVisitMs`/`lastVisitMs` epoch-ms fields (a session summary or a trail summary).
///
/// Each LOCAL field is inserted only when its ms sibling is present and integer, so a partial object
/// is enriched as far as it can be without ever overwriting or fabricating a value. The raw ms fields
/// stay so a caller that wants the exact instant still has it.
fn enrich_first_last_visit_local(item: &mut Value, zone: chrono_tz::Tz) {
    if let Some(ms) = item.get("firstVisitMs").and_then(Value::as_i64) {
        item["firstVisitLocal"] = json!(format_local_ms(ms, zone));
    }
    if let Some(ms) = item.get("lastVisitMs").and_then(Value::as_i64) {
        item["lastVisitLocal"] = json!(format_local_ms(ms, zone));
    }
}

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

mod user_records;
pub use user_records::{ListAnnotationsTool, ListStarsTool};

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
mod tests;
