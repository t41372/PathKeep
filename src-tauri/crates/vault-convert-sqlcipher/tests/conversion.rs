//! Responsibilities: convert generated SQLCipher facts into the native archive contract.
//! Not responsible for: UI upgrade presentation or SQLite engine behavior.
//! Failure modes: old percent-escape keys detach favorites/notes; resume duplicates
//! rows; digest mismatch is published; old files are removed; search omits converted pages.
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
    vault_core::test_support::legacy_fixtures::seed_reader_pages(
        &paths,
        &config,
        Some("old password"),
        &[
            ("https://example.com/%7Ealice?utm_source=old", "Escaped page"),
            ("https://example.com/~alice", "Current rule page"),
        ],
    );
    let db = vault_core::archive::open_archive_connection(&paths, &config, Some("old password"))
        .unwrap();
    db.execute("INSERT INTO star(entity_kind,entity_key,starred_at,source_profile) VALUES('url','https://example.com/%7Ealice','saved','chrome:Default')",[]).unwrap();
    db.execute("INSERT INTO url_annotations(url,notes,created_at,updated_at) VALUES('https://example.com/%7Ealice','legacy note','saved','saved')",[]).unwrap();
    let intelligence = vault_core::test_support::legacy_fixtures::open_legacy_intelligence_database(
        &paths,
        Some("old password"),
    );
    let payload = serde_json::json!({"historyId":1,"profileId":"chrome:Default","url":"https://example.com/%7Ealice?utm_source=old","title":"Escaped page"});
    intelligence.execute("INSERT INTO intelligence_jobs(id,job_type,plugin_id,state,dedupe_key,payload_json,created_at,scheduled_at,updated_at) VALUES(1,'enrichment-plugin','content-fetch','queued','content-fetch:https://example.com/%7Ealice',?1,'saved','saved','saved')",[payload.to_string()]).unwrap();
    intelligence.execute("INSERT INTO visit_content_enrichments(history_id,content_source,fetch_status,fetched_at,snippet_json,extraction_json,pipeline_version,enrichment_summary) VALUES(1,'content-fetch','success','saved','{}','{}','v1','summarylegacytoken')",[]).unwrap();
    intelligence.execute("INSERT INTO ai_embeddings(history_id,profile_id,url,title,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at) VALUES(1,'chrome:Default','https://example.com/%7Ealice?utm_source=old','Escaped page','example.com','saved','old-stored-hash',12,'local','fixture','saved')",[]).unwrap();
    let evidence = vault_core::archive::open_legacy_source_evidence_connection(
        &paths,
        &config,
        Some("old password"),
    )
    .unwrap();
    evidence.execute("INSERT INTO source_batches(id,source_profile_id,source_kind,schema_fingerprint,parser_version,capability_snapshot_json,coverage_stats_json,created_at) VALUES(1,1,'fixture','hash','v1','{}','{}','saved')",[]).unwrap();
    for _ in 0..2 {
        evidence.execute("INSERT INTO schema_observations VALUES(1,'History','{\"field\":\"kept\"}','saved')",[]).unwrap();
    }
    let agent = vault_core::test_support::legacy_fixtures::open_legacy_agent_database(&paths);
    agent.execute("INSERT INTO conversations(id,title,created_at,updated_at) VALUES('c','Converted conversation','saved','saved')",[]).unwrap();
    agent.execute("INSERT INTO messages(id,conversation_id,seq,role,content,created_at) VALUES('m','c',1,'user','Keep this message','saved')",[]).unwrap();
    agent.execute("INSERT INTO agent_runs(id,conversation_id,status,created_at,updated_at) VALUES('r','c','completed','saved','saved')",[]).unwrap();
    agent.execute("INSERT INTO agent_steps(id,run_id,seq,turn,kind,payload,created_at) VALUES('s','r',1,0,'tool-result','{}','saved')",[]).unwrap();
    agent.execute("INSERT INTO agent_citations(run_id,history_id,canonical_url,url,score) VALUES('r',1,'https://example.com/%7Ealice','https://example.com/%7Ealice?utm_source=old',0.5)",[]).unwrap();
    (dir, paths, config)
}

#[test]
fn encrypted_conversion_recomputes_keys_preserves_original_and_builds_search() {
    let (_dir, paths, config) = fixture();
    let assessment = assess(&paths, &config, Some("old password")).unwrap();
    assert!(assessment.required);
    assert!(assessment.tables.iter().any(|t| t.table == "visits" && t.rows == 2));
    let original = std::fs::read(&paths.archive_database_path).unwrap();
    let result =
        convert(&paths, &config, Some("old password"), CancelToken::new(), |_| {}).unwrap();
    assert_eq!(std::fs::read(&paths.archive_database_path).unwrap(), original);
    let vault = &result.vault;
    let keys = vault
        .store()
        .read(Db::Archive, |s| {
            s.query("SELECT canonical_url FROM pages ORDER BY id", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(keys, vec!["https://example.com/~alice"; 2]);
    let star = vault
        .store()
        .read(Db::Archive, |s| {
            s.query_one("SELECT entity_key FROM star", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(star, "https://example.com/~alice");
    assert_eq!(
        vault
            .store()
            .read(Db::Archive, |s| s
                .query_one("SELECT source_profile FROM star", (), |r| Ok(r.get::<String>(0)?),))
            .unwrap(),
        "chrome:Default"
    );
    let annotation =
        vault
            .store()
            .read(Db::Archive, |s| {
                s.query_one("SELECT canonical_url FROM url_annotations", (), |r| {
                    Ok(r.get::<String>(0)?)
                })
            })
            .unwrap();
    assert_eq!(annotation, star);
    let enrichment_key = vault
        .store()
        .read(Db::Intelligence, |s| {
            s.query_one("SELECT dedupe_key FROM intelligence_jobs WHERE id=1", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })
        .unwrap();
    assert_eq!(enrichment_key, "content-fetch:https://example.com/~alice");
    let embedding_key = vault
        .store()
        .read(Db::Intelligence, |s| {
            s.query_one("SELECT content_hash FROM ai_embeddings", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(
        embedding_key,
        vault_core::ai::build_dedup_content_hash(
            "https://example.com/~alice",
            Some("Escaped page"),
            Some("summarylegacytoken")
        )
    );
    let favorites =
        vault_core::stars::list_stars(vault, None, Default::default(), None, None).unwrap();
    assert_eq!(
        vault
            .store()
            .read(Db::SourceEvidence, |s| s.query_one(
                "SELECT count(*) FROM schema_observations",
                (),
                |r| Ok(r.get::<i64>(0)?)
            ))
            .unwrap(),
        2
    );
    assert_eq!(
        vault
            .store()
            .read(Db::Agent, |s| s.query_one(
                "SELECT content FROM messages WHERE id='m'",
                (),
                |r| Ok(r.get::<String>(0)?)
            ))
            .unwrap(),
        "Keep this message"
    );
    assert_eq!(
        vault
            .store()
            .read(Db::Agent, |s| s.query_one(
                "SELECT canonical_url FROM agent_citations",
                (),
                |r| Ok(r.get::<String>(0)?)
            ))
            .unwrap(),
        "https://example.com/~alice"
    );
    assert_eq!(favorites.items.len(), 1);
    assert_eq!(favorites.items[0].notes, "legacy note");
    assert_eq!(favorites.items[0].visit_count, 2);
    vault.search().cycle().unwrap();
    let mut search = vault_core::search::SearchSession::start(
        vault,
        vault_core::search::SearchRequest { query: "page".into(), ..Default::default() },
    )
    .unwrap();
    assert_eq!(search.finish_page().unwrap().items.len(), 2);
    let mut enriched = vault_core::search::SearchSession::start(
        vault,
        vault_core::search::SearchRequest {
            query: "summarylegacytoken".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(enriched.finish_page().unwrap().items.len(), 2);
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
fn stars_that_now_share_a_canonical_key_keep_the_latest_star_time() {
    let (_root, paths, config) = fixture();
    let db = vault_core::archive::open_archive_connection(&paths, &config, Some("old password"))
        .unwrap();
    db.execute("INSERT INTO star(entity_kind,entity_key,starred_at,source_profile) VALUES('url','https://example.com/~alice','zz-latest','chrome:Other')",[]).unwrap();
    drop(db);
    let result =
        convert(&paths, &config, Some("old password"), Default::default(), |_| {}).unwrap();
    let stars =
        vault_core::stars::list_stars(&result.vault, None, Default::default(), None, None).unwrap();
    assert_eq!(stars.items.len(), 1);
    assert_eq!(stars.items[0].starred_at, "zz-latest");
    assert_eq!(
        result
            .vault
            .store()
            .read(Db::Archive, |s| s
                .query_one("SELECT source_profile FROM star", (), |r| Ok(r.get::<String>(0)?),))
            .unwrap(),
        "chrome:Other"
    );
    assert_eq!(stars.items[0].notes, "legacy note");
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

#[test]
fn assessment_includes_committed_legacy_wal_bytes_and_retained_sidecars() {
    let (_root, paths, config) = fixture();
    let connection =
        vault_core::archive::open_archive_connection(&paths, &config, Some("old password"))
            .unwrap();
    connection.execute_batch("PRAGMA wal_autocheckpoint=0").unwrap();
    connection.execute("INSERT INTO star(entity_kind,entity_key,starred_at) VALUES('url','https://example.com/wal','saved')", []).unwrap();
    let wal = paths.archive_database_path.with_file_name("history-vault.sqlite-wal");
    let minimum = std::fs::metadata(&paths.archive_database_path).unwrap().len()
        + std::fs::metadata(&wal).unwrap().len();
    let preview = assess(&paths, &config, Some("old password")).unwrap();
    assert!(preview.files_retained.contains(&wal));
    assert!(preview.source_bytes >= minimum);
    assert!(preview.required_free_bytes >= minimum * 3);
}
