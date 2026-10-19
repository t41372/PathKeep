//! Shared classification of browser-access failures into stable front-end markers.
//!
//! ## Responsibilities
//! - Decide whether a browser-history access failure is a macOS Full Disk Access
//!   (TCC / `EACCES` / `EPERM`) denial versus an ordinary missing/corrupt/locked
//!   file.
//! - Own the STABLE marker strings the front end keys on. The front end owns ALL
//!   localized, user-facing copy; the backend only ever emits these markers.
//!
//! ## Not responsible for
//! - Any user-facing / localized copy (the front end renders it from the marker).
//! - Logging or I/O — `vault-core` stays log-free; callers decide how to surface a
//!   result.
//!
//! ## Why it exists
//! The onboarding snapshot path (`vault-worker::app::snapshot_browser_profiles`) and
//! the backup path (`archive::backup::classify_browser_access_error`) must reach the
//! SAME "is this a Full Disk Access problem?" verdict. Centralizing the judgement
//! keeps the two paths from drifting: a denial has to look identical whether the user
//! hits it during onboarding's "Choose browsers" step or during a backup run.

/// Marker: a macOS Full Disk Access denial was detected. The front end renders its
/// localized "grant Full Disk Access" guidance from this stable string.
pub const DISCOVERY_ISSUE_FULL_DISK_ACCESS: &str = "macos-full-disk-access";

/// Marker: browser discovery failed for a reason OTHER than a permission denial.
/// Surfaced (never hidden) so a real failure is visible instead of masquerading as
/// "you have no browsers installed".
pub const DISCOVERY_ISSUE_DISCOVERY_ERROR: &str = "discovery-error";

/// Marker: a specific history file exists but could not be read for a NON-permission
/// reason (corrupt / locked / transient I/O). Distinct from a Full Disk Access denial,
/// which resolves to [`DISCOVERY_ISSUE_FULL_DISK_ACCESS`].
pub const HISTORY_ISSUE_FILE_NOT_READABLE: &str = "history-file-not-readable";

/// Whether the current host treats browser-data permission denials as a macOS Full
/// Disk Access (TCC) problem.
///
/// Only macOS gates third-party browser data (under `~/Library/Application Support/…`)
/// and Safari data behind TCC, so this is the single OS-specific fact the classifiers
/// consume. It is expressed with `cfg!` rather than `#[cfg]` so BOTH outcomes stay
/// compiled and measured in the coverage build; only the returned constant differs per
/// target. Callers must never emit the Full Disk Access marker when this is `false`.
pub const fn full_disk_access_applies() -> bool {
    cfg!(target_os = "macos")
}

/// Returns whether an error chain represents an OS permission denial (macOS TCC /
/// `EACCES` / `EPERM`) rather than a missing/corrupt/locked file.
///
/// Mirrors the exact detection both consuming paths need: a `PermissionDenied`
/// [`std::io::Error`] anywhere in the chain, OR an "Operation not permitted" substring
/// — some layers stringize the cause and lose the typed `io::Error`, so the substring
/// fallback keeps the verdict stable across them.
pub fn is_permission_denied(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied)
    }) || format!("{error:#}").contains("Operation not permitted")
}

/// Classifies a single history file's read failure into a stable marker.
///
/// A permission denial on a host where Full Disk Access applies (macOS) is a Full Disk
/// Access problem for ANY browser family — on current macOS, third-party browser data
/// is TCC-protected exactly like Safari. Every other read failure (a non-permission
/// error, or a permission error on a host without TCC) is a plain "not readable".
///
/// The OS-specific fact (`fda_applies`) is INJECTED so both arms stay measured on every
/// host and are unit-testable without real TCC; production callers pass
/// [`full_disk_access_applies`].
pub fn classify_history_access_core(is_permission_denied: bool, fda_applies: bool) -> &'static str {
    if is_permission_denied && fda_applies {
        DISCOVERY_ISSUE_FULL_DISK_ACCESS
    } else {
        HISTORY_ISSUE_FILE_NOT_READABLE
    }
}
