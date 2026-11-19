//! Exercises the conversation service through the same archive operations as the worker.
//! Expected bytes come from the synthetic native files. No installed tool is consulted.

use super::support::*;
use agent_history::{Archive, SourceRoot, capture::BackupOptions};
use std::{fs, io::Read, sync::Arc};
use vault_core::{ArchiveMode, Vault};

#[path = "conversations/corpus.rs"]
mod corpus;
use corpus::{Corpus, fixtures, query, sessions};

fn stale(archive: &Archive) {
    assert!(archive.list_sessions(&Default::default()).is_err());
    assert!(archive.search(&query()).is_err());
    assert!(archive.rebuild_search().is_err());
    assert!(archive.start_backup_run().is_err());
}

#[test]
fn reopen_and_password_change() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        let vault = fixture.vault();
        let archive = vault.agent_history().unwrap();
        assert!(Arc::ptr_eq(
            &archive,
            &vault.with_config(vault.config().clone()).agent_history().unwrap()
        ));
        let corpus = Corpus::seed(&fixture);
        corpus.check(&vault, encrypted.then_some(PASSWORD));
        let mut reader = archive
            .raw_reader(archive.list_sessions(&Default::default()).unwrap().items[0].revision_id)
            .unwrap();
        reader.read_exact(&mut [0]).unwrap();
        let run = archive.start_backup_run().unwrap();
        fixture.session.close_vault().unwrap();
        stale(&archive);
        assert!(reader.read_exact(&mut [0]).is_err());
        assert!(vault.agent_history().is_err());
        let reopened = fixture.reopen(encrypted);
        assert_eq!(
            reopened.agent_history().unwrap().backup_run(run).unwrap().status,
            "interrupted"
        );
        corpus.check(&reopened, encrypted.then_some(PASSWORD));
        fixture.rekey(
            reopened.config().archive_mode.clone(),
            encrypted.then_some(PASSWORD),
            Some(NEW_PASSWORD),
        );
        corpus.check(&fixture.vault(), encrypted.then_some(NEW_PASSWORD));
        fixture.session.close_vault().unwrap();
        let paths = vault.paths().clone();
        let reopened = if encrypted {
            fixture
                .session
                .open_password(
                    paths.clone(),
                    vault_core::load_config(&paths).unwrap(),
                    NEW_PASSWORD.into(),
                )
                .unwrap()
        } else {
            fixture.reopen(false)
        };
        corpus.check(&reopened, encrypted.then_some(NEW_PASSWORD));
    }
}

#[test]
fn mode_switch_snapshot_and_restore() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(!encrypted);
        fixture.create_browser();
        assert_eq!(fixture.backup().run.unwrap().new_visits, 1);
        let mut corpus = Corpus::seed(&fixture);
        let old = fixture.vault().agent_history().unwrap();
        fixture.rekey(
            if encrypted { ArchiveMode::Encrypted } else { ArchiveMode::Plaintext },
            (!encrypted).then_some(PASSWORD),
            encrypted.then_some(NEW_PASSWORD),
        );
        stale(&old);
        let password = encrypted.then_some(NEW_PASSWORD);
        corpus.check(&fixture.vault(), password);
        let snapshot = safety_snapshot();
        // Open a disposable copy. Opening and synchronizing the sealed snapshot itself would modify it.
        let copy = fixture.sandbox.path().join("snapshot-copy");
        for name in expected_root::files_under(&snapshot).unwrap() {
            if name != "snapshot.json" {
                let destination = copy.join(&name);
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(snapshot.join(name), destination).unwrap();
            }
        }
        let paths = vault_core::project_paths_with_root(&copy);
        let key = password.map(|p| Vault::unlock_data_key(&paths, p).unwrap());
        let saved = Vault::open_staged(
            paths.clone(),
            vault_core::load_config(&paths).unwrap(),
            key.as_ref(),
        )
        .unwrap();
        corpus.check(&saved, password);
        saved.close().unwrap();
        corpus.retitle(&fixture.vault());
        fixture
            .vault()
            .store()
            .write(vault_store::Db::Archive, vault_store::Priority::Interactive, |s| {
                assert_eq!(
                    s.execute(
                        "UPDATE urls SET title='After the snapshot' WHERE title='pksearchelm'",
                        ()
                    )?,
                    1
                );
                Ok(())
            })
            .unwrap();
        let old = fixture.vault().agent_history().unwrap();
        vault_worker::run_full_archive_restore(
            &fixture.session,
            password,
            &vault_core::SnapshotRestoreRequest { snapshot_path: snapshot.display().to_string() },
        )
        .unwrap();
        stale(&old);
        corpus.check(&fixture.vault(), password);
        let title: String = fixture
            .vault()
            .store()
            .read(vault_store::Db::Archive, |s| {
                s.query_one(
                    "SELECT title FROM urls WHERE url='https://lifecycle.example.test/page'",
                    (),
                    |r| Ok(r.get(0)?),
                )
            })
            .unwrap();
        assert_eq!(title, "pksearchelm");
    }
}

#[test]
fn move_to_empty_and_populated_destinations() {
    for encrypted in [false, true] {
        let fixture = Fixture::new(encrypted);
        let corpus = Corpus::seed(&fixture);
        let vault = fixture.vault();
        let bundle = fixture.root.join("exports/real-conversations.pathkeep");
        vault_worker::export_app_data(&vault, bundle.clone(), EXPORT_PASSWORD).unwrap();
        corpus.check(&vault, encrypted.then_some(PASSWORD));
        for populated in [false, true] {
            let target = fixture.sandbox.path().join(format!("destination-{populated}"));
            fs::create_dir_all(&target).unwrap();
            unsafe {
                std::env::set_var("CHB_PROJECT_ROOT", &target);
            }
            let session = vault_worker::VaultSession::default();
            let paths = vault_core::project_paths_with_root(&target);
            if populated {
                vault_worker::initialize_archive_database_with_progress(
                    &session,
                    vault.config(),
                    encrypted.then_some(PASSWORD),
                    |_| {},
                )
                .unwrap();
                let other = fixture.sandbox.path().join("destination-pi");
                let file = fixtures::pi(
                    &other,
                    "/other-project",
                    "destination-session",
                    "destination only",
                );
                // A plaintext destination's safety copy legitimately retains its own messages.
                let text =
                    fs::read_to_string(&file).unwrap().replace(fixtures::PRIVATE_TEXT, "othertext");
                fs::write(file, text).unwrap();
                let history = session.vault().unwrap().agent_history().unwrap();
                history
                    .backup(
                        &[SourceRoot { harness: "pi".parse().unwrap(), root: other }],
                        &BackupOptions { read_git: false, ..Default::default() },
                    )
                    .unwrap();
                assert_eq!(sessions(&history)["pi"]["native_id"], "destination-session");
            }
            let current = session.current_vault();
            let old = current.as_ref().map(|v| v.agent_history().unwrap());
            let preview = vault_worker::preview_import(&paths, bundle.clone()).unwrap();
            assert_eq!(preview.will_overwrite_existing, populated);
            let lease = session.begin_maintenance().unwrap();
            let (_, installed) = vault_worker::apply_import(
                &paths,
                current.as_ref(),
                session.owner(),
                bundle.clone(),
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
            drop(lease);
            if let Some(old) = old {
                stale(&old);
            }
            corpus.check(&session.vault().unwrap(), Some(EXPORT_PASSWORD));
            session.close_vault().unwrap();
        }
    }
}

#[path = "conversations/ownership.rs"]
mod ownership;

#[path = "conversations/damage.rs"]
mod damage;
