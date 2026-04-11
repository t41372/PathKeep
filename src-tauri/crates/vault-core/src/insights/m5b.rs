use super::{
    INSIGHT_PIPELINE_VERSION, InsightEvidenceItem, VisitRecord, canonical_visit_key,
    chrome_gap_hours, chrome_gap_minutes, classify_query_stage, evidence_from_visit,
    is_chromium_profile, token_similarity, url_domain,
};
use crate::{
    deterministic::{EvidenceTier, PageCategory, tokenize_text},
    models::{
        InsightCard, InsightQueryGroupSummary, InsightQueryLadder, InsightReferencePageSummary,
        InsightSourceEffectivenessSummary, InsightTemplateSummary, InsightThreadSummary,
        InsightTopicSummary,
    },
};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, Row, params};
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone)]
struct GroupSnapshot {
    title: String,
    confidence: f32,
    tokens: HashSet<String>,
    domains: HashSet<String>,
    anchors: HashSet<String>,
}

fn visit_token_set(visit: &VisitRecord) -> HashSet<String> {
    visit.keywords.iter().cloned().collect()
}

fn query_token_set(query: &str) -> HashSet<String> {
    tokenize_text(query).into_iter().collect()
}

fn strongest_evidence_tier(items: impl Iterator<Item = EvidenceTier>) -> String {
    let mut best = EvidenceTier::TierC;
    for tier in items {
        match tier {
            EvidenceTier::TierA => return tier.as_str().to_string(),
            EvidenceTier::TierB if best == EvidenceTier::TierC => best = tier,
            EvidenceTier::TierB | EvidenceTier::TierC => {}
        }
    }
    best.as_str().to_string()
}

fn visit_is_search_seed(visit: &VisitRecord) -> bool {
    visit.query_term.as_deref().is_some_and(|value| !value.trim().is_empty())
        && (visit.has_canonical_search_term
            || visit.page_category_v2 == PageCategory::SearchResults)
}

fn visit_is_query_landing(visit: &VisitRecord) -> bool {
    matches!(
        visit.page_category_v2,
        PageCategory::DocsPage
            | PageCategory::Repo
            | PageCategory::Issue
            | PageCategory::PullRequest
            | PageCategory::ForumThread
            | PageCategory::ArticlePage
            | PageCategory::ProductPage
            | PageCategory::VideoPage
    )
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
    let mut group_snapshots = HashMap::<String, GroupSnapshot>::new();

    for group in query_groups.iter_mut() {
        let current_tokens = group_tokens(group);
        let current_domains = group_domains(group, visits);
        let current_anchors = group_anchors(group, visits);
        let current_start =
            group.visit_indexes.first().map(|index| visits[*index].visit_time).unwrap_or_default();
        let mut best_index = None;
        let mut best_score = 0.0f32;

        for (thread_index, thread) in threads.iter().enumerate() {
            if thread.profile_id != group.profile_id {
                continue;
            }
            let gap_days = current_start
                - thread
                    .visit_indexes
                    .last()
                    .map(|index| visits[*index].visit_time)
                    .unwrap_or_default();
            let gap_days = gap_days.max(0) / 1_000_000 / 60 / 60 / 24;
            if gap_days > THREAD_GAP_DAYS {
                continue;
            }

            let thread_tokens = thread
                .query_group_ids
                .iter()
                .filter_map(|id| group_snapshots.get(id))
                .flat_map(|snapshot| snapshot.tokens.iter().cloned())
                .collect::<HashSet<_>>();
            let thread_domains = thread
                .query_group_ids
                .iter()
                .filter_map(|id| group_snapshots.get(id))
                .flat_map(|snapshot| snapshot.domains.iter().cloned())
                .collect::<HashSet<_>>();
            let thread_anchors = thread
                .query_group_ids
                .iter()
                .filter_map(|id| group_snapshots.get(id))
                .flat_map(|snapshot| snapshot.anchors.iter().cloned())
                .collect::<HashSet<_>>();
            let token_score = token_similarity(&current_tokens, &thread_tokens);
            let domain_score = token_similarity(&current_domains, &thread_domains);
            let anchor_score = token_similarity(&current_anchors, &thread_anchors);
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
        group_snapshots.insert(
            group.query_group_id.clone(),
            GroupSnapshot {
                title: group.title.clone(),
                confidence: group.confidence,
                tokens: current_tokens,
                domains: current_domains,
                anchors: current_anchors,
            },
        );
    }

    for thread in &mut threads {
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
        let anchor_bonus = thread
            .query_group_ids
            .iter()
            .filter_map(|id| group_snapshots.get(id))
            .flat_map(|snapshot| snapshot.anchors.iter().cloned())
            .collect::<HashSet<_>>()
            .len() as f32
            * 0.08;
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
        if let Some(best_group) = thread
            .query_group_ids
            .iter()
            .filter_map(|id| group_snapshots.get(id))
            .max_by(|left, right| left.confidence.total_cmp(&right.confidence))
        {
            thread.title = best_group.title.clone();
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

pub(super) fn build_topics(
    visits: &[VisitRecord],
    query_groups: &[InsightQueryGroupSummary],
    threads: &[InsightThreadSummary],
    window_days: u32,
) -> Vec<InsightTopicSummary> {
    let mut topics = Vec::<InsightTopicSummary>::new();

    for thread in threads {
        let thread_tokens = tokenize_text(&thread.title).into_iter().collect::<HashSet<_>>();
        let mut best_index = None;
        let mut best_score = 0.0f32;
        for (topic_index, topic) in topics.iter().enumerate() {
            let topic_tokens = tokenize_text(&topic.label).into_iter().collect::<HashSet<_>>();
            let score = token_similarity(&thread_tokens, &topic_tokens);
            if score > best_score {
                best_score = score;
                best_index = Some(topic_index);
            }
        }

        let topic_index = best_index.filter(|_| best_score >= 0.28).unwrap_or_else(|| {
            topics.push(InsightTopicSummary {
                topic_id: format!("topic-{:03}", topics.len() + 1),
                label: thread.title.clone(),
                profile_scope: thread.profile_id.clone(),
                window_days,
                first_seen_at: thread.first_seen_at.clone(),
                last_seen_at: thread.last_seen_at.clone(),
                visit_count: 0,
                revisit_count: 0,
                trend_slope: 0.0,
                burst_score: 0.0,
                evidence: Vec::new(),
            });
            topics.len() - 1
        });

        let topic = &mut topics[topic_index];
        topic.first_seen_at = topic.first_seen_at.clone().min(thread.first_seen_at.clone());
        topic.last_seen_at = topic.last_seen_at.clone().max(thread.last_seen_at.clone());
        topic.visit_count += thread.visit_count;
        topic.revisit_count += thread.reopen_count;
        topic.burst_score += thread.query_group_count as f32;
        topic.trend_slope += thread.confidence;
        if topic.evidence.len() < 4 {
            topic.evidence.extend(thread.evidence.iter().take(1).cloned());
        }
    }

    for topic in &mut topics {
        let topic_tokens = tokenize_text(&topic.label).into_iter().collect::<HashSet<_>>();
        let matching_groups = query_groups
            .iter()
            .filter(|group| {
                let group_tokens = tokenize_text(&group.title).into_iter().collect::<HashSet<_>>();
                token_similarity(&topic_tokens, &group_tokens) >= 0.22
            })
            .count();
        topic.burst_score = (topic.burst_score / topic.visit_count.max(1) as f32
            + matching_groups as f32 * 0.15)
            .clamp(0.0, 4.0);
        topic.trend_slope = (topic.trend_slope / topic.visit_count.max(1) as f32).clamp(0.0, 1.0);
        topic.revisit_count = count_topic_revisits_like(topic, visits);
    }

    topics.sort_by(|left, right| {
        right
            .trend_slope
            .total_cmp(&left.trend_slope)
            .then(right.visit_count.cmp(&left.visit_count))
    });
    topics
}

fn count_topic_revisits_like(topic: &InsightTopicSummary, visits: &[VisitRecord]) -> usize {
    let topic_tokens = tokenize_text(&topic.label).into_iter().collect::<HashSet<_>>();
    let mut counts = HashMap::<String, usize>::new();
    for visit in visits {
        if token_similarity(&topic_tokens, &visit_token_set(visit)) >= 0.22 {
            *counts.entry(canonical_visit_key(visit)).or_insert(0) += 1;
        }
    }
    counts.values().filter(|count| **count > 1).count()
}

pub(super) fn build_reference_pages(
    visits: &[VisitRecord],
    profile_scope: &str,
) -> Vec<InsightReferencePageSummary> {
    #[derive(Default)]
    struct Acc {
        sample: Option<usize>,
        visit_indexes: Vec<usize>,
        query_groups: HashSet<String>,
        threads: HashSet<String>,
        days: HashSet<String>,
        evidence_tiers: Vec<EvidenceTier>,
    }

    let mut grouped = HashMap::<String, Acc>::new();
    for (index, visit) in visits.iter().enumerate() {
        if !visit_is_query_landing(visit) {
            continue;
        }
        let acc = grouped.entry(canonical_visit_key(visit)).or_default();
        acc.sample.get_or_insert(index);
        acc.visit_indexes.push(index);
        if let Some(query_group_id) = &visit.query_group_id {
            acc.query_groups.insert(query_group_id.clone());
        }
        if let Some(thread_id) = &visit.thread_id {
            acc.threads.insert(thread_id.clone());
        }
        if let Some(day) = day_key(&visit.visited_at) {
            acc.days.insert(day);
        }
        acc.evidence_tiers.push(visit.evidence_tier);
    }

    let mut pages = grouped
        .into_iter()
        .filter_map(|(_, acc)| {
            let sample_index = acc.sample?;
            let sample = &visits[sample_index];
            let revisit_count = acc.visit_indexes.len();
            let cross_day_revisits = acc.days.len().saturating_sub(1);
            let query_group_count = acc.query_groups.len();
            let thread_count = acc.threads.len();
            if revisit_count < 2 && cross_day_revisits == 0 && query_group_count < 2 {
                return None;
            }
            Some(InsightReferencePageSummary {
                reference_page_id: format!("reference-{:x}", fxhash(&sample.url)),
                profile_id: (profile_scope != "all").then(|| profile_scope.to_string()),
                url: sample.url.clone(),
                title: sample.readable_title.clone().or_else(|| sample.title.clone()),
                domain: url_domain(&sample.url),
                first_seen_at: acc
                    .visit_indexes
                    .first()
                    .map(|index| visits[*index].visited_at.clone())
                    .unwrap_or_else(|| sample.visited_at.clone()),
                last_seen_at: acc
                    .visit_indexes
                    .last()
                    .map(|index| visits[*index].visited_at.clone())
                    .unwrap_or_else(|| sample.visited_at.clone()),
                revisit_count,
                cross_day_revisits,
                query_group_count,
                thread_count,
                score: (revisit_count as f32 * 0.3
                    + cross_day_revisits as f32 * 0.35
                    + query_group_count as f32 * 0.25
                    + thread_count as f32 * 0.15)
                    .clamp(0.0, 5.0),
                evidence_tier: strongest_evidence_tier(acc.evidence_tiers.into_iter()),
                evidence: acc
                    .visit_indexes
                    .iter()
                    .rev()
                    .take(4)
                    .map(|index| {
                        evidence_from_visit(
                            &visits[*index],
                            Some("Reused as a stable reference page.".to_string()),
                        )
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    pages.sort_by(|left, right| right.score.total_cmp(&left.score));
    pages.truncate(12);
    pages
}

pub(super) fn build_source_effectiveness(
    visits: &[VisitRecord],
    profile_scope: &str,
    reference_pages: &[InsightReferencePageSummary],
) -> Vec<InsightSourceEffectivenessSummary> {
    #[derive(Default)]
    struct Acc {
        sample_indexes: Vec<usize>,
        query_groups: HashSet<String>,
        threads: HashSet<String>,
        stable_landing_count: usize,
        reference_page_count: usize,
        reopen_support_count: usize,
        roles: HashMap<String, usize>,
        evidence_tiers: Vec<EvidenceTier>,
    }

    let reference_domains =
        reference_pages.iter().map(|page| page.domain.clone()).collect::<HashSet<_>>();
    let mut grouped = HashMap::<String, Acc>::new();
    for (index, visit) in visits.iter().enumerate() {
        let acc = grouped.entry(visit.registrable_domain.clone()).or_default();
        if acc.sample_indexes.len() < 4 {
            acc.sample_indexes.push(index);
        }
        if let Some(query_group_id) = &visit.query_group_id {
            acc.query_groups.insert(query_group_id.clone());
        }
        if let Some(thread_id) = &visit.thread_id {
            acc.threads.insert(thread_id.clone());
        }
        if visit_is_query_landing(visit) {
            acc.stable_landing_count += 1;
        }
        if reference_domains.contains(&visit.registrable_domain) {
            acc.reference_page_count += 1;
        }
        if visit.query_group_id.is_some()
            && visit.thread_id.is_some()
            && visit_is_query_landing(visit)
        {
            acc.reopen_support_count += 1;
        }
        *acc.roles.entry(visit.source_role.clone()).or_insert(0) += 1;
        acc.evidence_tiers.push(visit.evidence_tier);
    }

    let mut rows = grouped
        .into_iter()
        .filter_map(|(domain, acc)| {
            let sample_index = *acc.sample_indexes.first()?;
            let sample = &visits[sample_index];
            let source_role = acc
                .roles
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(role, _)| role)
                .unwrap_or_else(|| sample.source_role.clone());
            Some(InsightSourceEffectivenessSummary {
                source_id: format!("source-{:x}", fxhash(&domain)),
                profile_id: (profile_scope != "all").then(|| profile_scope.to_string()),
                domain,
                source_role,
                query_group_count: acc.query_groups.len(),
                thread_count: acc.threads.len(),
                stable_landing_count: acc.stable_landing_count,
                reference_page_count: acc.reference_page_count,
                reopen_support_count: acc.reopen_support_count,
                effectiveness_score: (acc.stable_landing_count as f32 * 0.3
                    + acc.reference_page_count as f32 * 0.35
                    + acc.reopen_support_count as f32 * 0.15
                    + acc.query_groups.len() as f32 * 0.12)
                    .clamp(0.0, 5.0),
                evidence_tier: strongest_evidence_tier(acc.evidence_tiers.into_iter()),
                evidence: acc
                    .sample_indexes
                    .iter()
                    .map(|index| {
                        evidence_from_visit(
                            &visits[*index],
                            Some("Supports deterministic source effectiveness.".to_string()),
                        )
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.effectiveness_score.total_cmp(&left.effectiveness_score));
    rows.truncate(10);
    rows
}

pub(super) fn build_template_summaries(
    visits: &[VisitRecord],
    query_groups: &[InsightQueryGroupSummary],
    threads: &[InsightThreadSummary],
    reference_pages: &[InsightReferencePageSummary],
    source_effectiveness: &[InsightSourceEffectivenessSummary],
    profile_id: Option<&str>,
) -> Vec<InsightTemplateSummary> {
    let mut summaries = Vec::new();
    if let Some(group) = query_groups.first() {
        summaries.push(InsightTemplateSummary {
            summary_id: "summary-query-groups".to_string(),
            kind: "query-groups".to_string(),
            title: "Recent query refinement".to_string(),
            body: format!(
                "\"{}\" evolved through {} steps and {} visits in the current window.",
                group.root_query, group.step_count, group.visit_count
            ),
            confidence: group.confidence,
            profile_id: profile_id.map(ToString::to_string),
            evidence: group.evidence.clone(),
        });
    }
    if let Some(thread) = threads.first() {
        summaries.push(InsightTemplateSummary {
            summary_id: "summary-open-loop".to_string(),
            kind: "open-loop".to_string(),
            title: "Repeatedly reopened research".to_string(),
            body: format!(
                "\"{}\" reopened {} times and still reads like an active line of research.",
                thread.title, thread.reopen_count
            ),
            confidence: thread.confidence,
            profile_id: profile_id.map(ToString::to_string),
            evidence: thread.evidence.clone(),
        });
    }
    if let Some(reference) = reference_pages.first() {
        summaries.push(InsightTemplateSummary {
            summary_id: "summary-reference-pages".to_string(),
            kind: "reference-pages".to_string(),
            title: "Stable reference page".to_string(),
            body: format!(
                "{} kept resurfacing across {} groups and {} threads.",
                reference.title.clone().unwrap_or_else(|| reference.url.clone()),
                reference.query_group_count,
                reference.thread_count
            ),
            confidence: (0.45 + reference.score / 10.0).clamp(0.0, 1.0),
            profile_id: profile_id.map(ToString::to_string),
            evidence: reference.evidence.clone(),
        });
    }
    if let Some(source) = source_effectiveness.first() {
        summaries.push(InsightTemplateSummary {
            summary_id: "summary-source-effectiveness".to_string(),
            kind: "source-effectiveness".to_string(),
            title: "Consistent source anchor".to_string(),
            body: format!(
                "{} often became the stable landing point after searches and reformulations.",
                source.domain
            ),
            confidence: (0.4 + source.effectiveness_score / 10.0).clamp(0.0, 1.0),
            profile_id: profile_id.map(ToString::to_string),
            evidence: source.evidence.clone(),
        });
    }
    if summaries.is_empty() {
        if let Some(visit) = visits.last() {
            summaries.push(InsightTemplateSummary {
                summary_id: "summary-recent-activity".to_string(),
                kind: "recent-activity".to_string(),
                title: "Latest captured activity".to_string(),
                body: format!(
                    "Captured {} visit(s) in the current window, most recently {}.",
                    visits.len(),
                    visit
                        .readable_title
                        .clone()
                        .or_else(|| visit.title.clone())
                        .unwrap_or_else(|| visit.url.clone())
                ),
                confidence: (0.28 + visits.len().min(5) as f32 * 0.08).clamp(0.0, 0.7),
                profile_id: profile_id.map(ToString::to_string),
                evidence: vec![evidence_from_visit(
                    visit,
                    Some(
                        "Deterministic fallback summary for a minimal activity window.".to_string(),
                    ),
                )],
            });
        }
    }
    summaries
}

pub(super) fn build_cards(
    template_summaries: &[InsightTemplateSummary],
    threads: &[InsightThreadSummary],
    reference_pages: &[InsightReferencePageSummary],
    window_days: u32,
    profile_id: Option<&str>,
) -> Vec<InsightCard> {
    let mut cards = template_summaries
        .iter()
        .take(2)
        .map(|summary| InsightCard {
            card_id: format!("card-{}", summary.summary_id),
            kind: summary.kind.clone(),
            title: summary.title.clone(),
            summary: summary.body.clone(),
            window_days,
            profile_id: profile_id.map(ToString::to_string),
            score: summary.confidence,
            chromium_enhanced: summary
                .evidence
                .iter()
                .any(|item| is_chromium_profile(&item.profile_id)),
            evidence: summary.evidence.clone(),
        })
        .collect::<Vec<_>>();

    if let Some(thread) =
        threads.iter().max_by(|left, right| left.open_loop_score.total_cmp(&right.open_loop_score))
    {
        cards.push(InsightCard {
            card_id: format!("card-open-loop-{}", thread.thread_id),
            kind: "open-loop".to_string(),
            title: format!("Open loop: {}", thread.title),
            summary: format!(
                "This research line reopened {} times and still depends on repeated reference checks.",
                thread.reopen_count
            ),
            window_days,
            profile_id: profile_id.map(ToString::to_string),
            score: thread.open_loop_score,
            chromium_enhanced: thread.chromium_enhanced,
            evidence: thread.evidence.clone(),
        });
    }

    if let Some(reference) = reference_pages.first() {
        cards.push(InsightCard {
            card_id: format!("card-reference-{}", reference.reference_page_id),
            kind: "reference-page".to_string(),
            title: "Frequently reused reference".to_string(),
            summary: format!(
                "{} became a stable reference page across this window.",
                reference.title.clone().unwrap_or_else(|| reference.url.clone())
            ),
            window_days,
            profile_id: profile_id.map(ToString::to_string),
            score: reference.score,
            chromium_enhanced: false,
            evidence: reference.evidence.clone(),
        });
    }

    cards
}

pub(super) fn persist_bursts(connection: &Connection, bursts: &[BurstRecord]) -> Result<()> {
    connection.execute("DELETE FROM insight_bursts", [])?;
    for burst in bursts {
        connection.execute(
            "INSERT INTO insight_bursts
             (burst_id, profile_id, first_seen_at, last_seen_at, visit_count, confidence, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                burst.burst_id,
                burst.profile_id,
                burst.first_seen_at,
                burst.last_seen_at,
                burst.visit_indexes.len() as i64,
                burst.confidence,
                serde_json::to_string(&burst.visit_indexes)?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn persist_query_groups(
    connection: &Connection,
    query_groups: &[QueryGroupRecord],
    visits: &[VisitRecord],
) -> Result<()> {
    connection.execute("DELETE FROM insight_query_groups", [])?;
    connection.execute("DELETE FROM insight_query_group_members", [])?;
    for group in query_groups {
        connection.execute(
            "INSERT INTO insight_query_groups
             (query_group_id, profile_id, thread_id, title, root_query, latest_query, first_seen_at,
              last_seen_at, visit_count, burst_count, step_count, confidence, evidence_tier,
              chromium_enhanced, steps_json, stages_json, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                group.query_group_id,
                group.profile_id,
                group.thread_id,
                group.title,
                group.root_query,
                group.latest_query,
                group.first_seen_at,
                group.last_seen_at,
                group.visit_indexes.len() as i64,
                group.burst_ids.len() as i64,
                group.steps.len() as i64,
                group.confidence,
                group.evidence_tier,
                group.chromium_enhanced as i64,
                serde_json::to_string(&group.steps)?,
                serde_json::to_string(&group.stages)?,
                serde_json::to_string(
                    &group
                        .visit_indexes
                        .iter()
                        .rev()
                        .take(4)
                        .map(|index| evidence_from_visit(&visits[*index], Some(group.title.clone())))
                        .collect::<Vec<_>>(),
                )?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
        for (ordinal, index) in group.visit_indexes.iter().enumerate() {
            connection.execute(
                "INSERT INTO insight_query_group_members
                 (query_group_id, history_id, ordinal, visited_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    group.query_group_id,
                    visits[*index].history_id,
                    ordinal as i64,
                    visits[*index].visited_at,
                ],
            )?;
        }
    }
    Ok(())
}

pub(super) fn persist_topic_summaries(
    connection: &Connection,
    profile_scope: &str,
    window_days: u32,
    topics: &[InsightTopicSummary],
) -> Result<()> {
    connection.execute(
        "DELETE FROM insight_topics WHERE profile_scope = ?1 AND window_days = ?2",
        params![profile_scope, window_days as i64],
    )?;
    for topic in topics {
        connection.execute(
            "INSERT OR REPLACE INTO insight_topics
             (topic_id, profile_scope, window_days, label, first_seen_at, last_seen_at, visit_count,
              revisit_count, trend_slope, burst_score, evidence_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                topic.topic_id,
                profile_scope,
                window_days as i64,
                topic.label,
                topic.first_seen_at,
                topic.last_seen_at,
                topic.visit_count as i64,
                topic.revisit_count as i64,
                topic.trend_slope,
                topic.burst_score,
                serde_json::to_string(&topic.evidence)?,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn persist_threads(
    connection: &Connection,
    threads: &[ThreadRecord],
    visits: &[VisitRecord],
) -> Result<()> {
    connection.execute("DELETE FROM insight_threads", [])?;
    connection.execute("DELETE FROM insight_thread_members", [])?;
    for thread in threads {
        connection.execute(
            "INSERT INTO insight_threads
             (thread_id, profile_id, title, status, first_seen_at, last_seen_at, visit_count,
              query_group_count, reopen_count, open_loop_score, confidence, evidence_tier,
              dominant_topic_id, chromium_enhanced, evidence_json, summary_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, ?15, ?16)",
            params![
                thread.thread_id,
                thread.profile_id,
                thread.title,
                thread.status,
                thread.first_seen_at,
                thread.last_seen_at,
                thread.visit_indexes.len() as i64,
                thread.query_group_ids.len() as i64,
                thread.reopen_count as i64,
                thread.open_loop_score,
                thread.confidence,
                thread.evidence_tier,
                thread.chromium_enhanced as i64,
                serde_json::to_string(&thread.evidence)?,
                serde_json::to_string(&serde_json::json!({
                    "queryGroupCount": thread.query_group_ids.len(),
                    "reopenCount": thread.reopen_count,
                    "openLoopScore": thread.open_loop_score,
                }))?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
        for (ordinal, visit_index) in thread.visit_indexes.iter().enumerate() {
            connection.execute(
                "INSERT INTO insight_thread_members (thread_id, history_id, ordinal, visited_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    thread.thread_id,
                    visits[*visit_index].history_id,
                    ordinal as i64,
                    visits[*visit_index].visited_at,
                ],
            )?;
        }
    }
    Ok(())
}

pub(super) fn persist_reference_pages(
    connection: &Connection,
    profile_scope: &str,
    reference_pages: &[InsightReferencePageSummary],
) -> Result<()> {
    connection
        .execute("DELETE FROM insight_reference_pages WHERE profile_scope = ?1", [profile_scope])?;
    for page in reference_pages {
        connection.execute(
            "INSERT INTO insight_reference_pages
             (reference_page_id, profile_scope, url, title, domain, first_seen_at, last_seen_at,
              revisit_count, cross_day_revisits, query_group_count, thread_count, score,
              evidence_tier, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                page.reference_page_id,
                profile_scope,
                page.url,
                page.title,
                page.domain,
                page.first_seen_at,
                page.last_seen_at,
                page.revisit_count as i64,
                page.cross_day_revisits as i64,
                page.query_group_count as i64,
                page.thread_count as i64,
                page.score,
                page.evidence_tier,
                serde_json::to_string(&page.evidence)?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn persist_source_effectiveness(
    connection: &Connection,
    profile_scope: &str,
    source_effectiveness: &[InsightSourceEffectivenessSummary],
) -> Result<()> {
    connection.execute(
        "DELETE FROM insight_source_effectiveness WHERE profile_scope = ?1",
        [profile_scope],
    )?;
    for row in source_effectiveness {
        connection.execute(
            "INSERT INTO insight_source_effectiveness
             (source_id, profile_scope, domain, source_role, query_group_count, thread_count,
              stable_landing_count, reference_page_count, reopen_support_count,
              effectiveness_score, evidence_tier, evidence_json, pipeline_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                row.source_id,
                profile_scope,
                row.domain,
                row.source_role,
                row.query_group_count as i64,
                row.thread_count as i64,
                row.stable_landing_count as i64,
                row.reference_page_count as i64,
                row.reopen_support_count as i64,
                row.effectiveness_score,
                row.evidence_tier,
                serde_json::to_string(&row.evidence)?,
                INSIGHT_PIPELINE_VERSION,
            ],
        )?;
    }
    Ok(())
}

pub(super) fn load_query_groups(
    connection: &Connection,
    profile_id: Option<&str>,
    window_days: u32,
) -> Result<Vec<InsightQueryGroupSummary>> {
    let start = (Utc::now() - Duration::days(window_days as i64)).to_rfc3339();
    let sql = if profile_id.is_some() {
        "SELECT query_group_id, profile_id, thread_id, title, root_query, latest_query,
                first_seen_at, last_seen_at, visit_count, burst_count, step_count, confidence,
                evidence_tier, chromium_enhanced, steps_json, stages_json, evidence_json
         FROM insight_query_groups
         WHERE profile_id = ?1 AND last_seen_at >= ?2
         ORDER BY last_seen_at DESC"
    } else {
        "SELECT query_group_id, profile_id, thread_id, title, root_query, latest_query,
                first_seen_at, last_seen_at, visit_count, burst_count, step_count, confidence,
                evidence_tier, chromium_enhanced, steps_json, stages_json, evidence_json
         FROM insight_query_groups
         WHERE last_seen_at >= ?1
         ORDER BY last_seen_at DESC"
    };
    let mut statement = connection.prepare(sql)?;
    let rows = if let Some(profile_id) = profile_id {
        statement.query_map(params![profile_id, start], query_group_summary_from_row)?
    } else {
        statement.query_map(params![start], query_group_summary_from_row)?
    };
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn load_reference_pages(
    connection: &Connection,
    profile_scope: &str,
) -> Result<Vec<InsightReferencePageSummary>> {
    let mut statement = connection.prepare(
        "SELECT reference_page_id, url, title, domain, first_seen_at, last_seen_at, revisit_count,
                cross_day_revisits, query_group_count, thread_count, score, evidence_tier, evidence_json
         FROM insight_reference_pages
         WHERE profile_scope = ?1
         ORDER BY score DESC, last_seen_at DESC",
    )?;
    let rows = statement.query_map([profile_scope], |row| {
        Ok(InsightReferencePageSummary {
            reference_page_id: row.get(0)?,
            profile_id: (profile_scope != "all").then(|| profile_scope.to_string()),
            url: row.get(1)?,
            title: row.get(2)?,
            domain: row.get(3)?,
            first_seen_at: row.get(4)?,
            last_seen_at: row.get(5)?,
            revisit_count: row.get::<_, i64>(6)?.max(0) as usize,
            cross_day_revisits: row.get::<_, i64>(7)?.max(0) as usize,
            query_group_count: row.get::<_, i64>(8)?.max(0) as usize,
            thread_count: row.get::<_, i64>(9)?.max(0) as usize,
            score: row.get(10)?,
            evidence_tier: row.get(11)?,
            evidence: serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn load_source_effectiveness(
    connection: &Connection,
    profile_scope: &str,
) -> Result<Vec<InsightSourceEffectivenessSummary>> {
    let mut statement = connection.prepare(
        "SELECT source_id, domain, source_role, query_group_count, thread_count, stable_landing_count,
                reference_page_count, reopen_support_count, effectiveness_score, evidence_tier,
                evidence_json
         FROM insight_source_effectiveness
         WHERE profile_scope = ?1
         ORDER BY effectiveness_score DESC, domain ASC",
    )?;
    let rows = statement.query_map([profile_scope], |row| {
        Ok(InsightSourceEffectivenessSummary {
            source_id: row.get(0)?,
            profile_id: (profile_scope != "all").then(|| profile_scope.to_string()),
            domain: row.get(1)?,
            source_role: row.get(2)?,
            query_group_count: row.get::<_, i64>(3)?.max(0) as usize,
            thread_count: row.get::<_, i64>(4)?.max(0) as usize,
            stable_landing_count: row.get::<_, i64>(5)?.max(0) as usize,
            reference_page_count: row.get::<_, i64>(6)?.max(0) as usize,
            reopen_support_count: row.get::<_, i64>(7)?.max(0) as usize,
            effectiveness_score: row.get(8)?,
            evidence_tier: row.get(9)?,
            evidence: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub(super) fn load_thread_query_groups(
    connection: &Connection,
    thread_id: &str,
) -> Result<Vec<InsightQueryGroupSummary>> {
    let mut statement = connection.prepare(
        "SELECT query_group_id, profile_id, thread_id, title, root_query, latest_query,
                first_seen_at, last_seen_at, visit_count, burst_count, step_count, confidence,
                evidence_tier, chromium_enhanced, steps_json, stages_json, evidence_json
         FROM insight_query_groups
         WHERE thread_id = ?1
         ORDER BY first_seen_at ASC",
    )?;
    let rows = statement.query_map([thread_id], query_group_summary_from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn query_group_summary_from_row(row: &Row<'_>) -> rusqlite::Result<InsightQueryGroupSummary> {
    Ok(InsightQueryGroupSummary {
        query_group_id: row.get(0)?,
        profile_id: row.get(1)?,
        thread_id: row.get(2)?,
        title: row.get(3)?,
        root_query: row.get(4)?,
        latest_query: row.get(5)?,
        first_seen_at: row.get(6)?,
        last_seen_at: row.get(7)?,
        visit_count: row.get::<_, i64>(8)?.max(0) as usize,
        burst_count: row.get::<_, i64>(9)?.max(0) as usize,
        step_count: row.get::<_, i64>(10)?.max(0) as usize,
        confidence: row.get(11)?,
        evidence_tier: row.get(12)?,
        chromium_enhanced: row.get::<_, i64>(13)? != 0,
        steps: serde_json::from_str(&row.get::<_, String>(14)?).unwrap_or_default(),
        stages: serde_json::from_str(&row.get::<_, String>(15)?).unwrap_or_default(),
        evidence: serde_json::from_str(&row.get::<_, String>(16)?).unwrap_or_default(),
    })
}

fn day_key(visited_at: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(visited_at).ok().map(|value| value.format("%Y-%m-%d").to_string())
}

fn fxhash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
