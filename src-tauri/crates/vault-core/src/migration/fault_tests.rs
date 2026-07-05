//! Fault-injection regressions for whole-app import safety.
//!
//! ## Responsibilities
//! - Prove import refusal paths leave the existing project tree untouched.
//! - Pin fail-fast ordering before any `.bak-*` preservation rename runs.
//! - Pin the 2026-06-30 data-integrity fixes for `apply_import`: a prior crash's
//!   FOREIGN `-wal` is scrubbed (never replayed into the imported canonical DB,
//!   CRIT-3); config is written LAST + reconciled to the installed DBs' real
//!   at-rest mode (HIGH-A); the canonical set is one commit unit rolled back from
//!   the `.bak`s on any returned error AND recovered from the durable
//!   interrupted-import marker on a crash (HIGH-B); the old DB's hot `-wal` rides
//!   with its `.bak` (MEDIUM-C); older `.bak-<ts>` generations are pruned
//!   (MEDIUM-D); an omitted canonical DB is preserved+removed, not mode-drifted
//!   (MEDIUM-F).
//! - Unit-cover the DB-unit install helpers (`install_staged_db_durably`,
//!   `preserve_existing_as_bak`, `ensure_no_foreign_wal`) and the
//!   journal/rollback/recovery helpers (`recover_interrupted_import`,
//!   `final_archive_config`, `prune_previous_bak_generations`).
//!
//! - Pin that EVERY destructive pre-open path wired for crash-recovery actually
//!   recover-first: the unlock-path reconcile, the backup pre-open path, the rekey
//!   pre-open path (the rekey-on-a-half-import brick), AND — as of the lock-completion
//!   block — the retention-prune and snapshot-restore pre-open paths, plus the
//!   fail-closed mode-consistency guard that backstops recovery.
//!
//! ## Not responsible for
//! - Full bundle round-trip coverage, which remains in the parent test module.
//! - Scheduler, keyring, or app-lock fault contracts.
//! - The cross-process / in-process LOCK-HOLDING proofs for the destructive ops, which
//!   live beside the other write-lock tests in `archive::tests` and `archive::write_lock`.
//!
//! ## Dependencies
//! - Uses synthetic temp project roots and generated export bundles.
//! - Rewrites zip entries locally to simulate corrupted payload bytes.
//!
//! ## Performance notes
//! - Fixtures contain tiny SQLite archives and one small derived marker file.

use super::*;
use crate::config::{project_paths_with_root, save_config};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

fn fresh_paths() -> (TempDir, ProjectPaths) {
    let dir = TempDir::new().expect("tempdir");
    let paths = project_paths_with_root(dir.path());
    (dir, paths)
}

fn seed_plain_archive(paths: &ProjectPaths, marker: &[u8]) -> AppConfig {
    let config = AppConfig { initialized: true, ..AppConfig::default() };
    fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
        .expect("archive parent dir");
    fs::create_dir_all(&paths.derived_dir).expect("derived dir");
    crate::archive::create_schema(
        &crate::archive::open_archive_connection(paths, &config, None).expect("archive"),
    )
    .expect("schema");
    fs::write(paths.derived_dir.join("marker.txt"), marker).expect("marker");
    save_config(paths, &config).expect("config");
    config
}

fn export_encrypted_bundle(bundle_path: &Path, source_key: &str) {
    let (_src_dir, src_paths) = fresh_paths();
    let encrypted_config = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };
    crate::archive::ensure_archive_initialized(&src_paths, &encrypted_config, Some(source_key))
        .expect("encrypted source archive");
    save_config(&src_paths, &encrypted_config).expect("source config");
    export_app_data(&src_paths, &encrypted_config, Some(source_key), bundle_path)
        .expect("export encrypted bundle");
}

fn assert_no_backup_sidecars(root: &Path) {
    let backups: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.file_name().is_some_and(|name| name.to_string_lossy().contains(".bak-"))
        })
        .collect();
    assert!(backups.is_empty(), "refusal created backup sidecars: {backups:?}");
}

fn rewrite_zip_entry(bundle_path: &Path, updated_name: &str, updated_bytes: &[u8]) {
    let original = fs::read(bundle_path).expect("read bundle");
    let cursor = std::io::Cursor::new(original);
    let mut reader = ZipArchive::new(cursor).expect("read zip");
    let target = File::create(bundle_path).expect("rewrite bundle");
    let mut writer = ZipWriter::new(target);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let names: Vec<String> = (0..reader.len())
        .map(|i| reader.by_index(i).expect("zip entry").name().to_string())
        .collect();

    for name in &names {
        if name == updated_name {
            continue;
        }
        let mut entry = reader.by_name(name).expect("named entry");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read entry");
        writer.start_file(name, options).expect("copy entry");
        writer.write_all(&bytes).expect("write entry");
    }

    writer.start_file(updated_name, options).expect("start tampered entry");
    writer.write_all(updated_bytes).expect("write tampered entry");
    writer.finish().expect("finish zip");
}

#[test]
fn wrong_source_key_refusal_preserves_existing_project_tree() {
    let (src_dir, _) = fresh_paths();
    let bundle_path = src_dir.path().join("encrypted.pathkeep");
    export_encrypted_bundle(&bundle_path, "source-machine-key");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"target marker");

    let err = apply_import(
        &dest_paths,
        &dest_config,
        Some("target-session-key"),
        &bundle_path,
        &ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some("wrong-source-key".to_string()),
        },
    )
    .expect_err("wrong source key must refuse before touching live tree");

    let message = format!("{err:?}");
    assert!(
        message.contains(IMPORT_SOURCE_KEY_INVALID_PREFIX),
        "wrong-key refusal must keep typed copy prefix, got {message}",
    );
    assert_eq!(
        fs::read(dest_paths.derived_dir.join("marker.txt")).expect("marker after refusal"),
        b"target marker",
        "existing derived data must survive wrong-key refusal",
    );
    assert!(
        dest_paths.archive_database_path.exists(),
        "existing archive database must still be installed after wrong-key refusal",
    );
    assert_no_backup_sidecars(&dest_paths.app_root);
}

#[test]
fn payload_hash_mismatch_refusal_preserves_existing_project_tree() {
    let (src_dir, src_paths) = fresh_paths();
    let source_config = seed_plain_archive(&src_paths, b"source marker");
    let bundle_path = src_dir.path().join("tampered.pathkeep");
    export_app_data(&src_paths, &source_config, None, &bundle_path).expect("export bundle");
    rewrite_zip_entry(&bundle_path, "derived/marker.txt", b"counterfeit marker");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"target marker");

    let err = apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect_err("payload mismatch must refuse before touching live tree");

    let message = format!("{err:?}");
    assert!(
        message.contains("sha256 mismatch"),
        "payload mismatch should report hash mismatch, got {message}",
    );
    assert_eq!(
        fs::read(dest_paths.derived_dir.join("marker.txt")).expect("marker after refusal"),
        b"target marker",
        "existing derived data must survive payload mismatch refusal",
    );
    assert!(
        dest_paths.archive_database_path.exists(),
        "existing archive database must still be installed after payload mismatch refusal",
    );
    assert_no_backup_sidecars(&dest_paths.app_root);
}

// --- DB-unit install helpers (CRIT-3 scrub) ------------------------------------------------------

#[test]
fn wal_sidecar_path_appends_the_wal_suffix() {
    assert_eq!(
        wal_sidecar_path(Path::new("/x/history-vault.sqlite")),
        PathBuf::from("/x/history-vault.sqlite-wal"),
    );
}

#[test]
fn ensure_no_foreign_wal_passes_when_clean_and_refuses_a_leftover_wal() {
    let dir = TempDir::new().expect("tempdir");
    let db = dir.path().join("history-vault.sqlite");
    // A clean DB path (no `-wal` beside it) is allowed.
    ensure_no_foreign_wal(&db).expect("a clean DB path must pass");
    // A leftover `-wal` (a prior crash's hot journal) must be refused so SQLite can
    // never replay it into the freshly imported canonical database (CRIT-3).
    fs::write(wal_sidecar_path(&db), b"foreign frames").expect("seed foreign wal");
    let err = ensure_no_foreign_wal(&db).expect_err("a leftover -wal must be refused");
    assert!(
        format!("{err:#}").contains("foreign write-ahead log"),
        "the refusal must name the foreign WAL, got {err:#}",
    );
}

#[test]
fn preserve_existing_as_bak_moves_target_or_reports_none() {
    let dir = TempDir::new().expect("tempdir");
    let target = dir.path().join("history-vault.sqlite");
    // Absent target -> Ok(None); nothing to preserve on a fresh install.
    assert!(preserve_existing_as_bak(&target, "ts").expect("absent ok").is_none());
    // Present target -> renamed to the `.bak-<ts>` sibling, contents preserved.
    fs::write(&target, b"old archive").expect("seed target");
    let backup = preserve_existing_as_bak(&target, "ts").expect("preserve").expect("a backup path");
    assert!(!target.exists(), "the target must have moved to its .bak sibling");
    assert_eq!(fs::read(&backup).expect("read bak"), b"old archive");
}

#[cfg(unix)]
#[test]
fn preserve_existing_as_bak_surfaces_a_rename_failure() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().expect("tempdir");
    let parent = dir.path().join("p");
    fs::create_dir(&parent).expect("mkdir parent");
    let target = parent.join("history-vault.sqlite");
    fs::write(&target, b"x").expect("seed target");

    let original = fs::metadata(&parent).expect("perms").permissions();
    let mut locked = original.clone();
    locked.set_mode(0o500);
    fs::set_permissions(&parent, locked).expect("lock parent");
    let result = preserve_existing_as_bak(&target, "ts");
    fs::set_permissions(&parent, original).expect("restore perms");

    let err = result.expect_err("a readonly parent must fail the preserve rename");
    assert!(format!("{err:#}").contains("preserving previous"), "got {err:#}");
}

#[cfg(unix)]
#[test]
fn install_staged_db_durably_scrubs_a_foreign_wal_then_surfaces_install_failure() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().expect("tempdir");
    let staging = dir.path().join("staging");
    fs::create_dir(&staging).expect("mkdir staging");

    // Happy path: a fresh target with a pre-existing FOREIGN `-wal` is installed
    // durably; the foreign sidecar is scrubbed and never left beside the new DB.
    let archive_dir = dir.path().join("archive");
    fs::create_dir(&archive_dir).expect("mkdir archive");
    let target = archive_dir.join("history-vault.sqlite");
    fs::write(wal_sidecar_path(&target), b"foreign frames").expect("seed foreign wal");
    let staged = staging.join("history-vault.sqlite");
    fs::write(&staged, b"imported db bytes").expect("seed staged");
    let preserved = install_staged_db_durably(&staged, &target, "ts").expect("install");
    assert!(!preserved, "a fresh target preserves nothing");
    assert_eq!(fs::read(&target).expect("read installed"), b"imported db bytes");
    assert!(
        !wal_sidecar_path(&target).exists(),
        "the foreign -wal must be scrubbed, never installed beside the new DB",
    );

    // Install failure: a readonly target directory makes the durable rename fail,
    // surfacing the `installing ... into ...` context.
    let locked_dir = dir.path().join("locked");
    fs::create_dir(&locked_dir).expect("mkdir locked");
    let locked_target = locked_dir.join("history-vault.sqlite");
    let staged2 = staging.join("second.sqlite");
    fs::write(&staged2, b"second").expect("seed second staged");
    let original = fs::metadata(&locked_dir).expect("perms").permissions();
    let mut locked = original.clone();
    locked.set_mode(0o500);
    fs::set_permissions(&locked_dir, locked).expect("lock dir");
    let result = install_staged_db_durably(&staged2, &locked_target, "ts");
    fs::set_permissions(&locked_dir, original).expect("restore perms");
    let err = result.expect_err("a readonly target dir must fail the durable install");
    assert!(format!("{err:#}").contains("installing"), "got {err:#}");
}

// --- CRIT-3: foreign WAL is scrubbed, never replayed into the imported archive -------------------

#[test]
fn apply_import_scrubs_a_foreign_target_wal_rather_than_replaying_it() {
    // The bundle (archive B) carries a NEW star as its canonical content.
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    crate::stars::set_star(
        &src_paths,
        &src_config,
        None,
        crate::models::SetStarRequest {
            entity_kind: crate::models::StarEntityKind::Url,
            entity_key: "https://new.example/keep".into(),
            source_profile: Some("chrome:Default".into()),
        },
    )
    .expect("star the NEW page before export");
    let bundle_path = src_dir.path().join("foreign-wal.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export bundle");

    // The dest (archive A) has a hot, uncheckpointed `-wal` carrying an OLD sentinel
    // row that lives ONLY in the WAL — exactly a prior crash's leftover hot journal.
    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    {
        let conn =
            rusqlite::Connection::open(&dest_paths.archive_database_path).expect("open dest");
        conn.pragma_update(None, "journal_mode", "WAL").expect("wal mode");
        conn.pragma_update(None, "wal_autocheckpoint", 0i64).expect("no autocheckpoint");
        conn.execute(
            "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'OLD_WAL_SENTINEL', '2020-01-01T00:00:00Z', 'UTC', 'running', '[]', '[]', '{}', 0)",
            [],
        )
        .expect("stage the OLD sentinel in the WAL");
        // Leak the connection so close() never checkpoints — the OLD frames stay in
        // the `-wal`, reproducing a crash/force-quit that left a hot journal.
        std::mem::forget(conn);
    }
    let foreign_wal = wal_sidecar_path(&dest_paths.archive_database_path);
    assert!(
        foreign_wal.exists() && fs::metadata(&foreign_wal).expect("stat wal").len() > 0,
        "the hot WAL must carry the OLD sentinel frames before import",
    );

    apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect("import must succeed with the foreign WAL scrubbed, not replayed");

    // The imported archive is the bundle's, intact, with the foreign WAL NOT replayed:
    // integrity is ok, the OLD sentinel is absent, and the NEW star is present.
    let imported = crate::archive::open_archive_connection(&dest_paths, &dest_config, None)
        .expect("open the imported archive");
    let integrity: String =
        imported.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok", "the imported archive must be intact (no foreign WAL replay)");
    let old_rows: i64 = imported
        .query_row("SELECT COUNT(*) FROM runs WHERE trigger = 'OLD_WAL_SENTINEL'", [], |row| {
            row.get(0)
        })
        .expect("count old sentinel rows");
    assert_eq!(old_rows, 0, "the foreign WAL's OLD frames must NOT be replayed into the import");
    drop(imported);

    let new_star = crate::stars::is_starred_batch(
        &dest_paths,
        &dest_config,
        None,
        crate::models::StarEntityKind::Url,
        &["https://new.example/keep".to_string()],
    )
    .expect("read the imported star");
    assert_eq!(
        new_star.get("https://new.example/keep"),
        Some(&true),
        "the imported bundle's canonical content must be present after import",
    );
}

// --- crash windows: lock + config-LAST keep every half-state recoverable ------------------------

#[test]
fn apply_import_crash_after_stage_before_swap_leaves_live_tree_untouched() {
    // Window 1: a crash AFTER the staged DBs are built + verified but BEFORE any
    // swap must be a full no-op — the live archive + config + derived data are
    // untouched and no `.bak` was created.
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"bundle marker");
    let bundle_path = src_dir.path().join("crash-stage.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");

    let _guard =
        crate::fault_inject::FaultGuard::error_at_must_fire("import.after_stage_before_swap");
    let err = apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect_err("a crash before the swap must abort the import");

    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("import.after_stage_before_swap"),
        "the INJECTED fault must propagate, got: {rendered}",
    );
    assert_eq!(
        fs::read(dest_paths.derived_dir.join("marker.txt")).expect("dest marker"),
        b"dest marker",
        "the live derived data must survive a pre-swap crash",
    );
    assert!(dest_paths.archive_database_path.exists(), "the live archive must be untouched");
    assert_no_backup_sidecars(&dest_paths.app_root);
}

#[test]
fn apply_import_returned_error_after_swap_rolls_back_to_pre_import() {
    // HIGH-B: a RETURNED error after both canonical DBs are swapped but before the
    // config write must roll the whole commit unit back to the consistent pre-import
    // state from the `.bak`s — no half-applied mixed archive, no stale marker. The
    // bundle is encrypted; the dest is plaintext, so a leaked swap would be obvious.
    let source_key = "import-rollback-after-swap-source-key";
    let (src_dir, _src_paths) = fresh_paths();
    let bundle_path = src_dir.path().join("rollback-after-swap.pathkeep");
    export_encrypted_bundle(&bundle_path, source_key);

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");

    let _guard =
        crate::fault_inject::FaultGuard::error_at_must_fire("import.after_swap_before_config");
    let err = apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(source_key.to_string()),
        },
    )
    .expect_err("a returned error after the swap must roll back to pre-import");

    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("import.after_swap_before_config"),
        "the INJECTED fault must propagate, got: {rendered}",
    );

    // The dest's ORIGINAL plaintext archive is restored: it opens as plaintext with
    // no key (the encrypted bundle's content is gone — it would NOTADB as plaintext).
    let restored =
        crate::archive::open_archive_connection(&dest_paths, &dest_config, None).expect("open");
    let integrity: String =
        restored.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok", "the original plaintext archive must be restored intact");
    drop(restored);
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.archive_database_path),
        DiskEncryptionMode::Plaintext,
        "the imported encrypted bundle's content must be ABSENT (original restored)",
    );

    // Config reads the ORIGINAL (plaintext) mode, no `.bak-` remains, no marker remains.
    assert!(matches!(
        crate::config::load_config(&dest_paths).expect("load config").archive_mode,
        ArchiveMode::Plaintext
    ));
    assert_no_backup_sidecars(&dest_paths.app_root);
    assert!(
        !import_journal_path(&dest_paths).exists(),
        "a successful in-band rollback must clear the interrupted-import marker",
    );
}

#[test]
fn apply_import_crash_after_config_has_consistent_new_archive_and_config() {
    // Window 3: a crash AFTER the config write (before closeout) leaves config AND
    // the on-disk archive BOTH at the new (encrypted) mode — a consistent, openable
    // state. Migrations may not have finished, but they are idempotent + re-runnable.
    let source_key = "import-crash-after-config-key";
    let (src_dir, _src_paths) = fresh_paths();
    let bundle_path = src_dir.path().join("crash-after-config.pathkeep");
    export_encrypted_bundle(&bundle_path, source_key);

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");

    let _guard = crate::fault_inject::FaultGuard::error_at_must_fire("import.after_config");
    let err = apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(source_key.to_string()),
        },
    )
    .expect_err("a crash after the config write must abort closeout");

    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("import.after_config"),
        "the INJECTED fault must propagate, got: {rendered}",
    );

    // Config AND archive are both at the new (encrypted) mode — consistent.
    assert!(
        matches!(
            crate::config::load_config(&dest_paths).expect("load config").archive_mode,
            ArchiveMode::Encrypted
        ),
        "config must read the NEW encrypted mode after a crash past the config write",
    );
    let connection =
        rusqlite::Connection::open(&dest_paths.archive_database_path).expect("open new archive");
    apply_cipher_key(&connection, source_key).expect("apply source key");
    let integrity: String =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok", "the encrypted archive must be intact and consistent with config");
    // The marker was already removed at the commit point (before this checkpoint), so the
    // import is COMMITTED — recovery is a no-op, NOT rollback-eligible.
    assert!(
        !import_journal_path(&dest_paths).exists(),
        "the interrupted-import marker must be gone after the commit point (crash here keeps the new state)",
    );
}

// --- config-less bundle: dest config preserved + reconciled (HIGH-A) ----------------------------

#[test]
fn apply_import_without_a_bundle_config_leaves_target_config_and_still_imports() {
    // A config-less bundle (older/minimal exports) must import while preserving the
    // dest's non-archive settings: HIGH-A starts from the dest's existing config and
    // forces `archive_mode` to the installed (plaintext) DB, so the target keeps a
    // valid plaintext config. The archive still installs and migrations still run.
    let (src_dir, src_paths) = fresh_paths();
    let config = AppConfig::default();
    fs::create_dir_all(src_paths.archive_database_path.parent().expect("archive parent"))
        .expect("archive parent dir");
    fs::create_dir_all(&src_paths.derived_dir).expect("derived dir");
    crate::archive::create_schema(
        &crate::archive::open_archive_connection(&src_paths, &config, None).expect("archive"),
    )
    .expect("schema");
    fs::write(src_paths.derived_dir.join("marker.txt"), b"src").expect("marker");
    // No config saved on the source, so the bundle ships no config/config.json.
    let _ = fs::remove_file(&src_paths.config_path);
    let _ = fs::remove_file(&src_paths.source_evidence_database_path);
    let bundle_path = src_dir.path().join("no-config.pathkeep");
    export_app_data(&src_paths, &config, None, &bundle_path).expect("export config-less bundle");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    let result = apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect("a config-less bundle must still import");
    assert!(result.final_schema_version > 0, "the imported archive must resolve a schema version");
    // The target keeps a valid (plaintext) config since the bundle shipped none.
    assert!(matches!(
        crate::config::load_config(&dest_paths).expect("load config").archive_mode,
        ArchiveMode::Plaintext
    ));
}

// --- shared helpers for the HIGH/MEDIUM data-integrity tests -------------------------------------

/// Stars a URL in `paths`'s archive (the distinguishable canonical content the
/// rollback / recovery tests assert survives or is excluded).
fn star_url(paths: &ProjectPaths, config: &AppConfig, url: &str) {
    crate::stars::set_star(
        paths,
        config,
        None,
        crate::models::SetStarRequest {
            entity_kind: crate::models::StarEntityKind::Url,
            entity_key: url.into(),
            source_profile: Some("chrome:Default".into()),
        },
    )
    .expect("set star");
}

/// Reads whether `url` is starred in `paths`'s archive.
fn url_is_starred(paths: &ProjectPaths, config: &AppConfig, url: &str) -> bool {
    crate::stars::is_starred_batch(
        paths,
        config,
        None,
        crate::models::StarEntityKind::Url,
        &[url.to_string()],
    )
    .expect("read star")
    .get(url)
    .copied()
    .unwrap_or(false)
}

/// Exports an ENCRYPTED bundle that carries history-vault but OMITS source-evidence
/// (the MEDIUM-F shape): the source-evidence file is removed before the export walk.
fn export_encrypted_bundle_without_source_evidence(bundle_path: &Path, source_key: &str) {
    let (_src_dir, src_paths) = fresh_paths();
    let encrypted_config = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };
    crate::archive::ensure_archive_initialized(&src_paths, &encrypted_config, Some(source_key))
        .expect("encrypted source archive");
    // Drop the source-evidence DB + sidecars so the bundle ships none.
    let _ = fs::remove_file(&src_paths.source_evidence_database_path);
    remove_stale_sidecars(&src_paths.source_evidence_database_path);
    save_config(&src_paths, &encrypted_config).expect("source config");
    export_app_data(&src_paths, &encrypted_config, Some(source_key), bundle_path)
        .expect("export encrypted bundle without source-evidence");
}

// --- HIGH-A: config archive_mode reconciled to the installed DBs' at-rest mode -------------------

#[test]
fn apply_import_configless_bundle_onto_encrypted_dest_reconciles_config_to_installed_mode() {
    // A config-less PLAINTEXT bundle imported onto an ENCRYPTED dest must NOT leave the
    // dest's Encrypted config over the freshly installed plaintext DBs (the NOTADB
    // brick). HIGH-A forces config's `archive_mode` to the installed file's real header.
    let (src_dir, src_paths) = fresh_paths();
    let config = AppConfig::default();
    fs::create_dir_all(src_paths.archive_database_path.parent().expect("archive parent"))
        .expect("archive parent dir");
    fs::create_dir_all(&src_paths.derived_dir).expect("derived dir");
    crate::archive::create_schema(
        &crate::archive::open_archive_connection(&src_paths, &config, None).expect("archive"),
    )
    .expect("schema");
    fs::write(src_paths.derived_dir.join("marker.txt"), b"src").expect("marker");
    // Ship no config (config-less bundle).
    let _ = fs::remove_file(&src_paths.config_path);
    let _ = fs::remove_file(&src_paths.source_evidence_database_path);
    let bundle_path = src_dir.path().join("configless-plaintext.pathkeep");
    export_app_data(&src_paths, &config, None, &bundle_path)
        .expect("export config-less plaintext bundle");

    // ENCRYPTED dest (archive + config), with a key.
    let dest_key = "encrypted-dest-key";
    let (_dest_dir, dest_paths) = fresh_paths();
    let encrypted_config = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };
    crate::archive::ensure_archive_initialized(&dest_paths, &encrypted_config, Some(dest_key))
        .expect("init encrypted dest");
    save_config(&dest_paths, &encrypted_config).expect("save encrypted dest config");
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.archive_database_path),
        DiskEncryptionMode::Encrypted
    );

    apply_import(
        &dest_paths,
        &encrypted_config,
        Some(dest_key),
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect("config-less plaintext bundle onto encrypted dest must import");

    // Config reconciled to the installed (plaintext) DB — not the dest/bundle metadata.
    assert!(
        matches!(
            crate::config::load_config(&dest_paths).expect("load config").archive_mode,
            ArchiveMode::Plaintext
        ),
        "config must be reconciled to the installed plaintext archive",
    );
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.archive_database_path),
        DiskEncryptionMode::Plaintext
    );
    // No brick: the archive opens as plaintext with NO key.
    let plaintext_config = AppConfig::default();
    let connection = crate::archive::open_archive_connection(&dest_paths, &plaintext_config, None)
        .expect("open reconciled plaintext archive without a key");
    let integrity: String =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok");
}

#[test]
fn final_archive_config_forces_plaintext_when_installed_db_is_plaintext() {
    let (_dir, paths) = fresh_paths();
    fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
        .expect("archive dir");
    fs::write(&paths.archive_database_path, b"SQLite format 3\0plaintext header").expect("seed db");
    // Shipped config CLAIMS encrypted, but the installed bytes are plaintext.
    let shipped = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };
    let resolved = final_archive_config(&paths, &shipped, true, &AppConfig::default());
    assert!(matches!(resolved.archive_mode, ArchiveMode::Plaintext), "mode forced to plaintext");
    assert!(resolved.initialized, "non-archive settings from the shipped config survive");
}

#[test]
fn final_archive_config_forces_encrypted_from_dest_config_when_bundle_ships_none() {
    let (_dir, paths) = fresh_paths();
    fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
        .expect("archive dir");
    // Installed DB has a non-plaintext header (SQLCipher salt) => Encrypted.
    fs::write(&paths.archive_database_path, [7u8; 32]).expect("seed encrypted-looking db");
    // Bundle ships no config => base comes from the dest's saved config.
    let dest_config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        ..AppConfig::default()
    };
    save_config(&paths, &dest_config).expect("save dest config");
    let resolved =
        final_archive_config(&paths, &AppConfig::default(), false, &AppConfig::default());
    assert!(matches!(resolved.archive_mode, ArchiveMode::Encrypted), "mode forced to encrypted");
    assert!(resolved.initialized, "dest's non-archive settings survive a config-less bundle");
}

#[test]
fn final_archive_config_absent_archive_falls_back_and_leaves_mode_untouched() {
    let (_dir, paths) = fresh_paths();
    // No archive file installed (Absent) AND an unreadable dest config, so the
    // `load_config` fallback closure runs and the Absent arm leaves the mode as-is.
    fs::create_dir_all(paths.config_path.parent().expect("config parent")).expect("config dir");
    fs::write(&paths.config_path, b"{not valid json").expect("seed invalid config");
    let fallback = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };
    let resolved = final_archive_config(&paths, &AppConfig::default(), false, &fallback);
    assert!(
        matches!(resolved.archive_mode, ArchiveMode::Encrypted),
        "Absent archive must leave the fallback base's mode untouched",
    );
    assert!(resolved.initialized, "the fallback base is used when load_config fails");
}

// --- HIGH-B: the canonical set is one commit unit (crash -> recover, error -> rollback) ----------

#[test]
fn apply_import_crash_between_canonical_installs_is_recovered_not_silently_mixed() {
    use std::panic::{self, AssertUnwindSafe};

    // Bundle: plaintext, carrying a NEW star as distinguishable canonical content.
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    star_url(&src_paths, &src_config, "https://new.example/keep");
    let bundle_path = src_dir.path().join("crash-mid.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    // Dest: plaintext, with a distinguishable OLD star.
    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    star_url(&dest_paths, &dest_config, "https://old.example/keep");

    // A PANIC after the FIRST canonical install (history-vault swapped, source-evidence
    // not yet) unwinds PAST the in-band rollback, leaving the marker + half-state.
    crate::fault_inject::arm_panic_at("import.after_canonical_install");
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        )
    }));
    crate::fault_inject::disarm("import.after_canonical_install");
    assert!(outcome.is_err(), "the panic between canonical installs must unwind");

    // The durable marker + half-state survive the unwind (in-band rollback was skipped).
    assert!(
        import_journal_path(&dest_paths).exists(),
        "a crash mid-commit must leave the interrupted-import marker as the crash signal",
    );

    // Crash-recovery (the twin of the in-band rollback) restores the consistent state.
    let recovered = recover_interrupted_import(&dest_paths).expect("recovery must not error");
    assert!(recovered, "a present marker must drive a recovery");
    assert!(!import_journal_path(&dest_paths).exists(), "recovery must clear the marker");

    // The OLD archive is restored: integrity ok, OLD content present, NEW content absent —
    // never a silently-mixed "new history-vault + old source-evidence".
    let restored = crate::archive::open_archive_connection(&dest_paths, &dest_config, None)
        .expect("open restored archive");
    let integrity: String =
        restored.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok");
    drop(restored);
    assert!(
        url_is_starred(&dest_paths, &dest_config, "https://old.example/keep"),
        "the OLD content must be restored by recovery",
    );
    assert!(
        !url_is_starred(&dest_paths, &dest_config, "https://new.example/keep"),
        "the NEW bundle content must NOT have leaked in",
    );
}

#[test]
fn apply_import_returned_error_during_canonical_commit_rolls_back_db1() {
    // Bundle: plaintext, NEW star. Dest: plaintext, distinguishable OLD star.
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    star_url(&src_paths, &src_config, "https://new.example/keep");
    let bundle_path = src_dir.path().join("rollback-db1.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    star_url(&dest_paths, &dest_config, "https://old.example/keep");

    // The error returns AFTER history-vault is installed but BEFORE source-evidence,
    // so the in-band rollback restores history-vault (db1) from its `.bak`.
    let _guard =
        crate::fault_inject::FaultGuard::error_at_must_fire("import.after_canonical_install");
    let err = apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect_err("a returned error mid canonical commit must roll back");

    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("import.after_canonical_install"),
        "the INJECTED fault must propagate, got: {rendered}",
    );

    // Rolled back: marker gone, no `.bak`, OLD content present, NEW absent, integrity ok.
    assert!(!import_journal_path(&dest_paths).exists(), "rollback must clear the marker");
    assert_no_backup_sidecars(&dest_paths.app_root);
    let restored = crate::archive::open_archive_connection(&dest_paths, &dest_config, None)
        .expect("open restored archive");
    let integrity: String =
        restored.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok");
    drop(restored);
    assert!(url_is_starred(&dest_paths, &dest_config, "https://old.example/keep"), "OLD restored");
    assert!(
        !url_is_starred(&dest_paths, &dest_config, "https://new.example/keep"),
        "NEW must not leak",
    );
    assert!(matches!(
        crate::config::load_config(&dest_paths).expect("load config").archive_mode,
        ArchiveMode::Plaintext
    ));
}

#[test]
fn apply_import_rollback_preserves_a_not_yet_swapped_canonical_dbs_hot_wal() {
    // Regression: a returned error BETWEEN the two canonical installs must NOT strip the
    // OLD, not-yet-swapped source-evidence DB's hot WAL. Its committed-but-uncheckpointed
    // frames must survive the rollback's "leave untouched" branch (MEDIUM-C extends to
    // rollback, not just preserve). Pre-fix, `rollback_import` scrubbed every target's
    // sidecars unconditionally and silently dropped these committed rows.
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    let bundle_path = src_dir.path().join("rollback-se-wal.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    // The dest's source-evidence carries a committed-only-in-WAL sentinel (a prior
    // force-quit's hot journal). The bundle omits source-evidence, and the error fires
    // AFTER history-vault but BEFORE source-evidence would be touched, so the rollback's
    // source-evidence entry takes the "had_previous, no .bak -> leave untouched" branch.
    {
        let conn = crate::archive::open_source_evidence_connection(&dest_paths, &dest_config, None)
            .expect("seed dest source-evidence");
        conn.pragma_update(None, "journal_mode", "WAL").expect("wal mode");
        conn.pragma_update(None, "wal_autocheckpoint", 0i64).expect("no autocheckpoint");
        conn.execute(
            "INSERT INTO source_batches (
               source_profile_id, run_id, source_kind, browser_version,
               schema_version_text, schema_version_int, schema_fingerprint,
               parser_version, capability_snapshot_json, coverage_stats_json,
               artifact_refs_json, notes_json, created_at)
             VALUES (1, NULL, 'chromium', NULL, NULL, NULL, 'HOT_WAL_SE_SENTINEL', 'pv', '{}', '{}', NULL, NULL, '2026-06-28T00:00:00Z')",
            [],
        )
        .expect("stage the source-evidence sentinel in the WAL");
        // Leak the connection so close() never checkpoints — the frames stay in `-wal`.
        std::mem::forget(conn);
    }
    let se_wal = wal_sidecar_path(&dest_paths.source_evidence_database_path);
    assert!(
        se_wal.exists() && fs::metadata(&se_wal).expect("stat se wal").len() > 0,
        "the source-evidence hot WAL must carry the sentinel before import",
    );

    let _guard =
        crate::fault_inject::FaultGuard::error_at_must_fire("import.after_canonical_install");
    apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect_err("a returned error after history-vault must roll back");

    // The rollback left source-evidence (never swapped) ENTIRELY untouched, including its
    // hot WAL: opening it replays the WAL and the committed sentinel survives.
    let se = crate::archive::open_source_evidence_connection(&dest_paths, &dest_config, None)
        .expect("open restored source-evidence");
    let preserved: i64 = se
        .query_row(
            "SELECT COUNT(*) FROM source_batches WHERE schema_fingerprint = 'HOT_WAL_SE_SENTINEL'",
            [],
            |row| row.get(0),
        )
        .expect("count the sentinel");
    assert_eq!(
        preserved, 1,
        "a not-yet-swapped DB's committed-only-in-WAL frames must survive the rollback",
    );
}

// --- MEDIUM-C: the old DB's hot WAL rides with its `.bak` unit -----------------------------------

#[test]
fn apply_import_preserves_old_hot_wal_in_the_bak_unit() {
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"bundle marker");
    let bundle_path = src_dir.path().join("hot-wal-bak.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    // Leave a committed-only-in-WAL sentinel on the dest archive (a force-quit's hot WAL).
    {
        let conn =
            rusqlite::Connection::open(&dest_paths.archive_database_path).expect("open dest");
        conn.pragma_update(None, "journal_mode", "WAL").expect("wal mode");
        conn.pragma_update(None, "wal_autocheckpoint", 0i64).expect("no autocheckpoint");
        conn.execute(
            "INSERT INTO runs (run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only)
             VALUES ('backup', 'HOT_WAL_BAK_SENTINEL', '2020-01-01T00:00:00Z', 'UTC', 'running', '[]', '[]', '{}', 0)",
            [],
        )
        .expect("stage the sentinel in the WAL");
        std::mem::forget(conn);
    }
    let dest_wal = wal_sidecar_path(&dest_paths.archive_database_path);
    assert!(
        dest_wal.exists() && fs::metadata(&dest_wal).expect("stat wal").len() > 0,
        "the hot WAL must carry the sentinel before import",
    );

    apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect("import");

    // Locate the preserved `.bak` UNIT (main file, not a sidecar).
    let archive_dir = dest_paths.archive_database_path.parent().expect("archive dir");
    let bak: PathBuf = fs::read_dir(archive_dir)
        .expect("read archive dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            name.starts_with("history-vault.sqlite.bak-")
                && !name.ends_with("-wal")
                && !name.ends_with("-shm")
                && !name.ends_with("-journal")
        })
        .expect("a history-vault .bak unit must be preserved");

    // MEDIUM-C: the old DB's hot `-wal` rode ALONGSIDE the `.bak`.
    let bak_wal = PathBuf::from(format!("{}-wal", bak.display()));
    assert!(bak_wal.exists(), "the old DB's hot -wal must be preserved beside the .bak unit");

    // Opening the `.bak` replays its WAL: the committed-only-in-WAL sentinel survives.
    let bak_conn = rusqlite::Connection::open(&bak).expect("open the preserved .bak");
    let preserved: i64 = bak_conn
        .query_row("SELECT COUNT(*) FROM runs WHERE trigger = 'HOT_WAL_BAK_SENTINEL'", [], |row| {
            row.get(0)
        })
        .expect("count");
    assert_eq!(
        preserved, 1,
        "the old DB's committed-but-uncheckpointed frames must survive in the .bak unit",
    );
}

#[cfg(unix)]
#[test]
fn preserve_existing_as_bak_surfaces_a_sidecar_rename_failure() {
    // After the main rename succeeds, a sidecar rename onto an existing DIRECTORY at
    // the bak-sidecar path fails (EISDIR), surfacing the `preserving previous` context.
    let dir = TempDir::new().expect("tempdir");
    let target = dir.path().join("history-vault.sqlite");
    fs::write(&target, b"db").expect("seed target");
    fs::write(wal_sidecar_path(&target), b"hot wal").expect("seed target wal");

    let bak = backup_sidecar_path(&target, "ts");
    let bak_wal = PathBuf::from(format!("{}-wal", bak.display()));
    fs::create_dir(&bak_wal).expect("seed blocking dir at the bak-wal path");
    fs::write(bak_wal.join("inner"), b"x").expect("seed blocking child");

    let err =
        preserve_existing_as_bak(&target, "ts").expect_err("a sidecar rename failure must surface");
    assert!(format!("{err:#}").contains("preserving previous"), "got {err:#}");
}

// --- MEDIUM-D: bound `.bak-<ts>` growth ---------------------------------------------------------

#[test]
fn apply_import_prunes_older_bak_generations_keeping_the_current() {
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"bundle marker");
    let bundle_path = src_dir.path().join("prune-bak.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    let archive_dir = dest_paths.archive_database_path.parent().expect("archive dir").to_path_buf();
    // A stale db `.bak` file (archive dir) AND a stale subtree `.bak` dir (app_root).
    let stale_db_bak = archive_dir.join("history-vault.sqlite.bak-2020-01-01T00-00-00Z");
    fs::write(&stale_db_bak, b"stale bak").expect("seed stale db bak");
    let stale_dir_bak = dest_paths.app_root.join("derived.bak-2020-01-01T00-00-00Z");
    fs::create_dir_all(&stale_dir_bak).expect("seed stale dir bak");
    fs::write(stale_dir_bak.join("old.txt"), b"stale").expect("seed stale dir content");

    apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    )
    .expect("import");

    assert!(!stale_db_bak.exists(), "the older db .bak generation must be pruned");
    assert!(!stale_dir_bak.exists(), "the older subtree .bak generation must be pruned");

    // Exactly the current-timestamp db `.bak` remains as the undo backstop.
    let db_baks: Vec<String> = fs::read_dir(&archive_dir)
        .expect("read archive dir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("history-vault.sqlite.bak-"))
        .collect();
    assert!(!db_baks.is_empty(), "the current import's db .bak must remain as the undo backstop");
    assert!(
        !db_baks.iter().any(|name| name.contains("2020-01-01")),
        "no stale-timestamp db .bak may remain: {db_baks:?}",
    );
}

#[test]
fn prune_previous_bak_generations_skips_a_missing_archive_dir_and_keeps_current() {
    let (_dir, paths) = fresh_paths();
    // No archive dir yet: the read_dir miss must `continue`, not panic.
    prune_previous_bak_generations(&paths, "current");

    // Now seed both dirs with a stale + a current generation and prune again.
    let archive_dir = paths.archive_database_path.parent().expect("archive parent").to_path_buf();
    fs::create_dir_all(&archive_dir).expect("archive dir");
    fs::write(archive_dir.join("history-vault.sqlite.bak-stale"), b"old").expect("stale file");
    fs::write(archive_dir.join("history-vault.sqlite.bak-current"), b"new").expect("current file");
    let stale_dir = paths.app_root.join("derived.bak-stale");
    fs::create_dir_all(&stale_dir).expect("stale dir");
    let current_dir = paths.app_root.join("derived.bak-current");
    fs::create_dir_all(&current_dir).expect("current dir");

    prune_previous_bak_generations(&paths, "current");

    assert!(!archive_dir.join("history-vault.sqlite.bak-stale").exists(), "stale file pruned");
    assert!(archive_dir.join("history-vault.sqlite.bak-current").exists(), "current file kept");
    assert!(!stale_dir.exists(), "stale dir pruned");
    assert!(current_dir.exists(), "current dir kept");
}

// --- MEDIUM-F: an omitted canonical DB is preserved+removed, not left mode-drifted --------------

#[test]
fn apply_import_omitting_source_evidence_does_not_leave_mode_drift() {
    let source_key = "mediumf-source-key";
    let (src_dir, _src) = fresh_paths();
    let bundle_path = src_dir.path().join("omit-source-evidence.pathkeep");
    export_encrypted_bundle_without_source_evidence(&bundle_path, source_key);

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    // The dest has a PLAINTEXT source-evidence present.
    {
        let conn = crate::archive::open_source_evidence_connection(&dest_paths, &dest_config, None)
            .expect("seed dest source-evidence");
        drop(conn);
    }
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.source_evidence_database_path),
        DiskEncryptionMode::Plaintext,
    );

    apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(source_key.to_string()),
        },
    )
    .expect("encrypted bundle omitting source-evidence must import");

    // The stale plaintext source-evidence is NOT left live paired with the new
    // encrypted history-vault: it was preserved as a `.bak` and removed from the
    // canonical path.
    assert!(
        !dest_paths.source_evidence_database_path.exists(),
        "the stale plaintext source-evidence must be moved out of the canonical path",
    );
    let archive_dir = dest_paths.archive_database_path.parent().expect("archive dir");
    let has_se_bak =
        fs::read_dir(archive_dir).expect("read archive dir").filter_map(Result::ok).any(|entry| {
            entry.file_name().to_string_lossy().starts_with("source-evidence.sqlite.bak-")
        });
    assert!(has_se_bak, "the old plaintext source-evidence must be preserved as a .bak sibling");

    // The final config mode matches the installed (encrypted) history-vault.
    assert!(matches!(
        crate::config::load_config(&dest_paths).expect("load config").archive_mode,
        ArchiveMode::Encrypted
    ));
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.archive_database_path),
        DiskEncryptionMode::Encrypted,
    );
    // Opening with the source key succeeds (no brick).
    let encrypted_config = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };
    let connection =
        crate::archive::open_archive_connection(&dest_paths, &encrypted_config, Some(source_key))
            .expect("open imported encrypted archive with the source key");
    let integrity: String =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok");
}

// --- recover_interrupted_import: direct coverage of every rollback branch ------------------------

/// Creates `app_root/archive` so a hand-built journal can be written beside the
/// (possibly not-yet-existent) archive database.
fn ensure_archive_dir(paths: &ProjectPaths) {
    fs::create_dir_all(paths.archive_database_path.parent().expect("archive parent"))
        .expect("archive dir");
}

#[test]
fn recover_interrupted_import_without_a_marker_is_a_noop() {
    let (_dir, paths) = fresh_paths();
    assert!(
        !recover_interrupted_import(&paths).expect("no-marker recovery must not error"),
        "a missing marker must be a cheap no-op",
    );
}

#[test]
fn recover_interrupted_import_removes_a_corrupt_marker() {
    let (_dir, paths) = fresh_paths();
    ensure_archive_dir(&paths);
    fs::write(import_journal_path(&paths), b"not valid json at all").expect("seed corrupt marker");
    assert!(import_journal_path(&paths).exists());

    let recovered =
        recover_interrupted_import(&paths).expect("a corrupt marker must not error the recovery");
    assert!(!recovered, "a corrupt marker cannot drive a rollback");
    assert!(
        !import_journal_path(&paths).exists(),
        "the unactionable corrupt marker must be removed"
    );
}

#[test]
fn recover_interrupted_import_restores_a_preserved_bak_unit_over_the_target() {
    let (_dir, paths) = fresh_paths();
    ensure_archive_dir(&paths);
    let target = paths.archive_database_path.clone();
    let ts = "2026-06-30T00-00-00Z";
    // The cut commit left a NEW file at the target and the preserved OLD unit at `.bak`
    // (with a `-wal` sidecar — the MEDIUM-C unit).
    fs::write(&target, b"NEW half-installed db").expect("seed new target");
    let bak = backup_sidecar_path(&target, ts);
    fs::write(&bak, b"OLD db bytes").expect("seed bak");
    fs::write(PathBuf::from(format!("{}-wal", bak.display())), b"OLD hot wal")
        .expect("seed bak wal");
    let journal = ImportJournal {
        version: 1,
        timestamp: ts.to_string(),
        canonical: vec![ImportJournalEntry { target: target.clone(), had_previous: true }],
        subtrees: Vec::new(),
        previous_config: None, // none existed pre-import => config must be absent after.
    };
    write_import_journal(&paths, &journal).expect("write journal");

    assert!(recover_interrupted_import(&paths).expect("recover"), "a present marker must recover");
    assert!(!import_journal_path(&paths).exists(), "the marker must be cleared after recovery");
    assert_eq!(
        fs::read(&target).expect("read restored target"),
        b"OLD db bytes",
        "the OLD .bak unit must be restored over the target",
    );
    assert!(
        PathBuf::from(format!("{}-wal", target.display())).exists(),
        "the OLD unit's -wal must be restored beside the target (MEDIUM-C)",
    );
    assert!(!bak.exists(), "the .bak must be consumed by the restore");
    assert!(!paths.config_path.exists(), "a None previous_config must leave config absent");
}

#[test]
fn recover_interrupted_import_removes_a_fresh_install_with_no_prior_and_restores_config() {
    let (_dir, paths) = fresh_paths();
    ensure_archive_dir(&paths);
    let target = paths.archive_database_path.clone();
    fs::write(&target, b"freshly installed, no prior").expect("seed fresh target");
    let ts = "2026-06-30T01-00-00Z";
    let prior_config_bytes = serde_json::to_string_pretty(&AppConfig::default()).expect("config");
    let journal = ImportJournal {
        version: 1,
        timestamp: ts.to_string(),
        canonical: vec![
            // had_previous=false, target exists => removed (a fresh install).
            ImportJournalEntry { target: target.clone(), had_previous: false },
            // had_previous=false, target ABSENT => remove_path_if_exists is a clean no-op.
            ImportJournalEntry {
                target: target.with_file_name("absent.sqlite"),
                had_previous: false,
            },
        ],
        subtrees: Vec::new(),
        previous_config: Some(prior_config_bytes.clone()),
    };
    write_import_journal(&paths, &journal).expect("write journal");

    assert!(recover_interrupted_import(&paths).expect("recover"));
    assert!(!target.exists(), "a fresh install with no prior must be removed on rollback");
    assert!(!import_journal_path(&paths).exists(), "the marker must be cleared");
    assert_eq!(
        fs::read_to_string(&paths.config_path).expect("read restored config"),
        prior_config_bytes,
        "a Some previous_config must be restored verbatim",
    );
}

#[test]
fn recover_interrupted_import_removes_a_fresh_subtree_install() {
    let (_dir, paths) = fresh_paths();
    ensure_archive_dir(&paths);
    let target = paths.app_root.join("derived");
    fs::create_dir_all(&target).expect("seed fresh subtree");
    fs::write(target.join("new.txt"), b"new").expect("seed subtree content");
    let ts = "2026-06-30T02-00-00Z";
    let journal = ImportJournal {
        version: 1,
        timestamp: ts.to_string(),
        canonical: Vec::new(),
        // had_previous=false, dir target exists => remove_dir_all branch of remove_path_if_exists.
        subtrees: vec![ImportJournalEntry { target: target.clone(), had_previous: false }],
        previous_config: None,
    };
    write_import_journal(&paths, &journal).expect("write journal");

    assert!(recover_interrupted_import(&paths).expect("recover"));
    assert!(!target.exists(), "a fresh subtree install with no prior must be removed (dir branch)");
}

#[test]
fn write_import_journal_errors_when_the_archive_dir_is_missing() {
    // atomic_durable_write cannot create a sibling temp without the parent dir.
    let (_dir, paths) = fresh_paths();
    let journal = ImportJournal {
        version: 1,
        timestamp: "ts".to_string(),
        canonical: Vec::new(),
        subtrees: Vec::new(),
        previous_config: None,
    };
    let err = write_import_journal(&paths, &journal)
        .expect_err("writing the journal without an archive dir must error");
    assert!(format!("{err:#}").contains("interrupted-import journal"), "got {err:#}");
}

#[test]
fn recover_interrupted_import_finishes_an_interrupted_sidecar_restore() {
    // FIX-3: a rollback that crashed AFTER renaming the OLD main back but BEFORE moving
    // its `.bak` sidecars leaves the main `.bak` consumed and `<bak>-<ts>-wal` orphaned.
    // A re-run must FINISH the sidecar move (not strip the just-restored OLD wal), so the
    // restored DB keeps its committed-but-uncheckpointed frames.
    let (_dir, paths) = fresh_paths();
    ensure_archive_dir(&paths);
    let target = paths.archive_database_path.clone();
    let ts = "2026-06-30T03-00-00Z";
    // Mid-recovery state: the OLD main is ALREADY back at the target, the main `.bak` is
    // gone, and only its `-wal` sidecar is still orphaned at `<bak>-<ts>-wal`.
    fs::write(&target, b"OLD main already restored").expect("seed restored main");
    let bak = backup_sidecar_path(&target, ts);
    let bak_wal = PathBuf::from(format!("{}-wal", bak.display()));
    fs::write(&bak_wal, b"OLD orphaned wal").expect("seed orphaned bak wal");
    let journal = ImportJournal {
        version: 1,
        timestamp: ts.to_string(),
        canonical: vec![ImportJournalEntry { target: target.clone(), had_previous: true }],
        subtrees: Vec::new(),
        previous_config: None,
    };
    write_import_journal(&paths, &journal).expect("write journal");

    assert!(recover_interrupted_import(&paths).expect("recover"));
    assert!(!import_journal_path(&paths).exists(), "the marker must be cleared after recovery");
    let target_wal = PathBuf::from(format!("{}-wal", target.display()));
    assert!(
        target_wal.exists(),
        "the orphaned OLD `-wal` must be moved onto the restored target, not dropped",
    );
    assert_eq!(fs::read(&target_wal).expect("read restored wal"), b"OLD orphaned wal");
    assert!(!bak_wal.exists(), "the orphaned bak `-wal` must be consumed by the finish-restore");
    assert_eq!(
        fs::read(&target).expect("read main"),
        b"OLD main already restored",
        "the already-restored OLD main must be left untouched",
    );
}

#[test]
fn reconcile_archive_encryption_recovers_an_interrupted_import_before_opening() {
    // FIX-5: the wired unlock entry (`reconcile_archive_encryption`) must REVERT a
    // half-applied import before it opens the archive — proving the wiring's effect.
    use std::panic::{self, AssertUnwindSafe};

    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    star_url(&src_paths, &src_config, "https://new.example/keep");
    let bundle_path = src_dir.path().join("reconcile-recover.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    star_url(&dest_paths, &dest_config, "https://old.example/keep");

    // Crash between the two canonical installs -> marker + half-state left on disk.
    crate::fault_inject::arm_panic_at("import.after_canonical_install");
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        )
    }));
    crate::fault_inject::disarm("import.after_canonical_install");
    assert!(outcome.is_err(), "the import must have crashed mid-commit");
    assert!(import_journal_path(&dest_paths).exists(), "the half-import marker must be present");

    // The unlock-path reconcile entry must recover (revert) the half-import before it
    // opens/repairs the archive.
    crate::archive::reconcile_archive_encryption(&dest_paths, &dest_config, None)
        .expect("reconcile must recover then succeed");

    assert!(!import_journal_path(&dest_paths).exists(), "reconcile must have cleared the marker");
    assert!(
        url_is_starred(&dest_paths, &dest_config, "https://old.example/keep"),
        "the OLD pre-import content must be restored by the wired recovery",
    );
    assert!(
        !url_is_starred(&dest_paths, &dest_config, "https://new.example/keep"),
        "the NEW bundle content must NOT have survived the revert",
    );
}

#[test]
fn run_backup_recovers_an_interrupted_import_before_opening_the_archive() {
    // FIX-2: the backup pre-open path must recover (revert) a crashed import before it
    // opens — closing the out-of-process scheduled-backup hole where a same-mode
    // half-state would be backed up and recorded as a success. We drive the real backup
    // entry; it reverts the half-import first, then fails fast at profile selection
    // (no profiles selected) — recovery has already run by then.
    use std::panic::{self, AssertUnwindSafe};

    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    star_url(&src_paths, &src_config, "https://new.example/keep");
    let bundle_path = src_dir.path().join("backup-recover.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    star_url(&dest_paths, &dest_config, "https://old.example/keep");

    crate::fault_inject::arm_panic_at("import.after_canonical_install");
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        )
    }));
    crate::fault_inject::disarm("import.after_canonical_install");
    assert!(outcome.is_err(), "the import must have crashed mid-commit");
    assert!(import_journal_path(&dest_paths).exists(), "the half-import marker must be present");

    // Drive the real backup entry (manual). It recovers FIRST (before open); the backup
    // itself then fails fast (no selected profiles) — irrelevant, recovery already ran.
    let _ = crate::archive::run_backup(&dest_paths, &dest_config, None, false);

    assert!(
        !import_journal_path(&dest_paths).exists(),
        "the backup pre-open path must have cleared the half-import marker via recovery",
    );
    assert!(
        url_is_starred(&dest_paths, &dest_config, "https://old.example/keep"),
        "the OLD pre-import content must be restored before the backup opens the archive",
    );
    assert!(
        !url_is_starred(&dest_paths, &dest_config, "https://new.example/keep"),
        "the NEW bundle content must NOT have been backed up (it was reverted first)",
    );
}

/// Builds a dest archive carrying a half-applied import (marker + half-state on disk)
/// over OLD starred content, returning the dest paths/config. The src bundle carries NEW
/// starred content. Mirrors the backup/reconcile recover-first fixtures so the lock-
/// completion ops (retention-prune / snapshot-restore) can be driven against it.
fn seed_interrupted_import_over_old_content(
    src_dir: &TempDir,
    bundle_name: &str,
) -> (TempDir, ProjectPaths, AppConfig) {
    use std::panic::{self, AssertUnwindSafe};

    let src_paths = project_paths_with_root(src_dir.path());
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    star_url(&src_paths, &src_config, "https://new.example/keep");
    let bundle_path = src_dir.path().join(bundle_name);
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let dest_dir = TempDir::new().expect("tempdir");
    let dest_paths = project_paths_with_root(dest_dir.path());
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    star_url(&dest_paths, &dest_config, "https://old.example/keep");

    crate::fault_inject::arm_panic_at("import.after_canonical_install");
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        )
    }));
    crate::fault_inject::disarm("import.after_canonical_install");
    assert!(outcome.is_err(), "the import must have crashed mid-commit");
    assert!(import_journal_path(&dest_paths).exists(), "the half-import marker must be present");

    (dest_dir, dest_paths, dest_config)
}

#[test]
fn run_retention_prune_recovers_an_interrupted_import_before_opening() {
    // Lock-completion block: retention-prune is a destructive op that must hold the
    // write lock AND recover-first. Driven against a present half-import marker it must
    // REVERT the crashed import to the consistent pre-import state BEFORE it opens the
    // archive to prune — proving the recover-first wiring (FAILS on pre-change code,
    // which neither recovered nor took the lock here, so the marker would survive).
    let src_dir = TempDir::new().expect("tempdir");
    let (_dest_dir, dest_paths, dest_config) =
        seed_interrupted_import_over_old_content(&src_dir, "retention-recover.pathkeep");

    // Drive the real retention-prune entry; it recovers FIRST (before opening), then
    // prunes the (empty) exports bucket. The prune outcome is irrelevant here.
    let _ = crate::archive::run_retention_prune(
        &dest_paths,
        &dest_config,
        None,
        &crate::models::RetentionPruneRequest { bucket_ids: vec!["exports".to_string()] },
    );

    assert!(
        !import_journal_path(&dest_paths).exists(),
        "retention-prune's pre-open path must have cleared the half-import marker via recovery",
    );
    assert!(
        url_is_starred(&dest_paths, &dest_config, "https://old.example/keep"),
        "the OLD pre-import content must be restored before retention-prune opens the archive",
    );
    assert!(
        !url_is_starred(&dest_paths, &dest_config, "https://new.example/keep"),
        "the NEW bundle content must NOT have survived the revert",
    );
}

#[test]
fn run_snapshot_restore_recovers_an_interrupted_import_before_opening() {
    // Lock-completion block: snapshot-restore must hold the write lock AND recover-first.
    // Driven against a present half-import marker it must REVERT the crashed import to a
    // consistent pre-import state BEFORE opening the archive — even though it then errors
    // on a non-existent snapshot, recovery has already run. (FAILS on pre-change code: it
    // neither recovered nor took the lock, so the marker would survive.)
    let src_dir = TempDir::new().expect("tempdir");
    let (_dest_dir, dest_paths, dest_config) =
        seed_interrupted_import_over_old_content(&src_dir, "snapshot-recover.pathkeep");

    // Drive the real snapshot-restore entry with a missing snapshot: it recovers FIRST
    // (before opening), then fails fast at "snapshot not found" — irrelevant to recovery.
    let _ = crate::archive::run_snapshot_restore(
        &dest_paths,
        &dest_config,
        None,
        &crate::models::SnapshotRestoreRequest { snapshot_path: "does-not-exist".to_string() },
    );

    assert!(
        !import_journal_path(&dest_paths).exists(),
        "snapshot-restore's pre-open path must have cleared the half-import marker via recovery",
    );
    assert!(
        url_is_starred(&dest_paths, &dest_config, "https://old.example/keep"),
        "the OLD pre-import content must be restored before snapshot-restore opens the archive",
    );
    assert!(
        !url_is_starred(&dest_paths, &dest_config, "https://new.example/keep"),
        "the NEW bundle content must NOT have survived the revert",
    );
}

#[cfg(unix)]
#[test]
fn apply_import_in_band_rollback_failure_leaves_the_marker_for_recovery() {
    // FIX-6: if the in-band rollback itself FAILS (here: the config-restore can't write
    // because app_root is read-only), `apply_import` must LEAVE the marker in place so
    // `recover_interrupted_import` retries at the next open, and must still surface the
    // ORIGINAL injected commit error (not the rollback error).
    use std::os::unix::fs::PermissionsExt;

    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    let bundle_path = src_dir.path().join("rollback-fail.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    // Pre-create staging/ and archive/ (writable) so staging + the journal + the
    // history-vault install + its rollback all work; only the config-restore into the
    // read-only app_root will fail.
    fs::create_dir_all(dest_paths.app_root.join("staging")).expect("staging dir");
    fs::create_dir_all(dest_paths.archive_database_path.parent().expect("archive parent"))
        .expect("archive dir");

    let app_root = dest_paths.app_root.clone();
    let original = fs::metadata(&app_root).expect("perms").permissions();
    let mut locked = original.clone();
    locked.set_mode(0o500); // r-x: search works (archive/ writes succeed), app_root writes denied.
    fs::set_permissions(&app_root, locked).expect("lock app_root");

    // Error AFTER history-vault is installed -> in-band rollback runs; its config-restore
    // (atomic_durable_write into the read-only app_root) fails.
    let _guard =
        crate::fault_inject::FaultGuard::error_at_must_fire("import.after_canonical_install");
    let result = apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
    );

    fs::set_permissions(&app_root, original).expect("restore perms");
    let err = result.expect_err("the injected commit error must surface");
    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("simulated error at checkpoint")
            && rendered.contains("import.after_canonical_install"),
        "the ORIGINAL commit error must surface, not the rollback error, got: {rendered}",
    );
    // The rollback failed (config-restore denied), so the marker is LEFT for recovery.
    assert!(
        import_journal_path(&dest_paths).exists(),
        "a failed in-band rollback must LEAVE the marker so recovery retries at next open",
    );
}

// --- HIGH (rekey-on-a-half-import): rekey recovers-first, never bricks a half-applied import ------

#[test]
fn rekey_after_an_interrupted_import_recovers_first_and_does_not_brick() {
    // The confirmed HIGH: a crash leaves a half-applied import on a PLAINTEXT archive
    // (history-vault swapped, source-evidence not, marker present). The encryption-gated
    // launch reconcile never runs on a plaintext archive, so the half-import survives to
    // when the user enables encryption. Pre-fix, `rekey_archive` did NOT recover-first, so
    // it rekeyed the NEW history-vault AND the still-ORIGINAL source-evidence to Encrypted
    // (marker + plaintext `.bak` still present); the next `recover_interrupted_import` then
    // restored history-vault to its plaintext `.bak` + config to plaintext but LEFT
    // source-evidence Encrypted — a permanent config↔source-evidence drift brick. The fix
    // makes rekey recover-first, so the post-rekey archive is already consistent.
    use std::panic::{self, AssertUnwindSafe};

    // Bundle: plaintext (carries history-vault; ships no source-evidence).
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    let bundle_path = src_dir.path().join("rekey-half-import.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    // Dest: plaintext, WITH a plaintext source-evidence present — so the half-import leaves
    // source-evidence as the untouched, no-`.bak` canonical DB the brick needs.
    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    {
        let conn = crate::archive::open_source_evidence_connection(&dest_paths, &dest_config, None)
            .expect("seed dest source-evidence");
        drop(conn);
    }
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.source_evidence_database_path),
        DiskEncryptionMode::Plaintext,
    );

    // Crash between the two canonical installs: history-vault swapped (with its `.bak`),
    // source-evidence NOT yet (so it has no `.bak`), durable marker present.
    crate::fault_inject::arm_panic_at("import.after_canonical_install");
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        apply_import(
            &dest_paths,
            &dest_config,
            None,
            &bundle_path,
            &ApplyImportOptions { confirm_overwrite: true, ..Default::default() },
        )
    }));
    crate::fault_inject::disarm("import.after_canonical_install");
    assert!(outcome.is_err(), "the import must have crashed mid-commit");
    assert!(
        import_journal_path(&dest_paths).exists(),
        "the half-import marker must be present before the rekey",
    );

    // The user now enables encryption on the still-PLAINTEXT archive. Rekey MUST
    // recover-first so it never rekeys a half-applied import into a drift brick.
    let new_key = "enable-encryption-after-half-import";
    crate::archive::rekey_archive(
        &dest_paths,
        &dest_config,
        None,
        ArchiveMode::Encrypted,
        Some(new_key),
    )
    .expect("rekey must succeed by recovering the interrupted import first");

    // PRIMARY-FIX proof: rekey recovered the import before touching anything, so the marker
    // is already cleared (pre-fix leaves it here, which is what later bricks the archive).
    assert!(
        !import_journal_path(&dest_paths).exists(),
        "rekey must recover the interrupted import first, clearing the marker",
    );

    let encrypted_config = AppConfig {
        archive_mode: ArchiveMode::Encrypted,
        initialized: true,
        ..AppConfig::default()
    };
    assert!(
        matches!(
            crate::config::load_config(&dest_paths).expect("load config").archive_mode,
            ArchiveMode::Encrypted
        ),
        "config must read the new encrypted mode after the rekey",
    );
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.archive_database_path),
        DiskEncryptionMode::Encrypted,
    );
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.source_evidence_database_path),
        DiskEncryptionMode::Encrypted,
        "BOTH canonical DBs must be encrypted — no plaintext-vs-encrypted drift",
    );

    // Driving the next-open recovery is now a clean no-op (the marker is already gone). On
    // PRE-FIX code the marker would still be present and this would restore history-vault +
    // config to plaintext while leaving source-evidence encrypted — the permanent brick.
    assert!(
        !recover_interrupted_import(&dest_paths).expect("recovery must not error"),
        "the marker is gone, so the next-open recovery must be a clean no-op",
    );

    // End-to-end: both canonical DBs decrypt with the new key and pass integrity_check —
    // the consistent, openable state a drift brick could never reach.
    let archive =
        crate::archive::open_archive_connection(&dest_paths, &encrypted_config, Some(new_key))
            .expect("the rekeyed history-vault must open with the new key");
    let integrity: String =
        archive.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok");
    drop(archive);
    let evidence = crate::archive::open_source_evidence_connection(
        &dest_paths,
        &encrypted_config,
        Some(new_key),
    )
    .expect("source-evidence must open with the new key (no config<->DB drift brick)");
    let evidence_integrity: String = evidence
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("source-evidence integrity");
    assert_eq!(evidence_integrity, "ok");
}

// --- defense in depth: recovery is fail-closed on an unreconcilable canonical-DB mode drift -------

#[test]
fn recover_interrupted_import_fails_closed_on_mutually_inconsistent_canonical_modes() {
    // FIX (defense in depth): even if a destructive op rewrote one canonical DB's at-rest
    // mode in the unrecovered window (so the rollback's "leave untouched" branch leaves the
    // two canonical DBs at DIFFERENT modes that no single config can serve), recovery must
    // NOT silently remove the marker and commit a mode-drifted config. It must restore what
    // it can, then refuse — leaving the marker + surfacing a recoverable error.
    let (_dir, paths) = fresh_paths();
    ensure_archive_dir(&paths);
    let ts = "2026-06-30T09-00-00Z";

    // history-vault: a NEW (encrypted-looking) half-install over a PLAINTEXT `.bak`, so the
    // rollback restores it to plaintext.
    let history = paths.archive_database_path.clone();
    fs::write(&history, [9u8; 32]).expect("seed NEW encrypted-looking history-vault");
    let history_bak = backup_sidecar_path(&history, ts);
    fs::write(&history_bak, b"SQLite format 3\0OLD plaintext history").expect("seed plaintext bak");

    // source-evidence: PRESENT and ENCRYPTED with NO `.bak`, so the rollback's
    // "had_previous, no `.bak` -> leave untouched" branch keeps it encrypted.
    let evidence = paths.source_evidence_database_path.clone();
    fs::write(&evidence, [7u8; 32]).expect("seed encrypted-looking source-evidence");

    let journal = ImportJournal {
        version: 1,
        timestamp: ts.to_string(),
        canonical: vec![
            ImportJournalEntry { target: history.clone(), had_previous: true },
            ImportJournalEntry { target: evidence.clone(), had_previous: true },
        ],
        subtrees: Vec::new(),
        previous_config: None,
    };
    write_import_journal(&paths, &journal).expect("write journal");

    let err = recover_interrupted_import(&paths)
        .expect_err("recovery must fail closed on an unreconcilable canonical-DB mode drift");
    let rendered = format!("{err:#}");
    assert!(
        rendered.contains("single consistent at-rest mode")
            && rendered.contains("source-evidence.sqlite"),
        "the error must name the unreconcilable drift, got: {rendered}",
    );

    // The history-vault `.bak` was still restored (rollback is best-effort up to the
    // refusal), but the marker is LEFT so the state stays flagged + retry-able and NO
    // drifted config is committed.
    assert_eq!(
        detect_disk_encryption_mode(&history),
        DiskEncryptionMode::Plaintext,
        "history-vault is restored to its plaintext .bak",
    );
    assert_eq!(
        detect_disk_encryption_mode(&evidence),
        DiskEncryptionMode::Encrypted,
        "source-evidence stays encrypted (the irreconcilable half)",
    );
    assert!(
        import_journal_path(&paths).exists(),
        "a fail-closed recovery must LEAVE the marker, never silently brick + clear it",
    );
    assert!(!paths.config_path.exists(), "no drifted config may be committed on the refusal");
}

#[test]
fn disk_mode_for_maps_each_archive_mode() {
    // Both arms of the config-mode -> on-disk-header mapping the fail-closed guard relies on.
    assert_eq!(disk_mode_for(&ArchiveMode::Plaintext), DiskEncryptionMode::Plaintext);
    assert_eq!(disk_mode_for(&ArchiveMode::Encrypted), DiskEncryptionMode::Encrypted);
}

// --- E1/E3: config<->disk invariant post-conditions for whole-app import -------------------------
//
// `check_config_disk_consistency` is the cross-FILE behavioral assertion the 100%-coverage gate
// never had. Threading it into import's SUCCESS post-condition proves import commits config LAST +
// consistent; the exhaustive per-checkpoint crash torture proves EVERY import crash window recovers
// to a consistent-or-fail-closed state, never a mode-mixed brick (CRIT-3's shape). Import writes
// config LAST, so it always ends matching the installed DBs — the exact ordering the incident broke.

/// Opens the recovered archive in whatever mode `config.json` now records and asserts it is
/// structurally sound — the "openable, not bricked" half of consistent-or-fail-closed.
fn assert_recovered_archive_opens(dest_paths: &ProjectPaths, source_key: &str) {
    let recovered = crate::config::load_config(dest_paths).expect("load recovered config");
    let key = match recovered.archive_mode {
        ArchiveMode::Encrypted => Some(source_key),
        ArchiveMode::Plaintext => None,
    };
    let connection = crate::archive::open_archive_connection(dest_paths, &recovered, key)
        .expect("the recovered archive must open in its recorded mode");
    let integrity: String =
        connection.query_row("PRAGMA integrity_check", [], |row| row.get(0)).expect("integrity");
    assert_eq!(integrity, "ok", "the recovered archive must be structurally consistent");
}

#[test]
fn apply_import_success_leaves_config_matching_the_installed_dbs() {
    // E1 post-condition: a successful whole-app import of an ENCRYPTED bundle onto a PLAINTEXT dest
    // must end with config.json (read through the REAL load_config) matching the installed encrypted
    // DBs on disk — the invariant the 2026-06-30 incident violated (config lagged the files).
    let source_key = "phase-e-import-success-key";
    let (src_dir, _src) = fresh_paths();
    let bundle_path = src_dir.path().join("import-success.pathkeep");
    export_encrypted_bundle(&bundle_path, source_key);

    let (_dest_dir, dest_paths) = fresh_paths();
    let dest_config = seed_plain_archive(&dest_paths, b"dest marker");
    apply_import(
        &dest_paths,
        &dest_config,
        None,
        &bundle_path,
        &ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some(source_key.to_string()),
        },
    )
    .expect("the encrypted bundle must import");

    crate::archive::check_config_disk_consistency(&dest_paths)
        .expect("a successful import must leave config matching the installed DBs");
    assert_eq!(
        detect_disk_encryption_mode(&dest_paths.archive_database_path),
        DiskEncryptionMode::Encrypted,
        "the imported encrypted bundle installs an encrypted canonical archive",
    );
    assert_recovered_archive_opens(&dest_paths, source_key);
}

#[test]
fn apply_import_crash_at_every_checkpoint_recovers_consistent_or_fail_closed() {
    // E3 torture (deterministic, exhaustive): a crash at EACH named import checkpoint, followed by
    // launch recovery, must reach a state whose config matches the installed DBs — either rolled back
    // to the consistent pre-import archive or committed forward to the new one — never the mode-mixed
    // half-import the pre-hardening flow could ship. The fixed checkpoint enumeration keeps it
    // reproducible (no RNG / wall-clock).
    let source_key = "phase-e-import-torture-key";
    let (src_dir, _src) = fresh_paths();
    let bundle_path = src_dir.path().join("import-torture.pathkeep");
    export_encrypted_bundle(&bundle_path, source_key);

    for checkpoint in [
        "import.after_stage_before_swap",
        "import.after_canonical_install",
        "import.after_swap_before_config",
        "import.after_config",
    ] {
        let (_dest_dir, dest_paths) = fresh_paths();
        let dest_config = seed_plain_archive(&dest_paths, b"dest marker");

        {
            let _guard = crate::fault_inject::FaultGuard::error_at_must_fire(checkpoint);
            let error = apply_import(
                &dest_paths,
                &dest_config,
                None,
                &bundle_path,
                &ApplyImportOptions {
                    confirm_overwrite: true,
                    source_archive_key: Some(source_key.to_string()),
                },
            )
            .expect_err(&format!("the injected crash at {checkpoint} must abort the import"));
            let rendered = format!("{error:#}");
            assert!(
                rendered.contains(checkpoint),
                "the INJECTED fault must propagate at {checkpoint}, got: {rendered}",
            );
        }

        let outcome = crate::archive::recover_archive_on_launch(&dest_paths, &dest_config, None)
            .unwrap_or_else(|error| panic!("launch recovery at {checkpoint} errored: {error:#}"));
        // A plaintext-dest + encrypted-bundle import self-heals in every window here (rollback to the
        // consistent plaintext dest, or roll forward to the committed encrypted archive) — never
        // fail-closed — so recovery must not surface Unrecoverable.
        assert!(
            !matches!(outcome, crate::archive::LaunchRecovery::Unrecoverable(_)),
            "import crash at {checkpoint} must recover, got {outcome:?}",
        );
        crate::archive::check_config_disk_consistency(&dest_paths).unwrap_or_else(|error| {
            panic!("the invariant must hold after recovering an import crash at {checkpoint}: {error:#}")
        });
        assert_recovered_archive_opens(&dest_paths, source_key);
    }
}

// --- journal / rollback error + no-op arms -------------------------------------------------------

#[test]
fn read_import_journal_treats_a_missing_marker_as_absent() {
    // The no-marker fast arm: with no interrupted-import journal on disk, the reader reports `None`
    // (nothing to roll back) WITHOUT touching the archive, so a normal open — the overwhelmingly
    // common case — never mistakes "no crash" for one and never runs a spurious recovery.
    let (_dir, paths) = fresh_paths();
    assert!(!import_journal_path(&paths).exists(), "precondition: no marker on a fresh root");
    assert!(
        read_import_journal(&paths).expect("a missing marker must not error").is_none(),
        "a missing interrupted-import marker must read back as absent",
    );
}

#[cfg(unix)]
#[test]
fn restore_bak_sidecars_surfaces_a_rename_failure() {
    // The error-context arm of the sidecar move: a `<bak>-wal` exists to restore, but a DIRECTORY
    // squats on the `<target>-wal` destination, so `fs::rename` fails (a file cannot be renamed onto
    // a directory). The failure must be SURFACED (naming which sidecar) rather than swallowed —
    // silently dropping the OLD database's committed-but-uncheckpointed WAL frames would corrupt the
    // undo backstop.
    let dir = TempDir::new().expect("tempdir");
    let target = dir.path().join("history-vault.sqlite");
    let bak = backup_sidecar_path(&target, "2026-06-30T00-00-00Z");
    // The `.bak` unit's hot WAL to restore back onto the target.
    fs::write(PathBuf::from(format!("{}-wal", bak.display())), b"old hot wal")
        .expect("seed bak wal");
    // A non-empty directory squatting on the destination makes the rename fail (EISDIR/ENOTEMPTY).
    let target_wal = PathBuf::from(format!("{}-wal", target.display()));
    fs::create_dir(&target_wal).expect("seed blocking dir at the target wal path");
    fs::write(target_wal.join("inner"), b"x").expect("seed blocking child");

    let error = restore_bak_sidecars(&bak, &target)
        .expect_err("a blocked sidecar destination must surface a rename error");
    assert!(
        format!("{error:#}").contains("restoring sidecar"),
        "the error must name the sidecar-restore step, got: {error:#}",
    );
    // The source sidecar is left in place (never lost) when the move could not complete.
    assert!(
        PathBuf::from(format!("{}-wal", bak.display())).exists(),
        "a failed move must not lose the OLD database's hot WAL",
    );
}

#[cfg(unix)]
#[test]
fn rollback_import_surfaces_a_main_restore_rename_failure() {
    // The error-context arm of the OLD-main restore inside `rollback_import`: the swap had started
    // (a `.bak` unit exists), so rollback takes the restore-from-bak arm and tries to rename the
    // `.bak` back onto the target — but the archive directory is read-only, so the rename fails. The
    // failure must be surfaced (naming the restore) so a half-rolled-back archive is never mistaken
    // for a completed rollback that then clears the crash marker.
    use std::os::unix::fs::PermissionsExt;
    let (_dir, paths) = fresh_paths();
    let archive_dir = paths.archive_database_path.parent().expect("archive parent").to_path_buf();
    fs::create_dir_all(&archive_dir).expect("archive dir");
    let target = paths.archive_database_path.clone();
    let timestamp = "2026-06-30T00-00-00Z";
    // A `.bak` unit present => rollback takes the `bak.exists()` restore arm; the target itself is
    // absent (nothing installed yet), so the only step that can fail is the `.bak -> target` rename.
    let bak = backup_sidecar_path(&target, timestamp);
    fs::write(&bak, b"pre-import history-vault").expect("seed the .bak unit");

    let journal = ImportJournal {
        version: 1,
        timestamp: timestamp.to_string(),
        canonical: vec![ImportJournalEntry { target: target.clone(), had_previous: true }],
        subtrees: Vec::new(),
        previous_config: None,
    };

    // Read-only archive dir: the `.bak -> target` rename cannot create the restored file there.
    let original = fs::metadata(&archive_dir).expect("archive dir meta").permissions();
    let mut locked = original.clone();
    locked.set_mode(0o500);
    fs::set_permissions(&archive_dir, locked).expect("lock the archive dir read-only");

    let result = rollback_import(&paths, &journal);

    fs::set_permissions(&archive_dir, original).expect("restore archive dir perms");
    let error =
        result.expect_err("a read-only archive dir must surface the OLD-main restore rename error");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("restoring") && rendered.contains("history-vault"),
        "the error must name the OLD-main restore from the .bak, got: {rendered}",
    );
}

#[test]
fn apply_import_surfaces_a_failure_reading_the_pre_import_config() {
    // The error-context arm of capturing `previous_config` before the swap: a DIRECTORY squats where
    // `config.json` must be, so `config_path.exists()` is true but `read_to_string` fails (EISDIR).
    // apply_import must surface that read error and abort BEFORE it writes the crash journal or
    // renames any live file — a rollback that could not faithfully restore the prior config must
    // never be entered.
    let (src_dir, src_paths) = fresh_paths();
    let src_config = seed_plain_archive(&src_paths, b"src marker");
    let bundle_path = src_dir.path().join("preconfig-read-fail.pathkeep");
    export_app_data(&src_paths, &src_config, None, &bundle_path).expect("export");

    let (_dest_dir, dest_paths) = fresh_paths();
    // A fresh dest whose config path is an (unreadable-as-a-string) DIRECTORY: exists() is true, but
    // reading it as the pre-import config fails.
    fs::create_dir_all(&dest_paths.config_path).expect("squat a directory on the dest config path");

    let error = apply_import(
        &dest_paths,
        &AppConfig::default(),
        None,
        &bundle_path,
        &ApplyImportOptions { confirm_overwrite: false, ..Default::default() },
    )
    .expect_err("reading the pre-import config as a directory must fail the import");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("before import"),
        "the error must name the pre-import config read, got: {rendered}",
    );
    // Fail-fast contract: no crash marker was written and no live file was renamed to `.bak-*`.
    assert!(
        !import_journal_path(&dest_paths).exists(),
        "a pre-swap read failure must not leave an interrupted-import marker",
    );
    assert_no_backup_sidecars(&dest_paths.app_root);
}
