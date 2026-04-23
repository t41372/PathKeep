//! Visit evidence scoring and taxonomy classification.
//!
//! ## Responsibilities
//! - Combine normalized URLs, canonical search facts, titles, and overrides into
//!   one deterministic visit analysis result.
//! - Apply user overrides before static taxonomy packs.
//! - Keep rule matching separate from the rule-pack definitions themselves.
//!
//! ## Not responsible for
//! - Parsing browser history rows from SQLite.
//! - Persisting derived facts or runtime status.
//! - Fetching optional AI/provider classifications.
//!
//! ## Dependencies
//! - URL helpers for canonical host/path/query interpretation.
//! - Text helpers for title/query normalization and evidence tokenization.
//! - Static rule packs from `rules`.
//!
//! ## Performance notes
//! - Classification is per visit and bounded by the static rule-pack size.
//! - No archive-wide state is retained; callers can safely run this inside
//!   streamed rebuild batches.

use super::{
    rules::{HostPathRule, RULE_PACKS, TaxonomyRule, TaxonomyRulePack},
    text::{normalize_whitespace, tokenize_text},
    types::{
        DeterministicVisitAnalysis, EvidenceTier, NormalizedVisitUrl, TAXONOMY_VERSION,
        TaxonomyClassification, TaxonomyDecisionSource, TaxonomyOverride, TaxonomyOverrideTarget,
        VisitAnalysisInput, VisitEvidenceAssessment,
    },
    url::{host_matches_suffix, normalize_visit_url},
};

/// Produces deterministic evidence and taxonomy analysis for one visit.
pub fn analyze_visit(
    input: VisitAnalysisInput<'_>,
    overrides: &[TaxonomyOverride],
) -> DeterministicVisitAnalysis {
    let normalized_url = normalize_visit_url(input.url);
    let query = input
        .query
        .map(normalize_whitespace)
        .filter(|value| !value.is_empty())
        .or_else(|| normalized_url.as_ref().and_then(|value| value.search_query.clone()));
    let evidence = assess_visit_evidence(
        normalized_url.as_ref(),
        input.title,
        input.has_canonical_search_term,
        input.external_referrer_url,
        input.from_visit,
    );
    let taxonomy = classify_visit_taxonomy(
        normalized_url.as_ref(),
        input.url,
        input.title,
        query.as_deref(),
        overrides,
    );
    DeterministicVisitAnalysis { normalized_url, evidence, taxonomy }
}

/// Scores the deterministic evidence strength for one normalized visit.
fn assess_visit_evidence(
    normalized_url: Option<&NormalizedVisitUrl>,
    title: Option<&str>,
    has_canonical_search_term: bool,
    external_referrer_url: Option<&str>,
    from_visit: Option<i64>,
) -> VisitEvidenceAssessment {
    let mut tier_a_reasons = Vec::new();
    if has_canonical_search_term {
        tier_a_reasons.push("canonical-search-term".to_string());
    }
    if normalized_url.is_some_and(|value| value.is_search_results && value.search_query.is_some()) {
        tier_a_reasons.push("search-result-url".to_string());
    }
    if external_referrer_url.is_some() || from_visit.is_some() {
        tier_a_reasons.push("navigation-anchor".to_string());
    }
    if !tier_a_reasons.is_empty() {
        return VisitEvidenceAssessment { tier: EvidenceTier::TierA, reasons: tier_a_reasons };
    }

    let mut tier_b_reasons = Vec::new();
    if normalized_url.is_some_and(|value| value.path != "/") {
        tier_b_reasons.push("normalized-path".to_string());
    }
    if normalized_url.is_some_and(|value| !value.preserved_query.is_empty()) {
        tier_b_reasons.push("semantic-query-params".to_string());
    }
    if title.is_some_and(|value| !tokenize_text(value).is_empty()) {
        tier_b_reasons.push("title-tokens".to_string());
    }
    if !tier_b_reasons.is_empty() {
        return VisitEvidenceAssessment { tier: EvidenceTier::TierB, reasons: tier_b_reasons };
    }

    VisitEvidenceAssessment {
        tier: EvidenceTier::TierC,
        reasons: vec!["time-adjacency-only".to_string()],
    }
}

/// Applies override, exact-domain, host/path, and lexicon rules in priority order.
fn classify_visit_taxonomy(
    normalized_url: Option<&NormalizedVisitUrl>,
    raw_url: &str,
    title: Option<&str>,
    query: Option<&str>,
    overrides: &[TaxonomyOverride],
) -> TaxonomyClassification {
    if let Some(classification) = match_taxonomy_override(normalized_url, raw_url, overrides) {
        return classification;
    }

    let fallback_domain =
        normalized_url.map(|value| value.registrable_domain.as_str()).unwrap_or_default();
    let title_and_query = normalize_whitespace(
        &[
            title.unwrap_or_default(),
            query.unwrap_or_default(),
            normalized_url.and_then(|value| value.search_query.as_deref()).unwrap_or_default(),
        ]
        .join(" "),
    )
    .to_lowercase();

    for pack in RULE_PACKS {
        for rule in pack.exact_domains {
            if fallback_domain == rule.domain {
                return classification_from_rule(
                    pack,
                    rule.rule,
                    TaxonomyDecisionSource::ExactDomainRule,
                    0.95,
                    vec![format!("exact-domain={}", rule.domain)],
                );
            }
        }
    }

    if let Some(normalized_url) = normalized_url {
        for pack in RULE_PACKS {
            for rule in pack.host_path_rules {
                if matches_host_path_rule(normalized_url, rule) {
                    return classification_from_rule(
                        pack,
                        rule.rule,
                        TaxonomyDecisionSource::HostPathRule,
                        0.86,
                        vec![
                            format!("host={}", normalized_url.host),
                            format!("path={}", normalized_url.path),
                        ],
                    );
                }
            }
        }
    }

    for pack in RULE_PACKS {
        for rule in pack.lexicon_rules {
            if rule.tokens.iter().any(|token| title_and_query.contains(token)) {
                return classification_from_rule(
                    pack,
                    rule.rule,
                    TaxonomyDecisionSource::LexiconRule,
                    0.62,
                    vec![format!("lexicon={}", rule.tokens.join("|"))],
                );
            }
        }
    }

    TaxonomyClassification {
        reasons: if fallback_domain.is_empty() {
            vec!["unknown-fallback".to_string(), "invalid-or-empty-domain".to_string()]
        } else {
            vec![format!("unknown-fallback:{}", fallback_domain)]
        },
        ..TaxonomyClassification::default()
    }
}

/// Converts a matched static rule into the exported classification payload.
fn classification_from_rule(
    pack: &TaxonomyRulePack,
    rule: TaxonomyRule,
    source: TaxonomyDecisionSource,
    confidence: f32,
    mut reasons: Vec<String>,
) -> TaxonomyClassification {
    reasons.push(format!("rule-pack={}", pack.id));
    reasons.push(format!("rule-id={}", rule.id));
    TaxonomyClassification {
        domain_category: rule.domain_category,
        page_category: rule.page_category,
        interaction_kind: rule.interaction_kind,
        source,
        confidence,
        rule_pack: Some(pack.id.to_string()),
        rule_id: Some(rule.id.to_string()),
        version: format!("{}:{}", TAXONOMY_VERSION, pack.version),
        reasons,
    }
}

/// Applies user-provided override rules before any built-in taxonomy pack.
fn match_taxonomy_override(
    normalized_url: Option<&NormalizedVisitUrl>,
    raw_url: &str,
    overrides: &[TaxonomyOverride],
) -> Option<TaxonomyClassification> {
    for override_rule in overrides {
        let matches = match override_rule.target {
            TaxonomyOverrideTarget::ExactDomain => {
                normalized_url.is_some_and(|value| value.registrable_domain == override_rule.value)
            }
            TaxonomyOverrideTarget::Host => {
                normalized_url.is_some_and(|value| value.host == override_rule.value)
            }
            TaxonomyOverrideTarget::UrlPrefix => raw_url.starts_with(&override_rule.value),
        };
        if matches {
            let mut reasons = vec![format!("override-target={:?}", override_rule.target)];
            if let Some(note) = &override_rule.note {
                reasons.push(format!("override-note={note}"));
            }
            return Some(TaxonomyClassification {
                domain_category: override_rule.domain_category,
                page_category: override_rule.page_category,
                interaction_kind: override_rule.interaction_kind,
                source: TaxonomyDecisionSource::UserOverride,
                confidence: 1.0,
                rule_pack: Some("user-override".to_string()),
                rule_id: Some(override_rule.value.clone()),
                version: TAXONOMY_VERSION.to_string(),
                reasons,
            });
        }
    }
    None
}

/// Checks one host/path/query rule against the already-normalized URL fields.
fn matches_host_path_rule(normalized_url: &NormalizedVisitUrl, rule: &HostPathRule) -> bool {
    if !rule.host_suffixes.is_empty()
        && !rule
            .host_suffixes
            .iter()
            .any(|suffix| host_matches_suffix(&normalized_url.host, suffix))
    {
        return false;
    }

    let path = normalized_url.path.to_ascii_lowercase();
    if !rule.path_prefixes.is_empty()
        && !rule.path_prefixes.iter().any(|prefix| path.starts_with(prefix))
    {
        return false;
    }
    if !rule.path_contains.is_empty()
        && !rule.path_contains.iter().any(|needle| path.contains(needle))
    {
        return false;
    }
    if rule.path_segment_count_at_least > 0
        && path_segments(&path) < rule.path_segment_count_at_least
    {
        return false;
    }
    if !rule.query_keys.is_empty()
        && !normalized_url.preserved_query.iter().any(|(key, _)| {
            rule.query_keys.iter().any(|candidate| candidate.eq_ignore_ascii_case(key))
        })
    {
        return false;
    }
    if !rule.query_value_contains.is_empty()
        && !normalized_url.preserved_query.iter().any(|(_, value)| {
            let value = value.to_ascii_lowercase();
            rule.query_value_contains.iter().any(|candidate| value.contains(candidate))
        })
    {
        return false;
    }
    true
}

/// Counts non-empty path segments for rules that need route-like depth.
fn path_segments(path: &str) -> usize {
    path.split('/').filter(|segment| !segment.is_empty()).count()
}
