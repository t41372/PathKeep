//! Responsibilities: verify native health findings and conservative repairs.
//! Not responsible for: legacy intelligence or file recovery.
//! Failure modes: corruption reported healthy, schema checks omitted, cached totals
//! drift, missing search documents/outbox notifications, incomplete artifact checks,
//! repair restores hidden visits or records success after an artifact error,
//! a concurrent indexer consumes intentionally damaged outbox fixtures before diagnosis.
use super::*;
use vault_store::{Db, Priority};

#[test]
fn healthy_native_files_need_no_repair() {
    for key in [None, Some([79; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let report = doctor(&vault).unwrap();
        for code in [
            "archive-integrity",
            "source-evidence-integrity",
            "intelligence-integrity",
            "agent-integrity",
            "schema-version",
            "cached-totals",
            "search-outbox",
        ] {
            assert!(report.checks.iter().any(|c| c.code == code && c.ok), "{code}: {report:?}");
        }
        assert!(repair_health_issues(&vault, None).unwrap().run_id.is_none());
    }
}

#[test]
fn repairs_counters_missing_documents_and_visibility_provenance_without_unhiding() {
    for key in [None, Some([79; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        // Keep the corrupt fixture stable until doctor has diagnosed and repaired it.
        // The live consumer is resumed below to verify that the repaired handoff applies.
        let paused = vault.search().pause();
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("DELETE FROM search_document_profiles", ())?;
                s.execute("DELETE FROM search_documents", ())?;
                s.execute("INSERT INTO search_outbox(page_id,change) VALUES(999,'upsert')", ())?;
                s.execute("UPDATE archive_totals SET value=999", ())?;
                Ok(())
            })
            .unwrap();
        let before = doctor(&vault).unwrap();
        for code in ["cached-totals", "search-outbox", "broken-visibility-references"] {
            assert!(before.checks.iter().any(|c| c.code == code && !c.ok), "{code}");
        }
        let repaired = repair_health_issues(&vault, None).unwrap();
        assert!(repaired.run_id.is_some());
        assert_eq!(repaired.repaired_visibility_rows, 1);
        assert_eq!(repaired.repaired_total_counters, 8);
        assert_eq!(repaired.refreshed_search_pages, 1);
        assert!(repaired.queued_search_updates >= 2);
        let after = doctor(&vault).unwrap();
        for code in ["cached-totals", "search-outbox", "broken-visibility-references"] {
            assert!(after.checks.iter().any(|c| c.code == code && c.ok), "{code}: {after:?}");
        }
        vault
            .store()
            .read(Db::Archive, |s| {
                assert_eq!(
                    s.query_one(
                        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NOT NULL",
                        (),
                        |r| Ok(r.get::<i64>(0)?)
                    )?,
                    1
                );
                assert_eq!(
                    s.query_one("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
                        Ok(r.get::<i64>(0)?)
                    })?,
                    3
                );
                Ok(())
            })
            .unwrap();
        drop(paused);
        vault.search().cycle().unwrap();
        let result =
            crate::search::page(&vault, crate::search::SearchRequest::default(), None).unwrap();
        assert_eq!(result.count.value, 1);
        assert_eq!(result.visits.value, 3);
        assert_eq!(result.items[0].page_id, 1);
    }
}

#[test]
fn checks_every_snapshot_and_manifest_parent_and_hash() {
    for key in [None, Some([79; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let present = vault.paths().app_root.join("present");
        std::fs::write(&present, b"present").unwrap();
        let path = present.display().to_string();
        vault.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute("INSERT INTO snapshots(run_id,file_path,created_at) VALUES(0,'/missing/pathkeep-test',''),(0,?1,'')",[path.as_str()])?;
            s.execute("INSERT INTO manifests(run_id,content_hash,row_counts_json,created_at) VALUES(0,'a','{}',''),(0,'b','{}','')",())?;
            Ok(())
        }).unwrap();
        let report = doctor(&vault).unwrap();
        assert!(report.checks.iter().any(|c| c.code == "snapshot-artifacts" && !c.ok));
        assert!(report.checks.iter().any(|c| c.code == "manifest-chain" && !c.ok));
    }
}

#[test]
fn missing_import_audits_are_rebuilt_and_write_failures_leave_failed_runs() {
    for key in [None, Some([79; 32])] {
        for blocked in [false, true] {
            let vault = crate::Vault::open_temp_with_key(key).unwrap();
            vault.store().write(Db::Archive,Priority::Batch,|s| {
                s.execute("INSERT INTO import_batches(source_kind,source_path,profile_id,created_at,imported_at,status,summary_json) VALUES('takeout','/fixture','takeout','2026-10-05T00:00:00Z','2026-10-05T00:00:00Z','imported','{}')",())?;
                Ok(())
            }).unwrap();
            assert!(
                doctor(&vault)
                    .unwrap()
                    .checks
                    .iter()
                    .any(|c| c.code == "import-audit-artifacts" && !c.ok)
            );
            if blocked {
                std::fs::create_dir_all(&vault.paths().audit_repo_path).unwrap();
                std::fs::write(vault.paths().audit_repo_path.join("imports"), b"not a directory")
                    .unwrap();
                assert!(repair_health_issues(&vault, None).is_err());
                let status=vault.store().read(Db::Archive,|s|s.query_one("SELECT status FROM runs WHERE run_type='doctor' ORDER BY id DESC LIMIT 1",(),|r|Ok(r.get::<String>(0)?))).unwrap();
                assert_eq!(status, "failed");
            } else {
                let repaired = repair_health_issues(&vault, None).unwrap();
                assert_eq!(repaired.repaired_import_audits, 1);
                assert!(
                    doctor(&vault)
                        .unwrap()
                        .checks
                        .iter()
                        .any(|c| c.code == "import-audit-artifacts" && c.ok)
                );
            }
        }
    }
}

#[test]
fn manifest_hash_damage_and_stable_check_codes_are_reported() {
    for key in [None, Some([79; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let path = vault.paths().app_root.join("manifest.json");
        std::fs::write(&path, b"changed").unwrap();
        let path = path.display().to_string();
        vault.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute("INSERT INTO manifests(run_id,content_hash,row_counts_json,created_at,file_path) VALUES(0,'incorrect','{}','',?1)",[path])?;
            Ok(())
        }).unwrap();
        let report = doctor(&vault).unwrap();
        assert!(
            report
                .checks
                .iter()
                .any(|c| c.code == "manifest-chain" && !c.ok && c.detail.contains("hash mismatch"))
        );
        assert!(
            report.checks.iter().all(|c| !c.code.is_empty()
                && c.code.chars().all(|x| x.is_ascii_lowercase() || x == '-'))
        );
    }
}

#[test]
fn repairs_profile_and_import_counters_even_when_archive_totals_are_correct() {
    for key in [None, Some([79; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            s.execute("UPDATE search_document_profiles SET visible_visit_count=99",())?;
            s.execute("INSERT INTO import_batches(id,source_kind,source_path,profile_id,created_at,status,summary_json,visible_items) VALUES(1,'takeout','/fixture','takeout','','running','{}',99)",())?;
            s.execute("UPDATE visits SET import_batch_id=1",())?;
            for (name,count) in [("visits",3),("urls",2),("sourceProfiles",3)] {s.execute("UPDATE archive_totals SET value=?2 WHERE name=?1",vault_store::params![name,count])?;}
            Ok(())
        }).unwrap();
        let repaired = repair_health_issues(&vault, None).unwrap();
        assert_eq!(repaired.repaired_batch_totals, 1);
        let stats = crate::load_source_stats(&vault).unwrap();
        assert_eq!(stats.iter().map(|s| s.visit_count).sum::<i64>(), 3);
        vault
            .store()
            .read(Db::Archive, |s| {
                assert_eq!(
                    s.query_one(
                        "SELECT visible_items FROM import_batches WHERE id=1",
                        (),
                        |r| Ok(r.get::<i64>(0)?)
                    )?,
                    3
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn audit_repair_retains_optional_git_result_or_its_warning() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.store().write(Db::Archive,Priority::Batch,|s| {
        s.execute("INSERT INTO import_batches(source_kind,source_path,profile_id,created_at,status,summary_json) VALUES('takeout','/fixture','takeout','2026-10-05T00:00:00Z','imported','{}')",())?;Ok(())
    }).unwrap();
    let mut config = vault.config().clone();
    config.git_enabled = true;
    let report = repair_health_issues(&vault.with_config(config), None).unwrap();
    assert_eq!(report.repaired_import_audits, 1);
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.contains("audit commit")
                || n.contains("optional Git history step was skipped"))
    );
}

#[test]
fn a_native_derived_only_repair_keeps_its_audit_run_and_metadata_totals() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.store().write(vault_store::Db::Intelligence,vault_store::Priority::Batch,|s| {
        s.execute("INSERT INTO ai_embeddings(history_id,profile_id,url,title,domain,visited_at,content_hash,content_bytes,metadata_bytes,provider_id,model,indexed_at) VALUES(999,'profile','https://example.com','Title','example.com','2026-10-05','hash',1,1,'test','test','2026-10-05')",())?;
        s.execute("INSERT INTO ai_embedding_totals(provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at) VALUES('test','test',1,1,1,'2026-10-05')",())?;Ok(())
    }).unwrap();
    let report = repair_health_issues(&vault, None).unwrap();
    assert_eq!(report.cleared_derived_rows, 1);
    let run = report.run_id.expect("derived repairs must retain their audit trace");
    let detail = crate::load_audit_run_detail(&vault, run).unwrap();
    assert_eq!(detail.run.status, "success");
    assert_eq!(detail.stats["clearedDerivedRows"], 1);
}
