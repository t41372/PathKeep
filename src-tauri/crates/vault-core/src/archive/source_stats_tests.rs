//! Responsibilities: verify source counts, visibility, bounds and index seeks.
//! Not responsible for: discovery or ingestion fixtures.
//! Failure modes: reverted visits counted, empty profiles hidden, stale caches, full scans.
use super::*;
use crate::Vault;
use vault_store::{Db, Priority};

pub(crate) fn seeded_archive(key: Option<[u8; 32]>) -> Vault {
    let vault = Vault::open_temp_with_key(key).expect("open");
    vault.store().write(Db::Archive, Priority::Batch, |s| {
            let sql =                 "INSERT INTO runs (id, run_type, trigger, started_at, status, due_only)
                   VALUES (1, 'backup', 'manual', '2026-01-01T00:00:00Z', 'success', 0);
                 INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at, enabled, profile_key)
                   VALUES (1, 'chrome', 'Default', '/p/1', '2026-01-01T00:00:00Z', 1, 'chrome:Default'),
                          (2, 'firefox', 'dev', '/p/2', '2026-01-01T00:00:00Z', 1, 'firefox:dev'),
                          (3, 'edge', 'Empty', '/p/3', '2026-01-01T00:00:00Z', 1, 'edge:Empty');
                 INSERT INTO pages (id,url,canonical_url,registrable_domain) VALUES (1,'https://example.com/a','https://example.com/a','example.com');
                 INSERT INTO urls (id, page_id, url, title, visit_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id)
                   VALUES (1, 1, 'https://example.com/a', 'A', 3, 0, '', 0, '', 1, 1, 1),
                          (2, 1, 'https://example.com/a', 'A (ff)', 1, 0, '', 0, '', 2, 1, 1);
                 INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id, reverted_at)
                   VALUES (1, 1000, '', 1, 1, NULL),
                          (1, 5000, '', 1, 1, NULL),
                          (1, 9000, '', 1, 1, '2026-01-02T00:00:00Z'),
                          (2, 2000, '', 2, 1, NULL);";
            for statement in sql.split(';').filter(|s| !s.trim().is_empty()) {
                s.execute(statement, ())?;
            }
            crate::archive::search_documents::refresh(s, 1)?;
        s.execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',2),('sourceVisits:2',1)",())?;
            Ok(())
        }).expect("seed");
    vault
}

#[test]
fn counts_visible_visits_per_profile_and_keeps_empty_profiles() {
    for key in [None, Some([71; 32])] {
        let vault = seeded_archive(key);
        let stats = load_source_stats(&vault).expect("stats");
        assert_eq!(stats.len(), 3);
        let chrome = stats.iter().find(|row| row.profile_id == "chrome:Default").expect("chrome");
        assert_eq!(chrome.browser_name, "Google Chrome");
        assert_eq!(chrome.visit_count, 2, "reverted visits are not counted");
        assert_eq!(chrome.first_visit_at.as_deref(), Some("1970-01-01T00:00:01+00:00"));
        assert_eq!(chrome.last_visit_at.as_deref(), Some("1970-01-01T00:00:05+00:00"));
        let empty = stats.iter().find(|row| row.profile_id == "edge:Empty").expect("edge");
        assert_eq!((empty.visit_count, empty.first_visit_at.clone()), (0, None));
        assert_eq!(empty.browser_name, "Microsoft Edge");
    }
}

#[test]
fn stats_use_index_seeks_without_visits_or_urls_scans() {
    for key in [None, Some([71; 32])] {
        let vault = seeded_archive(key);
        vault
            .store()
            .read(Db::Archive, |s| {
                for sql in [PROFILE_COUNT_SQL, PROFILE_FIRST_SQL, PROFILE_LAST_SQL] {
                    let plan = s
                        .query(&format!("EXPLAIN QUERY PLAN {sql}"), [1], |r| {
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
fn source_counters_follow_real_import_duplicate_revert_and_restore_transactions() {
    for key in [None, Some([71; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let source = vault.paths().app_root.join("BrowserHistory.json");
        std::fs::write(&source,r#"{"Browser History":[{"url":"https://a.example/","title":"A","time_usec":1700000000000000},{"url":"https://b.example/","title":"B","time_usec":1700000000001000}]}"#).unwrap();
        let request =
            crate::TakeoutRequest { source_path: source.display().to_string(), dry_run: false };
        let batch = crate::import_takeout(&vault, &request).unwrap().import_batch.unwrap().id;
        assert_eq!(load_source_stats(&vault).unwrap()[0].visit_count, 2);
        crate::import_takeout(&vault, &request).unwrap();
        assert_eq!(
            load_source_stats(&vault).unwrap()[0].visit_count,
            2,
            "duplicate facts do not increment counters"
        );
        crate::revert_import_batch(&vault, batch).unwrap();
        assert_eq!(load_source_stats(&vault).unwrap()[0].visit_count, 0);
        crate::restore_import_batch(&vault, batch).unwrap();
        assert_eq!(load_source_stats(&vault).unwrap()[0].visit_count, 2);
    }
}

#[test]
fn visibility_deltas_follow_each_changed_visits_profile() {
    for key in [None, Some([71; 32])] {
        let vault = seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO import_batches(id,source_kind,source_path,profile_id,created_at,status,summary_json,visible_items) VALUES(1,'takeout','/fixture','chrome:Default','2026-10-05T00:00:00Z','imported','{\"notes\":[],\"noteDetails\":[]}',2)",())?;
            s.execute("UPDATE visits SET import_batch_id=1 WHERE id IN (1,4)",())?;
            s.execute("UPDATE archive_totals SET value=3 WHERE name='visits'",())?;
            Ok(())
        }).unwrap();
        crate::revert_import_batch(&vault, 1).unwrap();
        let stats = load_source_stats(&vault).unwrap();
        assert_eq!(stats.iter().find(|s| s.profile_id == "chrome:Default").unwrap().visit_count, 1);
        assert_eq!(stats.iter().find(|s| s.profile_id == "firefox:dev").unwrap().visit_count, 0);
        crate::restore_import_batch(&vault, 1).unwrap();
        assert_eq!(
            load_source_stats(&vault).unwrap().iter().map(|s| s.visit_count).sum::<i64>(),
            3
        );
    }
}
