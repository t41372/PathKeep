//! Responsibilities: prove reversible import visibility and atomic search handoff.
//! Not responsible for: Tantivy queries or old FTS synchronization.
//! Failure modes: shared pages disappear, hidden visits retain memberships,
//! chunk boundaries omit URLs, metadata disappears on restore, failed outbox
//! commits canonical visibility without its document.

use super::*;
use crate::Vault;
use vault_store::{Db, Priority};

struct Fixture {
    vault: Vault,
    paths: ProjectPaths,
    batch_id: i64,
}
impl Fixture {
    fn new(count: usize, key: Option<[u8; 32]>) -> Self {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let mut f = Self { paths: vault.paths().clone(), vault, batch_id: 0 };
        f.batch_id = f.import("batch", 0, count, 0);
        f
    }
    fn import(&self, name: &str, first_url: usize, count: usize, time_offset: i64) -> i64 {
        let source = self.paths.app_root.join(name);
        fs::create_dir_all(&source).expect("source directory");
        let records = (first_url..first_url + count)
            .map(|index| {
                json!({
                    "url": format!("https://batch.example/page/{index}"),
                    "title": format!("Batchneedle page {index}"),
                    "time_usec": (1_775_000_000_000 + time_offset + index as i64 * 1_000) * 1_000,
                })
            })
            .collect::<Vec<_>>();
        fs::create_dir_all(source.join("Chrome")).expect("Chrome directory");
        fs::write(
            source.join("Chrome/BrowserHistory.json"),
            json!({ "Browser History": records }).to_string(),
        )
        .expect("import fixture");
        import_takeout(
            &self.vault,
            &TakeoutRequest { source_path: source.display().to_string(), dry_run: false },
        )
        .expect("real import")
        .import_batch
        .expect("batch overview")
        .id
    }

    fn visible(&self) -> i64 {
        self.vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one("SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL", (), |r| {
                    Ok(r.get(0)?)
                })
            })
            .unwrap()
    }
    fn members(&self) -> Vec<(String, i64, i64)> {
        self.vault.store().read(Db::Archive,|s|s.query("SELECT d.url,p.visible_visit_count,p.newest_visit_ms FROM search_documents d JOIN search_document_profiles p ON p.page_id=d.page_id ORDER BY d.url,p.source_profile_id",(),|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))).unwrap()
    }
    fn assert_matches_rebuild(&self) {
        let expected = self.vault.store().read(Db::Archive, |s| {
            s.query("SELECT p.url,COUNT(*),MAX(v.visit_time_ms) FROM visits v JOIN urls u ON u.id=v.url_id JOIN pages p ON p.id=u.page_id WHERE v.reverted_at IS NULL GROUP BY p.id,v.source_profile_id ORDER BY p.url,v.source_profile_id", (), |r| Ok((r.get::<String>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?)))
        }).unwrap();
        assert_eq!(
            self.members(),
            expected,
            "memberships must equal independently counted visible facts"
        );
        let pages = self.vault.store().read(Db::Archive, |s| {
            s.query("SELECT p.id,MAX(v.visit_time_ms) FROM pages p LEFT JOIN urls u ON u.page_id=p.id LEFT JOIN visits v ON v.url_id=u.id AND v.reverted_at IS NULL GROUP BY p.id ORDER BY p.id", (), |r| Ok((r.get::<i64>(0)?,r.get::<Option<i64>>(1)?)))
        }).unwrap();
        assert_eq!(
            self.documents().into_iter().map(|(id, newest, _, _)| (id, newest)).collect::<Vec<_>>(),
            pages
        );
        let totals = self
            .vault
            .store()
            .read(Db::Archive, crate::archive::totals::load_cached_archive_totals)
            .unwrap();
        assert_eq!(totals["visits"], self.visible());
        let stats: Value = self
            .vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one(
                    "SELECT stats_json FROM runs WHERE status='success' ORDER BY id DESC LIMIT 1",
                    (),
                    |r| Ok(r.get::<String>(0)?),
                )
            })
            .map(|text| serde_json::from_str(&text).unwrap())
            .unwrap();
        assert_eq!(stats["totalVisits"], self.visible());
    }
    fn documents(&self) -> Vec<(i64, Option<i64>, String, String)> {
        self.vault.store().read(Db::Archive,|s|s.query("SELECT page_id,newest_visit_ms,notes_text,tags_text FROM search_documents ORDER BY page_id",(),|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))).unwrap()
    }
}

#[test]
fn revert_keeps_shared_urls_and_only_remaining_visits_searchable() {
    for key in [None, Some([71; 32])] {
        let f = Fixture::new(2, key);
        f.import("other", 0, 1, 86_400_000);
        f.import("untouched", 9, 1, 86_400_000);
        let batch = f.batch_id;
        f.vault.store().write(Db::Archive,Priority::Interactive,move |s| {
            s.execute("INSERT INTO visits(url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id)
                SELECT url_id,'regular-backup',visit_time_ms-1,visit_time_iso,source_profile_id,0 FROM visits WHERE import_batch_id=?1 ORDER BY id LIMIT 1",[batch])?;
            crate::archive::totals::add(s, "visits", 1)?;
            Ok(())
        }).unwrap();
        f.vault.store().write(Db::Archive,Priority::Interactive,|s| {
            s.execute("UPDATE search_documents SET updated_at='untouched' WHERE url='https://batch.example/page/9'",())?; Ok(())
        }).unwrap();
        revert_import_batch(&f.vault, f.batch_id).unwrap();
        assert_eq!(f.visible(), 3);
        assert_eq!(f.members().iter().find(|r| r.0.ends_with("/0")).unwrap().1, 2);
        assert_eq!(f.members().len(), 2);
        assert_eq!(f.documents().len(), 3, "page anchors survive revert");
        let time=f.vault.store().read(Db::Archive,|s|s.query_one("SELECT updated_at FROM search_documents WHERE url='https://batch.example/page/9'",(),|r|Ok(r.get::<String>(0)?))).unwrap();
        assert_eq!(time, "untouched");
        f.assert_matches_rebuild();
    }
}

#[test]
fn restore_makes_hidden_visits_and_metadata_searchable_again() {
    for key in [None, Some([72; 32])] {
        let f = Fixture::new(3, key);
        crate::set_notes(
            &f.vault,
            crate::SetNotesRequest {
                url: "https://batch.example/page/0".into(),
                notes: "annotationword 設計系統".into(),
                source_profile: None,
            },
        )
        .unwrap();
        crate::replace_tags(
            &f.vault,
            crate::ReplaceTagsRequest {
                url: "https://batch.example/page/0".into(),
                tags: vec!["tagword".into()],
                source_profile: None,
            },
        )
        .unwrap();
        f.vault
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                crate::archive::search_documents::set_enrichment(s, 1, "enrichedword")
            })
            .unwrap();
        let before = f.documents();
        revert_import_batch(&f.vault, f.batch_id).unwrap();
        assert_eq!(f.visible(), 0);
        assert!(f.members().is_empty());
        assert!(f.documents().iter().all(|d| d.1.is_none()));
        restore_import_batch(&f.vault, f.batch_id).unwrap();
        assert_eq!(f.visible(), 3);
        assert_eq!(f.documents(), before);
        f.assert_matches_rebuild();
        assert_eq!(
            f.vault
                .store()
                .read(Db::Archive, |s| s.query_one(
                    "SELECT enrichment_text FROM search_documents WHERE page_id=1",
                    (),
                    |r| Ok(r.get::<String>(0)?)
                ))
                .unwrap(),
            "enrichedword"
        );
    }
}

#[test]
fn visibility_refresh_crosses_chunk_boundaries_without_skipping_urls() {
    for key in [None, Some([73; 32])] {
        let f = Fixture::new(2_003, key);
        let batch = f.batch_id;
        f.vault.store().write(Db::Archive,Priority::Interactive,move |s| {
            for offset in 1..=4 {
                s.execute("INSERT INTO visits(url_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id,import_batch_id)
                    SELECT url_id,visit_time_ms+?1,visit_time_iso,source_profile_id,created_by_run_id,import_batch_id
                    FROM visits WHERE import_batch_id=?2 ORDER BY id LIMIT 1",vault_store::params![offset,batch])?;
            }
            crate::archive::totals::add(s, "visits", 4)?;
            s.execute("UPDATE import_batches SET visible_items=visible_items+4 WHERE id=?1", [batch])?;
            Ok(())
        }).unwrap();
        f.import("shared-across-boundary", 999, 3, 86_400_000);
        for restore in [false, true] {
            let sequence = crate::test_support::search_sequence(&f.vault);
            if restore {
                restore_import_batch(&f.vault, batch).unwrap();
            } else {
                revert_import_batch(&f.vault, batch).unwrap();
            }
            assert_eq!(f.visible(), if restore { 2_010 } else { 3 });
            assert_eq!(crate::test_support::search_sequence(&f.vault) - sequence, 2003);
            crate::test_support::assert_search_handoff(&f.vault);
            f.assert_matches_rebuild();
        }
    }
}

#[test]
fn handoff_failure_rolls_back_visits_documents_memberships_and_batch_status() {
    let f = Fixture::new(1_003, None);
    let before = f.documents();
    let members = f.members();
    f.vault
        .store()
        .write(Db::Archive, Priority::Interactive, |s| {
            s.execute("DROP TABLE search_outbox", ())?;
            Ok(())
        })
        .unwrap();
    assert!(revert_import_batch(&f.vault, f.batch_id).is_err());
    assert_eq!(f.visible(), 1_003);
    assert_eq!(f.documents(), before);
    assert_eq!(f.members(), members);
    assert_eq!(preview_import_batch(&f.vault, f.batch_id).unwrap().batch.status, "reverting");
}

#[test]
fn cancelled_multichunk_revert_resumes_from_committed_cursor() {
    for key in [None, Some([84; 32])] {
        let f = Fixture::new(1_105, key);
        let before = f.members();
        let cancel = vault_store::CancelToken::new();
        let result = super::super::visibility::change_visibility(
            &f.vault,
            f.batch_id,
            false,
            &cancel,
            |_| {
                assert!(
                    revert_import_batch(&f.vault, f.batch_id)
                        .unwrap_err()
                        .to_string()
                        .contains("already reverting")
                );
                assert!(restore_import_batch(&f.vault, f.batch_id).is_err());
                cancel.cancel();
            },
        );
        assert!(result.is_err());
        assert_eq!(f.visible(), 105);
        assert_eq!(preview_import_batch(&f.vault, f.batch_id).unwrap().batch.status, "reverting");
        assert!(restore_import_batch(&f.vault, f.batch_id).is_err());
        revert_import_batch(&f.vault, f.batch_id).unwrap();
        assert_eq!(f.visible(), 0);
        restore_import_batch(&f.vault, f.batch_id).unwrap();
        assert_eq!(f.members(), before);
    }
}

#[test]
fn visibility_crash_child() {
    let Some(root) = std::env::var_os("PATHKEEP_VISIBILITY_CRASH_ROOT") else { return };
    let key = std::env::var_os("PATHKEEP_VISIBILITY_CRASH_KEY").map(|_| [87; 32]);
    let vault = Vault::open(
        crate::project_paths_with_root(Path::new(&root)),
        AppConfig { initialized: true, git_enabled: false, ..Default::default() },
        key,
    )
    .unwrap();
    super::super::visibility::change_visibility(
        &vault,
        1,
        false,
        &vault_store::CancelToken::new(),
        |_| std::process::exit(87),
    )
    .unwrap();
    panic!("the process must exit after the first committed visibility chunk");
}

#[test]
fn process_exit_mid_revert_resumes_without_repeating_committed_pages() {
    for key in [None, Some([87; 32])] {
        let f = Fixture::new(1_105, key);
        let sequence = crate::test_support::search_sequence(&f.vault);
        f.vault.close().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "takeout::batches::search_tests::visibility_crash_child",
                "--nocapture",
            ])
            .env("PATHKEEP_VISIBILITY_CRASH_ROOT", &f.paths.app_root);
        if key.is_some() {
            child.env("PATHKEEP_VISIBILITY_CRASH_KEY", "1");
        }
        let output = child.output().unwrap();
        assert_eq!(output.status.code(), Some(87), "{}", String::from_utf8_lossy(&output.stderr));
        let vault = Vault::open(f.paths.clone(), f.vault.config().clone(), key).unwrap();
        assert_eq!(preview_import_batch(&vault, 1).unwrap().batch.status, "reverting");
        vault
            .store()
            .read(Db::Archive, |s| {
                assert_eq!(
                    s.query_one(
                        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",
                        (),
                        |r| Ok(r.get::<i64>(0)?)
                    )?,
                    105
                );
                assert_eq!(
                    s.query_one("SELECT status FROM runs WHERE run_type='rollback'", (), |r| Ok(
                        r.get::<String>(0)?
                    ))?,
                    "interrupted"
                );
                Ok(())
            })
            .unwrap();
        revert_import_batch(&vault, 1).unwrap();
        vault
            .store()
            .read(Db::Archive, |s| {
                assert_eq!(
                    s.query_one(
                        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NULL",
                        (),
                        |r| Ok(r.get::<i64>(0)?)
                    )?,
                    0
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(crate::test_support::search_sequence(&vault) - sequence, 1105);
        crate::test_support::assert_search_handoff(&vault);
    }
}

#[test]
fn reverting_one_profile_preserves_a_shared_pages_other_profile() {
    for key in [None, Some([71; 32])] {
        let f = Fixture::new(1, key);
        f.vault.write(Db::Archive, Priority::Batch, |s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,profile_key) VALUES(900,'chrome','other','other','now','chrome:other')", ())?;
            s.execute("INSERT INTO urls(id,url,visit_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,page_id) SELECT 900,url,1,2000000000000,'2033',2000000000000,'2033',900,0,page_id FROM urls LIMIT 1", ())?;
            s.execute("INSERT INTO visits(url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id) VALUES(900,900,2000000000000,'2033',0)", ())?;
            for field in ["sourceProfiles","urls","visits"] { crate::archive::totals::add(s, field, 1)?; }
            let page = s.query_one("SELECT page_id FROM urls WHERE id=900", (), |r| Ok(r.get::<i64>(0)?))?;
            crate::archive::search_documents::pages_written(s, 900, &Default::default(), &[(page, (1, 2000000000000))].into())?;
            s.execute("DELETE FROM search_outbox", ())?;
            Ok(())
        }).unwrap();
        revert_import_batch(&f.vault, f.batch_id).unwrap();
        assert_eq!(f.visible(), 1);
        assert_eq!(f.members().len(), 1);
        f.assert_matches_rebuild();
        restore_import_batch(&f.vault, f.batch_id).unwrap();
        assert_eq!(f.visible(), 2);
        assert_eq!(f.members().len(), 2);
        f.assert_matches_rebuild();
    }
}

#[test]
fn startup_lists_unfinished_visibility_even_outside_the_recent_batch_limit() {
    let mut f = Fixture::new(1_001, None);
    let cancel = vault_store::CancelToken::new();
    assert!(
        revert_import_batch_with_control(&f.vault, f.batch_id, &cancel, |_| cancel.cancel())
            .is_err()
    );
    f.vault.write(Db::Archive, Priority::Batch, |s| {
        for _ in 0..17 {
            s.execute("INSERT INTO import_batches(source_kind,source_path,profile_id,created_at,status,summary_json) VALUES('takeout','other','takeout::browser-history','now','imported','{}')", ())?;
        }
        Ok(())
    }).unwrap();
    let default_status = f
        .vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one(
                "SELECT previous_status FROM import_batches ORDER BY id DESC LIMIT 1",
                (),
                |r| Ok(r.get::<String>(0)?),
            )
        })
        .unwrap();
    assert_eq!(default_status, "imported");
    let original = f.vault.clone();
    original.store().close().unwrap();
    f.vault = Vault::open(f.paths.clone(), original.config().clone(), None).unwrap();
    assert!(
        load_import_batches(&f.vault, None, 16)
            .unwrap()
            .items
            .iter()
            .any(|batch| batch.id == f.batch_id && batch.status == "reverting")
    );
    revert_import_batch(&f.vault, f.batch_id).unwrap();
    assert_eq!(f.visible(), 0);
    restore_import_batch(&f.vault, f.batch_id).unwrap();
    assert_eq!(f.visible(), 1_001);
}
