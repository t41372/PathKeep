//! Responsibilities: pin the ZIP entries of a move file this build writes (format 3: the
//! released format 2 plus the agent-history stores) and real import into empty/populated owners.
//! Not responsible for: repairing today's Intelligence omission or authenticating v0.5 output.

use super::support::*;
use vault_store::Db;

#[test]
fn move_export_contains_five_databases_and_encrypted_portable_artifacts() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let bundle = fixture.root.join("exports/lifecycle.pathkeep");
        vault_worker::export_app_data(&fixture.vault(), bundle.clone(), EXPORT_PASSWORD).unwrap();
        let actual = observe(&fixture.root, Some(&fixture.vault()), &format!("export-{encrypted}"));
        assert_eq!(actual["unknown"], serde_json::json!([]));
        assert_eq!(actual, expected(&format!("export-{encrypted}")));
        let entries = zip_entries(&bundle);
        std::fs::write(
            fixture.sandbox.path().join("zip-entries.json"),
            serde_json::to_vec_pretty(&entries).unwrap(),
        )
        .unwrap();
        assert_eq!(
            entries,
            vec![
                "archive/agent-history-read-model.db",
                "archive/agent-history.db",
                "archive/agent.db",
                "archive/history-vault.db",
                "archive/keys.envelope",
                "archive/portable-artifacts.pkf",
                "archive/source-evidence.db",
                "config/config.json",
                "pathkeep-export-manifest.json",
                "pathkeep-export-manifest.sha256"
            ]
        );
    }
}

#[test]
fn empty_root_import_preserves_evidence_and_ask_but_loses_intelligence_metadata() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let bundle = fixture.root.join("exports/lifecycle.pathkeep");
        vault_worker::export_app_data(&fixture.vault(), bundle.clone(), EXPORT_PASSWORD).unwrap();
        let source = observe(
            &fixture.root,
            Some(&fixture.vault()),
            &format!("import-source-false-{encrypted}"),
        );
        assert_eq!(source["unknown"], serde_json::json!([]));
        assert_eq!(source, expected(&format!("export-{encrypted}")));
        let target = fixture.sandbox.path().join("destination");
        std::fs::create_dir_all(&target).unwrap();
        unsafe {
            std::env::set_var("CHB_PROJECT_ROOT", &target);
        }
        let session = vault_worker::VaultSession::default();
        let paths = vault_core::project_paths_with_root(&target);
        // No original browser is available to rescue a missing member during import.
        std::fs::remove_dir_all(fixture.sandbox.path().join("chrome")).unwrap();
        let preview = vault_worker::preview_import(&paths, bundle.clone()).unwrap();
        assert!(!preview.will_overwrite_existing);
        let (_, installed) = vault_worker::apply_import(
            &paths,
            session.current_vault().as_ref(),
            session.owner(),
            bundle,
            vault_core::ApplyImportOptions {
                confirm_overwrite: true,
                source_archive_key: Some(EXPORT_PASSWORD.into()),
            },
        )
        .unwrap();
        session
            .install_archive(
                installed,
                Some(EXPORT_PASSWORD.into()),
                vault_worker::KeychainAfterInstall::AsSaved,
            )
            .unwrap();
        let vault = session.vault().unwrap();
        let actual = observe(&target, Some(&vault), &format!("import-false-{encrypted}"));
        assert_eq!(actual["unknown"], serde_json::json!([]));
        assert_eq!(actual, expected("import-empty"));
        assert_eq!(store_marker(&vault, Db::Archive).as_deref(), Some("pkarchiveamber"));
        assert_eq!(store_marker(&vault, Db::SourceEvidence).as_deref(), Some("pkevidencebirch"));
        assert_eq!(store_marker(&vault, Db::Agent).as_deref(), Some("pkagentdenim"));
        assert_eq!(
            store_marker(&vault, Db::AgentHistoryEvidence).as_deref(),
            Some("pkagenthistoryivory")
        );
        assert_eq!(
            store_marker(&vault, Db::AgentHistoryReadModel).as_deref(),
            Some("pkreadmodeljade")
        );
        // Discrepancy I1 / Q1: format 2 drops original Intelligence choices and the saved
        // text locator even though the blob bytes themselves travel in the artifact TAR.
        assert_eq!(store_marker(&vault, Db::Intelligence), None);
        assert_eq!(
            vault
                .store()
                .read(Db::Intelligence, |s| s.query_one(
                    "SELECT count(*) FROM page_content_enrichments",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                ))
                .unwrap(),
            0
        );
        assert_eq!(
            vault
                .store()
                .read(Db::Intelligence, |s| s.query_one(
                    "SELECT count(*) FROM site_dictionary_overrides",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                ))
                .unwrap(),
            0
        );
        assert_eq!(
            vault_worker::load_ai_conversation(&vault, "lifecycle-chat").unwrap().unwrap().messages
                [0]
            .content,
            "pkaskhazel"
        );
        assert!(vault_worker::load_ai_conversation(&vault, "replaced-chat").unwrap().is_none());
        assert_eq!(search_titles(&vault), ["pksearchelm"]);
        assert_eq!(
            vault_core::load_config(&paths).unwrap().archive_mode,
            vault_core::ArchiveMode::Encrypted
        );
        session.close_vault().unwrap();
    }
}

#[test]
fn populated_root_import_replaces_derived_and_keeps_the_previous_archive() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        fixture.seed();
        let bundle = fixture.root.join("exports/lifecycle.pathkeep");
        vault_worker::export_app_data(&fixture.vault(), bundle.clone(), EXPORT_PASSWORD).unwrap();
        let source = observe(
            &fixture.root,
            Some(&fixture.vault()),
            &format!("import-source-true-{encrypted}"),
        );
        assert_eq!(source["unknown"], serde_json::json!([]));
        assert_eq!(source, expected(&format!("export-{encrypted}")));
        let target = fixture.sandbox.path().join("destination");
        std::fs::create_dir_all(&target).unwrap();
        unsafe {
            std::env::set_var("CHB_PROJECT_ROOT", &target);
        }
        let session = vault_worker::VaultSession::default();
        {
            let config =
                vault_core::load_config(fixture.session.current_vault().unwrap().paths()).unwrap();
            vault_worker::initialize_archive_database_with_progress(
                &session,
                &config,
                encrypted.then_some(PASSWORD),
                |_| {},
            )
            .unwrap();
            let vault = session.vault().unwrap();
            vault.search().set_paused_for_tests(true);
            vault_worker::run_backup_now(&vault, None, false).unwrap();
            for (db, marker) in [
                (Db::Archive, "targetarchive"),
                (Db::SourceEvidence, "targetevidence"),
                (Db::Intelligence, "targetintelligence"),
                (Db::Agent, "targetagent"),
                (Db::AgentHistoryEvidence, "targetagenthistory"),
                (Db::AgentHistoryReadModel, "targetreadmodel"),
            ] {
                vault
                    .store()
                    .write(db, vault_store::Priority::Interactive, move |s| {
                        s.execute("CREATE TABLE lifecycle_marker(body TEXT NOT NULL)", ())?;
                        s.execute("INSERT INTO lifecycle_marker VALUES(?1)", [marker])?;
                        Ok(())
                    })
                    .unwrap();
            }
            vault_worker::save_ai_conversation(
                &vault,
                &vault_core::SaveAgentConversationRequest {
                    id: "replaced-chat".into(),
                    title: Some("replaced".into()),
                    provider_id: None,
                    messages: vec![],
                },
            )
            .unwrap();
            let before =
                observe(&target, Some(&vault), &format!("import-target-before-{encrypted}"));
            assert_eq!(before["unknown"], serde_json::json!([]));
            assert_eq!(before, expected(&format!("import-target-before-{encrypted}")));
        }
        let paths = vault_core::project_paths_with_root(&target);
        // No original browser is available to rescue a missing member during import.
        std::fs::remove_dir_all(fixture.sandbox.path().join("chrome")).unwrap();
        let preview = vault_worker::preview_import(&paths, bundle.clone()).unwrap();
        assert!(preview.will_overwrite_existing);
        let (_, installed) = vault_worker::apply_import(
            &paths,
            session.current_vault().as_ref(),
            session.owner(),
            bundle,
            vault_core::ApplyImportOptions {
                confirm_overwrite: true,
                source_archive_key: Some(EXPORT_PASSWORD.into()),
            },
        )
        .unwrap();
        session
            .install_archive(
                installed,
                Some(EXPORT_PASSWORD.into()),
                vault_worker::KeychainAfterInstall::AsSaved,
            )
            .unwrap();
        let vault = session.vault().unwrap();
        let actual = observe(&target, Some(&vault), &format!("import-true-{encrypted}"));
        assert_eq!(actual["unknown"], serde_json::json!([]));
        assert_eq!(actual, expected(&format!("import-true-{encrypted}")));
        assert_eq!(store_marker(&vault, Db::Archive).as_deref(), Some("pkarchiveamber"));
        assert_eq!(store_marker(&vault, Db::SourceEvidence).as_deref(), Some("pkevidencebirch"));
        assert_eq!(store_marker(&vault, Db::Agent).as_deref(), Some("pkagentdenim"));
        assert_eq!(
            store_marker(&vault, Db::AgentHistoryEvidence).as_deref(),
            Some("pkagenthistoryivory")
        );
        assert_eq!(
            store_marker(&vault, Db::AgentHistoryReadModel).as_deref(),
            Some("pkreadmodeljade")
        );
        // Discrepancy I1 / Q1: format 2 drops original Intelligence choices and the saved
        // text locator even though the blob bytes themselves travel in the artifact TAR.
        assert_eq!(store_marker(&vault, Db::Intelligence), None);
        assert_eq!(
            vault
                .store()
                .read(Db::Intelligence, |s| s.query_one(
                    "SELECT count(*) FROM page_content_enrichments",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                ))
                .unwrap(),
            0
        );
        assert_eq!(
            vault
                .store()
                .read(Db::Intelligence, |s| s.query_one(
                    "SELECT count(*) FROM site_dictionary_overrides",
                    (),
                    |r| Ok(r.get::<i64>(0)?)
                ))
                .unwrap(),
            0
        );
        assert_eq!(
            vault_worker::load_ai_conversation(&vault, "lifecycle-chat").unwrap().unwrap().messages
                [0]
            .content,
            "pkaskhazel"
        );
        assert!(vault_worker::load_ai_conversation(&vault, "replaced-chat").unwrap().is_none());
        assert_eq!(search_titles(&vault), ["pksearchelm"]);
        assert_eq!(
            vault_core::load_config(&paths).unwrap().archive_mode,
            vault_core::ArchiveMode::Encrypted
        );
        session.close_vault().unwrap();
    }
}
