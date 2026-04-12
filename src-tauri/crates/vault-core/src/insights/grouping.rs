//! Query-group clustering helpers for deterministic insights.

use super::{
    InsightEvidenceItem, VisitRecord, canonical_visit_key, chrome_gap_hours, chrome_gap_minutes,
    classify_query_stage, evidence_from_visit, token_similarity,
};
use crate::{
    deterministic::{PageCategory, tokenize_text},
    models::{InsightQueryGroupSummary, InsightQueryLadder, InsightThreadSummary},
};
use chrono::{DateTime, Duration, Utc};
use std::collections::{HashMap, HashSet};

use super::shared::{strongest_evidence_tier, visit_is_query_landing, visit_token_set};

const BURST_GAP_MINUTES: i64 = 30;
const QUERY_GROUP_GAP_HOURS: i64 = 3;
const THREAD_GAP_DAYS: i64 = 30;

#[derive(Debug, Clone, Default)]
pub(super) struct BurstRecord {
    pub burst_id: String,
    pub profile_id: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub visit_indexes: Vec<usize>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct QueryGroupRecord {
    pub query_group_id: String,
    pub profile_id: String,
    pub thread_id: Option<String>,
    pub title: String,
    pub root_query: String,
    pub latest_query: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub visit_indexes: Vec<usize>,
    pub burst_ids: HashSet<String>,
    pub steps: Vec<String>,
    pub stages: Vec<String>,
    pub confidence: f32,
    pub evidence_tier: String,
    pub chromium_enhanced: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ThreadRecord {
    pub thread_id: String,
    pub profile_id: String,
    pub title: String,
    pub status: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub visit_indexes: Vec<usize>,
    pub query_group_ids: Vec<String>,
    pub reopen_count: usize,
    pub open_loop_score: f32,
    pub confidence: f32,
    pub evidence_tier: String,
    pub chromium_enhanced: bool,
    pub evidence: Vec<InsightEvidenceItem>,
}

#[derive(Debug, Clone, Default)]
struct ThreadAccumulator {
    title: String,
    confidence: f32,
    tokens: HashSet<String>,
    domains: HashSet<String>,
    anchors: HashSet<String>,
    last_visit_time: i64,
}

fn query_token_set(query: &str) -> HashSet<String> {
    tokenize_text(query).into_iter().collect()
}

fn visit_is_search_seed(visit: &VisitRecord) -> bool {
    visit.query_term.as_deref().is_some_and(|value| !value.trim().is_empty())
        && (visit.has_canonical_search_term
            || visit.page_category_v2 == PageCategory::SearchResults)
}

fn group_tokens(group: &QueryGroupRecord) -> HashSet<String> {
    group.steps.iter().flat_map(|value| tokenize_text(value)).collect()
}

fn group_domains(group: &QueryGroupRecord, visits: &[VisitRecord]) -> HashSet<String> {
    group.visit_indexes.iter().map(|index| visits[*index].registrable_domain.clone()).collect()
}

fn group_anchors(group: &QueryGroupRecord, visits: &[VisitRecord]) -> HashSet<String> {
    group
        .visit_indexes
        .iter()
        .filter_map(|index| {
            let visit = &visits[*index];
            visit_is_query_landing(visit).then(|| canonical_visit_key(visit))
        })
        .collect()
}

fn finish_query_group(group: &mut QueryGroupRecord, visits: &[VisitRecord]) {
    group.last_seen_at = group
        .visit_indexes
        .last()
        .map(|index| visits[*index].visited_at.clone())
        .unwrap_or_else(|| group.first_seen_at.clone());
    if group.title.is_empty() {
        group.title = group.latest_query.clone();
    }
    let step_bonus = (group.steps.len() as f32 * 0.18).clamp(0.0, 0.54);
    let landing_bonus =
        group.visit_indexes.iter().filter(|index| visit_is_query_landing(&visits[**index])).count()
            as f32
            * 0.08;
    let evidence_bonus = if group.evidence_tier == "tier-a" { 0.24 } else { 0.08 };
    group.confidence = (0.36 + step_bonus + landing_bonus + evidence_bonus).clamp(0.0, 1.0);
}

pub(super) fn build_bursts(visits: &mut [VisitRecord]) -> Vec<BurstRecord> {
    let mut bursts = Vec::new();
    let mut current: Option<BurstRecord> = None;
    let mut previous_index: Option<usize> = None;

    for (index, visit) in visits.iter().enumerate() {
        let continues = previous_index.is_some_and(|previous_index| {
            let previous = &visits[previous_index];
            previous.profile_id == visit.profile_id
                && chrome_gap_minutes(previous.visit_time, visit.visit_time) <= BURST_GAP_MINUTES
                && (visit.from_visit == Some(previous.source_visit_id)
                    || previous.from_visit == Some(visit.source_visit_id)
                    || previous.registrable_domain == visit.registrable_domain
                    || token_similarity(&visit_token_set(previous), &visit_token_set(visit))
                        >= 0.16)
        });

        if !continues {
            if let Some(current) = current.take() {
                bursts.push(current);
            }
            current = Some(BurstRecord {
                burst_id: format!("burst-{:03}", bursts.len() + 1),
                profile_id: visit.profile_id.clone(),
                first_seen_at: visit.visited_at.clone(),
                last_seen_at: visit.visited_at.clone(),
                visit_indexes: vec![index],
                confidence: 0.45,
            });
        } else if let Some(current) = &mut current {
            current.visit_indexes.push(index);
            current.last_seen_at = visit.visited_at.clone();
        }

        previous_index = Some(index);
    }

    if let Some(current) = current {
        bursts.push(current);
    }

    for burst in &mut bursts {
        let search_bonus =
            burst.visit_indexes.iter().filter(|index| visits[**index].query_term.is_some()).count()
                as f32
                * 0.08;
        burst.confidence = (0.4 + search_bonus).clamp(0.0, 1.0);
        for index in &burst.visit_indexes {
            visits[*index].burst_id = Some(burst.burst_id.clone());
        }
    }

    bursts
}

pub(super) fn build_query_groups(
    visits: &mut [VisitRecord],
) -> (Vec<QueryGroupRecord>, Vec<InsightQueryLadder>) {
    let mut groups = Vec::<QueryGroupRecord>::new();
    let mut current_by_profile = HashMap::<String, QueryGroupRecord>::new();

    for (index, visit) in visits.iter().enumerate() {
        let profile_id = visit.profile_id.clone();
        let visit_query = visit.query_term.clone();
        let starts_query = visit_is_search_seed(visit);

        if let Some(mut current) = current_by_profile.remove(&profile_id) {
            let gap_ok = current.visit_indexes.last().is_some_and(|previous_index| {
                chrome_gap_hours(visits[*previous_index].visit_time, visit.visit_time)
                    <= QUERY_GROUP_GAP_HOURS
            });
            let token_overlap = visit_query
                .as_deref()
                .map(|query| token_similarity(&group_tokens(&current), &query_token_set(query)))
                .unwrap_or_else(|| {
                    token_similarity(
                        &visit_token_set(visit),
                        &current
                            .visit_indexes
                            .last()
                            .map(|previous_index| visit_token_set(&visits[*previous_index]))
                            .unwrap_or_default(),
                    )
                });
            let landing_continuity = visit_is_query_landing(visit)
                && current.visit_indexes.last().is_some_and(|previous_index| {
                    visits[*previous_index].from_visit == Some(visit.source_visit_id)
                        || visit.from_visit == Some(visits[*previous_index].source_visit_id)
                        || visits[*previous_index].registrable_domain == visit.registrable_domain
                });
            let continues = gap_ok && (token_overlap >= 0.18 || landing_continuity);
            let strong_new_query = starts_query && token_overlap < 0.18;

            if continues {
                current.visit_indexes.push(index);
                if let Some(burst_id) = &visit.burst_id {
                    current.burst_ids.insert(burst_id.clone());
                }
                if let Some(query) = visit_query.as_deref() {
                    if current.steps.last().is_none_or(|value| value != query) {
                        let previous = current.steps.last().cloned();
                        current.steps.push(query.to_string());
                        current.stages.push(classify_query_stage(Some(query), previous.as_deref()));
                    }
                    current.latest_query = query.to_string();
                    current.title = query.to_string();
                }
                current.evidence_tier = strongest_evidence_tier(
                    current
                        .visit_indexes
                        .iter()
                        .map(|member_index| visits[*member_index].evidence_tier),
                );
                current.chromium_enhanced |= visit.has_canonical_search_term;
                current_by_profile.insert(profile_id.clone(), current);
                continue;
            }

            finish_query_group(&mut current, visits);
            if current.visit_indexes.len() > 1 || current.steps.len() > 1 || strong_new_query {
                groups.push(current);
            }
        }

        if starts_query {
            let query = visit_query.unwrap_or_else(|| "search".to_string());
            current_by_profile.insert(
                profile_id,
                QueryGroupRecord {
                    query_group_id: format!(
                        "query-group-{:03}",
                        groups.len() + current_by_profile.len() + 1
                    ),
                    profile_id: visit.profile_id.clone(),
                    thread_id: None,
                    title: query.clone(),
                    root_query: query.clone(),
                    latest_query: query.clone(),
                    first_seen_at: visit.visited_at.clone(),
                    last_seen_at: visit.visited_at.clone(),
                    visit_indexes: vec![index],
                    burst_ids: visit.burst_id.clone().into_iter().collect(),
                    steps: vec![query.clone()],
                    stages: vec![classify_query_stage(Some(&query), None)],
                    confidence: 0.0,
                    evidence_tier: visit.evidence_tier.as_str().to_string(),
                    chromium_enhanced: visit.has_canonical_search_term,
                },
            );
        }
    }

    for (_, mut current) in current_by_profile {
        finish_query_group(&mut current, visits);
        if current.visit_indexes.len() > 1 || current.steps.len() > 1 {
            groups.push(current);
        }
    }

    for group in &groups {
        for index in &group.visit_indexes {
            visits[*index].query_group_id = Some(group.query_group_id.clone());
        }
    }

    groups.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
    let mut ladders = groups
        .iter()
        .filter(|group| group.steps.len() > 1)
        .map(|group| InsightQueryLadder {
            query_group_id: Some(group.query_group_id.clone()),
            root_term: group.root_query.clone(),
            profile_id: group.profile_id.clone(),
            steps: group.steps.clone(),
            stages: group.stages.clone(),
            count: group.visit_indexes.len(),
            confidence: group.confidence,
            evidence_tier: group.evidence_tier.clone(),
            chromium_only: group.chromium_enhanced,
        })
        .collect::<Vec<_>>();
    ladders.sort_by(|left, right| {
        right.steps.len().cmp(&left.steps.len()).then(right.count.cmp(&left.count))
    });
    ladders.truncate(6);

    (groups, ladders)
}

pub(super) fn build_threads(
    visits: &mut [VisitRecord],
    query_groups: &mut [QueryGroupRecord],
) -> Vec<ThreadRecord> {
    query_groups.sort_by(|left, right| left.first_seen_at.cmp(&right.first_seen_at));
    let mut threads = Vec::<ThreadRecord>::new();
    let mut accumulators = Vec::<ThreadAccumulator>::new();

    for group in query_groups.iter_mut() {
        let current_tokens = group_tokens(group);
        let current_domains = group_domains(group, visits);
        let current_anchors = group_anchors(group, visits);
        let current_start =
            group.visit_indexes.first().map(|index| visits[*index].visit_time).unwrap_or_default();
        let current_end = group
            .visit_indexes
            .last()
            .map(|index| visits[*index].visit_time)
            .unwrap_or(current_start);
        let mut best_index = None;
        let mut best_score = 0.0f32;

        for (thread_index, thread) in threads.iter().enumerate() {
            if thread.profile_id != group.profile_id {
                continue;
            }
            let accumulator = &accumulators[thread_index];
            let gap_days = current_start - accumulator.last_visit_time;
            let gap_days = gap_days.max(0) / 1_000_000 / 60 / 60 / 24;
            if gap_days > THREAD_GAP_DAYS {
                continue;
            }
            let token_score = token_similarity(&current_tokens, &accumulator.tokens);
            let domain_score = token_similarity(&current_domains, &accumulator.domains);
            let anchor_score = token_similarity(&current_anchors, &accumulator.anchors);
            let reopen_bonus = if gap_days >= 1 && (token_score >= 0.22 || anchor_score >= 0.2) {
                0.12
            } else {
                0.0
            };
            let score =
                token_score * 0.45 + domain_score * 0.25 + anchor_score * 0.3 + reopen_bonus;
            if score > best_score {
                best_score = score;
                best_index = Some(thread_index);
            }
        }

        let thread_index = best_index.filter(|_| best_score >= 0.28).unwrap_or_else(|| {
            threads.push(ThreadRecord {
                thread_id: format!("thread-{:03}", threads.len() + 1),
                profile_id: group.profile_id.clone(),
                title: group.title.clone(),
                status: "active".to_string(),
                first_seen_at: group.first_seen_at.clone(),
                last_seen_at: group.last_seen_at.clone(),
                visit_indexes: Vec::new(),
                query_group_ids: Vec::new(),
                reopen_count: 0,
                open_loop_score: 0.0,
                confidence: 0.0,
                evidence_tier: group.evidence_tier.clone(),
                chromium_enhanced: group.chromium_enhanced,
                evidence: Vec::new(),
            });
            accumulators.push(ThreadAccumulator::default());
            threads.len() - 1
        });

        if let Some(previous_index) = threads[thread_index].visit_indexes.last().copied() {
            if chrome_gap_hours(visits[previous_index].visit_time, current_start) >= 24 {
                threads[thread_index].reopen_count += 1;
            }
        }

        threads[thread_index].last_seen_at = group.last_seen_at.clone();
        threads[thread_index].visit_indexes.extend(group.visit_indexes.iter().copied());
        threads[thread_index].query_group_ids.push(group.query_group_id.clone());
        threads[thread_index].chromium_enhanced |= group.chromium_enhanced;
        threads[thread_index].confidence =
            threads[thread_index].confidence.max(best_score.max(group.confidence));
        threads[thread_index].evidence_tier = if threads[thread_index].evidence_tier == "tier-a"
            || group.evidence_tier == "tier-a"
        {
            "tier-a".to_string()
        } else if threads[thread_index].evidence_tier == "tier-b" || group.evidence_tier == "tier-b"
        {
            "tier-b".to_string()
        } else {
            "tier-c".to_string()
        };
        group.thread_id = Some(threads[thread_index].thread_id.clone());
        let accumulator = &mut accumulators[thread_index];
        accumulator.title = if group.confidence >= accumulator.confidence {
            group.title.clone()
        } else {
            accumulator.title.clone()
        };
        accumulator.confidence = accumulator.confidence.max(group.confidence);
        accumulator.tokens.extend(current_tokens);
        accumulator.domains.extend(current_domains);
        accumulator.anchors.extend(current_anchors);
        accumulator.last_visit_time = current_end;
    }

    for (thread_index, thread) in threads.iter_mut().enumerate() {
        thread.visit_indexes.sort_unstable();
        for index in &thread.visit_indexes {
            visits[*index].thread_id = Some(thread.thread_id.clone());
        }
        thread.evidence = thread
            .visit_indexes
            .iter()
            .rev()
            .take(4)
            .map(|index| evidence_from_visit(&visits[*index], Some(thread.status.clone())))
            .collect();
        let query_group_bonus = thread
            .visit_indexes
            .iter()
            .filter(|index| visits[**index].query_group_id.is_some())
            .count() as f32
            * 0.08;
        let anchor_bonus = accumulators[thread_index].anchors.len() as f32 * 0.08;
        thread.open_loop_score =
            (thread.reopen_count as f32 * 0.6 + query_group_bonus + anchor_bonus).clamp(0.0, 4.0);
        let last_seen_recent = DateTime::parse_from_rfc3339(&thread.last_seen_at)
            .ok()
            .is_some_and(|value| Utc::now() - value.with_timezone(&Utc) <= Duration::days(7));
        thread.status = if thread.open_loop_score >= 1.2 {
            "open-loop".to_string()
        } else if last_seen_recent {
            "active".to_string()
        } else {
            "archived".to_string()
        };
        if accumulators[thread_index].confidence > 0.0 {
            thread.title = accumulators[thread_index].title.clone();
        }
    }

    threads.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
    threads
}

pub(super) fn query_group_summaries_from_records(
    query_groups: &[QueryGroupRecord],
    visits: &[VisitRecord],
) -> Vec<InsightQueryGroupSummary> {
    query_groups
        .iter()
        .map(|group| InsightQueryGroupSummary {
            query_group_id: group.query_group_id.clone(),
            profile_id: group.profile_id.clone(),
            thread_id: group.thread_id.clone(),
            title: group.title.clone(),
            root_query: group.root_query.clone(),
            latest_query: group.latest_query.clone(),
            first_seen_at: group.first_seen_at.clone(),
            last_seen_at: group.last_seen_at.clone(),
            visit_count: group.visit_indexes.len(),
            burst_count: group.burst_ids.len(),
            step_count: group.steps.len(),
            confidence: group.confidence,
            evidence_tier: group.evidence_tier.clone(),
            chromium_enhanced: group.chromium_enhanced,
            steps: group.steps.clone(),
            stages: group.stages.clone(),
            evidence: group
                .visit_indexes
                .iter()
                .rev()
                .take(4)
                .map(|index| evidence_from_visit(&visits[*index], Some(group.title.clone())))
                .collect(),
        })
        .collect()
}

pub(super) fn thread_summaries_from_records(threads: &[ThreadRecord]) -> Vec<InsightThreadSummary> {
    threads
        .iter()
        .map(|thread| InsightThreadSummary {
            thread_id: thread.thread_id.clone(),
            profile_id: thread.profile_id.clone(),
            title: thread.title.clone(),
            status: thread.status.clone(),
            first_seen_at: thread.first_seen_at.clone(),
            last_seen_at: thread.last_seen_at.clone(),
            visit_count: thread.visit_indexes.len(),
            query_group_count: thread.query_group_ids.len(),
            reopen_count: thread.reopen_count,
            open_loop_score: thread.open_loop_score,
            confidence: thread.confidence,
            evidence_tier: thread.evidence_tier.clone(),
            dominant_topic_id: None,
            chromium_enhanced: thread.chromium_enhanced,
            evidence: thread.evidence.clone(),
        })
        .collect()
}
