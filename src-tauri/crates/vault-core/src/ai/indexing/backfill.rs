//! Failure modes: batch limits truncate coverage or a crash repeats completed embeddings.
//! The resumable embed loop and the crash-window dedup state that guards it (W-AI-4 / 02 §C.6 R1).
//!
//! ## Responsibilities
//! - drive the build orchestration ([`build_ai_index_with_control`]): run-ledger bookkeeping, the generation owner, the embed backfill, and the derived-plane re-projection
//! - embed candidate rows in resumable, cancellable chunks ([`run_embedding_backfill`]) and persist
//!   them to the `.pkvec` store + `.pkmap` visit map + Turso metadata rows
//! - retain explicit serving pairs until a replacement completes
//! - encapsulate the no-dup/no-miss crash-window invariants in [`DedupTracker`] so they are tested
//!   directly rather than only through end-to-end resume tests
//!
//! ## Not responsible for
//! - candidate collection / embed-target selection (`super::candidates` owns those)
//! - the vector store / visit map byte format (`super::super::vector_store` / `visit_content_map`)

use super::super::generations::{self, BuildGeneration};
use super::super::native_ledger::{
    begin_ai_run, current_source_watermark, end_failed_ai_run, finalize_ai_run_success,
    record_index_ledger_failure, record_index_ledger_start, record_index_ledger_success,
};
use super::super::*;
use super::candidates::{collect_visit_chunk, select_embed_targets};
use super::store_rows::{
    chunk_size, cleanup_stale_embeddings, upsert_embedding, validate_embedding_batch_for_keys,
};
use super::{BackfillOutcome, IndexedVisit};
use crate::models::ReembedScope;
use std::collections::HashSet;
use vault_store::{Db, Priority, Session, params_owned as params};

/// Resolves the WorkingSet scope filter: the canonical URLs of the bounded heavy-tier working set.
///
/// `WorkingSet` → `Some(set)` of canonical URLs (one per page) from [`select_working_set`], which is
/// enumerates every eligible page through cursors. `Incremental` /
/// `Full` → `None`, meaning "no scope filter — touch every candidate". Computed once per backfill, so
/// the per-chunk membership test below is a cheap canonical-URL set lookup.
fn working_set_canonical_urls(
    vault: &crate::Vault,
    scope: ReembedScope,
) -> Result<Option<HashSet<String>>> {
    if scope != ReembedScope::WorkingSet {
        return Ok(None);
    }
    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut cursor = None;
    let mut urls = HashSet::new();
    loop {
        let page = super::super::select_working_set(
            vault,
            &super::super::WorkingSetConfig::default(),
            now_ms,
            4096,
            cursor,
        )?;
        urls.extend(page.items.into_iter().map(|row| row.canonical_url));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(Some(urls))
}

/// Whether a candidate visit's page is inside the (optional) WorkingSet scope filter.
///
/// `None` filter (Incremental/Full) → always `true` (no restriction). With a filter, the visit's raw
/// url is canonicalized the SAME way [`select_working_set`] keys its candidates so a tracking-param /
/// host-casing variant still matches its page; an unparseable url is treated as NOT in the set (it was
/// never a working-set member). Pure (no I/O).
fn visit_in_working_set(filter: &Option<HashSet<String>>, raw_url: &str) -> bool {
    match filter {
        None => true,
        Some(set) => set.contains(&crate::pages::canonical_key(raw_url)),
    }
}

/// Builds or refreshes the semantic sidecar for one embedding provider.
///
/// This is the main entrypoint for semantic indexing. It keeps all durable side effects
/// together: run-ledger bookkeeping, Turso metadata rows, optional sidecar sync, and
/// stale-row cleanup.
pub async fn build_ai_index(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    build_ai_index_with_control(vault, provider, request, None, 0, 0, None).await
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
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
    start_history_id: i64,
    start_embedded_so_far: u64,
    ledger: Option<Arc<dyn IndexBackfillLedger>>,
) -> Result<AiIndexReport> {
    validate_provider(provider, AiProviderPurpose::Embedding)?;
    let started_at = now_rfc3339();
    let source_watermark = current_source_watermark(vault)?;
    let run_id = begin_ai_run(
        vault,
        "ai_index",
        "manual",
        json!({
            "providerId":provider.config.id,"model":provider.config.default_model,
            "fullRebuild":request.full_rebuild,"clearOnly":request.clear_only,"limit":request.limit,
        }),
    )?;
    let full_rebuild = request.full_rebuild || request.scope == ReembedScope::Full;
    let job_id = ledger.as_ref().and_then(|ledger| ledger.job_id());
    let mut build_owner = None;
    let result: Result<AiIndexReport> = async {
        record_index_ledger_start(vault, provider, run_id, &started_at, source_watermark, request)?;
        checkpoint_ai_run(
            run_control.as_ref(),
            "Index build stopped before collecting candidates.",
        )?;
        build_owner = if request.clear_only {
            None
        } else {
            Some(BuildGeneration::open(
                vault,
                provider,
                job_id,
                run_id,
                full_rebuild,
                start_history_id > 0,
            )?)
        };
        let build = &build_owner;
        let (outcome, removed_items, searches) = if let Some(build) = build {
            let (resume, updated) =
                bind_cursor_lineage(vault, job_id, build.lineage(), build.fresh)?;
            let (cursor, embedded) =
                if resume { (start_history_id, start_embedded_so_far) } else { (0, 0) };
            let removed =
                cleanup_stale_embeddings(vault, provider, build.lineage(), run_control.as_ref())?;
            let outcome = run_embedding_backfill(
                build.source(),
                provider,
                request,
                run_control.as_ref(),
                cursor,
                embedded,
                updated,
                ledger.as_ref(),
                build.fresh,
                full_rebuild,
                build.lineage(),
                build.upper_history_id(full_rebuild),
            )
            .await?;
            let searches = super::super::search_vectors::backfill(
                build.source(),
                provider,
                run_control.as_ref(),
            )
            .await?;
            checkpoint_ai_run(run_control.as_ref(), "Index build stopped before projection.")?;
            build.prepare(provider)?;
            (outcome, removed, searches)
        } else {
            (BackfillOutcome::default(), 0, 0)
        };
        let pointer = build.as_ref().map_or("", |build| build.relative());
        let watermark = build
            .as_ref()
            .map_or(source_watermark, |build| build.watermark(source_watermark, full_rebuild));
        let finished_at = now_rfc3339();
        if !record_index_ledger_success(
            vault,
            provider,
            run_id,
            &finished_at,
            watermark,
            pointer,
            request,
            job_id.map(|id| super::super::native_ledger::IndexJobSuccess {
                id,
                summary: super::super::native_ledger::index_success_summary(
                    outcome.indexed_items,
                    outcome.updated_items,
                ),
            }),
        )? {
            return Err(AiRunCancelled::new("Index build stopped before completion.").into());
        }
        // The pointer and queue success have committed. A later stop or optional maintenance
        // failure cannot turn this into a cancelled/failed index pass.
        if let Err(error) = finalize_ai_run_success(
            vault,
            run_id,
            json!({
                "providerId": provider.config.id,
                "model": provider.config.default_model,
                "indexedItems": outcome.indexed_items,
                "updatedItems": outcome.updated_items,
                "removedItems": removed_items,
            }),
        ) {
            eprintln!("pathkeep: recording completed index run {run_id}: {error:#}");
        }
        let notes = if request.clear_only {
            vec!["Cleared the semantic index.".into()]
        } else {
            vec![
                format!(
                    "Embedded {} page vector(s) for {} history row(s); mapped {} updated row(s).",
                    outcome.embedded_vectors, outcome.indexed_items, outcome.updated_items,
                ),
                format!("Embedded {searches} distinct keyword search vector(s)."),
            ]
        };
        Ok(AiIndexReport {
            job_id: None,
            run_id: Some(run_id),
            provider_id: provider.config.id.clone(),
            model: provider.config.default_model.clone(),
            indexed_items: outcome.indexed_items,
            updated_items: outcome.updated_items,
            skipped_items: outcome.skipped_items,
            removed_items,
            last_indexed_at: finished_at,
            notes,
        })
    }
    .await;
    if let Some(build) = &build_owner {
        let finalized = if result.is_err() {
            build.publish_partial(provider).map(|_| ())
        } else {
            build.finish_intent()
        };
        if let Err(error) = finalized {
            eprintln!("pathkeep: partial index recovery will retry: {error:#}");
        }
    }
    drop(build_owner);
    if let Err(error) = &result {
        end_failed_ai_run(vault, run_id, error, json!({}));
        if let Err(recording) = record_index_ledger_failure(
            vault,
            provider,
            run_id,
            source_watermark,
            &format!("{error:#}"),
        ) {
            eprintln!("pathkeep: recording index failure: {recording:#}");
        }
    }
    // Warm first: after a full rebuild, cleanup deletes the replaced lineage's rows, which takes
    // minutes at 14.4M visits, and the new index is already serving.
    if result.is_ok() && !request.clear_only {
        if let Err(error) =
            super::super::search_group_snapshots::warm_search_group_snapshots(vault, provider, None)
        {
            eprintln!("warming search groups after index pass: {error:#}");
        }
    }
    // Queue-owned paused lineages remain resumable; direct-run abandoned lineages can go now.
    if let Err(error) = generations::collect(vault, true) {
        eprintln!("pathkeep: index generation cleanup: {error:#}");
    }
    result
}

/// Retires unconfigured pairs by removing their pointer; generation GC respects existing leases.
pub async fn forget_unconfigured_indexes(
    vault: &crate::Vault,
    configured: &[(String, String)],
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<usize> {
    let indexed=vault.read(Db::Intelligence,|s| Ok(s.query("SELECT provider_id,model FROM ai_embedding_totals UNION SELECT provider_id,model FROM ai_index_ledger",(),|r| Ok((r.get::<String>(0)?,r.get::<String>(1)?)))?))?;
    let mut forgotten = 0;
    for (provider, model) in indexed {
        if configured.contains(&(provider.clone(), model.clone())) {
            continue;
        }
        checkpoint_ai_run(control, "Index cleanup was stopped.")?;
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            s.execute(
                "DELETE FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
                params![provider, model],
            )?;
            Ok(())
        })?;
        forgotten += 1;
    }
    generations::collect(vault, true)?;
    Ok(forgotten)
}

/// The resume/incremental crash-window dedup state for one backfill pass (W-AI-4c, 02 §C.6 R1).
///
/// Encapsulates the three sets that together make the embed loop no-dup/no-miss across the two crash
/// windows the resumable backfill must survive — extracted from the loop's locals so the invariants
/// live in one place:
/// - `persisted_keys` — the u64 `content_key`s already on the `.pkvec` store (the storage-boundary
///   guard: the on-disk store carries only u64s). A crash after a chunk's vectors are
///   appended but before its SQLite rows / cursor advance leaves a key on the store while
///   `needs_embedding` flips true again; a key in this set is never re-appended, so a resume cannot
///   double a vector. Loaded from the store on resume/incremental AND accumulated this run.
/// - `persisted_hashes` — the FULL `content_hash`es embedded THIS run (the WORK identity):
///   the work-dedup keys on the full hash, NOT the truncated u64, so two DISTINCT pages whose hashes
///   collide on the first 8 bytes (~2.8e-6 at 14.4M) are EACH embedded rather than the second
///   silently collapsing onto the first's vector. This set is run-scoped (never loaded from disk,
///   since the on-disk store cannot recover the full hash).
/// - `mapped_ids` — the `history_id`s already in the `.pkmap` visit map. A crash mid-chunk can leave
///   some visits mapped; a visit in this set is never re-appended, so a resume never doubles a
///   mapping entry. Loaded from the map on resume/incremental AND accumulated this run.
///
/// [`load_existing`](Self::load_existing) populates the on-disk sets whenever an existing
/// store is present and NOT about to be wiped — not only on a `start_history_id > 0` resume. So a
/// PLAIN incremental pass (cursor 0, empty sets without this) does not re-embed + append a duplicate
/// `.pkvec` record for a NEW visit of an already-embedded page; it maps the visit but skips the embed.
pub(super) struct DedupTracker {
    /// u64 content_keys already on the `.pkvec` store (resume backstop + this run's appends).
    persisted_keys: HashSet<u64>,
    /// FULL content_hashes embedded this run (the work identity; never loaded from disk).
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

    /// Loads the resume/incremental on-disk dedup state when an existing store is present.
    ///
    /// Populates `persisted_keys` from the store's `existing_ids` and `mapped_ids` from the visit
    /// map's `mapped_history_ids` ONLY when `!new_source && store.exists()` — mirroring the loop's
    /// gate exactly. A true full rebuild (`new_source`) or a never-built store leaves the sets
    /// empty so the build starts clean. `persisted_hashes` is NEVER loaded: the on-disk store carries
    /// only the truncated u64, so the full hash is unrecoverable from it; the full-hash
    /// authority is built up this run via [`Self::record_embedded`].
    pub(super) fn load_existing(
        &mut self,
        store: &VectorStore,
        visit_map: &VisitContentMap,
        new_source: bool,
    ) -> Result<()> {
        if !new_source && store.exists() {
            self.persisted_keys = store.existing_ids()?;
            self.mapped_ids = visit_map.mapped_history_ids()?;
        }
        Ok(())
    }

    /// Selects, in first-seen order, the unique pages among `changed` still needing an embed.
    ///
    /// Delegates to the pure [`super::candidates::select_embed_targets`] with this tracker's
    /// encapsulated `persisted_hashes` + `persisted_keys`, so the production loop and the
    /// direct-collision unit test share one decision.
    pub(super) fn select_targets<'a>(&self, changed: &[&'a IndexedVisit]) -> Vec<&'a IndexedVisit> {
        select_embed_targets(changed, &self.persisted_hashes, &self.persisted_keys)
    }

    /// Records that one page (its full hash + u64 key) was embedded + persisted this run.
    ///
    /// After this, the page's full hash skips it run-wide (the work identity) and its u64 key
    /// skips it at the storage boundary, so a later chunk or a resume never re-embeds it.
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

/// The max candidate `history_id` in the archive right now — the determinate scan denominator.
///
/// One cheap `MAX` over the indexed `archive.visits` PK (the same `reverted_at IS NULL` predicate
/// [`collect_visit_chunk`](super::candidates::collect_visit_chunk) scans), so `next_history_id /
/// scan_target` is honest determinate progress. Captured ONCE at a build's true start; rows that
/// arrive afterward are a later incremental pass's work, not this build's. Returns 0 for an empty
/// archive (nothing to scan).
fn max_candidate_history_id(connection: &Session<'_>) -> Result<i64> {
    connection
        .query_one(
            "SELECT COALESCE(MAX(visits.id), 0)
             FROM archive.visits AS visits
             WHERE visits.reverted_at IS NULL",
            (),
            |row| Ok(row.get(0)?),
        )
        .context("reading the max candidate history id for the index scan target")
}

/// Resolves the scan denominator to REPORT this run: the captured max on a fresh build, else 0.
///
/// On the TRUE start of a build (`start_history_id == 0`) it captures
/// [`max_candidate_history_id`] so the cursor records the determinate denominator. On a RESUME
/// (`start_history_id > 0`) it returns 0 ("unknown — preserve the stored target"), so
/// [`crate::ai_queue::persist_index_cursor`] keeps the original `scan_target` rather than recomputing
/// against a corpus that may have grown under the build. Pure decision over `start_history_id`; the
/// fresh branch is the only one that touches the DB.
fn scan_target_for(connection: &Session<'_>, start_history_id: i64) -> Result<i64> {
    if start_history_id > 0 {
        return Ok(0);
    }
    max_candidate_history_id(connection)
}

/// Counts visible visits missing metadata for this exact pair. An empty index counts every
/// visit; a partial index counts only its missing visits; a rebuild, which reads every visit into
/// its lineage, counts them all. This runs once in the background, never during status polling.
/// Durable cursor progress preserves the target across resumes.
fn candidate_visit_count(
    connection: &Session<'_>,
    provider_id: &str,
    model: &str,
    rebuild: bool,
    lineage: &str,
) -> Result<u64> {
    let count: i64 = connection
        .query_one(
            "SELECT COUNT(*)
             FROM archive.visits AS visits
             WHERE visits.reverted_at IS NULL
               AND (?3 = 1 OR NOT EXISTS (
                 SELECT 1 FROM ai_embeddings AS e
                 WHERE e.provider_id = ?1 AND e.model = ?2 AND e.lineage=?4 AND e.history_id = visits.id
               ))",
            params![provider_id, model, i64::from(rebuild), lineage],
            |row| Ok(row.get(0)?),
        )
        .context("counting candidate pages needing embedding for the index embed target")?;
    Ok(count.max(0) as u64)
}

/// Resolves the embed-progress denominator to REPORT this run: the captured candidate count on a
/// fresh build, else 0 ("unknown — preserve the stored target"). Mirrors [`scan_target_for`] so a
/// resume never recounts a corpus that grew (or got partially embedded) under the build.
fn embed_target_for(
    connection: &Session<'_>,
    provider_id: &str,
    model: &str,
    start_history_id: i64,
    rebuild: bool,
    lineage: &str,
) -> Result<u64> {
    if start_history_id > 0 {
        return Ok(0);
    }
    candidate_visit_count(connection, provider_id, model, rebuild, lineage)
}

/// Embeds candidate rows in resumable, cancellable chunks and persists them to the vector plane.
///
/// The hot path of W-AI-4 (02 §C.6 R1). For each ascending-`history_id` chunk from
/// `start_history_id`: builds content, diffs the content hash to skip unchanged rows, embeds the
/// changed rows under [`EmbeddingRole::Document`] through the external provider, writes the vectors
/// to the [`VectorStore`] AND the Turso metadata, then advances the watermark via
/// the `ledger` so a restart resumes from the next id (not from scratch). Cancellation is checked
/// at each chunk boundary, where on-disk state is consistent. The vector store is lazily created
/// once the real dim is observed (D4: never assume a dim before a vector is returned).
///
/// Correctness 鐵律 (02 §C.3): `effective_dim` is read from the actual returned vector length; the
/// provider L2-normalizes defensively; the document role threads through. The fingerprint stamped
/// on the store records that truth (dtype/normalized set by the external adapter, A-S2 fix).
///
/// Every metadata lookup and write uses the source lineage. Published generations remain immutable.
#[allow(clippy::too_many_arguments)]
async fn run_embedding_backfill(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    run_control: Option<&Arc<dyn AiRunControl>>,
    start_history_id: i64,
    start_embedded_so_far: u64,
    start_updated_so_far: u64,
    ledger: Option<&Arc<dyn IndexBackfillLedger>>,
    new_source: bool,
    rebuild: bool,
    lineage: &str,
    captured_upper: Option<i64>,
) -> Result<BackfillOutcome> {
    let paths = vault.paths();

    // Config-driven engine selection: the in-app static model or an external `/v1/embeddings`.
    // The loop below is engine-agnostic.
    let embedder = super::super::embedding_engine::select_embedding_provider(paths, provider)?;
    // WorkingSet enumerates every eligible canonical page through cursors. Incremental/Full
    // visit every captured candidate; Full writes them into its empty lineage.
    let working_set_filter = working_set_canonical_urls(vault, request.scope)?;
    let limit = request.limit.map(|value| value as usize);
    let mut outcome = BackfillOutcome {
        indexed_items: start_embedded_so_far as usize,
        updated_items: start_updated_so_far as usize,
        ..Default::default()
    };
    // The store is resolved lazily on the first chunk that actually embeds rows, once the real dim
    // is known (D4) — see `vector_store_for_chunk`. Until then no store handle is held.
    let mut store: Option<VectorStore> = None;
    let mut source_directories_synced = false;
    let visit_map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    // The map is created up front so the per-chunk visit appends always have a header to append to.
    // A full rebuild's new lineage has none yet, so this writes a fresh map; on a resume it leaves the
    // existing map (and its prior visit rows) intact.
    visit_map.ensure_created(paths)?;
    let upper_history_id = if let Some(upper) = captured_upper {
        upper
    } else {
        vault.read(Db::Archive, |s| {
            Ok(s.query_one("SELECT COALESCE(MAX(id),0) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?)
        })?
    };
    let mut cursor = start_history_id;
    // CUMULATIVE across resumes: seed from the cursor's stored `embedded_so_far` so the persisted
    // count (and the `embedded / embed_target` progress bar) keeps climbing after a reclaim instead of
    // collapsing to this session's partial count. A fresh build passes 0.
    let mut embedded_total: u64 = start_embedded_so_far;
    // The determinate scan denominator the Activity page divides by (Change 1). Captured ONCE here, at
    // the TRUE start of a build, and reported on every chunk's progress; a resume reports 0 so the
    // stored target is preserved (see `scan_target_for` / `persist_index_cursor`).
    let scan_target =
        vault.read(Db::Intelligence, move |s| scan_target_for(s, start_history_id))?;
    // Count visits still missing from this pair before the first request. Resume keeps its target.
    let provider_id = provider.config.id.clone();
    let model = provider.config.default_model.clone();
    let build_lineage = lineage.to_owned();
    let embed_target = vault.read(Db::Intelligence, move |s| {
        embed_target_for(s, &provider_id, &model, start_history_id, rebuild, &build_lineage)
    })?;
    // The cursor reflecting the last chunk whose vectors+map+Turso are ALL durably persisted (F2.1).
    // It NEVER advances past a chunk that has not been fully persisted, so persisting it on the failure
    // path resumes from REAL progress instead of the origin — and never SKIPS an un-persisted chunk
    // (which advancing the scan `cursor` before the embed would do). Starts at the resume watermark.
    let mut last_durable = crate::ai::IndexBackfillProgress {
        next_history_id: start_history_id,
        embedded_so_far: start_embedded_so_far,
        updated_so_far: start_updated_so_far,
        scan_target,
        embed_target,
    };

    if let Some(ledger) = ledger {
        ledger.record(last_durable)?;
    }

    // The crash-window dedup state is encapsulated in
    // `DedupTracker`. `load_existing` populates the on-disk sets whenever a store/map already exists
    // and is NOT about to be wiped — so a PLAIN incremental pass (cursor 0) maps a re-visit of an
    // already-embedded page instead of re-embedding it, and a resume never doubles a
    // vector or a mapping entry. See `DedupTracker` for the full invariant rationale.
    let mut tracker = DedupTracker::new();
    let existing_store =
        VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model);
    if !new_source {
        // A crash mid-append leaves half a record at the end of either file; until it is cut off,
        // every incremental or resumed build fails reading them.
        existing_store.trim_torn_tail()?;
        visit_map.trim_torn_tail()?;
    }
    tracker.load_existing(&existing_store, &visit_map, new_source)?;

    // Run the chunked embed loop inside a Result-returning scope so an error anywhere inside it first
    // gets a chance to checkpoint the LAST-DURABLE cursor (F2.1) before propagating — a reclaim then
    // resumes from real progress, never the origin, and never past an un-persisted chunk.
    let backfill_result: Result<()> = async {
        loop {
            checkpoint_ai_run(run_control, "Index build was cancelled before the next chunk.")?;
            let remaining = limit;
            let mut chunk = collect_visit_chunk(
                vault,
                provider,
                cursor,
                upper_history_id,
                chunk_size(remaining),
                lineage,
            )?;
            // Older rebuild copies shared provenance with live files. A migrated row is not
            // evidence of source bytes: repair missing mappings/vectors before trusting its hash.
            for visit in &mut chunk {
                visit.needs_embedding |= !tracker.mapped_ids.contains(&visit.history_id)
                    || !tracker.persisted_keys.contains(&visit.content_key);
            }
            let Some(last_history_id) = chunk.last().map(|visit| visit.history_id) else {
                break; // No more candidate rows.
            };
            // Advance the scan cursor past this chunk regardless of how many rows were changed, so the
            // next pass never re-scans this id range.
            cursor = last_history_id + 1;

            let changed: Vec<&IndexedVisit> = chunk
                .iter()
                .filter(|visit| rebuild || visit.needs_embedding)
                // WorkingSet scope: keep only candidates whose page is in the bounded working set. A
                // `None` filter (Incremental/Full) keeps everything. Pages outside the set are counted as
                // skipped below, exactly like an unchanged page — the loop body is otherwise unchanged.
                .filter(|visit| visit_in_working_set(&working_set_filter, &visit.url))
                .collect();
            outcome.skipped_items += chunk.len() - changed.len();

            if !changed.is_empty() {
                // DEDUP: collect the UNIQUE pages among the changed visits that are not already embedded,
                // and embed each ONCE (the page identity is the FULL `content_hash`).
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
                    // `new_source` (NOT `request.full_rebuild`) drives the store reset: a full_rebuild
                    // RESUME appends to the partial store rather than truncating it. The
                    // fingerprint is stamped from the SELECTED engine's real descriptor (static = Mean,
                    // external = Unknown), so two engines never share a fingerprint.
                    let store = vector_store_for_chunk(
                        &mut store,
                        paths,
                        provider,
                        &embedder.descriptor(),
                        effective_dim,
                        new_source,
                    )?;
                    // Every record here is a page we have NOT embedded this run (the full-hash dedup above
                    // guaranteed it), so all are appended. Record each page's FULL hash + u64 key so a
                    // later chunk / resume skips it (the full hash is the work identity; the u64
                    // is the storage-boundary backstop). The store accepts duplicate u64s for the rare
                    // collision; `read_all` last-writer-wins resolves that storage-boundary case.
                    for (visit, (key, _)) in embed_targets.iter().zip(records.iter()) {
                        tracker.record_embedded(*key, &visit.content_hash);
                    }
                    // Persist vectors FIRST so the watermark we report is always backed by on-disk data.
                    store.append_vectors(&records)?;
                    // Count the vectors that actually landed on `.pkvec` (F3 0-byte honesty): the build's
                    // honesty note + the empty-store guard read this, not the per-visit `indexed_items`.
                    outcome.embedded_vectors += records.len() as u64;
                }

                // Map EVERY changed visit to its content_key (skip ones a prior crash already mapped), so
                // search/heavy-tier can fan a deduped vector out to all its visits. Turso metadata is
                // (re-)written for every visit whose row was missing or out of date, so its dedup hash
                // catches up and it is not re-scanned; a rebuild leaves the rows that are still true.
                // Both are per-visit; only the VECTOR is deduped.
                let map_records = tracker.take_unmapped(&changed);
                visit_map.append(&map_records)?;

                // Provenance/cursor commits must never get ahead of source bytes after power loss.
                // Append writers flush their buffers; pay the durable barrier before DB progress.
                if existing_store.exists() {
                    crate::durable_io::fsync_file_durably(existing_store.path())?;
                }
                crate::durable_io::fsync_file_durably(visit_map.path())?;
                if !source_directories_synced {
                    let mut directory = Some(paths.vectors_dir.as_path());
                    for _ in 0..4 {
                        if let Some(dir) = directory {
                            crate::durable_io::fsync_dir(dir)?;
                            directory = dir.parent();
                        }
                    }
                    source_directories_synced = true;
                }
                let indexed_at = now_rfc3339();
                let visits = changed
                    .iter()
                    .filter(|visit| visit.needs_embedding)
                    .map(|visit| (**visit).clone())
                    .collect::<Vec<_>>();
                let provider = provider.clone();
                let lineage = lineage.to_owned();
                let job_id = ledger.and_then(|ledger| ledger.job_id());
                let embedded_so_far = embedded_total + changed.len() as u64;
                let updated_so_far = outcome.updated_items as u64;
                let updated = vault.write(Db::Intelligence, Priority::Batch, move |s| {
                    let mut updated = 0;
                    for visit in &visits {
                        updated += usize::from(upsert_embedding(
                            s,
                            &provider,
                            visit,
                            &indexed_at,
                            &lineage,
                        )?);
                    }
                    if let Some(job_id) = job_id {
                        // Sources are already durable. Provenance, cursor and job counters share
                        // this commit, so a crash before the worker callback cannot lose counts.
                        let cursor = crate::ai_queue::IndexBackfillCursor {
                            lineage: Some(lineage),
                            next_history_id: cursor,
                            embedded_so_far,
                            updated_so_far: updated_so_far + updated as u64,
                            scan_target,
                            embed_target,
                        };
                        let persisted =
                            crate::ai_queue::store::persist_index_cursor(s, job_id, &cursor, None)?;
                        anyhow::ensure!(
                            persisted == crate::ai_queue::CursorPersistOutcome::Persisted,
                            "index job lost its queue lease"
                        );
                    }
                    Ok(updated)
                })?;
                outcome.indexed_items += changed.len();
                outcome.updated_items += updated;
                embedded_total += changed.len() as u64;
            }

            // This chunk's vectors + map + Turso are now ALL on disk, so the cursor past it is durable.
            // Record it as the resume watermark AND as the last-durable checkpoint so a later failure
            // re-persists this position (never the origin, never the un-persisted next chunk).
            last_durable = crate::ai::IndexBackfillProgress {
                next_history_id: cursor,
                embedded_so_far: embedded_total,
                updated_so_far: outcome.updated_items as u64,
                scan_target,
                embed_target,
            };
            if let Some(ledger) = ledger {
                ledger.record(last_durable)?;
            }

            if chunk.len() < chunk_size(remaining) {
                break; // Final (short) chunk: no more rows beyond this id range.
            }
        }
        Ok(())
    }
    .await;

    if let Err(error) = backfill_result {
        // F2.1: persist the LAST-DURABLE cursor before surfacing the failure, so a worker that
        // reclaims this job resumes from the rows already on disk instead of restarting at the origin
        // (the stuck "resume writes 0 / never recovers" bug). Best-effort: a secondary persist error
        // must not mask the original failure, and a lease-loss persist is itself the abort signal.
        if let Some(ledger) = ledger {
            let _ = ledger.record(last_durable);
        }
        return Err(error);
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
    new_source: bool,
) -> Result<&'a VectorStore> {
    if store.is_none() {
        // Stamp the fingerprint from the SELECTED engine's real descriptor (its true dtype /
        // normalized / pooling / instruction, A-S2 fix) with the observed dim, but key it by the
        // provider CONFIG identity so the on-disk store name + Turso ledger stay in sync. This is
        // what keeps a static-built and an external-built index distinct at the same dim while
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
        let created = if new_source || !existing.exists() {
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

/// Bind before scanning, including runs that stop before their first chunk. A lineage switch
/// invalidates positions, counts and denominators together; the normal candidate scan fills needs.
fn bind_cursor_lineage(
    vault: &crate::Vault,
    job_id: Option<i64>,
    lineage: &str,
    fresh: bool,
) -> Result<(bool, u64)> {
    let Some(job_id) = job_id else { return Ok((!fresh, 0)) };
    let lineage = lineage.to_owned();
    vault.write(vault_store::Db::Intelligence, vault_store::Priority::Batch, move |s| {
        let raw = s.query_one(
            "SELECT payload_json FROM ai_jobs WHERE id=?1 AND state='running'",
            [job_id],
            |r| Ok(r.get::<String>(0)?),
        )?;
        let mut payload: crate::ai_queue::AiJobPayload = serde_json::from_str(&raw)?;
        let crate::ai_queue::AiJobPayload::Index { cursor, .. } = &mut payload else {
            anyhow::bail!("index job payload changed")
        };
        let resume = !fresh && cursor.lineage.as_deref() == Some(lineage.as_str());
        if !resume {
            *cursor = crate::ai_queue::IndexBackfillCursor {
                lineage: Some(lineage),
                ..Default::default()
            };
        }
        let cursor_updated = cursor.updated_so_far;
        s.execute(
            "UPDATE ai_jobs SET payload_json=?2 WHERE id=?1 AND state='running'",
            vault_store::params_owned![job_id, serde_json::to_string(&payload)?],
        )?;
        Ok((resume, cursor_updated))
    })
}
