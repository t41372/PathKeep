//! Exercises partial admission, maintenance refusal and restart on real archives.
//! Source generation and conversation content comparisons belong to the shared corpus.

use super::*;
use std::{collections::BTreeMap, io::Write, path::Path};
use vault_store::{Db, Priority};

const CONVERSATIONS: [Db; 2] = [Db::AgentHistoryEvidence, Db::AgentHistoryReadModel];

#[derive(Clone, Copy, Debug)]
enum Damage {
    Truncated,
    Header,
    Directory,
    Missing,
    #[cfg(unix)]
    Unreadable,
    Migration,
}

const DAMAGE: &[Damage] = &[
    Damage::Truncated,
    Damage::Header,
    Damage::Directory,
    Damage::Missing,
    #[cfg(unix)]
    Damage::Unreadable,
    Damage::Migration,
];

fn spoil(fixture: &Fixture, db: Db, damage: Damage) {
    // Re-run an existing DDL step against its already-created schema. This reaches the real
    // migration transaction in plaintext and encrypted mode, with no injected engine error.
    if matches!(damage, Damage::Migration) {
        let vault = fixture.vault();
        let decoded = fixture.sandbox.path().join("migration-input");
        let file = decoded.join(vault_layout::database_path(db));
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        vault
            .store()
            .pause()
            .unwrap()
            .copy_database(db, file.clone(), None, Default::default(), |_| {})
            .unwrap();
        rusqlite::Connection::open(&file)
            .unwrap()
            .execute("DELETE FROM __pk_migrations", ())
            .unwrap();
        let key = (vault.config().archive_mode == ArchiveMode::Encrypted).then(|| {
            *Vault::unlock_data_key(vault.paths(), PASSWORD)
                .unwrap()
                .derive(vault_layout::database_spec(db).key)
                .expose_bytes()
        });
        let mut config = vault_store::Config::new(&decoded);
        config.databases = vec![Db::Scratch, db];
        let input = vault_store::Store::open(config).unwrap();
        let output = fixture.sandbox.path().join("unmigrated.db");
        input
            .pause()
            .unwrap()
            .copy_database(db, output.clone(), key, Default::default(), |_| {})
            .unwrap();
        fixture.session.close_vault().unwrap();
        fs::copy(output, fixture.root.join(vault_layout::database_path(db))).unwrap();
        return;
    }
    fixture.session.close_vault().unwrap();
    let path = fixture.root.join(vault_layout::database_path(db));
    match damage {
        Damage::Truncated => fs::write(path, b"bad").unwrap(),
        Damage::Header => {
            fs::OpenOptions::new().write(true).open(path).unwrap().write_all(&[0x55; 16]).unwrap()
        }
        Damage::Directory => {
            fs::remove_file(&path).unwrap();
            fs::create_dir(path).unwrap();
        }
        Damage::Missing => fs::remove_file(path).unwrap(),
        #[cfg(unix)]
        Damage::Unreadable => {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
            assert_eq!(
                fs::File::open(path).unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
        }
        Damage::Migration => {}
    }
}

#[cfg(unix)]
fn unreadable_digest(path: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let original = path.metadata().unwrap().permissions();
    assert_eq!(original.mode() & 0o777, 0);
    // Only the fixture owner can do this. Restore the denial before the open or retry.
    fs::set_permissions(path, fs::Permissions::from_mode(0o400)).unwrap();
    let bytes = fs::read(path).unwrap();
    fs::set_permissions(path, original).unwrap();
    format!("unreadable {}", vault_core::utils::sha256_hex(&bytes))
}

fn hashes(root: &Path) -> BTreeMap<String, String> {
    expected_root::files_under(root)
        .unwrap()
        .into_iter()
        .map(|path| {
            let digest = match fs::read(root.join(&path)) {
                Ok(bytes) => vault_core::utils::sha256_hex(&bytes),
                #[cfg(unix)]
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    unreadable_digest(&root.join(&path))
                }
                Err(_) if read_unless_locked(root, &path).is_none() => "locked".into(),
                Err(e) => panic!("{path}: {e}"),
            };
            (path, digest)
        })
        .collect()
}

fn open(fixture: &Fixture, encrypted: bool) -> anyhow::Result<Vault> {
    let paths = vault_core::project_paths_with_root(&fixture.root);
    let config = vault_core::load_config(&paths)?;
    if encrypted {
        Ok(Vault::open_password(paths, config, PASSWORD, None)?.0)
    } else {
        Ok(Vault::open(paths, config, None)?)
    }
}

#[test]
fn conversation_files_fail_as_one_group_and_browsing_works() {
    for encrypted in [false, true] {
        for role in CONVERSATIONS {
            for &damage in DAMAGE {
                let f = Fixture::new(encrypted);
                let _corpus = Corpus::seed(&f);
                spoil(&f, role, damage);
                let before = hashes(&f.root);
                let vault = open(&f, encrypted).unwrap();
                let failure = vault.conversation_availability().unwrap_err();
                assert_eq!(failure.role, role);
                assert_eq!(
                    failure.stage,
                    if matches!(damage, Damage::Migration) {
                        vault_store::OpenStage::Migrate
                    } else {
                        vault_store::OpenStage::Preflight
                    }
                );
                assert!(
                    matches!(vault.agent_history(), Err(agent_history::Error::Store(vault_store::Error::Open(reason))) if Arc::ptr_eq(&reason, &failure))
                );
                for db in CONVERSATIONS {
                    assert_eq!(vault.store().database_encrypted(db), None);
                }
                assert_eq!(
                    vault.available_kinds(),
                    [vault_layout::DataScope::Browsing, vault_layout::DataScope::Assistant]
                );
                if matches!(damage, Damage::Directory) {
                    assert!(f.root.join(vault_layout::database_path(role)).is_dir());
                }
                let during = hashes(&f.root);
                assert!(during.keys().eq(before.keys()), "{role:?} {damage:?}: membership changed");
                for (path, digest) in during {
                    // A Windows mandatory sentinel lock can only be hashed again after close.
                    if digest != "locked" {
                        assert_eq!(digest, before[&path], "{path}");
                    }
                }
                vault.close().unwrap();
                assert_eq!(hashes(&f.root), before);
                f.reopen(encrypted);
                f.create_browser();
                assert_eq!(f.backup().run.unwrap().new_visits, 1);
                let title: String = f.vault().store().read(Db::Archive, |s| {
                    s.query_one("SELECT title FROM urls WHERE url='https://lifecycle.example.test/page'", (), |r| Ok(r.get(0)?))
                }).unwrap();
                assert_eq!(title, "pksearchelm");
            }
        }
    }
}

#[test]
fn browsing_file_damage_refuses_the_archive() {
    for encrypted in [false, true] {
        for &damage in DAMAGE {
            let f = Fixture::new(encrypted);
            let _corpus = Corpus::seed(&f);
            spoil(&f, Db::SourceEvidence, damage);
            let error = open(&f, encrypted).unwrap_err();
            assert!(
                matches!(error.downcast_ref::<vault_store::Error>(), Some(vault_store::Error::Open(reason)) if reason.role == Db::SourceEvidence),
                "{error:#}"
            );
        }
    }
}

#[test]
fn shared_failures_win_over_conversation_damage() {
    let f = Fixture::new(true);
    let paths = f.vault().paths().clone();
    let config = f.vault().config().clone();
    let key = Vault::unlock_data_key(&paths, PASSWORD).unwrap();
    spoil(&f, CONVERSATIONS[0], Damage::Missing);
    let password_open = open(&f, true).unwrap();
    let password_failure = password_open.conversation_availability().unwrap_err();
    password_open.close().unwrap();
    let raw = Vault::open(paths.clone(), config.clone(), Some(*key.expose_bytes())).unwrap();
    let raw_failure = raw.conversation_availability().unwrap_err();
    assert_eq!(raw_failure.role, CONVERSATIONS[0]);
    assert_eq!(
        (raw_failure.role, raw_failure.stage),
        (password_failure.role, password_failure.stage)
    );
    raw.close().unwrap();
    let wrong = Vault::open_password(paths.clone(), config.clone(), "incorrect", None).unwrap_err();
    assert!(wrong.is::<vault_core::ArchivePasswordWrong>());
    let wrong = Vault::open(paths.clone(), config.clone(), Some([42; 32])).unwrap_err();
    assert!(matches!(wrong, vault_store::Error::Open(reason)
        if !CONVERSATIONS.contains(&reason.role) && reason.stage == vault_store::OpenStage::KeyCheck));
    let envelope = paths.artifact_path(vault_layout::ArtifactId::Envelope);
    let saved = fs::read(&envelope).unwrap();
    fs::remove_file(&envelope).unwrap();
    for missing in [true, false] {
        if !missing {
            fs::write(&envelope, b"invalid").unwrap();
        }
        assert!(open(&f, true).is_err());
        assert!(Vault::open_owned(paths.clone(), config.clone(), Some(&key), None).is_err());
    }
    fs::write(envelope, saved).unwrap();
    let mut control: serde_json::Value =
        serde_json::from_slice(&fs::read(&paths.config_path).unwrap()).unwrap();
    control["archiveMode"]["compatibility"]["revision"] = 100.into();
    control["archiveMode"]["compatibility"]["minWriter"] = 100.into();
    fs::write(&paths.config_path, serde_json::to_vec(&control).unwrap()).unwrap();
    let refused = open(&f, true).unwrap_err();
    assert!(
        refused.chain().any(|e| e
            .downcast_ref::<vault_core::admission::Refusal>()
            .is_some_and(|r| r.reason == vault_core::admission::Reason::WriterTooOld)),
        "{refused:#}"
    );
    for missing in [true, false] {
        if missing {
            fs::remove_file(&paths.config_path).unwrap();
        } else {
            fs::write(&paths.config_path, b"broken control").unwrap();
        }
        assert!(Vault::open_owned(paths.clone(), config.clone(), Some(&key), None).is_err());
    }
}

#[test]
fn remembered_key_opens_browsing_with_a_damaged_conversation_store_and_envelope_tag() {
    let f = Fixture::new(true);
    let paths = f.vault().paths().clone();
    let key = Vault::unlock_data_key(&paths, PASSWORD).unwrap();
    vault_worker::store_data_key_in_keyring(&key).unwrap();
    spoil(&f, Db::AgentHistoryReadModel, Damage::Header);

    let envelope = paths.artifact_path(vault_layout::ArtifactId::Envelope);
    let mut bytes = fs::read(&envelope).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&envelope, &bytes).unwrap();
    let password_error = open(&f, true).unwrap_err();
    assert!(password_error.is::<vault_core::ArchivePasswordWrong>());
    f.session.unlock_from_keychain().unwrap();
    assert_eq!(f.vault().conversation_availability().unwrap_err().role, Db::AgentHistoryReadModel);
    f.create_browser();
    assert_eq!(f.backup().run.unwrap().new_visits, 1);
    let title: String = f
        .vault()
        .store()
        .read(Db::Archive, |s| {
            s.query_one(
                "SELECT title FROM urls WHERE url='https://lifecycle.example.test/page'",
                (),
                |r| Ok(r.get(0)?),
            )
        })
        .unwrap();
    assert_eq!(title, "pksearchelm");
    assert_eq!(fs::read(envelope).unwrap(), bytes);
}

#[test]
fn plaintext_partial_open_refuses_a_key_envelope() {
    let f = Fixture::new(false);
    let envelope = f.vault().paths().artifact_path(vault_layout::ArtifactId::Envelope);
    spoil(&f, Db::AgentHistoryEvidence, Damage::Missing);
    fs::write(envelope, b"unexpected envelope").unwrap();
    assert!(open(&f, false).is_err());
}

#[test]
fn crash_between_groups_can_reopen() {
    const CHILD: &str = "PATHKEEP_GROUP_CRASH_MODE";
    if let Ok(mode) = std::env::var(CHILD) {
        vault_core::fault_inject::exit_process_at_next_checkpoint("browsing-group-open");
        let paths = vault_core::project_paths().unwrap();
        let config = vault_core::load_config(&paths).unwrap();
        if mode == "true" {
            Vault::open_password(paths, config, PASSWORD, None).unwrap();
        } else {
            Vault::open(paths, config, None).unwrap();
        }
        panic!("missed the crash boundary");
    }
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let corpus = Corpus::seed(&f);
        f.session.close_vault().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "conversations::damage::crash_between_groups_can_reopen"])
            .env(CHILD, encrypted.to_string())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86));
        corpus.check(&f.reopen(encrypted), encrypted.then_some(PASSWORD));
    }
}

#[test]
fn maintenance_waits_for_retry() {
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let corpus = Corpus::seed(&f);
        let path = f.root.join(vault_layout::database_path(CONVERSATIONS[1]));
        let safety = f.sandbox.path().join("saved-before-damage");
        vault_core::maintenance::snapshot(&f.vault(), &safety).unwrap();
        f.session.close_vault().unwrap();
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"bad").unwrap();
        let vault = f.reopen(encrypted);
        let snapshot = f.sandbox.path().join("snapshot");
        let bundle = f.sandbox.path().join("move.pathkeep");
        let mode = if encrypted { ArchiveMode::Plaintext } else { ArchiveMode::Encrypted };
        let switch = || {
            vault_worker::rekey_archive_database(
                &f.session,
                &vault_worker::RekeyRequest {
                    new_mode: mode.clone(),
                    current_key: encrypted.then(|| PASSWORD.into()),
                    new_key: (!encrypted).then(|| NEW_PASSWORD.into()),
                    remember_in_keychain: None,
                },
            )
        };
        let before = hashes(&f.root);
        assert!(vault.retry_conversations().is_err());
        let failure = vault.conversation_availability().unwrap_err();
        for error in [
            vault_core::maintenance::snapshot(&vault, &snapshot).unwrap_err(),
            vault_core::maintenance::restore_snapshot(
                &vault,
                &safety,
                encrypted.then_some(PASSWORD),
                Default::default(),
            )
            .unwrap_err(),
            vault_worker::export_app_data(&vault, bundle.clone(), EXPORT_PASSWORD).unwrap_err(),
            vault_worker::reconcile_archive_encryption(&vault).unwrap_err(),
            switch().unwrap_err(),
        ] {
            assert!(
                matches!(error.downcast_ref::<vault_store::Error>(), Some(vault_store::Error::Open(reason)) if Arc::ptr_eq(reason, &failure)),
                "{error:#}"
            );
        }
        assert!(!snapshot.exists() && !bundle.exists());
        assert_eq!(hashes(&f.root), before);
        vault
            .store()
            .write(Db::Scratch, Priority::Interactive, |s| {
                s.execute("CREATE TABLE retry_marker(id INTEGER)", ())?;
                Ok(())
            })
            .unwrap();
        fs::write(path, original).unwrap();
        vault.retry_conversations().unwrap();
        // Retry on this owner; re-opening it would discard retry_marker.
        corpus.check(&vault, encrypted.then_some(PASSWORD));
        vault
            .store()
            .read(Db::Scratch, |s| {
                s.query_one("SELECT count(*) FROM retry_marker", (), |r| Ok(r.get::<i64>(0)?))
            })
            .unwrap();
        vault
            .store()
            .read(Db::AgentHistoryReadModel, |s| {
                s.query_one("SELECT count(*) FROM scratch.retry_marker", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })
            })
            .unwrap();
        vault_core::maintenance::snapshot(&vault, &snapshot).unwrap();
        vault_worker::export_app_data(&vault, bundle, EXPORT_PASSWORD).unwrap();
        vault_worker::reconcile_archive_encryption(&vault).unwrap();
        switch().unwrap();
        vault_worker::run_full_archive_restore(
            &f.session,
            (!encrypted).then_some(NEW_PASSWORD),
            &vault_core::SnapshotRestoreRequest {
                snapshot_path: safety_snapshot().display().to_string(),
            },
        )
        .unwrap();
        corpus.check(&f.vault(), (!encrypted).then_some(NEW_PASSWORD));
    }
}

#[test]
fn wipe_includes_the_unavailable_group() {
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        let _corpus = Corpus::seed(&f);
        spoil(&f, CONVERSATIONS[0], Damage::Directory);
        assert!(f.reopen(encrypted).conversation_availability().is_err());
        vault_worker::wipe_all_data(vault_worker::WIPE_CONFIRMATION_WORD, || {
            f.session.close_vault().map_err(anyhow::Error::msg)
        })
        .unwrap();
        assert_eq!(
            observe(&f.root, None, "wipe-with-unavailable-conversations"),
            expected("wiped")
        );
    }
}

#[test]
fn lazy_recovery_failure_names_the_role_and_blocks_snapshot() {
    for (db, table) in
        [(CONVERSATIONS[0], "ah_backup_runs"), (CONVERSATIONS[1], "ah_projection_work")]
    {
        let f = Fixture::new(false);
        let corpus = Corpus::seed(&f);
        f.vault()
            .store()
            .write(db, Priority::Interactive, move |s| {
                s.execute(&format!("ALTER TABLE {table} RENAME TO temporarily_missing"), ())?;
                Ok(())
            })
            .unwrap();
        f.session.close_vault().unwrap();
        let vault = f.reopen(false);
        assert!(vault.agent_history().is_err());
        let why = vault.conversation_availability().unwrap_err();
        assert_eq!((why.role, why.stage), (db, vault_store::OpenStage::Recover));
        assert!(
            vault_core::maintenance::snapshot(&vault, &f.sandbox.path().join("snapshot")).is_err()
        );
        vault
            .store()
            .write(db, Priority::Interactive, move |s| {
                s.execute(&format!("ALTER TABLE temporarily_missing RENAME TO {table}"), ())?;
                Ok(())
            })
            .unwrap();
        vault.retry_conversations().unwrap();
        corpus.check(&vault, None);
    }
}

/// A retained worker is not proof that its pathname still belongs to a complete archive.
#[cfg(unix)]
#[test]
fn whole_operations_recheck_required_files() {
    let f = Fixture::new(false);
    let vault = f.vault();
    let path = f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel));
    let saved = f.sandbox.path().join("saved.db");
    fs::rename(&path, &saved).unwrap();
    let result = vault_worker::reconcile_archive_encryption(&vault);
    fs::rename(saved, path).unwrap();
    let error = result.unwrap_err();
    assert!(matches!(error.downcast_ref::<vault_store::Error>(),
        Some(vault_store::Error::Open(reason)) if reason.role == Db::AgentHistoryReadModel));
    vault_worker::reconcile_archive_encryption(&vault).unwrap();
}

#[test]
fn a_missing_sibling_cannot_hide_an_alias_to_browsing() {
    for encrypted in [false, true] {
        let f = Fixture::new(encrypted);
        spoil(&f, Db::AgentHistoryEvidence, Damage::Missing);
        let model = f.root.join(vault_layout::database_path(Db::AgentHistoryReadModel));
        fs::remove_file(&model).unwrap();
        fs::hard_link(f.root.join(vault_layout::database_path(Db::SourceEvidence)), &model)
            .unwrap();
        let before = hashes(&f.root);
        let failure = open(&f, encrypted).unwrap_err();
        assert!(
            matches!(failure.downcast_ref::<vault_store::Error>(), Some(vault_store::Error::Open(reason))
            if matches!(reason.cause, vault_store::Error::InvalidConfig(_))),
            "{failure:#}"
        );
        assert_eq!(hashes(&f.root), before);
    }
}

#[test]
fn a_file_lost_after_preflight_is_never_created_by_the_writer() {
    let f = Fixture::new(false);
    let _corpus = Corpus::seed(&f);
    f.session.close_vault().unwrap();
    let mut config = vault_store::Config::new(&f.root);
    config.migrations = vault_core::store_schema::migration_registry();
    config.required_existing = Db::ALL.into_iter().filter(|db| *db != Db::Scratch).collect();
    let path = config.paths.agent_history_read_model.clone();
    let store = vault_store::Store::open_grouped(config, || {
        fs::remove_file(&path)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(store.conversation_availability().unwrap_err().role, Db::AgentHistoryReadModel);
    assert!(!path.exists());
    for role in CONVERSATIONS {
        assert_eq!(store.database_encrypted(role), None);
    }
}
