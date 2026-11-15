//! Responsibilities: pin both mode changes and envelope-only password changes on the owner.
//! Not responsible for: fixing the plaintext historical-artifact gap or injecting power loss.

use super::support::*;
use vault_core::ArchiveMode;
use vault_store::Db;

#[test]
fn changing_mode_preserves_all_six_stores_and_rebuilds_search() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let before =
            observe(&fixture.root, Some(&fixture.vault()), &format!("mode-before-{encrypted}"));
        assert_eq!(before["unknown"], serde_json::json!([]));
        assert_eq!(before, expected(&format!("backup-{encrypted}")));
        fixture.rekey(
            if encrypted { ArchiveMode::Plaintext } else { ArchiveMode::Encrypted },
            encrypted.then_some(PASSWORD),
            (!encrypted).then_some(NEW_PASSWORD),
        );
        let vault = fixture.vault();
        let after = observe(&fixture.root, Some(&vault), &format!("mode-after-{encrypted}"));
        assert_eq!(after["unknown"], serde_json::json!([]));
        assert_eq!(after, expected(&format!("mode-after-{encrypted}")));
        assert_eq!(store_marker(&vault, Db::Archive).as_deref(), Some("pkarchiveamber"));
        assert_eq!(store_marker(&vault, Db::SourceEvidence).as_deref(), Some("pkevidencebirch"));
        assert_eq!(store_marker(&vault, Db::Intelligence).as_deref(), Some("pkintelligencecoral"));
        assert_eq!(store_marker(&vault, Db::Agent).as_deref(), Some("pkagentdenim"));
        assert_eq!(
            store_marker(&vault, Db::AgentHistoryEvidence).as_deref(),
            Some("pkagenthistoryivory")
        );
        assert_eq!(
            store_marker(&vault, Db::AgentHistoryReadModel).as_deref(),
            Some("pkreadmodeljade")
        );
        assert_eq!(search_titles(&vault), ["pksearchelm"]);
        assert_eq!(
            vault_worker::load_ai_conversation(&vault, "lifecycle-chat").unwrap().unwrap().messages
                [0]
            .content,
            "pkaskhazel"
        );
        assert_eq!(
            vault_core::load_config(vault.paths()).unwrap().archive_mode,
            if encrypted { ArchiveMode::Plaintext } else { ArchiveMode::Encrypted }
        );
        // Discrepancy Q2: mode conversion leaves saved text and browser checkpoints plaintext;
        // the newly created safety snapshot is in the destination mode, not the old mode.
    }
}

#[test]
fn password_change_rewraps_the_key_without_rewriting_any_database() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let vault = fixture.vault();
        let before = observe(&fixture.root, Some(&vault), &format!("password-before-{encrypted}"));
        assert_eq!(before["unknown"], serde_json::json!([]));
        assert_eq!(before, expected(&format!("backup-{encrypted}")));
        let files_before: Vec<_> = expected_root::files_under(&fixture.root)
            .unwrap()
            .into_iter()
            .filter(|path| path != "archive/keys.envelope")
            .map(|path| {
                let bytes = std::fs::read(fixture.root.join(&path)).unwrap();
                (path, bytes)
            })
            .collect();
        let key_before =
            encrypted.then(|| vault_core::Vault::unlock_data_key(vault.paths(), PASSWORD).unwrap());
        fixture.rekey(
            if encrypted { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext },
            encrypted.then_some(PASSWORD),
            Some(NEW_PASSWORD),
        );
        let after = observe(&fixture.root, Some(&vault), &format!("password-after-{encrypted}"));
        assert_eq!(after["unknown"], serde_json::json!([]));
        assert_eq!(after, expected(&format!("backup-{encrypted}")));
        assert_eq!(after, before);
        for (path, bytes) in files_before {
            assert_eq!(std::fs::read(fixture.root.join(&path)).unwrap(), bytes, "{path}");
        }
        if let Some(key) = key_before {
            let changed = vault_core::Vault::unlock_data_key(vault.paths(), NEW_PASSWORD).unwrap();
            assert_eq!(changed.expose_bytes(), key.expose_bytes());
            assert!(vault_core::Vault::unlock_data_key(vault.paths(), PASSWORD).is_err());
        } else {
            // Plaintext has no password: today's same-mode rekey request is a no-op.
            assert!(!fixture.root.join("archive/keys.envelope").exists());
        }
    }
}
