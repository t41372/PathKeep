//! Focused read model for exact local-calendar-day insights.
//!
//! This module exists so the new day entity surface does not add yet another
//! large composition block to `intelligence/mod.rs`. It deliberately reuses the
//! accepted Core Intelligence read models and only composes them into one
//! day-first payload for the frontend.

use crate::{
    config::ProjectPaths,
    models::{
        AppConfig, CategoryFilteredDateRangeRequest, DateRange, DayInsights, DayInsightsDrilldown,
        DayInsightsHourlyBucket, DayInsightsRequest, PagedDateRangeRequest, RefindPagesRequest,
        ScopedDateRangeRequest, TopSitesRequest,
    },
};
use anyhow::{Context, Result};
use chrono::NaiveDate;

use super::{
    get_activity_mix, get_browsing_rhythm, get_digest_summary, get_query_families,
    get_refind_pages, get_top_sites,
};

/// Loads the exact-day deterministic read model used by `/intelligence/day/:date`.
pub fn get_day_insights(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &DayInsightsRequest,
) -> Result<DayInsights> {
    let date_range = exact_day_range(&request.date)?;
    let scoped_request = ScopedDateRangeRequest {
        date_range: date_range.clone(),
        profile_id: request.profile_id.clone(),
    };
    let digest_summary = get_digest_summary(paths, config, key, &scoped_request)?;
    let top_sites = get_top_sites(
        paths,
        config,
        key,
        &TopSitesRequest {
            date_range: date_range.clone(),
            profile_id: request.profile_id.clone(),
            sort_by: Some("visit_count".to_string()),
            limit: Some(8),
        },
    )?;
    let activity_mix = get_activity_mix(paths, config, key, &scoped_request)?;
    let refind_pages = get_refind_pages(
        paths,
        config,
        key,
        &RefindPagesRequest {
            date_range: date_range.clone(),
            profile_id: request.profile_id.clone(),
            limit: Some(5),
        },
    )?;
    let query_families = get_query_families(
        paths,
        config,
        key,
        &PagedDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: request.profile_id.clone(),
            page: 0,
            page_size: 8,
        },
    )?;

    // TODO: M7 — `get_browsing_rhythm()` still exposes a generic weekly/hourly
    // bucket contract. Keep it wrapped here for exact-day hourly activity until
    // the lower-level API is renamed and split into clearer entity-first reads.
    let hourly_activity = get_browsing_rhythm(
        paths,
        config,
        key,
        &CategoryFilteredDateRangeRequest {
            date_range: date_range.clone(),
            profile_id: request.profile_id.clone(),
            category: None,
        },
    )
    .map(hourly_activity_from_rhythm)?;

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

#[cfg(test)]
mod tests {
    use super::{exact_day_range, hourly_activity_from_rhythm};
    use crate::models::{DayInsightsHourlyBucket, RhythmHeatmap, RhythmHeatmapCell};

    #[test]
    fn exact_day_range_rejects_invalid_calendar_days() {
        assert!(exact_day_range("2026-02-30").is_err());
        assert!(exact_day_range("not-a-date").is_err());
    }

    #[test]
    fn hourly_activity_folds_exact_day_rhythm_into_twenty_four_buckets() {
        let buckets = hourly_activity_from_rhythm(RhythmHeatmap {
            cells: vec![
                RhythmHeatmapCell { dow: 5, hour: 3, visit_count: 2 },
                RhythmHeatmapCell { dow: 5, hour: 3, visit_count: 1 },
                RhythmHeatmapCell { dow: 5, hour: 10, visit_count: 4 },
            ],
            max_count: 4,
        });

        assert_eq!(buckets.len(), 24);
        assert_eq!(buckets[3], DayInsightsHourlyBucket { hour: 3, visit_count: 3 });
        assert_eq!(buckets[10], DayInsightsHourlyBucket { hour: 10, visit_count: 4 });
        assert_eq!(buckets[0].visit_count, 0);
    }
}
