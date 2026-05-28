//! Native keyring adapter.
//!
//! This module is the only place that should know how PathKeep stores secrets
//! on the host. It supports native backends in production and a file-backed
//! override for tests/coverage so higher layers can stay deterministic.

use crate::test_support::{keyring_service, test_keyring_dir};
use anyhow::{Context, Result};
#[cfg(all(not(coverage), target_os = "macos"))]
use apple_native_keyring_store::keychain::Store as NativeKeyringStore;
#[cfg(all(not(coverage), any(target_os = "linux", target_os = "freebsd")))]
use dbus_secret_service_keyring_store::Store as NativeKeyringStore;
#[cfg(not(coverage))]
use keyring_core::{Entry, get_default_store, set_default_store};
#[cfg(all(not(coverage), target_os = "macos"))]
use std::collections::HashMap;
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::KeyringStatusReport;
#[cfg(all(not(coverage), target_os = "windows"))]
use windows_native_keyring_store::Store as NativeKeyringStore;

const KEYRING_DATABASE_USER: &str = "database-key";

fn provider_keyring_user(provider_id: &str) -> String {
    format!("ai-provider::{provider_id}")
}

#[cfg(not(coverage))]
fn keyring_entry(user: &str) -> Result<Entry> {
    ensure_native_keyring_store()?;
    Ok(Entry::new(&keyring_service(), user)?)
}

#[cfg(all(
    not(coverage),
    any(target_os = "macos", target_os = "windows", target_os = "linux", target_os = "freebsd",)
))]
fn ensure_native_keyring_store() -> Result<()> {
    if get_default_store().is_none() {
        set_default_store(
            NativeKeyringStore::new().context("initializing native keyring backend")?,
        );
    }
    Ok(())
}

#[cfg(all(
    not(coverage),
    not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd",
    ))
))]
fn ensure_native_keyring_store() -> Result<()> {
    anyhow::bail!("A native keyring backend is not available on this machine.")
}

#[cfg(all(not(coverage), target_os = "macos"))]
fn keyring_entry_exists_for_service(service: &str, user: &str) -> bool {
    ensure_native_keyring_store().ok();
    Entry::search(&HashMap::from([("service", service), ("user", user)]))
        .map(|entries| !entries.is_empty())
        .unwrap_or(false)
}

#[cfg(all(not(coverage), not(target_os = "macos")))]
fn keyring_entry_exists_for_service(service: &str, user: &str) -> bool {
    ensure_native_keyring_store().ok();
    Entry::new(service, user).ok().and_then(|entry| entry.get_password().ok()).is_some()
}

#[cfg(not(coverage))]
fn keyring_entry_exists(user: &str) -> bool {
    keyring_entry_exists_for_service(&keyring_service(), user)
}

#[cfg(all(target_os = "macos", not(coverage)))]
fn keyring_backend_name() -> &'static str {
    "macOS Keychain"
}

#[cfg(all(target_os = "windows", not(coverage)))]
fn keyring_backend_name() -> &'static str {
    "Windows Credential Manager"
}

#[cfg(all(not(any(target_os = "macos", target_os = "windows")), not(coverage)))]
fn keyring_backend_name() -> &'static str {
    "Linux Secret Service / keyutils"
}

#[cfg(coverage)]
/// Reports keyring availability using the coverage-mode file-backed backend.
pub fn keyring_status() -> KeyringStatusReport {
    let path = test_keyring_dir().expect("coverage keyring dir");
    KeyringStatusReport {
        available: true,
        backend: "File-backed test keyring".to_string(),
        stored_secret: test_keyring_path(&path, KEYRING_DATABASE_USER).exists(),
        message: None,
    }
}

#[cfg(not(coverage))]
/// Reports whether a usable keyring backend exists and whether a database key is stored.
pub fn keyring_status() -> KeyringStatusReport {
    if let Some(path) = test_keyring_dir() {
        let stored_secret = test_keyring_path(&path, KEYRING_DATABASE_USER).exists();
        return KeyringStatusReport {
            available: true,
            backend: "File-backed test keyring".to_string(),
            stored_secret,
            message: None,
        };
    }

    let backend = keyring_backend_name();
    let available = ensure_native_keyring_store().is_ok();
    let stored_secret = available && keyring_entry_exists(KEYRING_DATABASE_USER);

    KeyringStatusReport {
        available,
        backend: backend.to_string(),
        stored_secret,
        message: if available {
            None
        } else {
            Some("A native keyring backend is not available on this machine.".to_string())
        },
    }
}

#[cfg(coverage)]
/// Reads the stored archive database key from the file-backed coverage backend.
pub fn keyring_get_database_key() -> Result<Option<String>> {
    test_keyring_get(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_DATABASE_USER)
}

#[cfg(not(coverage))]
/// Reads the stored archive database key from the current backend.
pub fn keyring_get_database_key() -> Result<Option<String>> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_get(&path, KEYRING_DATABASE_USER);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

#[cfg(coverage)]
/// Stores the archive database key in the file-backed coverage backend.
pub fn keyring_set_database_key(key: &str) -> Result<()> {
    test_keyring_set(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_DATABASE_USER, key)
}

#[cfg(not(coverage))]
/// Stores the archive database key in the current backend.
pub fn keyring_set_database_key(key: &str) -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, KEYRING_DATABASE_USER, key);
    }

    ensure_native_keyring_store()?;
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    entry.set_password(key)?;
    Ok(())
}

#[cfg(coverage)]
/// Removes the archive database key from the file-backed coverage backend.
pub fn keyring_clear_database_key() -> Result<()> {
    test_keyring_clear(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_DATABASE_USER)
}

#[cfg(not(coverage))]
/// Removes the archive database key from the current backend.
pub fn keyring_clear_database_key() -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, KEYRING_DATABASE_USER);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    let _ = entry.delete_credential();
    Ok(())
}

#[cfg(coverage)]
/// Reads an AI provider API key from the file-backed coverage backend.
pub fn keyring_get_provider_api_key(provider_id: &str) -> Result<Option<String>> {
    let user = provider_keyring_user(provider_id);
    test_keyring_get(&test_keyring_dir().expect("coverage keyring dir"), &user)
}

#[cfg(not(coverage))]
/// Reads an AI provider API key from the current backend.
pub fn keyring_get_provider_api_key(provider_id: &str) -> Result<Option<String>> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_get(&path, &user);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(&user)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

#[cfg(coverage)]
/// Stores an AI provider API key in the file-backed coverage backend.
pub fn keyring_set_provider_api_key(provider_id: &str, api_key: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    test_keyring_set(&test_keyring_dir().expect("coverage keyring dir"), &user, api_key)
}

#[cfg(not(coverage))]
/// Stores an AI provider API key in the current backend.
pub fn keyring_set_provider_api_key(provider_id: &str, api_key: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, &user, api_key);
    }

    ensure_native_keyring_store()?;
    let entry = keyring_entry(&user)?;
    entry.set_password(api_key)?;
    Ok(())
}

#[cfg(coverage)]
/// Removes an AI provider API key from the file-backed coverage backend.
pub fn keyring_clear_provider_api_key(provider_id: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    test_keyring_clear(&test_keyring_dir().expect("coverage keyring dir"), &user)
}

#[cfg(not(coverage))]
/// Removes an AI provider API key from the current backend.
pub fn keyring_clear_provider_api_key(provider_id: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, &user);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(&user)?;
    let _ = entry.delete_credential();
    Ok(())
}

/// Returns whether an AI provider API key is currently stored in the active backend.
pub fn provider_api_key_saved(provider_id: &str) -> bool {
    #[cfg(coverage)]
    {
        keyring_get_provider_api_key(provider_id).ok().flatten().is_some()
    }

    #[cfg(not(coverage))]
    {
        let user = provider_keyring_user(provider_id);
        if let Some(path) = test_keyring_dir() {
            return test_keyring_path(&path, &user).exists();
        }

        keyring_entry_exists_for_service(&keyring_service(), &user)
    }
}

fn test_keyring_path(root: &Path, user: &str) -> PathBuf {
    root.join(format!("{}-{user}.secret", keyring_service()))
}

fn test_keyring_get(root: &Path, user: &str) -> Result<Option<String>> {
    let path = test_keyring_path(root, user);
    if !path.exists() {
        return Ok(None);
    }
    let value = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(value))
}

fn test_keyring_set(root: &Path, user: &str, value: &str) -> Result<()> {
    fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
    let path = test_keyring_path(root, user);
    fs::write(&path, value).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn test_keyring_clear(root: &Path, user: &str) -> Result<()> {
    let path = test_keyring_path(root, user);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        TEST_KEYRING_DIR_ENV, TEST_KEYRING_SERVICE_ENV, env_lock, restore_env_var,
    };
    use tempfile::tempdir;

    #[test]
    fn file_backed_test_keyring_roundtrips_secrets() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_keyring_dir = std::env::var_os(TEST_KEYRING_DIR_ENV);
        let original_keyring_service = std::env::var_os(TEST_KEYRING_SERVICE_ENV);
        unsafe {
            std::env::set_var(TEST_KEYRING_DIR_ENV, dir.path());
            std::env::set_var(TEST_KEYRING_SERVICE_ENV, "com.yi-ting.pathkeep.tests");
        }

        keyring_set_database_key("database-secret").expect("set db");
        assert_eq!(
            keyring_get_database_key().expect("get db"),
            Some("database-secret".to_string())
        );
        assert!(keyring_status().stored_secret);

        keyring_set_provider_api_key("openai-primary", "provider-secret").expect("set provider");
        assert_eq!(
            keyring_get_provider_api_key("openai-primary").expect("get provider"),
            Some("provider-secret".to_string())
        );
        assert!(provider_api_key_saved("openai-primary"));

        keyring_clear_database_key().expect("clear db");
        keyring_clear_provider_api_key("openai-primary").expect("clear provider");
        assert!(!provider_api_key_saved("openai-primary"));

        restore_env_var(TEST_KEYRING_DIR_ENV, original_keyring_dir.as_deref());
        restore_env_var(TEST_KEYRING_SERVICE_ENV, original_keyring_service.as_deref());
    }

    #[test]
    fn file_backed_test_keyring_handles_missing_entries_and_helpers() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_keyring_dir = std::env::var_os(TEST_KEYRING_DIR_ENV);
        let original_keyring_service = std::env::var_os(TEST_KEYRING_SERVICE_ENV);
        unsafe {
            std::env::set_var(TEST_KEYRING_DIR_ENV, dir.path());
            std::env::set_var(TEST_KEYRING_SERVICE_ENV, "com.yi-ting.pathkeep.tests");
        }

        assert_eq!(keyring_get_database_key().expect("missing db key"), None);
        assert_eq!(
            keyring_get_provider_api_key("missing-provider").expect("missing provider"),
            None
        );
        assert!(
            test_keyring_path(dir.path(), "sample-user")
                .display()
                .to_string()
                .contains("sample-user")
        );
        keyring_clear_database_key().expect("clear empty db key");
        keyring_clear_provider_api_key("missing-provider").expect("clear empty provider key");

        restore_env_var(TEST_KEYRING_DIR_ENV, original_keyring_dir.as_deref());
        restore_env_var(TEST_KEYRING_SERVICE_ENV, original_keyring_service.as_deref());
    }

    #[test]
    fn provider_key_does_not_satisfy_database_key_status_or_clear_database_secret() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_keyring_dir = std::env::var_os(TEST_KEYRING_DIR_ENV);
        let original_keyring_service = std::env::var_os(TEST_KEYRING_SERVICE_ENV);
        unsafe {
            std::env::set_var(TEST_KEYRING_DIR_ENV, dir.path());
            std::env::set_var(TEST_KEYRING_SERVICE_ENV, "com.yi-ting.pathkeep.tests");
        }

        keyring_set_provider_api_key("openai-primary", "provider-secret").expect("set provider");
        assert!(provider_api_key_saved("openai-primary"));
        assert!(
            !keyring_status().stored_secret,
            "provider API keys must not make the database key look saved",
        );

        keyring_set_database_key("database-secret").expect("set db");
        assert!(keyring_status().stored_secret);

        keyring_clear_provider_api_key("openai-primary").expect("clear provider");
        assert_eq!(
            keyring_get_database_key().expect("get db"),
            Some("database-secret".to_string()),
            "clearing a provider key must not clear the database key",
        );
        assert!(!provider_api_key_saved("openai-primary"));

        restore_env_var(TEST_KEYRING_DIR_ENV, original_keyring_dir.as_deref());
        restore_env_var(TEST_KEYRING_SERVICE_ENV, original_keyring_service.as_deref());
    }

    #[test]
    fn provider_keyring_user_and_file_backed_helpers_cover_extra_paths() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_keyring_dir = std::env::var_os(TEST_KEYRING_DIR_ENV);
        let original_keyring_service = std::env::var_os(TEST_KEYRING_SERVICE_ENV);
        unsafe {
            std::env::set_var(TEST_KEYRING_DIR_ENV, dir.path());
            std::env::set_var(TEST_KEYRING_SERVICE_ENV, "com.yi-ting.pathkeep.tests");
        }

        assert_eq!(provider_keyring_user("openai"), "ai-provider::openai");
        assert_eq!(keyring_status().backend, "File-backed test keyring");

        restore_env_var(TEST_KEYRING_DIR_ENV, original_keyring_dir.as_deref());
        restore_env_var(TEST_KEYRING_SERVICE_ENV, original_keyring_service.as_deref());
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_helpers_cover_default_keyring_root_and_restore_set_branch() {
        let _guard = env_lock().lock().expect("env lock");
        let original_keyring_dir = std::env::var_os(TEST_KEYRING_DIR_ENV);
        let seeded_value = std::ffi::OsString::from("/tmp/chb-platform-keyring");
        unsafe {
            std::env::remove_var(TEST_KEYRING_DIR_ENV);
        }

        let default_root = test_keyring_dir().expect("default keyring dir");
        assert!(default_root.to_string_lossy().contains("pathkeep-coverage-keyring"));

        restore_env_var(TEST_KEYRING_DIR_ENV, Some(seeded_value.as_os_str()));
        assert_eq!(std::env::var_os(TEST_KEYRING_DIR_ENV), Some(seeded_value));

        restore_env_var(TEST_KEYRING_DIR_ENV, original_keyring_dir.as_deref());
    }
}
