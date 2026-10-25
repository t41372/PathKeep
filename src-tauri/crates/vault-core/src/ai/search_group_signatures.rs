//! Responsibilities: cheap invalidation of search-group windows and stable input identities.
//! Not responsible for: linkage, snapshot publication, or marking data dirty with separate flags.
//! Failure modes: undo, search-rule rebuilds, same-model rebuilds, or mapping-only passes go unseen.
//! Tests: search_group_snapshots_tests; grouping digests the actual inputs after this guard moves.
use super::{AiProviderRuntime, FlatVectorIndex, ReverseVisitMap, search_vectors};
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

#[derive(Clone, Serialize)]
pub(super) struct Identity {
    version: u32,
    zone: String,
    model: String,
    pages: String,
    searches: String,
}

pub(super) fn identity(
    paths: &ProjectPaths,
    _provider: &AiProviderRuntime,
    zone: &str,
) -> Result<Identity> {
    let metadata = super::generations::serving_metadata(paths)?;
    let (pages, model) =
        metadata.pages.map(|h| (h.store_id, h.fingerprint_hash)).unwrap_or_default();
    let searches = metadata.searches.map(|h| h.store_id).unwrap_or_default();
    Ok(Identity { version: RULE_VERSION, zone: zone.into(), model, pages, searches })
}

/// Immutable file inputs captured under the caller's lease, before opening a DB session.
#[derive(Serialize)]
pub(super) struct Files {
    identity: Identity,
    pages: usize,
    searches: usize,
    forward: Option<(String, u64)>,
    page_projection: String,
    search_projection: String,
}
pub(super) fn files(paths: &ProjectPaths, provider: &AiProviderRuntime) -> Result<Files> {
    let pages = FlatVectorIndex::open(paths, &provider.config.id, &provider.config.default_model)?;
    let searches = FlatVectorIndex::open(
        paths,
        &search_vectors::identity(provider),
        &provider.config.default_model,
    )?;
    let forward =
        ReverseVisitMap::for_provider(paths, &provider.config.id, &provider.config.default_model)
            .projection_header(pages.fingerprint_hash())?;
    Ok(Files {
        identity: identity(paths, provider, "")?,
        pages: pages.len(),
        searches: searches.len(),
        forward,
        page_projection: pages.projection_id().to_owned(),
        search_projection: searches.projection_id().to_owned(),
    })
}

/// Counts/maxima seek the month; this function performs only database work.
pub(super) fn cheap(
    session: &Session<'_>,
    files: &Files,
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
    // Projection IDs also reject a reader racing publication at unchanged vector counts.
    Ok(crate::utils::sha256_hex(&serde_json::to_vec(&(
        Identity { zone: zone.name().into(), ..files.identity.clone() },
        super::generations::pointer(session, &provider.config.id, &provider.config.default_model)?,
        &range.start,
        &range.end,
        generations,
        load_search_rule_signature(session)?,
        load_archive_delta_signature(session)?,
        events,
        members,
        files.pages,
        files.searches,
        &files.forward,
        &files.page_projection,
        &files.search_projection,
    ))?))
}
