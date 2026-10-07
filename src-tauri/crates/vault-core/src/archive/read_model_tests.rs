//! Responsibilities: prove archive status, cached dashboard totals and complete run paging.
//! Not responsible for: legacy initialization or whole-file maintenance.
//! Failure modes: totals recounted/stale, seed run leaked, tied timestamps drop runs,
//! running batch hidden, manifest or warnings missing from audit detail.
use super::*;
use crate::Vault;
use crate::archive::run_support::{EARLIEST_VISIT_SQL, LATEST_VISIT_SQL};
use vault_store::{Db, Priority};

#[test]
fn dashboard_reads_committed_counters_and_runs_page_to_the_end() {
    for key in [None, Some([78; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("UPDATE archive_totals SET value=12345678 WHERE name='visits'",())?;
            for id in 2..=35 {
                s.execute("INSERT INTO runs(id,run_type,trigger,started_at,status,stats_json) VALUES(?1,'backup','manual','2026-01-01','running','{}')",[id])?;
            }
            Ok(())
        }).unwrap();
        assert!(archive_status(&vault).unwrap().unlocked);
        let dashboard = load_dashboard_snapshot(&vault).unwrap();
        assert_eq!(
            dashboard.total_visits, 12345678,
            "render uses committed counters, never recounts facts"
        );
        let mut cursor = None;
        let mut ids = Vec::new();
        loop {
            let page = load_recent_runs(&vault, cursor, 4).unwrap();
            ids.extend(page.items.iter().map(|run| run.id));
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(ids, (1..=35).rev().collect::<Vec<_>>());
        let detail = load_audit_run_detail(&vault, 35).unwrap();
        assert_eq!(detail.run.status, "running");
    }
}

#[test]
fn overview_bounds_seek_the_visible_time_index() {
    let vault = Vault::open_temp().unwrap();
    vault
        .store()
        .read(Db::Archive, |s| {
            for sql in [EARLIEST_VISIT_SQL, LATEST_VISIT_SQL] {
                let plan = s
                    .query(&format!("EXPLAIN QUERY PLAN {sql}"), (), |r| Ok(r.get::<String>(3)?))?
                    .join("\n");
                assert!(
                    plan.contains("SEARCH visits") && plan.contains("idx_visits_visible_time_id"),
                    "{plan}"
                );
                assert!(!plan.contains("TEMP B-TREE") && !plan.contains("SORTER"), "{plan}");
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn empty_status_file_sizes_and_visible_archive_bounds_remain_truthful() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    assert!(!archive_availability(&paths, &crate::AppConfig::default()).initialized);
    assert_eq!(directory_size(&root.path().join("missing")), 0);
    let file = root.path().join("file");
    std::fs::write(&file, "1234").unwrap();
    assert_eq!(directory_size(&file), 0);
    assert_eq!(directory_size(root.path()), 4);
    for key in [None, Some([78; 32])] {
        let empty = Vault::open_temp_with_key(key).unwrap();
        assert!(load_recent_runs(&empty, None, 12).unwrap().items.is_empty());
        let snapshot = load_dashboard_snapshot(&empty).unwrap();
        assert!(snapshot.earliest_visit_at.is_none() && snapshot.latest_visit_at.is_none());
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("UPDATE visits SET visit_time_iso=CASE id WHEN 1 THEN '2025-04-22T00:00:00Z' WHEN 2 THEN '2026-04-22T00:00:00Z' WHEN 3 THEN '2040-01-01T00:00:00Z' ELSE '2025-10-01T00:00:00Z' END",())?;
            Ok(())
        }).unwrap();
        let snapshot = load_dashboard_snapshot(&vault).unwrap();
        assert_eq!(snapshot.earliest_visit_at.as_deref(), Some("2025-04-22T00:00:00Z"));
        assert_eq!(snapshot.latest_visit_at.as_deref(), Some("2026-04-22T00:00:00Z"));
    }
}

#[test]
fn audit_detail_keeps_scope_warnings_manifest_and_every_snapshot() {
    for key in [None, Some([78; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("UPDATE runs SET profile_scope_json='[\"chrome:Default\"]',stats_json='{\"newVisits\":3}',warnings_json='[\"legacy warning\",{\"code\":\"source-missing\",\"message\":\"missing\"}]' WHERE id=1",())?;
            s.execute("INSERT INTO manifests(run_id,content_hash,file_path,row_counts_json,created_at) VALUES(1,'hash','manifest.json','{}','now')",())?;
            for id in 1..=20 {s.execute("INSERT INTO snapshots(run_id,file_path,created_at,reason) VALUES(1,?1,'now','checkpoint')",[format!("snapshot-{id}")])?;}
            Ok(())
        }).unwrap();
        let detail = load_audit_run_detail(&vault, 1).unwrap();
        assert_eq!(detail.run.new_visits, 3);
        assert_eq!(detail.profile_scope, ["chrome:Default"]);
        assert_eq!(detail.warnings.len(), 2);
        assert_eq!(detail.artifacts.len(), 20);
        assert_eq!(detail.manifest_path.as_deref(), Some("manifest.json"));
        assert!(load_audit_run_detail(&vault, 999).is_err());
    }
}

#[test]
fn an_explicit_zero_processed_profiles_is_not_replaced_by_the_requested_scope() {
    for key in [None, Some([78; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("UPDATE runs SET stats_json='{\"profilesProcessed\":0}',profile_scope_json='[\"chrome:Default\"]' WHERE id=1",())?;Ok(())
        }).unwrap();
        assert_eq!(load_recent_runs(&vault, None, 12).unwrap().items[0].profiles_processed, 0);
        assert_eq!(load_audit_run_detail(&vault, 1).unwrap().run.profiles_processed, 0);
    }
}
