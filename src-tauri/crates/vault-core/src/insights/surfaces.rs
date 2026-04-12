//! Shell-facing deterministic-insight surface builders.

use super::{
    VisitRecord, canonical_visit_key, evidence_from_visit, is_chromium_profile, url_domain,
};
use crate::{
    deterministic::EvidenceTier,
    models::{
        InsightCard, InsightQueryGroupSummary, InsightReferencePageSummary,
        InsightSourceEffectivenessSummary, InsightTemplateSummary, InsightThreadSummary,
    },
};
use std::collections::{HashMap, HashSet};

use super::shared::{day_key, fxhash, strongest_evidence_tier, visit_is_query_landing};

#[derive(Debug, Clone, Default)]
pub(super) struct ContrastWindowSummary {
    pub previous_visit_count: usize,
    pub previous_unique_domains: usize,
}

fn summary_window_label(window_days: u32) -> &'static str {
    match window_days {
        0..=7 => "last 7 days",
        8..=31 => "last 30 days",
        32..=92 => "last 90 days",
        93..=180 => "last 180 days",
        _ => "last 365 days",
    }
}

fn periodic_summary_evidence(
    visits: &[VisitRecord],
    threads: &[InsightThreadSummary],
    query_groups: &[InsightQueryGroupSummary],
) -> Vec<super::InsightEvidenceItem> {
    if let Some(thread) = threads.first() {
        return thread.evidence.clone();
    }
    if let Some(group) = query_groups.first() {
        return group.evidence.clone();
    }
    visits
        .iter()
        .rev()
        .take(4)
        .map(|visit| {
            evidence_from_visit(visit, Some("Recent activity in the current window.".to_string()))
        })
        .collect()
}

fn build_periodic_summary(
    visits: &[VisitRecord],
    query_groups: &[InsightQueryGroupSummary],
    threads: &[InsightThreadSummary],
    reference_pages: &[InsightReferencePageSummary],
    profile_id: Option<&str>,
    window_days: u32,
) -> Option<InsightTemplateSummary> {
    let primary_focus = threads
        .first()
        .map(|thread| thread.title.clone())
        .or_else(|| query_groups.first().map(|group| group.title.clone()))
        .or_else(|| {
            reference_pages
                .first()
                .map(|page| page.title.clone().unwrap_or_else(|| page.url.clone()))
        })?;
    let unique_domains =
        visits.iter().map(|visit| visit.registrable_domain.clone()).collect::<HashSet<_>>().len();
    Some(InsightTemplateSummary {
        summary_id: "summary-periodic".to_string(),
        kind: "periodic-summary".to_string(),
        title: "Periodic summary".to_string(),
        body: format!(
            "Over the {} this scope centered on \"{}\" across {} visits, {} domains, and {} query groups.",
            summary_window_label(window_days),
            primary_focus,
            visits.len(),
            unique_domains,
            query_groups.len()
        ),
        confidence: (0.36 + visits.len().min(24) as f32 * 0.02).clamp(0.0, 0.92),
        profile_id: profile_id.map(ToString::to_string),
        evidence: periodic_summary_evidence(visits, threads, query_groups),
    })
}

fn build_contrastive_summary(
    visits: &[VisitRecord],
    contrast: Option<&ContrastWindowSummary>,
    profile_id: Option<&str>,
    window_days: u32,
) -> Option<InsightTemplateSummary> {
    let contrast = contrast?;
    let current_unique_domains =
        visits.iter().map(|visit| visit.registrable_domain.clone()).collect::<HashSet<_>>().len();
    let evidence = visits
        .iter()
        .rev()
        .take(4)
        .map(|visit| {
            evidence_from_visit(
                visit,
                Some("Current-window evidence used for deterministic contrast.".to_string()),
            )
        })
        .collect::<Vec<_>>();
    let body = if contrast.previous_visit_count == 0 {
        format!(
            "Compared with the previous {} there was no earlier activity in scope; this window opened with {} visits across {} domains.",
            summary_window_label(window_days),
            visits.len(),
            current_unique_domains
        )
    } else {
        let visit_delta = visits.len() as i64 - contrast.previous_visit_count as i64;
        let direction = if visit_delta > 0 {
            "increased"
        } else if visit_delta < 0 {
            "decreased"
        } else {
            "held steady"
        };
        format!(
            "Compared with the previous {} activity {} from {} to {} visits, while domain breadth shifted from {} to {}.",
            summary_window_label(window_days),
            direction,
            contrast.previous_visit_count,
            visits.len(),
            contrast.previous_unique_domains,
            current_unique_domains
        )
    };
    Some(InsightTemplateSummary {
        summary_id: "summary-contrastive".to_string(),
        kind: "contrastive-summary".to_string(),
        title: "Contrastive summary".to_string(),
        body,
        confidence: 0.58,
        profile_id: profile_id.map(ToString::to_string),
        evidence,
    })
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
                domain: if sample.registrable_domain.trim().is_empty() {
                    url_domain(&sample.url)
                } else {
                    sample.registrable_domain.clone()
                },
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
    window_days: u32,
    contrast: Option<&ContrastWindowSummary>,
) -> Vec<InsightTemplateSummary> {
    let mut summaries = Vec::new();
    if let Some(summary) = build_periodic_summary(
        visits,
        query_groups,
        threads,
        reference_pages,
        profile_id,
        window_days,
    ) {
        summaries.push(summary);
    }
    if let Some(summary) = build_contrastive_summary(visits, contrast, profile_id, window_days) {
        summaries.push(summary);
    }
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
