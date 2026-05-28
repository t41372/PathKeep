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
        apply_cipher_key, export_archive_database, max_schema_version, open_archive_connection,
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
    /// The archive cipher key that was used on the *source* machine.
    /// Required when the bundle's manifest reports `archive_mode =
    /// "encrypted"`. Plaintext bundles ignore this field entirely.
    ///
    /// The previous implementation re-used the *caller's* session key to
    /// open the imported archive after the on-disk swap — but for a
    /// fresh install (key=None) or a target machine whose local archive
    /// happens to be locked with a different key, that key won't open
    /// the imported database. `apply_import` then failed AFTER renaming
    /// the live target to `.bak-*`, leaving the user with the import
    /// applied on disk but reported as failed. Codex review finding C4.
    ///
    /// Surfaced through the Settings → Data migration prompt so the
    /// user is asked for the source key only when the bundle actually
    /// needs one.
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
    // Reject any traversal / absolute / drive-letter entry now, before
    // the UI even shows a confirm prompt. The user must not be invited
    // to authorise overwriting "their archive" with a bundle that would
    // actually write outside the project root.
    for declared in &manifest.files {
        validate_bundle_relative_path(&declared.path)
            .with_context(|| format!("bundle manifest entry {} is not safe", declared.path))?;
    }
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
    // Codex C4: the caller's session key is no longer consulted by
    // `apply_import`. Encrypted bundles use
    // `options.source_archive_key`; plaintext bundles need no key. The
    // parameter stays in the public signature so all existing callers
    // (commands/migration.rs, worker_bridge::apply_app_data_import_impl,
    // dev_ipc_bridge dispatch) keep compiling without churn, and we
    // mark it `_key` so the compiler doesn't warn about it being
    // unused.
    _key: Option<&str>,
    bundle_path: &Path,
    options: &ApplyImportOptions,
) -> Result<ImportResult> {
    let preview = preview_import(paths, bundle_path)?;
    if preview.will_overwrite_existing && !options.confirm_overwrite {
        anyhow::bail!(
            "the target already has an initialized PathKeep archive; re-call apply_import with confirm_overwrite=true to authorise the destructive overwrite",
        );
    }

    // Stage the bundle under the project's own `app_root` (specifically
    // the `staging/` subtree that's already in `EXPORT_EXCLUSIONS_DOC`,
    // so a future export won't accidentally pack a half-extracted bundle
    // into a new bundle). The previous implementation created the staging
    // directory under `std::env::temp_dir()` (typically `/tmp`, often on
    // a different mount), which made the subsequent `fs::rename(staged,
    // target)` raise `EXDEV` whenever the user's project root lived on a
    // different filesystem. Worse, the `.bak` rename of the live target
    // runs *before* the install rename — failure left the live archive
    // gone (renamed to `.bak-*`) with nothing installed in its place.
    // Same-filesystem staging removes the cross-device boundary entirely.
    // Codex review finding C3.
    let staging_root = paths.app_root.join("staging");
    fs::create_dir_all(&staging_root)
        .with_context(|| format!("creating staging root {}", staging_root.display()))?;
    let staging = tempfile::TempDir::new_in(&staging_root)
        .with_context(|| format!("creating import staging dir under {}", staging_root.display()))?;
    extract_bundle_into(bundle_path, staging.path(), &preview.manifest)?;

    // Codex C4: when the bundle is encrypted, verify the source key
    // BEFORE renaming any live file to `.bak-*`. The previous
    // implementation reused the caller's session key for the post-
    // install schema check — but a fresh install (key=None) or a
    // differently-keyed target can't open the imported database, and
    // the failure happened only after the swap, leaving the user with
    // their live archive renamed away and the import reported as
    // failed.
    //
    // The fail-fast path here is read-only (it opens the *staged*
    // archive under `app_root/staging/...`, not the live one), so a
    // bad key surfaces as a typed error and the live tree is
    // untouched. The verified key is then threaded through to the
    // post-install schema check at the bottom of the function.
    let staged_config = load_staged_config(staging.path())?;
    let effective_post_install_key: Option<String> = if matches!(
        staged_config.archive_mode,
        ArchiveMode::Encrypted,
    ) {
        let source_key = options.source_archive_key.as_deref().filter(|k| !k.is_empty());
        let Some(source_key) = source_key else {
            anyhow::bail!(
                "{IMPORT_SOURCE_KEY_REQUIRED_PREFIX}: the imported bundle was encrypted on the source machine. Re-call apply_import with options.source_archive_key set to the source key. The live archive on this machine is unchanged.",
            );
        };
        let staged_archive_path = staging.path().join("archive/history-vault.sqlite");
        if let Err(error) = verify_archive_key(&staged_archive_path, source_key) {
            anyhow::bail!(
                "{IMPORT_SOURCE_KEY_INVALID_PREFIX}: the supplied source_archive_key does not decrypt the imported archive ({error:?}). The live archive on this machine is unchanged.",
            );
        }
        Some(source_key.to_string())
    } else {
        // Plaintext bundles ignore source_archive_key entirely. The
        // post-install schema check uses None too, matching the legacy
        // behaviour for unencrypted archives.
        None
    };

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

    // Run forward migrations on the newly-installed archive. Encrypted
    // bundles use the verified source key (see Codex C4 above);
    // plaintext bundles use `None`. The caller's session key is no
    // longer consulted here — it was the wrong reference frame for an
    // imported archive.
    let post_install_key = effective_post_install_key.as_deref();
    let migrations_applied = if preview.migrations_to_apply.is_empty() {
        Vec::new()
    } else {
        let connection = open_archive_for_migration(paths, &imported_config, post_install_key)?;
        run_migrations(&connection)?;
        preview.migrations_to_apply.clone()
    };
    let final_schema_version = {
        let connection = open_archive_for_migration(paths, &imported_config, post_install_key)?;
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

/// Verifies that `key` opens the SQLCipher-encrypted database at
/// `db_path`. Returns `Ok(())` if the key is correct, `Err(_)` otherwise.
///
/// Used by `apply_import` to refuse encrypted bundles BEFORE renaming
/// any live file to `.bak-<ts>` when the supplied source key is wrong
/// or absent. The cheapest end-to-end test of "does this key work" is
/// to run `SELECT count(*) FROM sqlite_master` against the staged
/// database after applying the cipher key — SQLCipher fails the query
/// with a "file is not a database" / decryption error when the key is
/// wrong.
///
/// Lives in `migration.rs` (not `archive/schema.rs`) because it
/// intentionally bypasses `ensure_archive_bootstrapped`,
/// `attach_search_database`, and `seed_search_projection_if_missing` —
/// the staged archive is read-only here and we don't want to bootstrap
/// projections against a database we may yet decide to reject.
fn verify_archive_key(db_path: &Path, key: &str) -> Result<()> {
    let connection = Connection::open(db_path)
        .with_context(|| format!("opening staged archive at {}", db_path.display()))?;
    apply_cipher_key(&connection, key)?;
    connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .with_context(|| format!("verifying source archive key for {}", db_path.display()))?;
    Ok(())
}

/// Reads the staged config JSON from a freshly-extracted bundle. Returns
/// the parsed `AppConfig` so callers can decide whether the bundle is
/// encrypted *before* any destructive on-disk rename. Falls back to
/// `AppConfig::default()` (plaintext) when the bundle did not ship a
/// config — older bundles or callers may legitimately produce that.
fn load_staged_config(staging_dir: &Path) -> Result<AppConfig> {
    let staged_config_path = staging_dir.join("config/config.json");
    if !staged_config_path.exists() {
        return Ok(AppConfig::default());
    }
    let content = fs::read_to_string(&staged_config_path)
        .with_context(|| format!("reading staged config {}", staged_config_path.display()))?;
    let config: AppConfig = serde_json::from_str(&content).context("parsing staged config json")?;
    Ok(config)
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
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
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
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
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
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
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

    #[test]
    fn export_app_data_bails_when_archive_database_missing() {
        // Tries to export a project root that has never been initialized.
        // The early bail keeps callers from producing a manifest that
        // references a non-existent archive db.
        let (src_dir, src_paths) = fresh_paths();
        let config = AppConfig::default();
        let target = src_dir.path().join("nope.pathkeep");
        let err = export_app_data(&src_paths, &config, None, &target)
            .expect_err("export must refuse missing archive db");
        let message = format!("{err:?}");
        assert!(
            message.contains("no archive to export"),
            "expected missing-archive bail, got {message}",
        );
        assert!(
            message.contains(&src_paths.archive_database_path.display().to_string()),
            "bail message must name the missing archive path, got {message}",
        );
    }

    #[test]
    fn export_app_data_rejects_encrypted_archive_without_key() {
        // Drives the `encryption_key_for_copy` error arm: the config asks
        // for an encrypted bundle but the caller forgot to pass the active
        // database key. We seed a plaintext archive on disk so the early
        // exists() check passes; the failure must surface from the key
        // resolver, not from a half-written zip.
        let (src_dir, src_paths) = fresh_paths();
        seed_archive(&src_paths);
        let encrypted_config =
            AppConfig { archive_mode: ArchiveMode::Encrypted, ..AppConfig::default() };
        let target = src_dir.path().join("enc-nokey.pathkeep");
        let err = export_app_data(&src_paths, &encrypted_config, None, &target)
            .expect_err("encrypted export without key must be rejected");
        assert!(
            format!("{err:?}").contains("encrypted archive must be unlocked"),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn export_app_data_writes_encrypted_mode_in_manifest_for_encrypted_archives() {
        // The `archive_mode` match arm for Encrypted is otherwise unreached
        // because every other test in this module relies on the plaintext
        // default. Drive a real sqlcipher-encrypted archive through export
        // so the manifest gets the `"encrypted"` string.
        let (src_dir, src_paths) = fresh_paths();
        let config = AppConfig {
            archive_mode: ArchiveMode::Encrypted,
            initialized: true,
            ..AppConfig::default()
        };
        let key = "vault-encrypted-test-key";
        crate::archive::ensure_archive_initialized(&src_paths, &config, Some(key))
            .expect("init encrypted archive");
        save_config(&src_paths, &config).expect("save encrypted config");

        let target = src_dir.path().join("encrypted.pathkeep");
        let bundle = export_app_data(&src_paths, &config, Some(key), &target)
            .expect("encrypted export should succeed");
        assert_eq!(bundle.manifest.archive_mode, "encrypted");
    }

    #[test]
    fn apply_import_rejects_encrypted_bundle_when_source_key_is_missing() {
        // Codex C4: encrypted bundle imported without
        // `options.source_archive_key` must refuse BEFORE the live tree
        // is touched. Caller's session key (legacy `key: Option<&str>`)
        // is ignored — that was the wrong reference frame.
        let (src_dir, src_paths) = fresh_paths();
        let encrypted_config = AppConfig {
            archive_mode: ArchiveMode::Encrypted,
            initialized: true,
            ..AppConfig::default()
        };
        let source_key = "source-machine-key";
        crate::archive::ensure_archive_initialized(&src_paths, &encrypted_config, Some(source_key))
            .expect("init encrypted source archive");
        save_config(&src_paths, &encrypted_config).expect("save source config");
        let bundle_path = src_dir.path().join("encrypted-needs-key.pathkeep");
        export_app_data(&src_paths, &encrypted_config, Some(source_key), &bundle_path)
            .expect("export encrypted bundle");

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        let err = apply_import(
            &dest_paths,
            &dest_config,
            None, // caller's session key — intentionally untouched here
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false, source_archive_key: None },
        )
        .expect_err("encrypted bundle without source key must be refused");

        let message = format!("{err:?}");
        assert!(
            message.contains(IMPORT_SOURCE_KEY_REQUIRED_PREFIX),
            "expected {IMPORT_SOURCE_KEY_REQUIRED_PREFIX} prefix, got: {message}",
        );
        // Live archive on dest never existed; confirm the refusal didn't
        // leave a `.bak-*` sibling next to the dest archive path, which
        // would mean the `.bak` rename loop ran. `fresh_paths` does not
        // pre-create `app_root/archive/`, so the *parent* directory is
        // absent on a refusal that bails before the rename loop — that
        // by itself is the assertion we want.
        assert!(
            !dest_paths.archive_database_path.exists(),
            "fresh dest must still have no archive after refusal",
        );
        assert!(
            !dest_paths.archive_database_path.parent().unwrap().exists(),
            "refusal must bail BEFORE creating the archive/ directory \
             (a `.bak-*` sibling would mean the rename loop ran)",
        );
    }

    #[test]
    fn apply_import_rejects_encrypted_bundle_when_source_key_is_wrong() {
        // Codex C4: encrypted bundle imported with a *wrong*
        // `source_archive_key` must refuse BEFORE the live tree is
        // touched, with a different error prefix than the "missing key"
        // case so the UI can swap copy between "please enter" and
        // "wrong key, try again."
        let (src_dir, src_paths) = fresh_paths();
        let encrypted_config = AppConfig {
            archive_mode: ArchiveMode::Encrypted,
            initialized: true,
            ..AppConfig::default()
        };
        let source_key = "source-machine-key";
        crate::archive::ensure_archive_initialized(&src_paths, &encrypted_config, Some(source_key))
            .expect("init encrypted source archive");
        save_config(&src_paths, &encrypted_config).expect("save source config");
        let bundle_path = src_dir.path().join("encrypted-wrong-key.pathkeep");
        export_app_data(&src_paths, &encrypted_config, Some(source_key), &bundle_path)
            .expect("export encrypted bundle");

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        let err = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions {
                confirm_overwrite: false,
                source_archive_key: Some("this-is-not-the-right-key".to_string()),
            },
        )
        .expect_err("encrypted bundle with wrong source key must be refused");

        let message = format!("{err:?}");
        assert!(
            message.contains(IMPORT_SOURCE_KEY_INVALID_PREFIX),
            "expected {IMPORT_SOURCE_KEY_INVALID_PREFIX} prefix, got: {message}",
        );
    }

    #[test]
    fn apply_import_accepts_encrypted_bundle_with_matching_source_key() {
        // Codex C4 happy path: source_archive_key matches the bundle's
        // cipher key → the staged-archive verify step passes, the
        // install proceeds, and the post-install schema check uses the
        // verified source key (not the caller's session key).
        let (src_dir, src_paths) = fresh_paths();
        let encrypted_config = AppConfig {
            archive_mode: ArchiveMode::Encrypted,
            initialized: true,
            ..AppConfig::default()
        };
        let source_key = "source-machine-key";
        crate::archive::ensure_archive_initialized(&src_paths, &encrypted_config, Some(source_key))
            .expect("init encrypted source archive");
        save_config(&src_paths, &encrypted_config).expect("save source config");
        let bundle_path = src_dir.path().join("encrypted-correct-key.pathkeep");
        export_app_data(&src_paths, &encrypted_config, Some(source_key), &bundle_path)
            .expect("export encrypted bundle");

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        let result = apply_import(
            &dest_paths,
            &dest_config,
            // The caller's session key on the target machine could be
            // None (fresh install) or some other unrelated key — the
            // import must NOT use it. Pass an obviously-wrong value to
            // pin that contract.
            Some("ignored-target-session-key"),
            &bundle_path,
            &ApplyImportOptions {
                confirm_overwrite: false,
                source_archive_key: Some(source_key.to_string()),
            },
        )
        .expect("encrypted bundle with matching source key must apply");

        assert!(
            dest_paths.archive_database_path.exists(),
            "encrypted archive must be installed at the dest after success",
        );
        assert!(
            result.final_schema_version > 0,
            "schema-check step must succeed with the verified source key",
        );
    }

    #[test]
    fn apply_import_ignores_source_archive_key_for_plaintext_bundles() {
        // Plaintext bundles must keep working even when a
        // source_archive_key is passed (e.g. the UI threads a value
        // through unconditionally). This protects the common case where
        // the user happens to type a key into the import dialog before
        // realising the bundle is plaintext.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("plaintext-with-key.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).expect("export plaintext bundle");

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions {
                confirm_overwrite: false,
                source_archive_key: Some("totally-unused".to_string()),
            },
        )
        .expect("plaintext bundle must apply regardless of source_archive_key");
        assert!(result.final_schema_version > 0);
    }

    #[test]
    fn apply_import_applies_pending_migrations_when_bundle_schema_is_older() {
        // Forge a manifest claiming the bundle was produced at one schema
        // version below the current binary so `migrations_to_apply` is
        // non-empty. The actual on-disk archive is already current, so
        // `run_migrations` is a checksum-matching no-op — what we're
        // verifying is that the else-branch (the migration runner) is
        // taken and propagated.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("older-schema.pathkeep");
        let mut bundle = export_app_data(&src_paths, &config, None, &bundle_path).unwrap();
        let local_max = max_schema_version();
        assert!(local_max >= 1, "this test assumes at least one migration exists");
        bundle.manifest.archive_schema_version = local_max - 1;
        rewrite_bundle_manifest(&bundle_path, &bundle.manifest);

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
        )
        .expect("apply with pending migrations");
        assert_eq!(result.migrations_applied, vec![local_max]);
        assert_eq!(result.final_schema_version, local_max);
    }

    #[test]
    fn add_dir_to_zip_if_exists_returns_early_when_source_directory_missing() {
        // Direct unit test for the early-return branch. Production tests
        // can't reach this through `export_app_data` because
        // `ensure_paths` recreates every included directory before the
        // zip walk runs.
        let dir = TempDir::new().expect("tempdir");
        let zip_path = dir.path().join("out.zip");
        let file = File::create(&zip_path).expect("create zip");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let mut manifest_files: Vec<ExportManifestFile> = Vec::new();

        let missing = dir.path().join("never-existed");
        add_dir_to_zip_if_exists(&mut zip, &missing, "prefix", options, &mut manifest_files)
            .expect("early return when source dir missing must succeed");
        zip.finish().expect("finish zip");

        assert!(manifest_files.is_empty(), "no files should be added for a missing source dir");
    }

    #[test]
    fn preview_import_rejects_bundle_without_export_manifest() {
        // Cover the `read_zip_entry_bytes` "missing manifest" with_context
        // closure: build a bare zip that lacks the export manifest entry.
        let dir = TempDir::new().expect("tempdir");
        let bundle_path = dir.path().join("no-manifest.zip");
        let file = File::create(&bundle_path).expect("create empty zip");
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("payload.bin", options).expect("start dummy entry");
        zip.write_all(b"placeholder").expect("write dummy payload");
        zip.finish().expect("finish zip");

        let (_dest_dir, dest_paths) = fresh_paths();
        let err = preview_import(&dest_paths, &bundle_path)
            .expect_err("bundle without manifest must be rejected");
        assert!(
            format!("{err:?}").contains("is this a PathKeep export"),
            "expected missing-manifest context, got {err:?}",
        );
    }

    #[test]
    fn extract_bundle_into_rejects_entries_whose_bytes_do_not_match_manifest_sha256() {
        // Tamper with a non-manifest entry inside an already-valid bundle:
        // the manifest still says the original sha256, but the on-disk
        // bytes are replaced. extract_bundle_into must bail before
        // touching the live project tree.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("payload-tampered.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        // Replace the marker file bytes inside the zip without updating
        // the manifest entry, so the extract-time hash check disagrees
        // with the manifest's recorded digest.
        rewrite_zip_entries(&bundle_path, &[("derived/marker.txt", b"counterfeit".to_vec())]);

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        let err = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
        )
        .expect_err("payload tamper must be rejected");
        assert!(
            format!("{err:?}").contains("sha256 mismatch"),
            "expected per-entry sha mismatch, got {err:?}",
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_import_propagates_file_preserve_rename_error_when_app_root_is_readonly() {
        // Covers the `with_context` closure on the *file* preserve rename:
        // we deny write on app_root so renaming the existing
        // `root/config.json` to its `.bak-<timestamp>` sibling fails at
        // the OS level. The error message must name the preserve step so
        // operators can diagnose the failed swap.
        use std::os::unix::fs::PermissionsExt;
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("file-preserve-fail.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = seed_archive(&dest_paths);
        // After Codex C3, staging lives under `app_root/staging/`. Pre-
        // create it so the read-only app_root doesn't short-circuit at
        // staging-dir creation — we want the original file-preserve
        // rename to be the failing point this test was written for.
        fs::create_dir_all(dest_paths.app_root.join("staging")).unwrap();

        let app_root = dest_paths.app_root.clone();
        let original = fs::metadata(&app_root).unwrap().permissions();
        let mut locked = original.clone();
        locked.set_mode(0o500);
        fs::set_permissions(&app_root, locked).unwrap();

        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        );

        // Restore permissions *before* asserting so a panic still lets
        // the tempdir drop succeed.
        fs::set_permissions(&app_root, original).unwrap();
        let err = result.expect_err("read-only app_root must surface preserve-rename error");
        assert!(
            format!("{err:?}").contains("preserving previous"),
            "expected preserve-rename context, got {err:?}",
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_import_propagates_directory_preserve_rename_error_when_app_root_is_readonly() {
        // Covers the *directory*-loop preserve rename closure. The trick
        // is to make every movable-file step skip (so they never trip on
        // the readonly app_root first) and only leave the included
        // directory rename to fail:
        //   - source: archive db + derived/marker.txt only, no config,
        //     no source-evidence (those entries are absent from the
        //     bundle, so apply_import's per-file `staged.exists()` skips
        //     each movable file iteration).
        //   - dest:  pre-create `app_root/archive/` so ensure_parent_dir
        //     for the staged archive db succeeds without needing write on
        //     app_root, and pre-create `app_root/derived/` as a non-empty
        //     dir so target.exists() is true and we hit the preserve
        //     branch.
        //   - app_root: chmod 0o500 → write denied at the app_root level,
        //     which makes `rename(app_root/derived, app_root/derived.bak)`
        //     fail (parent is read-only) while leaving the
        //     `app_root/archive/` install rename writable.
        use std::os::unix::fs::PermissionsExt;
        let (src_dir, src_paths) = fresh_paths();
        // Minimal source: archive db + derived dir only.
        let config = AppConfig::default();
        fs::create_dir_all(src_paths.archive_database_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&src_paths.derived_dir).unwrap();
        crate::archive::create_schema(
            &crate::archive::open_archive_connection(&src_paths, &config, None).unwrap(),
        )
        .unwrap();
        fs::write(src_paths.derived_dir.join("marker.txt"), b"derived sentinel").unwrap();
        // Deliberately do not save the config and remove the
        // source-evidence db that `ensure_paths` may have created.
        let _ = fs::remove_file(&src_paths.config_path);
        let _ = fs::remove_file(&src_paths.source_evidence_database_path);

        let bundle_path = src_dir.path().join("dir-preserve-fail.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        // Remove every dest-side movable target so the file loop has
        // nothing to rename, and pre-create the archive db parent so
        // ensure_parent_dir does not require write on app_root.
        let _ = fs::remove_file(&dest_paths.config_path);
        let _ = fs::remove_file(&dest_paths.archive_database_path);
        let _ = fs::remove_file(&dest_paths.source_evidence_database_path);
        fs::create_dir_all(dest_paths.archive_database_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&dest_paths.derived_dir).unwrap();
        fs::write(dest_paths.derived_dir.join("sentinel.txt"), b"pre-import").unwrap();
        // After Codex C3, apply_import stages under `app_root/staging/`.
        // Pre-create that directory so the read-only app_root permission
        // doesn't fail at the `fs::create_dir_all(staging_root)` line —
        // we want the test to still exercise the *dir-preserve* rename
        // closure that this test was originally written for.
        fs::create_dir_all(dest_paths.app_root.join("staging")).unwrap();

        let app_root = dest_paths.app_root.clone();
        let original = fs::metadata(&app_root).unwrap().permissions();
        let mut locked = original.clone();
        locked.set_mode(0o500);
        fs::set_permissions(&app_root, locked).unwrap();

        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
        );

        fs::set_permissions(&app_root, original).unwrap();
        let err = result.expect_err("read-only app_root must surface dir-preserve rename error");
        let message = format!("{err:?}");
        assert!(
            message.contains("preserving previous"),
            "expected dir-preserve context, got {message}",
        );
        // Confirm we really tripped on the directory branch (the message
        // must reference the derived path, not a config/archive file).
        assert!(
            message.contains("derived"),
            "preserve error must name the derived directory, got {message}",
        );
    }

    #[cfg(unix)]
    #[cfg(unix)]
    #[test]
    fn apply_import_propagates_file_install_rename_error_when_app_root_is_readonly() {
        // After Codex C3 the staging dir lives under `app_root/staging/`,
        // so EXDEV can no longer fire on the install rename. We can
        // still drive the install closure (line 495 `installing {staged}
        // into {target}`) by chmod-ing app_root to read-only AFTER
        // creating the staging subtree. fs::rename(staged, target)
        // then fails with EACCES because target's parent (= app_root)
        // is no longer writable. Pre-creating staging/ and archive/
        // beforehand keeps the staging extraction itself working.
        use std::os::unix::fs::PermissionsExt;
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("file-install-fail.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        // Pre-create app_root/staging/ AND app_root/archive/ so the
        // staging extraction step succeeds (TempDir + extract write
        // inside these subdirs, which inherit 0o755 from `fresh_paths`).
        fs::create_dir_all(dest_paths.app_root.join("staging")).unwrap();
        fs::create_dir_all(dest_paths.archive_database_path.parent().unwrap()).unwrap();

        let app_root = dest_paths.app_root.clone();
        let original = fs::metadata(&app_root).unwrap().permissions();
        let mut locked = original.clone();
        locked.set_mode(0o500);
        fs::set_permissions(&app_root, locked).unwrap();

        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
        );
        fs::set_permissions(&app_root, original).unwrap();

        let err = result.expect_err("install rename onto readonly app_root must surface error");
        let message = format!("{err:?}");
        assert!(message.contains("installing"), "expected install-rename context, got {message}",);
    }

    #[cfg(unix)]
    #[test]
    fn apply_import_propagates_directory_install_rename_error_when_app_root_is_readonly() {
        // Twin of the file-install test, but tightened so the failure
        // arises from the *directory* install loop (line 514). Trim
        // every non-derived manifest entry + zip entry so the file
        // loop's `staged.exists()` short-circuits each iteration; the
        // dir loop is then the only place left to trip.
        use std::os::unix::fs::PermissionsExt;
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("dir-install-fail.pathkeep");
        let bundle = export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let mut trimmed_manifest = bundle.manifest.clone();
        let drop_zip_entries: Vec<String> = trimmed_manifest
            .files
            .iter()
            .filter(|f| !f.path.starts_with("derived/"))
            .map(|f| f.path.clone())
            .collect();
        trimmed_manifest.files.retain(|f| f.path.starts_with("derived/"));
        remove_zip_entries(&bundle_path, &drop_zip_entries);
        rewrite_bundle_manifest(&bundle_path, &trimmed_manifest);

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        // Pre-create staging/ before locking app_root so the import
        // can extract its (single derived/...) entry. app_root/derived/
        // is intentionally NOT pre-created — `target.exists()` will be
        // false, the preserve branch is skipped, and only the install
        // rename remains to fail under the read-only app_root.
        fs::create_dir_all(dest_paths.app_root.join("staging")).unwrap();

        let app_root = dest_paths.app_root.clone();
        let original = fs::metadata(&app_root).unwrap().permissions();
        let mut locked = original.clone();
        locked.set_mode(0o500);
        fs::set_permissions(&app_root, locked).unwrap();

        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
        );
        fs::set_permissions(&app_root, original).unwrap();

        let err = result.expect_err("dir install rename onto readonly app_root must surface error");
        let message = format!("{err:?}");
        assert!(
            message.contains("installing") && message.contains("derived"),
            "expected derived-install context, got {message}",
        );
    }

    fn remove_zip_entries(bundle_path: &Path, drop: &[String]) {
        // zip-rs has no in-place delete API; rewrite the archive,
        // keeping every entry whose name is not in `drop`.
        let original = fs::read(bundle_path).unwrap();
        let cursor = std::io::Cursor::new(original);
        let mut reader = ZipArchive::new(cursor).unwrap();
        let target = File::create(bundle_path).unwrap();
        let mut writer = ZipWriter::new(target);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        let names: Vec<String> =
            (0..reader.len()).map(|i| reader.by_index(i).unwrap().name().to_string()).collect();
        let drop_set: std::collections::HashSet<&str> = drop.iter().map(String::as_str).collect();
        for name in &names {
            if drop_set.contains(name.as_str()) {
                continue;
            }
            let mut entry = reader.by_name(name).unwrap();
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).unwrap();
            writer.start_file(name, options).unwrap();
            writer.write_all(&buf).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn apply_import_succeeds_when_dest_lives_on_different_filesystem_than_default_tempdir() {
        // Codex review finding C3: the previous implementation staged
        // imports under `std::env::temp_dir()` (commonly `/tmp`,
        // tmpfs on most Linux distros). When the user's project root
        // lived on a different filesystem (USB drive, custom storage,
        // separate `/home` mount), `fs::rename(staged, target)` raised
        // `EXDEV` — but only AFTER the live target had already been
        // renamed to `.bak-*`. The user was left with a "preserved"
        // backup and nothing installed.
        //
        // The fix moves staging under `paths.app_root.join("staging")`,
        // which is by definition on the same filesystem as the install
        // targets. This test pins that contract: a dest anchored on
        // `/dev/shm` (tmpfs) imports cleanly even though the OS default
        // tempdir is `/tmp` (also tmpfs but a different mount), because
        // staging now follows the dest, not the system tempdir.
        let cross_fs_root = std::path::Path::new("/dev/shm");
        assert!(cross_fs_root.exists(), "this test needs /dev/shm to be writable");

        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("xdev-install.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let dest_dir = TempDir::new_in(cross_fs_root).expect("dest on /dev/shm");
        let dest_paths = project_paths_with_root(dest_dir.path());
        let dest_config = seed_archive(&dest_paths);

        let result = apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        );

        let outcome =
            result.expect("apply_import must succeed when dest is on a different fs than /tmp");
        assert!(
            outcome.preserved_previous_as_bak,
            "the dest had a live archive that must have been preserved as .bak",
        );
        assert!(
            dest_paths.archive_database_path.exists(),
            "the imported archive must be installed at the canonical target",
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_import_staging_dir_lives_under_app_root_not_system_tempdir() {
        // Direct verification that the staging root for imports is
        // anchored inside the project's own `app_root`. We can observe
        // this from the outside by confirming that `staging/` exists
        // under the dest app_root after a successful import (the
        // TempDir::new_in target dir is created up-front by
        // `fs::create_dir_all`). If a future change accidentally
        // reverted to `tempdir()` (under `std::env::temp_dir()`), this
        // sentinel directory would never appear under `app_root` and
        // the cross-fs guarantee would silently regress.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("staging-anchor.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let (dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
        )
        .expect("import must succeed on a fresh dest");

        let staging_root = dest_paths.app_root.join("staging");
        assert!(
            staging_root.exists(),
            "staging root {} must be created inside app_root, not under the system tempdir",
            staging_root.display(),
        );
        // The actual TempDir sub-directory inside staging/ is cleaned
        // up on RAII drop at the end of apply_import — its absence is
        // expected. The `staging/` parent stays because we created it
        // with `fs::create_dir_all` and it's the documented import
        // workspace location (see EXPORT_EXCLUSIONS_DOC).
        let _ = dest_dir;
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

    #[test]
    fn preview_import_rejects_bundle_with_traversal_manifest_entry() {
        // Build a real bundle and then rewrite its manifest to declare a
        // `..` entry. The sha sidecar is recomputed (it's inside the same
        // zip), mirroring what a malicious bundle author would actually
        // do. preview_import must reject before the UI ever sees an
        // overwrite confirmation prompt.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("traversal.pathkeep");
        let bundle = export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let mut tampered = bundle.manifest.clone();
        tampered.files.push(ExportManifestFile {
            path: "../escape.txt".to_string(),
            sha256: sha256_hex(b"escape-payload"),
            size_bytes: b"escape-payload".len() as u64,
        });
        // Add a matching zip entry so the bundle is otherwise consistent;
        // the rejection must come from validate_bundle_relative_path, not
        // from a missing-entry / sha-mismatch path.
        rewrite_zip_entries(&bundle_path, &[("../escape.txt", b"escape-payload".to_vec())]);
        rewrite_bundle_manifest(&bundle_path, &tampered);

        let (_dest_dir, dest_paths) = fresh_paths();
        let err = preview_import(&dest_paths, &bundle_path)
            .expect_err("traversal manifest entry must be rejected");
        assert!(
            format!("{err:?}").contains("..` component"),
            "expected `..` rejection, got {err:?}",
        );
    }

    #[test]
    fn preview_import_rejects_bundle_with_absolute_manifest_entry() {
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);
        let bundle_path = src_dir.path().join("absolute.pathkeep");
        let bundle = export_app_data(&src_paths, &config, None, &bundle_path).unwrap();

        let mut tampered = bundle.manifest.clone();
        tampered.files.push(ExportManifestFile {
            path: "/tmp/owned.txt".to_string(),
            sha256: sha256_hex(b"owned"),
            size_bytes: b"owned".len() as u64,
        });
        rewrite_zip_entries(&bundle_path, &[("/tmp/owned.txt", b"owned".to_vec())]);
        rewrite_bundle_manifest(&bundle_path, &tampered);

        let (_dest_dir, dest_paths) = fresh_paths();
        let err = preview_import(&dest_paths, &bundle_path)
            .expect_err("absolute manifest entry must be rejected");
        assert!(
            format!("{err:?}").contains("is absolute"),
            "expected absolute-path rejection, got {err:?}",
        );
    }

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

#[cfg(test)]
mod fault_tests;
