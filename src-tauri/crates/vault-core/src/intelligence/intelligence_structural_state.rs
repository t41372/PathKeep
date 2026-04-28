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
//! - Building refind, habit, or source-effectiveness aggregates.
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
};
use std::collections::HashMap;

/// Tracks one in-progress browsing session while visits stream through the
/// deterministic grouping rules.
#[derive(Debug)]
pub(super) struct SessionBuildState {
    pub(super) record: SessionRecord,
    domain_counts: HashMap<String, usize>,
    first_search_query: Option<String>,
    navigation_chain_depth: i64,
    new_domain_count: i64,
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
        self.navigation_chain_depth += i64::from(visit.from_visit.is_some());
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
#[derive(Debug)]
pub(super) struct TrailBuildState {
    pub(super) record: TrailRecord,
    next_ordinal: i64,
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
        }
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
        let role = if self.record.landing_url.is_none() { "landing" } else { "click" };
        self.record.visit_count += 1;
        self.record.last_visit_ms = visit.visit_time_ms;
        self.record.max_depth = self.record.max_depth.max(ordinal);
        self.record.landing_url.get_or_insert_with(|| visit.url.clone());
        self.record.landing_domain.get_or_insert_with(|| visit.registrable_domain.clone());
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
            profile_id: "chrome:Default".to_string(),
            url: url.to_string(),
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
