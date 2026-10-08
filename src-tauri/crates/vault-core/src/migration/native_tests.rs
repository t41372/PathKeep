//! Responsibilities: portable native bundle behavior on real temporary roots.
//! Not responsible for: SQLCipher's legacy export API or UI confirmation text.
//! Failure modes: omitted canonical/evidence/agent rows or saved page text, derived leakage, wrong
//! export password, tamper accepted, fresh-root import requires an existing archive.
use super::*;
use vault_store::{Db, Priority};

#[test]
fn export_then_import_into_an_empty_root_round_trips_native_tables() {
    for key in [None, Some([53; 32])] {
        let source = crate::Vault::open_temp_with_key(key).unwrap();
        let search_pause = source.search().pause();
        crate::store_schema::tests::seed_maintenance(&source);
        let expected = crate::store_schema::tests::maintenance_rows(
            &source,
            &[Db::Archive, Db::SourceEvidence, Db::Agent],
        );
        for db in [Db::Archive, Db::SourceEvidence, Db::Agent] {
            source
                .store()
                .write(db, Priority::Batch, |s| {
                    s.execute(
                        "CREATE TABLE bundle_sentinel(id INTEGER PRIMARY KEY,value BLOB)",
                        (),
                    )?;
                    s.execute("INSERT INTO bundle_sentinel VALUES(1,X'0001FF')", ())?;
                    Ok(())
                })
                .unwrap();
        }
        // Saved page text lives beside the databases and can't always be fetched again.
        let page_text = std::path::Path::new("generic-readable/ab/abcdef.txt");
        let blob = source.paths().intelligence_blobs_dir.join(page_text);
        std::fs::create_dir_all(blob.parent().unwrap()).unwrap();
        std::fs::write(&blob, "saved article body").unwrap();
        let output = tempfile::tempdir().unwrap();
        let bundle = output.path().join("portable.pathkeep");
        let exported =
            native::export(&source, &bundle, "export password", Default::default(), |_| {})
                .unwrap();
        assert!(exported.manifest.files.iter().all(|f| !f.path.starts_with("derived/")));
        let root = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(root.path());
        let preview = native::preview(&paths, &bundle).unwrap();
        assert!(!preview.will_overwrite_existing);
        let options = ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some("export password".into()),
        };
        let (_, imported) =
            native::import(&paths, None, None, &bundle, &options, Default::default()).unwrap();
        assert_eq!(
            crate::store_schema::tests::maintenance_rows(
                &imported.vault,
                &[Db::Archive, Db::SourceEvidence, Db::Agent]
            ),
            expected
        );
        for db in [Db::Archive, Db::SourceEvidence, Db::Agent] {
            assert_eq!(
                imported
                    .vault
                    .store()
                    .read(db, |s| s.query_one("SELECT value FROM bundle_sentinel", (), |r| Ok(
                        r.get::<Vec<u8>>(0)?
                    )))
                    .unwrap(),
                vec![0, 1, 255]
            );
        }
        assert_eq!(
            std::fs::read_to_string(paths.intelligence_blobs_dir.join(page_text)).unwrap(),
            "saved article body"
        );
        imported.vault.close().unwrap();
        assert!(crate::Vault::unlock_data_key(&paths, "wrong").is_err());
        assert!(crate::Vault::unlock_data_key(&paths, "export password").is_ok());
        drop(search_pause);
    }
}

#[test]
fn wrong_export_password_and_changed_database_bytes_preserve_the_target() {
    let source = crate::Vault::open_temp().unwrap();
    let files = tempfile::tempdir().unwrap();
    let bundle = files.path().join("bundle.pathkeep");
    native::export(&source, &bundle, "export password", Default::default(), |_| {}).unwrap();
    let (_root, target, _) = crate::maintenance::tests::encrypted();
    let paths = target.paths().clone();
    let envelope = std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap();
    let wrong = ApplyImportOptions {
        confirm_overwrite: true,
        source_archive_key: Some("wrong password".into()),
    };
    assert!(
        native::import(&paths, Some(&target), None, &bundle, &wrong, Default::default()).is_err()
    );
    assert_eq!(std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap(), envelope);
    let damaged = files.path().join("damaged.pathkeep");
    let mut original = zip::ZipArchive::new(std::fs::File::open(&bundle).unwrap()).unwrap();
    let mut output = zip::ZipWriter::new(std::fs::File::create(&damaged).unwrap());
    for index in 0..original.len() {
        let mut entry = original.by_index(index).unwrap();
        output.start_file(entry.name(), zip::write::SimpleFileOptions::default()).unwrap();
        if entry.name() == "archive/history-vault.db" {
            output.write_all(b"changed database").unwrap();
        } else {
            std::io::copy(&mut entry, &mut output).unwrap();
        }
    }
    output.finish().unwrap();
    let correct = ApplyImportOptions {
        confirm_overwrite: true,
        source_archive_key: Some("export password".into()),
    };
    assert!(
        native::import(&paths, Some(&target), None, &damaged, &correct, Default::default())
            .is_err()
    );
    assert!(target.store().is_open());
    assert_eq!(std::fs::read(paths.app_root.join("archive/keys.envelope")).unwrap(), envelope);
}

#[test]
fn native_preview_rejects_an_inventory_missing_a_database() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("missing.pathkeep");
    let manifest = ExportManifest {
        format_version: 2,
        app_version: "test".into(),
        archive_schema_version: 1,
        archive_mode: "encrypted".into(),
        exported_at: "now".into(),
        exporter_hostname: None,
        files: Vec::new(),
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&bundle).unwrap());
    zip.start_file(MANIFEST_ZIP_PATH, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(&bytes).unwrap();
    zip.start_file(MANIFEST_SHA_ZIP_PATH, zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(crate::utils::sha256_hex(&bytes).as_bytes()).unwrap();
    zip.finish().unwrap();
    let error = native::preview(&crate::project_paths_with_root(root.path()), &bundle).unwrap_err();
    assert!(error.to_string().contains("missing required entry"));
}

#[test]
fn export_cannot_replace_the_live_archive_or_its_configuration() {
    let vault = crate::Vault::open_temp().unwrap();
    for path in [&vault.paths().archive_database_path, &vault.paths().config_path] {
        let error = native::export(&vault, path, "export password", Default::default(), |_| {})
            .unwrap_err();
        assert!(error.to_string().contains("export-target-is-managed"));
    }
    assert!(vault.store().is_open());
    assert_eq!(
        vault
            .store()
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM pages", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        0
    );
}

#[test]
fn portable_provenance_is_encrypted_relocated_and_replayable_on_an_empty_root() {
    use browser_history_fixtures::{ChromiumHistoryFixture, ChromiumUrlRow, ChromiumVisitRow};
    for key in [None, Some([31; 32])] {
        let source = crate::Vault::open_temp_with_key(key).unwrap();
        let checkpoint = source.paths().raw_snapshots_dir.join("chrome-safe/checkpoint");
        std::fs::create_dir_all(&checkpoint).unwrap();
        ChromiumHistoryFixture::new()
            .add_url(ChromiumUrlRow {
                id: 100,
                url: "https://portable-artifact.test/secret".into(),
                title: Some("SENSITIVE PORTABLE CHECKPOINT".into()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: 1_800_000_000_000,
                hidden: false,
            })
            .add_visit(ChromiumVisitRow {
                id: 100,
                url_id: 100,
                visit_time_unix_ms: 1_800_000_000_000,
                from_visit: None,
                transition: Some(1),
                visit_duration_micros: None,
                is_known_to_sync: false,
                visited_link_id: None,
                external_referrer_url: None,
                app_id: None,
            })
            .write(&checkpoint.join("History"))
            .unwrap();
        let audit = source.paths().manifests_dir.join("portable.json");
        std::fs::create_dir_all(audit.parent().unwrap()).unwrap();
        std::fs::write(&audit, b"{\"provenance\":\"kept\"}").unwrap();
        let checkpoint_path = checkpoint.display().to_string();
        let audit_path = audit.display().to_string();
        let audit_hash = crate::utils::sha256_hex(&std::fs::read(&audit).unwrap());
        source.store().write(Db::Archive,Priority::Batch,move |s|{
            s.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,browser_family,profile_name,profile_path,discovered_at) VALUES(1,'chrome:Default','chrome','chromium','Default','/fixture','2026-10-06T00:00:00Z')",())?;
            s.execute("INSERT INTO runs(id,run_type,trigger,started_at,finished_at,status,profile_scope_json,stats_json,warnings_json) VALUES(41,'backup','manual','2026-10-06T00:00:00Z','2026-10-06T00:00:00Z','success','[\"chrome:Default\"]','{}','[]')",())?;
            s.execute("INSERT INTO snapshots(run_id,file_path,file_size,reason,created_at) VALUES(41,?1,1,'raw-source-checkpoint','2026-10-06T00:00:00Z')",[checkpoint_path])?;
            s.execute("INSERT INTO manifests(run_id,content_hash,row_counts_json,created_at,file_path) VALUES(41,?1,'{}','2026-10-06T00:00:00Z',?2)",vault_store::params![audit_hash,audit_path])?;
            for name in ["snapshots","manifests","sourceProfiles"] {s.execute("UPDATE archive_totals SET value=1 WHERE name=?1",[name])?;}
            Ok(())
        }).unwrap();
        let files = tempfile::tempdir().unwrap();
        let bundle = files.path().join("portable.pathkeep");
        native::export(&source, &bundle, "portable password", Default::default(), |_| {}).unwrap();
        let bytes = std::fs::read(&bundle).unwrap();
        let needle = b"https://portable-artifact.test/secret";
        assert!(!bytes.windows(needle.len()).any(|b| b == needle));
        let target = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(target.path());
        let (_, installed) = native::import(
            &paths,
            None,
            None,
            &bundle,
            &ApplyImportOptions {
                confirm_overwrite: true,
                source_archive_key: Some("portable password".into()),
            },
            Default::default(),
        )
        .unwrap();
        let imported = paths.raw_snapshots_dir.join("chrome-safe/checkpoint");
        assert_eq!(
            std::fs::read(imported.join("History")).unwrap(),
            std::fs::read(checkpoint.join("History")).unwrap()
        );
        assert_eq!(
            std::fs::read(paths.manifests_dir.join("portable.json")).unwrap(),
            std::fs::read(&audit).unwrap()
        );
        let restored = crate::run_snapshot_restore(
            &installed.vault,
            &crate::SnapshotRestoreRequest { snapshot_path: imported.display().to_string() },
        )
        .unwrap();
        assert_eq!(restored.run.unwrap().new_visits, 1);
    }
}

#[test]
fn portable_import_keeps_app_lock_policy_with_its_local_secret() {
    let source = crate::Vault::open_temp().unwrap();
    let mut config = source.config().clone();
    config.app_lock.enabled = true;
    crate::set_app_lock_passcode(
        source.paths(),
        &mut config,
        &crate::SetAppLockPasscodeRequest {
            passcode: "source passcode".into(),
            recovery_hint: Some("source hint".into()),
        },
    )
    .unwrap();
    let source = source.with_config(config);
    let output = tempfile::tempdir().unwrap();
    let bundle = output.path().join("portable.pathkeep");
    native::export(&source, &bundle, "export password", Default::default(), |_| {}).unwrap();
    for existing_lock in [false, true] {
        let destination = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(destination.path());
        let mut local = crate::AppConfig::default();
        local.app_lock.enabled = existing_lock;
        if existing_lock {
            crate::set_app_lock_passcode(
                &paths,
                &mut local,
                &crate::SetAppLockPasscodeRequest {
                    passcode: "destination passcode".into(),
                    recovery_hint: Some("destination hint".into()),
                },
            )
            .unwrap();
        }
        let (_, installed) = native::import(
            &paths,
            None,
            None,
            &bundle,
            &ApplyImportOptions {
                confirm_overwrite: false,
                source_archive_key: Some("export password".into()),
            },
            Default::default(),
        )
        .unwrap();
        let config = installed.vault.config();
        assert_eq!(config.app_lock.enabled, existing_lock);
        assert_eq!(config.app_lock.recovery_hint, local.app_lock.recovery_hint);
        crate::initialize_app_lock_session(&paths, config).unwrap();
        if existing_lock {
            let status = crate::unlock_app_session(
                &paths,
                config,
                &crate::UnlockAppSessionRequest {
                    passcode: Some("destination passcode".into()),
                    use_biometric: false,
                },
            )
            .unwrap();
            assert!(!status.locked);
        } else {
            assert!(!crate::app_lock_status(&paths, config).unwrap().locked);
        }
        installed.vault.close().unwrap();
    }
}
