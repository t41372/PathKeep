//! Responsibilities: import the whole-app files v0.3 exported (bundle format 1) on another
//! computer, the way the shipped v0.3 wrote them.
//! Not responsible for: native bundles (vault-core migration tests) or the table copy
//! (conversion.rs).
//! Failure modes: a v0.3 file is refused; a wrong password creates or changes this
//! machine's archive; saved page text, stars or notes are lost on the way; snapshot paths
//! still point into the exporting computer's folder; the preview promises chats v0.3 never
//! exported.
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use vault_convert_sqlcipher::{bundle, test_support};
use vault_core::{AppConfig, ApplyImportOptions, ArchiveMode, project_paths_with_root};
use vault_store::{CancelToken, Db};

/// An encrypted v0.3 data folder with a page, a star, a note, saved page text and a raw
/// snapshot, packed the way v0.3's `export_app_data` packed it.
fn v0_3_bundle(dir: &Path) -> PathBuf {
    let root = dir.join("old-computer/PathKeep");
    let paths = project_paths_with_root(&root);
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Encrypted,
        ..Default::default()
    };
    vault_core::config::ensure_paths(&paths).unwrap();
    vault_core::save_config(&paths, &config).unwrap();
    test_support::seed_reader_pages(
        &paths,
        &config,
        Some("old password"),
        &[("https://example.com/kept", "Kept page")],
    );
    let snapshot = root.join("raw-snapshots/run-1/History");
    std::fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    std::fs::write(&snapshot, b"browser history copy").unwrap();
    let archive = test_support::open_archive(&paths, &config, Some("old password"));
    archive.execute("INSERT INTO star(entity_kind,entity_key,starred_at,source_profile) VALUES('url','https://example.com/kept','saved','chrome:Default')",[]).unwrap();
    archive.execute("INSERT INTO url_annotations(url,notes,created_at,updated_at) VALUES('https://example.com/kept','a note','saved','saved')",[]).unwrap();
    archive.execute("INSERT INTO snapshots(run_id,file_path,file_size,created_at) VALUES((SELECT min(id) FROM runs),?1,20,'saved')",[snapshot.display().to_string()]).unwrap();
    drop(archive);
    let intelligence = test_support::open_intelligence(&paths, Some("old password"));
    intelligence.execute("INSERT INTO visit_content_enrichments(history_id,content_source,fetch_status,fetched_at,snippet_json,extraction_json,pipeline_version,enrichment_summary) VALUES(1,'content-fetch','success','saved','{}','{}','v1','saved page text')",[]).unwrap();
    drop(intelligence);

    let bundle = dir.join("from-v0.3.pathkeep-bundle");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&bundle).unwrap());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut files = Vec::new();
    let mut add = |source: &Path, name: String| {
        let bytes = std::fs::read(source).unwrap();
        zip.start_file(&name, options).unwrap();
        zip.write_all(&bytes).unwrap();
        files.push(serde_json::json!({
            "path": name,
            "sha256": hex::encode(Sha256::digest(&bytes)),
            "sizeBytes": bytes.len(),
        }));
    };
    add(&paths.config_path, "config/config.json".into());
    add(&root.join("archive/history-vault.sqlite"), "archive/history-vault.sqlite".into());
    for folder in ["derived", "raw-snapshots"] {
        let mut stack = vec![root.join(folder)];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let name =
                        path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                    add(&path, name);
                }
            }
        }
    }
    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "formatVersion": 1,
        "appVersion": "0.3.0",
        "archiveSchemaVersion": 16,
        "archiveMode": "encrypted",
        "exportedAt": "2026-06-01T10:00:00Z",
        "exporterHostname": "old-computer",
        "files": files,
    }))
    .unwrap();
    zip.start_file("pathkeep-export-manifest.json", options).unwrap();
    zip.write_all(&manifest).unwrap();
    zip.start_file("pathkeep-export-manifest.sha256", options).unwrap();
    zip.write_all(format!("{}\n", hex::encode(Sha256::digest(&manifest))).as_bytes()).unwrap();
    zip.finish().unwrap();
    bundle
}

fn options(password: &str) -> ApplyImportOptions {
    ApplyImportOptions { confirm_overwrite: false, source_archive_key: Some(password.into()) }
}

#[test]
fn a_file_exported_by_v0_3_imports_with_its_data_on_another_computer() {
    let dir = tempfile::tempdir().unwrap();
    let bundle_path = v0_3_bundle(dir.path());
    let here = project_paths_with_root(&dir.path().join("new-computer/PathKeep"));

    let preview = bundle::preview(&here, &bundle_path).unwrap();
    assert_eq!(preview.manifest.format_version, 1);
    assert!(!preview.will_overwrite_existing);
    assert!(preview.exclusion_notes.iter().any(|n| n.path == "derived/agent.sqlite"));

    let wrong = bundle::import(
        &here,
        None,
        None,
        &bundle_path,
        &options("not the password"),
        CancelToken::new(),
        |_| {},
    )
    .unwrap_err();
    assert!(wrong.is::<vault_core::ArchivePasswordWrong>(), "{wrong:#}");
    assert!(!here.app_root.join("archive/history-vault.db").exists());
    assert!(!here.config_path.exists());

    let (report, installed) = bundle::import(
        &here,
        None,
        None,
        &bundle_path,
        &options("old password"),
        CancelToken::new(),
        |_| {},
    )
    .unwrap();
    assert!(!report.preserved_previous_as_bak);
    let vault = &installed.vault;
    assert_eq!(vault.config().archive_mode, ArchiveMode::Encrypted);
    let (pages, stars, notes, snapshot) = vault
        .store()
        .read(Db::Archive, |s| {
            Ok((
                s.query("SELECT canonical_url FROM pages", (), |r| Ok(r.get::<String>(0)?))?,
                s.query_one("SELECT count(*) FROM star", (), |r| Ok(r.get::<i64>(0)?))?,
                s.query("SELECT notes FROM url_annotations", (), |r| Ok(r.get::<String>(0)?))?,
                s.query_one("SELECT file_path FROM snapshots", (), |r| Ok(r.get::<String>(0)?))?,
            ))
        })
        .unwrap();
    assert_eq!(pages, vec!["https://example.com/kept"]);
    assert_eq!(stars, 1);
    assert_eq!(notes, vec!["a note"]);
    let snapshot = PathBuf::from(snapshot);
    assert!(snapshot.starts_with(&here.app_root), "{}", snapshot.display());
    assert_eq!(std::fs::read(&snapshot).unwrap(), b"browser history copy");
    let saved_text = vault
        .store()
        .read(Db::Intelligence, |s| {
            s.query_one("SELECT count(*) FROM page_content_enrichments", (), |r| {
                Ok(r.get::<i64>(0)?)
            })
        })
        .unwrap();
    assert_eq!(saved_text, 1);
    for old in ["archive/history-vault.sqlite", "derived/history-intelligence.sqlite"] {
        assert!(!here.app_root.join(old).exists(), "{old} was installed");
    }
}
