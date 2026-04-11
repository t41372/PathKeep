use super::VisitRecord;
use crate::deterministic::{EvidenceTier, PageCategory};
use chrono::DateTime;
use std::collections::HashSet;

pub(super) fn visit_token_set(visit: &VisitRecord) -> HashSet<String> {
    visit.keywords.iter().cloned().collect()
}

pub(super) fn strongest_evidence_tier(items: impl Iterator<Item = EvidenceTier>) -> String {
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

pub(super) fn visit_is_query_landing(visit: &VisitRecord) -> bool {
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

pub(super) fn day_key(visited_at: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(visited_at).ok().map(|value| value.format("%Y-%m-%d").to_string())
}

pub(super) fn fxhash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
