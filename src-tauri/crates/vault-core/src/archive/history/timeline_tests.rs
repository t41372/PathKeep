//! Responsibilities: prove timeline completeness and reader isolation on real Stores.
//! Not responsible for: keyword/regex recall or frontend pagination.
//! Failure modes: timestamp ties skip/repeat visits, reverted facts leak, filtered
//! pages miss rows, OFFSET rescans, a reader waits on or sees an uncommitted batch.
use super::*;
use crate::archive::source_stats::tests::seeded_archive;
use vault_store::Priority;

#[test]
fn keysets_reach_each_visible_visit_once_in_both_directions() {
    for key in [None, Some([75; 32])] {
        let vault = seeded_archive(key);
        vault.store().write(Db::Archive, Priority::Batch, |s| {
            for id in 5..=31 {
                s.execute("INSERT INTO visits(id,url_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(?1,1,5000,'',1,0)", [id])?;
            }
            s.execute("UPDATE archive_totals SET value=30 WHERE name='visits'",())?;
            Ok(())
        }).unwrap();
        for oldest in [false, true] {
            let mut query = HistoryQuery {
                limit: Some(4),
                sort: Some(if oldest { "oldest" } else { "newest" }.into()),
                ..Default::default()
            };
            let mut ids = Vec::new();
            loop {
                let page = list_history(&vault, query.clone()).unwrap();
                assert_eq!(page.total, 30);
                assert!(page.total_exact);
                ids.extend(page.items.iter().map(|v| v.id));
                query.cursor = page.next_cursor;
                if query.cursor.is_none() {
                    break;
                }
                assert!(ids.len() <= 30);
            }
            let expected: Vec<i64> = if oldest {
                vec![1, 4, 2].into_iter().chain(5..=31).collect()
            } else {
                (5..=31).rev().chain([2, 4, 1]).collect()
            };
            assert_eq!(ids, expected);
        }
    }
}

#[test]
fn production_timeline_plans_seek_without_sorting_visits() {
    for key in [None, Some([75; 32])] {
        let vault = seeded_archive(key);
        vault
            .store()
            .read(Db::Archive, |s| {
                for sort in ["newest", "oldest"] {
                    for profile in [None, Some("chrome:Default".to_owned())] {
                        let query = HistoryQuery {
                            sort: Some(sort.into()),
                            profile_id: profile,
                            limit: Some(2),
                            cursor: Some("5000|20".into()),
                            ..Default::default()
                        };
                        for (sql, args) in [
                            timeline_sql(&query, 3, i64::MAX).unwrap(),
                            timeline_ties_sql(&query, 3, i64::MAX, (5000, 20)).unwrap(),
                        ] {
                            let plan = s
                                .query(&format!("EXPLAIN QUERY PLAN {sql}"), args, |r| {
                                    Ok(r.get::<String>(3)?)
                                })?
                                .join("\n");
                            assert!(plan.contains("SEARCH visits"), "{plan}");
                            assert!(
                                !plan.contains("SCAN visits") && !plan.contains("SCAN urls"),
                                "{plan}"
                            );
                            assert!(
                                !plan.contains("TEMP B-TREE") && !plan.contains("SORTER"),
                                "{plan}"
                            );
                        }
                    }
                }
                Ok(())
            })
            .unwrap();
    }
}
