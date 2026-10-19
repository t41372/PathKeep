//! Responsibilities: read a date range's site-day rollups once, in date order, and keep per site
//! what the cards that rank or count sites need: top sites, explore or focus (breadth), habits,
//! browsers compared and the number of sites in the KPI row. Keep the last few ranges read until
//! the Insights data changes.
//! Not responsible for: ranking or paging those cards (their own modules), classifying a habit,
//! or the category and summary rollups.
//! Failure modes: a range served after its data changed (each entry carries the content
//! fingerprint the all-time snapshot uses), memory growing with the days of a range instead of
//! its sites (a site keeps running totals, never its days), rows read out of date order (gaps
//! and distinct days would be wrong; the read orders by the index it walks), one profile's
//! numbers served for another (the profile set is part of the key).
//!
//! Each of those cards used to read the range's rows on its own: at 14.4M visits a year took
//! 15 to 32 s per card, the all-time habit read held every site's days (1.85 GB).
use super::intelligence_overview_snapshot::compute_overview_fingerprint;
use crate::models::DateRange;
use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use vault_store::{Session, Value};

/// Which rows: a date range, and the profiles to count (none listed: every profile).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SiteDayScope {
    pub(crate) start: String,
    pub(crate) end: String,
    profiles: Vec<String>,
}

impl SiteDayScope {
    pub(crate) fn new(range: &DateRange, profiles: impl IntoIterator<Item = String>) -> Self {
        let mut profiles = profiles.into_iter().collect::<Vec<_>>();
        profiles.sort();
        profiles.dedup();
        Self { start: range.start.clone(), end: range.end.clone(), profiles }
    }

    /// One profile, or every profile when `None`.
    pub(crate) fn of(range: &DateRange, profile: Option<&str>) -> Self {
        Self::new(range, profile.map(str::to_owned))
    }
}

/// The days a site was visited on, as running totals: enough to tell a habit (how many days,
/// the first and last, and how regular the gaps are) without keeping the days.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct VisitDays {
    pub(crate) count: i64,
    pub(crate) first: i32,
    pub(crate) last: i32,
    gap_squares: f64,
}

impl VisitDays {
    /// Adds a day not earlier than the last one added; the same day again changes nothing.
    pub(crate) fn add(&mut self, day: i32) {
        if self.count > 0 && day == self.last {
            return;
        }
        if self.count == 0 {
            self.first = day;
        } else {
            let gap = f64::from(day - self.last);
            self.gap_squares += gap * gap;
        }
        self.last = day;
        self.count += 1;
    }

    /// The mean of the gaps between consecutive days and their variance; `None` for one day.
    pub(crate) fn gaps(&self) -> Option<(f64, f64)> {
        let gaps = (self.count - 1) as f64;
        (self.count > 1).then(|| {
            let mean = f64::from(self.last - self.first) / gaps;
            (mean, (self.gap_squares / gaps - mean * mean).max(0.0))
        })
    }

    pub(crate) fn last_date(&self) -> Option<NaiveDate> {
        NaiveDate::from_num_days_from_ce_opt(self.last)
    }
}

/// One site over the scope's rows.
#[derive(Debug, Clone, Default)]
pub(crate) struct SiteDay {
    pub(crate) domain: String,
    /// Every visit, searches included (breadth, browsers compared).
    pub(crate) visits: i64,
    /// Visits that were not searches, the days they fell on, the pages they opened and the
    /// site's category, counted from the rows with any (top sites: a search result page is a
    /// search, not a visit to the site).
    pub(crate) page_visits: i64,
    pub(crate) page_days: i64,
    pub(crate) page_urls: i64,
    pub(crate) category: Option<String>,
    /// The days with visits, and the last visit (habits).
    pub(crate) days: VisitDays,
    pub(crate) last_visit_ms: i64,
    /// Visits per profile, by index into `SiteDays::profiles`.
    pub(crate) per_profile: Vec<(usize, i64)>,
    last_page_day: Option<i32>,
}

/// The scope's sites and, per profile, its visits and number of sites.
#[derive(Debug, Default)]
pub(crate) struct SiteDays {
    pub(crate) sites: Vec<SiteDay>,
    pub(crate) profiles: Vec<ProfileSites>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProfileSites {
    pub(crate) profile_id: String,
    pub(crate) visits: i64,
    pub(crate) sites: i64,
}

impl SiteDays {
    /// Reads the scope's rows once, in date order, through the range index.
    pub(crate) fn read(s: &Session<'_>, scope: &SiteDayScope) -> Result<Self> {
        let profiles = if scope.profiles.is_empty() {
            String::new()
        } else {
            let marks = (0..scope.profiles.len()).map(|i| format!("?{}", i + 3));
            format!("AND r.profile_id IN ({})", marks.collect::<Vec<_>>().join(","))
        };
        let mut params: Vec<Value> = vec![scope.start.clone().into(), scope.end.clone().into()];
        params.extend(scope.profiles.iter().map(|id| Value::from(id.clone())));
        let mut statement = s.prepare(&format!(
            "SELECT r.date_key,r.profile_id,r.registrable_domain,r.domain_category,r.visit_count,
                    r.search_count,r.unique_urls,r.last_visit_ms
             FROM domain_daily_rollups_rows r INDEXED BY idx_domain_daily_range
             WHERE r.date_key>=?1 AND r.date_key<=?2
               AND r.generation IN (SELECT daily FROM intelligence_generations) {profiles}
             ORDER BY r.date_key"
        ))?;
        let rows = statement.query_map(params, |r| {
            Ok(Row {
                date_key: r.get(0)?,
                profile_id: r.get(1)?,
                domain: r.get(2)?,
                category: r.get(3)?,
                visits: r.get(4)?,
                searches: r.get(5)?,
                urls: r.get(6)?,
                last_visit_ms: r.get(7)?,
            })
        })?;
        let mut days = SiteDays::default();
        let mut sites = HashMap::<String, usize>::new();
        let mut profile_index = HashMap::<String, usize>::new();
        let mut day = (String::new(), 0_i32);
        for row in rows {
            let row = row?;
            if row.date_key != day.0 {
                day = (row.date_key.clone(), day_number(&row.date_key)?);
            }
            let profile = *profile_index.entry(row.profile_id.clone()).or_insert_with(|| {
                days.profiles
                    .push(ProfileSites { profile_id: row.profile_id, ..Default::default() });
                days.profiles.len() - 1
            });
            days.profiles[profile].visits += row.visits;
            let index = *sites.entry(row.domain.clone()).or_insert_with(|| {
                days.sites.push(SiteDay { domain: row.domain, ..Default::default() });
                days.sites.len() - 1
            });
            let site = &mut days.sites[index];
            site.visits += row.visits;
            match site.per_profile.iter_mut().find(|(id, _)| *id == profile) {
                Some((_, visits)) => *visits += row.visits,
                None => {
                    site.per_profile.push((profile, row.visits));
                    days.profiles[profile].sites += 1;
                }
            }
            if row.visits > row.searches {
                site.page_visits += row.visits - row.searches;
                site.page_urls += row.urls;
                if site.last_page_day != Some(day.1) {
                    site.last_page_day = Some(day.1);
                    site.page_days += 1;
                }
                if site.category.as_ref().is_none_or(|category| row.category < *category) {
                    site.category = Some(row.category);
                }
            }
            if row.visits > 0 {
                site.days.add(day.1);
                site.last_visit_ms = site.last_visit_ms.max(row.last_visit_ms);
            }
        }
        Ok(days)
    }
}

struct Row {
    date_key: String,
    profile_id: String,
    domain: String,
    category: String,
    visits: i64,
    searches: i64,
    urls: i64,
    last_visit_ms: i64,
}

fn day_number(date_key: &str) -> Result<i32> {
    Ok(NaiveDate::parse_from_str(date_key, "%Y-%m-%d")
        .with_context(|| format!("site-day rollup date {date_key}"))?
        .num_days_from_ce())
}

/// Ranges kept: the page's range, the one before it (the KPI row compares the two), a profile
/// or two, and History's Sites view.
const KEEP: usize = 8;

/// The last few scopes read, each with the Insights content fingerprint it was read under: a
/// card reading a scope another card just read (the first and second batch, a new top-sites
/// order, its next page, History's Sites view) takes it from here. A single day is read every
/// time instead (On this day, the day page): it is cheap, and would push longer ranges out.
#[derive(Default)]
pub(crate) struct SiteDayCache {
    entries: Mutex<VecDeque<(String, SiteDayScope, Arc<SiteDays>)>>,
}

impl SiteDayCache {
    pub(crate) fn get(&self, s: &Session<'_>, scope: &SiteDayScope) -> Result<Arc<SiteDays>> {
        if scope.start == scope.end {
            return Ok(Arc::new(SiteDays::read(s, scope)?));
        }
        let fingerprint = compute_overview_fingerprint(s, None)?;
        let found = self.lock().iter().find_map(|(print, kept, days)| {
            (*print == fingerprint && kept == scope).then(|| days.clone())
        });
        if let Some(days) = found {
            return Ok(days);
        }
        let days = Arc::new(SiteDays::read(s, scope)?);
        let mut entries = self.lock();
        entries.retain(|(print, kept, _)| *print == fingerprint && kept != scope);
        entries.push_front((fingerprint, scope.clone(), days.clone()));
        entries.truncate(KEEP);
        Ok(days)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<(String, SiteDayScope, Arc<SiteDays>)>> {
        self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
