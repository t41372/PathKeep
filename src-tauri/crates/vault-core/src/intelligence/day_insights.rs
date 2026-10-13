//! Responsibilities: compose exact local-calendar-day read models.
//! Not responsible for: rebuilds or frontend rendering.
//! Failure modes: reads write schemas, a query escapes its owner, stale scopes leak,
//! or bounded results lose continuation. Native render-path and paging tests cover these.
//! Focused read model for exact local-calendar-day insights.
//!
//! This module exists so the new day entity surface does not add yet another
//! large composition block to `intelligence/mod.rs`. It deliberately reuses the
//! accepted Core Intelligence read models and only composes them into one
//! day-first payload for the frontend.

use crate::models::{
    CategoryFilteredDateRangeRequest, DateRange, DayInsights, DayInsightsDrilldown,
    DayInsightsHourlyBucket, DayInsightsRequest, PagedDateRangeRequest, ScopedDateRangeRequest,
    TopSitesRequest,
};
use anyhow::{Context, Result};
use chrono::NaiveDate;

use super::{
    get_activity_mix, get_browsing_rhythm, get_digest_summary, get_query_families, get_top_sites,
};

/// Loads the exact-day deterministic read model used by `/intelligence/day/:date`.
pub fn get_day_insights(vault: &crate::Vault, request: &DayInsightsRequest) -> Result<DayInsights> {
    let date_range = exact_day_range(&request.date)?;
    let scoped_request = ScopedDateRangeRequest {
        date_range: date_range.clone(),
        profile_id: request.profile_id.clone(),
    };
    let digest_summary = get_digest_summary(vault, &scoped_request)?;
    let top_sites = get_top_sites(
        vault,
        &TopSitesRequest {
            cursor: None,
            date_range: date_range.clone(),
            profile_id: request.profile_id.clone(),
            profile_ids: Vec::new(),
            sort_by: Some("visit_count".to_string()),
            limit: Some(8),
        },
    )?;
    let activity_mix = get_activity_mix(vault, &scoped_request)?;
    let (range, profile) = (date_range.clone(), request.profile_id.clone());
    let refind_pages = vault.read(vault_store::Db::Intelligence, move |s| {
        super::intelligence_refind::get_opened_refind_pages_with_connection(
            s,
            &range,
            profile.as_deref(),
            5,
        )
    })?;
    let query_families = get_query_families(
        vault,
        &PagedDateRangeRequest {
            cursor: None,
            date_range: date_range.clone(),
            profile_id: request.profile_id.clone(),

            page_size: 8,
        },
    )?;
    let hourly_activity = get_exact_day_hourly_activity(vault, request, &date_range)?;

    Ok(DayInsights {
        date: request.date.clone(),
        digest_summary,
        top_sites,
        activity_mix,
        refind_pages,
        query_families,
        hourly_activity,
        drilldown: DayInsightsDrilldown { explorer_date_range: date_range },
    })
}

fn exact_day_range(date: &str) -> Result<DateRange> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .with_context(|| format!("invalid local calendar day '{date}'"))?;
    Ok(DateRange { start: date.to_string(), end: date.to_string() })
}

fn get_exact_day_hourly_activity(
    vault: &crate::Vault,
    request: &DayInsightsRequest,
    date_range: &DateRange,
) -> Result<Vec<DayInsightsHourlyBucket>> {
    get_browsing_rhythm(
        vault,
        &CategoryFilteredDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: request.profile_id.clone(),
            category: None,
        },
    )
    .map(hourly_activity_from_rhythm)
}

fn hourly_activity_from_rhythm(
    rhythm: crate::models::RhythmHeatmap,
) -> Vec<DayInsightsHourlyBucket> {
    let mut buckets = [0_i64; 24];
    for cell in rhythm.cells {
        if (0..24).contains(&cell.hour) {
            buckets[cell.hour as usize] += cell.visit_count;
        }
    }

    buckets
        .into_iter()
        .enumerate()
        .map(|(hour, visit_count)| DayInsightsHourlyBucket { hour: hour as i64, visit_count })
        .collect()
}
