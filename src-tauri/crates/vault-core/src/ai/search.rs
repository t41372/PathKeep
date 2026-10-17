//! Responsibilities: expose optional-AI search and report use of a previous completed model.
//! Not responsible for: result windows, database ownership or queues.
//! Failure modes: lexical fallback loses coverage; provider drift is hidden; tool pagination
//! is capped by its output page size. Tests: AI, agent-tool, code-mode and MCP suites.

use super::*;

/// The 50-row cap a page of search results has on the paths a person or a model reads
/// ([`semantic_search_history`]). Distinct from the paged retrieval API (the retrieval ceiling the
/// sandbox uses); see that doc for why the two paths differ. Mirrors `agent_tools::MAX_TOOL_ROWS`.
pub(super) const MODEL_FACING_ROW_CAP: u32 = 50;

/// One semantic-index lookup result together with any degradation notes.
///
/// Semantic search can legitimately fall back to lexical-only results. Keeping the notes
/// attached here prevents that honesty metadata from getting lost during score merging. `hits`
/// carries the hydrated semantic matches (one per unique page, most-recent visit) ready to merge
/// with lexical recall; it is empty when the index is absent/stale/empty (with a matching note).
///
/// Each note is carried as a stable [`AiSearchNote`] CODE, NOT English prose, so the
/// front end can localize it; the English sentence is derived from the code only on the MODEL-facing
/// path ([`ai_search_note_text`]).
#[derive(Debug, Default)]
#[cfg(test)]
pub(super) struct SemanticMatchReport {
    pub hits: Vec<AiSearchEntry>,
    pub notes: Vec<AiSearchNote>,
}

/// Runs the semantic/keyword history search pipeline with explicit fallback behavior.
///
/// This is the FE / worker entry (`search_ai_history`). It caps `limit` at [`MODEL_FACING_ROW_CAP`]:
/// the shared `search_history_internal` ceiling was raised to the paged retrieval API so the `run_code`
/// SANDBOX (which calls `search_history_internal` DIRECTLY, bypassing this wrapper) can fetch large
/// pages and aggregate them in-sandbox — but a model/UI-facing caller must NOT inherit that. Pre-change
/// this path was hard-capped at 50, and a search UI never needs a 1000-row page; it paginates instead.
pub async fn semantic_search_history(
    vault: &crate::Vault,
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    let mut bounded = request.clone();
    if let Some(limit) = bounded.limit {
        bounded.limit = Some(limit.min(MODEL_FACING_ROW_CAP));
    }
    search_history_internal(vault, paths, config, key, provider, &bounded).await
}

/// Fuses complete lexical and semantic rankings over canonical pages. Each list
/// contributes weight / (rrf_k + rank), with a configurable star boost. Facets
/// constrain canonical membership before rank assignment; cursors enumerate the
/// same pinned engine reader used for the first page.
pub(super) async fn search_history_internal(
    vault: &crate::Vault,
    paths: &ProjectPaths,
    config: &AppConfig,
    _key: Option<&str>,
    provider: Option<&AiProviderRuntime>,
    request: &AiSearchRequest,
) -> Result<AiSearchResponse> {
    if request.query.trim().is_empty()
        || matches!(request.sort.as_deref(), Some("oldest" | "newest"))
        || (provider.is_none() && crate::stars::load_starred_matcher(vault)?.is_empty())
    {
        super::search_pages::lexical_page(vault, request)
    } else {
        super::search_ranking::page(vault, paths, config, provider, request).await
    }
}

/// Stable, locale-independent match-reason CODE for browse-by-recency rows (blank-query path).
///
/// See the [module match-reason contract](#match-reason-codes) below: this tells BOTH audiences that
/// the row is one of the newest visits and carries no relevance ranking at all — the model's entry
/// point for date-range questions ("last Friday"), and the FE's "Most recent visit" caption.
pub(super) const MATCH_REASON_RECENT_VISIT: &str = "recent-visit";

/// Match-reason CODE for a page only the lexical (keyword/FTS) plane matched.
pub(super) const MATCH_REASON_LEXICAL: &str = "lexical";

/// Match-reason CODE for lexical matches enumerated in pure visit-date order.
///
/// Distinct from [`MATCH_REASON_LEXICAL`] because the ordering is chronological, not relevance: it
/// tells the model these rows are an EARLIEST-/latest-first enumeration of the keyword's matches (the
/// entry point for "when did I first browse X?") so it never reads them as a ranked top-K.
pub(super) const MATCH_REASON_LEXICAL_DATE_ORDERED: &str = "lexical-date-ordered";

/// Suffix appended to a base match-reason code when the starred boost promoted the row.
///
/// The composed value is `{base}+starred` (e.g. `lexical+semantic+starred`), so the base vocabulary
/// stays a single closed set and the favorite affordance needs no extra wire field.
pub(super) const MATCH_REASON_STARRED_SUFFIX: &str = "+starred";

// <a name="match-reason-codes"></a>
// ## `match_reason` is a stable CODE, not prose
//
// [`AiSearchEntry::match_reason`] carries one of the codes above — never an English sentence —
// because it has TWO audiences with opposite needs:
//
// - MODEL-facing: the agent-tool summaries ([`super::agent_tools`]), the `run_code` row shape
//   ([`super::code_mode`]), the assistant preamble in this module, and the MCP search item all quote
//   the value verbatim into the model's context. A short stable token is as legible to the model as
//   prose and cannot drift between call sites.
// - USER-facing: the Explorer Smart-search surface renders it as the mono caption under EVERY ranked
//   result (`paperSearchEntryFromAiSearchItem` → `PaperSearchResult`). While this field held English
//   prose, every zh-CN/zh-TW user read English on every row.
//
// The front end resolves each code through its i18n catalog (`aiMatchReason*` in the `explorer`
// namespace, `+starred` handled as a suffix) and falls back to printing an UNKNOWN code verbatim, so
// adding a code here degrades to a visible token rather than a silently wrong translation. Codes are
// therefore additive-only: renaming one is a breaking wire change that must land with the catalog.

/// Resolves result content_keys → their visits via the keyed `.pkrev` sidecar, falling back to scan.
///
/// The bounded hydration join: when the keyed sidecars are usable this is O(k'·log n)
/// binary-search seeks over the few result content_keys; otherwise it degrades to the authoritative
/// `.pkmap` full scan (correct, just the old O(n) cost) so an older/torn index still serves the EXACT
/// same history_ids. Both paths return identical results — the sidecar is the SAME
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
#[cfg(test)]
pub(super) async fn semantic_matches(
    vault: &crate::Vault,
    paths: &ProjectPaths,
    _config: &AppConfig,
    _key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiSearchRequest,
    starred_content_keys: Option<&[u64]>,
) -> Result<SemanticMatchReport> {
    super::search_ranking::semantic_page(vault, paths, provider, request, starred_content_keys)
        .await
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
pub(super) fn semantic_config_drift_reason(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    engine: &super::embedding_engine::AnyEmbeddingProvider,
    index: &FlatVectorIndex,
    query_dim: usize,
) -> Result<Option<AiSearchNote>> {
    // Dim mismatch: the binarized query byte width differs from the plane's, so a search would
    // prefix-compare and score garbage. Reject loudly with a rebuild note.
    if query_dim != index.dim() {
        return Ok(Some(AiSearchNote::ConfigDriftDimension));
    }

    // Same-dim fingerprint drift: build the live fingerprint exactly as the build path stamps it and
    // ask whether the planes are stale against it. The selected engine carries the real
    // dtype/normalized/pooling/instruction (the build path uses these via `vector_store_for_chunk`);
    // we override the provider id/model + observed dim so the comparison keys match the stored stamp.
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
        return Ok(Some(AiSearchNote::ConfigDriftFingerprint));
    }
    Ok(None)
}

/// Resolves optional `"YYYY-MM-DD"` date filters on the request to the Unix-millisecond range
/// `list_history` expects. Unparseable dates are silently ignored (the user-typed a bad date).
pub(super) fn resolve_date_filter_ms(request: &AiSearchRequest) -> (Option<i64>, Option<i64>) {
    let start_ms = request
        .start_date
        .as_deref()
        .and_then(crate::utils::date_str_to_unix_ms_range)
        .map(|(start, _)| start);
    let end_ms = request
        .end_date
        .as_deref()
        .and_then(crate::utils::date_str_to_unix_ms_range)
        .map(|(_, end)| end);
    (start_ms, end_ms)
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
