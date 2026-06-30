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
//! - In-process worker quiescence around the swap. The cross-process
//!   [`ArchiveWriteLock`] this module holds excludes the OUT-OF-PROCESS
//!   scheduled backup; serializing against other IN-PROCESS workers
//!   (e.g. a GUI command dispatched concurrently) remains the
//!   lock-completion carry-in and is not yet wired (see MEDIUM-E).
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
        ArchiveWriteLock, DiskEncryptionMode, apply_cipher_key, detect_disk_encryption_mode,
        export_archive_database, max_schema_version, open_archive_connection,
        open_source_evidence_connection, remove_stale_sidecars, run_migrations,
    },
    config::{ProjectPaths, load_config, save_config},
    durable_io::{
        atomic_durable_write, fsync_file_durably, install_file_durably, remove_file_durably,
    },
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
    ("models/", "Downloaded embedding/reranker models are re-downloadable on the target machine."),
    ("derived/agent.sqlite", "Assistant chat transcripts stay on the source machine."),
    (
        "derived/vectors/",
        "Raw f32 embedding vectors (~59 GB at the 14.4M tail) are rebuildable derived state; re-embed on the target instead of shipping them.",
    ),
    (
        "sidecars/intelligence-blobs/",
        "Raw enrichment body/caption blobs are re-fetchable, possibly large, and possibly stale; the capped enrichment summary rides in the intelligence database so offline search still works.",
    ),
];

/// Top-level subtree name (under `sidecars/`) holding the raw enrichment body/caption blobs.
///
/// These are the content-addressed readable-text blobs the enrichment plane writes (title-plugin
/// fallbacks + W-ENRICH-1 fetched bodies). They are EXCLUDED from the export bundle (06 §3): they are
/// re-fetchable/re-derivable and can be large, while the CAPPED `enrichment_summary` rides inside the
/// exported intelligence database so offline keyword search still resolves on the target.
const SIDECAR_ENRICHMENT_BLOBS_DIRNAME: &str = "intelligence-blobs";

/// Returns true when a `sidecars/`-relative path is part of the raw enrichment blob plane (06 §3).
///
/// Matches the whole `sidecars/intelligence-blobs/` subtree so neither the title-plugin blobs nor the
/// W-ENRICH-1 fetched-body blobs ride the export. `relative` is the path under the `sidecars/` root.
fn is_sidecar_enrichment_blob_excluded(relative: &Path) -> bool {
    relative
        .components()
        .next()
        .is_some_and(|first| first.as_os_str() == SIDECAR_ENRICHMENT_BLOBS_DIRNAME)
}

/// Files under `derived/` that are skipped by [`add_dir_to_zip_if_exists`] even though `derived/`
/// is otherwise an included subtree.
///
/// The agent sidecar (`agent.sqlite` + its WAL/SHM siblings) holds assistant chat transcripts, a
/// privacy-sensitive plane that — by data-sovereignty default — never rides the portable export
/// bundle. The base name plus any `-wal`/`-shm`/`-journal` suffix is matched so WAL-mode artifacts
/// are excluded too.
const DERIVED_EXPORT_EXCLUDED_BASENAMES: &[&str] = &["agent.sqlite"];

/// Returns true when a file basename belongs to a `derived/`-level export exclusion (the agent
/// sidecar database and its WAL/SHM/journal siblings).
fn is_derived_export_excluded(file_name: &str) -> bool {
    DERIVED_EXPORT_EXCLUDED_BASENAMES.iter().any(|excluded| {
        file_name == *excluded
            || file_name == format!("{excluded}-wal")
            || file_name == format!("{excluded}-shm")
            || file_name == format!("{excluded}-journal")
    })
}

/// Subdirectory of `derived/` that holds the AI vector sidecar plane (the `.pkvec` stores).
const DERIVED_VECTOR_PLANE_DIRNAME: &str = "vectors";

/// File extensions of the AI vector sidecar plane excluded from the export bundle.
///
/// `pkvec` is the raw f32 vector store; `pkmap` is the visit→content_key map beside it (W-AI-4c);
/// `pkbin`/`pki8` are the W-AI-5 derived binary-recall + int8-rescore planes projected from `.pkvec`;
/// `pkrev`/`pkfwd` are the M-11 keyed reverse/forward sidecars projected from `.pkmap` (sorted by
/// content_key / history_id for bounded hydration + `is:starred` lookups). All are rebuildable derived
/// state that re-projects/re-embeds on the target, so none ride the portable export.
const DERIVED_VECTOR_PLANE_EXTENSIONS: &[&str] =
    &["pkvec", "pkmap", "pkbin", "pki8", "pkrev", "pkfwd"];

/// Returns true when a `derived/`-relative path is part of the vector sidecar plane and so must be
/// excluded from the export bundle (HIGH-4).
///
/// Matches the whole `derived/vectors/` subtree, plus any stray `.pkvec` / `.pkmap` store written
/// elsewhere under `derived/`, so the ~59 GB rebuildable f32 vectors AND their visit→content map never
/// ride the portable export. `relative` is the path under the `derived/` root (the first component is
/// the top-level child name).
fn is_derived_vector_plane_excluded(relative: &Path) -> bool {
    let under_vectors_dir = relative
        .components()
        .next()
        .is_some_and(|first| first.as_os_str() == DERIVED_VECTOR_PLANE_DIRNAME);
    let is_vector_plane_file = relative
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| DERIVED_VECTOR_PLANE_EXTENSIONS.contains(&ext));
    under_vectors_dir || is_vector_plane_file
}

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
/// Crash-safety contract (the 2026-06-30 data-integrity audit). The two canonical
/// renames are NOT atomic with each other, so rather than pretend they are, the
/// commit phase is bracketed by a DURABLE interrupted-import marker: a crash
/// mid-commit is SIGNALED and reverted, never silently opened as a mixed archive.
///   1. Re-validate the manifest + per-file sha256, extract into a same-filesystem
///      staging tree, and (for encrypted bundles) verify the source key — all
///      read-only on the live tree, so a refusal here changes nothing.
///   2. Hold the cross-process [`ArchiveWriteLock`] for the whole destructive
///      section, so the SEPARATE scheduled-backup process can never race the swap.
///   3. PREPARE: fsync each staged canonical DB in place, so the swap renames issue
///      back-to-back behind a barrier that is already paid (LOW-G: staging is the
///      SAME filesystem as the archive by construction — both immediate children of
///      `app_root` — so each `install_file_durably` rename is atomic; the only
///      un-fsynced half is the staging-dir unlink, harmless because staging is a
///      RAII-cleaned `TempDir`, so we fsync the bytes here instead of doubling I/O
///      by first copying a multi-GB archive into the archive dir).
///   4. Write the interrupted-import journal DURABLY BEFORE the first swap (HIGH-B).
///   5. COMMIT (one unit): install each canonical DB (`history-vault.sqlite`, then
///      `source-evidence.sqlite`) as a DB+sidecars unit — preserve the old file as
///      a complete `.bak-<ts>` unit (MEDIUM-C: its hot `-wal` rides along), scrub
///      the target's pre-existing sidecars + assert no FOREIGN `<target>-wal`
///      survives (CRIT-3), then durably swap the staged copy in. If the bundle
///      OMITS a canonical DB the dest still has, preserve+remove the stale one so it
///      can't pair (mode-drifted) with the new history-vault (MEDIUM-F). Then
///      install the rebuildable subtrees, and write config LAST (step 6). ANY
///      returned error in this phase rolls the whole unit back from the `.bak`s.
///   6. Config LAST, ALWAYS written (no skip branch), with `archive_mode` forced to
///      the INSTALLED DBs' actual at-rest mode read from their on-disk header
///      (HIGH-A) — the bytes on disk, not the bundle metadata, are the authority,
///      so config↔DB drift (the NOTADB brick) is impossible.
///   7. COMMIT POINT: clear the marker DURABLY — that is what commits the import.
///   8. Run forward schema migrations on the newly-installed archive (post-commit:
///      a failure here returns Err but does NOT roll back), then prune older
///      `.bak-<ts>` generations once the import is verified-openable (MEDIUM-D).
///
/// [`recover_interrupted_import`] is the crash-recovery twin of the in-band
/// rollback: it walks the SAME journal at next open and restores the consistent
/// pre-import state. It is wired at THREE destructive pre-open sites today — the
/// unlock-path [`crate::archive::reconcile_archive_encryption`], the backup pre-open
/// path ([`crate::archive::run_backup_with_progress`], which closes the out-of-process
/// scheduled-backup hole where a crashed import's same-mode half-state would otherwise
/// be backed up and recorded as success), AND the rekey pre-open path
/// ([`crate::archive::rekey_archive`], which closes the hole where a rekey on a PLAINTEXT
/// archive — never reached by the encryption-gated launch reconcile — would rekey a
/// half-applied import into a permanent config↔source-evidence mode-drift brick). The
/// remaining destructive pre-open paths (retention-prune / snapshot-restore) do NOT yet
/// hold the [`ArchiveWriteLock`], so they get recover-first wiring as part of the
/// lock-completion carry-in (Phase-C, MEDIUM-E) rather than a half-measure
/// recover-without-lock; until then the fail-closed
/// [`ensure_recovered_modes_are_consistent`] guard keeps any eventual recovery from
/// silently committing a mode-drifted config.
///
/// The fault-injection checkpoints (`import.after_stage_before_swap`,
/// `import.after_canonical_install`, `import.after_swap_before_config`,
/// `import.after_config`) are no-ops in production and let crash-window tests prove
/// the recoverability invariant at each step.
///
/// In-process worker quiescence is NOT this function's responsibility: the
/// cross-process [`ArchiveWriteLock`] covers the out-of-process scheduler; excluding
/// concurrent IN-PROCESS workers is the lock-completion carry-in, not yet wired
/// (MEDIUM-E).
pub fn apply_import(
    paths: &ProjectPaths,
    // The imported config now comes from the bundle's own `config/config.json`
    // (the source of truth for the imported archive's mode), written LAST and
    // reloaded through `load_config`. `config` survives only as the last-resort
    // fallback if that reload somehow fails.
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

    // (2) Serialize the entire destructive section against every other
    // archive-mutating op — crucially the SEPARATE scheduled-backup process —
    // until this guard drops at the end of the function. Acquired AFTER the
    // read-only staging + source-key verification above so a refusal path never
    // even materialises the archive directory (the lock sentinel lives there).
    // The manager is process-reentrant, so any nested helper that re-acquires
    // (none today) would be safe.
    let _write_lock =
        ArchiveWriteLock::acquire(paths).context("acquiring the archive write lock for import")?;

    // ---- PREPARE: push every staged canonical DB to the platter IN PLACE so the
    // swap renames below issue back-to-back behind a barrier that is already paid
    // (HIGH-B / LOW-G). The renames are cross-DIRECTORY (staging -> archive), but the
    // C3 fix guarantees both live directly under `app_root`, so each is atomic on one
    // filesystem; pre-fsyncing the bytes here avoids doubling I/O on a multi-GB
    // archive (see the `apply_import` doc comment).
    let canonical_dbs: &[(&str, &Path)] = &[
        ("archive/history-vault.sqlite", paths.archive_database_path.as_path()),
        ("archive/source-evidence.sqlite", paths.source_evidence_database_path.as_path()),
    ];
    for (zip_relative, _) in canonical_dbs {
        let staged = staging.path().join(zip_relative);
        if staged.exists() {
            fsync_file_durably(&staged)?;
        }
    }

    // A crash HERE is a full no-op: no marker written, no `.bak` made — only staged
    // temps under `staging/` exist, and they are RAII-cleaned with the `TempDir`.
    crate::fault_inject::checkpoint("import.after_stage_before_swap")?;

    let timestamp = crate::utils::now_rfc3339().replace(':', "-");

    // Build + durably write the interrupted-import journal BEFORE the first swap
    // (HIGH-B): its presence at next open SIGNALS a cut commit phase, and its
    // contents drive BOTH the in-band rollback below and `recover_interrupted_import`.
    let previous_config: Option<String> =
        if paths.config_path.exists() {
            Some(fs::read_to_string(&paths.config_path).with_context(|| {
                format!("reading {} before import", paths.config_path.display())
            })?)
        } else {
            None
        };
    let mut journal = ImportJournal {
        version: 1,
        timestamp: timestamp.clone(),
        canonical: Vec::new(),
        subtrees: Vec::new(),
        previous_config,
    };
    for (zip_relative, target) in canonical_dbs {
        let staged_present = staging.path().join(zip_relative).exists();
        // Journal an entry when we will INSTALL a staged DB, OR (MEDIUM-F) when the
        // dest has a stale canonical DB the bundle omits that we will preserve+remove.
        if staged_present || target.exists() {
            journal.canonical.push(ImportJournalEntry {
                target: target.to_path_buf(),
                had_previous: target.exists(),
            });
        }
    }
    for prefix in INCLUDED_DIRECTORY_PREFIXES {
        if staging.path().join(prefix).exists() {
            let target = paths.app_root.join(prefix);
            journal
                .subtrees
                .push(ImportJournalEntry { target: target.clone(), had_previous: target.exists() });
        }
    }
    write_import_journal(paths, &journal)?;

    // ---- COMMIT. Run inside a closure so ANY `?` failure routes through the SAME
    // in-band rollback below — the canonical set is one commit unit.
    let commit = (|| -> Result<bool> {
        let mut preserved_previous = false;
        for (zip_relative, target) in canonical_dbs {
            let staged = staging.path().join(zip_relative);
            if staged.exists() {
                if install_staged_db_durably(&staged, target, &timestamp)? {
                    preserved_previous = true;
                }
            } else if target.exists() {
                // MEDIUM-F: the bundle omits this canonical DB but the dest has one.
                // Leaving it would pair a (possibly mode-drifted) stale DB with the
                // freshly installed history-vault. Preserve it as a complete `.bak`
                // unit (with sidecars, MEDIUM-C) and remove the live copy, so the
                // next open recreates a fresh empty one in the final-config mode.
                preserve_existing_as_bak(target, &timestamp)?;
                remove_stale_sidecars(target);
                preserved_previous = true;
            }
            // Fires AFTER each canonical install — i.e. BETWEEN the two — so a crash
            // here leaves history-vault swapped but source-evidence not yet.
            crate::fault_inject::checkpoint("import.after_canonical_install")?;
        }

        // A crash HERE leaves both canonical DBs installed but config not yet written.
        crate::fault_inject::checkpoint("import.after_swap_before_config")?;

        // Rebuildable subtrees (derived/audit/raw-snapshots/sidecars).
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

        // HIGH-A: config LAST, ALWAYS written (no skip branch), with `archive_mode`
        // forced to the installed DBs' real at-rest mode — the on-disk bytes, not the
        // bundle metadata, are the authority, so config↔DB drift cannot brick a reopen.
        let final_config = final_archive_config(
            paths,
            &staged_config,
            staging.path().join("config/config.json").exists(),
            config,
        );
        save_config(paths, &final_config)
            .context("persisting the imported config (archive-at-rest mode reconciled)")?;
        Ok(preserved_previous)
    })();

    let preserved_previous = match commit {
        Ok(value) => value,
        Err(error) => {
            // In-band rollback to the consistent pre-import state. If rollback
            // completes, clear the marker; otherwise LEAVE it so
            // `recover_interrupted_import` retries at the next open. Surface the
            // ORIGINAL commit error either way.
            if rollback_import(paths, &journal).is_ok() {
                let _ = remove_file_durably(&import_journal_path(paths));
            }
            return Err(error);
        }
    };

    // ---- COMMIT POINT: config is durably on disk; clearing the marker DURABLY
    // commits the import (a power loss can no longer resurrect it -> recovery no-op).
    remove_file_durably(&import_journal_path(paths))
        .context("clearing the interrupted-import marker")?;

    // A crash HERE survives: the marker is gone, so recovery is a no-op and the new
    // archive + config stand.
    crate::fault_inject::checkpoint("import.after_config")?;

    // Reload the imported config through the normalizing loader so subsequent
    // operations see fresh state (the in-memory `config` is stale once config.json
    // has been replaced under it).
    let imported_config = load_config(paths).unwrap_or_else(|_| config.clone());

    // Run forward migrations on the newly-installed archive (post-commit: a failure
    // here returns Err but does NOT roll back — the import is already committed).
    // Encrypted bundles use the verified source key (Codex C4); plaintext use `None`.
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

    // MEDIUM-D: the import is verified-openable now, so prune older `.bak-<ts>`
    // generations, keeping the current set as the undo-import backstop (best-effort).
    prune_previous_bak_generations(paths, &timestamp);

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

/// Installs one staged canonical database onto `target` durably, treating the
/// database and its `-wal`/`-shm`/`-journal` sidecars as a single unit. Returns
/// `true` when a previous `target` was preserved as a `.bak-<timestamp>` sibling.
///
/// The order is the load-bearing CRIT-3 fix:
///   1. preserve any existing `target` as `.bak-<ts>` (the user's recovery copy),
///   2. SCRUB the target path's pre-existing sidecars — the old database's hot
///      `-wal` (committed-but-uncheckpointed frames left by a prior crash) is now
///      orphaned beside where the NEW database will sit, and SQLite would replay
///      its OLD page frames into the freshly imported canonical database,
///      corrupting it silently (a same-mode plaintext pair) or bricking page 1
///      with NOTADB (a mode-mismatched leftover),
///   3. assert no `<target>-wal` survives the scrub (defence in depth), then
///   4. durably swap the staged copy in (`install_file_durably`: F_FULLFSYNC +
///      atomic rename + dir fsync), and scrub once more so the first open starts
///      from a clean WAL (the staged copy is self-contained from `sqlcipher_export`,
///      so any sidecar that materialised is stale).
///
/// MEDIUM-C: the old database's `.bak` is now a COMPLETE unit — `preserve_existing_as_bak`
/// moves its hot `-wal`/`-shm`/`-journal` alongside the `.bak`, so committed-but-
/// uncheckpointed frames survive in the undo-import backstop instead of being dropped,
/// while the target path is still cleared of those foreign sidecars (CRIT-3). LOW-G:
/// `staged` lives under `app_root/staging` and `target` under `app_root/archive` — the
/// same filesystem by construction — so the `install_file_durably` rename is atomic
/// despite being cross-directory.
fn install_staged_db_durably(staged: &Path, target: &Path, timestamp: &str) -> Result<bool> {
    let preserved = preserve_existing_as_bak(target, timestamp)?.is_some();
    // Treat the DB as a unit: the old file's now-orphaned sidecars must not be
    // replayed against the imported database (CRIT-3).
    remove_stale_sidecars(target);
    ensure_no_foreign_wal(target)?;
    ensure_parent_dir(target)?;
    install_file_durably(staged, target)
        .with_context(|| format!("installing {} into {}", staged.display(), target.display()))?;
    // The staged copy is self-contained; clear any sidecar that appeared so the
    // first open does not inherit a stale WAL (mirrors the rekey swap).
    remove_stale_sidecars(target);
    Ok(preserved)
}

/// Preserves an existing canonical `target` as a sibling `.bak-<timestamp>` file —
/// AND moves the old database's `-wal`/`-shm`/`-journal` sidecars alongside it, so
/// the `.bak` is a COMPLETE, self-consistent copy of the database being replaced.
/// `Ok(None)` when `target` does not exist (a fresh install, nothing to preserve).
///
/// MEDIUM-C: the old database may carry a HOT `-wal` (committed-but-uncheckpointed
/// frames left by a force-quit). Renaming only the main file would strip those
/// frames — the rows the user committed last would silently vanish from the
/// undo-import backstop. Moving the sidecars to `<backup>-wal` etc. keeps them with
/// the `.bak` (so a restore replays them) while ALSO clearing the target path of the
/// old, now-foreign sidecars that must never replay into the freshly installed
/// database (the CRIT-3 hazard `install_staged_db_durably` re-scrubs as defence in
/// depth).
fn preserve_existing_as_bak(target: &Path, timestamp: &str) -> Result<Option<PathBuf>> {
    if !target.exists() {
        return Ok(None);
    }
    let backup = backup_sidecar_path(target, timestamp);
    fs::rename(target, &backup).with_context(|| {
        format!("preserving previous {} as {}", target.display(), backup.display())
    })?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let target_sidecar = PathBuf::from(format!("{}{}", target.display(), suffix));
        if target_sidecar.exists() {
            let backup_sidecar = PathBuf::from(format!("{}{}", backup.display(), suffix));
            fs::rename(&target_sidecar, &backup_sidecar).with_context(|| {
                format!(
                    "preserving previous {} as {}",
                    target_sidecar.display(),
                    backup_sidecar.display()
                )
            })?;
        }
    }
    Ok(Some(backup))
}

/// The path SQLite would treat as `db_path`'s write-ahead log.
fn wal_sidecar_path(db_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-wal", db_path.display()))
}

/// Refuses to install a canonical database while a `<db_path>-wal` still sits
/// beside it. After the scrub in [`install_staged_db_durably`] this always passes;
/// it is a cheap, explicit guard that a FOREIGN write-ahead log can never be left
/// for the post-install open to replay into the freshly imported database (CRIT-3).
fn ensure_no_foreign_wal(db_path: &Path) -> Result<()> {
    let wal = wal_sidecar_path(db_path);
    if wal.exists() {
        anyhow::bail!(
            concat!(
                "refusing to import {}: a foreign write-ahead log {} is still present and would be ",
                "replayed into the imported database",
            ),
            db_path.display(),
            wal.display(),
        );
    }
    Ok(())
}

/// Filename of the durable interrupted-import marker, placed beside the canonical
/// archive database. The `.pk-` prefix keeps retention / cleanup from deleting it
/// (the same dotfile-skip contract the archive write-lock sentinel relies on), so a
/// crash-mid-commit signal can never be swept away before recovery acts on it.
const IMPORT_JOURNAL_FILE: &str = ".pk-import-journal.json";

/// One canonical-DB or subtree path the import is about to replace, plus whether a
/// previous version existed there before the swap — so rollback knows whether to
/// restore a `.bak` or simply delete a fresh install.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportJournalEntry {
    target: PathBuf,
    had_previous: bool,
}

/// The durable record of an in-flight import's commit phase.
///
/// Written (durably) BEFORE the first swap and removed (durably) AFTER the config
/// write. Its mere presence at archive-open time SIGNALS that a crash cut the commit
/// phase; its contents are exactly what [`rollback_import`] needs to restore the
/// consistent pre-import state. The two canonical renames are NOT atomic, so this
/// marker — not the filesystem — is the source of truth for "did the import commit?".
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImportJournal {
    /// Schema version of this journal record (currently `1`).
    version: u32,
    /// The `.bak-<ts>` suffix the commit phase used, so rollback finds the backups.
    timestamp: String,
    /// Canonical databases (history-vault, source-evidence) in install order.
    canonical: Vec<ImportJournalEntry>,
    /// Rebuildable subtrees (derived/audit/raw-snapshots/sidecars).
    subtrees: Vec<ImportJournalEntry>,
    /// The dest's `config.json` bytes pre-import (`None` when none existed), so a
    /// rollback restores the exact prior config rather than guessing.
    previous_config: Option<String>,
}

/// Path of the interrupted-import marker (beside the canonical archive database).
fn import_journal_path(paths: &ProjectPaths) -> PathBuf {
    paths
        .archive_database_path
        .parent()
        .expect("archive database path has a parent directory")
        .join(IMPORT_JOURNAL_FILE)
}

/// Durably writes `journal` to the marker path (atomic temp + F_FULLFSYNC + rename
/// + dir fsync), so its presence is itself durable before the first swap runs.
fn write_import_journal(paths: &ProjectPaths, journal: &ImportJournal) -> Result<()> {
    let bytes =
        serde_json::to_vec(journal).context("serializing the interrupted-import journal")?;
    atomic_durable_write(&import_journal_path(paths), &bytes)
        .context("writing the interrupted-import journal")
}

/// Reads the interrupted-import marker. `Ok(None)` when it is absent OR unparseable.
///
/// A corrupt marker we cannot act on is best-effort removed and treated as absent:
/// the on-disk archive is left exactly as it is (we have no instructions to roll
/// anything back), so a damaged marker can never block opens forever.
fn read_import_journal(paths: &ProjectPaths) -> Result<Option<ImportJournal>> {
    let path = import_journal_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    match serde_json::from_slice::<ImportJournal>(&bytes) {
        Ok(journal) => Ok(Some(journal)),
        Err(_) => {
            let _ = remove_file_durably(&path);
            Ok(None)
        }
    }
}

/// Removes `path` whether it is a file or a directory; a no-op when absent. Shared by
/// [`rollback_import`] to clear a canonical DB file or a rebuildable subtree.
fn remove_path_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("removing directory {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))
    }
}

/// Moves a preserved `.bak` unit's `-wal`/`-shm`/`-journal` sidecars back onto `target`
/// (each only if present at `<bak><suffix>`). Idempotent: a re-run after some sidecars
/// already moved finds those `<bak>` sidecars gone and leaves the already-restored
/// `<target>` ones in place. Used by [`rollback_import`] to restore the OLD database's
/// hot WAL (MEDIUM-C) and to FINISH an interrupted restore.
fn restore_bak_sidecars(bak: &Path, target: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let bak_sidecar = PathBuf::from(format!("{}{}", bak.display(), suffix));
        if bak_sidecar.exists() {
            let target_sidecar = PathBuf::from(format!("{}{}", target.display(), suffix));
            fs::rename(&bak_sidecar, &target_sidecar).with_context(|| {
                format!(
                    "restoring sidecar {} from {}",
                    target_sidecar.display(),
                    bak_sidecar.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Undoes a partially-applied import, restoring the consistent pre-import state.
///
/// The SINGLE undo implementation shared by `apply_import`'s in-band rollback (a
/// returned error mid-commit) and [`recover_interrupted_import`] (a crash mid-commit
/// detected at next open), walking the SAME journal both produce so the two paths can
/// never diverge. Idempotent even across a crash WITHIN recovery: the OLD main is
/// renamed back FIRST and its `.bak` sidecars are moved AFTER, with no barrier between,
/// so a re-run that finds the main `.bak` already consumed but its sidecars still
/// orphaned takes the "orphaned sidecar" arm and just finishes the sidecar move — never
/// dropping a just-restored OLD `-wal`.
fn rollback_import(paths: &ProjectPaths, journal: &ImportJournal) -> Result<()> {
    for entry in journal.canonical.iter().chain(journal.subtrees.iter()) {
        let target = entry.target.as_path();
        let bak = backup_sidecar_path(target, &journal.timestamp);
        let bak_sidecar_present = ["-wal", "-shm", "-journal"]
            .iter()
            .any(|suffix| PathBuf::from(format!("{}{}", bak.display(), suffix)).exists());
        if bak.exists() {
            // The swap had started: scrub any half-installed NEW sidecars first (so a
            // NEW `-wal` cannot replay into the restored OLD main — CRIT-3), drop
            // whatever sits at the target, restore the OLD main, THEN move its `.bak`
            // sidecars across. The main rename is the commit point of the restore.
            remove_stale_sidecars(target);
            remove_path_if_exists(target)?;
            fs::rename(&bak, target).with_context(|| {
                format!("restoring {} from {}", target.display(), bak.display())
            })?;
            restore_bak_sidecars(&bak, target)?;
        } else if bak_sidecar_present {
            // An INTERRUPTED restore: a previous rollback pass already renamed the OLD
            // main back (the main `.bak` is gone) but crashed before moving its
            // sidecars, which are still orphaned at `<bak>-*`. Finish the move — and do
            // NOT scrub here, so the just-restored OLD `<target>-wal` (if some sidecars
            // already moved) is never deleted. This keeps recovery idempotent across a
            // crash WITHIN the restore.
            restore_bak_sidecars(&bak, target)?;
        } else if !entry.had_previous {
            // A fresh install with no prior version: scrub the NEW sidecars (so a NEW
            // `-wal` can't later replay into a fresh-created DB) and remove the NEW main
            // — pre-import there was nothing here.
            remove_stale_sidecars(target);
            remove_path_if_exists(target)?;
        }
        // else (had_previous but no `.bak` unit at all): the swap never started for this
        // entry, so the still-original target — INCLUDING its possibly-hot OLD `-wal`/
        // `-shm`/`-journal` (MEDIUM-C: committed-but-uncheckpointed frames) — is left
        // ENTIRELY untouched. Scrubbing here would silently drop committed rows from a DB
        // we are NOT replacing.
    }
    // FAIL-CLOSED (defense in depth): the "leave untouched" branch above assumes a
    // not-yet-swapped canonical DB is unchanged since the crash. That holds ONLY while no
    // destructive op rewrites the archive in the unrecovered window. If one did (the
    // rekey-on-a-half-import hazard: a plaintext archive skips the encryption-gated launch
    // reconcile, so a rekey can rewrite ONE canonical DB's at-rest mode while the other is
    // restored from a stale-mode `.bak`), the two canonical DBs can end at DIFFERENT modes
    // that no single config can serve. Committing `previous_config` over that mix is a
    // silent config↔source-evidence brick. Detect it here from the REAL on-disk headers and
    // refuse — leaving the marker (the caller keeps it on Err) and surfacing a recoverable
    // error so the state stays flagged + retry-able rather than silently bricked.
    ensure_recovered_modes_are_consistent(paths, journal)?;
    // Restore config to its pre-import bytes (None = none existed pre-import).
    match &journal.previous_config {
        Some(bytes) => atomic_durable_write(&paths.config_path, bytes.as_bytes())
            .context("restoring the pre-import config")?,
        None => {
            let _ = remove_file_durably(&paths.config_path);
        }
    }
    Ok(())
}

/// Maps a configured [`ArchiveMode`] to the on-disk header a file in that mode must
/// present, so a restored canonical DB's real header can be compared to the config the
/// recovery is about to write.
fn disk_mode_for(mode: &ArchiveMode) -> DiskEncryptionMode {
    match mode {
        ArchiveMode::Plaintext => DiskEncryptionMode::Plaintext,
        ArchiveMode::Encrypted => DiskEncryptionMode::Encrypted,
    }
}

/// Fail-closed guard run by [`rollback_import`] AFTER it has restored the canonical
/// databases but BEFORE it commits the recovered config.
///
/// A single `config.json` declares ONE `archive_mode`, so it can correctly serve the
/// archive only if BOTH canonical databases are at that one at-rest mode. This re-reads
/// each canonical DB's ACTUAL on-disk header (`detect_disk_encryption_mode`) and refuses
/// when a PRESENT database disagrees with the mode the recovery is about to commit — the
/// rekey-on-a-half-import signature (history-vault restored to Plaintext while the
/// untouched source-evidence is now Encrypted, or vice versa). The required mode is the
/// pre-import config's when the journal carries one (that IS the config about to be
/// written), else inferred from whichever canonical DB is present. An `Absent` database
/// is fine: the next open recreates it in the final-config mode. Returning `Err` here
/// leaves the marker in place (the caller does not clear it on Err), turning a silent
/// permanent brick into a loud, retry-able failure.
fn ensure_recovered_modes_are_consistent(
    paths: &ProjectPaths,
    journal: &ImportJournal,
) -> Result<()> {
    let history = detect_disk_encryption_mode(&paths.archive_database_path);
    let evidence = detect_disk_encryption_mode(&paths.source_evidence_database_path);
    // The single at-rest mode the recovered archive must converge to.
    let required = journal
        .previous_config
        .as_deref()
        .and_then(|bytes| serde_json::from_str::<AppConfig>(bytes).ok())
        .map(|config| disk_mode_for(&config.archive_mode))
        .or_else(|| {
            [history, evidence].into_iter().find(|mode| *mode != DiskEncryptionMode::Absent)
        });
    let Some(required) = required else {
        // Nothing installed and no config to honour — a clean empty state, nothing to drift.
        return Ok(());
    };
    for (label, mode) in [("history-vault.sqlite", history), ("source-evidence.sqlite", evidence)] {
        if mode != DiskEncryptionMode::Absent && mode != required {
            anyhow::bail!(
                concat!(
                    "interrupted-import recovery cannot reach a single consistent at-rest mode: {} ",
                    "is {:?} but the recovered archive must be {:?}. The canonical databases drifted ",
                    "apart (most likely a rekey ran on a half-applied import before recovery) and no ",
                    "single config can serve both, so the interrupted-import marker is left in place ",
                    "to keep the state flagged and retry-able rather than committing a silently ",
                    "mode-drifted config that would brick the next open.",
                ),
                label,
                mode,
                required,
            );
        }
    }
    Ok(())
}

/// Crash-recovery twin of `apply_import`'s in-band rollback.
///
/// Wired into the unlock-path reconcile (see
/// [`crate::archive::reconcile_archive_encryption`]), it runs BEFORE the archive is
/// opened: when an interrupted-import marker is present a crash cut a commit phase,
/// so it restores the archive to the consistent PRE-IMPORT state over the SAME
/// journal the in-band rollback uses, then clears the marker. Returns `true` only
/// when it actually recovered something.
///
/// Cheap in the common case — a single `stat` of the marker path, no lock taken — so
/// it is safe to call on every open. Only when the marker exists does it take the
/// cross-process [`ArchiveWriteLock`] (recovery rewrites canonical DB files, a
/// destructive archive op that must serialize against a concurrent scheduled backup)
/// and re-read the journal (it may have been cleared while we waited for the lock).
pub(crate) fn recover_interrupted_import(paths: &ProjectPaths) -> Result<bool> {
    if !import_journal_path(paths).exists() {
        return Ok(false);
    }
    let _write_lock = ArchiveWriteLock::acquire(paths)
        .context("acquiring the archive write lock for interrupted-import recovery")?;
    let Some(journal) = read_import_journal(paths)? else {
        return Ok(false);
    };
    rollback_import(paths, &journal)?;
    remove_file_durably(&import_journal_path(paths))
        .context("clearing the interrupted-import marker after recovery")?;
    Ok(true)
}

/// Resolves the config to persist LAST, with its `archive_mode` reconciled to the
/// at-rest mode of the DBs ACTUALLY installed on disk (HIGH-A).
///
/// WHY: the bundle's manifest/config records the SOURCE machine's mode, but the bytes
/// that landed on THIS disk are the only authority for whether the next open needs a
/// key. A config-less bundle installed onto an Encrypted dest would otherwise leave an
/// Encrypted config over plaintext DBs → PRAGMA key on a plaintext header → NOTADB
/// brick. Forcing `archive_mode` to the installed file's real header
/// (`detect_disk_encryption_mode`) makes config↔DB drift impossible.
///
/// Non-archive settings are preserved per-bundle: a bundle that shipped a config
/// starts from it; a config-less bundle starts from the dest's existing config (so the
/// user's local, non-archive preferences survive). `Absent` — no archive installed at
/// all — leaves the chosen base's mode untouched.
fn final_archive_config(
    paths: &ProjectPaths,
    staged_config: &AppConfig,
    staged_config_shipped: bool,
    fallback: &AppConfig,
) -> AppConfig {
    let mut base = if staged_config_shipped {
        staged_config.clone()
    } else {
        load_config(paths).unwrap_or_else(|_| fallback.clone())
    };
    match detect_disk_encryption_mode(&paths.archive_database_path) {
        DiskEncryptionMode::Plaintext => base.archive_mode = ArchiveMode::Plaintext,
        DiskEncryptionMode::Encrypted => base.archive_mode = ArchiveMode::Encrypted,
        DiskEncryptionMode::Absent => {}
    }
    base
}

/// Prunes `.bak-<ts>` generations left by EARLIER imports, keeping only the current
/// import's set as the single "undo import" backstop (MEDIUM-D).
///
/// Retention policy: each import preserves the replaced archive/subtrees as
/// `.bak-<ts>` so the user can undo a wrong import. Without pruning these accumulate
/// one full archive copy per import. Run ONLY after the current import is
/// verified-openable (migrations passed), it removes every `.bak-<ts>` whose suffix is
/// not the current `keep_timestamp`, leaving exactly the just-made backstop.
/// Best-effort: a prune failure must NEVER fail an import that already committed.
fn prune_previous_bak_generations(paths: &ProjectPaths, keep_timestamp: &str) {
    let keep_marker = format!(".bak-{keep_timestamp}");
    let archive_dir =
        paths.archive_database_path.parent().expect("archive database path has a parent directory");
    for dir in [archive_dir, paths.app_root.as_path()] {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.contains(".bak-") && !name.contains(&keep_marker) {
                let path = entry.path();
                if path.is_dir() {
                    let _ = fs::remove_dir_all(&path);
                } else {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
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
        // The agent sidecar (assistant chat transcripts) lives directly under `derived/` but is a
        // privacy-sensitive plane excluded from the export bundle by data-sovereignty default.
        if zip_prefix == "derived"
            && relative.parent().is_none_or(|parent| parent.as_os_str().is_empty())
            && relative
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(is_derived_export_excluded)
        {
            continue;
        }
        // The vector sidecar plane (`derived/vectors/*.pkvec`) is rebuildable derived state and
        // ~59 GB at the 14.4M-row tail (HIGH-4); it never rides the export — re-embed on the target.
        if zip_prefix == "derived" && is_derived_vector_plane_excluded(relative) {
            continue;
        }
        // The raw enrichment blob plane (`sidecars/intelligence-blobs/`) is re-fetchable + possibly
        // large; it never rides the export (06 §3). The capped summary rides in the intelligence DB.
        if zip_prefix == "sidecars" && is_sidecar_enrichment_blob_excluded(relative) {
            continue;
        }
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
    fn stars_survive_export_and_reimport_roundtrip() {
        // Stars are user-authored canonical content that lives in
        // history-vault.sqlite, so they must ride the portable bundle exactly
        // like notes/tags — proven end-to-end here by starring a page +
        // domain, exporting, importing onto a fresh root, and reading both
        // stars back through the public stars API.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);

        crate::stars::set_star(
            &src_paths,
            &config,
            None,
            crate::models::SetStarRequest {
                entity_kind: crate::models::StarEntityKind::Url,
                entity_key: "https://example.com/keepme".into(),
                source_profile: Some("chrome:Default".into()),
            },
        )
        .expect("star a page before export");
        crate::stars::set_star(
            &src_paths,
            &config,
            None,
            crate::models::SetStarRequest {
                entity_kind: crate::models::StarEntityKind::Domain,
                entity_key: "example.com".into(),
                source_profile: None,
            },
        )
        .expect("star a domain before export");

        let bundle_target = src_dir.path().join("bundle.pathkeep");
        export_app_data(&src_paths, &config, None, &bundle_target).expect("export");

        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = AppConfig::default();
        apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_target,
            &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
        )
        .expect("apply import onto fresh root");

        // Both stars must be present in the imported archive.
        let url_status = crate::stars::is_starred_batch(
            &dest_paths,
            &dest_config,
            None,
            crate::models::StarEntityKind::Url,
            &["https://example.com/keepme".to_string()],
        )
        .expect("read url star after import");
        assert_eq!(
            url_status.get("https://example.com/keepme"),
            Some(&true),
            "page star must survive the export/import bundle",
        );

        let counts = crate::stars::star_counts(&dest_paths, &dest_config, None)
            .expect("count stars after import");
        assert_eq!(counts.urls, 1, "exactly the page star survives");
        assert_eq!(counts.domains, 1, "the domain star survives too");
    }

    #[test]
    fn export_excludes_agent_chat_transcripts_from_the_bundle() {
        // Data-sovereignty default: the assistant chat sidecar (`derived/agent.sqlite`) must NOT
        // ride the portable export, even though it sits inside the otherwise-included `derived/`
        // subtree. Its WAL/SHM siblings are excluded too.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);

        // Seed the agent sidecar (and WAL-mode siblings) plus a sibling derived file that MUST ride.
        fs::write(src_paths.agent_database_path.as_path(), b"chat transcripts").unwrap();
        fs::write(src_paths.derived_dir.join("agent.sqlite-wal"), b"wal").unwrap();
        fs::write(src_paths.derived_dir.join("agent.sqlite-shm"), b"shm").unwrap();

        let bundle_target = src_dir.path().join("bundle.pathkeep");
        let bundle = export_app_data(&src_paths, &config, None, &bundle_target).expect("export");

        // The chat sidecar and all its siblings are absent from the manifest…
        for forbidden in
            ["derived/agent.sqlite", "derived/agent.sqlite-wal", "derived/agent.sqlite-shm"]
        {
            assert!(
                !bundle.manifest.files.iter().any(|f| f.path == forbidden),
                "{forbidden} must not be packed into the export bundle: {:?}",
                bundle.manifest.files,
            );
        }
        // …while an ordinary rebuildable derived file still rides.
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "derived/marker.txt"),
            "non-excluded derived files must still be packed",
        );
        // The exclusion is documented for the import-preview surface.
        assert!(
            EXPORT_EXCLUSIONS_DOC.iter().any(|(path, _)| *path == "derived/agent.sqlite"),
            "the agent sidecar exclusion must be documented",
        );
    }

    #[test]
    fn export_excludes_vector_sidecar_plane_from_the_bundle() {
        // HIGH-4: the raw f32 vector plane (`derived/vectors/*.pkvec`) is ~59 GB at the 14.4M tail
        // and rebuildable derived state, so it MUST NOT ride the export — re-embed on the target.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);

        // Seed a `.pkvec` store + its `.pkmap` visit map under derived/vectors, plus a stray `.pkvec`
        // AND a stray `.pkmap` directly under derived/, alongside an ordinary derived file that MUST
        // still ride. (W-AI-4c: the `.pkmap` is rebuildable derived state, excluded like `.pkvec`.)
        fs::create_dir_all(&src_paths.vectors_dir).unwrap();
        fs::write(src_paths.vectors_dir.join("pathkeep_embed_model.pkvec"), b"vectors").unwrap();
        fs::write(src_paths.vectors_dir.join("pathkeep_embed_model.pkmap"), b"visit map").unwrap();
        // W-AI-5 derived recall/rescore planes (also rebuildable; excluded like `.pkvec`/`.pkmap`).
        fs::write(src_paths.vectors_dir.join("pathkeep_embed_model.pkbin"), b"binary plane")
            .unwrap();
        fs::write(src_paths.vectors_dir.join("pathkeep_embed_model.pki8"), b"int8 plane").unwrap();
        // M-11 keyed reverse/forward sidecars (also rebuildable; excluded like the other planes).
        fs::write(src_paths.vectors_dir.join("pathkeep_embed_model.pkrev"), b"reverse sidecar")
            .unwrap();
        fs::write(src_paths.vectors_dir.join("pathkeep_embed_model.pkfwd"), b"forward sidecar")
            .unwrap();
        fs::write(src_paths.derived_dir.join("stray.pkvec"), b"stray vectors").unwrap();
        fs::write(src_paths.derived_dir.join("stray.pkmap"), b"stray map").unwrap();
        fs::write(src_paths.derived_dir.join("stray.pkbin"), b"stray binary").unwrap();
        fs::write(src_paths.derived_dir.join("stray.pki8"), b"stray int8").unwrap();
        fs::write(src_paths.derived_dir.join("stray.pkrev"), b"stray reverse").unwrap();
        fs::write(src_paths.derived_dir.join("stray.pkfwd"), b"stray forward").unwrap();

        let bundle_target = src_dir.path().join("bundle.pathkeep");
        let bundle = export_app_data(&src_paths, &config, None, &bundle_target).expect("export");

        // No `.pkvec` / `.pkmap` (under vectors/ or directly under derived/) is packed.
        assert!(
            !bundle.manifest.files.iter().any(|f| f.path.ends_with(".pkvec")),
            "no .pkvec vector store may be packed: {:?}",
            bundle.manifest.files,
        );
        assert!(
            !bundle.manifest.files.iter().any(|f| f.path.ends_with(".pkmap")),
            "no .pkmap visit map may be packed: {:?}",
            bundle.manifest.files,
        );
        assert!(
            !bundle
                .manifest
                .files
                .iter()
                .any(|f| f.path.ends_with(".pkbin") || f.path.ends_with(".pki8")),
            "no derived recall/rescore plane may be packed: {:?}",
            bundle.manifest.files,
        );
        // M-11: the keyed reverse/forward sidecars are excluded too (under vectors/ or stray).
        assert!(
            !bundle
                .manifest
                .files
                .iter()
                .any(|f| f.path.ends_with(".pkrev") || f.path.ends_with(".pkfwd")),
            "no keyed reverse/forward sidecar may be packed: {:?}",
            bundle.manifest.files,
        );
        assert!(
            !bundle.manifest.files.iter().any(|f| f.path.starts_with("derived/vectors/")),
            "the derived/vectors plane must be excluded entirely",
        );
        // …while an ordinary rebuildable derived file still rides.
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "derived/marker.txt"),
            "non-excluded derived files must still be packed",
        );
        // The exclusion is documented for the import-preview surface.
        assert!(
            EXPORT_EXCLUSIONS_DOC.iter().any(|(path, _)| *path == "derived/vectors/"),
            "the vector plane exclusion must be documented",
        );
    }

    #[test]
    fn export_excludes_raw_enrichment_blobs_but_includes_summary_in_intelligence_db() {
        // 06 §3: the raw enrichment body/caption blobs (`sidecars/intelligence-blobs/`) are
        // re-fetchable + possibly large, so they MUST NOT ride the export. The capped summary lives in
        // the intelligence DB (which DOES ride), so offline search still works on the target.
        let (src_dir, src_paths) = fresh_paths();
        let config = seed_archive(&src_paths);

        // Seed a raw enrichment blob under sidecars/intelligence-blobs and an ordinary sidecar file
        // that MUST still ride.
        fs::create_dir_all(src_paths.intelligence_blobs_dir.join("ab")).unwrap();
        fs::write(src_paths.intelligence_blobs_dir.join("ab/cdef.txt"), b"raw body").unwrap();
        fs::create_dir_all(src_paths.sidecars_dir.join("semantic-index")).unwrap();
        fs::write(src_paths.sidecars_dir.join("semantic-index/keep.txt"), b"keep me").unwrap();

        let bundle_target = src_dir.path().join("bundle.pathkeep");
        let bundle = export_app_data(&src_paths, &config, None, &bundle_target).expect("export");

        // No intelligence-blobs file is packed.
        assert!(
            !bundle
                .manifest
                .files
                .iter()
                .any(|f| f.path.starts_with("sidecars/intelligence-blobs/")),
            "raw enrichment blobs must be excluded: {:?}",
            bundle.manifest.files,
        );
        // …while an ordinary sidecar file still rides.
        assert!(
            bundle.manifest.files.iter().any(|f| f.path == "sidecars/semantic-index/keep.txt"),
            "non-excluded sidecar files must still be packed",
        );
        // The intelligence DB (carrying the capped summary column) DOES ride via the derived/ subtree.
        assert!(
            EXPORT_EXCLUSIONS_DOC.iter().any(|(path, _)| *path == "sidecars/intelligence-blobs/"),
            "the enrichment blob exclusion must be documented",
        );
    }

    #[test]
    fn is_sidecar_enrichment_blob_excluded_matches_only_the_blob_subtree() {
        assert!(is_sidecar_enrichment_blob_excluded(Path::new("intelligence-blobs/ab/cdef.txt")));
        assert!(is_sidecar_enrichment_blob_excluded(Path::new("intelligence-blobs/x.txt")));
        assert!(!is_sidecar_enrichment_blob_excluded(Path::new("semantic-index/keep.txt")));
        assert!(!is_sidecar_enrichment_blob_excluded(Path::new("other.txt")));
    }

    #[test]
    fn is_derived_vector_plane_excluded_matches_vectors_subtree_and_pkvec_and_pkmap() {
        assert!(is_derived_vector_plane_excluded(Path::new("vectors/store.pkvec")));
        assert!(is_derived_vector_plane_excluded(Path::new("vectors/store.pkmap")));
        assert!(is_derived_vector_plane_excluded(Path::new("vectors/store.pkbin")));
        assert!(is_derived_vector_plane_excluded(Path::new("vectors/store.pki8")));
        assert!(is_derived_vector_plane_excluded(Path::new("vectors/store.pkrev")));
        assert!(is_derived_vector_plane_excluded(Path::new("vectors/store.pkfwd")));
        assert!(is_derived_vector_plane_excluded(Path::new("vectors/nested/store.pkvec")));
        assert!(is_derived_vector_plane_excluded(Path::new("stray.pkvec")));
        // Stray derived recall/rescore planes are excluded too (consistency with `.pkvec`/`.pkmap`).
        assert!(is_derived_vector_plane_excluded(Path::new("stray.pkmap")));
        assert!(is_derived_vector_plane_excluded(Path::new("stray.pkbin")));
        assert!(is_derived_vector_plane_excluded(Path::new("stray.pki8")));
        // M-11 keyed reverse/forward sidecars (stray) are excluded too.
        assert!(is_derived_vector_plane_excluded(Path::new("stray.pkrev")));
        assert!(is_derived_vector_plane_excluded(Path::new("stray.pkfwd")));
        // A non-vector derived file is NOT excluded by this rule.
        assert!(!is_derived_vector_plane_excluded(Path::new("marker.txt")));
        assert!(!is_derived_vector_plane_excluded(Path::new("agent.sqlite")));
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
    #[test]
    fn apply_import_propagates_directory_install_rename_error_when_app_root_is_readonly() {
        // Drives the *directory* install loop's `with_context` closure
        // (`installing {staged} into {target}`). Trim every non-derived
        // manifest entry + zip entry so the canonical-DB loop's
        // `staged.exists()` short-circuits each iteration; the dir loop is
        // then the only place left to trip.
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
        // Pre-create staging/ before locking app_root so the import can
        // extract its (single derived/...) entry, and pre-create
        // `app_root/archive/` so the ArchiveWriteLock sentinel can be
        // created there before the swap (the lock lives in archive/, which
        // stays writable while app_root is locked). `app_root/derived/` is
        // intentionally NOT pre-created — `target.exists()` will be false,
        // the preserve branch is skipped, and only the install rename
        // remains to fail under the read-only app_root.
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
