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

/// One semantic-index lookup result together with any degradation notes.
///
/// Semantic search can legitimately fall back to lexical-only results. Keeping the notes
/// attached here prevents that honesty metadata from getting lost during score merging. `hits`
/// carries the hydrated semantic matches (one per unique page, most-recent visit) ready to merge
/// with lexical recall; it is empty when the index is absent/stale/empty (with a matching note).
#[derive(Debug, Default)]
pub(super) struct SemanticMatchReport {
    pub hits: Vec<AiSearchEntry>,
    pub notes: Vec<String>,
}

/// One hydrated semantic hit: a result content_key resolved to its representative visit + score.
///
/// The two-stage index returns `(content_key, score)`; hydration (the `.pkmap` fan-out + an archive
/// lookup) resolves each unique page to its MOST-RECENT visible visit so the UI shows one row per
/// page, not one per repeat visit. `score` is the int8-rescore cosine carried through verbatim.
struct SemanticHit {
    visit: HistoryEntry,
    score: f32,
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
        // Merge semantic hits into the lexical baseline. A page found by BOTH planes keeps the higher
        // score plus a combined reason; a semantic-only page joins as a fresh entry. Full hybrid
        // weighting / reranking is W-AI-6 — this is the basic max-merge that makes real semantic
        // recall visible without yet tuning the lexical↔semantic blend.
        for hit in semantic.hits {
            merge_semantic_hit(&mut merged, hit);
        }
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

/// Embeds the query, runs the flat two-stage vector index, and hydrates hits to representative visits.
///
/// The real semantic-retrieval path (W-AI-5, 05 §3): embed the query (f32, Query role) → the
/// [`FlatVectorIndex`] does binary Hamming recall → int8 rescore over the derived planes → returns
/// `(content_key, score)` → hydrate each unique page to its MOST-RECENT visible visit through the
/// `.pkmap` fan-out + one batched archive lookup (no N+1). Profile/domain facets are applied as a
/// post-hydration filter over an EXPANDED top-k so a tight facet still surfaces enough true matches.
///
/// Honest degradation, never a panic: a missing/empty index yields no hits with a clear note; a stale
/// ledger adds the staleness reason so the user knows to rebuild. The vectors live ONLY on the derived
/// planes — this never reads a vector from SQLite.
pub(super) async fn semantic_matches(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
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

    // Load the flat index over the derived planes. A never-built / empty index loads cleanly to zero
    // vectors (no panic); we report an honest note and return lexical-only.
    let index = FlatVectorIndex::open(paths, &provider.config.id, &provider.config.default_model)?;
    if index.is_empty() {
        notes.push(
            "The semantic index has no vectors yet; run Build index to enable meaning-based search. Showing lexical results only."
                .to_string(),
        );
        return Ok(SemanticMatchReport { hits: Vec::new(), notes });
    }

    // Embed the query under the Query role (asymmetric models encode queries differently).
    let query_vector = embed_query(provider, request.query.trim(), EmbeddingRole::Query).await?;

    // CONFIG-DRIFT GUARD (D1, 05 §10): the planes were binarized/int8-quantized at a STAMPED dim +
    // fingerprint. If the live embedding config changed, comparing a differently-shaped query against
    // them would silently score garbage (binary widths differ → prefix-only Hamming; or same width but
    // a different model/pooling/dtype → wrong geometry). Reject BOTH cases here, BEFORE searching, and
    // degrade to lexical-only with an honest note so no meaningless score reaches the result merge.
    if let Some(reason) = semantic_config_drift_reason(paths, provider, &index, query_vector.len())?
    {
        notes.push(reason);
        return Ok(SemanticMatchReport { hits: Vec::new(), notes });
    }

    // Facet filtering is post-hydration (profile/domain are per-visit, not per-content); recall an
    // expanded top-k when a facet is present so the filter still yields the requested limit.
    let limit = request.limit.unwrap_or(8).clamp(1, 50) as usize;
    let has_facet = request.profile_id.is_some() || request.domain.is_some();
    let recall_k = if has_facet { limit.saturating_mul(8).max(limit) } else { limit };
    // No content_key allowlist today (profile/domain are visit-level); the index's allowlist
    // post-filter is the seam W-AI-6 wires content-keyed facets (e.g. starred pages) into. A non-empty
    // index with k >= 1 always returns at least one match, so there is no separate empty-matches case
    // here (the empty index is handled above).
    let matches = index.search(&query_vector, recall_k, None)?;
    let hits = hydrate_semantic_hits(&connection, paths, provider, &matches, request, limit)?;
    if hits.is_empty() {
        notes.push(
            "Semantic matches were found but none are currently visible under the active filters."
                .to_string(),
        );
    }
    Ok(SemanticMatchReport {
        hits: hits.into_iter().map(semantic_hit_to_search_entry).collect(),
        notes,
    })
}

/// Returns an honest degradation note when the live embedding config no longer matches the planes.
///
/// The query-path counterpart to the build-time fingerprint stamp (D1, 05 §10). Two distinct drifts
/// would otherwise let a meaningless score reach the result merge:
/// - **dim change** (user-mutable `provider.config.dimensions`, MRL truncation): the query binarizes to
///   a DIFFERENT byte width than the stored bits, so `hamming_distance` would compare only the shared
///   prefix and the int8 rescore would dot mismatched lengths — pure noise. We detect it directly by
///   comparing the embedded query length to the loaded plane `dim`.
/// - **same-dim fingerprint drift** (pooling / normalization / instruction / dtype changed but the dim
///   held): the bytes line up but the GEOMETRY is different, so scores are still meaningless. We build
///   the LIVE fingerprint from the selected engine's descriptor (stamped exactly as the build path does
///   in `vector_store_for_chunk`: the engine's real dtype/pooling/instruction, keyed by the provider
///   config id/model, with the observed query dim) and ask [`planes_are_stale`] whether it matches the
///   plane's stamp.
///
/// Returns `Some(note)` (caller degrades to lexical-only) on either drift, `None` when the planes are
/// usable. An empty index is handled by the caller before this is reached, so a `None` here means a
/// real, dimension- and fingerprint-matched index ready to search.
fn semantic_config_drift_reason(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    index: &FlatVectorIndex,
    query_dim: usize,
) -> Result<Option<String>> {
    // Dim mismatch: the binarized query byte width differs from the plane's, so a search would
    // prefix-compare and score garbage. Reject loudly with a rebuild note.
    if query_dim != index.dim() {
        return Ok(Some(
            "The semantic index was built under a different embedding configuration (vector dimension changed), so meaning-based search is disabled until you run Build index. Showing lexical results only."
                .to_string(),
        ));
    }

    // Same-dim fingerprint drift: build the live fingerprint exactly as the build path stamps it and
    // ask whether the planes are stale against it. The selected engine carries the real
    // dtype/normalized/pooling/instruction (the build path uses these via `vector_store_for_chunk`);
    // we override the provider id/model + observed dim so the comparison keys match the stored stamp.
    let engine = super::embedding_candle::select_embedding_provider(paths, provider)?;
    let live = EmbeddingFingerprint::from_descriptor(&EmbeddingDescriptor {
        provider_id: provider.config.id.clone(),
        model_id: provider.config.default_model.clone(),
        effective_dim: Some(query_dim),
        ..engine.descriptor()
    })
    // `from_descriptor` only returns None when no dim is known; we just supplied the observed query
    // dim, so this is infallible — an unwrap rather than a dead else-branch keeps the invariant honest.
    .expect("live fingerprint always has the observed query dim");
    if planes_are_stale(paths, &provider.config.id, &provider.config.default_model, &live)? {
        return Ok(Some(
            "The semantic index was built under a different embedding configuration (model, pooling, normalization, or output type changed), so meaning-based search is disabled until you rebuild the semantic index. Showing lexical results only."
                .to_string(),
        ));
    }
    Ok(None)
}

/// Hydrates `(content_key, score)` index hits to one representative (most-recent) visit per page.
///
/// The dedup join (05 §1): a result content_key fans out to its visits via the `.pkmap`, and we pick
/// the most-recent VISIBLE visit so the UI shows one row per page. ONE batched archive lookup over the
/// fanned-out history_ids (chunked) keeps this O(candidates), never N+1 at 14.4M. Profile/domain facets
/// drop non-matching visits here (the post-hydration filter); the surviving pages are truncated to
/// `limit`, ranked by the carried semantic score (the index already ordered them).
fn hydrate_semantic_hits(
    connection: &Connection,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    matches: &[(u64, f32)],
    request: &AiSearchRequest,
    limit: usize,
) -> Result<Vec<SemanticHit>> {
    // content_key → score, preserving the index's ranking order.
    let mut score_by_key: HashMap<u64, f32> = HashMap::with_capacity(matches.len());
    let wanted: std::collections::HashSet<u64> = matches
        .iter()
        .map(|(key, score)| {
            score_by_key.insert(*key, *score);
            *key
        })
        .collect();

    let visit_map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    let inverse = visit_map.history_ids_for_content_keys(&wanted)?;
    // Flatten to the candidate history_ids and remember which content_key each came from. `inverse`
    // only contains keys that have at least one mapped visit, so every history_id below has a key.
    let mut key_by_history: HashMap<i64, u64> = HashMap::new();
    for (content_key, history_ids) in &inverse {
        for history_id in history_ids {
            key_by_history.insert(*history_id, *content_key);
        }
    }
    let candidate_ids: Vec<i64> = key_by_history.keys().copied().collect();

    // Batch-load the candidate visit rows (skips reverted), then per content_key keep the most-recent
    // VISIBLE visit that passes the facet filter. Every returned row.id is in `key_by_history` (the
    // SQL only queried those ids), so the lookup is total.
    let rows = load_visit_rows(connection, &candidate_ids)?;
    let mut best: HashMap<u64, HistoryEntry> = HashMap::new();
    for row in rows {
        if !visit_passes_facets(&row, request) {
            continue;
        }
        let content_key = key_by_history[&row.id];
        match best.get(&content_key) {
            Some(existing) if existing.visit_time >= row.visit_time => {}
            _ => {
                best.insert(content_key, row);
            }
        }
    }

    // Order the surviving pages by their semantic score (desc), then content_key (asc) for a stable
    // tie-break, and cap at the caller's limit.
    let mut hits: Vec<(u64, SemanticHit)> = best
        .into_iter()
        .map(|(content_key, visit)| {
            let score = score_by_key.get(&content_key).copied().unwrap_or(0.0);
            (content_key, SemanticHit { visit, score })
        })
        .collect();
    hits.sort_by(|left, right| {
        right
            .1
            .score
            .partial_cmp(&left.1.score)
            .unwrap_or(Ordering::Equal)
            .then(left.0.cmp(&right.0))
    });
    hits.truncate(limit);
    Ok(hits.into_iter().map(|(_, hit)| hit).collect())
}

/// Loads visit detail rows for a bounded set of history_ids, chunked, skipping reverted visits.
///
/// One statement per `SQLITE_BATCH_SIZE` chunk so the predicate list stays bounded at 14.4M — the
/// batched hydration that replaces a per-result N+1. Reverted visits are excluded so a result never
/// resolves to a row the user reverted.
fn load_visit_rows(connection: &Connection, history_ids: &[i64]) -> Result<Vec<HistoryEntry>> {
    let mut rows = Vec::new();
    for chunk in history_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT visits.id,
                    source_profiles.profile_key,
                    urls.url,
                    urls.title,
                    visits.visit_time_ms,
                    (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
             WHERE visits.reverted_at IS NULL
               AND visits.id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql)?;
        let params = rusqlite::params_from_iter(chunk.iter());
        let mapped = statement.query_map(params, |row: &Row<'_>| {
            let url: String = row.get(2)?;
            Ok(HistoryEntry {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                url: url.clone(),
                title: row.get(3)?,
                domain: url_domain(&url),
                favicon: None,
                visited_at: crate::utils::chrome_time_to_rfc3339(row.get::<_, i64>(5)?),
                visit_time: row.get(5)?,
                duration_ms: None,
                transition: None,
                source_visit_id: row.get(0)?,
                app_id: None,
                enrichment_excerpt: None,
            })
        })?;
        for entry in mapped {
            rows.push(entry?);
        }
    }
    Ok(rows)
}

/// Returns whether one hydrated visit passes the request's profile/domain facet filters.
///
/// Pure so the post-hydration facet predicate is unit-tested directly. An absent facet matches
/// everything; a present facet matches exactly (profile by id, domain by host) so the semantic
/// allowlist intent is honored even though it is applied at hydration rather than in the index.
fn visit_passes_facets(visit: &HistoryEntry, request: &AiSearchRequest) -> bool {
    if let Some(profile_id) = request.profile_id.as_deref() {
        if visit.profile_id != profile_id {
            return false;
        }
    }
    if let Some(domain) = request.domain.as_deref() {
        if visit.domain != domain {
            return false;
        }
    }
    true
}

/// Converts one hydrated semantic hit into the public AI search entry shape with an honest reason.
fn semantic_hit_to_search_entry(hit: SemanticHit) -> AiSearchEntry {
    history_entry_to_search_entry(&hit.visit, hit.score, "Semantic match")
}

/// Merges one semantic hit into the lexical baseline map (basic max-merge; full hybrid is W-AI-6).
///
/// A page surfaced by BOTH planes keeps the higher score and a combined reason so the user sees it
/// was a strong dual match; a semantic-only page joins as a fresh entry. Keyed by history_id like the
/// lexical entries so the two recall sets reconcile on the representative visit.
pub(super) fn merge_semantic_hit(merged: &mut HashMap<i64, AiSearchEntry>, hit: AiSearchEntry) {
    match merged.get_mut(&hit.history_id) {
        Some(existing) => {
            if hit.score > existing.score {
                existing.score = hit.score;
            }
            existing.match_reason = "Lexical + semantic match".to_string();
        }
        None => {
            merged.insert(hit.history_id, hit);
        }
    }
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
