//! Responsibilities: exercise the actual session used by UI and owner requests.
//! Not responsible for: macOS launch services or WebView rendering.
//! Failure modes: password replaced with data-key hex, failed startup leaks success,
//! concurrent reads fail instead of waiting, plaintext drops its compatibility key.
use super::*;

#[test]
fn password_and_data_key_have_separate_lifetimes() {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Encrypted, ..Default::default() };
    let state = VaultSession::default();
    state.open_password(paths.clone(), config.clone(), "typed password".into()).unwrap();
    assert_eq!(state.get_key().unwrap().as_deref(), Some("typed password"));
    let data = state.inner.0.lock().unwrap().data_key.clone().unwrap();
    state.close_vault().unwrap();
    state.open(|| state.open_store(paths.clone(), config, None, Some(data))).unwrap();
    assert_eq!(state.get_key().unwrap(), None);
    assert!(state.inner.0.lock().unwrap().vault.is_some());
}

#[test]
fn ui_read_waits_for_startup_open_and_receives_password() {
    let state = VaultSession::default();
    state.begin_startup();
    let reading = state.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || tx.send(reading.get_key()).unwrap());
    assert!(rx.recv_timeout(std::time::Duration::from_millis(50)).is_err());
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..Default::default() };
    let result = state.open_store(
        paths,
        config,
        Some(Zeroizing::new("compatibility password".to_string())),
        None,
    );
    state.finish_open(result).unwrap();
    assert_eq!(rx.recv().unwrap().unwrap().as_deref(), Some("compatibility password"));
    reader.join().unwrap();
}

#[test]
fn plaintext_session_keeps_legacy_password_without_an_envelope() {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..Default::default() };
    let state = VaultSession::default();
    let first = state.open_password(paths.clone(), config.clone(), "legacy key".into()).unwrap();
    let second = state.load_vault(paths, config).unwrap();
    assert!(Arc::ptr_eq(first.store(), second.store()));
    assert_eq!(state.get_key().unwrap().as_deref(), Some("legacy key"));
    assert!(!root.path().join("archive/keys.envelope").exists());
    state.close_vault().unwrap();
    assert!(state.get_key().unwrap().is_none());
    drop(Vault::open(first.paths().clone(), first.config().clone(), None).unwrap());
}

#[test]
fn concurrent_openers_receive_the_startup_result() {
    for succeeds in [false, true] {
        let state = VaultSession::default();
        state.begin_startup();
        let root = tempfile::tempdir().unwrap();
        let paths = vault_core::project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..Default::default()
        };
        let waiting = state.clone();
        let (wait_paths, wait_config) = (paths.clone(), config.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        let opener = std::thread::spawn(move || {
            tx.send(waiting.load_vault(wait_paths, wait_config)).unwrap()
        });
        assert!(rx.recv_timeout(std::time::Duration::from_millis(50)).is_err());
        if succeeds {
            let opened = state.open_store(paths, config, None, None).unwrap();
            state.finish_open(Ok(opened.clone())).unwrap();
            let joined = rx.recv().unwrap().unwrap();
            assert!(Arc::ptr_eq(opened.store(), joined.store()));
        } else {
            assert!(state.finish_open(Err("startup failure".into())).is_err());
            assert_eq!(rx.recv().unwrap().unwrap_err(), "startup failure");
            assert!(!root.path().join("archive/history-vault.db").exists());
        }
        opener.join().unwrap();
    }
}

#[test]
fn retained_result_survives_new_attempts() {
    let state = VaultSession::default();
    state.begin_startup();
    let first = state.inner.0.lock().unwrap().result.clone();
    state.finish_open(Err("first failure".into())).unwrap_err();
    state.close_vault().unwrap();
    state.begin_startup();
    state.finish_open(Err("second failure".into())).unwrap_err();
    assert_eq!(first.wait().unwrap_err(), "first failure");
}

#[test]
fn typed_password_waiter_retries_after_failed_startup() {
    let state = VaultSession::default();
    state.begin_startup();
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let waiting = state.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        tx.send(waiting.open_password(
            paths,
            AppConfig {
                initialized: true,
                archive_mode: ArchiveMode::Encrypted,
                ..Default::default()
            },
            "typed while opening".into(),
        ))
        .unwrap()
    });
    assert!(rx.recv_timeout(std::time::Duration::from_millis(30)).is_err());
    state.finish_open(Err("keychain-key-missing: none stored".into())).unwrap_err();
    assert!(rx.recv().unwrap().is_ok());
    thread.join().unwrap();
    assert_eq!(state.get_key().unwrap().as_deref(), Some("typed while opening"));
}
#[test]
fn panicking_open_wakes_waiters_and_allows_another_open() {
    let state = VaultSession::default();
    let attempt = state.clone();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || attempt.open(|| panic!("opening failed"))
        ))
        .is_err()
    );
    assert_eq!(state.inner.0.lock().unwrap().transition, Transition::Idle);
    assert!(
        state.inner.0.lock().unwrap().result.wait().unwrap_err().contains("archive-open-panicked")
    );
}

#[test]
fn legacy_keychain_password_is_unwrapped_and_replaced_in_the_backend() {
    let _guard = crate::tests::lock_env();
    let root = tempfile::tempdir().unwrap();
    let names = [crate::tests::PROJECT_ROOT_OVERRIDE_ENV, crate::tests::TEST_KEYRING_OVERRIDE_ENV];
    let old = names.map(std::env::var_os);
    unsafe {
        std::env::set_var(names[0], root.path());
        std::env::set_var(names[1], root.path().join("keyring"));
    }
    for password in
        ["legacy password", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
    {
        let paths = vault_core::project_paths().unwrap();
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Encrypted,
            ..Default::default()
        };
        crate::initialize_archive_database(&config, Some(password)).unwrap();
        vault_platform::keyring_set_database_key(password).unwrap();
        let state = VaultSession::default();
        if password.len() == 64 {
            state.open_password(paths.clone(), config.clone(), password.into()).unwrap();
            state.close_vault().unwrap();
        }
        state.unlock_from_keychain().unwrap();
        assert_eq!(state.get_key().unwrap().as_deref(), Some(password));
        let stored = crate::read_database_key_from_keyring().unwrap().unwrap();
        let key = decode_data_key(&stored).unwrap();
        assert_eq!(
            key.expose_bytes(),
            state.inner.0.lock().unwrap().data_key.as_ref().unwrap().expose_bytes()
        );
        state.close_vault().unwrap();
        state.unlock_from_keychain().unwrap();
        assert_eq!(state.get_key().unwrap(), None);
        assert!(state.current_vault().is_some());
        assert!(paths.app_root.join("archive/keys.envelope").exists());
        state.close_vault().unwrap();
        std::fs::remove_dir_all(root.path().join("archive")).unwrap();
        std::fs::remove_dir_all(root.path().join("derived")).unwrap();
    }
    for (name, value) in names.into_iter().zip(old) {
        crate::tests::restore_env_var(name, value.as_deref());
    }
}

#[test]
fn panicking_startup_wakes_joined_openers_with_its_error() {
    let state = VaultSession::default();
    state.begin_startup();
    let waiting = state.clone();
    let (sent, received) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        sent.send(waiting.open(|| panic!("waiter must join startup"))).unwrap()
    });
    assert!(received.recv_timeout(std::time::Duration::from_millis(30)).is_err());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || state.complete_open(|| panic!("startup panic"))
        ))
        .is_err()
    );
    assert!(received.recv().unwrap().unwrap_err().starts_with("archive-open-panicked:"));
    waiter.join().unwrap();
    assert_eq!(state.inner.0.lock().unwrap().transition, Transition::Idle);
}

#[test]
fn concurrent_first_password_unlocks_share_one_data_key() {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Encrypted, ..Default::default() };
    let state = VaultSession::default();
    let ready = Arc::new(std::sync::Barrier::new(2));
    let jobs = (0..2)
        .map(|_| {
            let (state, ready, paths, config) =
                (state.clone(), ready.clone(), paths.clone(), config.clone());
            std::thread::spawn(move || {
                ready.wait();
                state.open_password(paths, config, "shared password".into()).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let vaults = jobs.into_iter().map(|job| job.join().unwrap()).collect::<Vec<_>>();
    assert!(Arc::ptr_eq(vaults[0].store(), vaults[1].store()));
    let key = Vault::unlock_data_key(&paths, "shared password").unwrap();
    assert_eq!(
        key.expose_bytes(),
        state.inner.0.lock().unwrap().data_key.as_ref().unwrap().expose_bytes()
    );
}

#[test]
fn session_reads_remain_available_during_close() {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..Default::default() };
    let state = VaultSession::default();
    let vault = state.load_vault(paths.clone(), config.clone()).unwrap();
    let (entered, arrived) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    let writer = std::thread::spawn(move || {
        vault
            .store()
            .write(vault_store::Db::Archive, vault_store::Priority::Batch, move |_| {
                entered.send(()).unwrap();
                wait.recv().unwrap();
                Ok(())
            })
            .unwrap()
    });
    arrived.recv().unwrap();
    let closing = state.clone();
    let close = std::thread::spawn(move || closing.close_vault().unwrap());
    while state.inner.0.lock().unwrap().transition != Transition::Closing {
        std::thread::yield_now();
    }
    let reading = state.clone();
    let (sent, received) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        sent.send((reading.get_key(), reading.load_vault(paths, config))).unwrap()
    });
    let answer = received.recv_timeout(std::time::Duration::from_millis(50));
    release.send(()).unwrap();
    writer.join().unwrap();
    close.join().unwrap();
    reader.join().unwrap();
    let (key, open) = answer.expect("session reads cannot wait behind Store::close");
    assert_eq!(key.unwrap(), None);
    assert!(open.unwrap_err().starts_with("archive-busy:"));
}
