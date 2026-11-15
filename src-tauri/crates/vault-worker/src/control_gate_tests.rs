//! Responsibilities: prove who may write the archive's control, and when, through the worker's
//! own entry points: a settings save is not held up by a long operation on the archive, does not
//! write settings read before a wipe into the folder it emptied, and a provider key is stored only
//! for a provider the control lists.
//! Not responsible for: the gate itself (`vault_core::config::control_gate`) or the operations'
//! own writes of the control (`vault_core::maintenance` tests).
//! Failure modes: a save waits minutes behind an export; a save made from settings read before
//! "delete all data" recreates `config.json` after it; a key outlives the wipe that should have
//! cleared it because the control never listed its provider.
use crate::tests::{
    PROJECT_ROOT_OVERRIDE_ENV, TEST_KEYRING_OVERRIDE_ENV, configured_ai_config, initialized_config,
    lock_env,
};
use std::{sync::mpsc, time::Duration};
use vault_core::{AiProviderSecretInput, AppConfig, ControlGate, LanguagePreference};

/// A folder with a set-up archive's control, as setup leaves it; the archive itself is not opened.
fn set_up(config: &AppConfig) -> (tempfile::TempDir, vault_core::ProjectPaths) {
    let root = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path().join("data"));
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    let paths = vault_core::project_paths().unwrap();
    vault_core::config::ensure_paths(&paths).unwrap();
    vault_core::save_config(&ControlGate::acquire(&paths), config).unwrap();
    (root, paths)
}

fn in_chinese(config: &AppConfig) -> AppConfig {
    AppConfig { preferred_language: LanguagePreference::ZhTw, ..config.clone() }
}

#[test]
fn a_settings_save_does_not_wait_for_an_export_or_any_other_long_operation() {
    let _env = lock_env();
    let (_root, paths) = set_up(&initialized_config());
    // An export, a snapshot, a retention prune or a mode change holds this for minutes.
    let operation = vault_core::archive::ArchiveOpGate::acquire(&paths);
    let base = vault_core::load_config(&paths).unwrap();
    let changed = in_chinese(&base);
    let (sent, saved) = mpsc::channel();
    std::thread::spawn(move || {
        let result = crate::save_user_config_with_base(&changed, Some(&base), None);
        let _ = sent.send(result.map(|_| ()).map_err(|error| format!("{error:#}")));
    });
    let result = saved.recv_timeout(Duration::from_secs(20));
    drop(operation);
    assert!(matches!(result, Ok(Ok(()))), "the save waited for the operation: {result:?}");
    assert!(matches!(
        vault_core::load_config(&paths).unwrap().preferred_language,
        LanguagePreference::ZhTw
    ));
}

#[test]
fn settings_read_before_a_wipe_are_not_written_into_the_folder_it_emptied() {
    let _env = lock_env();
    let (_root, paths) = set_up(&initialized_config());
    let before = vault_core::load_config(&paths).unwrap();
    vault_core::wipe_all_data(
        &paths,
        |config| Ok(vault_core::WipeOutsideState::from_config(config, None)),
        || Ok(()),
        |_| Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    assert!(!paths.config_path.exists());

    // As the settings page saves: the change against the settings it showed.
    let error = crate::save_user_config_with_base(&in_chinese(&before), Some(&before), None)
        .expect_err("refused");
    assert!(format!("{error:#}").contains("archive-reset"), "{error:#}");
    assert!(!paths.config_path.exists(), "nothing was written");

    // A caller that writes a whole config cannot set an archive up either.
    let error = crate::save_user_config(&in_chinese(&before), None).expect_err("refused");
    assert!(format!("{error:#}").contains("archive-uninitialized"), "{error:#}");
    assert!(!paths.config_path.exists(), "nothing was written");
}

#[test]
fn a_provider_key_is_stored_only_for_a_provider_the_control_lists() {
    let _env = lock_env();
    let (_root, _paths) = set_up(&configured_ai_config());
    let unlisted =
        AiProviderSecretInput { provider_id: "not-listed".into(), api_key: "sk-test-1".into() };
    let error = crate::store_ai_provider_api_key(&unlisted, None, None).expect_err("refused");
    assert!(format!("{error:#}").contains("ai-provider-unknown"), "{error:#}");
    assert!(!vault_platform::provider_api_key_saved("not-listed"));

    let listed =
        AiProviderSecretInput { provider_id: "llm-primary".into(), api_key: "sk-test-2".into() };
    crate::store_ai_provider_api_key(&listed, None, None).unwrap();
    assert!(vault_platform::provider_api_key_saved("llm-primary"));
}
