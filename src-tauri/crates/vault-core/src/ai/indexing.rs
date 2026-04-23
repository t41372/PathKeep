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
//! - `crate::ai_sidecar` for LanceDB sidecar synchronization
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
/// normalized embedding payload and hash used for incremental change detection.
#[derive(Debug, Clone)]
pub(super) struct IndexedVisit {
    pub history_id: i64,
    pub profile_id: String,
    pub url: String,
    pub title: Option<String>,
    pub domain: String,
    pub visited_at: String,
    pub content: String,
    pub content_hash: String,
}

/// Builds or refreshes the semantic sidecar for one embedding provider.
///
/// This is the main entrypoint for semantic indexing. It keeps all durable side effects
/// together: run-ledger bookkeeping, SQLite compatibility rows, LanceDB sidecar sync, and
/// stale-row cleanup.
pub async fn build_ai_index(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
) -> Result<AiIndexReport> {
    build_ai_index_with_control(paths, config, key, provider, request, None).await
}

/// Builds the semantic index while honoring optional cooperative stop checkpoints.
///
/// The worker queue uses this path so a user-triggered cancel can stop between batches
/// without leaving the ledger and sidecar in a misleading half-finished state.
pub async fn build_ai_index_with_control(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    provider: &AiProviderRuntime,
    request: &AiIndexRequest,
    run_control: Option<Arc<dyn AiRunControl>>,
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

    let result: Result<AiIndexReport> = async {
        let run_control = run_control.as_ref();
        checkpoint_ai_run(run_control, "Index build was cancelled before collecting stale rows.")?;
        let stale_history_ids = collect_stale_history_ids(&connection, provider)?;

        if request.full_rebuild || request.clear_only {
            clear_provider_embeddings(&connection, provider)?;
        }

        let removed_items = cleanup_stale_embeddings(&connection, provider)?;
        let sidecar_removed = if request.full_rebuild || request.clear_only {
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
                    "Cleared the semantic index compatibility rows and the LanceDB sidecar."
                        .to_string(),
                ],
            });
        }

        checkpoint_ai_run(run_control, "Index build was cancelled before collecting candidates.")?;
        let candidates = collect_visits_to_index(paths, &connection, provider, request.limit)?;
        if candidates.is_empty() {
            await_with_ai_cancellation(
                run_control,
                "Index build was cancelled before the empty sidecar sync finished.",
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
                notes: vec!["No new or changed history rows required indexing.".to_string()],
            });
        }

        let timestamp = now_rfc3339();
        let mut indexed_items = 0usize;
        let mut updated_items = 0usize;
        let mut skipped_items = 0usize;
        let mut sidecar_rows = Vec::with_capacity(candidates.len());
        let mut partial_failure_notes = Vec::new();
        let existing_history_ids = load_existing_embedding_hashes(
            &connection,
            provider,
            &candidates.iter().map(|visit| visit.history_id).collect::<Vec<_>>(),
        )?
        .into_keys()
        .collect::<HashSet<_>>();

        for batch in candidates.chunks(EMBEDDING_BATCH_SIZE) {
            checkpoint_ai_run(
                run_control,
                "Index build was cancelled before the next embedding batch started.",
            )?;
            let texts = batch.iter().map(|visit| visit.content.clone()).collect::<Vec<_>>();
            match await_with_ai_cancellation(
                run_control,
                "Index build was cancelled while waiting for embedding batch results.",
                embed_batch_with_retry(provider, &texts),
            )
            .await
            {
                Ok(vectors) if vectors.len() == batch.len() => {
                    for (visit, vector) in batch.iter().zip(vectors.into_iter()) {
                        let had_prior_index = existing_history_ids.contains(&visit.history_id);
                        upsert_embedding(&connection, provider, visit, &timestamp)?;
                        sidecar_rows.push(SidecarEmbeddingRow {
                            history_id: visit.history_id,
                            profile_id: visit.profile_id.clone(),
                            url: visit.url.clone(),
                            title: visit.title.clone(),
                            domain: visit.domain.clone(),
                            visited_at: visit.visited_at.clone(),
                            provider_id: provider.config.id.clone(),
                            model: provider.config.default_model.clone(),
                            content_hash: visit.content_hash.clone(),
                            indexed_at: timestamp.clone(),
                            vector,
                        });
                        if had_prior_index {
                            updated_items += 1;
                        } else {
                            indexed_items += 1;
                        }
                    }
                }
                Ok(_) | Err(_) => {
                    for visit in batch {
                        checkpoint_ai_run(
                            run_control,
                            "Index build was cancelled before an individual retry embedding call.",
                        )?;
                        let had_prior_index = existing_history_ids.contains(&visit.history_id);
                        match await_with_ai_cancellation(
                            run_control,
                            "Index build was cancelled while retrying an individual embedding call.",
                            embed_single_with_retry(provider, &visit.content),
                        )
                        .await
                        {
                            Ok(vector) => {
                                upsert_embedding(&connection, provider, visit, &timestamp)?;
                                sidecar_rows.push(SidecarEmbeddingRow {
                                    history_id: visit.history_id,
                                    profile_id: visit.profile_id.clone(),
                                    url: visit.url.clone(),
                                    title: visit.title.clone(),
                                    domain: visit.domain.clone(),
                                    visited_at: visit.visited_at.clone(),
                                    provider_id: provider.config.id.clone(),
                                    model: provider.config.default_model.clone(),
                                    content_hash: visit.content_hash.clone(),
                                    indexed_at: timestamp.clone(),
                                    vector,
                                });
                                if had_prior_index {
                                    updated_items += 1;
                                } else {
                                    indexed_items += 1;
                                }
                            }
                            Err(error) => {
                                skipped_items += 1;
                                partial_failure_notes.push(format!(
                                    "Skipped history row {} after batch and per-row embedding retries: {}",
                                    visit.history_id, error
                                ));
                            }
                        }
                    }
                }
            }
        }
        let sidecar_synced = await_with_ai_cancellation(
            run_control,
            "Index build was cancelled while syncing the semantic sidecar.",
            ai_sidecar::sync_provider_embeddings(
                paths,
                &provider.config.id,
                &provider.config.default_model,
                &sidecar_rows,
                request.full_rebuild,
                false,
                &stale_history_ids,
            ),
        )
        .await?;

        Ok(AiIndexReport {
            job_id: None,
            run_id: Some(run_id),
            provider_id: provider.config.id.clone(),
            model: provider.config.default_model.clone(),
            indexed_items,
            updated_items,
            skipped_items,
            removed_items: removed_items + sidecar_removed,
            last_indexed_at: timestamp,
            notes: {
                let mut notes = vec![
                    format!(
                        "Indexed {} history rows with {}.",
                        candidates.len(),
                        provider.config.name
                    ),
                    format!(
                        "Processed {} embedding batch(es) with a batch size of {}.",
                        candidates.len().div_ceil(EMBEDDING_BATCH_SIZE),
                        EMBEDDING_BATCH_SIZE
                    ),
                    format!(
                        "Synced {} row(s) into the LanceDB semantic sidecar. PathKeep keeps the SQLite mirror only for metadata/debug compatibility, not for full-table semantic fallback scans.",
                        sidecar_synced
                    ),
                ];
                if skipped_items > 0 {
                    notes.push(format!(
                        "Skipped {} row(s) after retrying failed embedding batches individually.",
                        skipped_items
                    ));
                    notes.extend(partial_failure_notes);
                }
                notes
            },
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
            enrichment.and_then(|value| value.readable_text.as_deref()),
        );
        let content_hash = sha256_hex(content.as_bytes());
        if existing_hashes.get(&visit.history_id) == Some(&content_hash) {
            continue;
        }
        visit.domain = url_domain(&visit.url);
        visit.content = content;
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

/// Collects stale history ids so the LanceDB sidecar can drop rows that no longer exist.
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
pub(super) fn upsert_embedding(
    connection: &Connection,
    provider: &AiProviderRuntime,
    visit: &IndexedVisit,
    indexed_at: &str,
) -> Result<()> {
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
    Ok(())
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
