//! Responsibilities: verify provenance after Takeout payloads without canonical visits.
//! Not responsible for: parser formats, search or backup cursor selection.
//! Failure modes: an evidence-only payload clears the source-batch pointer or regresses
//! previously committed visit cursors; plaintext and encrypted stores disagree.

use super::*;

#[test]
fn evidence_only_payload_keeps_cursors_and_advances_source_provenance() {
    for key in [None, Some([111; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let source = tempdir().unwrap();
        let history = write_takeout_browser_json_fixture(source.path(), "history");
        import_takeout(
            &vault,
            &TakeoutRequest { source_path: history.display().to_string(), dry_run: false },
        )
        .unwrap();
        let before = watermark(&vault);
        assert!(before.0 > 0);
        assert!(before.2.is_some());

        let evidence = source.path().join("evidence/Chrome");
        fs::create_dir_all(&evidence).unwrap();
        // Auxiliary payloads are retained alongside an importable history file.
        // An empty history keeps this second import free of canonical batches.
        fs::write(evidence.join("BrowserHistory.json"), chrome_browser_history_payload(&[]))
            .unwrap();
        fs::write(
            evidence.join("Session.json"),
            r#"{"Session":[{"sessionTag":"device-1","tab":[{"tabId":1,"navigation":[{"virtualUrl":"https://example.com/one"}]}]}]}"#,
        )
        .unwrap();
        let report = import_takeout(
            &vault,
            &TakeoutRequest {
                source_path: source.path().join("evidence").display().to_string(),
                dry_run: false,
            },
        )
        .unwrap();
        assert_eq!(report.imported_items, 0);
        let batch = vault
            .read(Db::SourceEvidence, |s| {
                Ok(s.query_one(
                    "SELECT source_batch_id FROM native_entities WHERE entity_kind='takeout-session'",
                    (),
                    |r| Ok(r.get::<i64>(0)?),
                )?)
            })
            .unwrap();
        assert_eq!(watermark(&vault), (before.0, before.1, Some(batch)));
        assert_ne!(Some(batch), before.2);
    }
}

fn watermark(vault: &crate::Vault) -> (i64, i64, Option<i64>) {
    vault
        .read(Db::Archive, |s| {
            Ok(s.query_one(
                "SELECT last_visit_id,last_url_last_visit_time,last_source_batch_id
                 FROM profile_watermarks WHERE profile_id='takeout::browser-history'",
                (),
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap()
}
