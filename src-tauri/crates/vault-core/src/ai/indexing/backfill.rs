//! Failure modes: batch limits truncate coverage or a crash repeats completed embeddings.
//! The resumable embed loop and the crash-window dedup state that guards it (W-AI-4 / 02 §C.6 R1).
//!
//! ## Responsibilities
//! - drive the build orchestration ([`build_ai_index_with_control`]): run-ledger bookkeeping, the
//!   one-shot full-rebuild wipe, the embed backfill, and the derived-plane re-projection
//! - embed candidate rows in resumable, cancellable chunks ([`run_embedding_backfill`]) and persist
//!   them to the `.pkvec` store + `.pkmap` visit map + Turso metadata rows
//! - retain explicit serving pairs until a replacement completes
//! - encapsulate the no-dup/no-miss crash-window invariants in [`DedupTracker`] so they are tested
//!   directly rather than only through end-to-end resume tests
//!
//! ## Not responsible for
//! - candidate collection / embed-target selection (`super::candidates` owns those)
//! - the vector store / visit map byte format (`super::super::vector_store` / `visit_content_map`)

use super::super::native_ledger::{
    begin_ai_run, current_source_watermark, end_failed_ai_run, finalize_ai_run_success,
    record_index_ledger_failure, record_index_ledger_start, record_index_ledger_success,
};
use super::super::*;
use super::candidates::{collect_visit_chunk, select_embed_targets};
use super::store_rows::{
    chunk_size, cleanup_stale_embeddings, clear_provider_embeddings, upsert_embedding,
    validate_embedding_batch_for_keys,
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
    let paths = vault.paths();
    validate_provider(provider, AiProviderPurpose::Embedding)?;
    let started_at = now_rfc3339();
    let source_watermark = current_source_watermark(vault)?;
    let sidecar_table =
        ai_sidecar::provider_table_name(&provider.config.id, &provider.config.default_model);
    let run_id = begin_ai_run(
        vault,
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
    if let Err(error) = record_index_ledger_start(
        vault,
        provider,
        run_id,
        &started_at,
        source_watermark,
        &sidecar_table,
        request,
    ) {
        end_failed_ai_run(vault, run_id, &error, json!({}));
        return Err(error);
    }

    // A full rebuild's destructive wipe is a ONE-SHOT that runs only at the TRUE start of the job
    // (the persisted cursor is still at the origin). On any resume (`start_history_id > 0`) a worker
    // restart MUST NOT re-clear the partially-rebuilt store/metadata, or every row already embedded
    // below the cursor is silently lost while the job still reports success (02 §C.6 R1).
    // A resume appends to the existing store. Keep full-rebuild intent in the job so completion
    // can retire the previous serving pair, while this durable cursor gates the destructive wipe.
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

    let result: Result<AiIndexReport> = async {
        let run_control = run_control.as_ref();
        checkpoint_ai_run(run_control, "Index build was cancelled before collecting stale rows.")?;
        if perform_wipe {
            clear_provider_embeddings(vault, provider, run_control)?;
            super::super::search_vectors::delete(paths, provider)?;
        }
        let removed_items = cleanup_stale_embeddings(vault, provider, run_control)?;
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
            // The keyed reverse/forward sidecars (`.pkrev`/`.pkfwd`) are pure projections of the
            // `.pkmap` the wipe just cleared, so they are deleted too (a later build re-projects them).
            super::super::reverse_visit_map::ReverseVisitMap::for_provider(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )
            .delete()?;
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
        // backfill appends to it instead of deleting the rows it already embedded. The
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
            // the end anyway, and the partial `.pkvec` survives).
            super::super::vector_planes::delete_planes(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )?;
            // Drop the keyed reverse/forward sidecars (`.pkrev`/`.pkfwd`) alongside the planes so
            // the rebuild window never leaves them projected from the old (now-deleted) `.pkmap`; they
            // re-project after the backfill rewrites the `.pkmap` (same posture as the planes).
            super::super::reverse_visit_map::ReverseVisitMap::for_provider(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )
            .delete()?;
        }

        checkpoint_ai_run(run_control, "Index build was cancelled before collecting candidates.")?;
        let outcome = run_embedding_backfill(
            vault,
            provider,
            request,
            run_control,
            start_history_id,
            start_embedded_so_far,
            ledger.as_ref(),
            perform_wipe,
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
        // just the newly-embedded rows. After the streaming projection this is now RAM-bounded
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

        // Re-project the keyed reverse/forward sidecars (`.pkrev`/`.pkfwd`) from the now-current
        // `.pkmap`, stamped with the SAME `.pkvec` fingerprint hash so they share the planes' staleness
        // identity. These turn the always-on hydration join (content_key → visits) and the `is:starred`
        // forward join (history_id → content_key) into BOUNDED binary-search lookups (O(k'·log n) /
        // O(starred·log n)) instead of the O(n) `.pkmap` strides. RESUMABLE/RESTART-SAFE: the `.pkmap`
        // is the source of truth, so re-projecting at the end of any pass (fresh / resume / incremental)
        // reads the current map and yields the same correct sidecars (no dup/no miss). A missing/absent
        // `.pkvec` (nothing ever embedded) projects empty (header-only) sidecars so search degrades to
        // "no semantic matches" rather than a stale/absent-sidecar error.
        checkpoint_ai_run(
            run_control,
            "Index build was cancelled before projecting keyed reverse sidecars.",
        )?;
        rebuild_reverse_sidecars(paths, provider)?;
        let embedded_searches =
            super::super::search_vectors::backfill(vault, provider, run_control).await?;
        checkpoint_ai_run(run_control, "Index build stopped before completion.")?;

        let mut notes = if outcome.indexed_items == 0 {
            vec!["No new or changed history rows required indexing.".to_string()]
        } else if outcome.embedded_vectors == 0 {
            // Changed visits were processed but every page was already embedded (a re-visit / pure
            // incremental map), so NO new vector landed. Say so honestly rather than implying an embed
            // (F3): the vectors these visits map to already live on the plane.
            vec![format!(
                "Mapped {} history row(s) to existing vectors; no new embeddings were needed.",
                outcome.indexed_items
            )]
        } else {
            vec![format!(
                "Embedded {} page vector(s) for {} history row(s) into the vector plane (dim {}).",
                outcome.embedded_vectors,
                outcome.indexed_items,
                outcome.effective_dim.unwrap_or_default()
            )]
        };
        notes.push(format!("Embedded {embedded_searches} distinct keyword search vector(s)."));
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
        // The ledger first: the run is done only once the index says how far it got.
        Ok(report) => match record_index_ledger_success(
            vault,
            provider,
            run_id,
            &report.last_indexed_at,
            source_watermark,
            &sidecar_table,
            request,
            ledger.as_ref().and_then(|ledger| ledger.job_id()),
        )
        .and_then(|()| {
            finalize_ai_run_success(
                vault,
                run_id,
                json!({
                    "providerId": report.provider_id,
                    "model": report.model,
                    "indexedItems": report.indexed_items,
                    "updatedItems": report.updated_items,
                    "removedItems": report.removed_items,
                }),
            )
        }) {
            Ok(()) => Ok(report),
            Err(error) => {
                end_failed_ai_run(vault, run_id, &error, json!({}));
                let _ = record_index_ledger_failure(
                    vault,
                    provider,
                    run_id,
                    source_watermark,
                    &sidecar_table,
                    request,
                    &format!("{error:#}"),
                );
                Err(error)
            }
        },
        Err(error) => {
            end_failed_ai_run(
                vault,
                run_id,
                &error,
                json!({
                    "providerId": provider.config.id,
                    "model": provider.config.default_model,
                    "fullRebuild": request.full_rebuild,
                    "clearOnly": request.clear_only,
                }),
            );
            if let Err(recording) = record_index_ledger_failure(
                vault,
                provider,
                run_id,
                source_watermark,
                &sidecar_table,
                request,
                &format!("{error:#}"),
            ) {
                eprintln!(
                    "pathkeep: index run {run_id}: recording its failure failed: {recording:#}"
                );
            }
            Err(error)
        }
    }
}

/// Deletes pairs outside the caller's retention set, including their readiness ledger.
///
/// The worker retains chosen and serving at job start, then only chosen after a full rebuild.
/// Deletes metadata in batches and files off the UI thread.
pub async fn forget_unconfigured_indexes(
    vault: &crate::Vault,
    configured: &[(String, String)],
    control: Option<&Arc<dyn AiRunControl>>,
) -> Result<usize> {
    let indexed = vault.read(Db::Intelligence, |s| {
        Ok(s.query("SELECT provider_id,model FROM ai_embedding_totals UNION SELECT provider_id,model FROM ai_index_ledger", (), |r| {
            Ok((r.get::<String>(0)?, r.get::<String>(1)?))
        })?)
    })?;
    let paths = vault.paths().clone();
    let mut forgotten = 0;
    for (provider_id, model) in indexed {
        if configured.iter().any(|(id, kept)| *id == provider_id && *kept == model) {
            continue;
        }
        let gone = AiProviderRuntime {
            config: AiProviderConfig {
                id: provider_id.clone(),
                default_model: model.clone(),
                purpose: AiProviderPurpose::Embedding,
                ..AiProviderConfig::default()
            },
            api_key: None,
        };
        let (id, model_name) = (provider_id.clone(), model.clone());
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            s.execute(
                "UPDATE ai_index_ledger SET state='incomplete' WHERE provider_id=?1 AND model=?2",
                params![id, model_name],
            )?;
            Ok(())
        })?;
        clear_provider_embeddings(vault, &gone, control)?;
        super::super::search_vectors::delete(&paths, &gone)?;
        ai_sidecar::clear_provider_embeddings(&paths, &provider_id, &model).await?;
        VectorStore::for_provider(&paths, &provider_id, &model).delete()?;
        VisitContentMap::for_provider(&paths, &provider_id, &model).delete()?;
        super::super::vector_planes::delete_planes(&paths, &provider_id, &model)?;
        super::super::reverse_visit_map::ReverseVisitMap::for_provider(
            &paths,
            &provider_id,
            &model,
        )
        .delete()?;
        let (id, kept) = (provider_id.clone(), model.clone());
        vault.write(Db::Intelligence, Priority::Batch, move |s| {
            s.execute(
                "DELETE FROM ai_embedding_totals WHERE provider_id=?1 AND model=?2",
                params![id, kept],
            )?;
            s.execute(
                "DELETE FROM ai_index_ledger WHERE provider_id=?1 AND model=?2",
                params![id, kept],
            )?;
            Ok(())
        })?;
        forgotten += 1;
    }
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
    /// map's `mapped_history_ids` ONLY when `!perform_wipe && store.exists()` — mirroring the loop's
    /// gate exactly. A true full rebuild (`perform_wipe`) or a never-built store leaves the sets
    /// empty so the build starts clean. `persisted_hashes` is NEVER loaded: the on-disk store carries
    /// only the truncated u64, so the full hash is unrecoverable from it; the full-hash
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
/// visit; a partial index counts only its missing visits. This runs once in the background,
/// never during status polling. Durable cursor progress preserves the target across resumes.
fn candidate_visit_count(connection: &Session<'_>, provider_id: &str, model: &str) -> Result<u64> {
    let count: i64 = connection
        .query_one(
            "SELECT COUNT(*)
             FROM archive.visits AS visits
             WHERE visits.reverted_at IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM ai_embeddings AS e
                 WHERE e.provider_id = ?1 AND e.model = ?2 AND e.history_id = visits.id
               )",
            params![provider_id, model],
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
) -> Result<u64> {
    if start_history_id > 0 {
        return Ok(0);
    }
    candidate_visit_count(connection, provider_id, model)
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
#[allow(clippy::too_many_arguments)]
async fn run_embedding_backfill(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    run_control: Option<&Arc<dyn AiRunControl>>,
    start_history_id: i64,
    start_embedded_so_far: u64,
    ledger: Option<&Arc<dyn IndexBackfillLedger>>,
    perform_wipe: bool,
) -> Result<BackfillOutcome> {
    let paths = vault.paths();

    // Config-driven engine selection: the in-app static model or an external `/v1/embeddings`.
    // The loop below is engine-agnostic.
    let embedder = super::super::embedding_engine::select_embedding_provider(paths, provider)?;
    // WorkingSet enumerates every eligible canonical page through cursors. Incremental/Full
    // visit every captured candidate; Full performs its one-time wipe in the caller.
    let working_set_filter = working_set_canonical_urls(vault, request.scope)?;
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
    let upper_history_id = vault.read(Db::Archive, |s| {
        Ok(s.query_one("SELECT COALESCE(MAX(id),0) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?)
    })?;
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
    let embed_target = vault.read(Db::Intelligence, move |s| {
        embed_target_for(s, &provider_id, &model, start_history_id)
    })?;
    // The cursor reflecting the last chunk whose vectors+map+Turso are ALL durably persisted (F2.1).
    // It NEVER advances past a chunk that has not been fully persisted, so persisting it on the failure
    // path resumes from REAL progress instead of the origin — and never SKIPS an un-persisted chunk
    // (which advancing the scan `cursor` before the embed would do). Starts at the resume watermark.
    let mut last_durable = crate::ai::IndexBackfillProgress {
        next_history_id: start_history_id,
        embedded_so_far: start_embedded_so_far,
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
    if !perform_wipe {
        // A crash mid-append leaves half a record at the end of either file; until it is cut off,
        // every incremental or resumed build fails reading them.
        existing_store.trim_torn_tail()?;
        visit_map.trim_torn_tail()?;
    }
    tracker.load_existing(&existing_store, &visit_map, perform_wipe)?;

    // Run the chunked embed loop inside a Result-returning scope so an error anywhere inside it first
    // gets a chance to checkpoint the LAST-DURABLE cursor (F2.1) before propagating — a reclaim then
    // resumes from real progress, never the origin, and never past an un-persisted chunk.
    let backfill_result: Result<()> = async {
        loop {
            checkpoint_ai_run(run_control, "Index build was cancelled before the next chunk.")?;
            let remaining = limit;
            let chunk = collect_visit_chunk(
                vault,
                provider,
                cursor,
                upper_history_id,
                chunk_size(remaining),
            )?;
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
                    // `perform_wipe` (NOT `request.full_rebuild`) drives the store reset: a full_rebuild
                    // RESUME appends to the partial store rather than truncating it. The
                    // fingerprint is stamped from the SELECTED engine's real descriptor (static = Mean,
                    // external = Unknown), so two engines never share a fingerprint.
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
                // (re-)written for every changed visit so its dedup hash catches up and it is not
                // re-scanned. Both are per-visit; only the VECTOR is deduped.
                let map_records = tracker.take_unmapped(&changed);
                visit_map.append(&map_records)?;

                let indexed_at = now_rfc3339();
                let visits = changed.iter().map(|visit| (**visit).clone()).collect::<Vec<_>>();
                let provider = provider.clone();
                let updated = vault.write(Db::Intelligence, Priority::Batch, move |s| {
                    let mut updated = 0;
                    for visit in &visits {
                        updated += usize::from(upsert_embedding(s, &provider, visit, &indexed_at)?);
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
    full_rebuild: bool,
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

/// (Re)projects the keyed reverse/forward sidecars (`.pkrev`/`.pkfwd`) from the current `.pkmap`.
///
/// Reads the `.pkvec` store's fingerprint hash (the planes' staleness identity) and projects both
/// sorted sidecars from the authoritative `.pkmap`, stamped with that hash so they share the planes'
/// staleness identity and re-project on a model/dim/dtype change. When no `.pkvec` exists yet (nothing
/// ever embedded), it projects empty (header-only) sidecars under an empty fingerprint — the same
/// "present but empty" posture `build_planes_from_store` takes — so the read path loads cleanly to "no
/// semantic matches" rather than treating an absent sidecar as an error. Idempotent: re-running over
/// the same `.pkmap` yields byte-identical sidecars (no dup/no miss), which is what makes a resumed or
/// stale-rebuild pass safe.
fn rebuild_reverse_sidecars(paths: &ProjectPaths, provider: &AiProviderRuntime) -> Result<()> {
    let store =
        VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model);
    let fingerprint_hash =
        store.read_header()?.map(|header| header.fingerprint_hash).unwrap_or_default();
    let visit_map =
        VisitContentMap::for_provider(paths, &provider.config.id, &provider.config.default_model);
    let sidecars = super::super::reverse_visit_map::ReverseVisitMap::for_provider(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    );
    sidecars.build_from_visit_map(paths, &visit_map, &fingerprint_hash)?;
    Ok(())
}
