//! Responsibilities: verify old-rule variants merge without losing facts or triggers.
//! Not responsible for: intelligence algorithms or parser behavior.
//! Failure modes: key conflict refusal, order-dependent newest content, counter/set
//! loss, dangling triggers, digest omissions or double counting after cancellation.
use vault_core::{AppConfig, ArchiveMode, Vault, project_paths_with_root};
use vault_store::{CancelToken, Db};
const PASSWORD: &str = "collision password";

#[test]
fn old_variants_merge_content_counters_sets_and_triggers_even_after_resume() {
    for mode in [ArchiveMode::Plaintext, ArchiveMode::Encrypted] {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: mode.clone(),
            ..Default::default()
        };
        let password = (mode == ArchiveMode::Encrypted).then_some(PASSWORD);
        vault_core::config::ensure_paths(&paths).unwrap();
        vault_core::save_config(&paths, &config).unwrap();
        vault_core::test_support::legacy_fixtures::seed_reader_pages(
            &paths,
            &config,
            password,
            &[
                ("https://example.com/%7Ealice", "Escaped"),
                ("https://example.com/~alice", "Decoded"),
            ],
        );
        let db = vault_core::test_support::legacy_fixtures::open_legacy_intelligence_database(
            &paths, password,
        );
        // The lower ID is newer despite its earlier textual date (different UTC offset).
        for (id, url, at, title, attempt, count, profiles, visits) in [
            (
                7,
                "https://example.com/%7Ealice",
                "2026-10-05T23:00:00-02:00",
                "newest",
                2,
                4,
                vec!["a", "shared"],
                vec![1, 2],
            ),
            (
                9,
                "https://example.com/~alice",
                "2026-10-06T00:00:00Z",
                "older",
                3,
                6,
                vec!["b", "shared"],
                vec![2, 3],
            ),
        ] {
            let payload = serde_json::json!({"historyId":if id==7 {1} else {2},"profileId":"chrome:Default","url":url,"title":title});
            let artifact = serde_json::json!({"detail":title,"processedItems":count,"affectedProfiles":profiles,"notes":[title],"counters":{"fetches":count},"count":count,"statusCode":if id==7 {200} else {503}});
            db.execute("INSERT INTO intelligence_jobs(id,job_type,plugin_id,state,attempt,dedupe_key,payload_json,artifact_json,created_at,scheduled_at,updated_at) VALUES(?1,'enrichment-plugin','content-fetch','succeeded',?2,?3,?4,?5,?6,?6,?6)",rusqlite::params![id,attempt,format!("content-fetch:{url}"),payload.to_string(),artifact.to_string(),at]).unwrap();
            db.execute("INSERT INTO intelligence_job_triggers(id,job_id,run_id,reason,requested_at) VALUES(?1,?1,1,?2,?3)",rusqlite::params![id,title,at]).unwrap();
            let evidence = serde_json::json!({"cross_day_count":attempt,"visitIds":visits,"detail":title,"nested":{"trailCount":count,"labels":profiles}});
            db.execute("INSERT INTO refind_pages(profile_id,canonical_url,url,title,registrable_domain,cross_day_count,trail_count,search_arrival_count,typed_revisit_count,refind_score,evidence_json,first_seen_ms,last_seen_ms,computed_at) VALUES('chrome:Default',?1,?1,?2,'example.com',?3,?4,?3,?4,?4,?5,?3,?4,?6)",rusqlite::params![url,title,attempt,count as f64,evidence.to_string(),at]).unwrap();
        }
        drop(db);
        for (phase, table) in
            [("merging", "intelligence_jobs"), ("converting", "intelligence_job_triggers")]
        {
            let cancel = CancelToken::new();
            let signal = cancel.clone();
            assert!(
                vault_convert_sqlcipher::convert(&paths, &config, password, cancel, move |p| {
                    if p.phase == phase && p.table.as_deref() == Some(table) && p.processed > 0 {
                        signal.cancel();
                    }
                },)
                .is_err()
            );
        }
        assert!(!paths.app_root.join("archive/history-vault.db").exists());
        let result =
            vault_convert_sqlcipher::convert(&paths, &config, password, Default::default(), |_| {})
                .unwrap();
        assert_merged(&result.vault);
        result.vault.close().unwrap();
    }
}

fn assert_merged(vault: &Vault) {
    vault
        .store()
        .read(Db::Intelligence, |s| {
            let jobs = s.query(
                "SELECT id,dedupe_key,attempt,payload_json,artifact_json FROM intelligence_jobs WHERE job_type='enrichment-plugin'",
                (),
                |r| {
                    Ok((
                        r.get::<i64>(0)?,
                        r.get::<String>(1)?,
                        r.get::<i64>(2)?,
                        r.get::<String>(3)?,
                        r.get::<String>(4)?,
                    ))
                },
            )?;
            assert_eq!(jobs.len(), 1);
            // Insights aren't converted; conversion queues one full rebuild instead.
            let rebuilds = s.query_one(
                "SELECT COUNT(*) FROM intelligence_jobs WHERE job_type<>'enrichment-plugin'",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?;
            assert_eq!(rebuilds, 1);
            assert_eq!(
                (jobs[0].0, jobs[0].1.as_str(), jobs[0].2),
                (7, "content-fetch:https://example.com/~alice", 5)
            );
            let payload: serde_json::Value = serde_json::from_str(&jobs[0].3).unwrap();
            assert_eq!(payload["title"], "newest");
            assert_eq!(payload["historyId"], 1);
            let artifact: serde_json::Value = serde_json::from_str(&jobs[0].4).unwrap();
            assert_eq!(artifact["detail"], "newest");
            assert_eq!(artifact["processedItems"], 10);
            assert_eq!(artifact["counters"]["fetches"], 10);
            assert_eq!(artifact["count"], 10);
            assert_eq!(artifact["statusCode"], 200);
            assert_eq!(artifact["affectedProfiles"], serde_json::json!(["a", "b", "shared"]));
            assert_eq!(artifact["notes"], serde_json::json!(["newest", "older"]));
            let triggers = s.query(
                "SELECT id,job_id,reason FROM intelligence_job_triggers WHERE job_id IN (SELECT id FROM intelligence_jobs WHERE job_type='enrichment-plugin') ORDER BY id",
                (),
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<String>(2)?)),
            )?;
            assert_eq!(triggers, vec![(7, 7, "newest".into()), (9, 7, "older".into())]);
            // The old refind pages are derived data; Insights rebuilds them after conversion.
            Ok(())
        })
        .unwrap();
}
