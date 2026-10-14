//! Responsibilities: a job is claimed by one worker only, and a lease left by a dead worker is
//! taken back.
//! Not responsible for: network fetching or model execution.
//! Failure modes: two workers claim one job, or an expired lease keeps its job running forever.
use super::claims::{next_queued_intelligence_job_in_session, try_claim_enrichment_job};
use super::*;
use vault_store::params_owned as params;

#[test]
fn compare_and_set_claim_skips_jobs_taken_by_another_connection() {
    for key in [None, Some([79; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let id = vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, |s| {
                let payload = EnrichmentJobPayload {
                    history_id: Some(42),
                    profile_id: Some("chrome:Default".into()),
                    url: "https://example.com/race".into(),
                    title: Some("Race".into()),
                };
                enqueue_content_fetch_job(s, &payload)
            })
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles = (0..2)
            .map(|_| {
                let vault = vault.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    vault
                        .write(
                            vault_store::Db::Intelligence,
                            vault_store::Priority::Interactive,
                            move |s| try_claim_enrichment_job(s, id, &crate::utils::now_rfc3339()),
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let won = handles.into_iter().map(|h| h.join().unwrap()).filter(|won| *won).count();
        assert_eq!(won, 1);
        let claimed = vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, move |s| {
                try_claim_enrichment_job(s, id, &crate::utils::now_rfc3339())
            })
            .unwrap();
        assert!(!claimed);
        let attempt = vault
            .store()
            .read(vault_store::Db::Intelligence, move |s| {
                s.query_one("SELECT attempt FROM intelligence_jobs WHERE id=?1", [id], |r| {
                    Ok(r.get::<i64>(0)?)
                })
            })
            .unwrap();
        assert_eq!(attempt, 1);
        cancel_intelligence_job(&vault, id).unwrap();
        let unavailable = vault
            .write(vault_store::Db::Intelligence, vault_store::Priority::Interactive, move |s| {
                claim_content_fetch_job_by_id(s, id)
            })
            .unwrap();
        assert!(unavailable.is_none());
    }
}

#[test]
fn next_queued_intelligence_job_recovers_expired_leases() {
    let vault = crate::Vault::open_temp().unwrap();
    vault.write(vault_store::Db::Intelligence,vault_store::Priority::Interactive, |connection| {
    let expired_at = (Utc::now() - Duration::minutes(10)).to_rfc3339();
    let now = crate::utils::now_rfc3339();
    connection
        .execute(
            "INSERT INTO intelligence_jobs
             (job_type, plugin_id, run_id, state, priority, attempt, dedupe_key, payload_json,
              artifact_json, created_at, scheduled_at, started_at, heartbeat_at, lease_owner,
              lease_expires_at, updated_at, stop_requested)
             VALUES
                (?1, NULL, NULL, 'running', ?2, 1, 'expired-queued', ?3, '{}', ?4, ?4, ?4, ?4, 'worker', ?5, ?4, 0),
                (?6, NULL, NULL, 'running', ?7, 1, 'expired-cancelled', ?8, '{}', ?4, ?4, ?4, ?4, 'worker', ?5, ?4, 1)",
            params![
                STRUCTURAL_REBUILD_JOB_TYPE,
                STRUCTURAL_REBUILD_PRIORITY,
                serde_json::to_string(&DeterministicRebuildJobPayload {
                    job_type: STRUCTURAL_REBUILD_JOB_TYPE.to_string(),
                    request: CoreIntelligenceRebuildRequest::default(),
                    reason: "expired lease".to_string(),
                })
                .expect("payload"),
                now,
                expired_at,
                DAILY_ROLLUP_JOB_TYPE,
                DAILY_ROLLUP_PRIORITY,
                serde_json::to_string(&DeterministicRebuildJobPayload {
                    job_type: DAILY_ROLLUP_JOB_TYPE.to_string(),
                    request: CoreIntelligenceRebuildRequest::default(),
                    reason: "expired cancelled lease".to_string(),
                })
                .expect("payload"),
            ],
        )
        .expect("insert expired jobs");

    let next_job = next_queued_intelligence_job_in_session(connection)
        .expect("next queued job after recovery")
        .expect("recovered queued job");
    assert_eq!(next_job.job_type, STRUCTURAL_REBUILD_JOB_TYPE);

    let states = connection
        .prepare(
            "SELECT dedupe_key, state, last_error
             FROM intelligence_jobs
             WHERE dedupe_key IN ('expired-queued', 'expired-cancelled')
             ORDER BY dedupe_key ASC",
        )
        .expect("prepare recovered states")
        .query_map((), |row| {
            Ok((
                row.get::< String>(0)?,
                row.get::< String>(1)?,
                row.get::< Option<String>>(2)?,
            ))
        })
        .expect("query recovered states")
        .collect::<vault_store::Result<Vec<_>>>()
        .expect("collect recovered states");
    assert_eq!(states[0].0, "expired-cancelled");
    assert_eq!(states[0].1, "cancelled");
    assert_eq!(states[1].0, "expired-queued");
    assert_eq!(states[1].1, "queued");
    assert!(
        states[1].2.as_deref().is_some_and(|value| value.contains("expired intelligence lease"))
    );
    Ok(())
    }).unwrap();
}
