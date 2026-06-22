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
use super::search::search_history_internal;
use super::traits::LlmToolDef;
use crate::config::ProjectPaths;
use crate::models::{AiCitation, AiSearchRequest, AppConfig};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The result of executing one agent tool: text the model sees, plus structured citations.
///
/// `model_text` is the compact, row-id-bearing evidence threaded back into the conversation (never
/// a huge inlined result set — 02 §F bounded context); `citations` is the structured provenance the
/// harness journals + pins so an answer's evidence survives later compaction. Each citation carries
/// its `canonical_url` so WU-6 can star the cited page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolOutcome {
    /// Compact, model-facing summary of the tool result (bounded; ids + count, not full rows).
    pub model_text: String,
    /// Structured citations resolved from the result, each keyed by canonical_url for W-STAR.
    pub citations: Vec<AiCitation>,
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
        Ok(ToolOutcome { model_text, citations })
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
        };
        (dir, context)
    }

    #[test]
    fn registry_advertises_all_default_tools_in_stable_order() {
        let registry = ToolRegistry::with_default_search_tools();
        assert!(!registry.is_empty());
        let names: Vec<String> = registry.definitions().into_iter().map(|def| def.name).collect();
        // BTreeMap keeps the order deterministic (alphabetical), so the prompt is stable.
        assert_eq!(names, vec!["search_bm25", "search_history", "search_hybrid", "search_vector"]);
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
            next_cursor: None,
        };
        let empty_text = summarize_search_for_model("search_bm25", &empty);
        assert!(empty_text.contains("no matching history rows"));
        assert!(empty_text.contains("Note: nothing"));
    }
}
