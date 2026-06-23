//! Semantic search and assistant orchestration.
//!
//! ## Responsibilities
//! - execute hybrid lexical + semantic history search with explicit fallback behavior
//! - fuse the two ranked recall sets with Reciprocal Rank Fusion (RRF, W-AI-6) on a PAGE-STABLE key
//!   (canonical url, M-12) so a multi-visit page fuses into one dual-list row, and apply a BOUNDED,
//!   tunable starred boost so favorites rank higher without becoming a bookmark list (05 §10)
//! - constrain BOTH recall planes to starred pages for the `is:starred` facet (lexical post-filter +
//!   the semantic content_key allowlist seam)
//! - compose the assistant prompt, retrieval seed set, and `search_history` tool
//! - persist assistant run traces and citations
//! - keep semantic result ranking and the lexical↔semantic fusion rules in one owner module
//!
//! ## Not responsible for
//! - provider validation/client wiring beyond calling shared helpers
//! - semantic index ledger bookkeeping or sidecar build orchestration
//! - Settings-facing AI status/read-model assembly
//! - resolving the starred SET or its canonicalization (delegated to `crate::stars`)
//!
//! ## Dependencies
//! - `super::control` for cooperative cancellation while retrieval or generation runs
//! - `super::provider` for embedding queries and LLM dispatch
//! - `super::indexing` for semantic staleness/readiness helper lookups
//! - `crate::stars` for the starred matcher / starred-visit resolution behind the boost + facet
//!
//! ## Performance notes
//! - RRF fusion + the starred boost operate on the BOUNDED recall pools (top-k lexical + top-k'
//!   semantic), never the corpus, so the merge stays O(pool) at 14.4M — no full scan, no N+1
//! - the starred matcher is loaded once from the tiny `star` table and checked in-memory per result
//! - the `is:starred` facet's starred-VISIT resolution is bounded by the tiny star set (a forward
//!   `urls.id` seek + chunked `visits` IN-query in `crate::stars::starred_history_ids`), NOT a
//!   visits⋈urls scan; the lexical plane over-fetches a bounded pool so the starred post-filter still
//!   yields a full page (see [`lexical_history_results`])
//! - BOTH visit↔content joins are now BOUNDED by keyed binary-search sidecars (M-11, `.pkrev`/`.pkfwd`
//!   built alongside the planes): the ALWAYS-ON semantic hydration (result content_key → visits) is
//!   O(k'·log n) seeks ([`resolve_history_ids_for_content_keys`]), and the `is:starred` forward
//!   resolution (starred history_id → content_key) is O(starred·log n) seeks
//!   ([`resolve_content_keys_for_history_ids`], closing the prior XA-PERF-4 O(n) `.pkmap` stride). Both
//!   degrade to the authoritative `.pkmap` full scan only when the keyed sidecar is missing/stale (an
//!   older index, or a torn pair); the next index build re-projects it. No O(n) `.pkmap` stride is paid
//!   on the steady-state query path — see doc 05 §10
//! - lexical fallback remains explicit instead of scanning stale SQLite semantic metadata

use super::*;
use crate::visit_taxonomy::normalize_visit_url;

/// Lexical recall-pool expansion factor for the `is:starred` facet (Bug 2 / W-AI-6).
///
/// The caller post-filters lexical rows to the starred set, so fetching only the newest `limit` text
/// matches would drop older starred matches. We over-fetch by this factor (the same `× 8` precedent the
/// semantic plane's `recall_k` uses) so the post-filter still surfaces a full page. See
/// [`lexical_history_results`] for the bound + documented residual.
const LEXICAL_FACET_EXPANSION: u32 = 8;

/// Hard cap on the expanded lexical facet pool, matching `list_history`'s own `[1, 1000]` limit clamp so
/// the over-fetch never asks for more rows than `list_history` will return.
const LEXICAL_FACET_POOL_CAP: u32 = 1_000;

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
            starred_only: None,
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
                canonical_url: None,
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
            starred_only: None,
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
                canonical_url: None,
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

/// Runs the hybrid lexical + semantic search pipeline used by search and assistant retrieval (W-AI-6).
///
/// Reciprocal Rank Fusion (RRF, 05 §9.4): the lexical and semantic recall sets are each a RANKED list;
/// a result's fused score is `Σ_list weight_list / (rrf_k + rank_in_list)` (0-based rank). Fusion dedups
/// on a PAGE-STABLE key (the canonical url, M-12), not the per-visit id, so a frequently-visited page
/// whose several matching visits land in the lexical window fuses into ONE row (its most-recent visit)
/// rather than duplicating — the page in BOTH lists sums both contributions and reads "Lexical + semantic
/// match"; lexical-only reads "Lexical match"; semantic-only reads "Semantic match". RRF is deterministic,
/// model-free, and operates on the BOUNDED recall pools (never the corpus), so it is fast at 14.4M. After
/// fusion a BOUNDED, tunable
/// starred boost (05 §10) promotes favorites without letting them dominate. The `is:starred` facet
/// (W-AI-6) restricts BOTH recall sets to starred pages via the lexical post-filter + the semantic
/// allowlist seam. AI-off / no provider degrades to lexical-only (RRF over one list = the lexical order).
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

    let limit = request.limit.unwrap_or(8).clamp(1, 50) as usize;
    let facet_starred = request.starred_only.unwrap_or(false);

    // Load the starred set once (tiny by design). It powers BOTH the bounded boost (always, when any
    // page is starred) and the `is:starred` facet's lexical post-filter + semantic allowlist (when on).
    let starred = crate::stars::load_starred_matcher(paths, config, key)?;

    // The `is:starred` facet resolves starred VISITS → deduped content_keys so semantic recall can be
    // restricted to favorites (the allowlist seam). Resolved once, reused for the lexical post-filter.
    let starred_content_keys = if facet_starred {
        resolve_starred_content_keys(paths, config, key, provider, &starred)?
    } else {
        None
    };

    // LEXICAL ranked list. When the facet is on, drop non-starred rows so the lexical plane is
    // constrained too (today the lexical browse facet is FE-only; here it is enforced backend-side).
    // The facet also EXPANDS the lexical recall pool (mirroring the semantic plane's `recall_k`) so the
    // post-filter still yields a full page of starred matches rather than only the newest `limit` text
    // matches — see `lexical_history_results` for the boundedness + residual.
    let lexical = lexical_history_results(paths, config, key, request, query, facet_starred)?;
    let lexical_ranked: Vec<&HistoryEntry> = lexical
        .items
        .iter()
        .filter(|item| !facet_starred || starred.is_starred(&item.url))
        .take(limit)
        .collect();

    let mut notes = Vec::new();
    let mut provider_id = "lexical-fallback".to_string();
    let mut model = "none".to_string();
    let mut semantic_hits: Vec<AiSearchEntry> = Vec::new();

    if let Some(provider) = provider {
        validate_provider(provider, AiProviderPurpose::Embedding)?;
        provider_id = provider.config.id.clone();
        model = provider.config.default_model.clone();
        // The facet pushes the starred allowlist into the vector index (content_key post-filter).
        let semantic = semantic_matches(
            paths,
            config,
            key,
            provider,
            request,
            starred_content_keys.as_deref(),
        )
        .await?;
        notes.extend(semantic.notes);
        semantic_hits = semantic.hits;
    } else {
        notes.push(
            "No embedding provider is selected, so results use lexical retrieval only.".to_string(),
        );
    }

    // FUSE the two ranked lists with RRF, then apply the bounded starred boost.
    let fused = fuse_ranked_lists(&lexical_ranked, &semantic_hits, &config.ai);
    let mut items = apply_starred_boost(fused, &starred, config.ai.starred_boost);

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

/// One fused result mid-pipeline: the chosen entry plus its rank in each recall list (W-AI-6 RRF).
///
/// `lexical_rank` / `semantic_rank` are the 0-based positions in their respective ranked lists, or
/// `None` when the result was absent from that list. The RRF score and final `match_reason` are derived
/// from these before the starred boost runs.
struct FusedResult {
    entry: AiSearchEntry,
    lexical_rank: Option<usize>,
    semantic_rank: Option<usize>,
}

/// Returns the PAGE-STABLE fusion key for a result URL (M-12).
///
/// Fusion must dedup on PAGE identity, not the per-visit id, or a frequently-visited page whose two
/// newest matching visits both land in the lexical window produces TWO rows (one fused, one lexical-only)
/// and only the newest-visit row earns the RRF dual-list boost — defeating the "page in BOTH lists beats
/// single-list" guarantee. We key on the CANONICAL url (the same page-identity `crate::stars` keys by):
/// `normalize_visit_url` collapses tracking-param + host-casing variants, so every visit of one page maps
/// to one key, and a lexical visit fuses with the semantic representative of the same page even when their
/// raw urls differ. Unparseable urls fall back to the raw string (still stable per row, just not collapsed
/// — the honest "can't canonicalize" outcome).
fn fusion_page_key(url: &str) -> String {
    normalize_visit_url(url)
        .map(|normalized| normalized.canonical_url)
        .unwrap_or_else(|| url.to_string())
}

/// Fuses the lexical + semantic ranked lists into scored entries via Reciprocal Rank Fusion (W-AI-6).
///
/// Each result's score is `Σ_list weight_list / (rrf_k + rank)` over the lists it appears in (0-based
/// rank), so a page ranked high in BOTH lists beats one ranked high in only one — the core hybrid win,
/// deterministic and model-free. The entry shape prefers the SEMANTIC hydration (its representative
/// most-recent visit) when a page is in both, falling back to the lexical row; the `match_reason`
/// reflects which list(s) matched. Both lists are already bounded by `limit`, so this is O(pool), never
/// the corpus. The weights + `rrf_k` come from [`AiSettings`] (already clamped on config load).
///
/// Dedup is PAGE-STABLE (M-12): both lists key on [`fusion_page_key`] (canonical url), NOT the per-visit
/// id. So a page's multiple lexical visits collapse to ONE entry that takes the page's BEST lexical rank
/// (the lexical list is `sort=newest`, so the first occurrence is both the best-ranked AND the most-recent
/// visit — the surviving lexical-only shape), and that same entry fuses with the semantic representative
/// of the page, earning the dual-list boost in ONE row instead of duplicating the page.
fn fuse_ranked_lists(
    lexical_ranked: &[&HistoryEntry],
    semantic_hits: &[AiSearchEntry],
    settings: &crate::models::AiSettings,
) -> Vec<AiSearchEntry> {
    let rrf_k = settings.hybrid_rrf_k.max(1) as f32;
    let mut fused: HashMap<String, FusedResult> = HashMap::new();

    // Lexical contributions: seed each PAGE from its most-recent matching visit (the lexical list is
    // `sort=newest`, so the first row seen for a page is its newest visit AND its best rank).
    for (rank, item) in lexical_ranked.iter().enumerate() {
        let key = fusion_page_key(&item.url);
        let result = fused.entry(key).or_insert_with(|| FusedResult {
            entry: history_entry_to_search_entry(item, 0.0, "Lexical match"),
            lexical_rank: None,
            semantic_rank: None,
        });
        // First occurrence wins the rank: a page's later (older) visits never overwrite the best rank or
        // the most-recent-visit entry shape, so a frequently-visited page contributes ONE lexical row.
        result.lexical_rank.get_or_insert(rank);
    }

    // Semantic contributions: a page already present (dual match) ADOPTS the semantic hydration (its
    // representative visit + snippet-bearing entry); a semantic-only page joins fresh. Keying on the same
    // page-stable url is what lets a lexical visit and the semantic representative of the same page fuse.
    for (rank, hit) in semantic_hits.iter().enumerate() {
        let key = fusion_page_key(&hit.url);
        match fused.get_mut(&key) {
            Some(result) => {
                result.entry = hit.clone();
                result.semantic_rank.get_or_insert(rank);
            }
            None => {
                fused.insert(
                    key,
                    FusedResult {
                        entry: hit.clone(),
                        lexical_rank: None,
                        semantic_rank: Some(rank),
                    },
                );
            }
        }
    }

    // Score + label each fused result from its ranks.
    fused
        .into_values()
        .map(|mut result| {
            let mut score = 0.0f32;
            if let Some(rank) = result.lexical_rank {
                score += settings.lexical_weight / (rrf_k + rank as f32);
            }
            if let Some(rank) = result.semantic_rank {
                score += settings.semantic_weight / (rrf_k + rank as f32);
            }
            result.entry.score = score;
            result.entry.match_reason =
                fusion_reason(result.lexical_rank.is_some(), result.semantic_rank.is_some())
                    .to_string();
            result.entry
        })
        .collect()
}

/// Returns the honest match-reason label for a fused result given which lists matched (W-AI-6).
///
/// A page in both lists is the strongest signal ("Lexical + semantic match"); otherwise it names the
/// single list it came from. The all-false case is unreachable in `fuse_ranked_lists` (every fused id
/// has at least one rank), but is mapped to the lexical label as a total, panic-free default.
fn fusion_reason(has_lexical: bool, has_semantic: bool) -> &'static str {
    match (has_lexical, has_semantic) {
        (true, true) => "Lexical + semantic match",
        (false, true) => "Semantic match",
        _ => "Lexical match",
    }
}

/// Applies the BOUNDED, tunable starred boost to fused results, marking promoted favorites (W-AI-6).
///
/// 05 §10 boundedness: an UNbounded starred bias turns semantic search into a bookmark list. So the
/// boost is a CAPPED additive delta on the `[0, 1]`-normalized fusion score — `normalized + boost`,
/// where `boost <= MAX_STARRED_BOOST = 0.5` and the normalized top is `1.0`. A *relevant* starred page
/// (already near the top) is promoted; an *irrelevant* starred page (low normalized score) gains at most
/// `boost` and so can never leapfrog a strongly-relevant unstarred page near `1.0`. The boost is added
/// to the SAME normalized scale the un-boosted results are renormalized onto, so the ordering stays a
/// single comparable space. `boost == 0` (or nothing starred) is a no-op pass-through. Starred results
/// get a "(Starred)" suffix on their reason so the FE can show the favorite affordance without a new
/// field. Operates on the bounded fused pool — never the corpus.
fn apply_starred_boost(
    fused: Vec<AiSearchEntry>,
    starred: &crate::stars::StarredMatcher,
    boost: f32,
) -> Vec<AiSearchEntry> {
    // Normalize fusion scores onto [0, 1] so the additive boost has a stable, bounded meaning. An
    // empty pool or all-zero scores leave the (zero) scores untouched.
    let max_score = fused
        .iter()
        .map(|entry| entry.score)
        .fold(0.0f32, |acc, value| if value > acc { value } else { acc });
    let scale = if max_score > 0.0 { 1.0 / max_score } else { 1.0 };

    fused
        .into_iter()
        .map(|mut entry| {
            entry.score *= scale;
            // Skip the per-result canonicalization work when the boost is disabled or nothing is
            // starred — the common case stays free.
            if boost > 0.0 && !starred.is_empty() && starred.is_starred(&entry.url) {
                entry.score += boost;
                entry.match_reason = format!("{} (Starred)", entry.match_reason);
            }
            entry
        })
        .collect()
}

/// Whether the keyed reverse/forward sidecars are trustworthy for this provider's CURRENT planes.
///
/// The read-path staleness guard (M-11): the sidecars are stamped with the `.pkvec` fingerprint hash,
/// so they are usable iff both are present AND stamped for the live store's hash. A missing store
/// (nothing embedded) or a missing/stale/torn sidecar (an older index built before the sidecar
/// existed, or a half-written pair) makes the keyed path UNTRUSTED, and the caller falls back to the
/// authoritative `.pkmap` scan so results stay correct. `Ok(false)` whenever anything is off — the
/// keyed path is a pure optimization layered over the always-correct `.pkmap`.
fn reverse_sidecars_usable(paths: &ProjectPaths, provider: &AiProviderRuntime) -> Result<bool> {
    let store =
        VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model);
    let Some(header) = store.read_header()? else {
        return Ok(false); // No `.pkvec` source ⇒ no trustworthy sidecar to key off.
    };
    let sidecars =
        ReverseVisitMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    Ok(!sidecars.is_stale_against(&header.fingerprint_hash)?)
}

/// Resolves result content_keys → their visits via the keyed `.pkrev` sidecar, falling back to scan.
///
/// The bounded hydration join (M-11): when the keyed sidecars are usable this is O(k'·log n)
/// binary-search seeks over the few result content_keys; otherwise it degrades to the authoritative
/// `.pkmap` full scan (correct, just the old O(n) cost) so an older/torn index still serves the EXACT
/// same history_ids. Both paths return identical results — the sidecar is the SAME
/// `(content_key, history_id)` multiset as the `.pkmap`.
fn resolve_history_ids_for_content_keys(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    wanted: &std::collections::HashSet<u64>,
) -> Result<HashMap<u64, Vec<i64>>> {
    if reverse_sidecars_usable(paths, provider)? {
        let sidecars = ReverseVisitMap::for_provider(
            paths,
            &provider.config.id,
            &provider.config.default_model,
        );
        return sidecars.history_ids_for_content_keys(wanted);
    }
    let visit_map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    visit_map.history_ids_for_content_keys(wanted)
}

/// Resolves a bounded starred history_id set → content_keys via the keyed `.pkfwd` sidecar, else scan.
///
/// The bounded `is:starred` forward join (XA-PERF-4, M-11): when the keyed sidecars are usable this is
/// O(starred·log n) binary-search seeks; otherwise it degrades to the authoritative `.pkmap` full scan
/// so an older/torn index still serves the EXACT same content_key set.
fn resolve_content_keys_for_history_ids(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    wanted: &std::collections::HashSet<i64>,
) -> Result<std::collections::HashSet<u64>> {
    if reverse_sidecars_usable(paths, provider)? {
        let sidecars = ReverseVisitMap::for_provider(
            paths,
            &provider.config.id,
            &provider.config.default_model,
        );
        return sidecars.content_keys_for_history_ids(wanted);
    }
    let visit_map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    visit_map.content_keys_for_history_ids(wanted)
}

/// Resolves the starred content_key allowlist for the `is:starred` semantic facet (W-AI-6).
///
/// Maps starred URLs/domains → starred archive `visits.id`s (the bounded join, [`starred_history_ids`])
/// → deduped `content_key`s through the provider's `.pkmap` (the authoritative visit→content adjacency).
/// Returns `Some(keys)` — possibly EMPTY when nothing starred maps to a built vector — so the caller
/// passes an allowlist (an empty allowlist correctly yields no semantic hits, the honest "facet matched
/// nothing" outcome). Returns `None` only when there is no embedding provider (semantic recall is off
/// anyway, so the facet has nothing to constrain). The content_key is `hash(canonical_url + title +
/// enrichment)`, NOT derivable from the URL alone, so the `.pkmap` is the source of truth, not a re-hash.
fn resolve_starred_content_keys(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    starred: &crate::stars::StarredMatcher,
) -> Result<Option<Vec<u64>>> {
    let Some(provider) = provider else {
        return Ok(None);
    };
    let history_ids = crate::stars::starred_history_ids(paths, config, key, starred)?;
    // XA-PERF-4 (closed, M-11): resolve the BOUNDED starred history_id set → content_keys through the
    // keyed forward sidecar (`.pkfwd`, O(starred·log n) binary-search seeks) when present + fresh,
    // falling back to the authoritative `.pkmap` full scan only when the sidecar is missing/stale (an
    // older index, or a torn sidecar). The forward stride is no longer the always-paid O(n) pass; the
    // next index build re-projects the sidecar.
    let keys = resolve_content_keys_for_history_ids(paths, provider, &history_ids)?;
    Ok(Some(keys.into_iter().collect()))
}

/// Embeds the query, runs the flat two-stage vector index, and hydrates hits to representative visits.
///
/// The real semantic-retrieval path (W-AI-5, 05 §3): embed the query (f32, Query role) → the
/// [`FlatVectorIndex`] does binary Hamming recall → int8 rescore over the derived planes → returns
/// `(content_key, score)` → hydrate each unique page to its MOST-RECENT visible visit through the
/// `.pkmap` fan-out + one batched archive lookup (no N+1). Profile/domain facets are applied as a
/// post-hydration filter over an EXPANDED top-k so a tight facet still surfaces enough true matches.
///
/// `starred_content_keys` is the `is:starred` facet's allowlist (W-AI-6): when `Some`, semantic recall
/// is restricted to those deduped content vectors via the index's content_key post-filter (the seam
/// W-AI-5 left). `None` is unconstrained; an empty allowlist honestly returns no hits (nothing starred
/// is indexed) rather than ignoring the facet.
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
    starred_content_keys: Option<&[u64]>,
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

    // Profile/domain facets are post-hydration (per-visit, not per-content); the `is:starred` facet is
    // a CONTENT-key allowlist pushed INTO the index (W-AI-6 — the seam W-AI-5 left). Recall an expanded
    // top-k when EITHER kind of facet is present so the post-filter / allowlist still yields the limit.
    let limit = request.limit.unwrap_or(8).clamp(1, 50) as usize;
    let has_visit_facet = request.profile_id.is_some() || request.domain.is_some();
    let recall_k = if has_visit_facet || starred_content_keys.is_some() {
        limit.saturating_mul(8).max(limit)
    } else {
        limit
    };
    // The `is:starred` facet restricts SEMANTIC recall to starred pages via the index's content_key
    // allowlist post-filter (the seam from W-AI-5). An EMPTY allowlist correctly returns no semantic
    // hits — the honest "nothing starred is indexed" outcome — rather than silently ignoring the facet.
    // Without the facet the allowlist is `None` (unconstrained). The empty index is handled above.
    let matches = index.search(&query_vector, recall_k, starred_content_keys)?;
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
    // `false`: this path only needs the engine DESCRIPTOR for the fingerprint-drift comparison, and
    // the forward-pass device (CPU vs Metal) does not affect the descriptor/fingerprint (W-AI-9-D).
    let engine = super::embedding_candle::select_embedding_provider(paths, provider, false)?;
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

    // M-11: resolve each result content_key to its visits through the keyed reverse sidecar (`.pkrev`,
    // O(k'·log n) binary-search seeks) when it is present + fresh; otherwise fall back to the
    // authoritative `.pkmap` full scan so an older index (built before the sidecar existed) or a torn
    // sidecar still serves CORRECT results — the next index build re-projects the sidecar. The query
    // thread never triggers a heavy rebuild (Principle 3: no main-thread freeze); it just degrades to
    // the correct-but-slower scan for that one query.
    let inverse = resolve_history_ids_for_content_keys(paths, provider, &wanted)?;
    // Flatten to the candidate history_ids and remember which content_key each came from. `inverse`
    // only contains keys that have at least one mapped visit, so every history_id below has a key.
    let mut key_by_history: HashMap<i64, u64> = HashMap::new();
    for (content_key, history_ids) in &inverse {
        for history_id in history_ids {
            key_by_history.insert(*history_id, *content_key);
        }
    }
    let candidate_ids: Vec<i64> = key_by_history.keys().copied().collect();

    // Attach the derived `search` plane so the hydration JOIN can read each page's enrichment excerpt
    // (REACH-C3). The intelligence connection attaches only `archive` by default; the lexical path gets
    // `search` for free because `open_archive_connection` attaches it, so we mirror that ATTACH here for
    // the AI-search connection. Idempotent for this code path: `hydrate_semantic_hits` runs once per
    // freshly-opened connection, so `search` is never already attached.
    attach_search_database(connection, paths)?;

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
        // The LEFT JOIN onto `search.search_documents` (keyed on the indexed `url_id` PK, for the
        // already-bounded candidate set — no N+1, no scan) hydrates each row's enrichment summary so the
        // semantic/hybrid path can show the SAME honest excerpt the lexical reader does (REACH-C3).
        // Non-enriched pages have an empty/absent `enrichment_text`, which `cap_enrichment_excerpt`
        // collapses to `None`, so the FE affordance stays suppressed for the vast majority of rows.
        // Requires `search` to be ATTACHed on this connection — `hydrate_semantic_hits` attaches it
        // before calling here (the intelligence connection attaches only `archive` by default).
        let sql = format!(
            "SELECT visits.id,
                    source_profiles.profile_key,
                    urls.url,
                    urls.title,
                    visits.visit_time_ms,
                    (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time,
                    search_documents.enrichment_text
             FROM archive.visits AS visits
             JOIN archive.urls AS urls ON urls.id = visits.url_id
             JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
             LEFT JOIN search.search_documents AS search_documents ON search_documents.url_id = urls.id
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
                // ONE cap implementation, reused from the lexical reader (CJK-safe, ≤180 chars):
                // empty/whitespace text yields `None`.
                enrichment_excerpt: row
                    .get::<_, Option<String>>(6)?
                    .as_deref()
                    .and_then(cap_enrichment_excerpt),
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
///
/// `starred_only` is the `is:starred` facet flag. The caller post-filters the lexical rows to the
/// starred set, so fetching only the newest `limit` text matches would under-recall: a starred page that
/// matches the query text but was visited OLDER than the newest ~`limit` matches would be silently
/// dropped before the filter ever saw it. To fix that we EXPAND the lexical recall pool when the facet is
/// on (mirroring the semantic plane's `recall_k = limit * 8`), so the post-filter has enough query-
/// matching candidates to surface a full page of starred results. The pool is clamped to `list_history`'s
/// `[1, 1000]` bound. RESIDUAL (documented, accepted for now): a starred match older than the newest
/// `limit * 8` text matches can still be missed — pushing a `url_id IN (...)` predicate INTO
/// `list_history` (its frozen multi-path FTS/regex/SQL contract) is too invasive for this fix; the
/// expanded pool covers the realistic case (a handful of starred pages within a generous recency window)
/// and degrades honestly rather than hard-truncating to the newest `limit`.
pub(super) fn lexical_history_results(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &AiSearchRequest,
    query: &str,
    starred_only: bool,
) -> Result<crate::models::HistoryQueryResponse> {
    let base_limit = request.limit.unwrap_or(12).max(1);
    // Facet on → fetch an expanded pool so the caller's starred post-filter still yields a full page;
    // facet off → the lexical plane is the merge baseline and `base_limit` is enough.
    let fetch_limit = if starred_only {
        (base_limit.saturating_mul(LEXICAL_FACET_EXPANSION)).min(LEXICAL_FACET_POOL_CAP)
    } else {
        base_limit
    };
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
            limit: Some(fetch_limit),
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
        // The honest snippet (REACH-C3): carry whatever capped excerpt the hydration attached. The
        // lexical reader sets this for keyword hits; `load_visit_rows` sets it for semantic/hybrid hits;
        // every other hydration leaves it `None`, so non-enriched pages get no snippet (the band +
        // reason carry the "why").
        enrichment_excerpt: item.enrichment_excerpt.clone(),
    }
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
