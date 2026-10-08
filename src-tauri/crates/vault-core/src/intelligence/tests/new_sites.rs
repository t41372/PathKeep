//! New sites in a range (the digest's "New sites" and the discovery trend).
//!
//! A site is new on the first day it appears: in any browser profile for the whole archive, or
//! in the one profile asked for.
//!
//! Failure modes: summing each profile's own "new" counts counts a site once per profile, so the
//! whole archive reported more new sites than it had sites at all (5,244 against 4,115 on a real
//! archive); a site first seen before the range counts; the first and last day of the range are
//! dropped; the trend's days disagree with the digest's total.

use super::super::{get_digest_summary, get_discovery_trend};
use crate::models::{DateRange, GranularityDateRangeRequest, ScopedDateRangeRequest};
use vault_store::{Db, Priority};

fn seed(vault: &crate::Vault) {
    vault
        .write(Db::Intelligence, Priority::Interactive, |s| {
            for profile in ["a", "b"] {
                s.execute(
                    "INSERT INTO intelligence_generations(profile_id, daily) VALUES (?1, 1)",
                    [profile],
                )?;
            }
            // (day, profile, site, visits that were the site's first in that profile)
            let rows = [
                ("2026-03-31", "a", "z.com", 1), // before the range
                ("2026-04-01", "a", "x.com", 1), // the range's first day
                ("2026-04-03", "b", "y.com", 1),
                ("2026-04-05", "b", "x.com", 1), // new to b, not to the archive
                ("2026-04-10", "a", "z.com", 0),
                ("2026-04-30", "b", "w.com", 1), // the range's last day
            ];
            for (day, profile, site, new) in rows {
                s.execute(
                    "INSERT INTO domain_daily_rollups_rows(generation, date_key, profile_id,
                       registrable_domain, domain_category, visit_count, search_count,
                       new_domain_visits, unique_urls)
                     VALUES (1, ?1, ?2, ?3, 'other', 2, 0, ?4, 1)",
                    vault_store::params![day, profile, site, new],
                )?;
                // What each profile's daily summary says on its own.
                s.execute(
                    "INSERT INTO daily_summary_rollups_rows(generation, date_key, profile_id,
                       total_visits, total_searches, new_domains, unique_domains)
                     VALUES (1, ?1, ?2, 2, 0, ?3, 1)",
                    vault_store::params![day, profile, new],
                )?;
            }
            Ok(())
        })
        .unwrap();
}

fn april() -> DateRange {
    DateRange { start: "2026-04-01".into(), end: "2026-04-30".into() }
}

fn new_sites(vault: &crate::Vault, profile: Option<&str>) -> (i64, i64) {
    let digest = get_digest_summary(
        vault,
        &ScopedDateRangeRequest { date_range: april(), profile_id: profile.map(Into::into) },
    )
    .unwrap();
    (digest.new_domains.value, digest.distinct_domains.value)
}

#[test]
fn a_site_is_new_once_on_its_first_day_in_any_profile() {
    let vault = crate::Vault::open_temp().unwrap();
    seed(&vault);
    assert_eq!(new_sites(&vault, None), (3, 4), "x, y and w are new; z was seen in March");
    assert_eq!(new_sites(&vault, Some("b")), (3, 3), "x, y and w are new to b");
    assert_eq!(new_sites(&vault, Some("a")), (1, 2), "only x is new to a");

    let trend = get_discovery_trend(
        &vault,
        &GranularityDateRangeRequest {
            date_range: april(),
            profile_id: None,
            granularity: "day".into(),
        },
    )
    .unwrap();
    let days: Vec<(String, i64)> = trend
        .points
        .iter()
        .filter(|point| point.new_domain_count > 0)
        .map(|point| (point.date_key.clone(), point.new_domain_count))
        .collect();
    assert_eq!(
        days,
        vec![
            ("2026-04-01".to_string(), 1),
            ("2026-04-03".to_string(), 1),
            ("2026-04-30".to_string(), 1),
        ],
        "x counts on April 1 only, not again when b first visits it",
    );
}
