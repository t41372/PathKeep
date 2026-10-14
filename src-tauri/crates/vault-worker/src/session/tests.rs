//! Responsibilities: exercise the actual session used by UI and owner requests.
//! Not responsible for: macOS launch services or WebView rendering.
//! Failure modes: password replaced with data-key hex, failed startup leaks success,
//! concurrent reads fail instead of waiting, plaintext drops its compatibility key.
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
