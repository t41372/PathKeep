//! Responsibilities: prove every batch and visible preview entry remains reachable.
//! Not responsible for: ingestion, search or audit artifact rendering.
//! Failure modes: a fixed recent-batch cap hides older imports, active imports vanish,
//! tied visit times skip rows, reverted rows leak, counts require a fact scan.
use super::*;
use vault_store::{Db, Priority};

#[test]
fn batch_pages_reach_old_active_and_completed_imports_once() {
    for key in [None, Some([80; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            for id in 1..=37 {
                s.execute("INSERT INTO import_batches(id,source_kind,source_path,profile_id,created_at,status,summary_json,visible_items) VALUES(?1,'takeout','/fixture','takeout','',?2,'{}',?1)",vault_store::params![id,if id==1 {"reverting"} else {"imported"}])?;
            }
            Ok(())
        }).unwrap();
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let page = load_import_batches(&vault, cursor, 4).unwrap();
            for batch in page.items {
                assert_eq!(batch.visible_items, batch.id as usize);
                ids.push(batch.id);
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
            assert!(ids.len() <= 37);
        }
        assert_eq!(ids[0], 1, "unfinished operation remains visible");
        ids.sort_unstable();
        assert_eq!(ids, (1..=37).collect::<Vec<_>>());
    }
}

#[test]
fn review_cursor_reaches_tied_rows_after_the_old_preview_limit() {
    for key in [None, Some([80; 32])] {
        let vault = crate::archive::seed_read_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO import_batches(id,source_kind,source_path,profile_id,created_at,status,summary_json) VALUES(1,'takeout','/fixture','takeout','','running','{}')",())?;
            for id in 5..=45 {
                s.execute("INSERT INTO visits(id,url_id,visit_time_ms,visit_time_iso,source_visit_id,source_profile_id,created_by_run_id,import_batch_id) VALUES(?1,1,5000,'',?2,1,0,1)",vault_store::params![id,id.to_string()])?;
            }
            Ok(())
        }).unwrap();
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let page = preview_import_batch_page(&vault, 1, cursor, 4).unwrap();
            ids.extend(page.preview_entries.iter().map(|e| e.source_visit_id));
            cursor = page.preview_next_cursor;
            if cursor.is_none() {
                break;
            }
            assert!(ids.len() <= 41);
        }
        assert_eq!(ids, (5..=45).rev().collect::<Vec<_>>());
    }
}

#[test]
fn import_review_seeks_its_batch_time_index() {
    for key in [None, Some([80; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        vault
            .store()
            .read(Db::Archive, |s| {
                for ties in [false, true] {
                    let sql = batch_visits_sql(ties, true);
                    let plan = s
                        .query(
                            &format!("EXPLAIN QUERY PLAN {sql}"),
                            vault_store::params![1, 5000, 20, 5],
                            |r| Ok(r.get::<String>(3)?),
                        )?
                        .join("\n");
                    assert!(plan.contains("idx_visits_visible_batch_time_id"), "{plan}");
                    assert!(
                        !plan.contains("SCAN visits")
                            && !plan.contains("TEMP B-TREE")
                            && !plan.contains("SORTER"),
                        "{plan}"
                    );
                }
                Ok(())
            })
            .unwrap();
    }
}
