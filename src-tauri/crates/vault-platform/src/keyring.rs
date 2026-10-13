//! Native keyring adapter.
//!
//! This module is the only place that should know how PathKeep stores secrets
//! on the host. It supports native backends in production and a file-backed
//! override for tests so higher layers can stay deterministic.

use crate::test_support::{keyring_service, test_keyring_dir};
use anyhow::{Context, Result};
#[cfg(target_os = "macos")]
use apple_native_keyring_store::keychain::Store as NativeKeyringStore;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use dbus_secret_service_keyring_store::Store as NativeKeyringStore;
use keyring_core::{Entry, get_default_store, set_default_store};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use vault_core::KeyringStatusReport;
#[cfg(target_os = "windows")]
use windows_native_keyring_store::Store as NativeKeyringStore;

const KEYRING_DATABASE_USER: &str = "database-key";

/// How long a native keychain "is a key stored?" answer is reused. Settings and every config load
/// ask once per AI provider, and an OS keychain round trip can take tens of milliseconds.
const PROVIDER_KEY_SAVED_TTL: Duration = Duration::from_secs(60);

fn provider_key_saved_cache() -> &'static Mutex<HashMap<String, (bool, Instant)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (bool, Instant)>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn forget_provider_key_saved(user: &str) {
    provider_key_saved_cache().lock().expect("provider key cache lock").remove(user);
}

fn provider_keyring_user(provider_id: &str) -> String {
    format!("ai-provider::{provider_id}")
}

fn keyring_entry(user: &str) -> Result<Entry> {
    ensure_native_keyring_store()?;
    Ok(Entry::new(&keyring_service(), user)?)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux", target_os = "freebsd",))]
fn ensure_native_keyring_store() -> Result<()> {
    if get_default_store().is_none() {
        set_default_store(
            NativeKeyringStore::new().context("initializing native keyring backend")?,
        );
    }
    Ok(())
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "freebsd",
)))]
fn ensure_native_keyring_store() -> Result<()> {
    anyhow::bail!("A native keyring backend is not available on this machine.")
}

#[cfg(target_os = "macos")]
fn keyring_entry_exists_for_service(service: &str, user: &str) -> bool {
    ensure_native_keyring_store().ok();
    Entry::search(&HashMap::from([("service", service), ("user", user)]))
        .map(|entries| !entries.is_empty())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
fn keyring_entry_exists_for_service(service: &str, user: &str) -> bool {
    ensure_native_keyring_store().ok();
    Entry::new(service, user).ok().and_then(|entry| entry.get_password().ok()).is_some()
}

fn keyring_entry_exists(user: &str) -> bool {
    keyring_entry_exists_for_service(&keyring_service(), user)
}

#[cfg(target_os = "macos")]
fn keyring_backend_name() -> &'static str {
    "macOS Keychain"
}

#[cfg(target_os = "windows")]
fn keyring_backend_name() -> &'static str {
    "Windows Credential Manager"
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn keyring_backend_name() -> &'static str {
    "Linux Secret Service / keyutils"
}

/// Reports whether a usable keyring backend exists and whether a database key is stored.
pub fn keyring_status() -> KeyringStatusReport {
    if let Some(path) = test_keyring_dir() {
        let available = test_keyring_available(&path).is_ok();
        let stored_secret = available && test_keyring_path(&path, KEYRING_DATABASE_USER).exists();
        return KeyringStatusReport {
            available,
            backend: "File-backed test keyring".to_string(),
            stored_secret,
            message: (!available)
                .then(|| "A native keyring backend is not available on this machine.".to_string()),
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

/// Stores an AI provider API key in the current backend.
pub fn keyring_set_provider_api_key(provider_id: &str, api_key: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, &user, api_key);
    }

    ensure_native_keyring_store()?;
    let entry = keyring_entry(&user)?;
    let result = entry.set_password(api_key);
    forget_provider_key_saved(&user);
    result?;
    Ok(())
}

/// Removes an AI provider API key from the current backend.
pub fn keyring_clear_provider_api_key(provider_id: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, &user);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(&user)?;
    let _ = entry.delete_credential();
    forget_provider_key_saved(&user);
    Ok(())
}

/// Returns whether an AI provider API key is currently stored in the active backend.
pub fn provider_api_key_saved(provider_id: &str) -> bool {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_path(&path, &user).exists();
    }

    let cache = provider_key_saved_cache();
    if let Some((saved, at)) = cache.lock().expect("provider key cache lock").get(&user)
        && at.elapsed() < PROVIDER_KEY_SAVED_TTL
    {
        return *saved;
    }
    let saved = keyring_entry_exists_for_service(&keyring_service(), &user);
    cache.lock().expect("provider key cache lock").insert(user, (saved, Instant::now()));
    saved
}

fn test_keyring_path(root: &Path, user: &str) -> PathBuf {
    root.join(format!("{}-{user}.secret", keyring_service()))
}

/// Fails as the native store does where there is none, once a test made it unavailable
/// (`TEST_KEYRING_UNAVAILABLE_FILE`).
fn test_keyring_available(root: &Path) -> Result<()> {
    if root.join(crate::test_support::TEST_KEYRING_UNAVAILABLE_FILE).exists() {
        anyhow::bail!("A native keyring backend is not available on this machine.");
    }
    Ok(())
}

fn test_keyring_get(root: &Path, user: &str) -> Result<Option<String>> {
    test_keyring_available(root)?;
    #[cfg(debug_assertions)]
    crate::test_support::record_keyring_secret_read(root);
    let path = test_keyring_path(root, user);
    if !path.exists() {
        return Ok(None);
    }
    let value = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(value))
}

fn test_keyring_set(root: &Path, user: &str, value: &str) -> Result<()> {
    test_keyring_available(root)?;
    fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
    let path = test_keyring_path(root, user);
    fs::write(&path, value).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn test_keyring_clear(root: &Path, user: &str) -> Result<()> {
    test_keyring_available(root)?;
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
}
