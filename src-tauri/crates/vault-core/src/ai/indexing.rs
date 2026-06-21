//! Semantic index build and ledger persistence.
//!
//! ## Responsibilities
//! - build or clear the semantic sidecar for one embedding provider
//! - keep the AI index ledger and unified `runs` trace in sync with build outcomes
//! - collect changed canonical visits and write compatibility metadata rows
//! - expose storage/watermark helpers consumed by AI read models
//!
//! ## Not responsible for
//! - assistant prompt orchestration or semantic result ranking
//! - provider client setup and request-format-specific embedding execution
//! - Settings-facing read-model assembly
//!
//! ## Dependencies
//! - `super::control` for cooperative cancellation checkpoints
//! - `super::provider` for embedding retries and provider validation
//! - `crate::ai_sidecar` for optional vector sidecar synchronization
//!
//! ## Performance notes
//! - candidate collection diffs content hashes before embedding, which avoids re-embedding
//!   unchanged visits on large archives
//! - SQLite lookups chunk `history_id` predicates with `SQLITE_BATCH_SIZE` to keep
//!   statement size and memory bounded on 14.4M+ histories

use super::*;

/// One canonical visit that needs to be embedded into the semantic sidecar.
///
/// The index builder keeps this shape separate from `HistoryEntry` so it can carry the
/// normalized embedding payload and the dedup identity used for incremental change detection.
///
/// W-AI-4c dedup: `content_hash` is now the VISIT-INDEPENDENT dedup hash (canonical URL + title +
/// reserved enrichment summary, see [`super::dedup::build_dedup_content_hash`]), and `content_key`
/// is its u64 vector-store key — so many visits of one page share both and need ONE embedding.
/// `content` is the per-visit TEXT fed to the model (still via the enrichment funnel); two visits
/// sharing a content_key embed identical-enough text, so embedding the first and mapping the rest is
/// exact.
#[derive(Debug, Clone)]
pub(super) struct IndexedVisit {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub content: String,
    /// Visit-independent dedup hash (the page identity); equal across all visits of one page.
    pub content_hash: String,
    /// The u64 vector-store key derived from `content_hash` (the dedup key).
    pub content_key: u64,
    /// Whether the stored content hash differs from the freshly built one, so this visit's CONTENT
    /// must be (re-)embedded. `false` lets the backfill skip an unchanged row without an embed call.
    /// With dedup, the FIRST visit of an un-embedded page sets this; later visits of the SAME page in
    /// the same scan are skipped via the content_key-already-embedded check, not this flag.
    pub needs_embedding: bool,
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
    let wipe_requested = request.full_rebuild || request.clear_only;
    let perform_wipe = wipe_requested && !is_resume;

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
        if request.full_rebuild && !is_resume {
            VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model)
                .delete()?;
            VisitContentMap::for_provider(
                paths,
                &provider.config.id,
                &provider.config.default_model,
            )
            .delete()?;
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
                request.full_rebuild,
                false,
                &stale_history_ids,
            ),
        )
        .await?;

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

/// Aggregate outcome of one embedding-backfill pass, fed into the final `AiIndexReport`.
#[derive(Debug, Default)]
pub(super) struct BackfillOutcome {
    /// Vectors freshly embedded + persisted this pass.
    pub indexed_items: usize,
    /// Rows re-embedded because their content hash changed (subset of `indexed_items`).
    pub updated_items: usize,
    /// Rows scanned but skipped because their content was unchanged.
    pub skipped_items: usize,
    /// The actual returned vector length once observed (D4); `None` if nothing was embedded.
    pub effective_dim: Option<usize>,
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
) -> Result<BackfillOutcome> {
    // Config-driven engine selection: static (W-AI-4c) / candle (W-AI-4b) / external `/v1/embeddings`.
    // The loop below is engine-agnostic.
    let embedder = super::embedding_candle::select_embedding_provider(paths, provider)?;
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

    // DEDUP RESUME STATE (W-AI-4c). Two crash windows must stay no-dup/no-miss:
    // - CRITICAL-2 (vectors): a crash after a chunk's vectors are appended but before the SQLite
    //   rows / cursor advance leaves content_keys on the vector plane while `needs_embedding` is true
    //   again. `persisted_keys` is the resume-time set of content_keys already on the `.pkvec` store;
    //   we never re-append a key in it, so a resume cannot double a vector. This is keyed by the u64
    //   STORAGE key because the on-disk store only carries u64s — it is the storage-boundary guard.
    // - the visit map: a crash mid-chunk can leave some visits mapped; `mapped_ids` is the resume-time
    //   set of history_ids already in the `.pkmap`, so a resume never doubles a visit's mapping entry.
    // Both also accumulate this run's own appends so a key/id is written at most once per run.
    //
    // MEDIUM-4 (truncated-u64 collision): the WORK-dedup ("have we already embedded this page?") keys
    // on the FULL `content_hash`, NOT the truncated u64 `content_key`. Two DISTINCT pages whose hashes
    // truncate to the same u64 (~2.8e-6 at 14.4M) are then each embedded as their own page rather than
    // the second silently collapsing into the first's vector. Truncation to u64 happens ONLY at the
    // `.pkvec` storage boundary (where the format is fixed-width u64-keyed); the in-memory identity is
    // the full hash, so distinct pages never lose their distinct embedding to a u64 collision.
    let mut persisted_keys: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut persisted_hashes: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut mapped_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    // Load the resume/incremental dedup state whenever a store/map already exists — not only on a
    // resume (`start_history_id > 0`). MEDIUM-5: a PLAIN incremental job starts at cursor 0 with empty
    // sets; without this, a NEW visit of an already-embedded page would re-embed + append a duplicate
    // `.pkvec` record (bloat that `read_all` last-writer-wins merely masks). Gate on the existing
    // store being present and NOT about to be wiped, so a true full rebuild still starts clean.
    let existing_store =
        VectorStore::for_provider(paths, &provider.config.id, &provider.config.default_model);
    if !perform_wipe && existing_store.exists() {
        persisted_keys = existing_store.existing_ids()?;
        mapped_ids = visit_map.mapped_history_ids()?;
    }

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

        let changed: Vec<&IndexedVisit> =
            chunk.iter().filter(|visit| visit.needs_embedding).collect();
        outcome.skipped_items += chunk.len() - changed.len();

        if !changed.is_empty() {
            // DEDUP: collect the UNIQUE pages among the changed visits that are not already embedded,
            // and embed each ONCE (the page identity is the FULL `content_hash`, MEDIUM-4).
            let embed_targets = select_embed_targets(&changed, &persisted_hashes, &persisted_keys);

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
                    persisted_hashes.insert(visit.content_hash.clone());
                    persisted_keys.insert(*key);
                }
                // Persist vectors FIRST so the watermark we report is always backed by on-disk data.
                store.append_vectors(&records)?;
            }

            // Map EVERY changed visit to its content_key (skip ones a prior crash already mapped), so
            // search/heavy-tier can fan a deduped vector out to all its visits. SQLite metadata is
            // (re-)written for every changed visit so its dedup hash catches up and it is not
            // re-scanned. Both are per-visit; only the VECTOR is deduped.
            let map_records: Vec<(i64, u64)> = changed
                .iter()
                .filter(|visit| mapped_ids.insert(visit.history_id))
                .map(|visit| (visit.history_id, visit.content_key))
                .collect();
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

/// Selects, in first-seen order, the UNIQUE pages among `changed` that still need embedding (W-AI-4c).
///
/// The dedup decision, extracted PURE so the truncated-u64-collision invariant (MEDIUM-4) is
/// unit-tested directly. A page is an embed target unless:
/// - its FULL `content_hash` was already embedded this run (`persisted_hashes`), OR
/// - its u64 `content_key` is already on the `.pkvec` plane (`persisted_keys`, the storage-boundary
///   resume backstop), OR
/// - an earlier visit in this same call already selected its full hash.
///
/// The work-dedup keys on the FULL `content_hash`, never the truncated u64 — so two DISTINCT pages
/// whose hashes collide on the first 8 bytes are EACH selected (the second is not silently dropped
/// onto the first's vector). The returned visits are the rows to embed, one per unique page.
pub(super) fn select_embed_targets<'a>(
    changed: &[&'a IndexedVisit],
    persisted_hashes: &std::collections::HashSet<String>,
    persisted_keys: &std::collections::HashSet<u64>,
) -> Vec<&'a IndexedVisit> {
    let mut seen_in_chunk: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut embed_targets: Vec<&IndexedVisit> = Vec::new();
    for visit in changed {
        if persisted_hashes.contains(visit.content_hash.as_str()) {
            continue; // Already embedded this run (its page hash is recorded after each append).
        }
        // Storage-boundary backstop (CRITICAL-2): on a resume/incremental we know which u64 keys are
        // already on the `.pkvec` plane (the on-disk store carries only u64s, so the full hash is
        // unrecoverable from it). A key on disk whose hash we have NOT seen this run is an
        // already-embedded page → skip re-embedding it. A genuine u64 collision against a distinct
        // on-disk page is astronomically rare AND only costs a redundant re-embed of an existing
        // vector, never a correctness loss; the run-level full-hash set is the authority that keeps
        // two colliding pages distinct WITHIN a run.
        if persisted_keys.contains(&visit.content_key) {
            continue;
        }
        if seen_in_chunk.insert(visit.content_hash.as_str()) {
            embed_targets.push(visit);
        }
    }
    embed_targets
}

/// Validates one embedded batch against its CONTENT KEYS and pairs each vector with its key (W-AI-4c).
///
/// Pure (no I/O) so the defensive guards are unit-tested directly:
/// - rejects a count mismatch (a short/over-long batch would desync the key↔vector join), and
/// - rejects a ragged/empty batch (a vector of a different dim than the first), since the vector
///   store is fixed-stride and a ragged record would corrupt every later read.
///
/// Returns the effective dim (the actual returned length of the first vector, D4) and the
/// `(content_key, vector)` records ready to append. With dedup the keys are the per-page content
/// keys (one per unique page), not visit ids — the loop deduped before calling this, so each key
/// here is distinct.
pub(super) fn validate_embedding_batch_for_keys(
    content_keys: &[u64],
    vectors: &[Vec<f32>],
) -> Result<(usize, Vec<(u64, Vec<f32>)>)> {
    if vectors.len() != content_keys.len() {
        anyhow::bail!(
            "embedding provider returned {} vector(s) for {} input(s)",
            vectors.len(),
            content_keys.len()
        );
    }
    let effective_dim = vectors.first().map(Vec::len).unwrap_or_default();
    if effective_dim == 0 {
        anyhow::bail!("embedding provider returned an empty vector for the batch");
    }
    let mut records = Vec::with_capacity(content_keys.len());
    for (content_key, vector) in content_keys.iter().zip(vectors.iter()) {
        if vector.len() != effective_dim {
            anyhow::bail!(
                "embedding provider returned a ragged batch (content_key {content_key} dim {} vs {effective_dim})",
                vector.len()
            );
        }
        records.push((*content_key, vector.clone()));
    }
    Ok((effective_dim, records))
}

/// Returns the chunk size for one pass, clamped by any remaining caller `limit`.
///
/// `EMBEDDING_BACKFILL_CHUNK` is a positive constant, so `clamp(1, CHUNK)` never inverts its
/// bounds (it would only panic if `max < min`).
pub(super) fn chunk_size(remaining: Option<usize>) -> usize {
    match remaining {
        Some(cap) => cap.clamp(1, EMBEDDING_BACKFILL_CHUNK),
        None => EMBEDDING_BACKFILL_CHUNK,
    }
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

/// Collects up to `limit` candidate visits with `history_id >= start_history_id`, ascending.
///
/// Ascending-id order makes the scan resumable from a single watermark. Each visit carries its
/// freshly built embedding content + hash and a `needs_embedding` flag (false when the stored
/// content hash already matches, so the chunk can skip it without re-embedding).
pub(super) fn collect_visit_chunk(
    paths: &ProjectPaths,
    connection: &Connection,
    provider: &AiProviderRuntime,
    start_history_id: i64,
    limit: usize,
) -> Result<Vec<IndexedVisit>> {
    let mut statement = connection.prepare(
        "SELECT visits.id,
                source_profiles.profile_key,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
           AND visits.id >= ?1
         ORDER BY visits.id ASC
         LIMIT ?2",
    )?;
    let mut rows = statement.query(params![start_history_id, limit as i64])?;
    let mut raw_visits = Vec::new();
    while let Some(row) = rows.next()? {
        raw_visits.push(IndexedVisit {
            history_id: row.get(0)?,
            profile_id: row.get(1)?,
            url: row.get(2)?,
            title: row.get(3)?,
            domain: String::new(),
            visited_at: crate::utils::chrome_time_to_rfc3339(row.get::<_, i64>(4)?),
            content: String::new(),
            content_hash: String::new(),
            content_key: 0,
            needs_embedding: false,
        });
    }

    let history_ids = raw_visits.iter().map(|visit| visit.history_id).collect::<Vec<_>>();
    let existing_hashes = load_existing_embedding_hashes(connection, provider, &history_ids)?;
    let enrichments = load_best_enrichment_map_by_history_ids(paths, connection, &history_ids)?;
    let mut visits = Vec::with_capacity(raw_visits.len());
    for mut visit in raw_visits {
        let enrichment = enrichments.get(&visit.history_id);
        // The TEXT fed to the model still carries the per-visit enrichment funnel, now including the
        // capped summary (06 §4) and the ~512-token-capped body (05 §8).
        let content = build_embedding_content_from_parts(
            &visit.profile_id,
            &visit.url,
            visit.title.as_deref(),
            &visit.visited_at,
            enrichment.and_then(|value| value.readable_title.as_deref()),
            enrichment_summary_for(enrichment),
            enrichment.and_then(|value| value.readable_text.as_deref()),
        );
        // The DEDUP IDENTITY is visit-independent (05 §1): canonical URL + title + the reserved
        // enrichment summary slot (None until W-ENRICH-1). Two visits of one page share this hash and
        // its content_key, so the page is embedded ONCE. The enrichment summary is reserved now so
        // W-ENRICH-1 filling it re-hashes only the enriched URLs, not the whole corpus (06 §3).
        let content_hash = super::dedup::build_dedup_content_hash(
            &visit.url,
            visit.title.as_deref(),
            enrichment_summary_for(enrichment),
        );
        let content_key = super::dedup::content_key_from_hash(&content_hash);
        visit.needs_embedding = existing_hashes.get(&visit.history_id).map(String::as_str)
            != Some(content_hash.as_str());
        visit.domain = url_domain(&visit.url);
        visit.content = content;
        visit.content_hash = content_hash;
        visit.content_key = content_key;
        visits.push(visit);
    }
    Ok(visits)
}

/// Resolves the capped `enrichment_summary` for one visit's dedup hash (W-ENRICH-1, 06 §3).
///
/// Returns the stored capped summary from the best `visit_content_enrichments` row, so a freshly
/// enriched URL re-hashes (and thus re-embeds) ONLY itself — the bounded blast radius the dedup hash
/// was designed for (06 §3 / 05 §1). A visit with no enrichment (or a failure row that stored no
/// summary) contributes `None`, the same empty dedup segment as before W-ENRICH-1, so the corpus that
/// was never enriched keeps its existing content_hash and is NOT re-embedded. This is the SINGLE seam
/// between the enrichment plane and the embedding dedup identity — centralizing it here is why
/// W-ENRICH-1 only had to fill this one function (the reserved slot the dedup hash already carried).
///
/// Whitespace-only / empty summaries are treated as absent so they never perturb the hash; the stored
/// value is already capped + normalized by the extractor (`build_enrichment_summary`), so this is a
/// thin pass-through that keeps the dedup hash stable across re-reads of the same row.
fn enrichment_summary_for(
    enrichment: Option<&crate::enrichment::StoredEnrichment>,
) -> Option<&str> {
    enrichment
        .and_then(|value| value.enrichment_summary.as_deref())
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
}

/// Loads the current content hashes for a bounded set of candidate history rows.
pub(super) fn load_existing_embedding_hashes(
    connection: &Connection,
    provider: &AiProviderRuntime,
    history_ids: &[i64],
) -> Result<HashMap<i64, String>> {
    let mut hashes = HashMap::new();
    for chunk in history_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT history_id, content_hash
             FROM ai_embeddings
             WHERE provider_id = ?1
               AND model = ?2
               AND history_id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql)?;
        let params = std::iter::once(&provider.config.id as &dyn rusqlite::ToSql)
            .chain(std::iter::once(&provider.config.default_model as &dyn rusqlite::ToSql))
            .chain(chunk.iter().map(|history_id| history_id as &dyn rusqlite::ToSql));
        let rows = statement.query_map(rusqlite::params_from_iter(params), |row: &Row<'_>| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (history_id, content_hash) = row?;
            hashes.insert(history_id, content_hash);
        }
    }
    Ok(hashes)
}

/// Collects the canonical visits that need fresh embeddings for the selected provider.
///
/// Legacy recency-ordered (DESC) collector kept for the existing change-detection/cleanup unit
/// tests. The production backfill uses [`collect_visit_chunk`] (ascending, resumable from a
/// cursor); this DESC variant is no longer on the production path.
#[cfg(test)]
pub(super) fn collect_visits_to_index(
    paths: &ProjectPaths,
    connection: &Connection,
    provider: &AiProviderRuntime,
    limit: Option<u32>,
) -> Result<Vec<IndexedVisit>> {
    let limit_sql = limit.unwrap_or(0).max(1);
    let sql = if limit.is_some() {
        "SELECT visits.id,
                source_profiles.profile_key,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
         ORDER BY visits.visit_time_ms DESC
         LIMIT ?1"
    } else {
        "SELECT visits.id,
                source_profiles.profile_key,
                urls.url,
                urls.title,
                (visits.visit_time_ms * 1000 + 11644473600000000) AS visit_time
         FROM archive.visits AS visits
         JOIN archive.urls AS urls ON urls.id = visits.url_id
         JOIN archive.source_profiles AS source_profiles ON source_profiles.id = visits.source_profile_id
         WHERE visits.reverted_at IS NULL
         ORDER BY visits.visit_time_ms DESC"
    };

    let mut statement = connection.prepare(sql)?;
    let mut rows =
        if limit.is_some() { statement.query(params![limit_sql])? } else { statement.query([])? };

    let mut raw_visits = Vec::new();
    while let Some(row) = rows.next()? {
        raw_visits.push(IndexedVisit {
            history_id: row.get(0)?,
            profile_id: row.get(1)?,
            url: row.get(2)?,
            title: row.get(3)?,
            domain: String::new(),
            visited_at: crate::utils::chrome_time_to_rfc3339(row.get::<_, i64>(4)?),
            content: String::new(),
            content_hash: String::new(),
            content_key: 0,
            needs_embedding: true,
        });
    }

    let history_ids = raw_visits.iter().map(|visit| visit.history_id).collect::<Vec<_>>();
    let existing_hashes = load_existing_embedding_hashes(connection, provider, &history_ids)?;
    let enrichments = load_best_enrichment_map_by_history_ids(paths, connection, &history_ids)?;
    let mut visits = Vec::with_capacity(raw_visits.len());
    for mut visit in raw_visits {
        let enrichment = enrichments.get(&visit.history_id);
        let content = build_embedding_content_from_parts(
            &visit.profile_id,
            &visit.url,
            visit.title.as_deref(),
            &visit.visited_at,
            enrichment.and_then(|value| value.readable_title.as_deref()),
            enrichment_summary_for(enrichment),
            enrichment.and_then(|value| value.readable_text.as_deref()),
        );
        let content_hash = super::dedup::build_dedup_content_hash(
            &visit.url,
            visit.title.as_deref(),
            enrichment_summary_for(enrichment),
        );
        if existing_hashes.get(&visit.history_id) == Some(&content_hash) {
            continue;
        }
        visit.domain = url_domain(&visit.url);
        visit.content = content;
        visit.content_key = super::dedup::content_key_from_hash(&content_hash);
        visit.content_hash = content_hash;
        visits.push(visit);
    }
    Ok(visits)
}

/// Deletes SQLite compatibility rows whose canonical history ids are no longer visible.
pub(super) fn cleanup_stale_embeddings(
    connection: &Connection,
    provider: &AiProviderRuntime,
) -> Result<usize> {
    #[rustfmt::skip]
    let removed = connection.execute(
        DELETE_STALE_EMBEDDINGS_SQL,
        params![provider.config.id, provider.config.default_model],
    )?;
    Ok(removed)
}

/// Collects stale history ids so the optional semantic sidecar can drop rows that no longer exist.
pub(super) fn collect_stale_history_ids(
    connection: &Connection,
    provider: &AiProviderRuntime,
) -> Result<Vec<i64>> {
    let mut statement = connection.prepare(
        "SELECT history_id
         FROM ai_embeddings
         WHERE provider_id = ?1
           AND model = ?2
           AND history_id NOT IN (
             SELECT id FROM archive.visits WHERE reverted_at IS NULL
           )",
    )?;
    statement
        .query_map(params![provider.config.id, provider.config.default_model], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("collecting stale AI embedding ids")
}

/// Counts the current number of SQLite compatibility rows for one provider/model pair.
pub(super) fn provider_embedding_count(
    connection: &Connection,
    provider_id: &str,
    model: &str,
) -> Result<i64> {
    #[rustfmt::skip]
    let count = connection.query_row(
        "SELECT COUNT(*) FROM ai_embeddings WHERE provider_id = ?1 AND model = ?2",
        params![provider_id, model],
        |row: &Row<'_>| row.get::<_, i64>(0),
    )?;
    Ok(count)
}

/// Clears all SQLite compatibility rows for one provider/model pair.
pub(super) fn clear_provider_embeddings(
    connection: &Connection,
    provider: &AiProviderRuntime,
) -> Result<()> {
    #[rustfmt::skip]
    connection.execute(
        CLEAR_PROVIDER_EMBEDDINGS_SQL,
        params![provider.config.id, provider.config.default_model],
    )?;
    Ok(())
}

/// Upserts one SQLite compatibility row after a semantic embedding was produced.
///
/// Returns whether a metadata row for this `(history_id, provider, model)` already existed BEFORE
/// the write, so the backfill can count re-embeds (changed content) separately from first-time
/// embeds. The row keeps the lightweight provenance the read models report; the vector itself
/// lives only on the `derived/vectors/` plane, never in SQLite (02 §A).
pub(super) fn upsert_embedding(
    connection: &Connection,
    provider: &AiProviderRuntime,
    visit: &IndexedVisit,
    indexed_at: &str,
) -> Result<bool> {
    let prior_rows: i64 = connection.query_row(
        "SELECT COUNT(*) FROM ai_embeddings
         WHERE history_id = ?1 AND provider_id = ?2 AND model = ?3",
        params![visit.history_id, provider.config.id, provider.config.default_model],
        |row: &Row<'_>| row.get(0),
    )?;
    let existed = prior_rows > 0;
    // Clear any prior row for this (history_id, provider, model) so a content-hash change does not
    // leave a duplicate UNIQUE(history_id, provider, model, content_hash) row behind.
    connection.execute(
        "DELETE FROM ai_embeddings
         WHERE history_id = ?1 AND provider_id = ?2 AND model = ?3",
        params![visit.history_id, provider.config.id, provider.config.default_model],
    )?;
    #[rustfmt::skip]
    connection.execute(
        UPSERT_EMBEDDING_SQL,
        params![
            visit.history_id,
            visit.profile_id,
            visit.url,
            visit.title,
            visit.domain,
            visit.visited_at,
            visit.content_hash,
            visit.content.len() as i64,
            provider.config.id,
            provider.config.default_model,
            indexed_at
        ],
    )?;
    Ok(existed)
}

/// Builds the stable plain-text representation used to hash and embed one history row.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn build_embedding_content(
    profile_id: &str,
    url: &str,
    title: Option<&str>,
    visited_at: &str,
) -> String {
    let title = title.unwrap_or("(untitled)");
    format!(
        "Profile: {profile_id}\nVisited at: {visited_at}\nURL: {url}\nDomain: {domain}\nTitle: {title}",
        domain = url_domain(url)
    )
}
