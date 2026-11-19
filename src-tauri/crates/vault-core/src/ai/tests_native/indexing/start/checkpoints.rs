//! Checks write frequency and the positions saved at cooperative stops.
//! Uses actual planning pages and the same ledger callbacks used by the worker.
use super::*;
use crate::ai_queue;

#[test]
fn semantic_scope_unchanged_chunks_coalesce_checkpoints() {
    struct QueueLedger {
        vault: crate::Vault,
        job: i64,
    }
    impl IndexBackfillLedger for QueueLedger {
        fn job_id(&self) -> Option<i64> {
            Some(self.job)
        }
        fn record(&self, progress: IndexBackfillProgress) -> Result<()> {
            let cursor = ai_queue::IndexBackfillCursor {
                next_history_id: progress.next_history_id,
                scan: progress.scan,
                ..Default::default()
            };
            assert_eq!(
                ai_queue::persist_index_cursor(&self.vault, self.job, &cursor, None)?,
                ai_queue::CursorPersistOutcome::Persisted
            );
            Ok(())
        }
    }
    let runtime = Runtime::new().unwrap();
    let (_, config, connection) = prepared_archive();
    let vault = connection.vault.with_config(config);
    let provider = embedding_provider();
    seed_visits(&vault, pages(1..=18));
    let request = AiIndexRequest { limit: Some(1), ..Default::default() };
    runtime.block_on(build_ai_index(&vault, &provider, &request)).unwrap();
    let job = ai_queue::request_index_job(&vault, &request, false).unwrap().unwrap().id;
    ai_queue::claim_next_ai_job(&vault, 300).unwrap().unwrap();
    let ledger = Arc::new(QueueLedger { vault: vault.clone(), job });
    vault.store().capture_query_plans(Db::Intelligence, true).unwrap();
    runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &request,
            None,
            0,
            0,
            Some(ledger.clone()),
        ))
        .unwrap();
    vault.store().capture_query_plans(Db::Intelligence, false).unwrap();
    let persisted = vault
        .store()
        .take_query_plans(Db::Intelligence)
        .unwrap()
        .into_iter()
        .filter(|p| p.sql.starts_with("UPDATE ai_jobs") && p.sql.contains("SET payload_json"))
        .count();
    let ai_queue::AiJobPayload::Index { cursor, .. } =
        ai_queue::load_ai_job_payload(&vault, job).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(cursor.scan.unwrap().examined, 18);
    assert!((1..18).contains(&persisted), "{persisted} cursor writes for 18 unchanged chunks");
}

#[test]
fn semantic_scope_planning_saves_periodically_and_flushes_on_pause() {
    struct ObservePlanning {
        vault: crate::Vault,
        job: i64,
        positions: std::sync::Mutex<Vec<i64>>,
    }
    impl AiRunControl for ObservePlanning {
        fn checkpoint(&self, detail: &str) -> Result<()> {
            if detail == COUNT_STOPPED {
                let ai_queue::AiJobPayload::Index { cursor, .. } =
                    ai_queue::load_ai_job_payload(&self.vault, self.job)?
                else {
                    unreachable!()
                };
                let mut positions = self.positions.lock().unwrap();
                positions.push(cursor.scan.unwrap().planning_after);
                if positions.len() == 6 {
                    return Err(AiRunCancelled::new("stop after five planning pages").into());
                }
            }
            Ok(())
        }
    }
    let runtime = Runtime::new().unwrap();
    let (_, config, db) = prepared_archive();
    let vault = db.vault.with_config(config);
    seed_visits(&vault, pages([1]));
    vault.write(Db::Archive, Priority::Batch, |s| {
        s.execute("WITH RECURSIVE n(id) AS (VALUES(2) UNION ALL SELECT id+1 FROM n WHERE id<24576)
            INSERT INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id)
            SELECT id,1,(SELECT source_profile_id FROM visits WHERE id=1),id,'then',1 FROM n",())?;
        let profile = fixtures::profile_row_id("chrome:Default");
        crate::archive::totals::add_visits(s, profile, 24_575)?;
        Ok(())
    }).unwrap();
    let request = AiIndexRequest::default();
    let job = ai_queue::enqueue_index_job(&vault, &request, false).unwrap().id;
    ai_queue::claim_next_ai_job(&vault, 300).unwrap().unwrap();
    let observer =
        Arc::new(ObservePlanning { vault: vault.clone(), job, positions: Default::default() });
    let ledger = Arc::new(PauseAfterBatch {
        job_id: job,
        progress: Default::default(),
        pause: false,
        start_history_id: 0,
    });
    let error = runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &embedding_provider(),
            &request,
            Some(observer.clone()),
            0,
            0,
            Some(ledger),
        ))
        .unwrap_err();
    assert!(error.is::<AiRunCancelled>());
    let positions = observer.positions.lock().unwrap();
    assert!(positions.windows(2).any(|pair| pair[0] == pair[1]), "{positions:?}");
    assert!(positions.iter().any(|&after| after > 0), "no durable progress before pause");
    let ai_queue::AiJobPayload::Index { cursor, .. } =
        ai_queue::load_ai_job_payload(&vault, job).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(cursor.scan.unwrap().planning_after, 5 * COUNT_PAGE_IDS);
    assert_eq!(ai_queue::load_ai_job(&vault, job).unwrap().attempt, 0);
}
