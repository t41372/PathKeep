//! Structural aggregate builders for Core Intelligence.
//!
//! ## Responsibilities
//! - Build refind pages, path flows, and habit patterns from deterministic
//!   visit records.
//! - Build reopened-investigation rows from structural entities without pulling
//!   route-level explainability into the rebuild path.
//! - Provide the streamed aggregate-rebuild path used by structural staged
//!   rebuilds.
//!
//! ## Not responsible for
//! - Session/trail grouping or query-family similarity logic.
//! - Persisting structural rows into SQLite.
//! - Choosing fallback versus incremental rebuild scope.
//!
//! ## Dependencies
//! - Parent-module visit records, date helpers, and deterministic query
//!   normalization.
//! - Streamed derived-visit reads from `visit_derived_facts`.
//!
//! ## Performance notes
//! - The batched aggregate path consumes ordered visit batches and only keeps
//!   compact hash-map state for the current aggregates.
//! - Path-flow and habit builders deduplicate consecutive domains/days instead
//!   of retaining whole sessions in memory.

use super::{
    DerivedVisitBatchCursor, HabitPatternRecord, PathFlowRecord, QueryFamilyRecord,
    RefindPageRecord, ReopenedInvestigationRecord, SourceEffectivenessRecord, VisitRecord,
    load_profile_derived_visit_batch, local_date_key, local_datetime_from_millis, normalize_query,
};
use anyhow::Result;
use chrono::{NaiveDate, Utc};
use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};

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
    visit_ids: Vec<i64>,
}

/// Streams structural aggregate state across many derived-visit batches without
/// holding the full profile visit vector in memory.
#[derive(Debug, Default)]
struct StructuralAggregateAccumulator {
    profile_id: Option<String>,
    refind_pages: HashMap<String, RefindAccumulatorEntry>,
    flow_counts: HashMap<(String, String, i64), (i64, i64)>,
    current_session_id: Option<String>,
    current_profile_id: Option<String>,
    current_sequence: Vec<(String, i64)>,
    habit_days: HashMap<String, BTreeSet<NaiveDate>>,
    last_visit_ms: HashMap<String, i64>,
}

/// Builds refind pages from one already-materialized visit slice.
pub(super) fn build_refind_pages(visits: &[VisitRecord]) -> Vec<RefindPageRecord> {
    let mut grouped = HashMap::<String, Vec<&VisitRecord>>::new();
    for visit in visits.iter().filter(|visit| !visit.is_search_event) {
        grouped.entry(visit.canonical_url.clone()).or_default().push(visit);
    }
    grouped
        .into_iter()
        .filter_map(|(canonical_url, members)| {
            let first = members.first()?;
            let distinct_days = members
                .iter()
                .map(|visit| local_date_key(visit.visit_time_ms))
                .collect::<HashSet<_>>();
            let trail_ids = members
                .iter()
                .filter_map(|visit| visit.trail_id.clone())
                .collect::<HashSet<_>>();
            let search_arrival_count =
                members.iter().filter(|visit| visit.trail_id.is_some()).count() as i64;
            let typed_revisit_count = members
                .iter()
                .filter(|visit| visit.from_visit.is_none() && !visit.is_search_event)
                .count() as i64;
            let cross_day_count = distinct_days.len() as i64;
            let trail_count = trail_ids.len() as i64;
            let score = (cross_day_count as f32 * 2.0)
                + (trail_count as f32 * 1.5)
                + search_arrival_count as f32
                + (typed_revisit_count as f32 * 1.2);
            if cross_day_count < 2 && trail_count < 2 && typed_revisit_count < 2 {
                return None;
            }
            let visit_ids = members.iter().map(|visit| visit.visit_id).collect::<Vec<_>>();
            let evidence_json = json!({
                "factors": [
                    { "signal": "cross_day_count", "rawValue": cross_day_count, "weight": 2.0, "contribution": cross_day_count as f32 * 2.0 },
                    { "signal": "trail_count", "rawValue": trail_count, "weight": 1.5, "contribution": trail_count as f32 * 1.5 },
                    { "signal": "search_arrival_count", "rawValue": search_arrival_count, "weight": 1.0, "contribution": search_arrival_count as f32 },
                    { "signal": "typed_revisit_count", "rawValue": typed_revisit_count, "weight": 1.2, "contribution": typed_revisit_count as f32 * 1.2 }
                ],
                "visitIds": visit_ids
            })
            .to_string();
            Some(RefindPageRecord {
                profile_id: first.profile_id.clone(),
                canonical_url,
                url: first.url.clone(),
                title: first.title.clone(),
                registrable_domain: first.registrable_domain.clone(),
                cross_day_count,
                trail_count,
                search_arrival_count,
                typed_revisit_count,
                refind_score: score,
                evidence_json,
                first_seen_ms: members
                    .iter()
                    .map(|visit| visit.visit_time_ms)
                    .min()
                    .unwrap_or(first.visit_time_ms),
                last_seen_ms: members
                    .iter()
                    .map(|visit| visit.visit_time_ms)
                    .max()
                    .unwrap_or(first.visit_time_ms),
            })
        })
        .collect()
}

/// Builds source-effectiveness rows directly from in-memory trails and refind
/// pages during legacy scoped rebuilds.
pub(super) fn build_source_effectiveness(
    trails: &[super::TrailRecord],
    refind_pages: &[RefindPageRecord],
) -> Vec<SourceEffectivenessRecord> {
    let mut landing_counts = HashMap::<String, i64>::new();
    let mut trail_counts = HashMap::<String, HashSet<String>>::new();
    let mut first_seen = HashMap::<String, i64>::new();
    let mut last_seen = HashMap::<String, i64>::new();
    let profile_id = trails
        .first()
        .map(|trail| trail.profile_id.clone())
        .or_else(|| refind_pages.first().map(|page| page.profile_id.clone()))
        .unwrap_or_default();

    for trail in trails {
        if let Some(domain) = &trail.landing_domain {
            *landing_counts.entry(domain.clone()).or_default() += 1;
            trail_counts.entry(domain.clone()).or_default().insert(trail.trail_id.clone());
            first_seen
                .entry(domain.clone())
                .and_modify(|value| *value = (*value).min(trail.first_visit_ms))
                .or_insert(trail.first_visit_ms);
            last_seen
                .entry(domain.clone())
                .and_modify(|value| *value = (*value).max(trail.last_visit_ms))
                .or_insert(trail.last_visit_ms);
        }
    }

    let reference_counts =
        refind_pages.iter().fold(HashMap::<String, i64>::new(), |mut acc, page| {
            *acc.entry(page.registrable_domain.clone()).or_default() += 1;
            acc
        });
    let domains =
        landing_counts.keys().chain(reference_counts.keys()).cloned().collect::<BTreeSet<_>>();

    domains
        .into_iter()
        .map(|domain| {
            let stable_landing_count = *landing_counts.get(&domain).unwrap_or(&0);
            let reference_count = *reference_counts.get(&domain).unwrap_or(&0);
            let trail_count =
                trail_counts.get(&domain).map(|value| value.len()).unwrap_or(0) as i64;
            let source_role = if reference_count >= stable_landing_count && reference_count > 0 {
                "reference"
            } else {
                "landing"
            };
            let effectiveness_score = (stable_landing_count as f32 * 2.0)
                + (reference_count as f32 * 1.5)
                + (trail_count as f32 * 0.5);
            let evidence_json = json!({
                "stableLandingCount": stable_landing_count,
                "referenceCount": reference_count,
                "trailCount": trail_count
            })
            .to_string();
            SourceEffectivenessRecord {
                profile_id: profile_id.clone(),
                registrable_domain: domain.clone(),
                source_role: source_role.to_string(),
                trail_count,
                stable_landing_count,
                effectiveness_score,
                evidence_json,
                first_seen_ms: *first_seen.get(&domain).unwrap_or(&0),
                last_seen_ms: *last_seen.get(&domain).unwrap_or(&0),
            }
        })
        .collect()
}

/// Builds reopened-investigation rows from query families and refind pages.
pub(super) fn build_reopened_investigations(
    query_families: &[QueryFamilyRecord],
    refind_pages: &[RefindPageRecord],
) -> Vec<ReopenedInvestigationRecord> {
    let mut records = Vec::new();
    for family in query_families {
        let distinct_days =
            family.queries.iter().map(|query| normalize_query(query)).collect::<HashSet<_>>().len()
                as i64;
        if distinct_days > 1 || family.member_count > 1 {
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

/// Builds path-flow rows from an already-materialized visit slice.
pub(super) fn build_path_flows(visits: &[VisitRecord]) -> Vec<PathFlowRecord> {
    let mut flows = HashMap::<(String, String, i64), (i64, i64)>::new();
    let mut current_session = None::<String>;
    let mut current_profile = None::<String>;
    let mut current_sequence = Vec::<(String, i64)>::new();

    let mut flush_sequence = |profile_id: &str, sequence: &[(String, i64)]| {
        flush_path_flow_sequence(&mut flows, profile_id, sequence)
    };

    for visit in visits {
        let session_id = visit
            .session_id
            .clone()
            .unwrap_or_else(|| format!("sessionless:{}:{}", visit.profile_id, visit.visit_id));
        if current_session.as_deref() != Some(session_id.as_str()) {
            if let (Some(profile_id), false) =
                (current_profile.as_deref(), current_sequence.is_empty())
            {
                flush_sequence(profile_id, &current_sequence);
            }
            current_session = Some(session_id);
            current_profile = Some(visit.profile_id.clone());
            current_sequence.clear();
        }
        if current_sequence
            .last()
            .is_none_or(|(last_domain, _)| *last_domain != visit.registrable_domain)
        {
            current_sequence.push((visit.registrable_domain.clone(), visit.visit_time_ms));
        } else if let Some((_, last_seen_ms)) = current_sequence.last_mut() {
            *last_seen_ms = visit.visit_time_ms;
        }
    }
    if let (Some(profile_id), false) = (current_profile.as_deref(), current_sequence.is_empty()) {
        flush_sequence(profile_id, &current_sequence);
    }

    finish_path_flows(flows)
}

/// Builds habit-pattern rows from an already-materialized visit slice.
pub(super) fn build_habit_patterns(visits: &[VisitRecord]) -> Vec<HabitPatternRecord> {
    let mut by_domain = HashMap::<String, BTreeSet<NaiveDate>>::new();
    let mut last_visit = HashMap::<String, i64>::new();
    for visit in visits {
        by_domain
            .entry(visit.registrable_domain.clone())
            .or_default()
            .insert(local_datetime_from_millis(visit.visit_time_ms).date_naive());
        last_visit
            .entry(visit.registrable_domain.clone())
            .and_modify(|value| *value = (*value).max(visit.visit_time_ms))
            .or_insert(visit.visit_time_ms);
    }
    habit_records_for_domains(
        visits.first().map(|visit| visit.profile_id.clone()),
        by_domain,
        last_visit,
    )
}

/// Rebuilds refind/path-flow/habit rows by streaming already-derived visits in
/// visit order instead of materializing a second full profile visit vector.
pub(super) fn build_structural_profile_aggregates_from_batches(
    connection: &rusqlite::Connection,
    profile_id: &str,
) -> Result<(Vec<RefindPageRecord>, Vec<PathFlowRecord>, Vec<HabitPatternRecord>)> {
    let mut accumulator = StructuralAggregateAccumulator::default();
    let mut cursor = None;
    loop {
        let batch = load_profile_derived_visit_batch(
            connection,
            profile_id,
            cursor,
            super::STRUCTURAL_AGGREGATE_BATCH_SIZE,
        )?;
        if batch.is_empty() {
            break;
        }
        for visit in &batch {
            accumulator.add_visit(visit);
        }
        cursor = batch.last().map(|visit| DerivedVisitBatchCursor {
            visit_time_ms: visit.visit_time_ms,
            visit_id: visit.visit_id,
        });
    }
    Ok(accumulator.finish())
}

impl StructuralAggregateAccumulator {
    /// Adds one derived visit into every structural aggregate that still depends
    /// on visit-level evidence.
    fn add_visit(&mut self, visit: &VisitRecord) {
        self.profile_id.get_or_insert_with(|| visit.profile_id.clone());
        self.record_refind_page(visit);
        self.record_path_flow(visit);
        self.record_habit_day(visit);
    }

    /// Finalizes the streamed aggregate state into persisted structural rows.
    fn finish(mut self) -> (Vec<RefindPageRecord>, Vec<PathFlowRecord>, Vec<HabitPatternRecord>) {
        if let (Some(profile_id), false) =
            (self.current_profile_id.as_deref(), self.current_sequence.is_empty())
        {
            flush_path_flow_sequence(&mut self.flow_counts, profile_id, &self.current_sequence);
        }
        let StructuralAggregateAccumulator {
            profile_id,
            refind_pages,
            flow_counts,
            current_session_id: _,
            current_profile_id: _,
            current_sequence: _,
            habit_days,
            last_visit_ms,
        } = self;
        (
            finish_refind_pages(refind_pages),
            finish_path_flows(flow_counts),
            habit_records_for_domains(profile_id, habit_days, last_visit_ms),
        )
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
        entry.distinct_days.insert(local_date_key(visit.visit_time_ms));
        if let Some(trail_id) = &visit.trail_id {
            entry.trail_ids.insert(trail_id.clone());
            entry.search_arrival_count += 1;
        }
        if visit.from_visit.is_none() {
            entry.typed_revisit_count += 1;
        }
        entry.first_seen_ms = entry.first_seen_ms.min(visit.visit_time_ms);
        entry.last_seen_ms = entry.last_seen_ms.max(visit.visit_time_ms);
        entry.visit_ids.push(visit.visit_id);
    }

    /// Tracks session-local domain sequences for path-flow rebuilds.
    fn record_path_flow(&mut self, visit: &VisitRecord) {
        let session_id = visit
            .session_id
            .clone()
            .unwrap_or_else(|| format!("sessionless:{}:{}", visit.profile_id, visit.visit_id));
        if self.current_session_id.as_deref() != Some(session_id.as_str()) {
            if let (Some(profile_id), false) =
                (self.current_profile_id.as_deref(), self.current_sequence.is_empty())
            {
                flush_path_flow_sequence(&mut self.flow_counts, profile_id, &self.current_sequence);
            }
            self.current_session_id = Some(session_id);
            self.current_profile_id = Some(visit.profile_id.clone());
            self.current_sequence.clear();
        }
        if self
            .current_sequence
            .last()
            .is_none_or(|(last_domain, _)| *last_domain != visit.registrable_domain)
        {
            self.current_sequence.push((visit.registrable_domain.clone(), visit.visit_time_ms));
        } else if let Some((_, last_seen_ms)) = self.current_sequence.last_mut() {
            *last_seen_ms = visit.visit_time_ms;
        }
    }

    /// Records the unique local day touched by one domain visit for habit
    /// detection.
    fn record_habit_day(&mut self, visit: &VisitRecord) {
        self.habit_days
            .entry(visit.registrable_domain.clone())
            .or_default()
            .insert(local_datetime_from_millis(visit.visit_time_ms).date_naive());
        self.last_visit_ms
            .entry(visit.registrable_domain.clone())
            .and_modify(|value| *value = (*value).max(visit.visit_time_ms))
            .or_insert(visit.visit_time_ms);
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
            let score = (cross_day_count as f32 * 2.0)
                + (trail_count as f32 * 1.5)
                + entry.search_arrival_count as f32
                + (entry.typed_revisit_count as f32 * 1.2);
            if cross_day_count < 2 && trail_count < 2 && entry.typed_revisit_count < 2 {
                return None;
            }
            let evidence_json = json!({
                "factors": [
                    { "signal": "cross_day_count", "rawValue": cross_day_count, "weight": 2.0, "contribution": cross_day_count as f32 * 2.0 },
                    { "signal": "trail_count", "rawValue": trail_count, "weight": 1.5, "contribution": trail_count as f32 * 1.5 },
                    { "signal": "search_arrival_count", "rawValue": entry.search_arrival_count, "weight": 1.0, "contribution": entry.search_arrival_count as f32 },
                    { "signal": "typed_revisit_count", "rawValue": entry.typed_revisit_count, "weight": 1.2, "contribution": entry.typed_revisit_count as f32 * 1.2 }
                ],
                "visitIds": entry.visit_ids
            })
            .to_string();
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
            })
        })
        .collect()
}

/// Finalizes the compact path-flow counter map into persisted rows.
fn finish_path_flows(
    flow_counts: HashMap<(String, String, i64), (i64, i64)>,
) -> Vec<PathFlowRecord> {
    flow_counts
        .into_iter()
        .map(|((profile_id, flow_pattern, step_count), (occurrence_count, last_seen_ms))| {
            PathFlowRecord { profile_id, flow_pattern, step_count, occurrence_count, last_seen_ms }
        })
        .collect()
}

/// Finalizes domain-day accumulators into persisted habit-pattern rows.
fn habit_records_for_domains(
    profile_id: Option<String>,
    habit_days: HashMap<String, BTreeSet<NaiveDate>>,
    last_visit_ms: HashMap<String, i64>,
) -> Vec<HabitPatternRecord> {
    let Some(profile_id) = profile_id else {
        return Vec::new();
    };
    habit_days
        .into_iter()
        .filter_map(|(domain, days)| {
            if days.len() < 5 {
                return None;
            }
            let parsed_days = days.into_iter().collect::<Vec<_>>();
            if (*parsed_days.last()? - *parsed_days.first()?).num_days() < 14 {
                return None;
            }
            let intervals = parsed_days
                .windows(2)
                .map(|window| (window[1] - window[0]).num_days() as f32)
                .collect::<Vec<_>>();
            let mean = intervals.iter().sum::<f32>() / intervals.len() as f32;
            let variance = intervals.iter().map(|value| (*value - mean).powi(2)).sum::<f32>()
                / intervals.len() as f32;
            let std_dev = variance.sqrt();
            let cv = if mean == 0.0 { 0.0 } else { std_dev / mean };
            let habit_type = if mean < 2.0 && cv < 0.5 {
                Some("daily_habit")
            } else if (5.0..=10.0).contains(&mean) && cv < 0.6 {
                Some("weekly_habit")
            } else if mean > 10.0 && cv < 0.8 {
                Some("periodic_reference")
            } else {
                None
            }?;
            let last_visited_ms = *last_visit_ms.get(&domain).unwrap_or(&0);
            let days_since_last =
                ((Utc::now().timestamp_millis() - last_visited_ms) as f32 / 86_400_000.0).max(0.0);
            Some(HabitPatternRecord {
                profile_id: profile_id.clone(),
                registrable_domain: domain,
                habit_type: habit_type.to_string(),
                mean_interval_days: mean,
                cv,
                visit_count: parsed_days.len() as i64,
                last_visited_ms,
                is_interrupted: days_since_last > mean * 2.0,
            })
        })
        .collect()
}

/// Emits all path-flow n-grams for one completed session-local domain sequence.
fn flush_path_flow_sequence(
    flows: &mut HashMap<(String, String, i64), (i64, i64)>,
    profile_id: &str,
    sequence: &[(String, i64)],
) {
    for step_count in [2_usize, 3_usize, 4_usize] {
        if sequence.len() < step_count {
            continue;
        }
        for window in sequence.windows(step_count) {
            let flow_pattern =
                window.iter().map(|(domain, _)| domain.as_str()).collect::<Vec<_>>().join(" → ");
            let last_seen_ms =
                window.iter().map(|(_, visit_time_ms)| *visit_time_ms).max().unwrap_or(0);
            let key = (profile_id.to_string(), flow_pattern, step_count as i64);
            let entry = flows.entry(key).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.max(last_seen_ms);
        }
    }
}
