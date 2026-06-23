//! PathKeep-owned agent tool registry (plain Rust, NOT rig's `Tool`).
//!
//! ## Responsibilities
//! - declare the `AgentTool` boundary trait the W-AI-7 harness dispatches against, plus the
//!   `ToolOutcome` it returns (model-facing text + structured citations) and the `ToolRegistry`
//!   (name → tool) that builds the `LlmChatRequest.tools` definitions
//! - own the retrieval tools that wrap the W-AI-5/6 hybrid pipeline (`search_history_internal`):
//!   `search_history` plus the three escalation planes `search_bm25` / `search_vector` /
//!   `search_hybrid`, each reusing the SAME internal with a different recall plane
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
    query: String,
    profile_id: Option<String>,
    domain: Option<String>,
    limit: Option<u32>,
    /// The `is:starred` facet (W-AI-6); restricts recall to starred pages on both planes.
    #[serde(default)]
    starred_only: Option<bool>,
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
    /// The default full-hybrid `search_history` tool (lexical + semantic RRF + `is:starred`).
    pub fn search_history() -> Self {
        Self {
            name: "search_history",
            description: "Search browser history by meaning, URL, title, profile, or domain (hybrid lexical + semantic) and return the best matching visits with their ids.",
            plane: SearchPlane::Hybrid,
        }
    }

    /// The lexical-only `search_bm25` escalation start (works with NO embedding provider).
    pub fn search_bm25() -> Self {
        Self {
            name: "search_bm25",
            description: "Keyword/lexical (BM25) search over browser history. Works without any embedding provider; use it first or when meaning-based recall is unavailable.",
            plane: SearchPlane::Bm25,
        }
    }

    /// The semantic `search_vector` escalation (requires an embedding provider; else degrades).
    pub fn search_vector() -> Self {
        Self {
            name: "search_vector",
            description: "Meaning-based (vector) search over browser history. Requires an embedding provider; degrades to lexical results with a note when none is configured.",
            plane: SearchPlane::Vector,
        }
    }

    /// The combined `search_hybrid` escalation (lexical + semantic fused).
    pub fn search_hybrid() -> Self {
        Self {
            name: "search_hybrid",
            description: "Hybrid search combining keyword and meaning-based recall (RRF) over browser history. Best general-purpose retrieval when an embedding provider is configured.",
            plane: SearchPlane::Hybrid,
        }
    }

    /// Shared JSON-schema parameter object for the search tools.
    fn parameters() -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "What to search for in the browser history archive." },
                "profile_id": { "type": "string", "description": "Optional browser profile identifier filter." },
                "domain": { "type": "string", "description": "Optional domain filter." },
                "limit": { "type": "integer", "description": "Maximum number of visits to return." },
                "starred_only": { "type": "boolean", "description": "Restrict to starred (favorited) pages only." }
            },
            "required": ["query"]
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
        if parsed.query.trim().is_empty() {
            anyhow::bail!("the `query` argument must not be empty");
        }
        let request = AiSearchRequest {
            query: parsed.query,
            profile_id: parsed.profile_id.or_else(|| context.default_profile_id.clone()),
            domain: parsed.domain.or_else(|| context.default_domain.clone()),
            limit: parsed.limit.or(Some(context.default_limit)),
            cursor: None,
            starred_only: parsed.starred_only,
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
        let mut text = format!("{tool_name}: no matching history rows.");
        for note in &response.notes {
            text.push_str("\nNote: ");
            text.push_str(note);
        }
        return text;
    }
    let mut text = format!(
        "{tool_name}: {} match(es) (provider: {}).",
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
const RUN_CODE_DESCRIPTION: &str = "Run a short JavaScript program in a locked-down sandbox over the user's browser history when a question needs computation/aggregation across many visits (counts, grouping, joins, dedup) that a single search cannot express. The program is READ-ONLY: there is NO network, NO filesystem, NO real clock (Date.now() is 0), and NO randomness; it is bounded by hard time/memory/host-call/output limits, so loop and aggregate freely. Two globals are available, both synchronous and returning { rows: [...], notes: [...] }: query_history({ query: string (required), plane?: \"hybrid\"|\"vector\"|\"bm25\", limit?: number, profileId?: string, domain?: string, starredOnly?: boolean }) runs the same hybrid/lexical/semantic retrieval as the search tools (each row has id, url, title, domain, visitedAt, score, matchReason, canonicalUrl); fetch_visits(ids: number[]) resolves specific visit ids to rows. The `notes` array on each result holds honest warnings about retrieval scope or degradation (for example, semantic/hybrid search fell back to keyword-only because no embedding provider is configured) — read them and reflect any limitation in your answer; do not ignore them. Always end the program with `return <value>` to hand back a SMALL distilled result (a summary object/array, not the raw rows) — that returned value is the ONLY thing you will see, and it is truncated if too large. Even when there is no answer, return a small explicit result such as { found: false } rather than returning nothing.";

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

    /// Builds the full retrieval registry: `search_history` + the three escalation planes.
    ///
    /// `search_bm25` is always present (it needs no embedding provider), so a tool-capable model
    /// with no embedding configured can still escalate lexically — honest degradation, never an
    /// empty toolset.
    pub fn with_default_search_tools() -> Self {
        let mut registry = Self::new();
        registry.register(HistorySearchTool::search_history());
        registry.register(HistorySearchTool::search_bm25());
        registry.register(HistorySearchTool::search_vector());
        registry.register(HistorySearchTool::search_hybrid());
        // `run_code` is DEFAULT-ENABLED on every agent run (2026-06 decision): the Wasmtime sandbox
        // is the safety boundary, so there is no model-capability gate. The model gets it alongside
        // the search tools and chooses code-mode only when computation/aggregation needs it.
        registry.register(RunCodeTool::new());
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
            api_key: SecretString::from("k".to_string()),
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
        // BTreeMap keeps the order deterministic (alphabetical), so the prompt is stable. `run_code`
        // is registered by default (W-AI-8 WU-2), so it appears between `search_vector` lexically.
        assert_eq!(
            names,
            vec!["run_code", "search_bm25", "search_history", "search_hybrid", "search_vector"]
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
    async fn search_bm25_works_with_no_embedding_provider() {
        // The lexical plane must run with NO embedding provider configured (honest degradation).
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("search_bm25", json!({ "query": "tauri" }), &context)
            .await
            .expect("bm25 runs without an embedding provider");
        // No rows in an empty fixture archive, but the call succeeds and summarizes honestly.
        assert!(outcome.model_text.contains("search_bm25"));
    }

    #[tokio::test]
    async fn search_vector_degrades_when_no_embedding_provider() {
        // The semantic plane must NOT error without a provider — it degrades to lexical with a note.
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("search_vector", json!({ "query": "tauri" }), &context)
            .await
            .expect("vector degrades, never errors, without a provider");
        assert!(outcome.model_text.contains("search_vector"));
    }

    #[tokio::test]
    async fn search_hybrid_threads_the_embedding_provider() {
        // With a provider configured the hybrid plane runs the full pipeline; the empty fixture
        // archive yields no rows, but the call succeeds (no panic, no error).
        let (_dir, context) = context_with(Some(embedding_runtime()));
        let registry = ToolRegistry::with_default_search_tools();
        let outcome = registry
            .dispatch("search_hybrid", json!({ "query": "rust", "limit": 5 }), &context)
            .await
            .expect("hybrid runs with a provider");
        assert!(outcome.model_text.contains("search_hybrid"));
    }

    #[tokio::test]
    async fn empty_query_argument_is_rejected() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        let error = registry
            .dispatch("search_bm25", json!({ "query": "   " }), &context)
            .await
            .expect_err("empty query");
        assert!(error.to_string().contains("must not be empty"));
    }

    #[tokio::test]
    async fn invalid_arguments_are_rejected_without_panicking() {
        let (_dir, context) = context_with(None);
        let registry = ToolRegistry::with_default_search_tools();
        // `query` is required and must be a string; a number is a parse error, not a panic.
        let error = registry
            .dispatch("search_bm25", json!({ "query": 42 }), &context)
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
        };
        let empty_text = summarize_search_for_model("search_bm25", &empty);
        assert!(empty_text.contains("no matching history rows"));
        assert!(empty_text.contains("Note: nothing"));
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
}
