//! Responsibilities: cheap invalidation of search-group windows and stable input identities.
//! Not responsible for: linkage, snapshot publication, or marking data dirty with separate flags.
//! Failure modes: undo, search-rule rebuilds, same-model rebuilds, or mapping-only passes go unseen.
//! Tests: search_group_snapshots_tests; grouping digests the actual inputs after this guard moves.
use super::{AiProviderRuntime, FlatVectorIndex, ReverseVisitMap, VectorStore, search_vectors};
use crate::{
    ProjectPaths,
    intelligence::{
        InsightsZone, incremental::load_archive_delta_signature,
        intelligence_overview_snapshot::load_search_rule_signature,
    },
};
use anyhow::Result;
use serde::Serialize;
use vault_store::{Session, params_owned as params};

const RULE_VERSION: u32 = 1;

#[derive(Serialize)]
pub(super) struct Identity {
    version: u32,
    zone: String,
    model: String,
    pages: String,
    searches: String,
}

// Legacy stores get a file identity without rewriting their potentially huge vector payload.
// Appending must not change it. Any recreation by this app writes the new header's random ID.
fn legacy_identity(metadata: &std::fs::Metadata) -> Result<String> {
    if let Ok(created) = metadata.created() {
        return Ok(format!("legacy:{created:?}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("legacy:{}:{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(not(unix))]
    {
        // Unusual filesystems without creation time can conservatively invalidate on append.
        Ok(format!("legacy:{:?}", metadata.modified()?))
    }
}
fn store_identity(store: &VectorStore) -> Result<(String, String)> {
    let Some(header) = store.read_header()? else { return Ok(Default::default()) };
    let id = if header.store_id.is_empty() {
        legacy_identity(&std::fs::metadata(store.path())?)?
    } else {
        header.store_id
    };
    Ok((id, header.fingerprint_hash))
}

pub(super) fn identity(
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    zone: &str,
) -> Result<Identity> {
    let (pages, model) = store_identity(&VectorStore::for_provider(
        paths,
        &provider.config.id,
        &provider.config.default_model,
    ))?;
    let (searches, _) = store_identity(&VectorStore::for_provider(
        paths,
        &search_vectors::identity(provider),
        &provider.config.default_model,
    ))?;
    Ok(Identity { version: RULE_VERSION, zone: zone.into(), model, pages, searches })
}

/// Counts/maxima seek the month before touching vectors or constructing linkage inputs.
/// Archive runs/visible totals also catch undo while the previous navigation generation serves.
pub(super) fn cheap(
    session: &Session<'_>,
    paths: &ProjectPaths,
    provider: &AiProviderRuntime,
    range: &crate::DateRange,
) -> Result<String> {
    let zone = InsightsZone::for_reading(session)?;
    let bounds = zone.range_bounds(range)?;
    let generations = session.query(
        "SELECT profile_id,navigation FROM intelligence_generations ORDER BY profile_id",
        (),
        |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
    )?;
    let events = session.query_one(
        "SELECT COUNT(*),COALESCE(MAX(e.visit_id),0) FROM search_events_rows e INDEXED BY idx_search_events_kind_time
         CROSS JOIN intelligence_generations g ON g.profile_id=e.profile_id AND g.navigation=e.generation
         WHERE e.query_kind='keyword' AND e.visit_time_ms>=?1 AND e.visit_time_ms<?2",
        params![bounds.0,bounds.1], |r| Ok((r.get::<i64>(0)?,r.get::<i64>(1)?)))?;
    // Keep aggregation in one statement. A materialized set of month trails drives primary-key
    // member seeks; no per-trail command round trips and no scan of other months' members.
    let members = session.query_one(
        "WITH trails AS MATERIALIZED (
           SELECT DISTINCT e.profile_id,e.generation,e.trail_id FROM search_events_rows e INDEXED BY idx_search_events_kind_time
           CROSS JOIN intelligence_generations g ON g.profile_id=e.profile_id AND g.navigation=e.generation
           WHERE e.query_kind='keyword' AND e.visit_time_ms>=?1 AND e.visit_time_ms<?2 AND e.trail_id IS NOT NULL
         ) SELECT COUNT(*),COALESCE(MAX(m.visit_id),0) FROM trails t
         CROSS JOIN search_trail_members_rows m ON m.generation=t.generation AND m.trail_id=t.trail_id AND m.profile_id=t.profile_id",
        params![bounds.0,bounds.1], |r| Ok((r.get::<i64>(0)?,r.get::<i64>(1)?)))?;
    let pages = FlatVectorIndex::open(paths, &provider.config.id, &provider.config.default_model)?;
    let searches = FlatVectorIndex::open(
        paths,
        &search_vectors::identity(provider),
        &provider.config.default_model,
    )?;
    let forward =
        ReverseVisitMap::for_provider(paths, &provider.config.id, &provider.config.default_model)
            .projection_header(pages.fingerprint_hash())?;
    // Projection IDs also reject a reader racing publication at unchanged vector counts.
    Ok(crate::utils::sha256_hex(&serde_json::to_vec(&(
        identity(paths, provider, zone.name())?,
        &range.start,
        &range.end,
        generations,
        load_search_rule_signature(session)?,
        load_archive_delta_signature(session)?,
        events,
        members,
        pages.len(),
        searches.len(),
        forward,
        pages.projection_id(),
        searches.projection_id(),
    ))?))
}
