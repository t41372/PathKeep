//! Failure modes: repeated pages inflate scores.
//! Structural aggregate builders for Core Intelligence.
//!
//! ## Responsibilities
//! - Build refind pages from deterministic visit records.
//! - Build reopened-investigation rows from structural entities without pulling
//!   route-level explainability into the rebuild path.
//! - Provide the streamed aggregate-rebuild path used by structural staged
//!   rebuilds.
//!
//! ## Not responsible for
//! - Session/trail grouping or query-family similarity logic.
//! - Paths between sites: `path_flows` counts them from the sessions' stored segments.
//! - Habits: the reads work them out from the site-day rollups (`phase_three`).
//! - Persisting structural rows into SQLite.
//! - Choosing fallback versus incremental rebuild scope.
//!
//! ## Dependencies
//! - Parent-module visit records, date helpers, and deterministic query
//!   normalization.
//!
//! ## Performance notes
//! - The streamed aggregate path consumes visits in order and only keeps
//!   compact hash-map state for the current aggregates.
//! - Refind evidence keeps only a bounded, most-recent sample of visit ids plus
//!   a total counter, so a page revisited daily for decades cannot inflate the
//!   accumulator or the persisted evidence JSON beyond a fixed size.

use super::{
    InsightsZone, QueryFamilyRecord, RefindPageRecord, ReopenedInvestigationRecord, VisitRecord,
};
use crate::visit_taxonomy::Opener;
use serde_json::json;
use std::collections::{HashMap, HashSet};

/// Upper bound on how many visit ids are embedded in one refind-page evidence
/// payload.
///
/// The explainability panel only ever renders the first 20 ids as drill-down
/// chips and otherwise relies on a total count, so a bounded recent sample is
/// all the UI can use. Capping here keeps a popular page (visited every day for
/// decades) from holding hundreds of thousands of `i64` ids in the streaming
/// accumulator and from emitting multi-megabyte evidence blobs per row on a
/// 14.4M-record archive. The true visit total is preserved separately as
/// `totalVisitCount`.
const REFIND_EVIDENCE_VISIT_SAMPLE: usize = 50;

/// Bounded, recency-ordered reservoir of visit ids for one refind page.
///
/// ## Why this exists
/// Refind evidence must stay O(1) in size regardless of how many times a page
/// was visited. This keeps at most `REFIND_EVIDENCE_VISIT_SAMPLE` of the most
/// recent `(visit_time_ms, visit_id)` pairs while counting every visit, so both
/// the streaming accumulator and the in-memory builder can emit an identical,
/// deterministic sample plus an exact total.
#[derive(Debug, Clone, Default)]
struct RecentVisitSample {
    /// Most-recent pairs sorted descending by `(visit_time_ms, visit_id)`; never
    /// longer than `REFIND_EVIDENCE_VISIT_SAMPLE`.
    recent: Vec<(i64, i64)>,
    /// Exact number of visits observed, including those evicted from the sample.
    total: i64,
}

impl RecentVisitSample {
    /// Records one visit, keeping the sample bounded to the most recent ids.
    ///
    /// Sorting is descending by visit time and then visit id so ties resolve
    /// deterministically and the two refind builders agree on the same sample.
    fn observe(&mut self, visit_time_ms: i64, visit_id: i64) {
        self.total += 1;
        let candidate = (visit_time_ms, visit_id);
        if self.recent.len() == REFIND_EVIDENCE_VISIT_SAMPLE {
            // Fast path once warmed: anything not newer than the current oldest
            // kept pair can never enter the sample, so skip the insert entirely.
            let oldest = *self.recent.last().expect("sample is full");
            if candidate <= oldest {
                return;
            }
        }
        let position = self.recent.partition_point(|kept| *kept > candidate);
        self.recent.insert(position, candidate);
        self.recent.truncate(REFIND_EVIDENCE_VISIT_SAMPLE);
    }

    /// Returns the sampled visit ids most-recent first.
    fn visit_ids(&self) -> Vec<i64> {
        self.recent.iter().map(|(_, visit_id)| *visit_id).collect()
    }
}

/// Builds the shared refind evidence JSON: the weighted score factors plus a
/// bounded recent visit-id sample and the exact total visit count.
fn refind_evidence_json(
    cross_day_count: i64,
    trail_count: i64,
    search_arrival_count: i64,
    typed_revisit_count: i64,
    sample: &RecentVisitSample,
) -> String {
    evidence_with_visits(
        cross_day_count,
        trail_count,
        search_arrival_count,
        typed_revisit_count,
        &sample.visit_ids(),
        sample.total,
    )
}

fn evidence_with_visits(
    cross_day_count: i64,
    trail_count: i64,
    search_arrival_count: i64,
    typed_revisit_count: i64,
    visit_ids: &[i64],
    total: i64,
) -> String {
    let factors=refind_signals(cross_day_count,trail_count,search_arrival_count,typed_revisit_count).into_iter().map(|(signal,raw,weight)|json!({"signal":signal,"rawValue":raw,"weight":weight,"contribution":raw as f32*weight})).collect::<Vec<_>>();
    json!({
        "factors": factors,
        "visitIds": visit_ids,
        "totalVisitCount": total
    })
    .to_string()
}

fn refind_score(days: i64, trails: i64, searches: i64, typed: i64) -> f32 {
    (days as f32 * 2.0) + (trails as f32 * 1.5) + searches as f32 + (typed as f32 * 1.2)
}

/// What a page's new visits add to its stored refind row after an append, where every new visit
/// is later than every stored one.
#[derive(Debug, Default)]
pub(super) struct RefindAddition {
    /// The local days of the new visits.
    pub days: std::collections::BTreeSet<String>,
    /// Trails among the new visits that none of the page's earlier visits was in.
    pub new_trails: i64,
    pub search_arrivals: i64,
    pub typed_revisits: i64,
    /// `(visit_time_ms, visit_id)` of each new visit.
    pub visits: Vec<(i64, i64)>,
}

/// A stored refind row with an append's new visits added: what building the row from all the
/// page's visits gives (`append_aggregates.rs` checks that). A new day counts unless it is the
/// row's last day, since no stored visit is later; the new visits are the newest, so they lead the
/// visit sample. `stored_visit_ids` and `stored_total` are the row's evidence sample and count.
pub(super) fn add_to_refind_page(
    zone: InsightsZone,
    stored: RefindPageRecord,
    stored_visit_ids: &[i64],
    stored_total: i64,
    mut addition: RefindAddition,
) -> RefindPageRecord {
    let last_day = zone.date_key(stored.last_seen_ms);
    // Every new visit is later than every stored one, so only the stored last day can repeat.
    let new_days = std::mem::take(&mut addition.days)
        .into_iter()
        .filter(|day| *day != last_day)
        .collect::<Vec<_>>();
    let cross_day_count = stored.cross_day_count + new_days.len() as i64;
    let trail_count = stored.trail_count + addition.new_trails;
    let search_arrival_count = stored.search_arrival_count + addition.search_arrivals;
    let typed_revisit_count = stored.typed_revisit_count + addition.typed_revisits;
    addition.visits.sort_unstable_by(|a, b| b.cmp(a));
    let last_seen_ms = addition
        .visits
        .first()
        .map_or(stored.last_seen_ms, |(time, _)| (*time).max(stored.last_seen_ms));
    let visit_ids = addition
        .visits
        .iter()
        .map(|(_, id)| *id)
        .chain(stored_visit_ids.iter().copied())
        .take(REFIND_EVIDENCE_VISIT_SAMPLE)
        .collect::<Vec<_>>();
    let total = stored_total + addition.visits.len() as i64;
    RefindPageRecord {
        refind_score: refind_score(
            cross_day_count,
            trail_count,
            search_arrival_count,
            typed_revisit_count,
        ),
        evidence_json: evidence_with_visits(
            cross_day_count,
            trail_count,
            search_arrival_count,
            typed_revisit_count,
            &visit_ids,
            total,
        ),
        cross_day_count,
        trail_count,
        search_arrival_count,
        typed_revisit_count,
        last_seen_ms,
        days: new_days,
        ..stored
    }
}

/// The scoring vocabulary is shared by rebuilds and explanations.
pub(super) fn refind_signals(
    days: i64,
    trails: i64,
    searches: i64,
    typed: i64,
) -> [(&'static str, i64, f32); 4] {
    [
        ("cross_day_count", days, 2.0),
        ("trail_count", trails, 1.5),
        ("search_arrival_count", searches, 1.0),
        ("typed_revisit_count", typed, 1.2),
    ]
}

/// Compact accumulator state for one in-progress refind-page aggregate.
#[derive(Debug, Clone, Default)]
struct RefindAccumulatorEntry {
    profile_id: String,
    canonical_url: String,
    url: String,
    title: Option<String>,
    registrable_domain: String,
    distinct_days: HashSet<String>,
    trail_ids: HashSet<String>,
    search_arrival_count: i64,
    typed_revisit_count: i64,
    first_seen_ms: i64,
    last_seen_ms: i64,
    visit_sample: RecentVisitSample,
}

/// Streams structural aggregate state across many derived-visit batches without
/// holding the full profile visit vector in memory.
#[derive(Debug)]
pub(super) struct StructuralAggregateAccumulator {
    /// The zone the days a page was opened on are in (the build's).
    zone: InsightsZone,
    refind_pages: HashMap<String, RefindAccumulatorEntry>,
}

/// Builds reopened-investigation rows from query families and refind pages: a search the user came
/// back to on another day, or a page they reopened on another day (§4.5 of the Insights design).
pub(super) fn build_reopened_investigations(
    query_families: &[QueryFamilyRecord],
    refind_pages: &[RefindPageRecord],
) -> Vec<ReopenedInvestigationRecord> {
    let mut records = Vec::new();
    for family in query_families {
        // Days, not wordings: the count used to be how many ways the search was phrased.
        let distinct_days = family.active_days;
        if distinct_days > 1 {
            records.push(ReopenedInvestigationRecord {
                investigation_id: format!("reopened:{}:{}", family.profile_id, family.family_id),
                profile_id: family.profile_id.clone(),
                anchor_type: "query_family".to_string(),
                anchor_id: family.family_id.clone(),
                anchor_label: family.anchor_query.clone(),
                occurrence_count: family.member_count,
                distinct_days,
                first_seen_ms: family.first_seen_ms,
                last_seen_ms: family.last_seen_ms,
                evidence_json: json!({ "queries": family.queries }).to_string(),
            });
        }
    }
    for page in refind_pages {
        if page.cross_day_count > 1 {
            records.push(ReopenedInvestigationRecord {
                investigation_id: format!("reopened:{}:{}", page.profile_id, page.canonical_url),
                profile_id: page.profile_id.clone(),
                anchor_type: "reference_page".to_string(),
                anchor_id: page.canonical_url.clone(),
                anchor_label: page.title.clone().unwrap_or_else(|| page.url.clone()),
                occurrence_count: page.cross_day_count,
                distinct_days: page.cross_day_count,
                first_seen_ms: page.first_seen_ms,
                last_seen_ms: page.last_seen_ms,
                evidence_json: page.evidence_json.clone(),
            });
        }
    }
    records
}

impl StructuralAggregateAccumulator {
    pub(super) fn new(zone: InsightsZone) -> Self {
        Self { zone, refind_pages: HashMap::new() }
    }

    /// Accepts indexed page order while preserving chronological URL/title provenance.
    pub(super) fn add_refind_visit(&mut self, visit: &VisitRecord, earliest: bool) {
        self.record_refind_page(visit);
        if earliest && let Some(entry) = self.refind_pages.get_mut(&visit.canonical_url) {
            entry.url = visit.url.clone();
            entry.title = visit.title.clone();
        }
    }

    /// Finalizes the streamed aggregate state into persisted structural rows.
    pub(super) fn finish(self) -> Vec<RefindPageRecord> {
        finish_refind_pages(self.refind_pages)
    }

    /// Tracks one visit inside the refind-page accumulator keyed by canonical
    /// URL.
    fn record_refind_page(&mut self, visit: &VisitRecord) {
        if visit.is_search_event {
            return;
        }
        let entry = self.refind_pages.entry(visit.canonical_url.clone()).or_insert_with(|| {
            RefindAccumulatorEntry {
                profile_id: visit.profile_id.clone(),
                canonical_url: visit.canonical_url.clone(),
                url: visit.url.clone(),
                title: visit.title.clone(),
                registrable_domain: visit.registrable_domain.clone(),
                first_seen_ms: visit.visit_time_ms,
                last_seen_ms: visit.visit_time_ms,
                ..RefindAccumulatorEntry::default()
            }
        });
        entry.distinct_days.insert(self.zone.date_key(visit.visit_time_ms));
        if let Some(trail_id) = &visit.trail_id {
            entry.trail_ids.insert(trail_id.clone());
            entry.search_arrival_count += 1;
        }
        if Opener::of(visit.from_visit) == Opener::Nothing {
            entry.typed_revisit_count += 1;
        }
        entry.first_seen_ms = entry.first_seen_ms.min(visit.visit_time_ms);
        entry.last_seen_ms = entry.last_seen_ms.max(visit.visit_time_ms);
        entry.visit_sample.observe(visit.visit_time_ms, visit.visit_id);
    }
}

/// Finalizes the compact refind accumulator into persisted refind rows.
fn finish_refind_pages(
    refind_pages: HashMap<String, RefindAccumulatorEntry>,
) -> Vec<RefindPageRecord> {
    refind_pages
        .into_values()
        .filter_map(|entry| {
            let cross_day_count = entry.distinct_days.len() as i64;
            let trail_count = entry.trail_ids.len() as i64;
            let score = refind_score(
                cross_day_count,
                trail_count,
                entry.search_arrival_count,
                entry.typed_revisit_count,
            );
            if cross_day_count < 2 && trail_count < 2 && entry.typed_revisit_count < 2 {
                return None;
            }
            let evidence_json = refind_evidence_json(
                cross_day_count,
                trail_count,
                entry.search_arrival_count,
                entry.typed_revisit_count,
                &entry.visit_sample,
            );
            Some(RefindPageRecord {
                profile_id: entry.profile_id,
                canonical_url: entry.canonical_url,
                url: entry.url,
                title: entry.title,
                registrable_domain: entry.registrable_domain,
                cross_day_count,
                trail_count,
                search_arrival_count: entry.search_arrival_count,
                typed_revisit_count: entry.typed_revisit_count,
                refind_score: score,
                evidence_json,
                first_seen_ms: entry.first_seen_ms,
                last_seen_ms: entry.last_seen_ms,
                days: sorted(entry.distinct_days),
            })
        })
        .collect()
}

fn sorted(days: HashSet<String>) -> Vec<String> {
    let mut days = days.into_iter().collect::<Vec<_>>();
    days.sort_unstable();
    days
}
