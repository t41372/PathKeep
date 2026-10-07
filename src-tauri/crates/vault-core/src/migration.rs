//! Responsibilities: the `.pathkeep` bundle format shared by export and import: manifest types,
//! the manifest's own checksum, per-file sha256 while packing and unpacking, and safe relative
//! paths inside the zip.
//! Not responsible for: what goes into a bundle or how it is installed (`native`), which
//! portable artifacts travel (`portable_artifacts`), or password cryptography.
//! Failure modes: a tampered manifest or entry accepted, an entry escaping the staging folder
//! (`..`, absolute, drive-letter paths), unbounded memory on a large archive.
//! Tests: migration::tests, migration::native_tests.
//!
//! Packing and unpacking stream through a 64 KiB buffer, so memory stays flat however large
//! the archive is.

use crate::utils::sha256_hex;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub mod native;
mod portable_artifacts;
pub use portable_artifacts::resolve_imported_artifact;
#[cfg(test)]
mod native_tests;

const MANIFEST_ZIP_PATH: &str = "pathkeep-export-manifest.json";
const MANIFEST_SHA_ZIP_PATH: &str = "pathkeep-export-manifest.sha256";
const ZIP_COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifestFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifest {
    /// Bumped whenever the manifest schema or zip layout changes.
    pub format_version: u32,
    /// Semantic app version that produced the bundle (e.g. `0.3.0`).
    pub app_version: String,
    /// Highest archive migration version recorded in the exported SQLite.
    pub archive_schema_version: i64,
    /// `"encrypted"` or `"plaintext"` — informs the target user whether
    /// the imported archive will need an unlock key.
    pub archive_mode: String,
    /// RFC 3339 instant the bundle was produced.
    pub exported_at: String,
    /// Best-effort hostname of the source machine; advisory only.
    pub exporter_hostname: Option<String>,
    /// Every file in the bundle except the manifest itself. The sha256
    /// gives import a tamper check independent of the zip CRC.
    pub files: Vec<ExportManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedBundle {
    pub bundle_path: PathBuf,
    pub manifest: ExportManifest,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub manifest: ExportManifest,
    /// True when the bundle's archive_schema_version equals this binary's
    /// `max_schema_version()`. False means forward-migrations will run on
    /// the imported archive after extraction.
    pub schema_up_to_date: bool,
    /// Forward migration versions the import will apply on top of the
    /// bundle (empty when `schema_up_to_date`).
    pub migrations_to_apply: Vec<i64>,
    /// Bytes the bundle's payload will occupy after extraction. The user
    /// can compare against available disk before confirming.
    pub bytes_to_extract: u64,
    /// Localised-by-frontend strings (Stronghold, scheduler, …) describing
    /// what the user must reconfigure on the target machine.
    pub exclusion_notes: Vec<ImportExclusionNote>,
    /// True when applying the import will overwrite an existing
    /// initialized archive on the target. The frontend surfaces this as
    /// the loud destructive-action confirmation.
    pub will_overwrite_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportExclusionNote {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ApplyImportOptions {
    /// When true (the recommended default), the caller has confirmed the
    /// destructive overwrite and we can swap the new tree in. When false,
    /// the function refuses early so a UI bug can't slip past the
    /// PME confirmation.
    pub confirm_overwrite: bool,
    /// Native format-2 bundles use the export password. Legacy format-1 helpers
    /// use the source SQLCipher password. Kept under the existing serialized name
    /// so the caller can preview first, then supply the appropriate password.
    pub source_archive_key: Option<String>,
}

/// Error-message prefix used by `apply_import` to signal "this bundle
/// is encrypted but no source key was supplied." The frontend matches
/// on this prefix to render the source-key input on the Settings →
/// Data migration panel.
pub const IMPORT_SOURCE_KEY_REQUIRED_PREFIX: &str = "source_archive_key required";

/// Error-message prefix used by `apply_import` to signal "the source
/// key supplied for this encrypted bundle is wrong." Distinct from
/// `IMPORT_SOURCE_KEY_REQUIRED_PREFIX` so the frontend can swap copy
/// between "please enter" and "wrong key, try again."
pub const IMPORT_SOURCE_KEY_INVALID_PREFIX: &str = "source_archive_key invalid";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub manifest: ExportManifest,
    /// Migration versions actually applied on top of the bundle.
    pub migrations_applied: Vec<i64>,
    /// Final archive schema version after migrations.
    pub final_schema_version: i64,
    /// True when the previous project tree was preserved as a sibling
    /// `.bak-<timestamp>` directory next to each restored subtree so the
    /// user can recover if they imported the wrong bundle.
    pub preserved_previous_as_bak: bool,
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    Ok(())
}

fn add_file_to_zip_cancel(
    zip: &mut ZipWriter<File>,
    source_path: &Path,
    zip_path: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<ExportManifestFile>,
    cancel: &vault_store::CancelToken,
) -> Result<()> {
    let normalized = zip_path.replace('\\', "/");
    let source =
        File::open(source_path).with_context(|| format!("reading {}", source_path.display()))?;
    let mut reader = BufReader::new(source);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; ZIP_COPY_BUFFER_BYTES];
    let mut size_bytes = 0_u64;

    zip.start_file(&normalized, options)?;
    loop {
        cancel.check()?;
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("reading {}", source_path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        zip.write_all(&buffer[..read])?;
        size_bytes += read as u64;
    }

    manifest_files.push(ExportManifestFile {
        path: normalized,
        sha256: hex::encode(hasher.finalize()),
        size_bytes,
    });
    Ok(())
}

/// Rejects bundle-manifest entries whose `path` could escape the staging
/// directory or otherwise impersonate a system file when joined to a base
/// path.
///
/// The manifest is attacker-controlled: its sha256 sidecar lives inside
/// the same zip, so a malicious bundle author can recompute the hash for
/// any `files[].path` they like. Without this check
/// `target_dir.join(declared.path)` would happily follow `..` or accept
/// absolute paths, giving `File::create` an arbitrary-write primitive
/// under the running user's identity.
///
/// A valid entry must:
///   - be non-empty,
///   - contain only `Component::Normal` parts (so no `..`, no root, no
///     Windows drive-letter prefix, no leading `/`),
///   - contain no embedded NUL byte (defence in depth against C-string
///     truncation tricks if the path is ever handed to a non-Rust API).
///
/// Returns the normalized `PathBuf` so the caller doesn't accidentally
/// re-parse the raw string later.
fn validate_bundle_relative_path(raw: &str) -> Result<PathBuf> {
    if raw.is_empty() {
        anyhow::bail!("bundle entry has an empty path");
    }
    if raw.as_bytes().contains(&0) {
        anyhow::bail!("bundle entry path {raw} contains a NUL byte");
    }
    // Validate the raw string directly instead of routing through
    // `Path::components()`. The Component enum has a `Prefix` variant
    // that's only ever yielded on Windows (`C:\foo`); a Unix coverage
    // run can't reach it and the unit-test crate can't construct a
    // `PrefixComponent` either, so a match-based approach leaves a
    // dead arm. The string-level walk below has the same rejection
    // surface and every branch is reachable from a Unix test.
    let bytes = raw.as_bytes();
    // Reject absolute paths — both Unix (`/foo`) and Windows
    // (`\foo`) shapes. Done before component splitting so a single
    // leading separator is the absolute case, not a leading "empty
    // segment".
    if bytes[0] == b'/' || bytes[0] == b'\\' {
        anyhow::bail!("bundle entry path {raw} is absolute");
    }
    // Reject Windows drive-letter prefixes (`C:foo`, `c:`, etc.).
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        anyhow::bail!("bundle entry path {raw} has a drive-letter prefix");
    }
    let mut out = PathBuf::new();
    for segment in raw.split(['/', '\\']) {
        match segment {
            "" => {
                // Empty segment from a repeated separator like
                // "a//b". Treat as a no-op so legitimate-looking
                // bundles with a trailing slash still validate; the
                // important rejections (`..`, `.`, absolute) are
                // checked separately above and below.
            }
            "." => {
                anyhow::bail!("bundle entry path {raw} contains a `.` component")
            }
            ".." => {
                anyhow::bail!("bundle entry path {raw} contains a `..` component")
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

fn read_and_validate_manifest(bundle_path: &Path) -> Result<ExportManifest> {
    let file = File::open(bundle_path)
        .with_context(|| format!("opening bundle {}", bundle_path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("reading {} as a zip", bundle_path.display()))?;
    let manifest_bytes =
        read_zip_entry_bytes(&mut archive, MANIFEST_ZIP_PATH).with_context(|| {
            format!("bundle is missing {} (is this a PathKeep export?)", MANIFEST_ZIP_PATH)
        })?;
    let sha_entry = read_zip_entry_bytes(&mut archive, MANIFEST_SHA_ZIP_PATH)
        .with_context(|| format!("bundle is missing {}", MANIFEST_SHA_ZIP_PATH))?;
    let expected = String::from_utf8_lossy(&sha_entry).trim().to_string();
    let actual = sha256_hex(&manifest_bytes);
    if expected != actual {
        anyhow::bail!(
            "bundle manifest sha256 mismatch (expected {expected}, computed {actual}); the bundle may be tampered with or corrupted",
        );
    }
    let manifest: ExportManifest =
        serde_json::from_slice(&manifest_bytes).context("parsing bundle manifest")?;
    Ok(manifest)
}

fn read_zip_entry_bytes(archive: &mut ZipArchive<File>, entry_name: &str) -> Result<Vec<u8>> {
    let mut entry =
        archive.by_name(entry_name).with_context(|| format!("zip entry {entry_name} not found"))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn extract_bundle_into_cancel(
    bundle_path: &Path,
    target_dir: &Path,
    manifest: &ExportManifest,
    cancel: &vault_store::CancelToken,
) -> Result<()> {
    let file = File::open(bundle_path)
        .with_context(|| format!("re-opening bundle {}", bundle_path.display()))?;
    let mut archive = ZipArchive::new(file)?;

    // Verify every manifest file is present + bytes match before we
    // touch the live project tree. `preview_import` already rejected any
    // traversal / absolute / drive-letter entries; the second pass here
    // is defence-in-depth so `apply_import` callers that skipped preview
    // (synthetic CLI users, future automation) can't slip past the gate.
    for declared in &manifest.files {
        let safe_relative = validate_bundle_relative_path(&declared.path)
            .with_context(|| format!("bundle manifest entry {} is not safe", declared.path))?;
        let mut entry = archive
            .by_name(&declared.path)
            .with_context(|| format!("bundle is missing declared entry {}", declared.path))?;
        let dest = target_dir.join(&safe_relative);
        ensure_parent_dir(&dest)?;
        let mut out =
            File::create(&dest).with_context(|| format!("staging file {}", dest.display()))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; ZIP_COPY_BUFFER_BYTES];
        loop {
            cancel.check()?;
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            out.write_all(&buffer[..read])?;
        }
        let actual = hex::encode(hasher.finalize());
        if actual != declared.sha256 {
            anyhow::bail!(
                "bundle entry {} sha256 mismatch (manifest {} vs extracted {})",
                declared.path,
                declared.sha256,
                actual,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locks the two typed apply-import error prefixes to exact literals.
    ///
    /// The shell hand-copies these same literals in
    /// `src/lib/backend-client/migration.ts` because the source-key prompt is
    /// selected by prefix match. `src/lib/backend-client/migration.test.ts`
    /// pins the TypeScript side against the identical strings, so editing
    /// either language alone fails a test instead of silently turning the
    /// source-key prompt into a generic error banner.
    #[test]
    fn import_source_key_error_prefixes_stay_the_literals_the_shell_pins() {
        assert_eq!(IMPORT_SOURCE_KEY_REQUIRED_PREFIX, "source_archive_key required");
        assert_eq!(IMPORT_SOURCE_KEY_INVALID_PREFIX, "source_archive_key invalid");
    }

    #[test]
    fn validate_bundle_relative_path_accepts_a_normal_relative_path() {
        let normalized = validate_bundle_relative_path("archive/history-vault.sqlite").unwrap();
        let expected: PathBuf = ["archive", "history-vault.sqlite"].iter().collect();
        assert_eq!(normalized, expected);
    }

    #[test]
    fn validate_bundle_relative_path_rejects_parent_dir_traversal() {
        let err =
            validate_bundle_relative_path("../etc/passwd").expect_err("traversal must be rejected");
        assert!(format!("{err:?}").contains("..` component"), "expected `..` error, got {err:?}",);
    }

    #[test]
    fn validate_bundle_relative_path_rejects_nested_parent_dir_traversal() {
        let err = validate_bundle_relative_path("archive/../../escape.txt")
            .expect_err("nested traversal must be rejected");
        assert!(format!("{err:?}").contains("..` component"), "expected `..` error, got {err:?}",);
    }

    #[test]
    fn validate_bundle_relative_path_rejects_current_dir_component() {
        // `Path::new("./escape.txt").components()` yields
        // [CurDir, Normal("escape.txt")]. The bare `.` is also a CurDir.
        // Both must be rejected — a `.` component is an attacker-friendly
        // shape for normalising-vs-not collisions in downstream tools.
        let err = validate_bundle_relative_path("./escape.txt")
            .expect_err("`.` component must be rejected");
        assert!(
            format!("{err:?}").contains("`.` component"),
            "expected `.` rejection, got {err:?}",
        );
    }

    #[test]
    fn validate_bundle_relative_path_rejects_unix_absolute() {
        let err = validate_bundle_relative_path("/etc/passwd")
            .expect_err("absolute path must be rejected");
        assert!(
            format!("{err:?}").contains("is absolute"),
            "expected absolute-path error, got {err:?}",
        );
    }

    #[test]
    fn validate_bundle_relative_path_rejects_windows_style_absolute() {
        // `\foo` is the Windows-style absolute marker. The string-level
        // check rejects this too even on Unix CI, so an attacker can't
        // build a bundle that ships safely under `Path::components()`
        // on Unix and only blows up on a Windows target.
        let err = validate_bundle_relative_path("\\Windows\\System32")
            .expect_err("backslash-absolute path must be rejected");
        assert!(
            format!("{err:?}").contains("is absolute"),
            "expected absolute-path error, got {err:?}",
        );
    }

    #[test]
    fn validate_bundle_relative_path_rejects_windows_drive_letter_prefix() {
        // `C:foo` is the explicit Windows drive-letter shape. The
        // up-front string check rejects it on every platform so the
        // path is never handed to a downstream tool that might
        // interpret `C:` as an alternate root.
        let err = validate_bundle_relative_path("C:Windows\\evil")
            .expect_err("drive-letter prefix must be rejected");
        assert!(
            format!("{err:?}").contains("drive-letter prefix"),
            "expected drive-letter rejection, got {err:?}",
        );
    }

    #[test]
    fn validate_bundle_relative_path_collapses_repeated_separators() {
        // `a//b` produces an empty segment between `a` and `b`. The
        // empty arm intentionally folds the empty segment away so a
        // bundle that uses a doubled separator validates; the
        // important rejections (`..`, `.`, absolute, drive-letter)
        // are checked separately. The resulting PathBuf is the
        // dedup'd form.
        let normalized = validate_bundle_relative_path("archive//history-vault.sqlite").unwrap();
        let expected: PathBuf = ["archive", "history-vault.sqlite"].iter().collect();
        assert_eq!(normalized, expected);
    }

    #[test]
    fn validate_bundle_relative_path_rejects_empty_input() {
        let err = validate_bundle_relative_path("").expect_err("empty path must be rejected");
        assert!(format!("{err:?}").contains("empty path"), "got {err:?}");
    }

    #[test]
    fn validate_bundle_relative_path_rejects_nul_byte() {
        let err = validate_bundle_relative_path("ok/\u{0}bad").expect_err("NUL must be rejected");
        assert!(format!("{err:?}").contains("NUL byte"), "got {err:?}");
    }
}
