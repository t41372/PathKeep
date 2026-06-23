//! MCP-facing worker surface.
//!
//! The MCP server is a localhost-only, explicitly enabled bridge for external
//! AI tools. It must respect the same visibility and App Lock boundaries as the
//! first-party UI, and every query still writes a dedicated `mcp_query` run so
//! the archive keeps an auditable trace.

use crate::{
    context::{
        ai_archive_connection, derive_ai_status, load_hydrated_config, load_unlocked_config,
        resolved_app_lock_status,
    },
    intelligence::search_ai_history,
    security::read_database_key_from_keyring,
};
use anyhow::Result;
#[cfg(not(any(test, coverage)))]
use rmcp::ServiceExt;
use rmcp::schemars;
use rmcp::{
    ServerHandler,
    handler::server::wrapper::{Json, Parameters},
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use vault_core::{AiCapability, AiIndexStatus, AiSearchRequest, ensure_ai_capability_enabled};

/// MCP search request shape exposed to external AI tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpSearchRequest {
    /// Natural-language or keyword query to resolve against PathKeep history.
    pub(crate) query: String,
    /// Optional profile scope.
    pub(crate) profile_id: Option<String>,
    /// Optional domain filter.
    pub(crate) domain: Option<String>,
    /// Optional result limit.
    pub(crate) limit: Option<u32>,
}

/// MCP search response returned to external AI tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpSearchResult {
    /// Total matching items in the returned search mode.
    pub(crate) total: usize,
    /// Provider id used for the current search mode.
    pub(crate) provider_id: String,
    /// Model name or fallback mode label used for the search.
    pub(crate) model: String,
    /// Visit-level evidence items.
    pub(crate) items: Vec<McpSearchItem>,
    /// Honesty notes describing fallback or degraded behavior.
    pub(crate) notes: Vec<String>,
}

/// One visit-level MCP evidence item.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpSearchItem {
    /// Canonical history row id.
    pub(crate) history_id: i64,
    /// Profile scope for the visit.
    pub(crate) profile_id: String,
    /// URL evidence.
    pub(crate) url: String,
    /// Optional page title.
    pub(crate) title: Option<String>,
    /// Domain used for quick source inspection.
    pub(crate) domain: String,
    /// ISO timestamp for the visit.
    pub(crate) visited_at: String,
    /// Score produced by the active recall mode.
    pub(crate) score: f32,
    /// Short explanation for why this row matched.
    pub(crate) match_reason: String,
}

/// One titled section of the machine-facing usage guide (skill).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpUsageGuideSection {
    /// Stable section id (e.g. `granularity-ladder`) for an agent to key off.
    pub(crate) id: String,
    /// Short human/agent-readable title.
    pub(crate) title: String,
    /// Ordered procedural points the agent should follow for this section.
    pub(crate) points: Vec<String>,
}

/// Structured, machine-facing usage guide (the JSON skill, W-AI-9 Sub-block C).
///
/// This is procedural knowledge served to an EXTERNAL agent connected through
/// the MCP server: it teaches *how* to query PathKeep effectively. The body is
/// English-only by design — it is consumed by an LLM, not rendered in the
/// PathKeep UI — and it is gated on the separate `skill_enabled` consent flag.
/// When the flag is off, `enabled` is false, `sections` is empty, and `notice`
/// carries an honest "disabled in Settings" message instead of the guide.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpUsageGuide {
    /// Whether the user has enabled the usage guide in Settings.
    pub(crate) enabled: bool,
    /// Guide version, so a consumer can detect content changes over time.
    pub(crate) version: u32,
    /// One-line summary of what this guide teaches (always present).
    pub(crate) summary: String,
    /// The procedural sections — empty when `enabled` is false.
    pub(crate) sections: Vec<McpUsageGuideSection>,
    /// Honesty note: a disabled-state explanation, or empty when enabled.
    pub(crate) notice: Option<String>,
}

/// Current usage-guide content version (bump when the procedural body changes).
const MCP_USAGE_GUIDE_VERSION: u32 = 1;

/// Builds the canonical, machine-facing usage guide for external agents.
///
/// Authored faithfully to what the MCP tools actually accept and return: the
/// only egress tools are `search-history` (which takes `query` + optional
/// `profileId` / `domain` / `limit`, capped per call, and auto-selects its
/// recall mode) and `archive-status`. The guide therefore never instructs the
/// agent to pass a `mode` argument that does not exist, and it cites the
/// `historyId` + `url` that the result rows actually carry — not invented
/// fields. When `enabled` is false we return the same shape with an honest
/// disabled notice and no procedural body, so a consumer always parses cleanly.
fn build_mcp_usage_guide(enabled: bool) -> McpUsageGuide {
    if !enabled {
        return McpUsageGuide {
            enabled: false,
            version: MCP_USAGE_GUIDE_VERSION,
            summary: "PathKeep usage guide for querying browser history.".to_string(),
            sections: Vec::new(),
            notice: Some(
                "The PathKeep usage guide is disabled in Settings. Use the search-history and archive-status tools directly; enable the usage guide in PathKeep Settings to receive the full querying playbook."
                    .to_string(),
            ),
        };
    }
    McpUsageGuide {
        enabled: true,
        version: MCP_USAGE_GUIDE_VERSION,
        summary: "How to query a PathKeep browser-history archive effectively, read-only, with cited evidence.".to_string(),
        sections: vec![
            McpUsageGuideSection {
                id: "granularity-ladder".to_string(),
                title: "Granularity ladder — ask for the coarsest level that answers the question".to_string(),
                points: vec![
                    "PathKeep models history at increasing levels of aggregation: raw visits → sessions → trails (navigation paths) → query families (related searches) → domains → daily rollups → insights. Start coarse and drill down only when you need the underlying evidence.".to_string(),
                    "For \"what / when did I look into X\" questions, a search-history call already returns the most relevant individual visits; do not enumerate the whole archive.".to_string(),
                    "For trends or summaries (\"how often\", \"which sites most\", \"what was I doing that week\"), prefer reasoning over the returned visits' domains and timestamps rather than requesting more rows — the archive can hold tens of millions of visits and is never meant to be paged in full.".to_string(),
                    "Drill down to specific visits (their historyId, url, title, visitedAt) only to ground a claim. Always keep the working set small.".to_string(),
                ],
            },
            McpUsageGuideSection {
                id: "search-mode".to_string(),
                title: "Search mode is chosen for you — phrase the query to match what is available".to_string(),
                points: vec![
                    "The search-history tool takes a single `query` (plus optional `profileId`, `domain`, and `limit`); there is no mode parameter. PathKeep picks the recall mode automatically: semantic / hybrid when the user has a semantic index built, otherwise lexical (BM25 keyword) recall.".to_string(),
                    "Read the response to learn which mode actually ran: `providerId` and `model` name the recall path, and `notes` states honestly when it fell back to lexical-only (e.g. no embedding provider configured).".to_string(),
                    "When you can tell only lexical recall is available (a fallback note, or a lexical providerId), prefer precise keywords, distinctive terms, and exact phrases over conversational sentences.".to_string(),
                    "When semantic / hybrid recall is available, natural-language intent (\"articles about retirement planning I read last spring\") works well; you do not need to guess keywords.".to_string(),
                    "Narrow with `domain` or `profileId` instead of post-filtering large result sets, and issue a few focused queries rather than one broad one.".to_string(),
                ],
            },
            McpUsageGuideSection {
                id: "citation-discipline".to_string(),
                title: "Cite real evidence — every claim must point to returned rows".to_string(),
                points: vec![
                    "Every result row carries a stable `historyId` (the canonical visit identifier in this archive) and a `url`. Cite the `historyId` as your evidence handle and quote the `url`, `title`, and `visitedAt` when you reference a visit.".to_string(),
                    "Use `url` for a stable, human-readable reference; use `historyId` when you need to refer back to the exact visit unambiguously. Do not invent identifiers or fields that are not in the response.".to_string(),
                    "If the results do not support a claim, say so rather than guessing — answer from the returned evidence only.".to_string(),
                    "`matchReason` and `score` explain why a row matched and how strongly; surface that reasoning instead of asserting relevance without support.".to_string(),
                ],
            },
            McpUsageGuideSection {
                id: "bounds".to_string(),
                title: "Bounds — read-only, capped, and respectful of the user's privacy".to_string(),
                points: vec![
                    "Every tool here is strictly read-only. There is no way to modify, delete, or export the archive through this surface, and there is no filesystem or network access.".to_string(),
                    "Each search-history call returns a bounded page; `limit` is capped by the server, so a single call cannot pull the whole archive. Compose a few targeted calls instead of trying to widen one.".to_string(),
                    "Queries only see currently-visible history: reverted or hidden visits never appear, and the server refuses to read while PathKeep is locked.".to_string(),
                    "Every call you make is recorded in the user's local audit log. Query the minimum needed to answer, and treat the user's history as private.".to_string(),
                    "Use archive-status to check whether the archive is initialized, unlocked, and whether semantic search is available before relying on a particular recall mode.".to_string(),
                ],
            },
        ],
        notice: None,
    }
}

/// Compact MCP status snapshot for integration diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct McpArchiveStatus {
    /// Whether the canonical archive exists.
    pub(crate) initialized: bool,
    /// Whether the archive is encrypted at rest.
    pub(crate) encrypted: bool,
    /// Whether the archive is currently readable for this session.
    pub(crate) unlocked: bool,
    /// Whether AI is globally enabled.
    pub(crate) ai_enabled: bool,
    /// Whether assistant features are enabled.
    pub(crate) assistant_enabled: bool,
    /// Whether semantic indexing is enabled in config.
    pub(crate) semantic_index_enabled: bool,
    /// Number of indexed rows currently reported by the AI status model.
    pub(crate) indexed_items: usize,
    /// Optional refusal or degraded-state warning.
    pub(crate) warning: Option<String>,
}

/// Small MCP server wrapper used for stdio serving and test-time direct calls.
#[derive(Debug, Clone)]
pub(crate) struct BrowserHistoryMcpServer {
    /// Optional archive key resolved from the keyring when the worker starts.
    pub(crate) database_key: Option<String>,
}

impl BrowserHistoryMcpServer {
    /// Creates the MCP server wrapper with the current database key snapshot.
    pub(crate) fn new(database_key: Option<String>) -> Self {
        Self { database_key }
    }
}

/// Persists one external MCP request as a dedicated `mcp_query` run.
///
/// This is the transparency contract for the outward data surface: every
/// external tool call (`search-history` and `archive-status` alike) lands one
/// auditable row in the unified archive ledger so the user can see *what* an
/// external assistant asked and *how much* came back. The recorded
/// `stats_json` carries only a bounded query summary — the SQLCipher key and
/// raw archive rows are never written here, and never leave the worker
/// process.
fn record_mcp_query_run(
    connection: &rusqlite::Connection,
    tool: &str,
    profile_scope: &[String],
    warnings: &[String],
    summary: serde_json::Value,
) -> Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO runs (
           run_type,
           trigger,
           started_at,
           finished_at,
           timezone,
           status,
           profile_scope_json,
           warnings_json,
           stats_json,
           due_only
         )
         VALUES ('mcp_query', 'external', ?1, ?2, 'UTC', 'success', ?3, ?4, ?5, 0)",
        rusqlite::params![
            now,
            now,
            serde_json::to_string(profile_scope)?,
            serde_json::to_string(warnings)?,
            serde_json::to_string(&{
                let mut stats = summary;
                if let serde_json::Value::Object(map) = &mut stats {
                    map.insert("tool".to_string(), serde_json::json!(tool));
                }
                stats
            })?,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

/// Runs one MCP search request against the normal worker search surface.
pub(crate) fn mcp_search_result(
    database_key: Option<&str>,
    request: McpSearchRequest,
) -> Result<McpSearchResult> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    // Re-check MCP consent on EVERY call (M-4), not just at server start: config is read fresh here,
    // so a user who turns OFF the MCP server in Settings while an external tool still holds the stdio
    // connection must stop being served. Refuses with the honest "Enable AI and the MCP server"
    // message rather than returning history.
    ensure_ai_capability_enabled(&config, AiCapability::Mcp)?;
    let search_request = AiSearchRequest {
        query: request.query.clone(),
        profile_id: request.profile_id.clone(),
        domain: request.domain.clone(),
        limit: request.limit,
        cursor: None,
        // The MCP face does not expose the `is:starred` facet yet (W-AI-9 carryover); unfiltered.
        starred_only: None,
    };
    let response = search_ai_history(database_key, &search_request)?;
    let connection = ai_archive_connection(&paths, &config, database_key)?;
    record_mcp_query_run(
        &connection,
        "search-history",
        &request.profile_id.as_ref().map(|profile_id| vec![profile_id.clone()]).unwrap_or_default(),
        &response.notes,
        serde_json::json!({
            "query": request.query,
            "profileId": request.profile_id,
            "domain": request.domain,
            "limit": request.limit,
            "providerId": response.provider_id,
            "model": response.model,
            "total": response.total,
        }),
    )?;
    Ok(McpSearchResult {
        total: response.total,
        provider_id: response.provider_id,
        model: response.model,
        items: response
            .items
            .into_iter()
            .map(|item| McpSearchItem {
                history_id: item.history_id,
                profile_id: item.profile_id,
                url: item.url,
                title: item.title,
                domain: item.domain,
                visited_at: item.visited_at,
                score: item.score,
                match_reason: item.match_reason,
            })
            .collect(),
        notes: response.notes,
    })
}

/// Builds the MCP-facing archive status snapshot.
pub(crate) fn mcp_archive_status_result(database_key: Option<&str>) -> Result<McpArchiveStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    // Re-check MCP consent on EVERY call (M-4): a status probe is still an external touch served by
    // the MCP face, so it must refuse once the user turns the MCP server off mid-session — even
    // though (unlike search) it otherwise degrades gracefully while locked.
    ensure_ai_capability_enabled(&config, AiCapability::Mcp)?;
    let lock = resolved_app_lock_status(&paths, &config)?;
    let archive_status = if lock.locked {
        vault_core::archive_status(&paths, &config, None).unwrap_or_default()
    } else {
        vault_core::archive_status(&paths, &config, database_key)?
    };
    let ai_status = if lock.locked {
        AiIndexStatus {
            enabled: config.ai.enabled,
            assistant_enabled: config.ai.assistant_enabled,
            mcp_enabled: config.ai.mcp_enabled,
            skill_enabled: config.ai.skill_enabled,
            state: "blocked".to_string(),
            warning: Some("PathKeep is currently locked.".to_string()),
            ..AiIndexStatus::default()
        }
    } else {
        derive_ai_status(&paths, &config, database_key)
    };
    // Audit the status probe too, so the user sees every external touch — not
    // just searches. We can only write when unlocked: a locked status reads
    // nothing from the encrypted archive and we hold no writable connection,
    // so there is no archive access to record.
    if !lock.locked {
        let connection = ai_archive_connection(&paths, &config, database_key)?;
        record_mcp_query_run(
            &connection,
            "archive-status",
            &[],
            &[],
            serde_json::json!({
                "initialized": archive_status.initialized,
                "unlocked": archive_status.unlocked,
                "indexedItems": ai_status.indexed_items,
            }),
        )?;
    }
    Ok(McpArchiveStatus {
        initialized: archive_status.initialized,
        encrypted: archive_status.encrypted,
        unlocked: archive_status.unlocked && !lock.locked,
        ai_enabled: ai_status.enabled,
        assistant_enabled: ai_status.assistant_enabled,
        semantic_index_enabled: config.ai.semantic_index_enabled,
        indexed_items: ai_status.indexed_items,
        warning: if lock.locked {
            Some("PathKeep is currently locked.".to_string())
        } else {
            ai_status.warning.or(archive_status.warning)
        },
    })
}

/// Builds the MCP-facing usage guide (skill), gated on `skill_enabled`.
///
/// The guide is procedural knowledge for an external agent and reads nothing
/// from the canonical archive, so it works while locked and needs no database
/// key. Gating is honest rather than structural: rmcp registers every `#[tool]`
/// statically, so instead of hiding the tool we always answer but return a
/// disabled notice (and no body) when the user has not turned the skill on in
/// Settings — a consumer always parses the same shape.
///
/// When the guide is enabled AND the session is unlocked we record one
/// `mcp_query` audit run (`tool = "usage-guide"`) so a skill fetch is as
/// auditable as any other external touch (Sub-block B completeness). The audit
/// write needs a writable connection, which we only hold while unlocked; a
/// disabled or locked fetch reads nothing and writes nothing.
pub(crate) fn mcp_usage_guide_result(database_key: Option<&str>) -> Result<McpUsageGuide> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    // Re-check MCP consent on EVERY call (M-4): when the user turns the MCP server off mid-session no
    // tool should serve, so this refuses before building the guide. The skill SUB-flag
    // (`skill_enabled`) is handled below by the existing graceful disabled-notice shape — i.e. with
    // MCP on, a guide fetch still requires the skill toggle to return a body, but with MCP off the
    // whole face is sealed.
    ensure_ai_capability_enabled(&config, AiCapability::Mcp)?;
    let guide = build_mcp_usage_guide(config.ai.skill_enabled);
    if guide.enabled && !resolved_app_lock_status(&paths, &config)?.locked && config.initialized {
        let connection = ai_archive_connection(&paths, &config, database_key)?;
        record_mcp_query_run(
            &connection,
            "usage-guide",
            &[],
            &[],
            serde_json::json!({
                "version": guide.version,
                "sections": guide.sections.len(),
            }),
        )?;
    }
    Ok(guide)
}

#[tool_router]
impl BrowserHistoryMcpServer {
    /// Exposes canonical PathKeep history search to MCP clients.
    #[tool(
        name = "search-history",
        description = "Search PathKeep for relevant visits, URLs, titles, profiles, or domains."
    )]
    pub(crate) async fn search_history(
        &self,
        Parameters(request): Parameters<McpSearchRequest>,
    ) -> Result<Json<McpSearchResult>, rmcp::ErrorData> {
        let response = mcp_search_result(self.database_key.as_deref(), request)
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        Ok(Json(response))
    }

    /// Reports whether PathKeep is initialized, unlocked, and AI-ready.
    #[tool(
        name = "archive-status",
        description = "Report whether PathKeep is initialized, unlocked, and AI-ready."
    )]
    pub(crate) async fn archive_status(&self) -> Result<Json<McpArchiveStatus>, rmcp::ErrorData> {
        let snapshot = mcp_archive_status_result(self.database_key.as_deref())
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        Ok(Json(snapshot))
    }

    /// Serves the read-only usage guide (skill) that teaches an external agent
    /// how to query PathKeep effectively. Gated on the `skill_enabled` consent
    /// flag: when off, the response carries an honest disabled notice and no
    /// procedural body instead of the guide.
    #[tool(
        name = "usage-guide",
        description = "Get PathKeep's read-only guide on how to query browser history effectively: granularity ladder, how search modes are selected, and how to cite evidence. Disabled by default; enable the usage guide in PathKeep Settings."
    )]
    pub(crate) async fn usage_guide(&self) -> Result<Json<McpUsageGuide>, rmcp::ErrorData> {
        let guide = mcp_usage_guide_result(self.database_key.as_deref())
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        Ok(Json(guide))
    }
}

#[tool_handler]
impl ServerHandler for BrowserHistoryMcpServer {}

/// Runs the stdio MCP server when the feature is explicitly enabled.
pub(crate) fn run_mcp_stdio_server() -> Result<()> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    if !config.ai.enabled || !config.ai.mcp_enabled {
        anyhow::bail!(
            "Enable AI and the MCP server in Settings before starting the MCP server worker."
        );
    }
    if resolved_app_lock_status(&paths, &config)?.locked {
        anyhow::bail!("Unlock PathKeep before starting the MCP server worker.");
    }

    let database_key = read_database_key_from_keyring()?;

    // Under test/coverage builds we exercise the gated entry point and the real
    // read-only helpers, but never bind stdio: `serve(...).waiting()` blocks
    // forever on the transport, which can't run inside a unit test. We call the
    // synchronous helpers directly (the same ones the tool handlers delegate to)
    // — they own their own runtime, so we must not wrap them in an outer
    // `block_on` or tokio refuses to nest runtimes.
    #[cfg(any(test, coverage))]
    {
        let _ = mcp_archive_status_result(database_key.as_deref());
        let _ = mcp_usage_guide_result(database_key.as_deref());
        let _ = mcp_search_result(
            database_key.as_deref(),
            McpSearchRequest {
                query: "coverage".to_string(),
                profile_id: None,
                domain: None,
                limit: Some(1),
            },
        );
        Ok(())
    }

    #[cfg(not(any(test, coverage)))]
    {
        crate::context::tokio_runtime()?.block_on(async move {
            let service = BrowserHistoryMcpServer::new(database_key)
                .serve(rmcp::transport::io::stdio())
                .await?;
            service.waiting().await?;
            anyhow::Ok(())
        })
    }
}
