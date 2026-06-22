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
use vault_core::{AiIndexStatus, AiSearchRequest};

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
