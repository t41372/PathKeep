//! Failure modes: split-day uniqueness, first-domain counts or category ties change totals.
//! Daily-rollup accumulation helpers.
//!
//! ## Responsibilities
//! - Accumulate per-day domain/category/engine/summary rollup rows from
//!   normalized visit records.
//! - Keep the pure in-memory rollup math out of stage execution and SQLite
//!   fallback queries.
//! - Merge per-profile rollup bundles during scoped legacy rebuilds.
//!
//! ## Not responsible for
//! - Running checkpoint-aware daily-rollup stages.
//! - Loading persisted visit-derived rows from SQLite.
//! - Replacing rollup rows in the derived intelligence database.
//!
//! ## Dependencies
//! - Parent-module `VisitRecord`, `DailyRollupBundle`, and local date helper.
//! - Standard collection types for bounded aggregation state.
//!
//! ## Performance notes
//! - Accumulation is linear in the provided visit slice.
//! - The accumulator only stores one row per `(date, profile, key)` bucket and
//!   never touches SQLite.

use super::{DailyRollupBundle, VisitRecord, local_datetime_from_millis};
use chrono::Timelike;
use std::collections::{HashMap, HashSet};

type DailyDomainKey = (String, String, String);
type DailyDomainValue = (String, i64, i64, i64, HashSet<String>, i64);
type DailyCategoryKey = (String, String, String);
type DailyCategoryValue = (i64, HashSet<String>);
type DailyEngineKey = (String, String, String);
type DailySummaryKey = (String, String);
type DailySummaryValue =
    (i64, i64, HashSet<String>, HashSet<String>, HashMap<String, i64>, [i64; 24]);

#[derive(Debug, Default)]
pub(super) struct DailyRollupAccumulator {
    domains: HashMap<DailyDomainKey, DailyDomainValue>,
    categories: HashMap<DailyCategoryKey, DailyCategoryValue>,
    engines: HashMap<DailyEngineKey, i64>,
    summaries: HashMap<DailySummaryKey, DailySummaryValue>,
}

impl DailyRollupAccumulator {
    pub(super) fn add_visit(&mut self, visit: &VisitRecord) {
        // One local-time conversion per visit gives both the day and the hour.
        let local = local_datetime_from_millis(visit.visit_time_ms);
        let date_key = local.format("%Y-%m-%d").to_string();
        let domain_key =
            (date_key.clone(), visit.profile_id.clone(), visit.registrable_domain.clone());
        let domain_entry = self.domains.entry(domain_key).or_insert((
            visit.domain_category.clone(),
            0,
            0,
            0,
            HashSet::new(),
            0,
        ));
        domain_entry.5 = domain_entry.5.max(visit.visit_time_ms);
        if visit.domain_category < domain_entry.0 {
            domain_entry.0 = visit.domain_category.clone();
        }
        domain_entry.1 += 1;
        domain_entry.2 += i64::from(visit.is_search_event);
        domain_entry.3 += i64::from(visit.is_new_domain);
        domain_entry.4.insert(visit.canonical_url.clone());

        let category_key =
            (date_key.clone(), visit.profile_id.clone(), visit.domain_category.clone());
        let category_entry = self.categories.entry(category_key).or_insert((0, HashSet::new()));
        category_entry.0 += 1;
        category_entry.1.insert(visit.registrable_domain.clone());

        if let Some(engine) = &visit.search_engine {
            *self
                .engines
                .entry((date_key.clone(), visit.profile_id.clone(), engine.clone()))
                .or_default() += 1;
        }

        let summary_key = (date_key, visit.profile_id.clone());
        let summary_entry = self.summaries.entry(summary_key).or_insert((
            0,
            0,
            HashSet::new(),
            HashSet::new(),
            HashMap::new(),
            [0; 24],
        ));
        summary_entry.0 += 1;
        summary_entry.5[local.hour() as usize] += 1;
        summary_entry.1 += i64::from(visit.is_search_event);
        if visit.is_new_domain {
            summary_entry.2.insert(visit.registrable_domain.clone());
        }
        summary_entry.3.insert(visit.registrable_domain.clone());
        *summary_entry.4.entry(visit.registrable_domain.clone()).or_default() += 1;
    }

    pub(super) fn finish(self) -> DailyRollupBundle {
        DailyRollupBundle {
            domain_rows: self
                .domains
                .into_iter()
                .map(
                    |(
                        (date_key, profile_id, registrable_domain),
                        (
                            domain_category,
                            visit_count,
                            search_count,
                            new_domain_visits,
                            unique_urls,
                            last_visit_ms,
                        ),
                    )| super::DomainDayRow {
                        date_key,
                        profile_id,
                        registrable_domain,
                        domain_category,
                        visit_count,
                        search_count,
                        new_domain_visits,
                        unique_urls: unique_urls.len() as i64,
                        last_visit_ms,
                    },
                )
                .collect(),
            category_rows: self
                .categories
                .into_iter()
                .map(|((date_key, profile_id, domain_category), (visit_count, unique_domains))| {
                    (
                        date_key,
                        profile_id,
                        domain_category,
                        visit_count,
                        unique_domains.len() as i64,
                    )
                })
                .collect(),
            engine_rows: self
                .engines
                .into_iter()
                .map(|((date_key, profile_id, search_engine), search_count)| {
                    (date_key, profile_id, search_engine, search_count)
                })
                .collect(),
            summary_rows: self
                .summaries
                .into_iter()
                .map(
                    |(
                        (date_key, profile_id),
                        (
                            total_visits,
                            total_searches,
                            new_domains,
                            unique_domains,
                            domain_counts,
                            hours,
                        ),
                    )| {
                        let hhi = domain_counts
                            .values()
                            .map(|count| {
                                let share = *count as f32 / total_visits as f32;
                                share * share
                            })
                            .sum::<f32>();
                        let discovery_rate = new_domains.len() as f32 / total_visits as f32;
                        super::DaySummaryRow {
                            date_key,
                            profile_id,
                            total_visits,
                            total_searches,
                            new_domains: new_domains.len() as i64,
                            unique_domains: unique_domains.len() as i64,
                            hhi_score: hhi,
                            discovery_rate,
                            hours: hours.to_vec(),
                        }
                    },
                )
                .collect(),
        }
    }
}
