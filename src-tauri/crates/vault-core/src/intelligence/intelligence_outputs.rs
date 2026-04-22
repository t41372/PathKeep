//! Export-facing Core Intelligence payload builders.
//!
//! ## Responsibilities
//! - Build embed-card, widget, and public-snapshot payloads from existing Core
//!   Intelligence read models.
//! - Keep export formatting separate from the underlying SQL read models so
//!   trusted and redacted surfaces can evolve independently.
//!
//! ## Not responsible for
//! - Recomputing intelligence facts.
//! - Route-shell overview composition.
//! - Entity explanation and host-artifact generation.
//!
//! ## Dependencies
//! - `intelligence_summary`, `intelligence_domain`, and parent-module top-site
//!   / refind helpers.
//! - Shared entity-reference models for link targets.
//!
//! ## Performance notes
//! - Output builders intentionally reuse already-aggregated read models instead
//!   of reaching back to visit-level tables.

use crate::{
    config::ProjectPaths,
    models::{
        AppConfig, InsightEntityReference, IntelligenceEmbedCardPayload,
        IntelligenceEmbedCardsRequest, IntelligencePublicSnapshot, IntelligenceWidgetSnapshot,
        RefindPagesRequest, ScopedDateRangeRequest, TopSitesRequest,
    },
    utils::now_rfc3339,
};
use anyhow::Result;

/// Produces compact highlight cards for trusted embeds without reopening new
/// query contracts beyond the existing read models.
pub fn get_intelligence_embed_cards(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<Vec<IntelligenceEmbedCardPayload>> {
    let scoped = ScopedDateRangeRequest {
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
    };
    let digest = super::intelligence_summary::get_digest_summary(paths, config, key, &scoped)?;
    let top_sites = super::get_top_sites(
        paths,
        config,
        key,
        &TopSitesRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            sort_by: Some("visit_count".to_string()),
            limit: Some(3),
        },
    )?;
    let refind_pages = super::get_refind_pages(
        paths,
        config,
        key,
        &RefindPagesRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            limit: Some(2),
        },
    )?;
    let stable_sources =
        super::intelligence_summary::get_stable_sources(paths, config, key, &scoped)?;
    let on_this_day = super::intelligence_domain::get_on_this_day(
        paths,
        config,
        key,
        request.profile_id.as_deref(),
    )?;
    let mut cards = vec![
        IntelligenceEmbedCardPayload {
            card_id: "digest:visits".to_string(),
            card_type: "digest".to_string(),
            title: "Visits".to_string(),
            eyebrow: Some(format!("{} → {}", request.date_range.start, request.date_range.end)),
            body: "Total visits in the selected intelligence window.".to_string(),
            metric_label: Some("visit_count".to_string()),
            metric_value: Some(digest.total_visits.value.to_string()),
            href: None,
            primary_target: None,
            secondary_targets: Vec::new(),
            internal_only: false,
        },
        IntelligenceEmbedCardPayload {
            card_id: "digest:searches".to_string(),
            card_type: "digest".to_string(),
            title: "Searches".to_string(),
            eyebrow: Some(format!("{} → {}", request.date_range.start, request.date_range.end)),
            body: "Total search events observed in the selected intelligence window.".to_string(),
            metric_label: Some("search_count".to_string()),
            metric_value: Some(digest.total_searches.value.to_string()),
            href: None,
            primary_target: None,
            secondary_targets: Vec::new(),
            internal_only: false,
        },
    ];
    if let Some(site) = top_sites.first() {
        cards.push(IntelligenceEmbedCardPayload {
            card_id: format!("top-site:{}", site.registrable_domain),
            card_type: "top_site".to_string(),
            title: site.display_name.clone().unwrap_or_else(|| site.registrable_domain.clone()),
            eyebrow: Some("Top site".to_string()),
            body: format!(
                "{} was one of the most frequently visited domains in this window.",
                site.registrable_domain
            ),
            metric_label: Some("visit_count".to_string()),
            metric_value: Some(site.visit_count.to_string()),
            href: Some(site.registrable_domain.clone()),
            primary_target: Some(InsightEntityReference::Domain {
                domain: site.registrable_domain.clone(),
            }),
            secondary_targets: Vec::new(),
            internal_only: false,
        });
    }
    if let Some(page) = refind_pages.first() {
        cards.push(IntelligenceEmbedCardPayload {
            card_id: format!("refind:{}", page.canonical_url),
            card_type: "refind_page".to_string(),
            title: page.title.clone().unwrap_or_else(|| page.registrable_domain.clone()),
            eyebrow: Some("Refind".to_string()),
            body: format!(
                "This page kept resurfacing across {} days and {} trails.",
                page.cross_day_count, page.trail_count
            ),
            metric_label: Some("refind_score".to_string()),
            metric_value: Some(format!("{:.2}", page.refind_score)),
            href: Some(page.canonical_url.clone()),
            primary_target: Some(InsightEntityReference::RefindPage {
                canonical_url: page.canonical_url.clone(),
            }),
            secondary_targets: vec![InsightEntityReference::Domain {
                domain: page.registrable_domain.clone(),
            }],
            internal_only: true,
        });
    }
    if let Some(source) = stable_sources.first() {
        cards.push(IntelligenceEmbedCardPayload {
            card_id: format!("stable-source:{}", source.registrable_domain),
            card_type: "stable_source".to_string(),
            title: source.display_name.clone().unwrap_or_else(|| source.registrable_domain.clone()),
            eyebrow: Some("Stable source".to_string()),
            body: format!(
                "{} often resolves trails as a {} source.",
                source.registrable_domain, source.source_role
            ),
            metric_label: Some("effectiveness_score".to_string()),
            metric_value: Some(format!("{:.2}", source.effectiveness_score)),
            href: None,
            primary_target: Some(InsightEntityReference::Domain {
                domain: source.registrable_domain.clone(),
            }),
            secondary_targets: Vec::new(),
            internal_only: false,
        });
    }
    if let Some(entry) = on_this_day.first() {
        cards.push(IntelligenceEmbedCardPayload {
            card_id: format!("on-this-day:{}", entry.year),
            card_type: "on_this_day".to_string(),
            title: format!("On This Day · {}", entry.year),
            eyebrow: Some(entry.date.clone()),
            body: entry.summary.clone().unwrap_or_else(|| {
                format!(
                    "{} visits and {} deep-dive sessions on this day.",
                    entry.total_visits, entry.deep_dive_sessions
                )
            }),
            metric_label: Some("visit_count".to_string()),
            metric_value: Some(entry.total_visits.to_string()),
            href: None,
            primary_target: Some(InsightEntityReference::Day { date: entry.date.clone() }),
            secondary_targets: entry
                .top_domains
                .iter()
                .take(2)
                .cloned()
                .map(|domain| InsightEntityReference::Domain { domain })
                .collect(),
            internal_only: false,
        });
    }
    cards.truncate(request.limit.unwrap_or(6).max(1) as usize);
    Ok(cards)
}

/// Produces the trusted widget snapshot that other local surfaces can cache or
/// render without exposing visit-level identifiers.
pub fn get_intelligence_widget_snapshot(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &IntelligenceEmbedCardsRequest,
) -> Result<IntelligenceWidgetSnapshot> {
    let digest_summary = super::intelligence_summary::get_digest_summary(
        paths,
        config,
        key,
        &ScopedDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
        },
    )?;
    let highlights = get_intelligence_embed_cards(
        paths,
        config,
        key,
        &IntelligenceEmbedCardsRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            limit: Some(request.limit.unwrap_or(4).min(4)),
        },
    )?;
    Ok(IntelligenceWidgetSnapshot {
        generated_at: now_rfc3339(),
        date_range: request.date_range.clone(),
        digest_summary,
        highlights,
        notes: vec![
            "Widget snapshots only expose aggregate Core Intelligence read models.".to_string(),
            "Cards marked internal_only should stay inside trusted PathKeep surfaces.".to_string(),
        ],
    })
}

/// Produces the redacted public snapshot that keeps only aggregate facts safe
/// for export outside trusted local surfaces.
pub fn get_intelligence_public_snapshot(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &ScopedDateRangeRequest,
) -> Result<IntelligencePublicSnapshot> {
    let digest_summary =
        super::intelligence_summary::get_digest_summary(paths, config, key, request)?;
    let top_domains = super::get_top_sites(
        paths,
        config,
        key,
        &TopSitesRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            sort_by: Some("visit_count".to_string()),
            limit: Some(5),
        },
    )?
    .into_iter()
    .map(|site| site.display_name.unwrap_or(site.registrable_domain))
    .collect::<Vec<_>>();
    let search_engines = super::get_search_engine_ranking(paths, config, key, request)?;
    let discovery_trend = super::intelligence_domain::get_discovery_trend(
        paths,
        config,
        key,
        &crate::models::GranularityDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            granularity: "week".to_string(),
        },
    )?;
    Ok(IntelligencePublicSnapshot {
        generated_at: now_rfc3339(),
        date_range: request.date_range.clone(),
        digest_summary,
        top_domains,
        search_engines,
        discovery_trend,
        notes: vec![
            "Public snapshots intentionally omit visit-level identifiers and direct page URLs."
                .to_string(),
            "Use trusted internal Core Intelligence surfaces for entity-level drilldown."
                .to_string(),
        ],
    })
}
