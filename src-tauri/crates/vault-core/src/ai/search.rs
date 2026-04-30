//! Semantic search and assistant orchestration.
//!
//! ## Responsibilities
//! - execute semantic + lexical history search with explicit fallback behavior
//! - compose the assistant prompt, retrieval seed set, and `search_history` tool
//! - persist assistant run traces and citations
//! - keep semantic result ranking and lexical merge rules in one owner module
//!
//! ## Not responsible for
//! - provider validation/client wiring beyond calling shared helpers
//! - semantic index ledger bookkeeping or sidecar build orchestration
//! - Settings-facing AI status/read-model assembly
//!
//! ## Dependencies
//! - `super::control` for cooperative cancellation while retrieval or generation runs
//! - `super::provider` for embedding queries and LLM dispatch
//! - `super::indexing` for semantic staleness/readiness helper lookups
//!
//! ## Performance notes
//! - semantic recall reads at most the requested top-k and merges with bounded lexical
//!   results, so the assistant never materializes unbounded search candidate sets
//! - lexical fallback remains explicit instead of scanning stale SQLite semantic metadata

use super::*;

/// One semantic search hit ready to merge with lexical recall.
///
/// The merge stage only needs a compact score-bearing shape, not the full sidecar row.
#[cfg(test)]
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(super) struct StoredEmbedding {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub score: f32,
}

/// One semantic-sidecar lookup result together with any degradation notes.
///
/// Semantic search can legitimately fall back to lexical-only results. Keeping the notes
/// attached here prevents that honesty metadata from getting lost during score merging.
#[derive(Debug, Default)]
pub(super) struct SemanticMatchReport {
    pub notes: Vec<String>,
}

/// Shared context captured by the assistant's `search_history` tool.
///
/// Tool calls need enough state to rerun retrieval with the current unlock state, scope
/// defaults, and cooperative cancellation hook, without reopening higher-level assistant
/// orchestration code.
#[derive(Clone)]
pub(super) struct SearchContext {
    pub paths: ProjectPaths,
    pub config: AppConfig,
    pub database_key: Option<String>,
    pub embedding_provider: Option<AiProviderRuntime>,
    pub default_profile_id: Option<String>,
    pub default_domain: Option<String>,
    pub default_limit: u32,
    pub citations: Arc<Mutex<Vec<AiCitation>>>,
    pub run_control: Option<Arc<dyn AiRunControl>>,
}

/// JSON payload accepted by the assistant's `search_history` tool.
#[derive(Debug, Deserialize)]
pub(super) struct SearchHistoryArgs {
    pub query: String,
    pub profile_id: Option<String>,
    pub domain: Option<String>,
    pub limit: Option<u32>,
}

/// JSON payload returned by the assistant's `search_history` tool.
#[derive(Debug, Serialize)]
pub(super) struct SearchHistoryOutput {
    pub items: Vec<AiSearchEntry>,
}

/// Tool-layer error wrapper used by rig.rs dynamic tool calls.
#[derive(Debug, Error)]
#[error("{0}")]
pub(super) struct SearchToolError(String);

/// Tool wrapper that lets an assistant do follow-up evidence searches during one answer.
#[derive(Clone)]
pub(super) struct SearchHistoryTool {
    pub context: SearchContext,
}

/// Implements the assistant-facing `search_history` tool contract.
impl Tool for SearchHistoryTool {
    const NAME: &'static str = "search_history";
    type Error = SearchToolError;
    type Args = SearchHistoryArgs;
    type Output = SearchHistoryOutput;

    /// Describes the tool schema exposed to the LLM runtime.
    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search browser history by meaning, URL, title, profile, or domain and return the best matching visits.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to search for in the browser history archive."
                    },
                    "profile_id": {
                        "type": "string",
                        "description": "Optional browser profile identifier."
                    },
                    "domain": {
                        "type": "string",
                        "description": "Optional domain filter."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of visits to return."
                    }
                },
                "required": ["query"]
            }),
        }
    }

    /// Executes one tool-driven history search and appends the resulting citations.
    async fn call(&self, args: Self::Args) -> std::result::Result<Self::Output, Self::Error> {
        if let Some(control) = self.context.run_control.as_ref() {
            control
                .checkpoint("Assistant run was cancelled before an additional history search.")
                .map_err(|error| SearchToolError(error.to_string()))?;
        }
        let request = AiSearchRequest {
            query: args.query,
            profile_id: args.profile_id.or_else(|| self.context.default_profile_id.clone()),
            domain: args.domain.or_else(|| self.context.default_domain.clone()),
            limit: args.limit.or(Some(self.context.default_limit)),
            cursor: None,
        };
        let response = search_history_internal(
            &self.context.paths,
            &self.context.config,
            self.context.database_key.as_deref(),
            self.context.embedding_provider.as_ref(),
            &request,
        )
        .await
        .map_err(|error| SearchToolError(error.to_string()))?;
        if let Some(control) = self.context.run_control.as_ref() {
            control
                .checkpoint("Assistant run was cancelled after the latest history search.")
                .map_err(|error| SearchToolError(error.to_string()))?;
        }
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
            })
            .collect::<Vec<_>>();
        self.context.citations.lock().await.extend(citations);
        Ok(SearchHistoryOutput { items: response.items })
    }
}

/// Runs the semantic/keyword history search pipeline with explicit fallback behavior.
pub async fn semantic_search_history(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    search_history_internal(paths, config, key, provider, request).await
}

/// Answers one user question against archive history with evidence-backed citations.
pub async fn answer_history_question(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    llm_provider: &AiProviderRuntime,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &AiAssistantRequest,
) -> Result<AiAssistantResponse> {
    answer_history_question_with_control(
        paths,
        config,
        key,
        llm_provider,
        embedding_provider,
        request,
        None,
    )
    .await
}

/// Answers one assistant question while honoring optional cooperative stop checkpoints.
pub async fn answer_history_question_with_control(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    llm_provider: &AiProviderRuntime,
    embedding_provider: Option<&AiProviderRuntime>,
    request: &AiAssistantRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
) -> Result<AiAssistantResponse> {
    validate_provider(llm_provider, AiProviderPurpose::Llm)?;
    if !config.ai.enabled || !config.ai.assistant_enabled {
        anyhow::bail!("Enable AI analysis and the assistant in Settings before asking questions.")
    }
    let archive = open_archive_connection(paths, config, key)?;
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    let run_id = begin_ai_run(
        &archive,
        "assistant",
        "manual",
        json!({
            "providerId": llm_provider.config.id,
            "embeddingProviderId": embedding_provider
                .map(|provider| provider.config.id.clone())
                .unwrap_or_else(|| "lexical-fallback".to_string()),
            "questionLength": request.question.len(),
        }),
    )?;

    let result: Result<AiAssistantResponse> = async {
        let retrieval_request = AiSearchRequest {
            query: request.question.clone(),
            profile_id: request.profile_id.clone(),
            domain: request.domain.clone(),
            limit: Some(config.ai.retrieval_top_k.max(1)),
            cursor: None,
        };
        let run_control = run_control.as_ref();
        let search_response = await_with_ai_cancellation(
            run_control,
            "Assistant run was cancelled before retrieval finished.",
            search_history_internal(paths, config, key, embedding_provider, &retrieval_request),
        )
        .await?;
        let seeded_citations = search_response
            .items
            .iter()
            .map(|item| AiCitation {
                history_id: item.history_id,
                profile_id: item.profile_id.clone(),
                url: item.url.clone(),
                title: item.title.clone(),
                visited_at: item.visited_at.clone(),
                score: Some(item.score),
            })
            .collect::<Vec<_>>();
        let citations = Arc::new(Mutex::new(seeded_citations.clone()));
        let tool_context = SearchContext {
            paths: paths.clone(),
            config: config.clone(),
            database_key: key.map(ToOwned::to_owned),
            embedding_provider: embedding_provider.cloned(),
            default_profile_id: request.profile_id.clone(),
            default_domain: request.domain.clone(),
            default_limit: config.ai.retrieval_top_k.max(1),
            citations: Arc::clone(&citations),
            run_control: run_control.cloned(),
        };
        let tools: Vec<Box<dyn ToolDyn>> =
            vec![Box::new(SearchHistoryTool { context: tool_context })];
        let preamble = build_assistant_preamble(config, &search_response);
        let answer = await_with_ai_cancellation(
            run_control,
            "Assistant run was cancelled while waiting for the model response.",
            run_llm_agent(llm_provider, &preamble, tools, &request.question),
        )
        .await?;

        let mut final_citations = citations.lock().await.clone();
        final_citations.sort_by_key(|item| item.history_id);
        final_citations.dedup_by_key(|item| item.history_id);

        let embedding_provider_id = embedding_provider
            .map(|provider| provider.config.id.clone())
            .unwrap_or_else(|| "lexical-fallback".to_string());
        let final_answer = if final_citations.is_empty() {
            "I couldn't find enough matching history evidence to answer that confidently yet. Try narrowing the profile or domain, or rebuild the semantic index and ask again.".to_string()
        } else {
            answer
        };
        #[rustfmt::skip]
        record_assistant_run(
            &connection,
            run_id,
            request,
            &final_answer,
            &llm_provider.config.id,
            &embedding_provider_id,
            &final_citations,
            &search_response.notes,
        )?;

        Ok(AiAssistantResponse {
            state: if final_citations.is_empty() {
                "insufficient-evidence".to_string()
            } else {
                "completed".to_string()
            },
            answer: final_answer,
            job_id: None,
            run_id: Some(run_id),
            provider_id: llm_provider.config.id.clone(),
            embedding_provider_id,
            citations: final_citations,
            notes: search_response.notes,
        })
    }
    .await;

    match result {
        Ok(response) => {
            finalize_ai_run_success(
                &archive,
                run_id,
                json!({
                    "providerId": response.provider_id,
                    "embeddingProviderId": response.embedding_provider_id,
                    "citations": response.citations.len(),
                }),
            )?;
            Ok(response)
        }
        Err(error) => {
            finalize_ai_run_failure(
                &archive,
                run_id,
                &error.to_string(),
                json!({
                    "providerId": llm_provider.config.id,
                    "embeddingProviderId": embedding_provider
                        .map(|provider| provider.config.id.clone())
                        .unwrap_or_else(|| "lexical-fallback".to_string()),
                }),
            )?;
            Err(error)
        }
    }
}

/// Builds the assistant preamble from the retrieval seed set and system prompt.
pub(super) fn build_assistant_preamble(
    config: &AppConfig,
    search_response: &AiSearchResponse,
) -> String {
    let context = search_response
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "[{index}] {visited_at} | {profile_id} | {url}\nTitle: {title}\nMatch: {reason}\nScore: {score:.3}",
                index = index + 1,
                visited_at = item.visited_at,
                profile_id = item.profile_id,
                url = item.url,
                title = item.title.clone().unwrap_or_else(|| "(untitled)".to_string()),
                reason = item.match_reason,
                score = item.score
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "{system_prompt}\n\nYou are working inside PathKeep. Always ground answers in the history evidence below or by calling the search_history tool. Cite the visit date, profile, and URL you relied on. If the evidence is incomplete, say so.\n\nInitial evidence:\n{context}",
        system_prompt = config.ai.assistant_system_prompt,
        context = if context.is_empty() {
            "No indexed evidence was found. Use the search_history tool or explain that the archive has no matching records.".to_string()
        } else {
            context
        }
    )
}

/// Runs the lexical + semantic merge pipeline used by search and assistant retrieval.
pub(super) async fn search_history_internal(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let query = request.query.trim();
    if query.is_empty() {
        anyhow::bail!("Enter a question or search query first.")
    }

    fn parse_search_cursor(cursor: Option<&str>) -> usize {
        cursor.and_then(|value| value.parse::<usize>().ok()).unwrap_or(0)
    }

    let lexical = lexical_history_results(paths, config, key, request, query)?;
    let mut merged = HashMap::<i64, AiSearchEntry>::new();
    let limit = request.limit.unwrap_or(8).clamp(1, 50) as usize;

    for (index, item) in lexical.items.iter().take(limit).enumerate() {
        merged.insert(
            item.id,
            history_entry_to_search_entry(item, lexical_score(index, limit), "Lexical match"),
        );
    }

    let mut notes = Vec::new();
    let mut provider_id = "lexical-fallback".to_string();
    let mut model = "none".to_string();

    if let Some(provider) = provider {
        validate_provider(provider, AiProviderPurpose::Embedding)?;
        provider_id = provider.config.id.clone();
        model = provider.config.default_model.clone();
        let semantic = semantic_matches(paths, config, key, provider, request).await?;
        notes.extend(semantic.notes.clone());
        notes.push(
            "No indexed semantic matches were found; showing lexical results only.".to_string(),
        );
    } else {
        notes.push(
            "No embedding provider is selected, so results use lexical retrieval only.".to_string(),
        );
    }

    let mut items = merged.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then(left.visited_at.cmp(&right.visited_at))
    });
    let total = items.len();
    let offset = parse_search_cursor(request.cursor.as_deref()).min(total);
    let next_offset = (offset + limit).min(total);
    let next_cursor = (next_offset < total).then(|| next_offset.to_string());
    let items = items.into_iter().skip(offset).take(limit).collect();

    Ok(AiSearchResponse { total, provider_id, model, items, notes, next_cursor })
}

/// Queries the semantic sidecar and returns visible semantic matches plus staleness notes.
pub(super) async fn semantic_matches(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    _request: &AiSearchRequest,
) -> Result<SemanticMatchReport> {
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    let mut notes = Vec::new();
    let ledger =
        load_index_ledger(&connection, &provider.config.id, &provider.config.default_model)?;
    if let Some(reason) = semantic_index_staleness_reason(
        &connection,
        &provider.config.id,
        &provider.config.default_model,
        ledger.source_watermark,
        ledger.last_indexed_at.as_deref(),
    )? {
        notes.push(reason);
    }

    let sqlite_embedding_count =
        provider_embedding_count(&connection, &provider.config.id, &provider.config.default_model)?;
    if sqlite_embedding_count > 0 {
        notes.push(
            "The optional semantic sidecar is disabled in PathKeep v0.1.0, so PathKeep returned lexical matches only instead of relying on stale SQLite semantic metadata."
                .to_string(),
        );
    } else {
        notes.push(
            "Semantic search is coming in a future PathKeep release; showing lexical results only."
                .to_string(),
        );
    }
    Ok(SemanticMatchReport { notes })
}

/// Persists one assistant run trace after the final answer is known.
pub(super) fn record_assistant_run(
    connection: &Connection,
    run_id: i64,
    request: &AiAssistantRequest,
    answer: &str,
    llm_provider_id: &str,
    embedding_provider_id: &str,
    citations: &[AiCitation],
    notes: &[String],
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(
        INSERT_ASSISTANT_RUN_SQL,
        params![
            run_id,
            request.question,
            answer,
            llm_provider_id,
            embedding_provider_id,
            serde_json::to_string(citations)?,
            serde_json::to_string(notes)?,
            now_rfc3339()
        ],
    )?;
    Ok(())
}

/// Executes the lexical history query used as the fallback and merge baseline.
pub(super) fn lexical_history_results(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &AiSearchRequest,
    query: &str,
) -> Result<crate::models::HistoryQueryResponse> {
    list_history(
        paths,
        config,
        key,
        HistoryQuery {
            q: Some(query.to_string()),
            profile_id: request.profile_id.clone(),
            browser_kind: None,
            domain: request.domain.clone(),
            start_time_ms: None,
            end_time_ms: None,
            sort: Some("newest".to_string()),
            limit: Some(request.limit.unwrap_or(12).max(1)),
            page: None,
            cursor: None,
            regex_mode: Some(false),
        },
    )
}

/// Orders semantic matches from strongest to weakest score.
#[cfg(test)]
pub(super) fn sort_stored_embeddings_desc(
    left: &StoredEmbedding,
    right: &StoredEmbedding,
) -> Ordering {
    right.score.partial_cmp(&left.score).unwrap_or(Ordering::Equal)
}

/// Converts one lexical history row into the public AI search entry shape.
pub(super) fn history_entry_to_search_entry(
    item: &HistoryEntry,
    score: f32,
    reason: &str,
) -> AiSearchEntry {
    AiSearchEntry {
        history_id: item.id,
        profile_id: item.profile_id.clone(),
        url: item.url.clone(),
        title: item.title.clone(),
        domain: item.domain.clone(),
        visited_at: item.visited_at.clone(),
        score,
        match_reason: reason.to_string(),
    }
}

/// Computes the bounded lexical baseline score for one ranked lexical hit.
pub(super) fn lexical_score(index: usize, limit: usize) -> f32 {
    0.42 + ((limit.saturating_sub(index)) as f32 / limit.max(1) as f32) * 0.18
}

/// Computes the bounded lexical boost added to semantic hits during result merging.
#[cfg(test)]
pub(super) fn lexical_boost(index: usize, limit: usize) -> f32 {
    ((limit.saturating_sub(index)) as f32 / limit.max(1) as f32) * 0.08
}

/// Computes cosine similarity for deterministic embedding test helpers.
#[cfg(test)]
pub(super) fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let len = left.len().min(right.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut left_norm = 0.0f32;
    let mut right_norm = 0.0f32;
    for index in 0..len {
        dot += left[index] * right[index];
        left_norm += left[index] * left[index];
        right_norm += right[index] * right[index];
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

/// Checks whether a SQLite table exists in the current intelligence connection.
pub(super) fn sqlite_table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row: &Row<'_>| row.get(0),
    )?;
    Ok(count > 0)
}

/// Explains why the semantic index should be considered stale for the selected provider/model.
pub(super) fn semantic_index_staleness_reason(
    connection: &Connection,
    provider_id: &str,
    model: &str,
    source_watermark: i64,
    last_indexed_at: Option<&str>,
) -> Result<Option<String>> {
    if provider_embedding_count(connection, provider_id, model)? == 0 {
        return Ok(None);
    }

    let visible_watermark = current_source_watermark(connection)?;
    if source_watermark != 0 && visible_watermark != source_watermark {
        return Ok(Some(
            "The semantic index no longer matches the current archive visibility or import watermark. Run Build index so semantic retrieval includes recent imports and reflects reverted rows."
                .to_string(),
        ));
    }

    if let Some(last_indexed_at) = last_indexed_at {
        if sqlite_table_exists(connection, "visit_content_enrichments")? {
            let latest_enrichment: Option<String> = connection
                .query_row(
                    "SELECT fetched_at
                     FROM visit_content_enrichments
                     WHERE fetch_status = 'success'
                     ORDER BY fetched_at DESC
                     LIMIT 1",
                    [],
                    |row: &Row<'_>| row.get(0),
                )
                .optional()?;
            if latest_enrichment.as_deref().is_some_and(|value| value > last_indexed_at) {
                return Ok(Some(
                    "Readable-content enrichment changed after the last semantic build. Run Build index to refresh embeddings with the latest extracted text."
                        .to_string(),
                ));
            }
        }
    }

    Ok(None)
}
