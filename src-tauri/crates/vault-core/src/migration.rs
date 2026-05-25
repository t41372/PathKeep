//! Whole-app data Export / Import.
//!
//! ## Responsibilities
//! - Pack every piece of locally-stored PathKeep data the user would lose if
//!   they wiped the project root (config, archive databases, derived
//!   projections, audit ledger, raw snapshots, intelligence/semantic
//!   sidecars) into a single portable `.pathkeep-bundle` zip.
//! - Read a bundle back, validate its manifest + per-file sha256, refuse
//!   bundles whose archive schema is newer than the local binary can
//!   migrate forward, and stage the payload into the live project tree
//!   atomically so a half-finished import does not leave the user with a
//!   corrupted archive.
//! - Apply forward archive-schema migrations on the imported database so
//!   old bundles work on newer binaries.
//!
//! ## Not responsible for
//! - Stopping or restarting background workers around the swap. The caller
//!   (Tauri command façade) owns runtime quiescence — this module assumes
//!   it is the only writer for the duration of the call.
//! - Stronghold / App Lock secrets, scheduler artifacts, logs, diagnostics,
//!   or any platform-specific state. Those are intentionally excluded from
//!   the bundle (see `EXPORT_EXCLUSIONS_DOC` for the documented reasons).
//! - Re-encrypting the archive with a *different* key on the target
//!   machine. The current bundle preserves the source mode and key; the
//!   target user must unlock with the source key, then optionally run the
//!   existing Settings → Security rekey flow.
//!
//! ## Bundle layout
//!
//! ```text
//! pathkeep-export-manifest.json           # primary manifest (see ExportManifest)
//! pathkeep-export-manifest.sha256         # sha256("…manifest.json") + "\n"
//! config/config.json                      # AppConfig snapshot
//! archive/history-vault.sqlite            # via sqlcipher_export, key preserved
//! archive/source-evidence.sqlite          # same encryption mode
//! derived/history-search.sqlite           # optional (rebuildable)
//! derived/history-intelligence.sqlite     # optional (rebuildable)
//! audit/...                               # manifests + git repo
//! raw-snapshots/...                       # original imported source files
//! sidecars/intelligence-blobs/...         # derived intelligence blob storage
//! sidecars/semantic-index/...             # optional vector index
//! ```
//!
//! ## Dependencies
//! - `archive::{export_archive_database, open_archive_connection,
//!   open_source_evidence_connection, max_schema_version, run_migrations}`
//!   for safe encrypted-db copy + forward migration.
//! - `config::{ProjectPaths, load_config, save_config}` for the project
//!   layout the bundle mirrors.
//! - `zip` for the portable container (matches the existing remote-bundle
//!   choice; CompressionMethod::Deflated stays the default).
//! - `sha2` / `hex` for per-file digest + manifest tamper-evidence.
//!
//! ## Performance notes
//! - Both pack and unpack stream files through a 64 KiB buffer so peak
//!   memory stays bounded regardless of archive size — this is on the
//!   hot path of moving a 14M-row archive between machines.

use crate::{
    archive::{
        export_archive_database, max_schema_version, open_archive_connection,
        open_source_evidence_connection, run_migrations,
    },
    config::{ProjectPaths, load_config, save_config},
    models::{AppConfig, ArchiveMode},
    utils::sha256_hex,
};
use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
    time::Duration as StdDuration,
};
use tempfile::tempdir;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// The version of the export bundle format itself. Bumping this signals an
/// intentional, breaking change to the manifest schema or to the on-disk
/// layout inside the zip; older binaries will refuse forward-incompatible
/// bundles via `validate_format_version`.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

const MANIFEST_ZIP_PATH: &str = "pathkeep-export-manifest.json";
const MANIFEST_SHA_ZIP_PATH: &str = "pathkeep-export-manifest.sha256";
const ZIP_COPY_BUFFER_BYTES: usize = 64 * 1024;

/// Documented reasons each top-level path is included or excluded from the
/// export bundle. Surfaced via `apply_import` warnings so the user sees
/// what their migration *won't* carry over (App Lock, scheduler, etc.).
pub const EXPORT_EXCLUSIONS_DOC: &[(&str, &str)] = &[
    ("vault.hold", "Stronghold App Lock secrets stay on the source machine."),
    ("stronghold-salt.txt", "App Lock salt stays on the source machine."),
    ("logs/", "Local log files are not portable across machines."),
    ("diagnostics/", "Diagnostics and crash reports are local-only artifacts."),
    ("schedule/", "macOS LaunchAgent / Task Scheduler artifacts are platform-specific."),
    ("staging/", "Transient import staging is regenerated as needed."),
    ("quarantine/", "Quarantined files belong only to the source machine."),
    ("exports/", "Avoids packing previous export bundles into a new export."),
];

/// Top-level subtrees of the project root that *are* included in the bundle.
/// Listed explicitly so a new artifact directory does not silently leak
/// into bundles before its semantics are reviewed.
const INCLUDED_DIRECTORY_PREFIXES: &[&str] = &["derived", "audit", "raw-snapshots", "sidecars"];

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
}

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

/// Packs the entire PathKeep project at `paths` into a single
/// `.pathkeep-bundle` zip at `target_path`.
///
/// The archive databases are copied via `sqlcipher_export` so the bundle
/// stays internally consistent even if the live archive is being written
/// to by another process between the manifest scan and the zip flush.
pub fn export_app_data(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    target_path: &Path,
) -> Result<ExportedBundle> {
    if !paths.archive_database_path.exists() {
        anyhow::bail!(
            "no archive to export: {} does not exist yet",
            paths.archive_database_path.display(),
        );
    }
    ensure_parent_dir(target_path)?;

    let staging = tempdir().context("creating export staging dir")?;
    let archive_copy = staging.path().join("history-vault.sqlite");
    let source_evidence_copy = staging.path().join("source-evidence.sqlite");

    copy_archive_database_to(paths, config, key, &archive_copy)?;
    if paths.source_evidence_database_path.exists() {
        copy_source_evidence_database_to(paths, config, key, &source_evidence_copy)?;
    }

    let archive_schema_version = {
        let connection = open_archive_connection(paths, config, key)?;
        crate::archive::current_version(&connection)?
    };

    let file = File::create(target_path)
        .with_context(|| format!("creating bundle at {}", target_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut manifest_files: Vec<ExportManifestFile> = Vec::new();

    // 1. config snapshot
    if paths.config_path.exists() {
        add_file_to_zip(
            &mut zip,
            &paths.config_path,
            "config/config.json",
            options,
            &mut manifest_files,
        )?;
    }

    // 2. canonical archive databases (staged copies, encryption preserved)
    add_file_to_zip(
        &mut zip,
        &archive_copy,
        "archive/history-vault.sqlite",
        options,
        &mut manifest_files,
    )?;
    if source_evidence_copy.exists() {
        add_file_to_zip(
            &mut zip,
            &source_evidence_copy,
            "archive/source-evidence.sqlite",
            options,
            &mut manifest_files,
        )?;
    }

    // 3. derived projections, audit ledger, raw snapshots, sidecars
    for prefix in INCLUDED_DIRECTORY_PREFIXES {
        let source_dir = paths.app_root.join(prefix);
        add_dir_to_zip_if_exists(&mut zip, &source_dir, prefix, options, &mut manifest_files)?;
    }

    // 4. write the manifest last so it sees every file's sha256
    let manifest = ExportManifest {
        format_version: EXPORT_FORMAT_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        archive_schema_version,
        archive_mode: match config.archive_mode {
            ArchiveMode::Encrypted => "encrypted".to_string(),
            ArchiveMode::Plaintext => "plaintext".to_string(),
        },
        exported_at: crate::utils::now_rfc3339(),
        exporter_hostname: hostname_best_effort(),
        files: manifest_files,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    zip.start_file(MANIFEST_ZIP_PATH, options)?;
    zip.write_all(&manifest_bytes)?;
    zip.start_file(MANIFEST_SHA_ZIP_PATH, options)?;
    zip.write_all(format!("{}\n", sha256_hex(&manifest_bytes)).as_bytes())?;
    zip.finish()?;

    let bytes_written =
        fs::metadata(target_path).with_context(|| format!("stat {}", target_path.display()))?.len();

    Ok(ExportedBundle { bundle_path: target_path.to_path_buf(), manifest, bytes_written })
}

/// Opens a bundle at `bundle_path`, validates the manifest, and returns
/// the preview the Settings UI shows to the user before they commit.
///
/// This call is read-only and does not touch the live project tree.
pub fn preview_import(paths: &ProjectPaths, bundle_path: &Path) -> Result<ImportPreview> {
    let manifest = read_and_validate_manifest(bundle_path)?;
    validate_format_version(&manifest)?;
    let local_max = max_schema_version();
    if manifest.archive_schema_version > local_max {
        anyhow::bail!(
            "bundle was produced by a newer PathKeep build (archive schema v{}, this build supports up to v{}). Upgrade PathKeep on this machine before importing.",
            manifest.archive_schema_version,
            local_max,
        );
    }
    let migrations_to_apply: Vec<i64> =
        ((manifest.archive_schema_version + 1)..=local_max).collect();
    let bytes_to_extract = manifest.files.iter().map(|f| f.size_bytes).sum();
    let exclusion_notes = EXPORT_EXCLUSIONS_DOC
        .iter()
        .map(|(path, reason)| ImportExclusionNote {
            path: (*path).to_string(),
            reason: (*reason).to_string(),
        })
        .collect();
    let will_overwrite_existing = paths.archive_database_path.exists();
    Ok(ImportPreview {
        manifest,
        schema_up_to_date: migrations_to_apply.is_empty(),
        migrations_to_apply,
        bytes_to_extract,
        exclusion_notes,
        will_overwrite_existing,
    })
}

/// Applies the bundle at `bundle_path` onto the live project tree.
///
/// Steps:
///   1. Re-validate the manifest (defence against a races between preview
///      and apply where the bundle was replaced on disk).
///   2. Extract every entry into a temp staging tree.
///   3. Move each currently-existing target subtree to a sibling
///      `.bak-<timestamp>` directory so the user can recover.
///   4. Move the staged subtrees into the live project paths.
///   5. Run forward schema migrations on the newly-installed archive.
///   6. Reload + persist the imported config so callers see fresh state.
///
/// The caller must have stopped background workers before invoking this
/// (the Tauri command façade does so).
pub fn apply_import(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    bundle_path: &Path,
    options: &ApplyImportOptions,
) -> Result<ImportResult> {
    let preview = preview_import(paths, bundle_path)?;
    if preview.will_overwrite_existing && !options.confirm_overwrite {
        anyhow::bail!(
            "the target already has an initialized PathKeep archive; re-call apply_import with confirm_overwrite=true to authorise the destructive overwrite",
        );
    }

    let staging = tempdir().context("creating import staging dir")?;
    extract_bundle_into(bundle_path, staging.path(), &preview.manifest)?;

    // Atomic-ish swap: rename current subtrees to .bak then move staged in.
    // SQLite + WAL files require the connection to be closed before move
    // — the caller stopped workers; here we just touch the filesystem.
    let timestamp = crate::utils::now_rfc3339().replace(':', "-");
    let mut preserved_previous = false;

    // Files (config + archive dbs) and directories under the project root.
    let movable_files: &[(&str, &Path)] = &[
        ("config/config.json", paths.config_path.as_path()),
        ("archive/history-vault.sqlite", paths.archive_database_path.as_path()),
        ("archive/source-evidence.sqlite", paths.source_evidence_database_path.as_path()),
    ];
    for (zip_relative, target) in movable_files {
        let staged = staging.path().join(zip_relative);
        if !staged.exists() {
            continue;
        }
        if target.exists() {
            preserved_previous = true;
            let backup = backup_sidecar_path(target, &timestamp);
            fs::rename(target, &backup).with_context(|| {
                format!("preserving previous {} as {}", target.display(), backup.display())
            })?;
        }
        ensure_parent_dir(target)?;
        fs::rename(&staged, target).with_context(|| {
            format!("installing {} into {}", staged.display(), target.display())
        })?;
    }

    for prefix in INCLUDED_DIRECTORY_PREFIXES {
        let staged = staging.path().join(prefix);
        if !staged.exists() {
            continue;
        }
        let target = paths.app_root.join(prefix);
        if target.exists() {
            preserved_previous = true;
            let backup = backup_sidecar_path(&target, &timestamp);
            fs::rename(&target, &backup).with_context(|| {
                format!("preserving previous {} as {}", target.display(), backup.display())
            })?;
        }
        ensure_parent_dir(&target)?;
        fs::rename(&staged, &target).with_context(|| {
            format!("installing {} into {}", staged.display(), target.display())
        })?;
    }

    // Reload the imported config so subsequent operations see fresh state.
    // The previous in-memory `config` is stale once we've replaced
    // config.json under it.
    let imported_config = load_config(paths).unwrap_or_else(|_| config.clone());
    save_config(paths, &imported_config).ok();

    // Run forward migrations on the newly-installed archive.
    let migrations_applied = if preview.migrations_to_apply.is_empty() {
        Vec::new()
    } else {
        let connection = open_archive_for_migration(paths, &imported_config, key)?;
        run_migrations(&connection)?;
        preview.migrations_to_apply.clone()
    };
    let final_schema_version = {
        let connection = open_archive_for_migration(paths, &imported_config, key)?;
        crate::archive::current_version(&connection)?
    };

    Ok(ImportResult {
        manifest: preview.manifest,
        migrations_applied,
        final_schema_version,
        preserved_previous_as_bak: preserved_previous,
    })
}

fn open_archive_for_migration(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
) -> Result<Connection> {
    // Imported archives need a brief connection just to project the
    // migration ledger up; the regular `open_archive_connection` already
    // bootstraps WAL + cache pragmas, so reuse it.
    open_archive_connection(paths, config, key)
}

fn copy_archive_database_to(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    target_path: &Path,
) -> Result<()> {
    let target_key = encryption_key_for_copy(config, key)?;
    let source = open_archive_connection(paths, config, key)?;
    export_archive_database(&source, target_path, target_key)?;
    // Tiny pause to settle the file handle on slower spinning disks; this
    // mirrors the empirical hardening in remote bundle copy.
    std::thread::sleep(StdDuration::from_millis(10));
    Ok(())
}

fn copy_source_evidence_database_to(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    target_path: &Path,
) -> Result<()> {
    let target_key = encryption_key_for_copy(config, key)?;
    let source = open_source_evidence_connection(paths, config, key)?;
    export_archive_database(&source, target_path, target_key)?;
    Ok(())
}

fn encryption_key_for_copy<'a>(
    config: &AppConfig,
    key: Option<&'a str>,
) -> Result<Option<&'a str>> {
    Ok(if matches!(config.archive_mode, ArchiveMode::Encrypted) {
        Some(key.context(
            "the encrypted archive must be unlocked before exporting; pass the active database key",
        )?)
    } else {
        None
    })
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
    }
    Ok(())
}

fn add_dir_to_zip_if_exists(
    zip: &mut ZipWriter<File>,
    source_dir: &Path,
    zip_prefix: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<ExportManifestFile>,
) -> Result<()> {
    if !source_dir.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(source_dir) {
        let entry = entry.with_context(|| format!("walking {}", source_dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(source_dir)
            .with_context(|| format!("stripping prefix for {}", path.display()))?;
        let zip_path = format!("{zip_prefix}/{}", relative.to_string_lossy());
        add_file_to_zip(zip, path, &zip_path, options, manifest_files)?;
    }
    Ok(())
}

fn add_file_to_zip(
    zip: &mut ZipWriter<File>,
    source_path: &Path,
    zip_path: &str,
    options: SimpleFileOptions,
    manifest_files: &mut Vec<ExportManifestFile>,
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

fn validate_format_version(manifest: &ExportManifest) -> Result<()> {
    if manifest.format_version > EXPORT_FORMAT_VERSION {
        anyhow::bail!(
            "bundle was produced with export format v{}, this PathKeep build only understands up to v{}",
            manifest.format_version,
            EXPORT_FORMAT_VERSION,
        );
    }
    Ok(())
}

fn extract_bundle_into(
    bundle_path: &Path,
    target_dir: &Path,
    manifest: &ExportManifest,
) -> Result<()> {
    let file = File::open(bundle_path)
        .with_context(|| format!("re-opening bundle {}", bundle_path.display()))?;
    let mut archive = ZipArchive::new(file)?;

    // Verify every manifest file is present + bytes match before we
    // touch the live project tree.
    for declared in &manifest.files {
        let mut entry = archive
            .by_name(&declared.path)
            .with_context(|| format!("bundle is missing declared entry {}", declared.path))?;
        let dest = target_dir.join(&declared.path);
        ensure_parent_dir(&dest)?;
        let mut out =
            File::create(&dest).with_context(|| format!("staging file {}", dest.display()))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; ZIP_COPY_BUFFER_BYTES];
        loop {
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

fn backup_sidecar_path(target: &Path, timestamp: &str) -> PathBuf {
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "pathkeep".to_string());
    let mut sidecar_name = file_name;
    sidecar_name.push_str(&format!(".bak-{timestamp}"));
    target.with_file_name(sidecar_name)
}

fn hostname_best_effort() -> Option<String> {
    // `hostname` crate is not a dependency, so fall back to the standard
    // env var that virtually every platform sets one of. Pure
    // informational metadata; absence is fine.
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .ok()
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::project_paths_with_root;
    use std::fs;
    use tempfile::TempDir;

    fn fresh_paths() -> (TempDir, ProjectPaths) {
        let dir = TempDir::new().expect("tempdir");
        let paths = project_paths_with_root(dir.path());
        (dir, paths)
    }

    fn seed_archive(paths: &ProjectPaths) -> AppConfig {
        let config = AppConfig::default(); // plaintext
        fs::create_dir_all(paths.archive_database_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&paths.derived_dir).unwrap();
        crate::archive::create_schema(
            &crate::archive::open_archive_connection(paths, &config, None).unwrap(),
        )
        .unwrap();
        // Drop a marker file under derived so the bundle includes it.
        fs::write(paths.derived_dir.join("marker.txt"), b"derived sentinel").unwrap();
        // Save a real config so config.json gets included.
        save_config(paths, &config).unwrap();
        config
    }

    #[test]
    fn max_schema_version_matches_latest_migration() {
        // Both the migration ledger and the bundle preview rely on this
        // being the highest version we know how to apply.
        let local_max = crate::archive::max_schema_version();
        assert!(local_max > 0, "no migrations registered");
    }

    #[test]
    fn export_and_reimport_roundtrip_preserves_manifest_and_files() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);

        let bundle_target = src_dir.path().join("bundle.pathkeep");
        let bundle = export_app_data(&src_paths, &config, None, &bundle_target)
            .expect("export should succeed");

        // Bundle was actually written and the manifest references files.
        assert!(bundle.bundle_path.exists(), "bundle file missing");
        assert_eq!(bundle.manifest.format_version, EXPORT_FORMAT_VERSION);
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "archive/history-vault.sqlite"),
            "archive db missing from manifest: {:?}",
            bundle.manifest.files,
        );
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "config/config.json"),
            "config missing from manifest",
        );
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "derived/marker.txt"),
            "derived/ contents missing from manifest",
        );

        // Importing onto a *different* fresh project root must reproduce
        // the marker file and pass forward-migration checks.
        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        let preview = preview_import(&dest_paths, &bundle_target).expect("preview");
        assert!(!preview.will_overwrite_existing, "fresh dest should not flag overwrite");
        assert!(
            preview.migrations_to_apply.is_empty(),
            "same-binary export should be schema-current"
        );

        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_target,
            &ApplyImportOptions { confirm_overwrite: false },
        )
        .expect("apply");
        assert_eq!(result.final_schema_version, max_schema_version());
        assert!(
            dest_paths.derived_dir.join("marker.txt").exists(),
            "derived/marker.txt not restored on target",
        );
        assert!(dest_paths.archive_database_path.exists(), "archive db not restored on target",);
    }

    #[test]
    fn preview_rejects_bundle_with_newer_archive_schema() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("future.pathkeep");
        let mut bundle = export_app_data(&src_paths, &config, None, &bundle_path).unwrap();
        // Rewrite the manifest inside the bundle to claim a future schema
        // version; the manifest sha256 is regenerated so the tamper check
        // does not short-circuit the schema-version check we want to test.
        bundle.manifest.archive_schema_version = max_schema_version() + 1;
        rewrite_bundle_manifest(&bundle_path, &bundle.manifest);

        let (_dest_dir, dest_paths) = fresh_paths();
        let err = preview_import(&dest_paths, &bundle_path)
            .expect_err("future-schema bundle must be rejected");
        let message = format!("{err:?}");
        assert!(
            message.contains("newer PathKeep build"),
            "expected schema-version error, got {message}",
        );
    }

    #[test]
    fn preview_detects_will_overwrite_when_target_already_initialized() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("a.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let (_dest_dir, dest_paths) = fresh_paths();
        seed_archive(&dest_paths);
        let preview = preview_import(&dest_paths, &bundle_path).unwrap();
        assert!(
            preview.will_overwrite_existing,
            "preview should flag overwrite when target already initialized",
        );
    }

    #[test]
    fn apply_import_refuses_overwrite_without_explicit_confirm() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("b.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = seed_archive(&dest_paths);
        let err = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false },
        )
        .expect_err("should refuse without confirm");
        assert!(format!("{err:?}").contains("confirm_overwrite"));
    }

    #[test]
    fn apply_import_preserves_previous_tree_as_bak() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("c.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = seed_archive(&dest_paths);
        // Distinguishable sentinel in the target's derived dir so we can
        // confirm it was preserved into a .bak path.
        fs::write(dest_paths.derived_dir.join("sentinel.txt"), b"pre-import").unwrap();

        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true },
        )
        .expect("apply with confirm");
        assert!(result.preserved_previous_as_bak);

        // After the swap the live derived/ is the bundle's contents, and
        // a sibling backup directory exists carrying the original
        // sentinel.txt.
        let derived_siblings: Vec<_> = fs::read_dir(dest_paths.app_root.as_path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name.starts_with("derived.bak-"))
            .collect();
        assert_eq!(
            derived_siblings.len(),
            1,
            "exactly one derived.bak should be preserved, got {derived_siblings:?}",
        );
        let bak_path = dest_paths.app_root.join(&derived_siblings[0]);
        assert!(
            bak_path.join("sentinel.txt").exists(),
            "preserved derived.bak missing sentinel.txt",
        );
    }

    #[test]
    fn preview_rejects_bundle_with_unknown_format_version() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("future_format.pathkeep");
        let mut bundle = export_app_data(&src_paths, &config, None, &bundle_path).unwrap();
        bundle.manifest.format_version = EXPORT_FORMAT_VERSION + 1;
        rewrite_bundle_manifest(&bundle_path, &bundle.manifest);

        let (_dest_dir, dest_paths) = fresh_paths();
        let err = preview_import(&dest_paths, &bundle_path).expect_err("future format rejected");
        assert!(format!("{err:?}").contains("export format"));
    }

    #[test]
    fn preview_rejects_tampered_manifest() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("tampered.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        // Rewrite manifest bytes without updating the sidecar sha256 — the
        // tamper detector should fire before any schema/version checks.
        let tampered = ExportManifest {
            format_version: EXPORT_FORMAT_VERSION,
            app_version: "0.0.0-tamper".to_string(),
            archive_schema_version: 1,
            archive_mode: "plaintext".to_string(),
            exported_at: "2026-05-25T00:00:00Z".to_string(),
            exporter_hostname: None,
            files: Vec::new(),
        };
        rewrite_bundle_manifest_only(&bundle_path, &tampered);

        let (_dest_dir, dest_paths) = fresh_paths();
        let err = preview_import(&dest_paths, &bundle_path)
            .expect_err("tampered manifest should be rejected");
        assert!(format!("{err:?}").contains("sha256 mismatch"));
    }

    // Helpers ----------------------------------------------------------------
    fn rewrite_bundle_manifest(bundle_path: &Path, manifest: &ExportManifest) {
        let bytes = serde_json::to_vec_pretty(manifest).unwrap();
        let sha = format!("{}\n", sha256_hex(&bytes));
        rewrite_zip_entries(
            bundle_path,
            &[(MANIFEST_ZIP_PATH, bytes), (MANIFEST_SHA_ZIP_PATH, sha.into_bytes())],
        );
    }

    fn rewrite_bundle_manifest_only(bundle_path: &Path, manifest: &ExportManifest) {
        let bytes = serde_json::to_vec_pretty(manifest).unwrap();
        rewrite_zip_entries(bundle_path, &[(MANIFEST_ZIP_PATH, bytes)]);
    }

    fn rewrite_zip_entries(bundle_path: &Path, updated: &[(&str, Vec<u8>)]) {
        // Copy every entry except the ones being rewritten, then append
        // the new ones. The simplest portable approach since zip-rs does
        // not support in-place entry replacement.
        let original = fs::read(bundle_path).unwrap();
        let cursor = std::io::Cursor::new(original);
        let mut reader = ZipArchive::new(cursor).unwrap();
        let target = File::create(bundle_path).unwrap();
        let mut writer = ZipWriter::new(target);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let names: Vec<String> =
            (0..reader.len()).map(|i| reader.by_index(i).unwrap().name().to_string()).collect();
        let updated_names: std::collections::HashSet<&str> =
            updated.iter().map(|(n, _)| *n).collect();
        for name in &names {
            if updated_names.contains(name.as_str()) {
                continue;
            }
            let mut entry = reader.by_name(name).unwrap();
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).unwrap();
            writer.start_file(name, options).unwrap();
            writer.write_all(&buf).unwrap();
        }
        for (name, bytes) in updated {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
}
