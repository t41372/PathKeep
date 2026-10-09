//! Failure modes: batch boundaries split equivalent query families.
//! Query-family builders for structural Core Intelligence. Sessions and search trails are
//! built while visits stream (`intelligence_structural_stream`).
//!
//! ## Responsibilities
//! - Group keyword search events into query families for both in-memory and
//!   batched rebuild paths.
//! - Keep the query-family similarity heuristic out of stage orchestration.
//!
//! ## Not responsible for
//! - Persisting structural rows into SQLite.
//! - Running refind or habit aggregate logic.
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

#[cfg(test)]
use super::SearchEventBatchCursor;
#[cfg(test)]
use super::SearchQueryKind;
use super::{
    QueryFamilyRecord, SearchEventRecord, local_date_key, normalize_query, query_token_set,
};
#[cfg(test)]
use anyhow::Result;
#[cfg(test)]
use vault_store::{Session, params_owned as params};

/// Minimal accumulator for building query families incrementally from ordered
/// search events.
#[derive(Debug, Default)]
pub(super) struct QueryFamilyAccumulator {
    families: Vec<QueryFamilyRecord>,
    /// `(profile, engine, token)` → the `(family, wording)` pairs whose wording has the token.
    by_token: std::collections::HashMap<(String, String, String), Vec<(usize, usize)>>,
    /// Each family's wordings, as the tokens they are compared by.
    wordings: Vec<Vec<std::collections::HashSet<String>>>,
    /// `(profile, engine, normalized query)` → the family that holds that wording, so the same
    /// search always lands in the same family.
    by_query: std::collections::HashMap<(String, String, String), usize>,
    queries: Vec<std::collections::HashSet<String>>,
    days: Vec<std::collections::HashSet<String>>,
}

/// Whether a search may join a family through one of its wordings: they share at least half
/// their tokens between them, or one holds the other (`deterministic-intelligence.md` §5.5).
fn similar(
    search: &std::collections::HashSet<String>,
    wording: &std::collections::HashSet<String>,
) -> bool {
    super::jaccard(search, wording) >= 0.5 || search.is_subset(wording) || wording.is_subset(search)
}

/// Groups keyword search events into deterministic families using the accepted
/// normalized-query similarity heuristic.
#[cfg(test)]
pub(super) fn build_query_families(events: &[SearchEventRecord]) -> Vec<QueryFamilyRecord> {
    let mut families: Vec<QueryFamilyRecord> = Vec::new();
    let mut days: Vec<std::collections::HashSet<String>> = Vec::new();
    for event in events.iter().filter(|event| event.query_kind.is_keyword()) {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            continue;
        }
        // The family that already holds this wording; else the one with the wording most like
        // it, the earliest made on a tie.
        let mut matched = families.iter().position(|family| {
            family.profile_id == event.profile_id
                && family.search_engine == event.search_engine
                && family
                    .queries
                    .iter()
                    .any(|query| normalize_query(query) == event.normalized_query)
        });
        if matched.is_none() {
            let mut best = None::<(f32, usize)>;
            for (index, family) in families.iter().enumerate() {
                if family.profile_id != event.profile_id
                    || family.search_engine != event.search_engine
                {
                    continue;
                }
                for query in &family.queries {
                    let wording = query_token_set(&normalize_query(query));
                    let score = super::jaccard(&tokens, &wording);
                    if similar(&tokens, &wording) && best.is_none_or(|(top, _)| score > top) {
                        best = Some((score, index));
                    }
                }
            }
            matched = best.map(|(_, index)| index);
        }
        if let Some(index) = matched {
            let family = &mut families[index];
            family.member_count += 1;
            family.last_seen_ms = family.last_seen_ms.max(event.visit_time_ms);
            if !family.queries.iter().any(|query| normalize_query(query) == event.normalized_query)
            {
                family.queries.push(event.raw_query.clone());
            }
            if days[index].insert(super::local_date_key(event.visit_time_ms)) {
                family.active_days += 1;
            }
        }
        if matched.is_none() {
            days.push(std::collections::HashSet::from([super::local_date_key(
                event.visit_time_ms,
            )]));
            families.push(QueryFamilyRecord {
                family_id: format!("family:{}:{:04}", event.profile_id, families.len() + 1),
                profile_id: event.profile_id.clone(),
                anchor_query: event.raw_query.clone(),
                member_count: 1,
                search_engine: event.search_engine.clone(),
                first_seen_ms: event.visit_time_ms,
                last_seen_ms: event.visit_time_ms,
                queries: vec![event.raw_query.clone()],
                active_days: 1,
            });
        }
    }
    families
}

/// Test helper that reloads persisted search events for one profile in a stable
/// order so in-memory and batched grouping can be compared directly.
#[cfg(test)]
pub(super) fn load_profile_search_events(
    connection: &Session<'_>,
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
                query_kind: super::parse_search_query_kind(&row.get::<String>(5)?),
                trail_id: row.get(6)?,
                visit_time_ms: row.get(7)?,
            })
        })?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Streams persisted search events in visit-id order for the batched query
/// family builder.
#[cfg(test)]
fn load_profile_search_event_batch(
    connection: &Session<'_>,
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
                    query_kind: super::parse_search_query_kind(&row.get::<String>(5)?),
                    trail_id: row.get(6)?,
                    visit_time_ms: row.get(7)?,
                })
            },
        )?
        .collect::<vault_store::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Rebuilds query families by streaming persisted search events instead of
/// loading unrelated structural aggregates into memory.
#[cfg(test)]
pub(super) fn build_query_families_from_batches(
    connection: &Session<'_>,
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
    /// Continues a build that stopped: `families` are one profile's families in the order they
    /// were made, each with the days it was searched on counted. Every event added after this
    /// must be later than the events already in them (an append: an earlier visit makes the
    /// build start over), so an event's day is new unless it is its family's last day.
    pub(super) fn resume(families: Vec<QueryFamilyRecord>) -> Self {
        let mut accumulator = Self::default();
        for family in families {
            let index = accumulator.families.len();
            accumulator.wordings.push(Vec::new());
            for query in &family.queries {
                let query = normalize_query(query);
                let tokens = query_token_set(&query);
                accumulator.add_wording(
                    index,
                    &family.profile_id,
                    &family.search_engine,
                    &query,
                    tokens,
                );
            }
            accumulator
                .queries
                .push(family.queries.iter().map(|query| normalize_query(query)).collect());
            accumulator
                .days
                .push(std::collections::HashSet::from([local_date_key(family.last_seen_ms)]));
            accumulator.families.push(family);
        }
        accumulator
    }

    /// The families as they stand, in the order they were made.
    pub(super) fn families(&self) -> &[QueryFamilyRecord] {
        &self.families
    }

    /// Merges one ordered search event into the current family set without
    /// materializing a second copy of every family token set.
    pub(super) fn add_event(&mut self, event: &SearchEventRecord) {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            return;
        }
        // The family that already holds this wording; else the one with the wording most like
        // it (the most tokens shared for those they have), the earliest made on a tie. The first
        // family with any like wording is not enough: a rewording shares the words its search
        // added with other searches reworded the same way.
        let held = self
            .by_query
            .get(&(
                event.profile_id.clone(),
                event.search_engine.clone(),
                event.normalized_query.clone(),
            ))
            .copied();
        let candidates = tokens
            .iter()
            .flat_map(|token| {
                self.by_token
                    .get(&(event.profile_id.clone(), event.search_engine.clone(), token.clone()))
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect::<std::collections::BTreeSet<_>>();
        let joined = held.or_else(|| {
            let mut best = None::<(f32, usize)>;
            for (family, wording) in candidates {
                let wording = &self.wordings[family][wording];
                let score = super::jaccard(&tokens, wording);
                if similar(&tokens, wording) && best.is_none_or(|(top, _)| score > top) {
                    best = Some((score, family));
                }
            }
            best.map(|(_, family)| family)
        });
        if let Some(index) = joined {
            let family = &mut self.families[index];
            family.member_count += 1;
            family.last_seen_ms = family.last_seen_ms.max(event.visit_time_ms);
            if self.days[index].insert(local_date_key(event.visit_time_ms)) {
                family.active_days += 1;
            }
            if self.queries[index].insert(event.normalized_query.clone()) {
                family.queries.push(event.raw_query.clone());
                let (profile, engine) = (family.profile_id.clone(), family.search_engine.clone());
                self.add_wording(index, &profile, &engine, &event.normalized_query, tokens);
            }
            return;
        }
        let index = self.families.len();
        self.wordings.push(Vec::new());
        self.add_wording(
            index,
            &event.profile_id,
            &event.search_engine,
            &event.normalized_query,
            tokens,
        );
        self.queries.push(std::collections::HashSet::from([event.normalized_query.clone()]));
        self.days.push(std::collections::HashSet::from([local_date_key(event.visit_time_ms)]));
        self.families.push(QueryFamilyRecord {
            family_id: format!("family:{}:{:04}", event.profile_id, self.families.len() + 1),
            profile_id: event.profile_id.clone(),
            anchor_query: event.raw_query.clone(),
            member_count: 1,
            search_engine: event.search_engine.clone(),
            first_seen_ms: event.visit_time_ms,
            last_seen_ms: event.visit_time_ms,
            queries: vec![event.raw_query.clone()],
            active_days: 1,
        });
    }

    /// Indexes one more wording of `family` by its text and tokens, so later searches can find
    /// it.
    fn add_wording(
        &mut self,
        family: usize,
        profile: &str,
        engine: &str,
        query: &str,
        tokens: std::collections::HashSet<String>,
    ) {
        self.by_query.insert((profile.to_owned(), engine.to_owned(), query.to_owned()), family);
        let wording = self.wordings[family].len();
        for token in &tokens {
            self.by_token
                .entry((profile.to_owned(), engine.to_owned(), token.clone()))
                .or_default()
                .push((family, wording));
        }
        self.wordings[family].push(tokens);
    }

    /// Finishes the batched family build with stable ordering preserved from the
    /// streamed event input.
    pub(super) fn finish(self) -> Vec<QueryFamilyRecord> {
        self.families
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// A family is compared by every wording in it, not only its first. Compared by its first
    /// alone, a family begun by 台北天氣預報 did not take 台北天氣 v2 eg, so the same search
    /// showed as two things the user came back to. And a search goes to the family it is most
    /// like, not the first one it is like enough.
    #[test]
    fn a_family_takes_any_wording_like_one_of_its_own() {
        let queries = [
            "台北天氣預報",
            "台北天氣",
            "台北天氣 v2 eg",
            "sqlite wal checkpoint not working",
            "sqlite wal",
            "sqlite wal too large",
            "tokio vs smol",
            "a7c ii autofocus",
            "a7c ii autofocus v2 eg",
            "fujifilm x-t5 vs a7c ii",
            "fujifilm x-t5 vs a7c ii v2 eg",
        ];
        let events = queries
            .iter()
            .enumerate()
            .map(|(index, query)| {
                search_event(index as i64 + 1, "chrome:Default", "google", query, query)
            })
            .collect::<Vec<_>>();
        let mut stream = QueryFamilyAccumulator::default();
        for event in &events {
            stream.add_event(event);
        }
        let anchors = |families: Vec<QueryFamilyRecord>| {
            families
                .into_iter()
                .map(|family| (family.anchor_query, family.member_count))
                .collect::<Vec<_>>()
        };
        let expected = vec![
            ("台北天氣預報".to_string(), 3),
            ("sqlite wal checkpoint not working".to_string(), 3),
            ("tokio vs smol".to_string(), 1),
            // Its rewording shares a7c, ii, v2 and eg with the other's (half), but more with it.
            ("a7c ii autofocus".to_string(), 2),
            ("fujifilm x-t5 vs a7c ii".to_string(), 2),
        ];
        assert_eq!(anchors(stream.finish()), expected);
        assert_eq!(anchors(build_query_families(&events)), expected, "the reference builder");
    }

    #[test]
    fn indexed_family_candidates_preserve_the_reference_first_match() {
        let events = (0..1003)
            .map(|i| {
                let query = match i % 5 {
                    0 => format!("topic{i} rust"),
                    1 => format!("topic{} browser", i - 1),
                    2 => "rust browser".into(),
                    3 => format!("unrelated{i}"),
                    _ => "browser".into(),
                };
                SearchEventRecord {
                    visit_id: i,
                    profile_id: format!("profile{}", i % 2),
                    search_engine: format!("engine{}", i % 3),
                    raw_query: query.clone(),
                    normalized_query: query,
                    query_kind: SearchQueryKind::Keyword,
                    trail_id: None,
                    visit_time_ms: i * 1000,
                }
            })
            .collect::<Vec<_>>();
        let expected = build_query_families(&events);
        let mut stream = QueryFamilyAccumulator::default();
        for event in &events {
            stream.add_event(event);
        }
        let rows = |families: Vec<QueryFamilyRecord>| {
            families
                .into_iter()
                .map(|f| {
                    (
                        f.family_id,
                        f.anchor_query,
                        f.member_count,
                        f.queries,
                        f.first_seen_ms,
                        f.last_seen_ms,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(rows(stream.finish()), rows(expected));
    }
}
