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

use super::{DailyRollupBundle, VisitRecord, local_date_key};
use std::collections::{HashMap, HashSet};

type DailyDomainKey = (String, String, String);
type DailyDomainValue = (String, i64, i64, i64, HashSet<String>);
type DailyCategoryKey = (String, String, String);
type DailyCategoryValue = (i64, HashSet<String>);
type DailyEngineKey = (String, String, String);
type DailySummaryKey = (String, String);
type DailySummaryValue = (i64, i64, HashSet<String>, HashSet<String>, HashMap<String, i64>);

#[derive(Debug, Default)]
struct DailyRollupAccumulator {
    domains: HashMap<DailyDomainKey, DailyDomainValue>,
    categories: HashMap<DailyCategoryKey, DailyCategoryValue>,
    engines: HashMap<DailyEngineKey, i64>,
    summaries: HashMap<DailySummaryKey, DailySummaryValue>,
}

impl DailyRollupAccumulator {
    fn add_visit(&mut self, visit: &VisitRecord) {
        let date_key = local_date_key(visit.visit_time_ms);
        let domain_key =
            (date_key.clone(), visit.profile_id.clone(), visit.registrable_domain.clone());
        let domain_entry = self.domains.entry(domain_key).or_insert((
            visit.domain_category.clone(),
            0,
            0,
            0,
            HashSet::new(),
        ));
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
        ));
        summary_entry.0 += 1;
        summary_entry.1 += i64::from(visit.is_search_event);
        if visit.is_new_domain {
            summary_entry.2.insert(visit.registrable_domain.clone());
        }
        summary_entry.3.insert(visit.registrable_domain.clone());
        *summary_entry.4.entry(visit.registrable_domain.clone()).or_default() += 1;
    }

    fn extend<'a>(&mut self, visits: impl IntoIterator<Item = &'a VisitRecord>) {
        for visit in visits {
            self.add_visit(visit);
        }
    }

    fn finish(self) -> DailyRollupBundle {
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
                        ),
                    )| {
                        (
                            date_key,
                            profile_id,
                            registrable_domain,
                            domain_category,
                            visit_count,
                            search_count,
                            new_domain_visits,
                            unique_urls.len() as i64,
                        )
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
                        (total_visits, total_searches, new_domains, unique_domains, domain_counts),
                    )| {
                        let hhi = if total_visits == 0 {
                            0.0
                        } else {
                            domain_counts
                                .values()
                                .map(|count| {
                                    let share = *count as f32 / total_visits as f32;
                                    share * share
                                })
                                .sum::<f32>()
                        };
                        let discovery_rate = if total_visits == 0 {
                            0.0
                        } else {
                            new_domains.len() as f32 / total_visits as f32
                        };
                        (
                            date_key,
                            profile_id,
                            total_visits,
                            total_searches,
                            new_domains.len() as i64,
                            unique_domains.len() as i64,
                            hhi,
                            discovery_rate,
                        )
                    },
                )
                .collect(),
        }
    }
}

/// Builds daily rollups from an already loaded visit slice without touching any
/// later deterministic structures.
pub(super) fn build_daily_rollups(visits: &[VisitRecord]) -> DailyRollupBundle {
    let mut accumulator = DailyRollupAccumulator::default();
    accumulator.extend(visits.iter());
    accumulator.finish()
}

/// Merges per-profile rollup bundles into one full-rebuild aggregate without
/// changing any per-row semantics.
pub(super) fn merge_rollups(target: &mut DailyRollupBundle, next: DailyRollupBundle) {
    target.domain_rows.extend(next.domain_rows);
    target.category_rows.extend(next.category_rows);
    target.engine_rows.extend(next.engine_rows);
    target.summary_rows.extend(next.summary_rows);
}
