//! Responsibilities: exercise the actual session used by UI and owner requests.
//! Not responsible for: macOS launch services or WebView rendering.
//! Failure modes: password replaced with data-key hex, failed startup leaks success,
//! concurrent reads fail instead of waiting, plaintext drops its compatibility key, an open
//! takes the files a maintenance operation is replacing, a keychain unlock takes the answer of an
//! open that had no key.
use super::*;

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

// An operation that puts another generation of the archive in place (a mode change, a restore, an
// import, a conversion) closes the archive first. An open in the same process meanwhile must not
// open what is on disk: those are the files being moved, and the open would replay the
// operation's install record while it is being written. It waits, and gets the replacement.
#[test]
fn an_open_during_maintenance_waits_for_the_replacement() {
    let _env = crate::tests::lock_env();
    let root = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    let paths = vault_core::project_paths().unwrap();
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..Default::default() };
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let state = VaultSession::default();
    let vault = state.background_vault().unwrap();
    let owner = vault.store().owner().unwrap();

    let lease = state.begin_maintenance().unwrap();
    assert!(state.begin_maintenance().is_err(), "one replacement at a time");
    vault.store().pause().unwrap().close().unwrap();
    vault.close().unwrap();
    let (sent, received) = std::sync::mpsc::channel();
    let waiter = {
        let state = state.clone();
        std::thread::spawn(move || sent.send(state.background_vault()).unwrap())
    };
    state.wait_until_parked(1);
    assert!(received.try_recv().is_err(), "an open went ahead while the archive was replaced");

    let replacement = Vault::open_owned(paths, config, None, Some(owner)).unwrap();
    state
        .install_archive(
            vault_core::maintenance::InstalledArchive {
                vault: replacement.clone(),
                data_key: None,
                previous_files: Default::default(),
                previous_unsealed: None,
            },
            None,
            KeychainAfterInstall::AsSaved,
        )
        .unwrap();
    drop(lease);
    let opened = received.recv_timeout(std::time::Duration::from_secs(10)).unwrap().unwrap();
    waiter.join().unwrap();
    assert!(Arc::ptr_eq(opened.store(), replacement.store()), "it got the replacement");
}

// A close while the archive is replaced (App Lock, an idle exit) waits for the replacement and
// closes that. Closing first would end the lease's hold, and an open could then take the files
// still being moved.
#[test]
fn a_close_during_maintenance_waits_and_closes_the_replacement() {
    let _env = crate::tests::lock_env();
    let root = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    let paths = vault_core::project_paths().unwrap();
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Plaintext, ..Default::default() };
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let state = VaultSession::default();
    let vault = state.background_vault().unwrap();
    let owner = vault.store().owner().unwrap();

    let lease = state.begin_maintenance().unwrap();
    vault.store().pause().unwrap().close().unwrap();
    vault.close().unwrap();
    let (sent, received) = std::sync::mpsc::channel();
    let closer = {
        let state = state.clone();
        std::thread::spawn(move || sent.send(state.close_vault()).unwrap())
    };
    state.wait_until_parked(1);
    assert!(received.try_recv().is_err(), "a close went ahead while the archive was replaced");
    assert!(state.begin_maintenance().is_err(), "the lease still holds");

    let replacement = Vault::open_owned(paths, config, None, Some(owner)).unwrap();
    state
        .install_archive(
            vault_core::maintenance::InstalledArchive {
                vault: replacement.clone(),
                data_key: None,
                previous_files: Default::default(),
                previous_unsealed: None,
            },
            None,
            KeychainAfterInstall::AsSaved,
        )
        .unwrap();
    drop(lease);
    received.recv_timeout(std::time::Duration::from_secs(10)).unwrap().unwrap();
    closer.join().unwrap();
    assert!(!replacement.store().is_open(), "the close closed the replacement");
    assert!(state.current_vault().is_none());
}

// The window asks the keychain once when it loads. An open that was already running without a
// key (any command that needs the archive, while the window loads) can only answer that a
// password is needed. If the keychain unlock took that answer as its own, the window would show
// the password screen with the key in the keychain. It waits for that open to end and tries the
// keychain itself.
#[test]
fn a_keychain_unlock_is_not_answered_by_an_open_that_had_no_key() {
    let _env = crate::tests::lock_env();
    let root = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var(crate::tests::PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(crate::tests::TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    let paths = vault_core::project_paths().unwrap();
    let config =
        AppConfig { initialized: true, archive_mode: ArchiveMode::Encrypted, ..Default::default() };
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let state = VaultSession::default();
    state.open_password(paths.clone(), config, "the password".into()).unwrap();
    crate::store_data_key_in_keyring(&Vault::unlock_data_key(&paths, "the password").unwrap())
        .unwrap();
    state.close_vault().unwrap();

    state.begin_startup();
    let (sent, received) = std::sync::mpsc::channel();
    let unlock = {
        let state = state.clone();
        std::thread::spawn(move || sent.send(state.unlock_from_keychain()).unwrap())
    };
    // The unlock holds the running open's result once it waits for it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while Arc::strong_count(&state.inner.0.lock().unwrap().result) < 2 {
        assert!(std::time::Instant::now() < deadline, "the unlock never waited for the open");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    state.abort_startup("archive-password-required: unlock the archive first".into());
    let unlocked = received.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    unlock.join().unwrap();
    assert_eq!(unlocked, Ok(()));
    assert!(state.current_vault().is_some());
}
