//! Shared helpers for desktop crate tests that mutate process-wide state.
//!
//! ## Responsibilities
//!
//! - Provide one lock for tests that temporarily override PathKeep environment variables.
//! - Keep override variable names in one place so desktop tests do not drift.
//!
//! ## Not responsible for
//!
//! - Creating archive fixtures or browser-history databases.
//! - Hiding tests that should avoid global process state.
//!
//! ## Dependencies
//!
//! - Standard-library synchronization primitives only.
//!
//! ## Performance notes
//!
//! The lock is intentionally coarse because the guarded tests touch process
//! environment variables. It only serializes fixture setup/teardown paths.

use std::sync::{Mutex, MutexGuard, OnceLock};

/// Env var used by tests to redirect the app root away from the user's real data.
pub(crate) const PROJECT_ROOT_OVERRIDE_ENV: &str = "CHB_PROJECT_ROOT";
/// Env var used by tests to point browser discovery at a synthetic Chrome root.
pub(crate) const CHROME_USER_DATA_OVERRIDE_ENV: &str = "CHB_CHROME_USER_DATA_DIR";
/// Env var used by tests to isolate local keyring/secret fixtures.
pub(crate) const TEST_KEYRING_OVERRIDE_ENV: &str = "CHB_TEST_KEYRING_DIR";

/// Serializes tests that temporarily mutate PathKeep process environment.
pub(crate) fn lock_env() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
