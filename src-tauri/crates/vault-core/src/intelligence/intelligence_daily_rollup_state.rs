//! Failure modes: split-day uniqueness, first-domain counts, or a site's category for the day
//! following one visit instead of most of them.
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

use super::{DailyRollupBundle, InsightsZone, VisitRecord};
use chrono::Timelike;
use std::collections::{HashMap, HashSet};

type DailyDomainKey = (String, String, String);
/// The visits per category, then visits, searches, first visits, pages and the last visit time.
type DailyDomainValue = (Vec<(String, i64)>, i64, i64, i64, HashSet<String>, i64);
type DailyCategoryKey = (String, String, String);
type DailyCategoryValue = (i64, HashSet<String>);
type DailyEngineKey = (String, String, String);
type DailySummaryKey = (String, String);
type DailySummaryValue =
    (i64, i64, HashSet<String>, HashSet<String>, HashMap<String, i64>, [i64; 24]);

#[derive(Debug)]
pub(super) struct DailyRollupAccumulator {
    /// The zone the days and hours are in (the build's).
    zone: InsightsZone,
    domains: HashMap<DailyDomainKey, DailyDomainValue>,
    categories: HashMap<DailyCategoryKey, DailyCategoryValue>,
    engines: HashMap<DailyEngineKey, i64>,
    summaries: HashMap<DailySummaryKey, DailySummaryValue>,
}

impl DailyRollupAccumulator {
    pub(super) fn new(zone: InsightsZone) -> Self {
        Self {
            zone,
            domains: HashMap::new(),
            categories: HashMap::new(),
            engines: HashMap::new(),
            summaries: HashMap::new(),
        }
    }

    pub(super) fn add_visit(&mut self, visit: &VisitRecord) {
        // One conversion per visit gives both the day and the hour.
        let local = self.zone.datetime(visit.visit_time_ms);
        let date_key = local.format("%Y-%m-%d").to_string();
        // A visit with no site (an extension page, a file) counts in the day's totals, not as a site.
        let site = (!visit.registrable_domain.is_empty()).then_some(&visit.registrable_domain);
        if let Some(site) = site {
            let domain_key = (date_key.clone(), visit.profile_id.clone(), site.clone());
            let domain_entry =
                self.domains.entry(domain_key).or_insert((Vec::new(), 0, 0, 0, HashSet::new(), 0));
            domain_entry.5 = domain_entry.5.max(visit.visit_time_ms);
            match domain_entry.0.iter_mut().find(|(category, _)| *category == visit.domain_category)
            {
                Some((_, count)) => *count += 1,
                None => domain_entry.0.push((visit.domain_category.clone(), 1)),
            }
            domain_entry.1 += 1;
            domain_entry.2 += i64::from(visit.is_search_event);
            domain_entry.3 += i64::from(visit.is_new_domain);
            domain_entry.4.insert(visit.canonical_url.clone());
        }

        let category_key =
            (date_key.clone(), visit.profile_id.clone(), visit.domain_category.clone());
        let category_entry = self.categories.entry(category_key).or_insert((0, HashSet::new()));
        category_entry.0 += 1;
        if let Some(site) = site {
            category_entry.1.insert(site.clone());
        }

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
        if let Some(site) = site {
            if visit.is_new_domain {
                summary_entry.2.insert(site.clone());
            }
            summary_entry.3.insert(site.clone());
            *summary_entry.4.entry(site.clone()).or_default() += 1;
        }
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
                            categories,
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
                        domain_category: site_category(categories),
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

/// A site's category for the day: the one most of its visits had (the site is a search engine
/// even on a day one of its pages counted as docs). A tie goes to a known category over `unknown`,
/// then to the first by name, so every rebuild picks the same one.
fn site_category(counts: Vec<(String, i64)>) -> String {
    counts
        .into_iter()
        .max_by(|(a, a_visits), (b, b_visits)| {
            a_visits
                .cmp(b_visits)
                .then_with(|| (a != "unknown").cmp(&(b != "unknown")))
                .then_with(|| b.cmp(a))
        })
        .map(|(category, _)| category)
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A site's category for a day goes wrong when one visit's category outweighs the rest (the
    // smallest name used to win, so one docs page made a day of searching "docs"), when unknown
    // wins a tie against a known category, or when a tie is broken differently on each rebuild.

    fn visit(visit_id: i64, category: &str) -> VisitRecord {
        VisitRecord {
            visit_id,
            profile_id: "chrome:Default".into(),
            source_profile_id: 1,
            source_visit_id: visit_id,
            source_url_id: visit_id,
            url: format!("https://google.com/{visit_id}"),
            title: None,
            visit_time_ms: 1_711_929_600_000 + visit_id,
            from_visit: None,
            transition_type: None,
            external_referrer_url: None,
            canonical_url: format!("https://google.com/{visit_id}"),
            registrable_domain: "google.com".into(),
            domain_category: category.into(),
            page_category: "unknown".into(),
            search_engine: None,
            search_query: None,
            is_new_domain: false,
            is_search_event: false,
            evidence_tier: "tier-c".into(),
            taxonomy_source: "rules".into(),
            taxonomy_pack: None,
            taxonomy_version: None,
            display_name: None,
            session_id: None,
            trail_id: None,
        }
    }

    fn category_of(categories: &[&str]) -> String {
        let mut day = DailyRollupAccumulator::new(InsightsZone::named("UTC").unwrap());
        for (id, category) in categories.iter().enumerate() {
            day.add_visit(&visit(id as i64, category));
        }
        let rows = day.finish().domain_rows;
        assert_eq!(rows.len(), 1);
        rows[0].domain_category.clone()
    }

    #[test]
    fn a_site_takes_the_category_most_of_its_visits_had() {
        assert_eq!(category_of(&["search", "docs", "search"]), "search");
        assert_eq!(category_of(&["work", "work", "ai"]), "work");
    }

    #[test]
    fn ties_go_to_a_known_category_then_the_first_by_name() {
        assert_eq!(category_of(&["unknown", "work"]), "work");
        assert_eq!(category_of(&["work", "unknown"]), "work");
        assert_eq!(category_of(&["work", "search"]), "search");
        assert_eq!(category_of(&["search", "work"]), "search");
        assert_eq!(category_of(&["unknown"]), "unknown");
    }
}
