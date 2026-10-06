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

#[test]
fn reader_returns_committed_batches_while_the_next_batch_is_open() {
    for key in [None, Some([75; 32])] {
        let vault = seeded_archive(key);
        let (entered, ready) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let writer = vault.clone();
        let handle = std::thread::spawn(move || {
            writer.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute("INSERT INTO visits(id,url_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(99,1,10000,'',1,0)",())?;
            entered.send(()).unwrap();
            blocked.recv().unwrap();
            Ok(())
        })
        });
        ready.recv().unwrap();
        let reader = vault.clone();
        let (done, result) = std::sync::mpsc::channel();
        let read = std::thread::spawn(move || {
            done.send(list_history(&reader, HistoryQuery::default())).unwrap()
        });
        let page = result.recv_timeout(std::time::Duration::from_secs(5));
        release.send(()).unwrap();
        handle.join().unwrap().unwrap();
        read.join().unwrap();
        let page = page.expect("read must return before writer commits").unwrap();
        assert_eq!(page.items.iter().map(|v| v.id).collect::<Vec<_>>(), [2, 4, 1]);
        assert_eq!(list_history(&vault, HistoryQuery::default()).unwrap().items[0].id, 99);
    }
}

#[test]
fn cursor_seek_uses_the_id_after_an_equal_timestamp() {
    let vault = seeded_archive(None);
    vault
        .read(Db::Archive, |s| {
            let query = HistoryQuery {
                cursor: Some("5000|20".into()),
                limit: Some(2),
                ..Default::default()
            };
            let (sql, args) = timeline_ties_sql(&query, 3, i64::MAX, (5000, 20))?;
            let bytecode = s.query(&format!("EXPLAIN {sql}"), args, |r| {
                Ok((
                    r.get::<String>(1)?,
                    r.get_value(2)?,
                    r.get_value(3)?,
                    r.get_value(4)?,
                    r.get_value(5)?,
                ))
            })?;
            let seeks = bytecode
                .iter()
                .filter(|(op, _, _, _, _)| op.starts_with("Seek"))
                .collect::<Vec<_>>();
            eprintln!("cursor seek bytecode: {seeks:?}");
            assert!(
                seeks.iter().any(|(op, _, _, _, _)| op == "SeekLT"),
                "the cursor must seek both time and id: {seeks:?}"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn profile_browser_domain_and_date_filters_keep_counts_and_pages_consistent() {
    for key in [None, Some([75; 32])] {
        let vault = seeded_archive(key);
        for (query, expected) in [
            (
                HistoryQuery { profile_id: Some("chrome:Default".into()), ..Default::default() },
                vec![2, 1],
            ),
            (HistoryQuery { browser_kind: Some("firefox".into()), ..Default::default() }, vec![4]),
            (
                HistoryQuery {
                    domain: Some("example.com".into()),
                    start_time_ms: Some(2000),
                    end_time_ms: Some(5000),
                    ..Default::default()
                },
                vec![2, 4],
            ),
            (HistoryQuery { domain: Some("absent.test".into()), ..Default::default() }, vec![]),
        ] {
            let mut query = HistoryQuery { limit: Some(1), ..query };
            let mut ids = Vec::new();
            loop {
                let page = list_history(&vault, query.clone()).unwrap();
                assert_eq!(page.total, expected.len());
                ids.extend(page.items.iter().map(|r| r.id));
                query.cursor = page.next_cursor;
                if query.cursor.is_none() {
                    break;
                }
            }
            assert_eq!(ids, expected);
        }
    }
}
