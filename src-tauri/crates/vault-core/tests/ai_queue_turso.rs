//! Responsibilities: verify durable queue transitions through the real owner.
//! Not responsible for: model execution or search.
//! Failure modes: two claims win, an abandoned claim remains running after reopen,
//! completed work is claimed again, cancellation is overwritten, or paging hides jobs.

use vault_core::{AiIndexRequest, Vault, ai_queue::*};

#[test]
fn reopening_recovers_only_abandoned_claims_and_keeps_finished_work() {
    for key in [None, Some([62; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let finished = enqueue_index_job(&vault, &AiIndexRequest::default(), false).unwrap();
        claim_ai_job_by_id(&vault, finished.id, 300).unwrap().unwrap();
        assert!(mark_ai_job_succeeded(&vault, finished.id, None, None).unwrap());
        let unfinished = enqueue_index_job(&vault, &AiIndexRequest::default(), false).unwrap();
        claim_ai_job_by_id(&vault, unfinished.id, 300).unwrap().unwrap();
        persist_index_cursor(
            &vault,
            unfinished.id,
            &IndexBackfillCursor {
                next_history_id: 317,
                embedded_so_far: 316,
                scan_target: 700,
                embed_target: 700,
            },
            None,
        )
        .unwrap();
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let reopened = Vault::open(paths, config, key).unwrap();
        assert_eq!(load_ai_job(&reopened, unfinished.id).unwrap().state, "queued");
        let next = claim_next_ai_job(&reopened, 300).unwrap().unwrap();
        assert_eq!(next.id, unfinished.id);
        let AiJobPayload::Index { cursor, .. } = next.payload else { panic!("index") };
        assert_eq!(cursor.next_history_id, 317);
        assert!(claim_next_ai_job(&reopened, 300).unwrap().is_none());
        assert_eq!(load_ai_job(&reopened, finished.id).unwrap().state, "succeeded");
    }
}

#[test]
fn queue_pages_reach_every_job_and_a_cancel_cannot_become_success() {
    for key in [None, Some([63; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        for _ in 0..37 {
            enqueue_index_job(&vault, &AiIndexRequest::default(), false).unwrap();
        }
        let job = claim_next_ai_job(&vault, 300).unwrap().unwrap();
        cancel_ai_job(&vault, job.id).unwrap();
        assert!(!mark_ai_job_succeeded(&vault, job.id, None, None).unwrap());
        let mut cursor = None;
        let mut ids = std::collections::BTreeSet::new();
        loop {
            let page = list_ai_jobs(&vault, cursor, 7).unwrap();
            for job in page.items {
                assert!(ids.insert(job.id));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(ids.len(), 37);
    }
}

#[test]
fn process_exit_requeues_inflight_claim_and_preserves_completed_job() {
    if let Ok(root) = std::env::var("PATHKEEP_AI_QUEUE_CRASH_CHILD") {
        let key = (std::env::var("PATHKEEP_AI_QUEUE_CRASH_KEY").as_deref() == Ok("1"))
            .then_some([73; 32]);
        let vault = Vault::open(
            vault_core::project_paths_with_root(std::path::Path::new(&root)),
            Default::default(),
            key,
        )
        .unwrap();
        let done = enqueue_index_job(&vault, &Default::default(), false).unwrap();
        claim_ai_job_by_id(&vault, done.id, 300).unwrap().unwrap();
        mark_ai_job_succeeded(&vault, done.id, None, None).unwrap();
        let unfinished = enqueue_index_job(&vault, &Default::default(), false).unwrap();
        claim_ai_job_by_id(&vault, unfinished.id, 300).unwrap().unwrap();
        persist_index_cursor(
            &vault,
            unfinished.id,
            &IndexBackfillCursor {
                next_history_id: 6001,
                embedded_so_far: 6000,
                scan_target: 70_000,
                embed_target: 70_000,
            },
            None,
        )
        .unwrap();
        std::process::exit(23);
    }
    for key in [None, Some([73; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.store().close().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_exit_requeues_inflight_claim_and_preserves_completed_job",
                "--nocapture",
            ])
            .env("PATHKEEP_AI_QUEUE_CRASH_CHILD", &paths.app_root)
            .env("PATHKEEP_AI_QUEUE_CRASH_KEY", if key.is_some() { "1" } else { "0" })
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(23));
        let reopened = Vault::open(paths, config, key).unwrap();
        let job = claim_next_ai_job(&reopened, 300).unwrap().unwrap();
        assert_eq!(job.id, 2);
        let AiJobPayload::Index { cursor, .. } = job.payload else { panic!("index claim") };
        assert_eq!(cursor.next_history_id, 6001);
        assert_eq!(load_ai_job(&reopened, 1).unwrap().state, "succeeded");
        assert!(claim_next_ai_job(&reopened, 300).unwrap().is_none());
    }
}
