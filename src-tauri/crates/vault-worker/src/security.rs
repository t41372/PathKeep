//! Responsibilities: coordinate App Lock, native rekey previews and stored data-key access.
//! Not responsible for: envelope cryptography or database/file installation.
//! Failure modes: wrong current password accepted, stale keychain data or App Lock conflated with
//! archive encryption.
//!
//! Security, App Lock, and keyring worker flows.
//!
//! This module owns the session-boundary and native-secret parts of the worker
//! contract:
//!
//! - App Lock setup/lock/unlock read models
//! - rekey preview
//! - native keyring and stored-secret helpers
//!
//! The accepted product rule here is important: App Lock is a UI session
//! boundary, not archive encryption. The helpers below therefore preserve both
//! stories separately instead of blending them into one “locked” concept.

use crate::{
    app::RekeyRequest,
    context::{
        current_app_lock_biometric_state, load_hydrated_config, load_unlocked_config,
        resolved_app_lock_status,
    },
};
use anyhow::Result;
use vault_core::{
    AiProviderSecretInput, AppLockStatus, ArchiveMode, KeyringStatusReport,
    clear_app_lock_passcode, lock_app_session, set_app_lock_passcode,
    unlock_app_session_with_biometric,
};
use vault_platform::{
    authenticate_app_lock_biometric, keyring_clear_database_key, keyring_clear_provider_api_key,
    keyring_get_database_key, keyring_set_database_key, keyring_set_provider_api_key,
    keyring_status,
};

/// Loads the App Lock status without requiring the archive to be unlocked.
pub fn load_app_lock_status() -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    resolved_app_lock_status(&paths, &config)
}

/// Configures an App Lock passcode and returns the updated lock status.
///
/// Routed through `load_unlocked_config` so the passcode can only be set or
/// changed from an unlocked session. Otherwise an actor with IPC access could
/// overwrite the passcode with a known value while the app is locked — without
/// proving knowledge of the current one — and then unlock. (Initial setup still
/// works: when no lock is enabled yet, the unlock check is a no-op.)
pub fn configure_app_lock_passcode(
    request: &vault_core::SetAppLockPasscodeRequest,
) -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let mut config = load_unlocked_config(&paths)?;
    set_app_lock_passcode(&paths, &mut config, request)?;
    resolved_app_lock_status(&paths, &config)
}

/// Clears the App Lock passcode and returns the updated lock status.
///
/// Routed through `load_unlocked_config` so the lock cannot be removed while the
/// session is locked (which would otherwise bypass the lock without the
/// passcode). Recovering a *forgotten* passcode is an explicit out-of-band flow,
/// not this normal-path mutation.
pub fn remove_app_lock_passcode() -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let mut config = load_unlocked_config(&paths)?;
    clear_app_lock_passcode(&paths, &mut config)?;
    resolved_app_lock_status(&paths, &config)
}

/// Locks the current UI session.
pub fn lock_app_ui_session(reason: Option<&str>) -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    lock_app_session(&paths, &config, reason)?;
    resolved_app_lock_status(&paths, &config)
}

/// Unlocks the current UI session, optionally using biometrics.
pub fn unlock_app_ui_session(
    request: &vault_core::UnlockAppSessionRequest,
) -> Result<AppLockStatus> {
    let paths = vault_core::project_paths()?;
    let config = load_hydrated_config(&paths)?;
    unlock_app_session_with_biometric(
        &paths,
        &config,
        request,
        current_app_lock_biometric_state(),
        authenticate_app_lock_biometric_result,
    )
}

fn authenticate_app_lock_biometric_result() -> Result<()> {
    authenticate_app_lock_biometric().map_err(biometric_authentication_error)
}

fn biometric_authentication_error(message: String) -> anyhow::Error {
    anyhow::Error::msg(message)
}

/// Previews the archive rewrite that a rekey/mode switch would perform.
///
/// Checks the current password exactly as the rekey will (see
/// [`crate::current_password`]), so a wrong one is refused on the first step
/// instead of after the user has read the plan.
pub fn preview_rekey_archive(request: &RekeyRequest) -> Result<vault_core::RekeyPreview> {
    let paths = vault_core::project_paths()?;
    let config = load_unlocked_config(&paths)?;
    anyhow::ensure!(
        config.initialized && paths.archive_db.exists(),
        "initialize the archive before previewing a rekey operation"
    );
    crate::current_password::verified_current_key(&paths, &config, request)?;
    let rewrap = config.archive_mode == request.new_mode;
    let copy = rekey_copy(&config.preferred_language);
    let mut preview = vault_core::RekeyPreview {
        current_mode: config.archive_mode,
        next_mode: request.new_mode.clone(),
        requires_new_key: request.new_mode == ArchiveMode::Encrypted,
        snapshot_path: if rewrap {
            String::new()
        } else {
            paths.raw_snapshots_dir.join("maintenance/rekey-<id>").display().to_string()
        },
        temp_database_path: if rewrap {
            String::new()
        } else {
            paths.staging_dir.join("maintenance-<id>").display().to_string()
        },
        steps: if rewrap {
            vec![copy[0].into()]
        } else {
            copy[1..4].iter().map(|text| (*text).into()).collect()
        },
        ..Default::default()
    };
    if preview.requires_new_key && request.new_key.is_none() {
        preview.warnings.push(copy[4].into());
        preview.warning_codes.push(vault_core::REKEY_WARNING_NEW_KEY_REQUIRED.into());
    }
    Ok(preview)
}

// The frontend owns phase labels; preview sentences also work without a translated client.
fn rekey_copy(language: &vault_core::LanguagePreference) -> [&'static str; 5] {
    match language {
        vault_core::LanguagePreference::ZhCn => [
            "用新密码重新包装密钥信封。数据库文件和钥匙串中的数据密钥保持不变。",
            "将每个数据库复制为所选加密模式。",
            "核对每张表的行数和内容摘要，再以新模式重建搜索索引。",
            "以新模式保存已验证的安全快照，再通过可恢复的流程替换文件。",
            "执行前请输入新的存档密码。",
        ],
        vault_core::LanguagePreference::ZhTw => [
            "用新密碼重新包裝金鑰信封。資料庫檔案和鑰匙圈中的資料金鑰保持不變。",
            "將每個資料庫複製為所選加密模式。",
            "核對每張表的列數和內容摘要，再以新模式重建搜尋索引。",
            "以新模式保存已驗證的安全快照，再透過可恢復的流程替換檔案。",
            "執行前請輸入新的封存密碼。",
        ],
        _ => [
            "Rewrap the key envelope with the new password. Database files and the remembered data key stay unchanged.",
            "Copy every database into the selected encryption mode.",
            "Verify every table's row count and content digest, then rebuild search in the new mode.",
            "Save a verified safety snapshot in the new mode, then install the files with interruption recovery.",
            "Enter the new archive password before executing.",
        ],
    }
}

/// Reads the stored archive key from the native keyring, if present.
pub fn read_database_key_from_keyring() -> Result<Option<zeroize::Zeroizing<String>>> {
    Ok(keyring_get_database_key()?.map(zeroize::Zeroizing::new))
}

/// Stores the archive key in the native keyring and returns the updated keyring status.
pub fn write_database_key_to_keyring(key: &str) -> Result<KeyringStatusReport> {
    let paths = vault_core::project_paths()?;
    let data_key = vault_core::Vault::unlock_data_key(&paths, key)?;
    store_data_key_in_keyring(&data_key)
}

/// Stores an already unwrapped data key in the existing OS credential entry.
pub fn store_data_key_in_keyring(key: &vault_crypto::DataKey) -> Result<KeyringStatusReport> {
    keyring_set_database_key(&crate::session::encode_data_key(key))?;
    Ok(keyring_status())
}

/// Clears the stored archive key from the native keyring.
pub fn clear_database_key_from_keyring() -> Result<KeyringStatusReport> {
    keyring_clear_database_key()?;
    Ok(keyring_status())
}

/// Returns the current native keyring status snapshot.
pub fn keyring_report() -> KeyringStatusReport {
    keyring_status()
}

/// Stores an AI provider secret and returns a refreshed app snapshot. The snapshot is read
/// through the session's open archive: without it an archive unlocked from the keychain
/// would come back as locked and replace the shell's snapshot.
pub fn store_ai_provider_api_key(
    input: &AiProviderSecretInput,
    vault: Option<&vault_core::Vault>,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot> {
    keyring_set_provider_api_key(&input.provider_id, &input.api_key)?;
    crate::app_snapshot_with_vault(vault, session_database_key)
}

/// Clears an AI provider secret and returns a refreshed app snapshot, read like
/// [`store_ai_provider_api_key`]'s.
pub fn clear_ai_provider_api_key(
    provider_id: &str,
    vault: Option<&vault_core::Vault>,
    session_database_key: Option<&str>,
) -> Result<vault_core::AppSnapshot> {
    keyring_clear_provider_api_key(provider_id)?;
    crate::app_snapshot_with_vault(vault, session_database_key)
}
