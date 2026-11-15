//! Responsibilities: pin actual safety-snapshot membership and whole-archive restore results.
//! Not responsible for: old release fixtures or a manual snapshot command (none exists today).
//! The real rekey command produces the safety snapshot; restore uses the worker command.

use super::support::*;
use vault_core::ArchiveMode;
use vault_store::{Db, Priority};

#[test]
fn safety_snapshot_copies_six_durable_databases_but_no_search_or_loose_artifacts() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(!encrypted);
        fixture.seed();
        fixture.rekey(
            if encrypted { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext },
            (!encrypted).then_some(PASSWORD),
            encrypted.then_some(NEW_PASSWORD),
        );
        let snapshot = safety_snapshot();
        let actual =
            observe(&fixture.root, Some(&fixture.vault()), &format!("snapshot-{encrypted}"));
        assert_eq!(actual["unknown"], serde_json::json!([]));
        assert_eq!(actual, expected(&format!("mode-after-{}", !encrypted)));
        let mut members = expected_root::files_under(&snapshot).unwrap();
        members.sort();
        let mut expected = vec![
            "archive/agent-history.db",
            "archive/history-vault.db",
            "archive/source-evidence.db",
            "config.json",
            "derived/agent-history-read-model.db",
            "derived/agent.db",
            "derived/intelligence.db",
            "snapshot.json",
        ];
        if encrypted {
            expected.push("archive/keys.envelope");
        }
        expected.sort();
        assert_eq!(members, expected);
        let proof: serde_json::Value =
            serde_json::from_slice(&std::fs::read(snapshot.join("snapshot.json")).unwrap())
                .unwrap();
        let mut proof_members: Vec<_> =
            proof.as_array().unwrap().iter().map(|entry| entry["path"].as_str().unwrap()).collect();
        proof_members.sort();
        expected.retain(|p| *p != "snapshot.json");
        assert_eq!(proof_members, expected);
    }
}

#[test]
fn restore_recovers_evidence_intelligence_and_ask_and_keeps_the_replaced_generation() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(!encrypted);
        fixture.seed();
        fixture.rekey(
            if encrypted { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext },
            (!encrypted).then_some(PASSWORD),
            encrypted.then_some(NEW_PASSWORD),
        );
        let snapshot = safety_snapshot();
        let vault = fixture.vault();
        for db in [
            Db::Archive,
            Db::SourceEvidence,
            Db::Intelligence,
            Db::Agent,
            Db::AgentHistoryEvidence,
            Db::AgentHistoryReadModel,
        ] {
            vault
                .store()
                .write(db, Priority::Interactive, |s| {
                    s.execute("UPDATE lifecycle_marker SET body='discardedgeneration'", ())?;
                    Ok(())
                })
                .unwrap();
        }
        vault_worker::delete_ai_conversation(&vault, "lifecycle-chat").unwrap();
        let before = observe(&fixture.root, Some(&vault), &format!("restore-before-{encrypted}"));
        assert_eq!(before["unknown"], serde_json::json!([]));
        assert_eq!(before, expected(&format!("restore-before-{encrypted}")));
        let report = vault_worker::run_full_archive_restore(
            &fixture.session,
            encrypted.then_some(NEW_PASSWORD),
            &vault_core::SnapshotRestoreRequest { snapshot_path: snapshot.display().to_string() },
        )
        .unwrap();
        assert!(!report.source_evidence_rebuilt);
        // Discrepancy: archive_flows.rs describes empty evidence reconstruction. Native restore
        // actually restores the captured evidence; only legacy single-file restore rebuilds it.
        let restored = fixture.vault();
        let actual = observe(&fixture.root, Some(&restored), &format!("restore-after-{encrypted}"));
        assert_eq!(actual["unknown"], serde_json::json!([]));
        assert_eq!(actual, expected(&format!("restore-after-{encrypted}")));
        assert_eq!(store_marker(&restored, Db::Archive).as_deref(), Some("pkarchiveamber"));
        assert_eq!(store_marker(&restored, Db::SourceEvidence).as_deref(), Some("pkevidencebirch"));
        assert_eq!(
            store_marker(&restored, Db::Intelligence).as_deref(),
            Some("pkintelligencecoral")
        );
        assert_eq!(store_marker(&restored, Db::Agent).as_deref(), Some("pkagentdenim"));
        // A restore does not roll agent conversations back: they are as they were before it.
        assert_eq!(
            store_marker(&restored, Db::AgentHistoryEvidence).as_deref(),
            Some("discardedgeneration")
        );
        assert_eq!(
            store_marker(&restored, Db::AgentHistoryReadModel).as_deref(),
            Some("discardedgeneration")
        );
        assert_eq!(search_titles(&restored), ["pksearchelm"]);
        assert_eq!(
            vault_worker::load_ai_conversation(&restored, "lifecycle-chat")
                .unwrap()
                .unwrap()
                .messages[0]
                .content,
            "pkaskhazel"
        );
        assert!(
            std::path::Path::new(&report.quarantine_dir).join("archive/history-vault.db").is_file()
        );
    }
}
