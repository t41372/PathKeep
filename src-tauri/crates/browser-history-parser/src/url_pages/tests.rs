//! Responsibilities: verify production paging against browser schemas as shipped.
//! Not responsible for: inventing indexes or replacing the source SQL in a test.
//! Failure modes: a URL-time sort hides quadratic paging, or a visit parent behind
//! the persisted rowid is excluded by the resume predicate.
use super::*;
#[test]
fn browser_url_pages_seek_rowids_without_order_sorters() {
    use browser_history_fixtures::{
        ChromiumHistoryFixture, FirefoxPlacesFixture, SafariHistoryFixture,
    };
    let root = tempfile::tempdir().unwrap();
    for family in [Family::Chromium, Family::Firefox, Family::Safari] {
        let (table, _, _) = family.tables();
        let path = root.path().join(table);
        match family {
            Family::Chromium => ChromiumHistoryFixture::new().write(&path).unwrap(),
            Family::Firefox => FirefoxPlacesFixture::new().write(&path).unwrap(),
            Family::Safari => SafariHistoryFixture::new().write(&path).unwrap(),
        };
        let db = Connection::open(path).unwrap();
        for full in [false, true] {
            for new in [None, Some(1000)] {
                let scan = UrlScan {
                    after_id: 100,
                    time_floor: 100,
                    visit_floor: 1,
                    visit_ceiling: 1000,
                    full,
                };
                let (sql, values) = page_query(family, scan, new, 100, 10);
                let plan = db
                    .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                    .unwrap()
                    .query_map(rusqlite::params_from_iter(values), |r| r.get::<_, String>(3))
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap();
                assert!(
                    plan.iter()
                        .any(|line| line
                            .contains(&format!("SEARCH {table} USING INTEGER PRIMARY KEY"))),
                    "{plan:?}"
                );
                assert!(
                    !plan.iter().any(|line| line.contains("TEMP B-TREE FOR ORDER BY")),
                    "{plan:?}"
                );
                assert!(!plan.iter().any(|line| line.starts_with("SCAN v")), "{plan:?}");
            }
        }
    }
}
#[test]
fn newly_synced_visit_parents_bypass_the_old_scan_position() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("History");
    browser_history_fixtures::ChromiumHistoryFixture::new().write(&path).unwrap();
    let db = Connection::open(path).unwrap();
    db.execute("INSERT INTO urls(id,url,title,visit_count,typed_count,last_visit_time,hidden) VALUES(5,'https://synced.test','Synced',1,0,1,0)",[]).unwrap();
    db.execute("INSERT INTO visits(id,url,visit_time) VALUES(101,5,1)", []).unwrap();
    for full in [false, true] {
        let (sql, values) = page_query(
            Family::Chromium,
            UrlScan { after_id: 100, time_floor: 100, visit_floor: 0, visit_ceiling: 100, full },
            Some(100),
            0,
            10,
        );
        let ids = db
            .prepare(&sql)
            .unwrap()
            .query_map(rusqlite::params_from_iter(values), |r| r.get::<_, i64>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(ids, vec![5]);
    }
}
