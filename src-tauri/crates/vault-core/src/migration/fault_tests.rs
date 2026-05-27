//! Fault-injection regressions for whole-app import safety.
//!
//! ## Responsibilities
//! - Prove import refusal paths leave the existing project tree untouched.
//! - Pin fail-fast ordering before any `.bak-*` preservation rename runs.
//!
//! ## Not responsible for
//! - Full bundle round-trip coverage, which remains in the parent test module.
//! - Scheduler, keyring, or app-lock fault contracts.
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
