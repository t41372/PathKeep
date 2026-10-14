//! Top sites across several browser profiles (one browser can have several).
//!
//! Failure modes: summing per-profile pages on the client misses sites that
//! rank low in each profile but high together, and a client-side cap hides
//! the rest; profiles outside the filter leak in; paging repeats or skips a
//! site on a tie.

use super::super::get_top_sites;
use crate::models::{DateRange, TopSitesRequest};
use vault_store::{Db, Priority};

fn seed(vault: &crate::Vault) {
    vault
        .write(Db::Intelligence, Priority::Interactive, |s| {
            for profile in ["a", "b", "c"] {
                s.execute(
                    "INSERT INTO intelligence_generations(profile_id, daily) VALUES (?1, 1)",
                    [profile],
                )?;
            }
            // Search result pages are searches, not site visits: google.com has only those,
            // and two of x.com's visits in b were searches on it.
            for (profile, domain, visits, searches) in [
                ("a", "x.com", 5, 0),
                ("a", "y.com", 3, 0),
                ("a", "google.com", 50, 50),
                ("b", "x.com", 4, 2),
                ("b", "z.com", 7, 0),
                ("c", "y.com", 100, 0),
            ] {
                s.execute(
                    "INSERT INTO domain_daily_rollups_rows(generation, date_key, profile_id,
                       registrable_domain, domain_category, visit_count, search_count,
                       new_domain_visits, unique_urls)
                     VALUES (1, '2026-04-02', ?1, ?2, 'other', ?3, ?4, 0, 1)",
                    vault_store::params![profile, domain, visits, searches],
                )?;
            }
            Ok(())
        })
        .unwrap();
}

fn all_pages(vault: &crate::Vault, mut request: TopSitesRequest) -> Vec<(String, i64)> {
    let mut seen = Vec::new();
    loop {
        let page = get_top_sites(vault, &request).unwrap();
        seen.extend(
            page.items.iter().map(|site| (site.registrable_domain.clone(), site.visit_count)),
        );
        match page.next_cursor {
            Some(cursor) => request.cursor = Some(cursor),
            None => return seen,
        }
    }
}

fn request(profile_ids: &[&str]) -> TopSitesRequest {
    TopSitesRequest {
        cursor: None,
        date_range: DateRange { start: "2026-04-01".into(), end: "2026-04-30".into() },
        profile_id: None,
        profile_ids: profile_ids.iter().map(|id| id.to_string()).collect(),
        sort_by: None,
        limit: Some(1),
    }
}

#[test]
fn several_profiles_are_summed_and_paged_through_every_site_once_without_searches() {
    let vault = crate::Vault::open_temp().unwrap();
    seed(&vault);
    assert_eq!(
        all_pages(&vault, request(&["a", "b"])),
        vec![("x.com".into(), 7), ("z.com".into(), 7), ("y.com".into(), 3)],
        "a and b only, summed, ties in domain order",
    );
    assert_eq!(
        all_pages(&vault, request(&[])),
        vec![("y.com".into(), 103), ("x.com".into(), 7), ("z.com".into(), 7)],
        "no filter means every profile",
    );
    let mut single = request(&[]);
    single.profile_id = Some("c".into());
    assert_eq!(all_pages(&vault, single), vec![("y.com".into(), 100)]);
}
