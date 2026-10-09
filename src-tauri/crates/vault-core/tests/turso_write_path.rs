//! Responsibilities: verify real public import, annotation and revert/restore APIs.
//! Not responsible for: browser discovery, index consumption or UI orchestration.
//! Failure modes: split commits, duplicate imports, lost shared-page membership,
//! encrypted writes differ, reverted pages keep a visible search timestamp.

use browser_history_fixtures::{TakeoutBrowserHistoryFixture, TakeoutBrowserRecord};
use vault_core::{ReplaceTagsRequest, SetNotesRequest, TakeoutRequest, Vault};
use vault_store::{Db, Priority};

#[test]
fn imports_annotations_and_visibility_share_the_search_transaction() {
    for key in [None, Some([31; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let source = tempfile::tempdir().unwrap();
        let file = source.path().join("Chrome/BrowserHistory.json");
        TakeoutBrowserHistoryFixture::new()
            .add_record(TakeoutBrowserRecord {
                url: "https://example.com/read".into(),
                title: Some("Reading".into()),
                visit_time_unix_ms: 1_800_000_000_000,
                page_transition: Some("LINK".into()),
                client_id: None,
                favicon_url: None,
                ptoken: None,
            })
            .write(&file)
            .unwrap();
        let request =
            TakeoutRequest { source_path: source.path().display().to_string(), dry_run: false };
        let result = vault_core::import_takeout(&vault, &request).unwrap();
        assert_eq!(result.imported_items, 1);
        let batch = result.import_batch.unwrap().id;
        assert_eq!(vault_core::import_takeout(&vault, &request).unwrap().imported_items, 0);
        vault_core::set_notes(
            &vault,
            SetNotesRequest {
                url: "https://example.com/read".into(),
                notes: "Remember the source".into(),
                source_profile: None,
            },
        )
        .unwrap();
        vault_core::replace_tags(
            &vault,
            ReplaceTagsRequest {
                url: "https://example.com/read".into(),
                tags: vec!["Research".into()],
                source_profile: None,
            },
        )
        .unwrap();
        let document = || {
            vault.store().read(Db::Archive,|s|s.query_one("SELECT notes_text,tags_text,newest_visit_ms,(SELECT COUNT(*) FROM search_document_profiles),(SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='search_outbox'),0)) FROM search_documents",(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<Option<i64>>(2)?,r.get::<i64>(3)?,r.get::<i64>(4)?)))).unwrap()
        };
        let before = document();
        assert_eq!(
            (&*before.0, &*before.1, before.2, before.3),
            ("Remember the source", "Research", Some(1_800_000_000_000), 1)
        );
        vault_core::revert_import_batch(&vault, batch).unwrap();
        let reverted = document();
        assert_eq!((reverted.2, reverted.3), (None, 0));
        assert!(reverted.4 > before.4);
        vault_core::restore_import_batch(&vault, batch).unwrap();
        let restored = document();
        assert_eq!((restored.2, restored.3), (before.2, before.3));
        vault
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                s.execute("DROP TABLE search_outbox", ())?;
                Ok(())
            })
            .unwrap();
        assert!(
            vault_core::set_notes(
                &vault,
                SetNotesRequest {
                    url: "https://example.com/read".into(),
                    notes: "cannot commit".into(),
                    source_profile: None
                }
            )
            .is_err()
        );
        assert_eq!(
            vault_core::get_annotation(&vault, "https://example.com/read").unwrap().unwrap().notes,
            "Remember the source"
        );
    }
}

#[test]
fn cancelled_import_keeps_batch_identity_and_committed_progress() {
    for key in [None, Some([32; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::create_dir(source.path().join("Chrome")).unwrap();
        let records = (0..20_003_i64)
            .map(|id| {
                serde_json::json!({"url":"https://example.com/batched","title":"Batch",
                    "time_usec":(1_800_000_000_000 + id) * 1_000})
            })
            .collect::<Vec<_>>();
        std::fs::write(
            source.path().join("Chrome/BrowserHistory.json"),
            serde_json::json!({ "Browser History": records }).to_string(),
        )
        .unwrap();
        let cancel = vault_store::CancelToken::new();
        let result = vault_core::takeout::import_takeout_with_control(
            &vault,
            &TakeoutRequest { source_path: source.path().display().to_string(), dry_run: false },
            &cancel,
            |event| {
                if event.imported_records.is_some_and(|n| n >= 10_000) {
                    let batch =
                        vault_core::load_import_batches(&vault, None, 16).unwrap().items.remove(0);
                    assert!(
                        vault_core::revert_import_batch(&vault, batch.id).is_err(),
                        "an active producer must not race batch reversal"
                    );
                    cancel.cancel();
                }
            },
        );
        assert!(result.is_err());
        let batches = vault_core::load_import_batches(&vault, None, 16).unwrap().items;
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].status, "interrupted");
        assert_eq!(batches[0].visible_items, 10_000);
        assert_eq!(batches[0].imported_items, 10_000);
        vault_core::revert_import_batch(&vault, batches[0].id).unwrap();
        assert_eq!(
            vault_core::preview_import_batch(&vault, batches[0].id).unwrap().batch.visible_items,
            0
        );
        let restored = vault_core::restore_import_batch(&vault, batches[0].id).unwrap();
        assert_eq!(
            restored.batch.status, "interrupted",
            "restoring a cancelled import must preserve its partial state"
        );
        assert_eq!(
            vault_core::preview_import_batch(&vault, batches[0].id).unwrap().batch.visible_items,
            10_000
        );
    }
}

#[test]
fn failed_batch_creation_cannot_leave_a_phantom_import_run() {
    for key in [None, Some([77; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let source = vault.paths().app_root.join("input");
        std::fs::create_dir_all(source.join("Chrome")).unwrap();
        std::fs::write(
            source.join("Chrome/BrowserHistory.json"),
            r#"{"Browser History":[{"url":"https://atomic.test","time_usec":1767225600000000}]}"#,
        )
        .unwrap();
        vault
            .store()
            .write(Db::Archive, Priority::Interactive, |s| {
                s.execute("DROP TABLE import_batches", ())?;
                Ok(())
            })
            .unwrap();
        assert!(
            vault_core::import_takeout(
                &vault,
                &vault_core::TakeoutRequest {
                    source_path: source.display().to_string(),
                    dry_run: false
                }
            )
            .is_err()
        );
        vault
            .store()
            .read(Db::Archive, |s| {
                assert_eq!(
                    s.query_one("SELECT COUNT(*) FROM runs WHERE run_type='import'", (), |r| Ok(
                        r.get::<i64>(0)?
                    ))?,
                    0
                );
                assert_eq!(
                    s.query_one("SELECT COUNT(*) FROM source_profiles", (), |r| Ok(
                        r.get::<i64>(0)?
                    ))?,
                    0
                );
                Ok(())
            })
            .unwrap();
    }
}
