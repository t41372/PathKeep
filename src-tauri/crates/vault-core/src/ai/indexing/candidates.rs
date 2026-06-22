//! Candidate-visit collection and the pure embed-target selection (W-AI-4c dedup).
//!
//! ## Responsibilities
//! - collect ascending-`history_id` candidate chunks (resumable from a cursor) with their freshly
//!   built embedding content + dedup hash + `needs_embedding` flag
//! - load the stored content hashes for change detection and the stale-id set for cleanup
//! - decide, PURELY, which unique pages among a changed chunk still need embedding (MEDIUM-4: the
//!   work-dedup keys on the FULL `content_hash`, never the truncated u64 key)
//!
//! ## Not responsible for
//! - the embed loop / crash-resume dedup state (`super::backfill` owns those)
//! - SQLite compatibility-row writes (`super::store_rows` owns those)

use super::super::*;
use super::IndexedVisit;
use super::content::enrichment_summary_for;

/// Collects up to `limit` candidate visits with `history_id >= start_history_id`, ascending.
///
/// Ascending-id order makes the scan resumable from a single watermark. Each visit carries its
/// freshly built embedding content + hash and a `needs_embedding` flag (false when the stored
/// content hash already matches, so the chunk can skip it without re-embedding).
pub(in crate::ai) fn collect_visit_chunk(
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
        let content_hash = super::super::dedup::build_dedup_content_hash(
            &visit.url,
            visit.title.as_deref(),
            enrichment_summary_for(enrichment),
        );
        let content_key = super::super::dedup::content_key_from_hash(&content_hash);
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

/// Loads the current content hashes for a bounded set of candidate history rows.
pub(in crate::ai) fn load_existing_embedding_hashes(
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

/// Collects stale history ids so the optional semantic sidecar can drop rows that no longer exist.
pub(in crate::ai) fn collect_stale_history_ids(
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
///
/// The crash-resume state that feeds the two sets is encapsulated in [`super::backfill::DedupTracker`];
/// this stays a free fn taking borrowed sets so the collision invariant is exercised directly without
/// constructing a tracker.
pub(in crate::ai) fn select_embed_targets<'a>(
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
