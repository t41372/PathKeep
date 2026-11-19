//! The quit prompt can promise to resume a live migration. Startup discards a staged copy.
//! Exercise both Vault entry points with v1 stores, including the debug PID marker the E2E reads.
#![cfg(debug_assertions)]

use crate::{AppConfig, ArchiveMode, Vault, project_paths_with_root, schema_update_running};
use std::{fs, path::PathBuf, process::Command, sync::mpsc, thread, time::Duration};
use vault_store::{Db, Store};

// The quit flag, progress listener and debug environment belong to the whole process.
fn run_isolated(case: &str) {
    let root = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "store_schema::tests::quit_tracking::open_v1",
            "--ignored",
            "--nocapture",
        ])
        .env("PATHKEEP_TEST_OPEN_CASE", case)
        .env("PATHKEEP_TEST_HOLD_SCHEMA_UPDATE", root.path().join("hold"))
        .status()
        .unwrap();
    assert!(status.success(), "{case}: {status}");
}

#[test]
fn staged_migration_does_not_count_toward_quitting() {
    run_isolated("staged");
}

#[test]
fn live_migration_counts_until_open_returns() {
    run_isolated("live-held");
}

#[test]
fn live_migration_records_its_pid_even_without_a_hold_file() {
    run_isolated("live-unheld");
}

#[test]
#[ignore = "runs in a separate process for each quit-tracking case"]
fn open_v1() {
    let Ok(case) = std::env::var("PATHKEEP_TEST_OPEN_CASE") else { return };
    let live = case != "staged";
    let wait = case == "live-held";
    let hold = PathBuf::from(std::env::var_os("PATHKEEP_TEST_HOLD_SCHEMA_UPDATE").unwrap());
    let marker = hold.with_extension("held");
    let pid = std::process::id().to_string();
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..Default::default() };
    crate::config::write_staged_control(&paths, &config, None).unwrap();
    let mut storage = super::config(root.path(), None);
    storage.databases = vault_layout::databases_in(vault_layout::ArchiveLayout::BrowsingV1);
    storage.migrations.register(Db::Archive, vec![super::super::archive::migration()]);
    storage.migrations.register(Db::Intelligence, vec![super::super::intelligence::migration()]);
    Store::open(storage).unwrap().close().unwrap();

    let (send, progress) = mpsc::channel();
    crate::register_archive_upgrade_listener(move |event| {
        if let Some(db) = event.database {
            send.send((db, schema_update_running())).unwrap();
        }
    });
    assert!(!schema_update_running());
    if wait {
        fs::write(&hold, "").unwrap();
    } else if live {
        fs::write(&marker, "previous process").unwrap();
    }
    let archive = paths.archive_db.clone();
    let open = thread::spawn(move || {
        if live {
            Vault::open_owned(paths, config, None, None)
        } else {
            Vault::open_staged(paths, config, None)
        }
    });
    if wait {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while fs::read_to_string(&marker).ok().as_ref() != Some(&pid) {
            assert!(!open.is_finished() && std::time::Instant::now() < deadline, "missing hold");
            thread::sleep(Duration::from_millis(10));
        }
        assert!(schema_update_running(), "before the first schema change");
        // The observer must already count while the released schema is still intact.
        let db = rusqlite::Connection::open_with_flags(
            archive,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let version: i64 =
            db.query_row("SELECT MAX(version) FROM __pk_migrations", [], |row| row.get(0)).unwrap();
        assert_eq!(version, 1);
        drop(db);
        fs::remove_file(hold).unwrap();
    }
    let _vault = open.join().unwrap().unwrap();
    assert!(!schema_update_running(), "the returned Vault no longer counts");
    let events: Vec<_> = progress.try_iter().collect();
    for db in [Db::Archive, Db::Intelligence] {
        assert!(events.iter().any(|(name, _)| *name == format!("{db:?}")), "{events:?}");
    }
    assert!(events.iter().all(|(_, running)| *running == live), "{case}: {events:?}");
    if live {
        assert_eq!(fs::read_to_string(marker).unwrap(), pid);
    } else {
        assert!(!marker.exists(), "a staged update activated the live update observer");
    }
}
