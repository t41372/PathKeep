//! Session, trail, and query-family builders for structural Core Intelligence.
//!
//! ## Responsibilities
//! - Build deterministic sessions and search trails from normalized visit
//!   records.
//! - Group keyword search events into query families for both in-memory and
//!   batched rebuild paths.
//! - Keep the query-family similarity heuristic out of stage orchestration.
//!
//! ## Not responsible for
//! - Persisting structural rows into SQLite.
//! - Running refind, habit, or source-effectiveness aggregate logic.
//! - Choosing fallback versus incremental rebuild paths.
//!
//! ## Dependencies
//! - Shared structural state builders in `intelligence_structural_state`.
//! - Parent-module query normalization and similarity helpers.
//! - `search_events` / `search_trails` tables for the batched replay path.
//!
//! ## Performance notes
//! - In-memory builders only hold one active session/trail at a time.
//! - The batched query-family replay streams ordered search events instead of
//!   materializing unrelated archive state.

use super::intelligence_structural_state::SessionBuildState;
use super::{
    QueryFamilyRecord, SearchEventBatchCursor, SearchEventRecord, SearchQueryKind,
    TrailMemberRecord, TrailRecord, VisitRecord, classify_search_query_kind, normalize_query,
    query_token_set,
};
use anyhow::Result;
use rusqlite::{Connection, params};

/// Minimal accumulator for building query families incrementally from ordered
/// search events.
#[derive(Debug, Default)]
pub(super) struct QueryFamilyAccumulator {
    families: Vec<QueryFamilyRecord>,
}

/// Builds deterministic sessions from normalized visits while preserving the
/// session ids needed by later trail and refind builders.
pub(super) fn build_sessions(visits: &mut [VisitRecord]) -> Vec<super::SessionRecord> {
    let mut sessions = Vec::new();
    let mut current: Option<SessionBuildState> = None;
    for visit in visits.iter_mut() {
        let start_new = current.as_ref().is_none_or(|session| {
            visit.visit_time_ms - session.record.last_visit_ms > super::SESSION_GAP_MS
        });
        if start_new {
            if let Some(session) = current.take() {
                sessions.push(session.finish());
            }
            current = Some(SessionBuildState::new(&super::StructuralVisitRecord {
                visit_id: visit.visit_id,
                profile_id: visit.profile_id.clone(),
                url: visit.url.clone(),
                visit_time_ms: visit.visit_time_ms,
                from_visit: visit.from_visit,
                registrable_domain: visit.registrable_domain.clone(),
                search_engine: visit.search_engine.clone(),
                search_query: visit.search_query.clone(),
                is_new_domain: visit.is_new_domain,
                is_search_event: visit.is_search_event,
            }));
        }
        let session = current.as_mut().expect("current session");
        session.push_visit(&super::StructuralVisitRecord {
            visit_id: visit.visit_id,
            profile_id: visit.profile_id.clone(),
            url: visit.url.clone(),
            visit_time_ms: visit.visit_time_ms,
            from_visit: visit.from_visit,
            registrable_domain: visit.registrable_domain.clone(),
            search_engine: visit.search_engine.clone(),
            search_query: visit.search_query.clone(),
            is_new_domain: visit.is_new_domain,
            is_search_event: visit.is_search_event,
        });
        visit.session_id = Some(session.record.session_id.clone());
    }
    if let Some(session) = current.take() {
        sessions.push(session.finish());
    }
    sessions
}

/// Builds deterministic search trails and search-event rows from visits that
/// have already been normalized and session-grouped.
pub(super) fn build_search_trails(
    visits: &mut [VisitRecord],
) -> (Vec<SearchEventRecord>, Vec<TrailRecord>) {
    let mut search_events = Vec::new();
    let mut trails = Vec::new();
    let mut current: Option<TrailRecord> = None;

    for visit in visits.iter_mut() {
        let visit_query = visit.search_query.clone();
        if visit.is_search_event {
            search_events.push(SearchEventRecord {
                visit_id: visit.visit_id,
                profile_id: visit.profile_id.clone(),
                search_engine: visit.search_engine.clone().unwrap_or_else(|| "unknown".to_string()),
                raw_query: visit_query.clone().unwrap_or_default(),
                normalized_query: visit_query.as_deref().map(normalize_query).unwrap_or_default(),
                query_kind: SearchQueryKind::Keyword,
                trail_id: None,
                visit_time_ms: visit.visit_time_ms,
            });
            if let Some(trail) = current.take() {
                trails.push(trail);
            }
            let query = visit_query.unwrap_or_else(|| "search".to_string());
            let trail_id = format!("trail:{}:{}", visit.profile_id, visit.visit_id);
            visit.trail_id = Some(trail_id.clone());
            if let Some(last) = search_events.last_mut() {
                last.trail_id = Some(trail_id.clone());
            }
            current = Some(TrailRecord {
                trail_id: trail_id.clone(),
                profile_id: visit.profile_id.clone(),
                session_id: visit.session_id.clone().unwrap_or_default(),
                initial_query: query.clone(),
                search_engine: visit.search_engine.clone().unwrap_or_else(|| "unknown".to_string()),
                reformulation_count: 0,
                visit_count: 1,
                landing_url: None,
                landing_domain: None,
                first_visit_ms: visit.visit_time_ms,
                last_visit_ms: visit.visit_time_ms,
                max_depth: 0,
                queries: vec![query],
                members: vec![TrailMemberRecord {
                    trail_id,
                    profile_id: visit.profile_id.clone(),
                    visit_id: visit.visit_id,
                    ordinal: 0,
                    role: "search_event".to_string(),
                }],
            });
            continue;
        }

        let Some(trail) = current.as_mut() else {
            continue;
        };
        if trail.session_id != visit.session_id.clone().unwrap_or_default()
            || visit.visit_time_ms - trail.last_visit_ms > super::TRAIL_GAP_MS
        {
            let finished = current.take().expect("trail");
            trails.push(finished);
            continue;
        }

        let depth = trail.members.len() as i64;
        let role = if trail.landing_url.is_none() { "landing" } else { "click" };
        trail.visit_count += 1;
        trail.last_visit_ms = visit.visit_time_ms;
        trail.max_depth = trail.max_depth.max(depth);
        trail.landing_url.get_or_insert_with(|| visit.url.clone());
        trail.landing_domain.get_or_insert_with(|| visit.registrable_domain.clone());
        trail.members.push(TrailMemberRecord {
            trail_id: trail.trail_id.clone(),
            profile_id: trail.profile_id.clone(),
            visit_id: visit.visit_id,
            ordinal: depth,
            role: role.to_string(),
        });
        visit.trail_id = Some(trail.trail_id.clone());
    }
    if let Some(trail) = current.take() {
        trails.push(trail);
    }

    let landing_domains = trails
        .iter()
        .map(|trail| (trail.trail_id.clone(), trail.landing_domain.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    for event in &mut search_events {
        event.query_kind = classify_search_query_kind(
            &event.raw_query,
            &event.normalized_query,
            event
                .trail_id
                .as_ref()
                .and_then(|trail_id| landing_domains.get(trail_id))
                .and_then(|domain| domain.as_deref()),
        );
    }

    let mut trail_events = std::collections::HashMap::<String, Vec<String>>::new();
    for event in &search_events {
        if let Some(trail_id) = &event.trail_id {
            trail_events.entry(trail_id.clone()).or_default().push(event.raw_query.clone());
        }
    }
    for trail in &mut trails {
        let queries = trail_events.get(&trail.trail_id).cloned().unwrap_or_default();
        let deduped = queries.into_iter().fold(Vec::<String>::new(), |mut acc, query| {
            if acc.last().is_none_or(|last| normalize_query(last) != normalize_query(&query)) {
                acc.push(query);
            }
            acc
        });
        if let Some(first) = deduped.first() {
            trail.initial_query = first.clone();
            trail.queries = deduped.clone();
            trail.reformulation_count = deduped.len().saturating_sub(1) as i64;
        }
    }

    (search_events, trails)
}

/// Groups keyword search events into deterministic families using the accepted
/// normalized-query similarity heuristic.
pub(super) fn build_query_families(events: &[SearchEventRecord]) -> Vec<QueryFamilyRecord> {
    let mut families: Vec<QueryFamilyRecord> = Vec::new();
    for event in events.iter().filter(|event| event.query_kind.is_keyword()) {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            continue;
        }
        let mut matched = None;
        for (index, family) in families.iter_mut().enumerate() {
            if family.profile_id != event.profile_id || family.search_engine != event.search_engine
            {
                continue;
            }
            let family_tokens = query_token_set(&normalize_query(&family.anchor_query));
            if super::jaccard(&tokens, &family_tokens) >= 0.5
                || tokens.is_subset(&family_tokens)
                || family_tokens.is_subset(&tokens)
            {
                family.member_count += 1;
                family.last_seen_ms = family.last_seen_ms.max(event.visit_time_ms);
                if !family
                    .queries
                    .iter()
                    .any(|query| normalize_query(query) == event.normalized_query)
                {
                    family.queries.push(event.raw_query.clone());
                }
                matched = Some(index);
                break;
            }
        }
        if matched.is_none() {
            families.push(QueryFamilyRecord {
                family_id: format!("family:{}:{:04}", event.profile_id, families.len() + 1),
                profile_id: event.profile_id.clone(),
                anchor_query: event.raw_query.clone(),
                member_count: 1,
                search_engine: event.search_engine.clone(),
                first_seen_ms: event.visit_time_ms,
                last_seen_ms: event.visit_time_ms,
                queries: vec![event.raw_query.clone()],
            });
        }
    }
    families
}

/// Test helper that reloads persisted search events for one profile in a stable
/// order so in-memory and batched grouping can be compared directly.
#[cfg(test)]
pub(super) fn load_profile_search_events(
    connection: &Connection,
    profile_id: &str,
) -> Result<Vec<SearchEventRecord>> {
    let mut statement = connection.prepare(
        "SELECT search_events.visit_id,
                search_events.profile_id,
                search_events.search_engine,
                search_events.raw_query,
                search_events.normalized_query,
                search_events.query_kind,
                search_events.trail_id,
                archive.visits.visit_time_ms
         FROM search_events
         JOIN archive.visits ON archive.visits.id = search_events.visit_id
         WHERE profile_id = ?1
         ORDER BY visit_id ASC",
    )?;
    statement
        .query_map([profile_id], |row| {
            Ok(SearchEventRecord {
                visit_id: row.get(0)?,
                profile_id: row.get(1)?,
                search_engine: row.get(2)?,
                raw_query: row.get(3)?,
                normalized_query: row.get(4)?,
                query_kind: super::parse_search_query_kind(&row.get::<_, String>(5)?),
                trail_id: row.get(6)?,
                visit_time_ms: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Streams persisted search events in visit-id order for the batched query
/// family builder.
fn load_profile_search_event_batch(
    connection: &Connection,
    profile_id: &str,
    after: Option<SearchEventBatchCursor>,
    limit: usize,
) -> Result<Vec<SearchEventRecord>> {
    let mut statement = connection.prepare(
        "SELECT search_events.visit_id,
                search_events.profile_id,
                search_events.search_engine,
                search_events.raw_query,
                search_events.normalized_query,
                search_events.query_kind,
                search_events.trail_id,
                archive.visits.visit_time_ms
         FROM search_events
         JOIN archive.visits ON archive.visits.id = search_events.visit_id
         WHERE search_events.profile_id = ?1
           AND (?2 IS NULL OR search_events.visit_id > ?2)
         ORDER BY search_events.visit_id ASC
         LIMIT ?3",
    )?;
    statement
        .query_map(
            params![profile_id, after.map(|cursor| cursor.visit_id), limit.max(1) as i64],
            |row| {
                Ok(SearchEventRecord {
                    visit_id: row.get(0)?,
                    profile_id: row.get(1)?,
                    search_engine: row.get(2)?,
                    raw_query: row.get(3)?,
                    normalized_query: row.get(4)?,
                    query_kind: super::parse_search_query_kind(&row.get::<_, String>(5)?),
                    trail_id: row.get(6)?,
                    visit_time_ms: row.get(7)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Rebuilds query families by streaming persisted search events instead of
/// loading unrelated structural aggregates into memory.
pub(super) fn build_query_families_from_batches(
    connection: &Connection,
    profile_id: &str,
) -> Result<Vec<QueryFamilyRecord>> {
    let mut accumulator = QueryFamilyAccumulator::default();
    let mut cursor = None;
    loop {
        let batch = load_profile_search_event_batch(
            connection,
            profile_id,
            cursor,
            super::STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        for event in &batch {
            accumulator.add_event(event);
        }
        cursor = batch.last().map(|event| SearchEventBatchCursor { visit_id: event.visit_id });
    }
    Ok(accumulator.finish())
}

impl QueryFamilyAccumulator {
    /// Merges one ordered search event into the current family set without
    /// materializing a second copy of every family token set.
    fn add_event(&mut self, event: &SearchEventRecord) {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            return;
        }
        for family in &mut self.families {
            if family.profile_id != event.profile_id || family.search_engine != event.search_engine
            {
                continue;
            }
            let family_tokens = query_token_set(&normalize_query(&family.anchor_query));
            if super::jaccard(&tokens, &family_tokens) >= 0.5
                || tokens.is_subset(&family_tokens)
                || family_tokens.is_subset(&tokens)
            {
                family.member_count += 1;
                family.last_seen_ms = family.last_seen_ms.max(event.visit_time_ms);
                if !family
                    .queries
                    .iter()
                    .any(|query| normalize_query(query) == event.normalized_query)
                {
                    family.queries.push(event.raw_query.clone());
                }
                return;
            }
        }
        self.families.push(QueryFamilyRecord {
            family_id: format!("family:{}:{:04}", event.profile_id, self.families.len() + 1),
            profile_id: event.profile_id.clone(),
            anchor_query: event.raw_query.clone(),
            member_count: 1,
            search_engine: event.search_engine.clone(),
            first_seen_ms: event.visit_time_ms,
            last_seen_ms: event.visit_time_ms,
            queries: vec![event.raw_query.clone()],
        });
    }

    /// Finishes the batched family build with stable ordering preserved from the
    /// streamed event input.
    fn finish(self) -> Vec<QueryFamilyRecord> {
        self.families
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visit(visit_id: i64, is_search_event: bool, session_id: Option<&str>) -> VisitRecord {
        VisitRecord {
            visit_id,
            profile_id: "chrome:Default".to_string(),
            source_profile_id: 1,
            source_visit_id: visit_id,
            source_url_id: visit_id,
            url: format!("https://example.com/{visit_id}"),
            title: Some(format!("Visit {visit_id}")),
            visit_time_ms: 1_762_000_000_000 + visit_id,
            from_visit: None,
            transition_type: None,
            external_referrer_url: None,
            canonical_url: format!("https://example.com/{visit_id}"),
            registrable_domain: "example.com".to_string(),
            domain_category: "docs".to_string(),
            page_category: "article-page".to_string(),
            search_engine: is_search_event.then(|| "google".to_string()),
            search_query: is_search_event.then(|| "rust coverage".to_string()),
            is_new_domain: visit_id == 1,
            is_search_event,
            evidence_tier: "tier-a".to_string(),
            taxonomy_source: "test".to_string(),
            taxonomy_pack: None,
            taxonomy_version: None,
            display_name: Some("Example".to_string()),
            session_id: session_id.map(str::to_string),
            trail_id: None,
        }
    }

    fn search_event(
        visit_id: i64,
        profile_id: &str,
        search_engine: &str,
        raw_query: &str,
        normalized_query: &str,
    ) -> SearchEventRecord {
        SearchEventRecord {
            visit_id,
            profile_id: profile_id.to_string(),
            search_engine: search_engine.to_string(),
            raw_query: raw_query.to_string(),
            normalized_query: normalized_query.to_string(),
            query_kind: SearchQueryKind::Keyword,
            trail_id: None,
            visit_time_ms: 1_762_000_000_000 + visit_id,
        }
    }

    #[test]
    fn search_trail_builder_ignores_pre_search_visits_and_flushes_final_trail() {
        let mut visits = vec![
            visit(1, false, Some("session:1")),
            visit(2, true, Some("session:1")),
            visit(3, false, Some("session:1")),
            visit(4, true, Some("session:1")),
        ];

        let (events, trails) = build_search_trails(&mut visits);

        assert!(visits[0].trail_id.is_none());
        assert_eq!(events.len(), 2);
        assert_eq!(trails.len(), 2);
        assert_eq!(trails[0].members.len(), 2);
        assert_eq!(visits[2].trail_id, Some(trails[0].trail_id.clone()));
        assert_eq!(visits[3].trail_id, Some(trails[1].trail_id.clone()));
    }

    #[test]
    fn query_family_builders_skip_empty_tokens_and_scope_by_profile_and_engine() {
        let events = vec![
            search_event(1, "chrome:Default", "google", "a", ""),
            search_event(2, "chrome:Default", "google", "rust coverage", "rust coverage"),
            search_event(3, "chrome:Default", "bing", "rust coverage", "rust coverage"),
            search_event(4, "firefox:Default", "google", "rust coverage", "rust coverage"),
        ];

        let families = build_query_families(&events);
        assert_eq!(families.len(), 3);

        let mut accumulator = QueryFamilyAccumulator::default();
        accumulator.add_event(&events[0]);
        assert!(accumulator.finish().is_empty());
    }
}
