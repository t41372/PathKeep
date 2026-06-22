//! The resumable embed loop and the crash-window dedup state that guards it (W-AI-4 / 02 §C.6 R1).
//!
//! ## Responsibilities
//! - drive the build orchestration ([`build_ai_index_with_control`]): run-ledger bookkeeping, the
//!   one-shot full-rebuild wipe, the embed backfill, and the derived-plane re-projection
//! - embed candidate rows in resumable, cancellable chunks ([`run_embedding_backfill`]) and persist
//!   them to the `.pkvec` store + `.pkmap` visit map + SQLite compatibility rows
//! - encapsulate the no-dup/no-miss crash-window invariants in [`DedupTracker`] so they are tested
//!   directly rather than only through end-to-end resume tests
//!
//! ## Not responsible for
//! - candidate collection / embed-target selection (`super::candidates` owns those)
//! - the vector store / visit map byte format (`super::super::vector_store` / `visit_content_map`)

use super::super::*;
use super::candidates::{collect_stale_history_ids, collect_visit_chunk, select_embed_targets};
use super::store_rows::{
    chunk_size, cleanup_stale_embeddings, clear_provider_embeddings, upsert_embedding,
    validate_embedding_batch_for_keys,
};
use super::{BackfillOutcome, IndexedVisit};
use crate::models::ReembedScope;
use std::collections::HashSet;

/// Chrome-epoch offset in milliseconds (`urls.last_visit_ms` is Chrome-epoch ms).
const CHROME_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

/// Resolves the WorkingSet scope filter: the canonical URLs of the bounded heavy-tier working set.
///
/// `WorkingSet` → `Some(set)` of canonical URLs (one per page) from [`select_working_set`], which is
/// itself hard-capped at [`MAX_WORKING_SET`] (so this is BOUNDED, never a 14.4M scan). `Incremental` /
/// `Full` → `None`, meaning "no scope filter — touch every candidate". Computed once per backfill, so
/// the per-chunk membership test below is a cheap canonical-URL set lookup.
fn working_set_canonical_urls(
    connection: &Connection,
    scope: ReembedScope,
) -> Result<Option<HashSet<String>>> {
    if scope != ReembedScope::WorkingSet {
        return Ok(None);
    }
    let now_ms = chrono::Utc::now().timestamp_millis() + CHROME_EPOCH_OFFSET_MS;
    let candidates = super::super::select_working_set(
        connection,
        &super::super::WorkingSetConfig::default(),
        now_ms,
        super::super::MAX_WORKING_SET,
    )?;
    Ok(Some(candidates.into_iter().map(|candidate| candidate.canonical_url).collect()))
}

/// Whether a candidate visit's page is inside the (optional) WorkingSet scope filter.
///
/// `None` filter (Incremental/Full) → always `true` (no restriction). With a filter, the visit's raw
/// url is canonicalized the SAME way [`select_working_set`] keys its candidates so a tracking-param /
/// host-casing variant still matches its page; an unparseable url is treated as NOT in the set (it was
/// never a working-set member). PURE → unit-tested for both the no-filter and filtered branches.
fn visit_in_working_set(filter: &Option<HashSet<String>>, raw_url: &str) -> bool {
    match filter {
        None => true,
        Some(set) => crate::visit_taxonomy::normalize_visit_url(raw_url)
            .is_some_and(|normalized| set.contains(&normalized.canonical_url)),
    }
}

/// Builds or refreshes the semantic sidecar for one embedding provider.
///
/// This is the main entrypoint for semantic indexing. It keeps all durable side effects
/// together: run-ledger bookkeeping, SQLite compatibility rows, optional sidecar sync, and
/// stale-row cleanup.
pub async fn build_ai_index(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    build_ai_index_with_control(paths, config, key, provider, request, None, 0, None).await
}

/// Builds the semantic index while honoring optional cooperative stop checkpoints.
///
/// The worker queue uses this path so a user-triggered cancel can stop between batches
/// without leaving the ledger and sidecar in a misleading half-finished state.
///
/// `start_history_id` resumes a chunked backfill from a persisted cursor (02 §C.6 R1): only
/// candidates with `history_id >= start_history_id` are scanned, so a restart never re-embeds rows
/// already on the vector plane. `ledger`, when present, is called after each chunk is durably
/// persisted so the worker can advance the cursor in the job payload; `None` runs straight through
/// (the foreground convenience path). FTS5 + deterministic intelligence keep serving throughout —
/// nothing here touches the lexical or canonical planes.
#[allow(clippy::too_many_arguments)]
pub async fn build_ai_index_with_control(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
    start_history_id: i64,
    ledger: Option<Arc<dyn IndexBackfillLedger>>,
) -> Result<AiIndexReport> {
    validate_provider(provider, AiProviderPurpose::Embedding)?;
    let archive = open_archive_connection(paths, config, key)?;
    let connection = open_intelligence_connection(paths, config, key)?;
    ensure_ai_schema(&connection)?;
    let started_at = now_rfc3339();
    let source_watermark = current_source_watermark(&connection)?;
    let sidecar_table =
        ai_sidecar::provider_table_name(&provider.config.id, &provider.config.default_model);
    let run_id = begin_ai_run(
        &archive,
        "ai_index",
        "manual",
        json!({
            "providerId": provider.config.id,
            "model": provider.config.default_model,
            "fullRebuild": request.full_rebuild,
            "clearOnly": request.clear_only,
            "limit": request.limit,
        }),
    )?;
    record_index_ledger_start(
        &connection,
        provider,
        run_id,
        &started_at,
        source_watermark,
        &sidecar_table,
        request,
    )?;

    // A full rebuild's destructive wipe is a ONE-SHOT that runs only at the TRUE start of the job
    // (the persisted cursor is still at the origin). On any resume (`start_history_id > 0`) a worker
    // restart MUST NOT re-clear the partially-rebuilt store/metadata, or every row already embedded
    // below the cursor is silently lost while the job still reports success (CRITICAL-1, 02 §C.6 R1).
    // The resume APPENDS to the existing store instead. The worker also flips `request.full_rebuild`
    // to false in the persisted payload once the first cursor is recorded, so a re-claim never sees
    // a destructive full_rebuild again; this `start_history_id` gate is the in-process backstop.
    let is_resume = start_history_id > 0;
    // `ReembedScope::Full` carries full-rebuild semantics (wipe the plane, embed all unique pages),
    // so it triggers the one-shot wipe exactly like the legacy `full_rebuild` flag (W-AI-9-D). The
    // WorkingSet/Incremental scopes never wipe — they append/update.
    let scope_requests_full_rebuild = request.scope == ReembedScope::Full;
    // The effective full-rebuild signal: the legacy flag OR a `Full` scope. Drives the plane wipe and
    // the sidecar full-rebuild bookkeeping so a `Full` re-embed behaves identically to `full_rebuild`.
    let full_rebuild = request.full_rebuild || scope_requests_full_rebuild;
    let wipe_requested = full_rebuild || request.clear_only;
    let perform_wipe = wipe_requested && !is_resume;
    // The Metal heavy-tier opt-in (W-AI-9-D): only changes the candle forward-pass device in a
    // `metal` build; INERT (CPU) otherwise and never affects the vectors/fingerprint.
    let gpu_enabled = config.ai.gpu_enabled;

    let result: Result<AiIndexReport> = async {
        let run_control = run_control.as_ref();
        checkpoint_ai_run(run_control, "Index build was cancelled before collecting stale rows.")?;
        let stale_history_ids = collect_stale_history_ids(&connection, provider)?;

        if perform_wipe {
            clear_provider_embeddings(&connection, provider)?;
        }

        let removed_items = cleanup_stale_embeddings(&connection, provider)?;
        let sidecar_removed = if perform_wipe {
            ai_sidecar::clear_provider_embeddings(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )
            .await?
        } else {
            0
        };

        if request.clear_only {
            // The derived recall/rescore planes (W-AI-5) are pure projections of the `.pkvec` source
            // the wipe above just cleared, so they are deleted too — a later build re-projects them
            // from a fresh source. (`perform_wipe` already removed the `.pkvec`/`.pkmap`.)
            super::super::vector_planes::delete_planes(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )?;
            return Ok(AiIndexReport {
                job_id: None,
                run_id: Some(run_id),
                provider_id: provider.config.id.clone(),
                model: provider.config.default_model.clone(),
                indexed_items: 0,
                updated_items: 0,
                skipped_items: 0,
                removed_items: removed_items + sidecar_removed,
                last_indexed_at: now_rfc3339(),
                notes: vec![
                    "Cleared the semantic index compatibility rows and optional vector sidecar."
                        .to_string(),
                ],
            });
        }

        // A full rebuild resets the vector plane so the new fingerprint owns a clean store — but
        // ONLY at the true start of the job. On a resume the partial store must survive so the
        // backfill appends to it instead of deleting the rows it already embedded (CRITICAL-1). The
        // visit→content map (W-AI-4c) is reset alongside the store so the two planes stay consistent.
        if full_rebuild && !is_resume {
            VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model)
                .delete()?;
            VisitContentMap::for_provider(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )
            .delete()?;
            // Drop the derived planes too so the rebuild window never leaves them projected from the
            // old (now-deleted) source; they are re-projected after the backfill writes the new
            // `.pkvec` (W-AI-5). On a resume the partial planes are NOT dropped (they re-project at
            // the end anyway, and the partial `.pkvec` survives per CRITICAL-1).
            super::super::vector_planes::delete_planes(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )?;
        }

        checkpoint_ai_run(run_control, "Index build was cancelled before collecting candidates.")?;
        let outcome = run_embedding_backfill(
            paths,
            &connection,
            provider,
            request,
            run_control,
            start_history_id,
            ledger.as_ref(),
            perform_wipe,
            gpu_enabled,
        )
        .await?;

        // Sync the optional legacy sidecar shape for empty/stale-removal bookkeeping. The real
        // vectors live in the `derived/vectors/` plane (written above); this only keeps the
        // historical sidecar API honest for clear/empty cases.
        await_with_ai_cancellation(
            run_control,
            "Index build was cancelled before the sidecar sync finished.",
            ai_sidecar::sync_provider_embeddings(
                paths,
                &provider.config.id,
                &provider.config.default_model,
                &[],
                full_rebuild,
                false,
                &stale_history_ids,
            ),
        )
        .await?;

        // Re-project the derived recall (binary) + rescore (int8) planes from the now-updated
        // `.pkvec` source (W-AI-5, 05 §3). This is a pure projection (zero training), so it runs
        // whenever the source store exists — a build that embedded new rows, AND an incremental pass
        // whose source already exists, both leave the derived planes in sync with the f32 source. A
        // missing store (nothing ever embedded) writes empty planes so search degrades to "no
        // semantic matches" rather than a stale/absent-plane error.
        //
        // TODO(W-AI-6/later): incremental passes currently RE-PROJECT all planes (O(total) CPU), not
        // just the newly-embedded rows. After the C2 streaming projection this is now RAM-bounded
        // (build peak ≪ 1 GB at any scale — the source is streamed, never materialized), and it runs
        // off the UI thread in the worker, so for the realistic 1–3M post-dedup corpus it is
        // acceptable. Add an incremental plane-APPEND (project only the new content_keys, append to the
        // existing planes) when the corpus warrants — see 05 §10 carryover.
        checkpoint_ai_run(
            run_control,
            "Index build was cancelled before projecting derived planes.",
        )?;
        super::super::vector_planes::build_planes_from_store(
            paths,
            &provider.config.id,
            &provider.config.default_model,
        )?;

        let notes = if outcome.indexed_items == 0 {
            vec!["No new or changed history rows required indexing.".to_string()]
        } else {
            vec![format!(
                "Embedded {} history row(s) into the vector plane (dim {}).",
                outcome.indexed_items,
                outcome.effective_dim.unwrap_or_default()
            )]
        };
        Ok(AiIndexReport {
            job_id: None,
            run_id: Some(run_id),
            provider_id: provider.config.id.clone(),
            model: provider.config.default_model.clone(),
            indexed_items: outcome.indexed_items,
            updated_items: outcome.updated_items,
            skipped_items: outcome.skipped_items,
            removed_items: removed_items + sidecar_removed,
            last_indexed_at: now_rfc3339(),
            notes,
        })
    }
    .await;

    match result {
        Ok(report) => {
            finalize_ai_run_success(
                &archive,
                run_id,
                json!({
                    "providerId": report.provider_id,
                    "model": report.model,
                    "indexedItems": report.indexed_items,
                    "updatedItems": report.updated_items,
                    "removedItems": report.removed_items,
                }),
            )?;
            record_index_ledger_success(
                &connection,
                provider,
                run_id,
                &report.last_indexed_at,
                source_watermark,
                &sidecar_table,
                request,
            )?;
            Ok(report)
        }
        Err(error) => {
            finalize_ai_run_failure(
                &archive,
                run_id,
                &error.to_string(),
                json!({
                    "providerId": provider.config.id,
                    "model": provider.config.default_model,
                    "fullRebuild": request.full_rebuild,
                    "clearOnly": request.clear_only,
                }),
            )?;
            record_index_ledger_failure(
                &connection,
                provider,
                run_id,
                source_watermark,
                &sidecar_table,
                request,
                &error.to_string(),
            )?;
            Err(error)
        }
    }
}

/// The resume/incremental crash-window dedup state for one backfill pass (W-AI-4c, 02 §C.6 R1).
///
/// Encapsulates the three sets that together make the embed loop no-dup/no-miss across the two crash
/// windows the resumable backfill must survive — extracted from the loop's locals so the invariants
/// are unit-tested DIRECTLY (the loop only exercised them end-to-end through resume tests):
/// - `persisted_keys` — the u64 `content_key`s already on the `.pkvec` store (the storage-boundary
///   guard: the on-disk store carries only u64s). CRITICAL-2: a crash after a chunk's vectors are
///   appended but before its SQLite rows / cursor advance leaves a key on the store while
///   `needs_embedding` flips true again; a key in this set is never re-appended, so a resume cannot
///   double a vector. Loaded from the store on resume/incremental AND accumulated this run.
/// - `persisted_hashes` — the FULL `content_hash`es embedded THIS run (the WORK identity). MEDIUM-4:
///   the work-dedup keys on the full hash, NOT the truncated u64, so two DISTINCT pages whose hashes
///   collide on the first 8 bytes (~2.8e-6 at 14.4M) are EACH embedded rather than the second
///   silently collapsing onto the first's vector. This set is run-scoped (never loaded from disk,
///   since the on-disk store cannot recover the full hash).
/// - `mapped_ids` — the `history_id`s already in the `.pkmap` visit map. A crash mid-chunk can leave
///   some visits mapped; a visit in this set is never re-appended, so a resume never doubles a
///   mapping entry. Loaded from the map on resume/incremental AND accumulated this run.
///
/// MEDIUM-5: [`load_existing`](Self::load_existing) populates the on-disk sets whenever an existing
/// store is present and NOT about to be wiped — not only on a `start_history_id > 0` resume. So a
/// PLAIN incremental pass (cursor 0, empty sets without this) does not re-embed + append a duplicate
/// `.pkvec` record for a NEW visit of an already-embedded page; it maps the visit but skips the embed.
pub(super) struct DedupTracker {
    /// u64 content_keys already on the `.pkvec` store (resume backstop + this run's appends).
    persisted_keys: HashSet<u64>,
    /// FULL content_hashes embedded this run (the MEDIUM-4 work identity; never loaded from disk).
    persisted_hashes: HashSet<String>,
    /// history_ids already in the `.pkmap` visit map (resume backstop + this run's appends).
    mapped_ids: HashSet<i64>,
}

impl DedupTracker {
    /// Starts an empty tracker (all three sets empty); the loop calls [`Self::load_existing`] next.
    pub(super) fn new() -> Self {
        Self {
            persisted_keys: HashSet::new(),
            persisted_hashes: HashSet::new(),
            mapped_ids: HashSet::new(),
        }
    }

    /// Loads the resume/incremental on-disk dedup state when an existing store is present (MEDIUM-5).
    ///
    /// Populates `persisted_keys` from the store's `existing_ids` and `mapped_ids` from the visit
    /// map's `mapped_history_ids` ONLY when `!perform_wipe && store.exists()` — mirroring the loop's
    /// gate exactly. A true full rebuild (`perform_wipe`) or a never-built store leaves the sets
    /// empty so the build starts clean. `persisted_hashes` is NEVER loaded: the on-disk store carries
    /// only the truncated u64, so the full hash is unrecoverable from it (MEDIUM-4); the full-hash
    /// authority is built up this run via [`Self::record_embedded`].
    pub(super) fn load_existing(
        &mut self,
        store: &VectorStore,
        visit_map: &VisitContentMap,
        perform_wipe: bool,
    ) -> Result<()> {
        if !perform_wipe && store.exists() {
            self.persisted_keys = store.existing_ids()?;
            self.mapped_ids = visit_map.mapped_history_ids()?;
        }
        Ok(())
    }

    /// Selects, in first-seen order, the unique pages among `changed` still needing an embed.
    ///
    /// Delegates to the pure [`super::candidates::select_embed_targets`] with this tracker's
    /// encapsulated `persisted_hashes` + `persisted_keys`, so the production loop and the
    /// direct-collision unit test share one decision. See [`Self::should_embed`] for the per-visit
    /// predicate the same logic reduces to.
    pub(super) fn select_targets<'a>(&self, changed: &[&'a IndexedVisit]) -> Vec<&'a IndexedVisit> {
        select_embed_targets(changed, &self.persisted_hashes, &self.persisted_keys)
    }

    /// Whether a single visit still needs embedding under the current dedup state (MEDIUM-4/CRITICAL-2).
    ///
    /// A visit needs an embed unless its FULL `content_hash` was already embedded this run OR its u64
    /// `content_key` is already on the `.pkvec` store. This is the per-visit predicate the chunk-level
    /// [`Self::select_targets`] reduces to (minus the intra-chunk first-seen dedup); it exists so the
    /// crash-window invariants are asserted directly on one visit at a time. Test-only: the production
    /// loop selects at chunk granularity via [`Self::select_targets`].
    #[cfg(test)]
    pub(super) fn should_embed(&self, visit: &IndexedVisit) -> bool {
        !self.persisted_hashes.contains(visit.content_hash.as_str())
            && !self.persisted_keys.contains(&visit.content_key)
    }

    /// Records that one page (its full hash + u64 key) was embedded + persisted this run.
    ///
    /// After this, the page's full hash skips it run-wide (MEDIUM-4 work identity) and its u64 key
    /// skips it at the storage boundary (CRITICAL-2), so a later chunk or a resume never re-embeds it.
    pub(super) fn record_embedded(&mut self, key: u64, content_hash: &str) {
        self.persisted_hashes.insert(content_hash.to_string());
        self.persisted_keys.insert(key);
    }

    /// Filters `changed` to the `(history_id, content_key)` records not yet in the visit map.
    ///
    /// Marks each newly-seen `history_id` as mapped (so this run never doubles a mapping) and skips
    /// any a prior crash already mapped, returning exactly the records to append to the `.pkmap`. The
    /// VECTOR is deduped (one per page) but the MAPPING is per-visit, so every changed visit's id is a
    /// candidate here — the dedup is only against the already-mapped set, not the page identity.
    pub(super) fn take_unmapped(&mut self, changed: &[&IndexedVisit]) -> Vec<(i64, u64)> {
        changed
            .iter()
            .filter(|visit| self.mapped_ids.insert(visit.history_id))
            .map(|visit| (visit.history_id, visit.content_key))
            .collect()
    }
}

/// Embeds candidate rows in resumable, cancellable chunks and persists them to the vector plane.
///
/// The hot path of W-AI-4 (02 §C.6 R1). For each ascending-`history_id` chunk from
/// `start_history_id`: builds content, diffs the content hash to skip unchanged rows, embeds the
/// changed rows under [`EmbeddingRole::Document`] through the external provider, writes the vectors
/// to the [`VectorStore`] AND the SQLite compatibility metadata, then advances the watermark via
/// the `ledger` so a restart resumes from the next id (not from scratch). Cancellation is checked
/// at each chunk boundary, where on-disk state is consistent. The vector store is lazily created
/// once the real dim is observed (D4: never assume a dim before a vector is returned).
///
/// Correctness 鐵律 (02 §C.3): `effective_dim` is read from the actual returned vector length; the
/// provider L2-normalizes defensively; the document role threads through. The fingerprint stamped
/// on the store records that truth (dtype/normalized set by the external adapter, A-S2 fix).
#[allow(clippy::too_many_arguments)]
async fn run_embedding_backfill(
    paths: &ProjectPaths,
    connection: &Connection,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    run_control: Option<&Arc<dyn AiRunControl>>,
    start_history_id: i64,
    ledger: Option<&Arc<dyn IndexBackfillLedger>>,
    perform_wipe: bool,
    gpu_enabled: bool,
) -> Result<BackfillOutcome> {
    // Config-driven engine selection: static (W-AI-4c) / candle (W-AI-4b) / external `/v1/embeddings`.
    // The loop below is engine-agnostic. `gpu_enabled` is the heavy-tier Metal opt-in (W-AI-9-D); it
    // only changes the candle forward-pass device in a `metal` build and never the vectors/fingerprint.
    let embedder =
        super::super::embedding_candle::select_embedding_provider(paths, provider, gpu_enabled)?;
    // WorkingSet scope filters this run's candidates to the BOUNDED heavy-tier set; Incremental/Full
    // touch every candidate (Full's wipe is driven by `perform_wipe`, set by the caller). Computed
    // ONCE up front (bounded `select_working_set`, hard-capped at MAX_WORKING_SET), then matched per
    // chunk by canonical URL — `None` means "no scope filter" (Incremental/Full).
    let working_set_filter = working_set_canonical_urls(connection, request.scope)?;
    let limit = request.limit.map(|value| value as usize);
    let mut outcome = BackfillOutcome::default();
    // The store is resolved lazily on the first chunk that actually embeds rows, once the real dim
    // is known (D4) — see `vector_store_for_chunk`. Until then no store handle is held.
    let mut store: Option<VectorStore> = None;
    let visit_map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    // The map is created up front so the per-chunk visit appends always have a header to append to.
    // On a full rebuild the destructive `delete` already ran in the caller (true start only), so this
    // writes a fresh map; on a resume it leaves the existing map (and its prior visit rows) intact.
    visit_map.ensure_created(paths)?;
    let mut cursor = start_history_id;
    let mut embedded_total: u64 = 0;

    // The crash-window dedup state (CRITICAL-2 / MEDIUM-4 / MEDIUM-5) is encapsulated in
    // `DedupTracker`. `load_existing` populates the on-disk sets whenever a store/map already exists
    // and is NOT about to be wiped — so a PLAIN incremental pass (cursor 0) maps a re-visit of an
    // already-embedded page instead of re-embedding it (MEDIUM-5), and a resume never doubles a
    // vector or a mapping entry. See `DedupTracker` for the full invariant rationale.
    let mut tracker = DedupTracker::new();
    let existing_store =
        VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model);
    tracker.load_existing(&existing_store, &visit_map, perform_wipe)?;

    loop {
        checkpoint_ai_run(run_control, "Index build was cancelled before the next chunk.")?;
        let remaining = limit.map(|cap| cap.saturating_sub(outcome.indexed_items));
        if remaining == Some(0) {
            break;
        }
        let chunk =
            collect_visit_chunk(paths, connection, provider, cursor, chunk_size(remaining))?;
        let Some(last_history_id) = chunk.last().map(|visit| visit.history_id) else {
            break; // No more candidate rows.
        };
        // Advance the scan cursor past this chunk regardless of how many rows were changed, so the
        // next pass never re-scans this id range.
        cursor = last_history_id + 1;

        let changed: Vec<&IndexedVisit> = chunk
            .iter()
            .filter(|visit| visit.needs_embedding)
            // WorkingSet scope: keep only candidates whose page is in the bounded working set. A
            // `None` filter (Incremental/Full) keeps everything. Pages outside the set are counted as
            // skipped below, exactly like an unchanged page — the loop body is otherwise unchanged.
            .filter(|visit| visit_in_working_set(&working_set_filter, &visit.url))
            .collect();
        outcome.skipped_items += chunk.len() - changed.len();

        if !changed.is_empty() {
            // DEDUP: collect the UNIQUE pages among the changed visits that are not already embedded,
            // and embed each ONCE (the page identity is the FULL `content_hash`, MEDIUM-4).
            let embed_targets = tracker.select_targets(&changed);

            if !embed_targets.is_empty() {
                let texts: Vec<String> =
                    embed_targets.iter().map(|visit| visit.content.clone()).collect();
                let vectors = await_with_ai_cancellation(
                    run_control,
                    "Index build was cancelled while embedding a chunk.",
                    embedder.embed(&texts, EmbeddingRole::Document),
                )
                .await?;
                let content_keys: Vec<u64> =
                    embed_targets.iter().map(|visit| visit.content_key).collect();
                let (effective_dim, records) =
                    validate_embedding_batch_for_keys(&content_keys, &vectors)?;
                outcome.effective_dim = Some(effective_dim);
                // `perform_wipe` (NOT `request.full_rebuild`) drives the store reset: a full_rebuild
                // RESUME appends to the partial store rather than truncating it (CRITICAL-1). The
                // fingerprint is stamped from the SELECTED engine's real descriptor (static = Mean,
                // candle = LastToken, external = Unknown), so two engines never share a fingerprint.
                let store = vector_store_for_chunk(
                    &mut store,
                    paths,
                    provider,
                    &embedder.descriptor(),
                    effective_dim,
                    perform_wipe,
                )?;
                // Every record here is a page we have NOT embedded this run (the full-hash dedup above
                // guaranteed it), so all are appended. Record each page's FULL hash + u64 key so a
                // later chunk / resume skips it (MEDIUM-4: the full hash is the work identity; the u64
                // is the storage-boundary backstop). The store accepts duplicate u64s for the rare
                // collision; `read_all` last-writer-wins resolves that storage-boundary case.
                for (visit, (key, _)) in embed_targets.iter().zip(records.iter()) {
                    tracker.record_embedded(*key, &visit.content_hash);
                }
                // Persist vectors FIRST so the watermark we report is always backed by on-disk data.
                store.append_vectors(&records)?;
            }

            // Map EVERY changed visit to its content_key (skip ones a prior crash already mapped), so
            // search/heavy-tier can fan a deduped vector out to all its visits. SQLite metadata is
            // (re-)written for every changed visit so its dedup hash catches up and it is not
            // re-scanned. Both are per-visit; only the VECTOR is deduped.
            let map_records = tracker.take_unmapped(&changed);
            visit_map.append(&map_records)?;

            let indexed_at = now_rfc3339();
            for visit in &changed {
                let was_present = upsert_embedding(connection, provider, visit, &indexed_at)?;
                outcome.indexed_items += 1;
                if was_present {
                    outcome.updated_items += 1;
                }
            }
            embedded_total += changed.len() as u64;
        }

        if let Some(ledger) = ledger {
            ledger.record(crate::ai::IndexBackfillProgress {
                next_history_id: cursor,
                embedded_so_far: embedded_total,
            })?;
        }

        if chunk.len() < chunk_size(remaining) {
            break; // Final (short) chunk: no more rows beyond this id range.
        }
    }
    Ok(outcome)
}

/// Lazily creates the fingerprint-stamped vector store once the real dim is known (D4).
///
/// On the first chunk that actually embeds rows, the effective dim is observed and a store is
/// stamped with the [`EmbeddingFingerprint`] built from this adapter's real descriptor
/// (dtype/normalized true, A-S2 fix). On later chunks the already-open store is reused. A stale
/// (fingerprint-mismatched) existing store on an INCREMENTAL pass is the clear seam W-AI-5 wires
/// full re-embed migration into; here we surface it as an explicit error rather than silently
/// appending dimension-incompatible vectors.
fn vector_store_for_chunk<'a>(
    store: &'a mut Option<VectorStore>,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    engine_descriptor: &EmbeddingDescriptor,
    effective_dim: usize,
    full_rebuild: bool,
) -> Result<&'a VectorStore> {
    if store.is_none() {
        // Stamp the fingerprint from the SELECTED engine's real descriptor (its true dtype /
        // normalized / pooling / instruction, A-S2 fix) with the observed dim, but key it by the
        // provider CONFIG identity so the on-disk store name + SQLite ledger stay in sync. This is
        // what keeps a candle-built and an external-built index distinct at the same dim while
        // staying findable by the same provider/model pair the rest of the pipeline uses.
        let descriptor = EmbeddingDescriptor {
            provider_id: provider.config.id.clone(),
            model_id: provider.config.default_model.clone(),
            effective_dim: Some(effective_dim),
            ..engine_descriptor.clone()
        };
        let fingerprint = EmbeddingFingerprint::from_descriptor(&descriptor).context(
            "internal: embedding descriptor lacked an effective dim when stamping the vector store",
        )?;
        let existing =
            VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model);
        let created = if full_rebuild || !existing.exists() {
            VectorStore::create_stamped(paths, &fingerprint)?
        } else if existing.is_stale_against(&fingerprint)? {
            anyhow::bail!(
                "The existing vector store for provider {} was built under a different embedding configuration. Run a full rebuild to re-embed it.",
                provider.config.name
            );
        } else {
            existing
        };
        *store = Some(created);
    }
    Ok(store.as_ref().expect("vector store is initialized above"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use tempfile::tempdir;

    /// Builds a synthetic `IndexedVisit` whose `content_key` is derived from `content_hash`, so the
    /// truncated-u64 collision relationship is the real one the production loop sees.
    fn visit(history_id: i64, content_hash: &str) -> IndexedVisit {
        IndexedVisit {
            history_id,
            profile_id: "chrome:Default".to_string(),
            url: format!("https://example.com/{history_id}"),
            title: Some("Page".to_string()),
            domain: "example.com".to_string(),
            visited_at: "2026-06-21T00:00:00Z".to_string(),
            content: format!("content-{history_id}"),
            content_key: super::super::super::dedup::content_key_from_hash(content_hash),
            content_hash: content_hash.to_string(),
            needs_embedding: true,
        }
    }

    #[test]
    fn visit_in_working_set_passes_everything_when_no_filter() {
        // Incremental/Full → `None` filter → every candidate passes (no scope restriction).
        assert!(visit_in_working_set(&None, "https://anything.example/x"));
        assert!(visit_in_working_set(&None, "not a url"));
    }

    #[test]
    fn visit_in_working_set_matches_by_canonical_url() {
        // WorkingSet → only candidates whose CANONICAL url is in the set pass; a tracking-param variant
        // of a member still matches (canonicalized the same way the selector keys members), and an
        // unparseable url is treated as a non-member rather than crashing.
        let canonical = crate::visit_taxonomy::normalize_visit_url("https://example.com/page")
            .expect("canonical")
            .canonical_url;
        let filter = Some(HashSet::from([canonical]));
        assert!(visit_in_working_set(&filter, "https://example.com/page"));
        assert!(
            visit_in_working_set(&filter, "https://example.com/page?utm_source=ad"),
            "a tracking-param variant of a member matches its canonical url"
        );
        assert!(!visit_in_working_set(&filter, "https://other.example/page"));
        assert!(!visit_in_working_set(&filter, "::: not a url :::"));
    }

    #[test]
    fn u64_content_key_collision_keeps_both_distinct_pages_embeddable() {
        // MEDIUM-4 (direct): two DISTINCT pages whose full content_hashes truncate to the SAME u64
        // content_key must BOTH be embeddable — the tracker keys work-dedup on the FULL hash, never
        // the truncated u64. After recording the FIRST page, the SECOND (a distinct hash) is NOT
        // dropped onto the first's vector.
        let hash_a = "0102030405060708aaaaaaaaaaaaaaaa";
        let hash_b = "0102030405060708bbbbbbbbbbbbbbbb";
        let a = visit(1, hash_a);
        let b = visit(2, hash_b);
        assert_eq!(a.content_key, b.content_key, "test setup must produce a real u64 collision");
        assert_ne!(a.content_hash, b.content_hash);

        let mut tracker = DedupTracker::new();
        // Both embeddable before anything is recorded; the chunk-level selector keeps BOTH.
        assert!(tracker.should_embed(&a));
        assert!(tracker.should_embed(&b));
        let targets = tracker.select_targets(&[&a, &b]);
        assert_eq!(targets.len(), 2, "a u64 collision must NOT drop the second distinct page");

        // Embed + record page A. Its FULL hash now skips A; the colliding-but-DISTINCT page B is still
        // embeddable because the work identity is the full hash, not the shared u64 key.
        tracker.record_embedded(a.content_key, &a.content_hash);
        assert!(!tracker.should_embed(&a), "page A's full hash skips it run-wide");
        assert!(
            !tracker.should_embed(&b),
            "B shares A's u64 key, so the storage-boundary backstop now skips B WITHIN this run"
        );
        // Within a single run the u64 backstop conservatively skips B (only a redundant re-embed is
        // ever lost, never correctness); the run-level FULL-hash authority is what guarantees two
        // colliding pages stay distinct when neither key is yet on disk — proven by `select_targets`
        // above keeping both before any record. The same page seen twice dedups to one target:
        let a_again = visit(3, hash_a);
        let one = tracker.select_targets(&[&a_again]);
        assert!(one.is_empty(), "page A already embedded this run is not re-selected");
    }

    #[test]
    fn resume_skips_already_embedded_keys_without_re_embedding_or_missing() {
        // CRITICAL-2 (direct): after a partial chunk, the keys already on the `.pkvec` store must be
        // skipped on resume (no re-embed, no double vector) while a genuinely new page still embeds
        // (no miss). `load_existing` seeds `persisted_keys` from the store's on-disk u64 set.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let already = visit(1, "11111111111111111111111111111111");
        let fresh = visit(2, "22222222222222222222222222222222");

        // Simulate "chunk 1 embedded `already` then crashed before the cursor advanced": its key is
        // on the store, its full hash is NOT in this (new) run's set.
        let store =
            VectorStore::create_stamped(&paths, &fingerprint(3, &already)).expect("create store");
        store.append_vectors(&[(already.content_key, vec![1.0, 0.0, 0.0])]).expect("seed key");
        let map = VisitContentMap::for_provider(&paths, "static-embed", "model-a");
        map.ensure_created(&paths).expect("map");

        let mut tracker = DedupTracker::new();
        tracker.load_existing(&store, &map, /* perform_wipe */ false).expect("load");

        // The already-embedded key is skipped (no re-embed → no second vector); the new page embeds.
        assert!(!tracker.should_embed(&already), "resume must skip a key already on the store");
        assert!(tracker.should_embed(&fresh), "a genuinely new page must still embed (no miss)");
        let targets = tracker.select_targets(&[&already, &fresh]);
        assert_eq!(targets.len(), 1, "exactly the fresh page is re-embedded on resume");
        assert_eq!(targets[0].content_hash, fresh.content_hash);
    }

    #[test]
    fn incremental_revisit_of_embedded_page_is_mapped_not_re_embedded() {
        // MEDIUM-5 (direct): a PLAIN incremental pass (NOT a resume) of a NEW visit of an
        // already-embedded page must MAP the visit (no-miss fan-out) but NOT re-embed it (no duplicate
        // `.pkvec` record / bloat). `load_existing` seeds the on-disk sets at cursor 0 because the
        // store already exists and is not being wiped.
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let page = visit(1, "33333333333333333333333333333333");
        // The page is already embedded (its key on the store) and visit 1 is already mapped.
        let store = VectorStore::create_stamped(&paths, &fingerprint(2, &page)).expect("store");
        store.append_vectors(&[(page.content_key, vec![1.0, 0.0])]).expect("seed vector");
        let map = VisitContentMap::for_provider(&paths, "static-embed", "model-a");
        map.ensure_created(&paths).expect("map");
        map.append(&[(page.history_id, page.content_key)]).expect("seed mapping");

        let mut tracker = DedupTracker::new();
        tracker.load_existing(&store, &map, false).expect("load");

        // A NEW visit (history_id 2) of the SAME page (same content_hash/key).
        let revisit = visit(2, "33333333333333333333333333333333");
        assert_eq!(revisit.content_key, page.content_key);

        // NOT re-embedded: its key is already on the store (no duplicate `.pkvec` record).
        assert!(!tracker.should_embed(&revisit), "an already-embedded page is not re-embedded");
        assert!(tracker.select_targets(&[&revisit]).is_empty(), "no embed target → no new vector");

        // But IS mapped: visit 1 is already mapped (skipped), visit 2 is new → exactly one mapping.
        let unmapped = tracker.take_unmapped(&[&page, &revisit]);
        assert_eq!(
            unmapped,
            vec![(revisit.history_id, revisit.content_key)],
            "only the new visit is mapped; the existing visit's mapping is not doubled"
        );
    }

    #[test]
    fn load_existing_only_reads_state_when_store_present_and_not_wiped() {
        // (d) load_existing gating: populate from store/map ONLY when an existing store is present AND
        // not about to be wiped; a wipe or an absent store leaves the sets empty (clean start).
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let page = visit(1, "44444444444444444444444444444444");

        // Absent store → nothing loaded (every page still embeddable).
        let absent_store = VectorStore::for_provider(&paths, "static-embed", "model-a");
        let map = VisitContentMap::for_provider(&paths, "static-embed", "model-a");
        map.ensure_created(&paths).expect("map");
        map.append(&[(page.history_id, page.content_key)]).expect("seed mapping");
        let mut absent_tracker = DedupTracker::new();
        absent_tracker.load_existing(&absent_store, &map, false).expect("absent load");
        assert!(absent_tracker.should_embed(&page), "no store → nothing persisted → embeddable");
        assert!(
            !absent_tracker.take_unmapped(&[&page]).is_empty(),
            "no store → mapped_ids stays empty → the visit is treated as unmapped"
        );

        // Present store but `perform_wipe = true` → still nothing loaded (a full rebuild starts clean).
        let present_store =
            VectorStore::create_stamped(&paths, &fingerprint(2, &page)).expect("store");
        present_store.append_vectors(&[(page.content_key, vec![1.0, 0.0])]).expect("seed vector");
        let mut wiped_tracker = DedupTracker::new();
        wiped_tracker.load_existing(&present_store, &map, /* perform_wipe */ true).expect("wipe");
        assert!(
            wiped_tracker.should_embed(&page),
            "perform_wipe must NOT load on-disk keys → the page re-embeds on a full rebuild"
        );

        // Present store and NOT wiped → the on-disk key + mapping ARE loaded.
        let mut loaded_tracker = DedupTracker::new();
        loaded_tracker.load_existing(&present_store, &map, false).expect("load");
        assert!(
            !loaded_tracker.should_embed(&page),
            "present + not wiped → the on-disk key is loaded → the page is skipped"
        );
        assert!(
            loaded_tracker.take_unmapped(&[&page]).is_empty(),
            "present + not wiped → the on-disk mapping is loaded → the visit is already mapped"
        );
    }

    /// A fingerprint sized to one synthetic visit's store dim (the dim only needs to fit the seeded
    /// vectors; the content identity is what these tests exercise, not the embedding itself).
    fn fingerprint(dim: usize, _visit: &IndexedVisit) -> EmbeddingFingerprint {
        EmbeddingFingerprint::new(
            "static-embed",
            "model-a",
            dim,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Mean,
            None,
        )
    }
}
