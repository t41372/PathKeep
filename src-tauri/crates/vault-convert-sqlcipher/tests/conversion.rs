//! Responsibilities: convert generated SQLCipher facts into the native archive contract.
//! Not responsible for: UI upgrade presentation or SQLite engine behavior.
//! Failure modes: old percent-escape keys detach favorites/notes; resume duplicates
//! rows; digest mismatch is published; old files are removed; search omits converted pages;
//! a file still keyed with an earlier password fails the whole conversion, or is skipped
//! without the assessment saying so; the evidence each import stored again survives, or a
//! conversion resumed after de-duplicating it checks the shrunken tables against the old file.
use vault_convert_sqlcipher::{assess, convert};
use vault_core::{AppConfig, ArchiveMode, project_paths_with_root};
use vault_store::{CancelToken, Db};

fn fixture() -> (tempfile::TempDir, vault_core::ProjectPaths, AppConfig) {
    let dir = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(dir.path());
    let config = AppConfig {
        initialized: true,
        git_enabled: false,
        archive_mode: ArchiveMode::Encrypted,
        ..Default::default()
    };
    vault_core::config::ensure_paths(&paths).unwrap();
    vault_core::save_config(&paths, &config).unwrap();
    vault_convert_sqlcipher::test_support::seed_reader_pages(
        &paths,
        &config,
        Some("old password"),
        &[
            ("https://example.com/%7Ealice?utm_source=old", "Escaped page"),
            ("https://example.com/~alice", "Current rule page"),
        ],
    );
    let db =
        vault_convert_sqlcipher::test_support::open_archive(&paths, &config, Some("old password"));
    db.execute("INSERT INTO star(entity_kind,entity_key,starred_at,source_profile) VALUES('url','https://example.com/%7Ealice','saved','chrome:Default')",[]).unwrap();
    db.execute("INSERT INTO url_annotations(url,notes,created_at,updated_at) VALUES('https://example.com/%7Ealice','legacy note','saved','saved')",[]).unwrap();
    let intelligence =
        vault_convert_sqlcipher::test_support::open_intelligence(&paths, Some("old password"));
    let payload = serde_json::json!({"historyId":1,"profileId":"chrome:Default","url":"https://example.com/%7Ealice?utm_source=old","title":"Escaped page"});
    intelligence.execute("INSERT INTO intelligence_jobs(id,job_type,plugin_id,state,dedupe_key,payload_json,created_at,scheduled_at,updated_at) VALUES(1,'enrichment-plugin','content-fetch','queued','content-fetch:https://example.com/%7Ealice',?1,'saved','saved','saved')",[payload.to_string()]).unwrap();
    intelligence.execute("INSERT INTO visit_content_enrichments(history_id,content_source,fetch_status,fetched_at,snippet_json,extraction_json,pipeline_version,enrichment_summary) VALUES(1,'content-fetch','success','saved','{}','{}','v1','summarylegacytoken')",[]).unwrap();
    intelligence.execute("INSERT INTO ai_embeddings(history_id,profile_id,url,title,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at) VALUES(1,'chrome:Default','https://example.com/%7Ealice?utm_source=old','Escaped page','example.com','saved','old-stored-hash',12,'local','fixture','saved')",[]).unwrap();
    let evidence = vault_convert_sqlcipher::test_support::open_source_evidence(
        &paths,
        &config,
        Some("old password"),
    );
    evidence.execute("INSERT INTO source_batches(id,source_profile_id,source_kind,schema_fingerprint,parser_version,capability_snapshot_json,coverage_stats_json,created_at) VALUES(1,1,'fixture','hash','v1','{}','{}','saved')",[]).unwrap();
    for _ in 0..2 {
        evidence.execute("INSERT INTO schema_observations VALUES(1,'History','{\"field\":\"kept\"}','saved')",[]).unwrap();
    }
    let agent = vault_convert_sqlcipher::test_support::open_agent(&paths);
    agent.execute("INSERT INTO conversations(id,title,created_at,updated_at) VALUES('c','Converted conversation','saved','saved')",[]).unwrap();
    agent.execute("INSERT INTO messages(id,conversation_id,seq,role,content,created_at) VALUES('m','c',1,'user','Keep this message','saved')",[]).unwrap();
    agent.execute("INSERT INTO agent_runs(id,conversation_id,status,created_at,updated_at) VALUES('r','c','completed','saved','saved')",[]).unwrap();
    agent.execute("INSERT INTO agent_steps(id,run_id,seq,turn,kind,payload,created_at) VALUES('s','r',1,0,'tool-result','{}','saved')",[]).unwrap();
    agent.execute("INSERT INTO agent_citations(run_id,history_id,canonical_url,url,score) VALUES('r',1,'https://example.com/%7Ealice','https://example.com/%7Ealice?utm_source=old',0.5)",[]).unwrap();
    (dir, paths, config)
}

#[test]
fn interrupted_conversion_resumes_without_duplicate_visits() {
    let (_dir, paths, config) = fixture();
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let first = convert(&paths, &config, Some("old password"), cancel, move |p| {
        if p.table.as_deref() == Some("urls") {
            stop.cancel();
        }
    });
    assert!(first.is_err());
    assert!(!paths.app_root.join("archive/history-vault.db").exists());
    let resumed =
        convert(&paths, &config, Some("old password"), CancelToken::new(), |_| {}).unwrap();
    let count = resumed
        .vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn a_changed_staged_value_refuses_the_swap() {
    let (_dir, paths, config) = fixture();
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    assert!(
        convert(&paths, &config, Some("old password"), cancel, move |p| {
            if p.table.as_deref() == Some("urls") {
                stop.cancel();
            }
        })
        .is_err()
    );
    let staged_paths = project_paths_with_root(&paths.staging_dir.join("sqlcipher-conversion"));
    let key = vault_core::Vault::unlock_data_key(&staged_paths, "old password").unwrap();
    let staged = vault_core::Vault::open_staged(staged_paths, config.clone(), Some(&key)).unwrap();
    staged
        .store()
        .write(Db::Archive, vault_store::Priority::Batch, |s| {
            s.execute("UPDATE urls SET title='damaged' WHERE id=1", ())?;
            Ok(())
        })
        .unwrap();
    staged.close().unwrap();
    let result = convert(&paths, &config, Some("old password"), Default::default(), |_| {});
    let error = result.expect_err("digest mismatch must refuse publication");
    assert!(format!("{error:#}").contains("conversion-digest-mismatch"));
    assert!(!paths.app_root.join("archive/history-vault.db").exists());
    assert!(paths.archive_database_path.exists());
}

#[test]
fn a_process_exit_between_tables_resumes_committed_rows() {
    let (_dir, paths, config) = fixture();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "conversion_crash_child", "--ignored", "--nocapture"])
        .env("PATHKEEP_CONVERSION_CRASH_ROOT", &paths.app_root)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(86));
    let result =
        convert(&paths, &config, Some("old password"), Default::default(), |_| {}).unwrap();
    let count = result
        .vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT count(*) FROM urls", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
#[ignore = "child exits without running destructors during conversion"]
fn conversion_crash_child() {
    let paths = project_paths_with_root(std::path::Path::new(
        &std::env::var("PATHKEEP_CONVERSION_CRASH_ROOT").unwrap(),
    ));
    let config = vault_core::load_config(&paths).unwrap();
    let _ = convert(&paths, &config, Some("old password"), Default::default(), |p| {
        if p.table.as_deref() == Some("urls") {
            std::process::exit(86);
        }
    })
    .unwrap();
    panic!("the conversion boundary was not reached");
}

/// v0.1–v0.3 changed the password of the archive only, so an install that changed it can
/// carry a source-evidence file keyed with the password before. That file can't be read;
/// everything else converts, and the file stays as it was.
#[test]
fn a_file_keyed_with_an_earlier_password_is_named_left_in_place_and_skipped() {
    let (_root, paths, config) = fixture();
    let evidence = &paths.source_evidence_database_path;
    std::fs::remove_file(evidence).unwrap();
    let earlier = vault_convert_sqlcipher::test_support::open_source_evidence(
        &paths,
        &config,
        Some("earlier password"),
    );
    earlier.execute("INSERT INTO source_batches(id,source_profile_id,source_kind,schema_fingerprint,parser_version,capability_snapshot_json,coverage_stats_json,created_at) VALUES(9,1,'fixture','hash','v1','{}','{}','saved')",[]).unwrap();
    drop(earlier);
    let archive =
        vault_convert_sqlcipher::test_support::open_archive(&paths, &config, Some("old password"));
    archive.execute("INSERT INTO profile_watermarks(profile_id,updated_at,last_source_batch_id) VALUES('chrome:Default','saved',9)",[]).unwrap();
    drop(archive);
    let untouched = std::fs::read(evidence).unwrap();

    let wrong = assess(&paths, &config, Some("earlier password")).unwrap_err();
    assert!(wrong.is::<vault_core::ArchivePasswordWrong>(), "{wrong:#}");
    let preview = assess(&paths, &config, Some("old password")).unwrap();
    assert_eq!(
        preview.files_locked,
        vec![vault_convert_sqlcipher::LockedFile {
            database: "SourceEvidence".into(),
            path: evidence.clone(),
        }]
    );
    assert!(preview.tables.iter().any(|t| t.table == "visits" && t.rows == 2));
    assert!(preview.tables.iter().all(|t| t.database != "SourceEvidence"));

    let result =
        convert(&paths, &config, Some("old password"), CancelToken::new(), |_| {}).unwrap();
    assert_eq!(std::fs::read(evidence).unwrap(), untouched);
    let vault = &result.vault;
    let (visits, watermark) = vault
        .store()
        .read(Db::Archive, |s| {
            Ok((
                s.query_one("SELECT count(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?,
                s.query_one("SELECT last_source_batch_id FROM profile_watermarks", (), |r| {
                    Ok(r.get::<Option<i64>>(0)?)
                })?,
            ))
        })
        .unwrap();
    assert_eq!((visits, watermark), (2, None));
    let batches = vault
        .store()
        .read(Db::SourceEvidence, |s| {
            s.query_one("SELECT count(*) FROM source_batches", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    assert_eq!(batches, 0);
    let chats = vault
        .store()
        .read(Db::Agent, |s| {
            s.query_one("SELECT count(*) FROM messages", (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    assert_eq!(chats, 1);
}

/// The old format stored a visit's evidence again for every import that read it. The conversion
/// keeps one row per evidence, and a value that changed (the visit's duration, read after its tab
/// closed) as a row of its own. Stopped after that, it carries on without checking the evidence
/// tables against the old file again.
#[test]
fn evidence_stored_again_by_each_import_is_kept_once_also_after_a_stop() {
    let (_dir, paths, config) = fixture();
    let evidence = vault_convert_sqlcipher::test_support::open_source_evidence(
        &paths,
        &config,
        Some("old password"),
    );
    for (batch, duration) in [(1, 4_000), (1, 4_000), (1, 4_000), (1, 9_000)] {
        evidence
            .execute(
                "INSERT INTO visit_engagement_evidence(source_batch_id,source_profile_id,source_visit_id,metric_key,metric_value_int,source_field,recorded_at)
                 VALUES(?1,1,'7','engagement.total_foreground_duration_ms',?2,'visits.total_foreground_duration','saved')",
                [batch, duration],
            )
            .unwrap();
    }
    drop(evidence);
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let first = convert(&paths, &config, Some("old password"), cancel, move |p| {
        if p.phase == "projecting" {
            stop.cancel();
        }
    });
    assert!(first.is_err(), "stopped after the evidence was de-duplicated");
    let resumed =
        convert(&paths, &config, Some("old password"), CancelToken::new(), |_| {}).unwrap();
    let durations = resumed
        .vault
        .store()
        .read(Db::SourceEvidence, |s| {
            s.query(
                "SELECT metric_value_int, content_key IS NOT NULL FROM visit_engagement_evidence ORDER BY id",
                (),
                |r| Ok((r.get::<i64>(0)?, r.get::<bool>(1)?)),
            )
        })
        .unwrap();
    assert_eq!(durations, vec![(4_000, true), (9_000, true)]);
}
