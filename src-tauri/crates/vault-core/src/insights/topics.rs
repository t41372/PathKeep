//! Topic-summary helpers for deterministic insights.

use super::VisitRecord;
use crate::{
    deterministic::tokenize_text,
    models::{InsightQueryGroupSummary, InsightThreadSummary, InsightTopicSummary},
};
use std::collections::{HashMap, HashSet};

use super::shared::visit_token_set;
use super::{canonical_visit_key, token_similarity};

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
