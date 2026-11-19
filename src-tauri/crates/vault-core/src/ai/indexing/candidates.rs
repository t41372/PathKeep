//! Failure modes: a chunk cursor skips unchanged rows or dedup collisions suppress pages.
//! Candidate-visit collection and the pure embed-target selection (W-AI-4c dedup).
//!
//! ## Responsibilities
//! - collect ascending-`history_id` candidate chunks (resumable from a cursor) with their freshly
//!   built embedding content + dedup hash + `needs_embedding` flag
//! - load the stored content hashes for change detection and the stale-id set for cleanup
//! - decide, PURELY, which unique pages among a changed chunk still need embedding (the
//!   work-dedup keys on the FULL `content_hash`, never the truncated u64 key)
//!
//! ## Not responsible for
//! - the embed loop / crash-resume dedup state (`super::backfill` owns those)
//! - Turso metadata-row writes (`super::store_rows` owns those)

use super::super::*;
use super::IndexedVisit;
use super::content::enrichment_summary_for;
use vault_store::params_owned as params;
use vault_store::{Db, Session};

/// Test view of the visible candidates in a bounded physical visit page.
///
/// Ascending-id order makes the scan resumable from a single watermark. Each visit carries its
/// freshly built embedding content + hash and a `needs_embedding` flag (false when the stored
/// content hash already matches, so the chunk can skip it without re-embedding).
#[cfg(test)]
pub(in crate::ai) fn collect_visit_chunk(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    start_history_id: i64,
    upper_history_id: i64,
    limit: usize,
    lineage: &str,
) -> Result<Vec<IndexedVisit>> {
    Ok(read_visit_page(vault, provider, start_history_id, upper_history_id, limit, lineage)?.visits)
}

/// Physical membership drives the cursor even when a whole page is hidden.
pub(super) struct VisitPage {
    pub last_id: Option<i64>,
    pub physical_count: usize,
    pub visits: Vec<IndexedVisit>,
}

pub(super) fn read_visit_page(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    start: i64,
    upper: i64,
    limit: usize,
    lineage: &str,
) -> Result<VisitPage> {
    let mut page = vault.read(Db::Archive, move |s| {
        let mut statement = s.prepare(
            "SELECT v.id,u.id IS NOT NULL AND p.id IS NOT NULL,p.profile_key,u.url,u.title,
                    v.visit_time_ms*1000+11644473600000000
             FROM visits v LEFT JOIN urls u ON v.reverted_at IS NULL AND u.id=v.url_id
             LEFT JOIN source_profiles p ON v.reverted_at IS NULL AND p.id=v.source_profile_id
             WHERE v.id>=?1 AND v.id<=?2 ORDER BY v.id LIMIT ?3",
        )?;
        let mut rows = statement.query(params![start, upper, limit as i64])?;
        let mut page = VisitPage { last_id: None, physical_count: 0, visits: Vec::new() };
        while let Some(row) = rows.next()? {
            let id = row.get(0)?;
            page.last_id = Some(id);
            page.physical_count += 1;
            if row.get::<i64>(1)? != 0 {
                page.visits.push(IndexedVisit {
                    history_id: id,
                    profile_id: row.get(2)?,
                    url: row.get(3)?,
                    title: row.get(4)?,
                    domain: String::new(),
                    visited_at: crate::utils::chrome_time_to_rfc3339(row.get::<i64>(5)?),
                    content: String::new(),
                    content_hash: String::new(),
                    content_key: 0,
                    needs_embedding: false,
                });
            }
        }
        Ok(page)
    })?;
    if !page.visits.is_empty() {
        page.visits = hydrate(vault, provider, page.visits, lineage)?;
    }
    Ok(page)
}

fn hydrate(
    vault: &crate::Vault,
    provider: &AiProviderRuntime,
    raw_visits: Vec<IndexedVisit>,
    lineage: &str,
) -> Result<Vec<IndexedVisit>> {
    let history_ids = raw_visits.iter().map(|visit| visit.history_id).collect::<Vec<_>>();
    let prompt =
        super::super::embedding_prompt::PromptProfile::for_model(&provider.config.default_model);
    let lineage = lineage.to_owned();
    let provider = provider.clone();
    let paths = vault.paths().clone();
    let (existing_hashes, enrichments) = vault.read(Db::Intelligence, move |s| {
        Ok((
            load_existing_embedding_hashes(s, &provider, &history_ids, &lineage)?,
            load_best_enrichment_map_by_history_ids(&paths, s, &history_ids)?,
        ))
    })?;
    let mut visits = Vec::with_capacity(raw_visits.len());
    for mut visit in raw_visits {
        let enrichment = enrichments.get(&visit.history_id);
        let document = build_embedding_document(
            &visit.url,
            visit.title.as_deref(),
            enrichment.and_then(|value| value.readable_title.as_deref()),
            enrichment_summary_for(enrichment),
            enrichment.and_then(|value| value.readable_text.as_deref()),
        );
        let content = prompt.render_document(&document);
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
    connection: &Session<'_>,
    provider: &AiProviderRuntime,
    history_ids: &[i64],
    lineage: &str,
) -> Result<HashMap<i64, String>> {
    let mut hashes = HashMap::new();
    for chunk in history_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders =
            (4..chunk.len() + 4).map(|index| format!("(?{index})")).collect::<Vec<_>>().join(",");
        let sql = format!(
            "WITH requested(history_id) AS (VALUES {placeholders}) SELECT e.history_id,e.content_hash
             FROM requested CROSS JOIN ai_embeddings e INDEXED BY idx_ai_embeddings_lineage_history ON e.history_id=requested.history_id
             WHERE e.provider_id=?1 AND e.model=?2 AND e.lineage=?3"
        );
        let mut statement = connection.prepare(&sql)?;
        let params = std::iter::once(vault_store::Value::Text(provider.config.id.clone()))
            .chain(std::iter::once(vault_store::Value::Text(provider.config.default_model.clone())))
            .chain(std::iter::once(vault_store::Value::Text(lineage.to_owned())))
            .chain(chunk.iter().copied().map(vault_store::Value::Integer));
        let rows = statement.query_map(vault_store::params_from_iter(params), |row| {
            Ok((row.get::<i64>(0)?, row.get::<String>(1)?))
        })?;
        for row in rows {
            let (history_id, content_hash) = row?;
            hashes.insert(history_id, content_hash);
        }
    }
    Ok(hashes)
}

/// Selects, in first-seen order, the UNIQUE pages among `changed` that still need embedding (W-AI-4c).
///
/// The dedup decision, extracted PURE so the truncated-u64-collision invariant is
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
        // Storage-boundary backstop: on a resume/incremental we know which u64 keys are
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
