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
//! sentinel list — is plain and host-independent; the classification is unit-tested, so
//! no real TCC is required. Only the macOS sentinel list and the real `read_dir` endpoint
//! are `#[cfg]`-selected per target.

use std::io::ErrorKind;
use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

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
/// A permission denial is a hard "denied"; a missing sentinel or
/// any other error is deliberately `Inconclusive` (never a false "denied").
fn classify_sentinel_read(result: std::io::Result<()>) -> FullDiskAccessProbe {
    match result {
        Ok(()) => FullDiskAccessProbe::Granted,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => FullDiskAccessProbe::Denied,
        Err(_) => FullDiskAccessProbe::Inconclusive,
    }
}

/// Probes a single sentinel through the given access check, so the verdict does not
/// depend on how the directory is read.
pub fn probe_full_disk_access_at<F>(sentinel: &Path, access_check: F) -> FullDiskAccessProbe
where
    F: FnOnce(&Path) -> std::io::Result<()>,
{
    classify_sentinel_read(access_check(sentinel))
}

/// Aggregates a probe across an ordered sentinel list: the FIRST conclusive verdict
/// (`Granted` or `Denied`) wins; if every sentinel is inconclusive the result is
/// `Inconclusive`.
///
/// The only caller lives in the macOS arm of [`probe_full_disk_access`], so other
/// targets must not compile this (dead_code under `-D warnings` on Linux CI).
#[cfg(target_os = "macos")]
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
/// the classification/aggregation logic is shared by every host.
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
}
