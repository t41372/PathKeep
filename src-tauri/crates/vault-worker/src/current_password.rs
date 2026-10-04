//! Proof of the archive's current password before a rekey.
//!
//! Responsibilities:
//! - Refuse to change the password of, or decrypt, an encrypted archive unless
//!   the request carries the archive's current password and that password
//!   actually decrypts the file on disk.
//! - Hand the verified password back, so the rekey opens the archive with it
//!   rather than with whatever key the session happens to hold.
//!
//! Not responsible for: the rekey itself (`vault_core::rekey_archive`), the
//! App Lock passcode (`vault_core::app_lock`), or the keychain.
//!
//! Why this exists: an unlocked window already holds the session key, so
//! without this check anyone sitting at it could re-encrypt the archive with a
//! password of their own (locking the owner out) or write it out in plaintext.
//!
//! Failure modes this guards against, each tested below or in `tests.rs` /
//! the dev-bridge dispatch tests:
//! 1. A wrong password is accepted: refused for preview and execute, and the
//!    archive is untouched afterwards (the old password still opens it).
//! 2. The right password is refused: including passwords with quotes and
//!    non-ASCII characters, which must reach SQLCipher exactly as onboarding
//!    stored them.
//! 3. The check is bypassed through the dev bridge or an older client that
//!    sends no `currentKey`: both transports go through the same worker call,
//!    a missing password is refused, and the session key is never used in its
//!    place.
//! 4. A timing leak: no password bytes are ever compared; the only cost is
//!    SQLCipher's key derivation, which is the same for every candidate.
//! 5. The password is logged: `RekeyRequest`'s `Debug` redacts both keys, and
//!    no refusal message contains the candidate.
//! 6. An I/O failure (missing file, busy database) is reported as a wrong
//!    password: only SQLCipher's "not a database" answer counts as wrong.

use crate::app::RekeyRequest;
use anyhow::{Context, Result};
use vault_core::{AppConfig, ArchiveMode, ProjectPaths};

/// Refusal when an encrypted archive is rekeyed without its current password.
/// `command_error.rs` classifies this sentence as `archive-password-required`.
pub const REKEY_CURRENT_PASSWORD_REQUIRED: &str =
    "Enter the current archive password to change or remove encryption.";

/// Refusal when the current password given for a rekey does not decrypt the
/// archive. `command_error.rs` classifies this sentence as `archive-password-wrong`.
pub const REKEY_CURRENT_PASSWORD_WRONG: &str =
    "The current archive password did not open the archive.";

/// Returns the archive's current key after proving the caller knows it, or
/// `None` for a plaintext archive (encrypting one needs no password).
///
/// The proof is opening the archive file with the candidate
/// ([`vault_core::archive_key_opens`]), not a comparison with the session key:
/// - The file is the only authority. The session key can be absent (a caller
///   of the dev bridge that never set one), or came from the keychain and was
///   never typed by the person at the window; neither says what the file
///   accepts, and neither proves the person knows the password.
/// - Nothing is compared byte by byte, so there is no early exit to time.
///
/// The verified key is then the key the rekey opens the archive with, so a
/// caller that skipped this check would still fail on a wrong password.
pub(crate) fn verified_current_key<'a>(
    paths: &ProjectPaths,
    config: &AppConfig,
    request: &'a RekeyRequest,
) -> Result<Option<&'a str>> {
    if !matches!(config.archive_mode, ArchiveMode::Encrypted) {
        return Ok(None);
    }
    let candidate = request
        .current_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .context(REKEY_CURRENT_PASSWORD_REQUIRED)?;
    if !vault_core::archive_key_opens(paths, candidate)? {
        anyhow::bail!(REKEY_CURRENT_PASSWORD_WRONG);
    }
    Ok(Some(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vault_core::{ensure_archive_initialized, project_paths_with_root};

    const PASSWORD: &str = "it's a «secret» 密碼";

    fn encrypted_archive() -> (tempfile::TempDir, ProjectPaths, AppConfig) {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Encrypted,
            ..AppConfig::default()
        };
        ensure_archive_initialized(&paths, &config, Some(PASSWORD)).expect("create archive");
        (dir, paths, config)
    }

    fn request(current_key: Option<&str>) -> RekeyRequest {
        RekeyRequest {
            new_mode: ArchiveMode::Plaintext,
            new_key: None,
            current_key: current_key.map(str::to_string),
        }
    }

    #[test]
    fn the_password_the_archive_was_created_with_is_accepted() {
        let (_dir, paths, config) = encrypted_archive();
        let request = request(Some(PASSWORD));
        let verified = verified_current_key(&paths, &config, &request).expect("right password");
        assert_eq!(verified, Some(PASSWORD));
    }

    #[test]
    fn a_wrong_or_missing_password_is_refused_without_echoing_it() {
        let (_dir, paths, config) = encrypted_archive();

        for wrong in ["it's a «secret» 密", "IT'S A «SECRET» 密碼", "x'00'"] {
            let error = verified_current_key(&paths, &config, &request(Some(wrong)))
                .expect_err("wrong password");
            let message = format!("{error:#}");
            assert_eq!(message, REKEY_CURRENT_PASSWORD_WRONG);
            assert!(!message.contains(wrong));
        }
        for missing in [None, Some("")] {
            let error = verified_current_key(&paths, &config, &request(missing))
                .expect_err("missing password");
            assert_eq!(format!("{error:#}"), REKEY_CURRENT_PASSWORD_REQUIRED);
            // Must not read as "unlock first": that code sends the shell to the unlock gate.
            assert!(!format!("{error:#}").contains("database key is required"));
        }
    }

    #[test]
    fn a_plaintext_archive_needs_no_password() {
        let dir = tempdir().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        let config = AppConfig { initialized: true, ..AppConfig::default() };
        ensure_archive_initialized(&paths, &config, None).expect("create archive");
        let encrypt = RekeyRequest {
            new_mode: ArchiveMode::Encrypted,
            new_key: Some("new password".to_string()),
            current_key: None,
        };
        assert_eq!(verified_current_key(&paths, &config, &encrypt).expect("plaintext"), None);
    }

    #[test]
    fn a_missing_archive_is_an_error_not_a_wrong_password() {
        let (_dir, paths, config) = encrypted_archive();
        std::fs::remove_file(&paths.archive_database_path).expect("remove archive");
        let error = verified_current_key(&paths, &config, &request(Some(PASSWORD)))
            .expect_err("missing archive");
        assert!(!format!("{error:#}").contains(REKEY_CURRENT_PASSWORD_WRONG));
    }

    #[test]
    fn debug_output_never_shows_either_key() {
        let request = RekeyRequest {
            new_mode: ArchiveMode::Encrypted,
            new_key: Some("new-secret-value".to_string()),
            current_key: Some("old-secret-value".to_string()),
        };
        let printed = format!("{request:?}");
        assert!(!printed.contains("new-secret-value"));
        assert!(!printed.contains("old-secret-value"));
        assert!(printed.contains("<redacted>"));
    }
}
