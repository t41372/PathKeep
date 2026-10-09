//! Failure modes: resumed streams duplicate members or change stable identifiers.
//! Shared structural rebuild state machines for Core Intelligence.
//!
//! ## Responsibilities
//! - Hold the mutable session/trail builders reused by both in-memory rebuilds
//!   and streamed structural replay.
//! - Keep session-title and trail-member bookkeeping out of higher-level stage
//!   orchestration.
//! - Preserve deterministic session/trail ids across both fallback and streamed
//!   rebuild paths.
//!
//! ## Not responsible for
//! - Running structural rebuild stages or deciding when fallbacks are needed.
//! - Persisting rows into SQLite.
//! - Building refind or habit aggregates.
//!
//! ## Dependencies
//! - Parent-module structural record types and domain-display helpers.
//! - Session and trail gap constants from the parent module.
//!
//! ## Performance notes
//! - These builders only hold state for the current session or trail and stay
//!   bounded regardless of full archive size.

use super::{
    SessionRecord, StructuralVisitRecord, TrailMemberRecord, TrailRecord, display_name_for_domain,
    jaccard, local_date_key, normalize_query, query_token_set,
};
use crate::visit_taxonomy::Opener;
use std::collections::{HashMap, HashSet, VecDeque};

/// The latest visits whose opener the walk can tell. Openers are nearly always a visit or a few
/// before; the window keeps the walk's state small when one session runs for days, and an opener
/// older than it counts as one PathKeep did not see.
const OPENER_WINDOW: usize = 512;

/// The walk's latest visits by the browser's id (what `from_visit` names): each one's site and
/// whether it was a search. Kept across sessions, since a tab left open is still clicked later.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct OpenerWindow {
    order: VecDeque<i64>,
    visits: HashMap<i64, (String, bool)>,
}

impl OpenerWindow {
    /// The site of the visit the browser calls `id`, and whether it was a search.
    pub(super) fn get(&self, id: i64) -> Option<&(String, bool)> {
        self.visits.get(&id)
    }

    pub(super) fn push(&mut self, id: i64, site: &str, is_search: bool) {
        if self.visits.insert(id, (site.to_owned(), is_search)).is_none() {
            self.order.push_back(id);
        }
        if self.order.len() > OPENER_WINDOW
            && let Some(oldest) = self.order.pop_front()
        {
            self.visits.remove(&oldest);
        }
    }
}

/// Unrelated visits in a row that end a search trail (`deterministic-intelligence.md` §5.4).
const UNRELATED_VISITS_ENDING_A_TRAIL: u32 = 3;

/// The longest common path is four sites, so a segment keeps the three sites before its own.
const PATH_PREFIX_SITES: usize = 3;
/// Sites a segment holds of its own before the next one starts: bounds a row, and the work of
/// re-saving the open segment after every batch, when one session runs for days (a visit every
/// few minutes never ends a session).
const PATH_SEGMENT_SITES: usize = 256;

/// One stretch of a session's sites, in order, each with the time of its last visit in that run
/// of visits. Bounded ranges count common paths from these (`path_flows::get_path_flows`): a path
/// belongs to the segment holding its last site, so a segment starts with the last
/// `PATH_PREFIX_SITES` sites of the one before it (`prefix_len` of them), and every path is in
/// exactly one segment. A segment's own sites all start on one local day (`date_key`), which is
/// what a date range selects.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct PathSegment {
    pub(super) segment: i64,
    pub(super) date_key: String,
    pub(super) prefix_len: usize,
    pub(super) sites: Vec<(String, i64)>,
}

impl PathSegment {
    /// Adds one visit. A visit to the site already last in the path only moves its time; a new
    /// site on another day than the segment's own, or past the size bound, closes this segment
    /// and returns it to be saved.
    pub(super) fn push(&mut self, domain: &str, visit_time_ms: i64) -> Option<PathSegment> {
        if let Some((last, time)) = self.sites.last_mut()
            && last == domain
        {
            *time = visit_time_ms;
            return None;
        }
        let day = local_date_key(visit_time_ms);
        let own = self.sites.len() - self.prefix_len;
        let closed = (own > 0 && (own >= PATH_SEGMENT_SITES || day != self.date_key)).then(|| {
            let prefix = self.sites[self.sites.len().saturating_sub(PATH_PREFIX_SITES)..].to_vec();
            std::mem::replace(
                self,
                PathSegment {
                    segment: self.segment + 1,
                    date_key: day.clone(),
                    prefix_len: prefix.len(),
                    sites: prefix,
                },
            )
        });
        if self.sites.len() == self.prefix_len {
            self.date_key = day;
        }
        self.sites.push((domain.to_owned(), visit_time_ms));
        closed
    }

    /// Whether the segment has sites of its own to save.
    pub(super) fn has_own_sites(&self) -> bool {
        self.sites.len() > self.prefix_len
    }
}

/// Tracks one in-progress browsing session while visits stream through the
/// deterministic grouping rules.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct SessionBuildState {
    pub(super) record: SessionRecord,
    #[serde(skip)]
    pub(super) domain_counts: HashMap<String, usize>,
    first_search_query: Option<String>,
    navigation_chain_depth: i64,
    new_domain_count: i64,
    /// The open path segment; earlier ones are already saved.
    #[serde(default)]
    pub(super) path: PathSegment,
}

impl SessionBuildState {
    /// Starts a new deterministic session anchored at the first visit that
    /// crossed the current session boundary.
    pub(super) fn new(visit: &StructuralVisitRecord) -> Self {
        Self {
            record: SessionRecord {
                session_id: format!("session:{}:{}", visit.profile_id, visit.visit_id),
                profile_id: visit.profile_id.clone(),
                first_visit_ms: visit.visit_time_ms,
                last_visit_ms: visit.visit_time_ms,
                visit_count: 0,
                search_count: 0,
                domain_count: 0,
                is_deep_dive: false,
                auto_title: None,
            },
            domain_counts: HashMap::new(),
            first_search_query: None,
            navigation_chain_depth: 0,
            new_domain_count: 0,
            path: PathSegment::default(),
        }
    }

    /// Incorporates one visit into the current session summary while keeping
    /// only the counters needed by the deep-dive heuristic.
    pub(super) fn push_visit(&mut self, visit: &StructuralVisitRecord) {
        self.record.last_visit_ms = visit.visit_time_ms;
        self.record.visit_count += 1;
        self.record.search_count += i64::from(visit.is_search_event);
        *self.domain_counts.entry(visit.registrable_domain.clone()).or_default() += 1;
        if self.first_search_query.is_none() {
            self.first_search_query = visit.search_query.clone();
        }
        self.navigation_chain_depth +=
            i64::from(matches!(Opener::of(visit.from_visit), Opener::Visit(_)));
        self.new_domain_count += i64::from(visit.is_new_domain);
    }

    /// Finalizes the session summary and stamps the deterministic deep-dive
    /// heuristic once no more visits belong to this session.
    pub(super) fn finish(mut self) -> SessionRecord {
        self.record.domain_count = self.domain_counts.len() as i64;
        self.record.auto_title = build_session_title_from_summary(
            &self.domain_counts,
            self.first_search_query.as_deref(),
        );
        self.record.is_deep_dive = self.navigation_chain_depth >= 4
            && self.record.domain_count >= 5
            && self.record.visit_count >= 8
            && self.new_domain_count >= 1;
        self.record
    }
}

/// Tracks one in-progress search trail while streamed visits continue to land
/// inside the same session-local search journey.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct TrailBuildState {
    pub(super) record: TrailRecord,
    next_ordinal: i64,
    /// A rewording was just added, so the next page opened is the new landing. Saved with the
    /// rest of the state between rebuild batches; states saved before it existed read as false.
    #[serde(default)]
    lands_next: bool,
    /// What a page shares with the trail when it belongs to it (`relates`): the trail's visits
    /// by the browser's ids (`from_visit` names those), its pages' sites and its searches' words.
    #[serde(default)]
    source_visit_ids: HashSet<i64>,
    #[serde(default)]
    sites: HashSet<String>,
    #[serde(default)]
    words: HashSet<String>,
    /// Visits in a row since the last one that belonged to the trail.
    #[serde(default)]
    unrelated: u32,
}

impl TrailBuildState {
    /// Starts a fresh trail at a search event that seeded a new navigation
    /// chain.
    pub(super) fn new(visit: &StructuralVisitRecord, session_id: &str) -> Self {
        let query = visit.search_query.clone().unwrap_or_else(|| "search".to_string());
        let trail_id = format!("trail:{}:{}", visit.profile_id, visit.visit_id);
        let search_event_member = TrailMemberRecord {
            trail_id: trail_id.clone(),
            profile_id: visit.profile_id.clone(),
            visit_id: visit.visit_id,
            ordinal: 0,
            role: "search_event".to_string(),
        };
        Self {
            record: TrailRecord {
                trail_id: trail_id.clone(),
                profile_id: visit.profile_id.clone(),
                session_id: session_id.to_string(),
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
                members: vec![search_event_member],
            },
            next_ordinal: 1,
            lands_next: false,
            source_visit_ids: HashSet::from([visit.source_visit_id]),
            sites: HashSet::new(),
            words: query_token_set(visit.search_query.as_deref().unwrap_or_default()),
            unrelated: 0,
        }
    }

    /// Whether a page belongs to the search that started this trail (`deterministic-intelligence.md`
    /// §5.4): it was opened from one of the trail's visits, it is on a site the trail already
    /// went to, or its title shares a word with the trail's searches. Other pages of the same
    /// session are not part of the search, however soon they follow it.
    pub(super) fn relates(&self, visit: &StructuralVisitRecord) -> bool {
        visit.from_visit.is_some_and(|from| self.source_visit_ids.contains(&from))
            || (!visit.registrable_domain.is_empty()
                && self.sites.contains(&visit.registrable_domain))
            || visit.title.as_deref().is_some_and(|title| {
                query_token_set(title).iter().any(|word| self.words.contains(word))
            })
    }

    /// Counts a visit that does not belong to the trail; true once enough came in a row that
    /// the search is over.
    pub(super) fn pass_unrelated(&mut self) -> bool {
        self.unrelated += 1;
        self.unrelated >= UNRELATED_VISITS_ENDING_A_TRAIL
    }

    /// Builds the synthetic member row for the search event that seeded this
    /// trail.
    pub(super) fn search_event_member(&self, visit_id: i64) -> TrailMemberRecord {
        self.record
            .members
            .iter()
            .find(|member| member.visit_id == visit_id && member.role == "search_event")
            .cloned()
            .unwrap_or_else(|| TrailMemberRecord {
                trail_id: self.record.trail_id.clone(),
                profile_id: self.record.profile_id.clone(),
                visit_id,
                ordinal: 0,
                role: "search_event".to_string(),
            })
    }

    /// Appends one non-search visit to the current trail and records whether it
    /// served as the landing page or a later click.
    pub(super) fn append_visit(&mut self, visit: &StructuralVisitRecord) -> TrailMemberRecord {
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        self.unrelated = 0;
        self.source_visit_ids.insert(visit.source_visit_id);
        if !visit.registrable_domain.is_empty() {
            self.sites.insert(visit.registrable_domain.clone());
        }
        let lands = self.record.landing_url.is_none() || self.lands_next;
        let role = if lands { "landing" } else { "click" };
        self.record.visit_count += 1;
        self.record.last_visit_ms = visit.visit_time_ms;
        self.record.max_depth = self.record.max_depth.max(ordinal);
        if lands {
            self.record.landing_url = Some(visit.url.clone());
            self.record.landing_domain = Some(visit.registrable_domain.clone());
            self.lands_next = false;
        }
        let member = TrailMemberRecord {
            trail_id: self.record.trail_id.clone(),
            profile_id: self.record.profile_id.clone(),
            visit_id: visit.visit_id,
            ordinal,
            role: role.to_string(),
        };
        self.record.members.push(member.clone());
        member
    }

    /// Whether `search` rewords this trail's latest query, on the same engine: the two share at
    /// least half their words, as a query family requires ("nikon z f review", "nikon z f vs
    /// z6iii"), or one's words are all in the other ("tokio select", "tokio select macro").
    /// Repeating the query counts too, as the same search. Session and time gap are the caller's
    /// to check.
    pub(super) fn is_rewording(&self, search: &StructuralVisitRecord) -> bool {
        if search.search_engine.as_deref().unwrap_or("unknown") != self.record.search_engine {
            return false;
        }
        let latest = query_token_set(self.record.queries.last().map_or("", String::as_str));
        let next = query_token_set(search.search_query.as_deref().unwrap_or_default());
        !latest.is_empty()
            && !next.is_empty()
            && (latest.is_subset(&next)
                || next.is_subset(&latest)
                || jaccard(&latest, &next) >= 0.5)
    }

    /// Adds a search that rewords this trail (see `is_rewording`). A new wording counts as a
    /// reformulation. The next page opened becomes the trail's landing, so the trail ends where
    /// the last search led; if none is opened it keeps the landing it had.
    pub(super) fn append_rewording(&mut self, search: &StructuralVisitRecord) -> TrailMemberRecord {
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        self.unrelated = 0;
        self.source_visit_ids.insert(search.source_visit_id);
        self.words.extend(query_token_set(search.search_query.as_deref().unwrap_or_default()));
        let query = search.search_query.clone().unwrap_or_else(|| "search".to_string());
        if self
            .record
            .queries
            .last()
            .is_none_or(|last| normalize_query(last) != normalize_query(&query))
        {
            self.record.queries.push(query);
            self.record.reformulation_count += 1;
        }
        self.record.visit_count += 1;
        self.record.last_visit_ms = search.visit_time_ms;
        self.lands_next = true;
        TrailMemberRecord {
            trail_id: self.record.trail_id.clone(),
            profile_id: self.record.profile_id.clone(),
            visit_id: search.visit_id,
            ordinal,
            role: "search_event".to_string(),
        }
    }

    /// Finalizes the trail once the session-local search journey ends.
    pub(super) fn finish(self) -> TrailRecord {
        self.record
    }
}

/// Produces a stable human-readable title for a deterministic session from the
/// dominant domain and the first search query, when either exists.
pub(super) fn build_session_title_from_summary(
    domain_counts: &HashMap<String, usize>,
    first_search_query: Option<&str>,
) -> Option<String> {
    let top_domain = domain_counts
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(domain, _)| display_name_for_domain(domain).unwrap_or_else(|| domain.clone()));
    match (top_domain, first_search_query) {
        (Some(domain), Some(query)) => Some(format!("{domain} · {query}")),
        (Some(domain), None) => Some(domain),
        (None, Some(query)) => Some(query.to_string()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn structural_visit(
        visit_id: i64,
        url: &str,
        domain: &str,
        query: Option<&str>,
        is_search_event: bool,
    ) -> StructuralVisitRecord {
        StructuralVisitRecord {
            visit_id,
            source_visit_id: visit_id,
            profile_id: "chrome:Default".to_string(),
            url: url.to_string(),
            title: None,
            visit_time_ms: 1_762_000_000_000 + visit_id,
            from_visit: None,
            registrable_domain: domain.to_string(),
            search_engine: is_search_event.then(|| "google".to_string()),
            search_query: query.map(str::to_string),
            is_new_domain: visit_id == 1,
            is_search_event,
        }
    }

    #[test]
    fn trail_state_returns_persisted_search_member_and_defensive_fallback() {
        let search_visit = structural_visit(
            1,
            "https://www.google.com/search?q=docs",
            "google.com",
            Some("docs"),
            true,
        );
        let mut trail = TrailBuildState::new(&search_visit, "session:chrome:Default:1");

        let seeded_member = trail.search_event_member(search_visit.visit_id);
        assert_eq!(seeded_member.role, "search_event");
        assert_eq!(seeded_member.ordinal, 0);
        assert_eq!(seeded_member.trail_id, trail.record.trail_id);

        let fallback_member = trail.search_event_member(99);
        assert_eq!(fallback_member.visit_id, 99);
        assert_eq!(fallback_member.role, "search_event");
        assert_eq!(fallback_member.ordinal, 0);

        let landing_visit =
            structural_visit(2, "https://example.com/docs", "example.com", None, false);
        let click_visit =
            structural_visit(3, "https://example.com/docs/api", "example.com", None, false);
        assert_eq!(trail.append_visit(&landing_visit).role, "landing");
        assert_eq!(trail.append_visit(&click_visit).role, "click");

        let record = trail.finish();
        assert_eq!(record.visit_count, 3);
        assert_eq!(record.landing_domain.as_deref(), Some("example.com"));
        assert_eq!(record.members.len(), 3);
    }

    #[test]
    fn session_title_summary_covers_empty_and_query_only_inputs() {
        let mut domain_counts = HashMap::new();
        domain_counts.insert("example.com".to_string(), 2);
        domain_counts.insert("docs.example.com".to_string(), 1);

        assert_eq!(
            build_session_title_from_summary(&domain_counts, Some("rust coverage")),
            Some("example.com · rust coverage".to_string())
        );
        assert_eq!(
            build_session_title_from_summary(&HashMap::new(), Some("rust coverage")),
            Some("rust coverage".to_string())
        );
        assert_eq!(build_session_title_from_summary(&HashMap::new(), None), None);
    }
}
