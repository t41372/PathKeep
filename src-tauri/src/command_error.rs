//! Structured error envelope for the desktop command surface.
//!
//! `docs/architecture/module-boundary-map.md` requires the user-facing error
//! model to carry `error_code` / `action_hint` / `retry_hint` rather than a
//! bare string. This type is that envelope: `message` keeps the full `anyhow`
//! cause chain for diagnostics and bug reports, while `code` carries the
//! machine-readable classification the frontend uses for remediation flows
//! (the unlock gate, the Full Disk Access repair button) instead of sniffing
//! message text — which historically drifted into three divergent frontend
//! implementations, two of them matching the frontend's own translations.
//!
//! Classification lives here, on the backend side of the IPC boundary,
//! because this is the layer that owns the canonical marker strings the
//! backup/ingest/app-lock paths guarantee (`browser_access.rs` for Full Disk
//! Access, `app_lock.rs` / `archive::schema` for lock refusals).

use serde::Serialize;

/// Machine-readable code: the archive is encrypted or App Lock is engaged and
/// the operation needs an unlock first. The shell's unlock gate consumes this.
pub const ERROR_CODE_LOCK_REQUIRED: &str = "lock-required";

/// Machine-readable code: macOS denied browser-history reads because PathKeep
/// lacks the Full Disk Access (TCC) entitlement.
pub const ERROR_CODE_FULL_DISK_ACCESS: &str = "full-disk-access";

/// Machine-readable code: the host cannot offer biometric unlock right now
/// (Touch ID unavailable on this Mac, or a build without biometric support).
pub const ERROR_CODE_BIOMETRIC_UNAVAILABLE: &str = "biometric-unavailable";

/// Machine-readable code: biometric hardware is present but no credentials
/// (fingerprints) are enrolled on the host.
pub const ERROR_CODE_BIOMETRIC_NOT_ENROLLED: &str = "biometric-not-enrolled";

/// Machine-readable code: the host has locked biometric authentication out
/// (too many failed attempts) until the user unlocks it at the OS level.
pub const ERROR_CODE_BIOMETRIC_LOCKOUT: &str = "biometric-lockout";

/// Machine-readable code: the biometric prompt ended without authenticating —
/// the user (or the system/PathKeep) canceled it or chose the passcode path.
pub const ERROR_CODE_BIOMETRIC_CANCELED: &str = "biometric-canceled";

/// Machine-readable code: biometric unlock is switched off in PathKeep's own
/// Settings, so the prompt was refused before reaching the host.
pub const ERROR_CODE_BIOMETRIC_TURNED_OFF: &str = "biometric-turned-off";

/// Machine-readable code: the biometric prompt ran but did not verify the
/// user (mismatch, interruption, expiry, or timeout). Retry or use passcode.
pub const ERROR_CODE_BIOMETRIC_FAILED: &str = "biometric-failed";

/// Action hint: the frontend should surface its "open Full Disk Access
/// settings" repair affordance.
pub const ACTION_HINT_OPEN_FULL_DISK_ACCESS: &str = "open-full-disk-access-settings";

/// Action hint: the frontend should route the user through the unlock gate.
pub const ACTION_HINT_UNLOCK: &str = "unlock";

/// Action hint: the frontend should steer the user to the app-lock passcode
/// field, which is the honest fallback for every biometric refusal.
pub const ACTION_HINT_USE_PASSCODE: &str = "use-app-lock-passcode";

/// Retry hint: the failed operation is expected to succeed once the user has
/// completed the hinted action.
pub const RETRY_HINT_AFTER_ACTION: &str = "retry-after-action";

/// The user-facing error envelope every desktop command returns on failure.
///
/// `message` is the full cause chain (`"top: cause: root"`); it is diagnostic
/// content, not translation input. `code` / `action_hint` / `retry_hint` are
/// the stable machine-readable fields the frontend keys behavior and i18n on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_hint: Option<String>,
}

impl CommandError {
    /// Wraps an infrastructure failure (thread-pool join, payload parse) that
    /// carries no user remediation — no code, hints, or classification.
    pub fn internal(message: impl Into<String>) -> Self {
        Self { message: message.into(), code: None, action_hint: None, retry_hint: None }
    }

    /// Builds the envelope for a worker/core failure, classifying the message
    /// against the backend's canonical marker strings.
    pub fn classified(message: String) -> Self {
        let (code, action_hint) = classify_message(&message);
        let retry_hint = code.map(|_| RETRY_HINT_AFTER_ACTION.to_string());
        Self {
            message,
            code: code.map(str::to_string),
            action_hint: action_hint.map(str::to_string),
            retry_hint,
        }
    }
}

/// Maps a formatted error chain onto `(code, action_hint)` using the marker
/// strings the backend itself guarantees:
///
/// - `browser_access.rs` routes every macOS TCC denial into copy containing
///   the ASCII `"Full Disk Access"` marker (with the legacy Safari-specific
///   `"Safari History.db is not readable yet"` variant still produced by the
///   ingest skip path).
/// - `app_lock.rs` refusals say `"PathKeep is currently locked"`; encrypted
///   archives without a session key fail `"database key is required"`
///   (`archive::schema` / `source_evidence` / `intelligence_projection`).
/// - biometric unlock failures come from the canonical sentences produced by
///   `vault-platform`'s `map_touch_id_error` and `vault-core`'s `app_lock.rs`
///   biometric branch. Only those exact marker phrases classify; an arbitrary
///   platform `localizedDescription` passed through the Touch ID fallback arm
///   stays uncoded so the shell renders it verbatim. A wrong app-lock passcode
///   (`"The app lock passcode did not match."`) is deliberately NEVER
///   classified: it is a user input error, and coding it (especially as
///   lock-required) would loop the unlock gate on itself.
fn classify_message(message: &str) -> (Option<&'static str>, Option<&'static str>) {
    if message.contains("Full Disk Access")
        || message.contains("Safari History.db is not readable yet")
    {
        return (Some(ERROR_CODE_FULL_DISK_ACCESS), Some(ACTION_HINT_OPEN_FULL_DISK_ACCESS));
    }
    if message.contains("database key is required")
        || message.contains("PathKeep is currently locked")
    {
        return (Some(ERROR_CODE_LOCK_REQUIRED), Some(ACTION_HINT_UNLOCK));
    }
    if let Some(code) = classify_biometric_message(message) {
        return (Some(code), Some(ACTION_HINT_USE_PASSCODE));
    }
    (None, None)
}

/// Maps the canonical biometric-unlock failure sentences onto stable codes.
///
/// Each marker below is a phrase the backend itself guarantees (LAError code →
/// sentence in `map_touch_id_error`, plus the `app_lock.rs` refusals), listed
/// most-specific first so "no fingerprints are enrolled" never falls through
/// to a generic bucket.
fn classify_biometric_message(message: &str) -> Option<&'static str> {
    if message.contains("no fingerprints are enrolled") {
        return Some(ERROR_CODE_BIOMETRIC_NOT_ENROLLED);
    }
    if message.contains("Touch ID is locked out") {
        return Some(ERROR_CODE_BIOMETRIC_LOCKOUT);
    }
    if message.contains("Touch ID is unavailable on this Mac")
        || message.contains("Biometric unlock is not available in the current desktop build")
    {
        return Some(ERROR_CODE_BIOMETRIC_UNAVAILABLE);
    }
    if message.contains("Touch ID unlock was canceled") || message.contains("Touch ID was skipped")
    {
        return Some(ERROR_CODE_BIOMETRIC_CANCELED);
    }
    if message.contains("Biometric unlock is currently turned off in Settings") {
        return Some(ERROR_CODE_BIOMETRIC_TURNED_OFF);
    }
    if message.contains("Touch ID could not verify your identity")
        || message.contains("Touch ID unlock was interrupted")
        || message.contains("Touch ID unlock is no longer valid")
        || message.contains("Touch ID unlock failed")
        || message.contains("Touch ID did not finish before PathKeep timed out")
    {
        return Some(ERROR_CODE_BIOMETRIC_FAILED);
    }
    None
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::classified(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_full_disk_access_markers() {
        let generic = CommandError::classified(
            "backup failed: reading Safari history: Full Disk Access is required".to_string(),
        );
        assert_eq!(generic.code.as_deref(), Some(ERROR_CODE_FULL_DISK_ACCESS));
        assert_eq!(generic.action_hint.as_deref(), Some(ACTION_HINT_OPEN_FULL_DISK_ACCESS));
        assert_eq!(generic.retry_hint.as_deref(), Some(RETRY_HINT_AFTER_ACTION));

        let legacy = CommandError::classified(
            "Skipped `Safari` because Safari History.db is not readable yet.".to_string(),
        );
        assert_eq!(legacy.code.as_deref(), Some(ERROR_CODE_FULL_DISK_ACCESS));
    }

    #[test]
    fn classifies_lock_required_markers() {
        let encrypted = CommandError::classified(
            "open archive: database key is required for encrypted archives".to_string(),
        );
        assert_eq!(encrypted.code.as_deref(), Some(ERROR_CODE_LOCK_REQUIRED));
        assert_eq!(encrypted.action_hint.as_deref(), Some(ACTION_HINT_UNLOCK));

        let app_lock = CommandError::classified(
            "PathKeep is currently locked. Unlock the app before requesting archive data."
                .to_string(),
        );
        assert_eq!(app_lock.code.as_deref(), Some(ERROR_CODE_LOCK_REQUIRED));
    }

    #[test]
    fn leaves_unrelated_errors_unclassified() {
        let error = CommandError::classified("disk full".to_string());
        assert_eq!(error.code, None);
        assert_eq!(error.action_hint, None);
        assert_eq!(error.retry_hint, None);
    }

    #[test]
    fn classifies_biometric_markers_from_their_producing_sentences() {
        // Every case uses the exact sentence the backend produces
        // (`vault-platform/src/biometric.rs` / `vault-core/src/app_lock.rs`),
        // because those sentences ARE the canonical markers.
        let cases = [
            (
                "Touch ID is available on this Mac, but no fingerprints are enrolled. Use the app lock passcode instead.",
                ERROR_CODE_BIOMETRIC_NOT_ENROLLED,
            ),
            (
                "Touch ID is locked out on this Mac right now. Unlock it in macOS or use the app lock passcode instead.",
                ERROR_CODE_BIOMETRIC_LOCKOUT,
            ),
            (
                "Touch ID is unavailable on this Mac right now. Use the app lock passcode instead.",
                ERROR_CODE_BIOMETRIC_UNAVAILABLE,
            ),
            (
                "Biometric unlock is not available in the current desktop build.",
                ERROR_CODE_BIOMETRIC_UNAVAILABLE,
            ),
            ("Touch ID unlock was canceled.", ERROR_CODE_BIOMETRIC_CANCELED),
            ("Touch ID unlock was canceled by PathKeep.", ERROR_CODE_BIOMETRIC_CANCELED),
            (
                "Touch ID was skipped. Use the app lock passcode instead.",
                ERROR_CODE_BIOMETRIC_CANCELED,
            ),
            (
                "Biometric unlock is currently turned off in Settings.",
                ERROR_CODE_BIOMETRIC_TURNED_OFF,
            ),
            (
                "Touch ID could not verify your identity. Try again or use the app lock passcode.",
                ERROR_CODE_BIOMETRIC_FAILED,
            ),
            (
                "Touch ID unlock was interrupted by macOS. Try again or use the app lock passcode.",
                ERROR_CODE_BIOMETRIC_FAILED,
            ),
            ("Touch ID unlock is no longer valid. Try again.", ERROR_CODE_BIOMETRIC_FAILED),
            (
                "Touch ID unlock failed. Use the app lock passcode instead.",
                ERROR_CODE_BIOMETRIC_FAILED,
            ),
            (
                "Touch ID did not finish before PathKeep timed out. Try again or use the app lock passcode.",
                ERROR_CODE_BIOMETRIC_FAILED,
            ),
        ];
        for (message, expected_code) in cases {
            let error = CommandError::classified(message.to_string());
            assert_eq!(error.code.as_deref(), Some(expected_code), "message: {message}");
            assert_eq!(error.action_hint.as_deref(), Some(ACTION_HINT_USE_PASSCODE));
            assert_eq!(error.retry_hint.as_deref(), Some(RETRY_HINT_AFTER_ACTION));
        }
    }

    #[test]
    fn keeps_wrong_passcode_and_opaque_touch_id_descriptions_unclassified() {
        // A wrong passcode is a user input error, not an "unlock first"
        // refusal — classifying it would loop the unlock gate on itself.
        let wrong_passcode =
            CommandError::classified("The app lock passcode did not match.".to_string());
        assert_eq!(wrong_passcode.code, None);
        assert_eq!(wrong_passcode.action_hint, None);

        // An arbitrary platform `localizedDescription` passed through the
        // Touch ID fallback arm is not a canonical marker and stays uncoded so
        // the shell renders the diagnostic prose verbatim.
        let opaque = CommandError::classified("Biometry rejected by policy 0x2f.".to_string());
        assert_eq!(opaque.code, None);
    }

    #[test]
    fn internal_never_classifies() {
        let error = CommandError::internal("join failed: Full Disk Access text in a panic");
        assert_eq!(error.code, None);
    }

    #[test]
    fn display_and_from_string_round_trip_the_message() {
        let error = CommandError::from("plain failure".to_string());
        assert_eq!(error.to_string(), "plain failure");
    }

    #[test]
    fn serializes_camel_case_and_skips_empty_hints() {
        let plain = serde_json::to_value(CommandError::internal("boom")).expect("serialize");
        assert_eq!(plain, serde_json::json!({ "message": "boom" }));

        let classified = serde_json::to_value(CommandError::classified(
            "database key is required for encrypted archives".to_string(),
        ))
        .expect("serialize");
        assert_eq!(classified["code"], "lock-required");
        assert_eq!(classified["actionHint"], "unlock");
        assert_eq!(classified["retryHint"], "retry-after-action");
    }
}
