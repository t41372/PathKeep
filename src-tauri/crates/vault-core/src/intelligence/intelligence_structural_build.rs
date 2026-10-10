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
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use vault_store::{Session, params_owned as params};

/// How alike a search is to a family's first search, as a fraction compared exactly: the tokens
/// they share over all the tokens either has.
#[derive(Debug, Clone, Copy)]
struct Likeness {
    shared: usize,
    all: usize,
}

impl Likeness {
    /// A search of `search` tokens and a first search of `first`, `shared` of them in both.
    fn of(shared: usize, search: usize, first: usize) -> Self {
        Self { shared, all: search + first - shared }
    }

    /// The most a first search of `first` tokens can be like a search of `search` tokens.
    fn at_most(search: usize, first: usize) -> Self {
        Self { shared: search.min(first), all: search.max(first) }
    }

    /// Whether the search may join the family: they share at least half of all their tokens
    /// and at least two, or have the same tokens (`deterministic-intelligence.md` §5.4).
    fn joins(self, search: usize, first: usize) -> bool {
        (self.shared >= 2 && 2 * self.shared >= self.all)
            || (self.shared == search && self.shared == first)
    }

    fn cmp(self, other: Self) -> Ordering {
        (self.shared * other.all).cmp(&(other.shared * self.all))
    }
}

/// Whether `candidate` beats `best`: more alike, or as alike and made earlier.
fn better(candidate: (Likeness, usize), best: Option<(Likeness, usize)>) -> bool {
    best.is_none_or(|(top, earliest)| match candidate.0.cmp(top) {
        Ordering::Greater => true,
        Ordering::Equal => candidate.1 < earliest,
        Ordering::Less => false,
    })
}

/// Groups one profile's keyword searches into query families as they come, oldest first
/// (`deterministic-intelligence.md` §5.4). A search goes to the family that already holds its
/// wording; else, when it continues the trail of the search just before it, to that search's
/// family; else to the family whose first search it is most like; else it starts a family.
///
/// Families are compared by their first search only. Compared by every wording, a family
/// chained through them: a one-word wording took every search with that word, and at 576,000
/// searches one family held 164,788 of 304,835 wordings, the build taking 46 minutes. A search
/// that held every word of a short first search, or every word of it held by one, joined too;
/// short first searches took any longer search with their words (6,683 wordings in one family
/// begun by two words, and 13% of all searches in families begun by one). The first searches
/// are indexed by token and size, and a search reads only the sizes that could still beat the
/// best it has found, and at each size only the tokens with the fewest first searches
/// (`candidates`).
#[derive(Debug, Default)]
pub(super) struct QueryFamilyAccumulator {
    families: Vec<QueryFamilyRecord>,
    /// A number for each profile and engine: families never cross them.
    scopes: HashMap<String, HashMap<String, usize>>,
    scope_count: usize,
    /// A number for each token of a first search.
    token_ids: HashMap<String, u32>,
    /// Each family's first search, as its token numbers in order.
    firsts: Vec<Vec<u32>>,
    /// `(scope, token, size)` → the families whose first search has the token and `size`
    /// tokens, oldest first.
    by_token: HashMap<(usize, u32, usize), Vec<usize>>,
    /// The most tokens a first search has, per scope.
    largest: HashMap<usize, usize>,
    /// Per scope, the family that holds each wording, so the same search always lands in the
    /// same family.
    by_query: HashMap<usize, HashMap<String, usize>>,
    queries: Vec<HashSet<String>>,
    days: Vec<HashSet<String>>,
    /// The trail of the last search added, and the family it went to.
    last_trail: Option<(usize, String, usize)>,
}

/// Groups keyword search events into query families the plain way, comparing each search with
/// every family's first search in turn. `QueryFamilyAccumulator` must group exactly like this.
#[cfg(test)]
pub(super) fn build_query_families(events: &[SearchEventRecord]) -> Vec<QueryFamilyRecord> {
    let mut families: Vec<QueryFamilyRecord> = Vec::new();
    let mut days: Vec<HashSet<String>> = Vec::new();
    let mut last_trail = None::<(String, String, String, usize)>;
    for event in events.iter().filter(|event| event.query_kind.is_keyword()) {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            last_trail = None;
            continue;
        }
        let in_scope = |family: &QueryFamilyRecord| {
            family.profile_id == event.profile_id && family.search_engine == event.search_engine
        };
        let mut matched = families.iter().position(|family| {
            in_scope(family)
                && family
                    .queries
                    .iter()
                    .any(|query| normalize_query(query) == event.normalized_query)
        });
        if matched.is_none() {
            matched = match (&last_trail, &event.trail_id) {
                (Some((profile, engine, trail, family)), Some(current))
                    if *profile == event.profile_id
                        && *engine == event.search_engine
                        && trail == current =>
                {
                    Some(*family)
                }
                _ => None,
            };
        }
        if matched.is_none() {
            let mut best = None::<(Likeness, usize)>;
            for (index, family) in
                families.iter().enumerate().filter(|(_, family)| in_scope(family))
            {
                let first = query_token_set(&normalize_query(&family.anchor_query));
                let likeness =
                    Likeness::of(tokens.intersection(&first).count(), tokens.len(), first.len());
                if likeness.joins(tokens.len(), first.len()) && better((likeness, index), best) {
                    best = Some((likeness, index));
                }
            }
            matched = best.map(|(_, index)| index);
        }
        let index = match matched {
            Some(index) => {
                let family = &mut families[index];
                family.member_count += 1;
                family.last_seen_ms = family.last_seen_ms.max(event.visit_time_ms);
                if !family
                    .queries
                    .iter()
                    .any(|query| normalize_query(query) == event.normalized_query)
                {
                    family.queries.push(event.raw_query.clone());
                }
                if days[index].insert(local_date_key(event.visit_time_ms)) {
                    family.active_days += 1;
                }
                index
            }
            None => {
                days.push(HashSet::from([local_date_key(event.visit_time_ms)]));
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
                families.len() - 1
            }
        };
        last_trail = event
            .trail_id
            .clone()
            .map(|trail| (event.profile_id.clone(), event.search_engine.clone(), trail, index));
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
    /// were made, each with the days it was searched on counted, and `last` is the last search
    /// added to them, whose trail the next search may continue. Every event added after this
    /// must be later than the events already in them (an append: an earlier visit makes the
    /// build start over), so an event's day is new unless it is its family's last day.
    pub(super) fn resume(
        families: Vec<QueryFamilyRecord>,
        last: Option<SearchEventRecord>,
    ) -> Self {
        let mut accumulator = Self::default();
        for family in families {
            let index = accumulator.families.len();
            let scope = accumulator.scope(&family.profile_id, &family.search_engine);
            accumulator.add_first(scope, &query_token_set(&normalize_query(&family.anchor_query)));
            let queries =
                family.queries.iter().map(|query| normalize_query(query)).collect::<HashSet<_>>();
            let by_query = accumulator.by_query.entry(scope).or_default();
            for query in &queries {
                by_query.entry(query.clone()).or_insert(index);
            }
            accumulator.queries.push(queries);
            accumulator.days.push(HashSet::from([local_date_key(family.last_seen_ms)]));
            accumulator.families.push(family);
        }
        if let Some(event) = last {
            let scope = accumulator.scope(&event.profile_id, &event.search_engine);
            let family = accumulator
                .by_query
                .get(&scope)
                .and_then(|queries| queries.get(&event.normalized_query))
                .copied();
            accumulator.last_trail =
                event.trail_id.zip(family).map(|(trail, family)| (scope, trail, family));
        }
        accumulator
    }

    /// The families as they stand, in the order they were made.
    pub(super) fn families(&self) -> &[QueryFamilyRecord] {
        &self.families
    }

    /// Adds one search, later than every search added before it.
    pub(super) fn add_event(&mut self, event: &SearchEventRecord) {
        let tokens = query_token_set(&event.normalized_query);
        if tokens.is_empty() {
            self.last_trail = None;
            return;
        }
        let scope = self.scope(&event.profile_id, &event.search_engine);
        let joined = self
            .by_query
            .get(&scope)
            .and_then(|queries| queries.get(&event.normalized_query))
            .copied()
            .or_else(|| match (&self.last_trail, &event.trail_id) {
                (Some((last_scope, trail, family)), Some(current))
                    if *last_scope == scope && trail == current =>
                {
                    Some(*family)
                }
                _ => None,
            })
            .or_else(|| self.most_like(scope, &tokens));
        let index = match joined {
            Some(index) => {
                let family = &mut self.families[index];
                family.member_count += 1;
                family.last_seen_ms = family.last_seen_ms.max(event.visit_time_ms);
                if self.days[index].insert(local_date_key(event.visit_time_ms)) {
                    family.active_days += 1;
                }
                if self.queries[index].insert(event.normalized_query.clone()) {
                    family.queries.push(event.raw_query.clone());
                    self.by_query
                        .entry(scope)
                        .or_default()
                        .insert(event.normalized_query.clone(), index);
                }
                index
            }
            None => {
                let index = self.families.len();
                self.add_first(scope, &tokens);
                self.by_query
                    .entry(scope)
                    .or_default()
                    .insert(event.normalized_query.clone(), index);
                self.queries.push(HashSet::from([event.normalized_query.clone()]));
                self.days.push(HashSet::from([local_date_key(event.visit_time_ms)]));
                self.families.push(QueryFamilyRecord {
                    family_id: format!("family:{}:{:04}", event.profile_id, index + 1),
                    profile_id: event.profile_id.clone(),
                    anchor_query: event.raw_query.clone(),
                    member_count: 1,
                    search_engine: event.search_engine.clone(),
                    first_seen_ms: event.visit_time_ms,
                    last_seen_ms: event.visit_time_ms,
                    queries: vec![event.raw_query.clone()],
                    active_days: 1,
                });
                index
            }
        };
        self.last_trail = event.trail_id.clone().map(|trail| (scope, trail, index));
    }

    /// The number of a profile and engine.
    fn scope(&mut self, profile: &str, engine: &str) -> usize {
        if let Some(scope) = self.scopes.get(profile).and_then(|engines| engines.get(engine)) {
            return *scope;
        }
        let scope = self.scope_count;
        self.scope_count += 1;
        self.scopes.entry(profile.to_owned()).or_default().insert(engine.to_owned(), scope);
        scope
    }

    /// Indexes the first search of the family about to be added.
    fn add_first(&mut self, scope: usize, tokens: &HashSet<String>) {
        let family = self.firsts.len();
        let mut ids = tokens
            .iter()
            .map(|token| {
                let next = self.token_ids.len() as u32;
                *self.token_ids.entry(token.clone()).or_insert(next)
            })
            .collect::<Vec<_>>();
        ids.sort_unstable();
        for &id in &ids {
            self.by_token.entry((scope, id, ids.len())).or_default().push(family);
        }
        let largest = self.largest.entry(scope).or_default();
        *largest = (*largest).max(ids.len());
        self.firsts.push(ids);
    }

    /// The family whose first search the search with `tokens` is most like, the earliest made
    /// on a tie, if it may join any.
    fn most_like(&self, scope: usize, tokens: &HashSet<String>) -> Option<usize> {
        let size = tokens.len();
        let mut known = tokens
            .iter()
            .filter_map(|token| self.token_ids.get(token).copied())
            .collect::<Vec<_>>();
        if known.is_empty() {
            return None;
        }
        known.sort_unstable();
        // First searches by size, those that could be most like it first.
        let mut sizes = (1..=self.largest.get(&scope).copied().unwrap_or(0)).collect::<Vec<_>>();
        sizes.sort_by(|&a, &b| Likeness::at_most(size, b).cmp(Likeness::at_most(size, a)));
        let mut best = None::<(Likeness, usize)>;
        for first_size in sizes {
            if best.is_some_and(|(top, _)| Likeness::at_most(size, first_size).cmp(top).is_lt()) {
                break;
            }
            for family in self.candidates(scope, &known, size, first_size) {
                let likeness = Likeness::of(shared(&known, &self.firsts[family]), size, first_size);
                if likeness.joins(size, first_size) && better((likeness, family), best) {
                    best = Some((likeness, family));
                }
            }
        }
        best.map(|(_, family)| family)
    }

    /// The families whose first search has `first_size` tokens and may be one a search of
    /// `size` tokens joins (`known` are its tokens some first search has). Such a first search
    /// shares at least `least` of them, so it has one of any `known.len() - least + 1` of
    /// them: those with the fewest first searches of that size are read.
    fn candidates(
        &self,
        scope: usize,
        known: &[u32],
        size: usize,
        first_size: usize,
    ) -> Vec<usize> {
        // Half of all their tokens and at least two; a one-token search or first search joins
        // only the same token.
        let least = if size == 1 || first_size == 1 {
            if size != first_size {
                return Vec::new();
            }
            1
        } else {
            (size + first_size).div_ceil(3).max(2)
        };
        if least > known.len() || least > size.min(first_size) {
            return Vec::new();
        }
        let mut lists = known
            .iter()
            .map(|&token| {
                self.by_token.get(&(scope, token, first_size)).map_or(&[][..], Vec::as_slice)
            })
            .collect::<Vec<_>>();
        lists.sort_by_key(|list| list.len());
        let mut found = lists[..known.len() - least + 1].concat();
        found.sort_unstable();
        found.dedup();
        found
    }

    /// Finishes the batched family build with stable ordering preserved from the
    /// streamed event input.
    pub(super) fn finish(self) -> Vec<QueryFamilyRecord> {
        self.families
    }
}

/// How many tokens two sorted token lists share.
fn shared(left: &[u32], right: &[u32]) -> usize {
    let (mut i, mut j, mut count) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        match left[i].cmp(&right[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                count += 1;
                i += 1;
                j += 1;
            }
        }
    }
    count
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
    /// Searches in order, each `(trail, query)`; every one in profile `chrome:Default` on Google.
    fn trail_events(searches: &[(&str, &str)]) -> Vec<SearchEventRecord> {
        searches
            .iter()
            .enumerate()
            .map(|(index, (trail, query))| SearchEventRecord {
                trail_id: Some((*trail).to_string()),
                ..search_event(index as i64 + 1, "chrome:Default", "google", query, query)
            })
            .collect()
    }

    /// Each family as its first search and how many searches it holds, by the accumulator, and
    /// checked against the reference builder (every first search compared in turn).
    fn grouped(events: &[SearchEventRecord]) -> Vec<(String, i64)> {
        let mut stream = QueryFamilyAccumulator::default();
        for event in events {
            stream.add_event(event);
        }
        let rows = |families: Vec<QueryFamilyRecord>| {
            families
                .into_iter()
                .map(|family| (family.anchor_query, family.member_count))
                .collect::<Vec<_>>()
        };
        let grouped = rows(stream.finish());
        assert_eq!(grouped, rows(build_query_families(events)), "the reference builder");
        grouped
    }

    fn family(anchor: &str, members: i64) -> (String, i64) {
        (anchor.to_string(), members)
    }

    // Failure modes of the family rule (`deterministic-intelligence.md` §5.4):
    // (a) a rewording in the trail of the search just before it, unlike the family's first
    //     search, starts a family of its own (台北天氣預報 … 台北天氣 v2 eg showed twice);
    // (b) a search like only a wording that a trail brought in joins that family, so families
    //     chain through their wordings (one family took 164,788 of 304,835 wordings);
    // (c) a search like two first searches goes to the first family made rather than the one it
    //     is most like, or a tie goes to the later family;
    // (c') a short first search takes every longer search with its words, so a family grows into
    //     a hub (one begun by two words held 6,683 wordings; families begun by one word held 13%
    //     of all searches);
    // (d) a wording already in a family is counted in another;
    // (e) an append that continues a trail left open by the build before it groups differently
    //     from a build from scratch;
    // (f) the indexed lookup misses a first search that comparing with every one would find.

    /// (a), and the spec's own ladder searched broad to narrow.
    #[test]
    fn a_rewording_in_its_trail_joins_the_family_of_the_search_before_it() {
        let events = trail_events(&[
            ("t1", "台北天氣預報"),
            ("t2", "台北天氣"),
            ("t2", "台北天氣 v2"),
            ("t2", "台北天氣 v2 eg"),
            ("t3", "sqlite wal"),
            ("t4", "sqlite wal too large"),
            ("t5", "sqlite wal checkpoint not working"),
        ]);
        assert_eq!(grouped(&events), vec![family("台北天氣預報", 4), family("sqlite wal", 3)]);
    }

    /// (b): python joined through its trail; a later search that shares only python with the
    /// family's first search is a new thing, not a rewording of it.
    #[test]
    fn a_search_like_only_a_wording_its_trail_brought_in_starts_its_own_family() {
        let events = trail_events(&[
            ("t1", "python asyncio"),
            ("t1", "python"),
            ("t2", "python list comprehension"),
            ("t3", "list comprehension"),
        ]);
        assert_eq!(
            grouped(&events),
            vec![family("python asyncio", 2), family("python list comprehension", 2)]
        );
    }

    /// (c): most alike wins, the earliest family on a tie.
    #[test]
    fn a_search_goes_to_the_first_search_it_is_most_like() {
        let events = trail_events(&[
            ("t1", "a7c ii autofocus"),
            ("t2", "fujifilm x-t5 vs a7c ii"),
            // 2 of 3 words of the first, 2 of 5 of the second.
            ("t3", "a7c ii"),
            // All 5 of the second and one more; 2 of 7 with the first.
            ("t4", "a7c ii fujifilm x-t5 review"),
            ("t5", "rust tokio spawn blocking"),
            ("t6", "rust tokio select macro"),
            // Half of each: the earlier.
            ("t7", "rust tokio"),
        ]);
        assert_eq!(
            grouped(&events),
            vec![
                family("a7c ii autofocus", 2),
                family("fujifilm x-t5 vs a7c ii", 2),
                family("rust tokio spawn blocking", 2),
                family("rust tokio select macro", 1),
            ]
        );
    }

    /// (c'): a word on its own is not a rewording of every search with it; nor is a search that
    /// shares only it.
    #[test]
    fn a_short_first_search_does_not_take_every_search_with_its_words() {
        let events = trail_events(&[
            ("t1", "python"),
            ("t2", "python asyncio"),
            ("t3", "python"),
            ("t4", "python decorators"),
            // Two of six words: not the same thing as the first search, though it holds it.
            ("t5", "python asyncio event loop blocking call"),
        ]);
        assert_eq!(
            grouped(&events),
            vec![
                family("python", 2),
                family("python asyncio", 1),
                family("python decorators", 1),
                family("python asyncio event loop blocking call", 1),
            ]
        );
    }

    /// (d): a wording keeps its family even inside a trail that began in another; the trail
    /// goes on from the family that search went to.
    #[test]
    fn a_wording_already_in_a_family_stays_there_inside_another_trail() {
        let events = trail_events(&[
            ("t1", "rust tokio"),
            ("t2", "rust smol"),
            ("t3", "rust smol"),
            ("t3", "rust tokio"),
            ("t3", "rust tokio runtime"),
        ]);
        assert_eq!(grouped(&events), vec![family("rust tokio", 3), family("rust smol", 2)]);
    }

    /// (e): the append's first search rewords the last search of the build before it.
    #[test]
    fn an_append_that_continues_an_open_trail_groups_like_a_build_from_scratch() {
        let events = trail_events(&[
            ("t1", "台北天氣預報"),
            ("t2", "台北天氣"),
            ("t2", "台北天氣 v2"),
            ("t2", "台北天氣 v2 eg"),
            ("t3", "sqlite wal"),
        ]);
        let rows = |families: &[QueryFamilyRecord]| {
            families
                .iter()
                .map(|f| (f.family_id.clone(), f.queries.clone(), f.member_count))
                .collect::<Vec<_>>()
        };
        let mut whole = QueryFamilyAccumulator::default();
        for event in &events {
            whole.add_event(event);
        }
        let mut before = QueryFamilyAccumulator::default();
        for event in &events[..3] {
            before.add_event(event);
        }
        let mut appended = QueryFamilyAccumulator::resume(before.finish(), Some(events[2].clone()));
        for event in &events[3..] {
            appended.add_event(event);
        }
        assert_eq!(rows(appended.families()), rows(whole.families()));
    }

    /// (f): a long history with common words, short and long searches and Chinese, in trails,
    /// in two profiles and two engines, grouped by the indexed lookup and by comparing every
    /// first search in turn.
    #[test]
    fn the_indexed_lookup_finds_what_comparing_every_first_search_finds() {
        let mut random = crate::test_support::search_history::Random(11);
        let mut trail = 0;
        let events = crate::test_support::search_history::synthetic_searches(6_000)
            .into_iter()
            .enumerate()
            .map(|(index, query)| {
                if random.unit() > 0.3 {
                    trail += 1;
                }
                SearchEventRecord {
                    visit_id: index as i64 + 1,
                    profile_id: format!("profile{}", index % 2),
                    search_engine: format!("engine{}", index / 2 % 2),
                    raw_query: query.clone(),
                    normalized_query: normalize_query(&query),
                    query_kind: SearchQueryKind::Keyword,
                    trail_id: Some(format!("trail{trail}")),
                    visit_time_ms: index as i64 * 60_000,
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
                        f.active_days,
                    )
                })
                .collect::<Vec<_>>()
        };
        let (stream, expected) = (rows(stream.finish()), rows(expected));
        assert!(expected.len() > 100 && expected.len() < 5_000, "{} families", expected.len());
        assert_eq!(stream, expected);
    }
}
