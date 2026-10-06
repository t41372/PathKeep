//! Responsibilities: verify URL visibility, calendar buckets and query plans.
//! Not responsible for: ingestion.
//! Failure modes: profile loss, reverted rows counted, bad week bounds, unknown URL errors, full scans.
use super::*;
use crate::archive::source_stats::tests::seeded_archive;
use vault_store::{Db, Priority};

fn ms(day: NaiveDate, hour: u32) -> i64 {
    let local = day.and_hms_opt(hour, 0, 0).expect("time");
    Local.from_local_datetime(&local).earliest().expect("local").timestamp_millis()
}

#[test]
fn buckets_are_twelve_monday_weeks_ending_this_week() {
    for key in [None, Some([72; 32])] {
        let vault = seeded_archive(key);
        let today = NaiveDate::from_ymd_opt(2026, 10, 3).expect("date"); // Saturday
        let this_monday = NaiveDate::from_ymd_opt(2026, 9, 28).expect("monday");
        vault.store().write(Db::Archive, Priority::Batch, move |s| {
            let sql = format!(
                "DELETE FROM visits;
                 INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id, reverted_at) VALUES
                   (1, {a}, '', 1, 1, NULL),
                   (1, {b}, '', 1, 1, NULL),
                   (1, {c}, '', 1, 1, NULL),
                   (2, {d}, '', 2, 1, NULL),
                   (1, {e}, '', 1, 1, '2026-01-01T00:00:00Z');",
                a = ms(this_monday, 0),
                b = ms(today, 12),
                c = ms(this_monday - Duration::days(1), 23),
                d = ms(this_monday - Duration::weeks(11), 9),
                e = ms(today, 13),
            );
            for statement in sql.split(';').filter(|s| !s.trim().is_empty()) { s.execute(statement, ())?; }
            crate::archive::search_documents::refresh(s, 1)?;
            Ok(())
        }).expect("visits");

        let detail = vault
            .read(Db::Archive, move |s| url_detail_for_session(s, "https://example.com/a", today))
            .expect("detail");
        assert_eq!(detail.total_visits, 4);
        assert_eq!(detail.weekly_visits.len(), 12);
        assert_eq!(detail.weekly_visits[11].week_start, "2026-09-28");
        assert_eq!(detail.weekly_visits[0].week_start, "2026-07-13");
        let counts: Vec<i64> = detail.weekly_visits.iter().map(|week| week.visits).collect();
        assert_eq!(counts, vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2]);
        // Chrome recorded three of the four visible visits, so it is listed first.
        assert_eq!(detail.browsers, vec!["Google Chrome", "Firefox"]);
        assert_eq!(detail.title.as_deref(), Some("A"));
        assert_eq!(detail.domain, "example.com");
        assert!(detail.first_visit_at < detail.last_visit_at);
    }
}

#[test]
fn unknown_url_yields_an_empty_detail() {
    for key in [None, Some([72; 32])] {
        let vault = seeded_archive(key);
        let today = NaiveDate::from_ymd_opt(2026, 10, 3).expect("date");
        let detail = vault
            .read(Db::Archive, move |s| url_detail_for_session(s, "https://nowhere.test/x", today))
            .expect("detail");
        assert_eq!(detail.total_visits, 0);
        assert!(detail.browsers.is_empty() && detail.first_visit_at.is_none());
        assert_eq!(detail.weekly_visits.len(), 12);
        assert_eq!(detail.domain, "nowhere.test");
    }
}
#[test]
fn detail_queries_seek_indexes() {
    for key in [None, Some([72; 32])] {
        let vault = seeded_archive(key);
        vault
            .store()
            .read(Db::Archive, |s| {
                for (sql, args) in [
                    (
                        URL_ROWS_SQL,
                        vault_store::params!["https://example.com/a"]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_COUNTS_SQL,
                        vault_store::params!["https://example.com/a"]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_FIRST_SQL,
                        vault_store::params![1].into_iter().collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_LAST_SQL,
                        vault_store::params![1].into_iter().collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_RANGE_SQL,
                        vault_store::params![1, 0, i64::MAX]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_DOMAIN_SQL,
                        vault_store::params!["https://example.com/a"]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                ] {
                    let plan = s
                        .query(&format!("EXPLAIN QUERY PLAN {sql}"), args, |r| {
                            Ok(r.get::<String>(3)?)
                        })?
                        .join("\n");
                    assert!(plan.contains("SEARCH"), "{plan}");
                    assert!(!plan.contains("SCAN visits") && !plan.contains("SCAN urls"), "{plan}");
                }
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn multiple_observations_in_one_profile_do_not_multiply_cached_page_counts() {
    for key in [None, Some([72; 32])] {
        let vault = seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(3,1,'https://example.com/a','Variant',1500,'',1500,'',1,0,3)",())?;
            s.execute("INSERT INTO visits(url_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(3,1500,'',1,0)",())?;
            crate::archive::search_documents::refresh(s,1)?;
            Ok(())
        }).unwrap();
        assert_eq!(get_url_detail(&vault, "https://example.com/a").unwrap().total_visits, 4);
    }
}
