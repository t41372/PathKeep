//! Responsibilities: map backend failures to stable desktop error codes and recovery hints.
//! Not responsible for: performing recovery or translating frontend copy.
//! Failure modes: wrong-password codes changed, digest failures obscured or a published archive
//! reported as rolled back.
//!
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

/// Machine-readable code: a request to change or remove archive encryption
/// came without the archive's current password.
pub const ERROR_CODE_ARCHIVE_PASSWORD_REQUIRED: &str = "archive-password-required";

/// Machine-readable code: the current archive password given to change or
/// remove encryption does not open the archive.
pub const ERROR_CODE_ARCHIVE_PASSWORD_WRONG: &str = "archive-password-wrong";

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

/// The affected archive member, for a separate path line and Reveal in Folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffectedFile {
    path: String,
    role: &'static str,
}

impl AffectedFile {
    fn new(path: &std::path::Path, role: vault_store::ArchiveFileRole) -> Self {
        use vault_store::ArchiveFileRole::*;
        Self {
            path: path.display().to_string(),
            role: match role {
                Browsing => "browsing",
                Evidence => "evidence",
                ReadModel => "readModel",
                HeldGeneration => "heldGeneration",
            },
        }
    }
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_file: Option<Box<AffectedFile>>,
}

impl CommandError {
    /// Wraps an infrastructure failure (thread-pool join, payload parse) that
    /// carries no user remediation — no code, hints, or classification.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: None,
            action_hint: None,
            retry_hint: None,
            affected_file: None,
        }
    }

    #[cfg(any(not(test), feature = "devtools-bridge"))]
    pub(crate) fn invalid_request(error: impl std::fmt::Display) -> Self {
        Self { code: Some("invalid-request".into()), ..Self::internal(error.to_string()) }
    }

    /// Recover structured file identity before formatting the diagnostic chain for display.
    pub(crate) fn from_backend(error: &anyhow::Error) -> Self {
        use vault_store::Error;
        let mut envelope = Self::classified(format!("{error:#}"));
        if let Some(cause) = error.downcast_ref::<vault_core::AiRefusal>() {
            envelope.code = Some(cause.code().into());
            return envelope;
        }
        if let Some(member) = error.downcast_ref::<vault_core::maintenance::SnapshotMemberFailure>()
        {
            envelope.affected_file = Some(Box::new(AffectedFile::new(&member.path, member.role)));
            envelope.code = Some(member.code.into());
            envelope.retry_hint = Some(RETRY_HINT_AFTER_ACTION.into());
            return envelope;
        }
        for cause in error.chain().filter_map(|e| e.downcast_ref::<Error>()) {
            let (path, role, remediation) = match cause {
                Error::PreserveFile { path, role, .. } => {
                    (path, *role, Some(("conversation-preservation-failed", None)))
                }
                Error::WriteFile { path, role, .. } => {
                    (path, *role, Some(("archive-file-write-failed", None)))
                }
                Error::Open(f) | Error::RepairRequired(f) | Error::AccessUnavailable(f) => {
                    let path = match &f.cause {
                        // A companion or held handle carries its own more precise location.
                        Error::PreserveFile { .. } | Error::WriteFile { .. } => continue,
                        Error::MissingStore(path) | Error::ReplacedFile(path) => path,
                        _ => &f.path,
                    };
                    let remediation = match cause {
                        Error::RepairRequired(_) => Some((
                            "conversation-repair-required",
                            Some("preview_agent_history_repair"),
                        )),
                        Error::AccessUnavailable(_) => {
                            Some(("conversation-access-unavailable", Some("retry_agent_store")))
                        }
                        _ => None,
                    };
                    (path, f.role.into(), remediation)
                }
                _ => continue,
            };
            envelope.affected_file = Some(Box::new(AffectedFile::new(path, role)));
            if let Some((code, action)) = remediation {
                envelope.code = Some(code.into());
                envelope.action_hint = action.map(str::to_owned);
                envelope.retry_hint = Some(RETRY_HINT_AFTER_ACTION.into());
            }
            break;
        }
        envelope
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
            affected_file: None,
        }
    }
}

/// Codes the backend writes as the first word of a failure (`code: detail`). The archive
/// refusals (`vault_core::admission::Reason`) are classified the same way, from their own list.
const CODED_FAILURES: [&str; 24] = [
    "envelope-missing",
    "archive-password-wrong",
    "archive-busy",
    "keychain-key-missing",
    "invalid-keychain-key",
    "keychain-unavailable",
    "archive-password-required",
    "archive-open-panicked",
    "invalid-cursor",
    "invalid-page-size",
    "copy-digest-mismatch",
    "conversion-digest-mismatch",
    "snapshot-digest-mismatch",
    "encryption-mode-change-requires-rekey",
    "archive-installed-keychain-unavailable",
    "archive-conversion-required",
    "export-target-is-managed",
    "snapshot-not-listed",
    "not-in-history",
    "backup-no-profiles",
    "archive-wipe-unfinished",
    "archive-wipe-stopped",
    "move-file-too-new",
    "archive-store-missing",
];

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
/// - a rekey refused for a missing or wrong current archive password carries
///   the sentences exported by `vault-worker`'s `current_password` module.
///   Neither is lock-required: the archive is open; the person only has to
///   show they know its password.
/// - every other code is the first word of a failure (`code: detail`), listed in
///   [`CODED_FAILURES`] or an archive refusal's; when a chain holds several, the
///   first one names the failure.
fn classify_message(message: &str) -> (Option<&'static str>, Option<&'static str>) {
    if message.starts_with("conversation-access-unavailable:") {
        return (Some("conversation-access-unavailable"), Some("retry_agent_store"));
    }
    if message.starts_with("conversation-repair-required:") {
        return (Some("conversation-repair-required"), Some("preview_agent_history_repair"));
    }
    for code in [
        "conversation-preservation-failed",
        "archive-file-write-failed",
        "conversation-copy-no-verified-evidence",
        "conversation-copy-not-listed",
        "conversation-copy-incomplete",
        "conversation-copy-invalid-seal",
        "conversation-schema-too-new",
        "conversation-copy-duplicate-role",
        "conversation-copy-unsafe-path",
        "conversation-repair-preview-stale",
        "conversation-repair-healthy",
        "conversation-evidence-readable",
        "conversation-pair-unverified",
    ] {
        if message == code || message.strip_prefix(code).is_some_and(|rest| rest.starts_with(':')) {
            return (Some(code), None);
        }
    }
    let refusals = vault_core::admission::Reason::ALL.map(vault_core::admission::Reason::code);
    // The first code in the chain wins: the outermost context is what the operation concluded and
    // a code further in only its cause; a code inside a longer one (`keychain-unavailable` in
    // `archive-installed-keychain-unavailable`) starts after the longer one does.
    let first = CODED_FAILURES
        .iter()
        .chain(&refusals)
        .filter_map(|code| message.find(&format!("{code}:")).map(|at| (at, *code)))
        .min_by_key(|(at, _)| *at);
    if let Some((_, code)) = first {
        return (Some(code), None);
    }
    if message.contains("backup-already-running:") {
        return (Some("backup-already-running"), None);
    }

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
    if message.contains(vault_worker::REKEY_CURRENT_PASSWORD_WRONG) {
        return (Some(ERROR_CODE_ARCHIVE_PASSWORD_WRONG), None);
    }
    if message.contains(vault_worker::REKEY_CURRENT_PASSWORD_REQUIRED) {
        return (Some(ERROR_CODE_ARCHIVE_PASSWORD_REQUIRED), None);
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

    /// Every reason an archive or a file is refused for reaches the window with its code,
    /// wherever in the cause chain the refusal sits, so the window can say it in the person's
    /// language instead of showing the English chain.
    #[test]
    fn every_refusal_reaches_the_window_with_its_code() {
        for reason in vault_core::admission::Reason::ALL {
            let message = format!("importing the file: {}: needs level 2", reason.code());
            assert_eq!(
                CommandError::classified(message).code.as_deref(),
                Some(reason.code()),
                "{reason:?}"
            );
        }
        for code in ["move-file-too-new", "archive-store-missing"] {
            let message = format!("checking the file: {code}: details");
            assert_eq!(CommandError::classified(message).code.as_deref(), Some(code));
        }
        assert_eq!(CommandError::classified("restoring: disk full".to_string()).code, None);
    }

    /// The first code in the chain is what the operation concluded; a code further in is only
    /// its cause, and a code inside a longer one is not that code.
    #[test]
    fn the_outermost_code_names_the_failure() {
        let wipe = "archive-wipe-unfinished: everything was moved out of the archive: \
                    clearing the keychain: keychain-unavailable: the keychain is locked";
        assert_eq!(
            CommandError::classified(wipe.to_string()).code.as_deref(),
            Some("archive-wipe-unfinished")
        );
        let installed = "archive-installed-keychain-unavailable: the new archive is in place";
        assert_eq!(
            CommandError::classified(installed.to_string()).code.as_deref(),
            Some("archive-installed-keychain-unavailable")
        );
    }
}
