//! macOS Full Disk Access probe.
//!
//! ## Responsibilities
//! - Answer, cheaply and without touching any secret material, whether the host is
//!   currently BLOCKING reads of TCC-protected user data (macOS Full Disk Access
//!   denied).
//! - Give the onboarding snapshot a signal that is INDEPENDENT of whether browser
//!   discovery happened to error. On some macOS setups a TCC-protected directory makes
//!   discovery silently skip a browser (a bare `path.exists()` returns `false`) instead
//!   of surfacing an error, so "discovery found nothing" alone cannot tell a fresh
//!   install apart from a permission wall. This probe closes that gap.
//!
//! ## Not responsible for
//! - Reading any file CONTENTS. It only checks whether a sentinel DIRECTORY can be
//!   listed; it never opens browser history, secrets, or archive material.
//! - Emitting user-facing copy. The caller maps the verdict onto a stable marker the
//!   front end localizes.
//!
//! ## Design
//! The verdict logic — classify one sentinel read, then aggregate across an ordered
//! sentinel list — is plain, host-independent, and unit-tested via an injected access
//! closure, so no real TCC is required. Only the macOS sentinel list and the real
//! `read_dir` endpoint are `#[cfg]`-selected per target; the decision logic stays in the
//! measured (coverage) build on every host.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Outcome of probing macOS Full Disk Access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullDiskAccessProbe {
    /// A TCC-protected sentinel was listed successfully — access is not being blocked.
    Granted,
    /// Listing a sentinel was refused with a permission denial — Full Disk Access is off.
    Denied,
    /// No sentinel could be judged (missing, or a non-permission error). NEVER treated as
    /// denied: the caller must not nag the user on an inconclusive probe.
    Inconclusive,
    /// The host has no Full Disk Access concept (non-macOS). The caller must never emit
    /// the Full Disk Access marker for this result.
    NotApplicable,
}

/// Classifies ONE sentinel directory-read result into a verdict.
///
/// Measured on every host. A permission denial is a hard "denied"; a missing sentinel or
/// any other error is deliberately `Inconclusive` (never a false "denied").
fn classify_sentinel_read(result: std::io::Result<()>) -> FullDiskAccessProbe {
    match result {
        Ok(()) => FullDiskAccessProbe::Granted,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => FullDiskAccessProbe::Denied,
        Err(_) => FullDiskAccessProbe::Inconclusive,
    }
}

/// Probes a single sentinel through an injected access check.
///
/// This is the test seam: point `access_check` at a readable temp dir (⇒ `Granted`), a
/// permission-denied path (⇒ `Denied`), or a missing path (⇒ `Inconclusive`) to exercise
/// every arm without real TCC.
pub fn probe_full_disk_access_at<F>(sentinel: &Path, access_check: F) -> FullDiskAccessProbe
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    classify_sentinel_read(access_check(sentinel))
}

/// Aggregates a probe across an ordered sentinel list: the FIRST conclusive verdict
/// (`Granted` or `Denied`) wins; if every sentinel is inconclusive the result is
/// `Inconclusive`. Measured on every host via the seam tests.
fn probe_sentinels<F>(sentinels: &[PathBuf], mut access_check: F) -> FullDiskAccessProbe
where
    F: FnMut(&Path) -> std::io::Result<()>,
{
    for sentinel in sentinels {
        match probe_full_disk_access_at(sentinel, &mut access_check) {
            FullDiskAccessProbe::Inconclusive => continue,
            conclusive => return conclusive,
        }
    }
    FullDiskAccessProbe::Inconclusive
}

/// Probes whether macOS Full Disk Access is currently denied, by attempting to LIST a
/// TCC-protected sentinel directory. Reads zero file contents. Non-macOS hosts return
/// [`FullDiskAccessProbe::NotApplicable`].
///
/// Only the sentinel selection and the real filesystem `read_dir` endpoint are per-OS;
/// the classification/aggregation logic is shared and unit-tested on every host.
pub fn probe_full_disk_access() -> FullDiskAccessProbe {
    #[cfg(target_os = "macos")]
    {
        probe_sentinels(&macos_full_disk_access_sentinels(), |path| {
            std::fs::read_dir(path).map(|_| ())
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        FullDiskAccessProbe::NotApplicable
    }
}

/// Ordered macOS Full Disk Access sentinels. `~/Library/Safari` is the canonical
/// TCC-protected directory present on essentially every install; `~/Library/Mail` is a
/// secondary fallback for the rare install without a Safari directory.
#[cfg(target_os = "macos")]
fn macos_full_disk_access_sentinels() -> Vec<PathBuf> {
    directories::UserDirs::new()
        .map(|dirs| {
            let home = dirs.home_dir();
            vec![home.join("Library/Safari"), home.join("Library/Mail")]
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    #[test]
    fn classify_sentinel_read_maps_each_outcome() {
        assert_eq!(classify_sentinel_read(Ok(())), FullDiskAccessProbe::Granted);
        assert_eq!(
            classify_sentinel_read(Err(Error::from(ErrorKind::PermissionDenied))),
            FullDiskAccessProbe::Denied
        );
        // A missing sentinel (or any non-permission error) is inconclusive, never denied.
        assert_eq!(
            classify_sentinel_read(Err(Error::from(ErrorKind::NotFound))),
            FullDiskAccessProbe::Inconclusive
        );
    }

    #[test]
    fn probe_full_disk_access_at_reads_through_the_injected_seam() {
        let readable = probe_full_disk_access_at(Path::new("/sentinel"), |_| Ok(()));
        assert_eq!(readable, FullDiskAccessProbe::Granted);

        let denied = probe_full_disk_access_at(Path::new("/sentinel"), |_| {
            Err(Error::from(ErrorKind::PermissionDenied))
        });
        assert_eq!(denied, FullDiskAccessProbe::Denied);

        let missing = probe_full_disk_access_at(Path::new("/sentinel"), |_| {
            Err(Error::from(ErrorKind::NotFound))
        });
        assert_eq!(missing, FullDiskAccessProbe::Inconclusive);
    }

    #[test]
    fn probe_sentinels_returns_the_first_conclusive_verdict() {
        let sentinels = vec![PathBuf::from("/missing"), PathBuf::from("/protected")];

        // The first sentinel is inconclusive, so the second (a denial) decides.
        let denied = probe_sentinels(&sentinels, |path| {
            if path == Path::new("/protected") {
                Err(Error::from(ErrorKind::PermissionDenied))
            } else {
                Err(Error::from(ErrorKind::NotFound))
            }
        });
        assert_eq!(denied, FullDiskAccessProbe::Denied);

        // A readable first sentinel short-circuits to granted.
        let granted = probe_sentinels(&sentinels, |_| Ok(()));
        assert_eq!(granted, FullDiskAccessProbe::Granted);

        // Every sentinel inconclusive ⇒ inconclusive (no false denial).
        let inconclusive = probe_sentinels(&sentinels, |_| Err(Error::from(ErrorKind::NotFound)));
        assert_eq!(inconclusive, FullDiskAccessProbe::Inconclusive);

        // An empty sentinel list is inconclusive rather than a crash or false denial.
        let empty = probe_sentinels(&[], |_| Ok(()));
        assert_eq!(empty, FullDiskAccessProbe::Inconclusive);
    }

    #[test]
    fn probe_full_disk_access_reports_a_verdict_for_the_current_host() {
        let verdict = probe_full_disk_access();
        if cfg!(target_os = "macos") {
            // macOS must reach a real read-based verdict (any of granted/denied/inconclusive),
            // never NotApplicable.
            assert_ne!(
                verdict,
                FullDiskAccessProbe::NotApplicable,
                "macOS must produce a real Full Disk Access verdict"
            );
        } else {
            assert_eq!(
                verdict,
                FullDiskAccessProbe::NotApplicable,
                "non-macOS hosts have no Full Disk Access concept"
            );
        }
    }
}
