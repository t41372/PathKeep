//! Regression coverage for the semantic index job's lease and attempt budget.
//!
//! ## Responsibilities
//!
//! - A worker whose lease was taken learns it when it writes its cursor, so two workers
//!   never write the same vectors.
//! - A build that keeps making progress is never judged stuck by the attempt budget.
//!
//! ## Not responsible for
//!
//! - Worker-side AI job execution.
//! - Provider network behavior.

use super::*;

#[test]
fn persist_index_cursor_reports_lease_loss_when_job_left_running_state() {
    // A de-leased worker (its job was stale-swept out of `running` and reclaimed) must learn
    // its cursor write matched 0 rows so it can ABORT instead of double-writing the vector store.
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    claim_next_ai_job(&vault, 60).expect("claim").expect("running job");

    // Simulate the reclaim: another worker moved this row out of `running` (e.g. back to 'stale').
    execute(&vault, "UPDATE ai_jobs SET state = 'stale' WHERE id = ?1", [queued.id])
        .expect("reclaim lease");

    let outcome = persist_index_cursor(
        &vault,
        queued.id,
        &IndexBackfillCursor { next_history_id: 99, embedded_so_far: 50, ..Default::default() },
        Some("Embedded 50 row(s) so far."),
    )
    .expect("persist returns Ok even on lease loss");
    assert_eq!(outcome, CursorPersistOutcome::LeaseLost);
}

#[test]
fn durable_progress_resets_the_attempt_budget_so_interrupted_builds_keep_resuming() {
    // Review fix: the attempt counter bounds INTERRUPTIONS, not just failures. A build that makes
    // durable progress each run but is interrupted (app quit → stale → reclaim) MORE than max_attempts
    // times must NOT go terminal — resetting `attempt` at the durable-cursor checkpoint earns the
    // budget back, so only a build that makes NO progress across the budget is judged wedged.
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    assert_eq!(queued.max_attempts, 3);

    // Six interrupt+reclaim cycles (well past max_attempts), each advancing the durable cursor.
    for round in 1..=6_i64 {
        let claimed = claim_next_ai_job(&vault, 1)
            .expect("claim")
            .expect("a progressing job stays claimable past the raw budget");
        assert_eq!(claimed.id, queued.id);
        let cursor = IndexBackfillCursor {
            next_history_id: round * 10,
            embedded_so_far: round as u64,
            ..Default::default()
        };
        assert_eq!(
            persist_index_cursor(&vault, queued.id, &cursor, Some("progress")).expect("persist"),
            CursorPersistOutcome::Persisted,
        );
        // Durable progress reset the attempt counter back to 0.
        assert_eq!(load_ai_job(&vault, queued.id).expect("load").attempt, 0);
        // Interrupt: the job is left stale (lease lost) with its progress intact.
        execute(
            &vault,
            "UPDATE ai_jobs SET state = 'stale', heartbeat_at = NULL WHERE id = ?1",
            [queued.id],
        )
        .expect("interrupt");
    }

    // Still claimable + not terminal after far more than max_attempts interruptions, because it kept
    // making progress.
    let still = claim_next_ai_job(&vault, 1)
        .expect("claim")
        .expect("a build that keeps progressing never exhausts the budget");
    assert_eq!(still.id, queued.id);
    assert_ne!(load_ai_job(&vault, queued.id).expect("load").state, "failed");
}

fn execute(
    vault: &Vault,
    sql: &'static str,
    params: impl vault_store::IntoParams,
) -> vault_store::Result<u64> {
    let params = params.into_params().unwrap();
    vault.store().write(Db::Intelligence, Priority::Interactive, move |s| s.execute(sql, params))
}
