//! Regression coverage for persistent AI queue storage.
//! Failure modes: duplicate claims, lost cursors, overridden cancellation and hidden jobs.
//!
//! ## Responsibilities
//!
//! - Exercise queue enqueue, claim, retry, replay, cancel, and read-model transitions.
//! - Keep concurrent claim behavior covered without mixing the suite into runtime code.
//! - Provide compact in-memory fixtures for queue lifecycle tests.
//!
//! ## Not responsible for
//!
//! - Worker-side AI job execution.
//! - Provider network behavior.
//! - Core Intelligence deterministic queue recovery.
//!
//! ## Dependencies
//!
//! - The parent `ai_queue` module for private transition helpers under test.
//! - `rusqlite` in-memory databases for deterministic queue storage.
//!
//! ## Performance notes
//!
//! The suite keeps fixtures tiny; large-data guarantees for batch processing live in
//! the archive, parser, and intelligence rebuild regression suites.

use super::store::{decode_job_state, decode_job_type, encode_job_state};
use super::*;
use vault_store::params_owned as params;

/// Protects the shell-facing status counts and recent-job ordering used by Settings and Jobs.
#[test]
fn enqueue_and_load_queue_status_tracks_counts_and_recent_jobs() {
    let vault = Vault::open_temp().unwrap();
    enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue index");
    enqueue_assistant_job(
        &vault,
        &AiAssistantRequest {
            question: "What changed?".to_string(),
            profile_id: None,
            domain: None,
        },
        "llm-primary",
        Some("embed-primary"),
        true,
    )
    .expect("enqueue assistant");

    let status = load_ai_queue_status(&vault, true, 2, 8).expect("status");
    assert!(status.paused);
    assert_eq!(status.concurrency, 2);
    assert_eq!(status.queued, 2);
    assert_eq!(status.running, 0);
    assert_eq!(status.failed, 0);
    // The index-only counts must EXCLUDE the assistant job, so the
    // Smart-search build callout never reads an in-flight chat as build
    // progress. The aggregate `queued` is 2 (index + assistant), but only the
    // single index job counts toward `index_queued`.
    assert_eq!(status.index_queued, 1);
    assert_eq!(status.index_running, 0);
    assert_eq!(status.recent_jobs.len(), 2);
    assert_eq!(status.recent_jobs[0].state, "paused");
}

/// Locks down the ordinary success lifecycle from queued work through persisted run metadata.
#[test]
fn claim_and_complete_job_advances_lifecycle() {
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_next_ai_job(&vault, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);
    heartbeat_ai_job(&vault, claimed.id).expect("heartbeat");

    assert!(
        mark_ai_job_succeeded(&vault, claimed.id, Some(42), Some("Index refreshed"))
            .expect("succeeded"),
        "success transition should win while no cancellation is pending"
    );
    let finished = load_ai_job(&vault, claimed.id).expect("load finished job");
    assert_eq!(finished.state, "succeeded");
    assert_eq!(finished.run_id, Some(42));
    assert_eq!(finished.summary.as_deref(), Some("Index refreshed"));
    assert!(finished.finished_at.is_some());
}

/// Ensures retryable failures respect the attempt budget before becoming terminal.
#[test]
fn retryable_failures_requeue_until_attempt_budget_is_exhausted() {
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_next_ai_job(&vault, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);

    let retried = mark_ai_job_failed(
        &vault,
        claimed.id,
        None,
        &AiJobFailure {
            error_code: Some("network-error".to_string()),
            error_message: "vault timed out".to_string(),
            retryable: true,
            retry_after_seconds: 5,
            summary: Some("Will retry after timeout".to_string()),
        },
        false,
    )
    .expect("mark retryable failure");
    assert_eq!(retried.state, "queued");
    assert_eq!(retried.error_code.as_deref(), Some("network-error"));

    execute(
        &vault,
        "UPDATE ai_jobs SET available_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), retried.id],
    )
    .expect("release retry delay");
    let claimed_again =
        claim_next_ai_job(&vault, 60).expect("claim second attempt").expect("second job");
    assert_eq!(claimed_again.attempt, 2);

    let retried_again = mark_ai_job_failed(
        &vault,
        claimed_again.id,
        None,
        &AiJobFailure {
            error_code: Some("network-error".to_string()),
            error_message: "vault timed out".to_string(),
            retryable: true,
            retry_after_seconds: 5,
            summary: Some("Retry budget exhausted".to_string()),
        },
        false,
    )
    .expect("mark second retry");
    assert_eq!(retried_again.state, "queued");
    execute(
        &vault,
        "UPDATE ai_jobs SET available_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), retried_again.id],
    )
    .expect("release final retry delay");
    let final_claim =
        claim_next_ai_job(&vault, 60).expect("claim final attempt").expect("final job");
    assert_eq!(final_claim.attempt, 3);

    let terminal = mark_ai_job_failed(
        &vault,
        final_claim.id,
        None,
        &AiJobFailure {
            error_code: Some("network-error".to_string()),
            error_message: "vault timed out".to_string(),
            retryable: true,
            retry_after_seconds: 5,
            summary: Some("Retry budget exhausted".to_string()),
        },
        false,
    )
    .expect("mark terminal failure");
    assert_eq!(terminal.state, "failed");
    assert!(terminal.finished_at.is_some());
}

/// Covers stale-job recovery, replay, and queued cancellation as one boundary story.
#[test]
fn stale_jobs_are_reclaimed_and_replay_cancel_respect_boundaries() {
    let vault = Vault::open_temp().unwrap();
    // An INDEX job (max_attempts 3) so a single stale reclaim is still within budget — an assistant
    // job has max_attempts 1, which the retry bound terminalizes on the second claim (covered
    // separately by `stale_job_past_attempt_budget_terminates_instead_of_recycling`).
    let queued =
        enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue index");
    let claimed = claim_next_ai_job(&vault, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);
    execute(
        &vault,
        "UPDATE ai_jobs SET heartbeat_at = ?1 WHERE id = ?2",
        params!["2000-01-01T00:00:00+00:00", claimed.id],
    )
    .expect("age heartbeat");

    let reclaimed = claim_next_ai_job(&vault, 1).expect("claim stale").expect("stale job");
    assert_eq!(reclaimed.id, claimed.id);
    let failed = mark_ai_job_failed(
        &vault,
        reclaimed.id,
        None,
        &AiJobFailure {
            error_code: Some("bad-model".to_string()),
            error_message: "model missing".to_string(),
            retryable: false,
            retry_after_seconds: 0,
            summary: Some("Pick a valid model".to_string()),
        },
        false,
    )
    .expect("terminal fail");
    let replayed = replay_ai_job(&vault, failed.id, true).expect("replay");
    assert_eq!(replayed.state, "paused");
    let cancelled = cancel_ai_job(&vault, replayed.id).expect("cancel");
    assert_eq!(cancelled.state, "cancelled");
}

/// THE WEDGE REGRESSION: a job orphaned in `running` (its worker died — stale heartbeat) must be
/// visible to the drain-kick signal, or it never resumes.
///
/// The real-machine bug: an index build was claimed, its worker process died, and the job sat in
/// `running` with a 5-day-stale heartbeat. The drain pool is spawned only when there is recoverable
/// work, and the OLD signal for that was the read-model `queued` count — which counts only
/// `queued`/`paused`/`stale`, NEVER an orphaned `running` job. So `queued` stayed 0, the drain was
/// never spawned, `claim_next_ai_job` (which would `mark_stale_jobs` it and re-claim from its cursor)
/// was never called, and the build stayed wedged across every app launch. This asserts the new
/// `count_recoverable_ai_jobs` signal sees the orphan (so the kick fires) while `queued` is blind to
/// it, and that a claim then resumes it.
#[test]
fn orphaned_running_job_is_recoverable_even_though_queued_count_is_blind_to_it() {
    let vault = Vault::open_temp().unwrap();
    let queued =
        enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue index");
    let claimed = claim_next_ai_job(&vault, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);

    let job_types =
        [AiQueueJobType::IndexBuild, AiQueueJobType::IndexClear, AiQueueJobType::Assistant];

    // A HEALTHY running job (fresh heartbeat) must NOT register as recoverable — otherwise every
    // status poll would spuriously kick a drain against a job a live worker already owns.
    assert_eq!(
        count_recoverable_ai_jobs(&vault, 60).expect("recoverable healthy"),
        0,
        "a freshly-claimed, actively-heartbeating job is not recoverable work"
    );

    // Orphan it: the owning worker died, so the heartbeat goes stale.
    execute(
        &vault,
        "UPDATE ai_jobs SET heartbeat_at = ?1 WHERE id = ?2",
        params!["2000-01-01T00:00:00+00:00", claimed.id],
    )
    .expect("age heartbeat");

    // The OLD kick signal is blind: the job is still `running`, so `queued` (queued/paused/stale) is 0
    // and the drain would never have been spawned — the exact deadlock that wedged the build.
    assert_eq!(
        load_queue_job_counts(&vault, &job_types).expect("counts").queued,
        0,
        "an orphaned running job is invisible to the queued count (the wedge)"
    );

    // The NEW signal sees it, so the periodic status poll now kicks the drain.
    assert_eq!(
        count_recoverable_ai_jobs(&vault, 1).expect("recoverable orphan"),
        1,
        "an orphaned running job IS recoverable work and must trigger the drain kick"
    );

    // And once the drain runs, the claim reclaims the same job (resumes from its cursor).
    let reclaimed = claim_next_ai_job(&vault, 1).expect("claim stale").expect("stale job");
    assert_eq!(reclaimed.id, claimed.id);
}

/// `count_recoverable_ai_jobs`' third predicate (running + pending stop + dead worker) must also count
/// as recoverable, mirroring `mark_stale_jobs` branch 1 → `cancelled`: the drain has to run to move a
/// stop-requested-but-orphaned job to a terminal state instead of leaving it stuck `running` forever.
#[test]
fn count_recoverable_includes_a_stop_requested_orphan() {
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_next_ai_job(&vault, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);

    // A stop was requested, then the owning worker died (stale heartbeat AND expired lease).
    execute(
        &vault,
        "UPDATE ai_jobs
             SET stop_requested = 1, heartbeat_at = ?1, lease_expires_at = ?1
             WHERE id = ?2",
        params!["2000-01-01T00:00:00+00:00", claimed.id],
    )
    .expect("orphan with a pending stop");

    assert_eq!(
        count_recoverable_ai_jobs(&vault, 1).expect("recoverable stop-orphan"),
        1,
        "a stop-requested job orphaned by a dead worker is recoverable (drain → cancelled)"
    );
}

/// Ensures cancellation remains cooperative while a worker owns the running job.
#[test]
fn running_cancel_sets_stop_request_until_worker_finishes_cancel() {
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_next_ai_job(&vault, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);

    let running = cancel_ai_job(&vault, claimed.id).expect("request cancellation");
    assert_eq!(running.state, "running");
    assert!(ai_job_stop_requested(&vault, claimed.id).expect("stop flag"));

    let cancelled =
        mark_running_ai_job_cancelled(&vault, claimed.id, Some("Cancelled while running."))
            .expect("finalize cancellation");
    assert_eq!(cancelled.state, "cancelled");
    assert_eq!(cancelled.summary.as_deref(), Some("Cancelled while running."));
}

/// Guards targeted job claims plus pause/resume controls used by manual queue review.
#[test]
fn claim_by_id_and_pause_resume_cover_targeted_orchestration_paths() {
    let vault = Vault::open_temp().unwrap();
    let first = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("first");
    let second = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("second");

    let claimed = claim_ai_job_by_id(&vault, second.id, 60).expect("claim by id").expect("job");
    assert_eq!(claimed.id, second.id);
    assert_eq!(claimed.attempt, 1);

    let untouched = load_ai_job(&vault, first.id).expect("first remains queued");
    assert_eq!(untouched.state, "queued");

    let paused = pause_queued_jobs(&vault).expect("pause queued jobs");
    assert_eq!(paused, 1);
    assert_eq!(load_ai_job(&vault, first.id).expect("paused job").state, "paused");

    let resumed = resume_paused_jobs(&vault).expect("resume paused jobs");
    assert_eq!(resumed, 1);
    assert_eq!(load_ai_job(&vault, first.id).expect("resumed job").state, "queued");
    assert!(claim_ai_job_by_id(&vault, 9_999, 60).expect("missing job is not claimable").is_none());
}

/// Protects the compare-and-set claim path that prevents duplicate worker ownership.
#[test]
fn compare_and_set_claim_prevents_double_claims() {
    let first_connection = Vault::open_temp().unwrap();
    let second_connection = first_connection.clone();
    enqueue_index_job(&first_connection, &AiIndexRequest::default(), false).expect("enqueue");

    let first_claim = claim_next_ai_job(&first_connection, 60).expect("first claim");
    let second_claim = claim_next_ai_job(&second_connection, 60).expect("second claim");
    assert!(first_claim.is_some() ^ second_claim.is_some());
}

/// Covers queue failure edges that used to be invisible to the broad green test run.
#[test]
fn malformed_payloads_and_terminal_edges_stay_observable() {
    let vault = Vault::open_temp().unwrap();
    assert_eq!(
        count_jobs_for_types(&vault, &[], &[encode_job_state(AiQueueJobState::Queued)])
            .expect("empty types count"),
        0
    );
    assert_eq!(
        count_jobs_for_types(&vault, &[AiQueueJobType::IndexBuild], &[])
            .expect("empty states count"),
        0
    );
    assert!(!ai_job_stop_requested(&vault, 99_999).expect("missing stop flag"));

    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_ai_job_by_id(&vault, queued.id, 60).expect("claim by id").expect("claimed");
    cancel_ai_job(&vault, claimed.id).expect("request stop");
    assert!(
        !mark_ai_job_succeeded(&vault, claimed.id, Some(42), Some("too late"))
            .expect("success should lose after stop request")
    );
    let cancelled = mark_running_ai_job_cancelled(&vault, claimed.id, Some("worker observed stop"))
        .expect("cancel running");
    assert_eq!(cancelled.state, encode_job_state(AiQueueJobState::Cancelled));
    let replayed = replay_ai_job(&vault, cancelled.id, false).expect("replay cancelled");
    assert_eq!(replayed.state, encode_job_state(AiQueueJobState::Queued));

    let paused_failure = enqueue_assistant_job(
        &vault,
        &AiAssistantRequest {
            question: "Will this pause?".to_string(),
            profile_id: None,
            domain: None,
        },
        "llm-primary",
        None,
        false,
    )
    .expect("enqueue assistant");
    let claimed_failure = claim_ai_job_by_id(&vault, paused_failure.id, 60)
        .expect("claim paused failure")
        .expect("claimed paused failure");
    let paused = mark_ai_job_failed(
        &vault,
        claimed_failure.id,
        None,
        &AiJobFailure {
            error_code: Some("rate-limit".to_string()),
            error_message: "provider asked us to wait".to_string(),
            retryable: true,
            retry_after_seconds: 30,
            summary: Some("Paused after provider rate limit".to_string()),
        },
        true,
    )
    .expect("paused failure");
    assert_eq!(paused.state, encode_job_state(AiQueueJobState::Paused));

    let succeeded =
        enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue success");
    let claimed_success = claim_ai_job_by_id(&vault, succeeded.id, 60)
        .expect("claim success")
        .expect("claimed success");
    assert!(
        mark_ai_job_succeeded(&vault, claimed_success.id, None, Some("done"))
            .expect("mark succeeded")
    );
    let replay_error =
        replay_ai_job(&vault, succeeded.id, false).expect_err("succeeded replay is invalid");
    assert!(replay_error.to_string().contains("Only failed"));
    let cancel_error =
        cancel_ai_job(&vault, succeeded.id).expect_err("succeeded cancel is invalid");
    assert!(cancel_error.to_string().contains("cannot be cancelled"));

    let now = now_rfc3339();
    execute(
        &vault,
        "INSERT INTO ai_jobs (
               id, job_type, state, priority, attempt, max_attempts, payload_json,
               available_at, created_at, updated_at
             )
             VALUES (99990, 'assistant', 'queued', 99, 0, 1, '{not-json', ?1, ?1, ?1)",
        params![now],
    )
    .expect("insert malformed queued job");
    let malformed_id = 99990;
    let malformed_payload =
        load_ai_job_payload(&vault, malformed_id).expect_err("malformed payload load");
    assert!(malformed_payload.to_string().contains("loading AI job payload"));
    let malformed_claim =
        claim_ai_job_by_id(&vault, malformed_id, 60).expect_err("malformed targeted claim");
    assert!(format!("{malformed_claim:#}").contains("line 1 column"));
    // The head-of-queue claim must NOT propagate the parse error (which would
    // wedge the queue forever): it quarantines the unparseable row as `failed`
    // and keeps draining. Loop until the queue is empty of claimable rows.
    while claim_next_ai_job(&vault, 60)
        .expect("claim_next must quarantine a malformed payload instead of erroring")
        .is_some()
    {}
    let (malformed_state, malformed_error): (String, String) = query(
        &vault,
        "SELECT state, error_code FROM ai_jobs WHERE id = ?1",
        params![malformed_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("load quarantined malformed row");
    assert_eq!(malformed_state, "failed");
    assert_eq!(malformed_error, "payload-parse-error");

    assert_eq!(encode_job_state(AiQueueJobState::Running), "running");
    assert_eq!(encode_job_state(AiQueueJobState::Succeeded), "succeeded");
    assert_eq!(encode_job_state(AiQueueJobState::Failed), "failed");
    assert_eq!(encode_job_state(AiQueueJobState::Paused), "paused");
    assert_eq!(encode_job_state(AiQueueJobState::Stale), "stale");
    assert_eq!(decode_job_type("index-clear"), AiQueueJobType::IndexClear);
    assert_eq!(decode_job_type("assistant"), AiQueueJobType::Assistant);
    assert_eq!(decode_job_type("anything-else"), AiQueueJobType::IndexBuild);
    assert_eq!(decode_job_state("running"), AiQueueJobState::Running);
    assert_eq!(decode_job_state("succeeded"), AiQueueJobState::Succeeded);
    assert_eq!(decode_job_state("failed"), AiQueueJobState::Failed);
    assert_eq!(decode_job_state("paused"), AiQueueJobState::Paused);
    assert_eq!(decode_job_state("cancelled"), AiQueueJobState::Cancelled);
    assert_eq!(decode_job_state("stale"), AiQueueJobState::Stale);
    assert_eq!(decode_job_state("unknown"), AiQueueJobState::Queued);
}

#[test]
fn persist_index_cursor_rewrites_running_index_payload_and_summary() {
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    claim_next_ai_job(&vault, 60).expect("claim").expect("running job");

    let cursor =
        IndexBackfillCursor { next_history_id: 512, embedded_so_far: 480, ..Default::default() };
    let outcome =
        persist_index_cursor(&vault, queued.id, &cursor, Some("Embedded 480 row(s) so far."))
            .expect("persist cursor");
    assert_eq!(outcome, CursorPersistOutcome::Persisted);

    // The cursor is now in the payload so a restart resumes from it.
    match load_ai_job_payload(&vault, queued.id).expect("payload") {
        AiJobPayload::Index { cursor: stored, .. } => assert_eq!(stored, cursor),
        other => panic!("expected index payload, got {other:?}"),
    }
    let summary: String =
        query(&vault, "SELECT summary FROM ai_jobs WHERE id = ?1", [queued.id], |row| {
            Ok(row.get(0)?)
        })
        .expect("summary");
    assert_eq!(summary, "Embedded 480 row(s) so far.");
}

#[test]
fn persist_index_cursor_is_a_noop_for_missing_or_non_index_jobs() {
    let vault = Vault::open_temp().unwrap();
    // Missing job id → NotIndexJob (the job vanished mid-flight), NOT a lease loss.
    assert_eq!(
        persist_index_cursor(&vault, 9_999, &IndexBackfillCursor::default(), Some("ignored"))
            .expect("missing job no-op"),
        CursorPersistOutcome::NotIndexJob,
    );

    // An assistant job is left untouched (only index payloads carry a cursor).
    let assistant = enqueue_assistant_job(
        &vault,
        &AiAssistantRequest { question: "q".to_string(), profile_id: None, domain: None },
        "llm",
        None,
        false,
    )
    .expect("enqueue assistant");
    claim_next_ai_job(&vault, 60).expect("claim").expect("running assistant");
    assert_eq!(
        persist_index_cursor(
            &vault,
            assistant.id,
            &IndexBackfillCursor { next_history_id: 7, embedded_so_far: 7, ..Default::default() },
            Some("ignored"),
        )
        .expect("non-index no-op"),
        CursorPersistOutcome::NotIndexJob,
    );
    // The assistant payload is unchanged (no cursor field added).
    assert!(matches!(
        load_ai_job_payload(&vault, assistant.id).expect("payload"),
        AiJobPayload::Assistant { .. }
    ));
}

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
fn persist_index_cursor_disarms_full_rebuild_after_first_progress() {
    // Durable backstop: once the cursor leaves the origin, the stored request must no
    // longer carry `full_rebuild`, so a worker that re-claims this job never re-runs the destructive
    // wipe that would lose rows already embedded below the cursor.
    let vault = Vault::open_temp().unwrap();
    let request = AiIndexRequest { full_rebuild: true, ..AiIndexRequest::default() };
    let queued = enqueue_index_job(&vault, &request, false).expect("enqueue full rebuild");
    claim_next_ai_job(&vault, 60).expect("claim").expect("running job");

    // First durable chunk advances the cursor past the origin.
    persist_index_cursor(
        &vault,
        queued.id,
        &IndexBackfillCursor { next_history_id: 3, embedded_so_far: 2, ..Default::default() },
        None,
    )
    .expect("persist first cursor");

    match load_ai_job_payload(&vault, queued.id).expect("payload") {
        AiJobPayload::Index { request: stored, cursor } => {
            assert!(!stored.full_rebuild, "full_rebuild must be disarmed after first progress");
            assert_eq!(cursor.next_history_id, 3);
        }
        other => panic!("expected index payload, got {other:?}"),
    }
}

#[test]
fn persist_index_cursor_preserves_scan_target_across_a_resume() {
    // Change 1: the scan denominator is captured ONCE at the build's TRUE start; a resume reports
    // scan_target = 0 ("unknown"), and persist must KEEP the stored value rather than clobbering it.
    let vault = Vault::open_temp().unwrap();
    let queued = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue");
    claim_next_ai_job(&vault, 60).expect("claim").expect("running job");

    // True start: the cursor carries both captured denominators.
    persist_index_cursor(
        &vault,
        queued.id,
        &IndexBackfillCursor {
            next_history_id: 10,
            embedded_so_far: 5,
            scan_target: 100,
            embed_target: 200,
        },
        None,
    )
    .expect("persist fresh cursor");

    // Resume: both denominators reported as 0, but the stored values must survive while the watermark
    // advances.
    persist_index_cursor(
        &vault,
        queued.id,
        &IndexBackfillCursor {
            next_history_id: 40,
            embedded_so_far: 22,
            scan_target: 0,
            embed_target: 0,
        },
        None,
    )
    .expect("persist resumed cursor");

    match load_ai_job_payload(&vault, queued.id).expect("payload") {
        AiJobPayload::Index { cursor, .. } => {
            assert_eq!(cursor.next_history_id, 40);
            assert_eq!(cursor.embedded_so_far, 22);
            assert_eq!(cursor.scan_target, 100, "a resume must preserve the captured scan target");
            assert_eq!(
                cursor.embed_target, 200,
                "a resume must preserve the captured embed target"
            );
        }
        other => panic!("expected index payload, got {other:?}"),
    }
}

#[test]
fn load_ai_queue_status_surfaces_index_progress_and_none_for_assistant() {
    // Change 1: the Activity page reads a determinate index-build bar from the recent-jobs read model.
    // An index job's cursor surfaces embedded/scanned/scan_target; an assistant job surfaces None.
    let vault = Vault::open_temp().unwrap();
    let index = enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("index");
    claim_next_ai_job(&vault, 60).expect("claim").expect("running index job");
    persist_index_cursor(
        &vault,
        index.id,
        &IndexBackfillCursor {
            next_history_id: 50,
            embedded_so_far: 30,
            scan_target: 100,
            embed_target: 250,
        },
        None,
    )
    .expect("persist cursor");

    let assistant = enqueue_assistant_job(
        &vault,
        &AiAssistantRequest { question: "q".to_string(), profile_id: None, domain: None },
        "llm",
        None,
        false,
    )
    .expect("assistant");

    let status = load_ai_queue_status(&vault, false, 1, 8).expect("status");
    let index_job =
        status.recent_jobs.iter().find(|job| job.id == index.id).expect("index job in recent");
    assert_eq!(index_job.progress_embedded, Some(30));
    assert_eq!(index_job.progress_scanned, Some(50));
    assert_eq!(index_job.progress_scan_target, Some(100));
    assert_eq!(index_job.progress_embed_target, Some(250));

    let assistant_job = status
        .recent_jobs
        .iter()
        .find(|job| job.id == assistant.id)
        .expect("assistant job in recent");
    assert_eq!(assistant_job.progress_embedded, None);
    assert_eq!(assistant_job.progress_scanned, None);
    assert_eq!(assistant_job.progress_scan_target, None);
    assert_eq!(assistant_job.progress_embed_target, None);
}

#[test]
fn legacy_index_payload_without_cursor_defaults_to_start() {
    // A pre-4a payload (`{"kind":"index","request":{...}}`, NO `cursor` field) must
    // deserialize with `cursor` defaulting to the origin so an upgraded build resumes from scratch
    // rather than failing to parse (which would quarantine the job).
    let legacy = r#"{"kind":"index","request":{"providerId":null,"fullRebuild":false,"clearOnly":false,"limit":null}}"#;
    let payload: AiJobPayload =
        serde_json::from_str(legacy).expect("legacy index payload deserializes");
    match payload {
        AiJobPayload::Index { cursor, .. } => {
            assert_eq!(
                cursor,
                IndexBackfillCursor {
                    next_history_id: 0,
                    embedded_so_far: 0,
                    scan_target: 0,
                    embed_target: 0,
                }
            );
            assert_eq!(cursor, IndexBackfillCursor::default());
        }
        other => panic!("expected index payload, got {other:?}"),
    }
}

#[test]
fn stale_job_past_attempt_budget_terminates_instead_of_recycling() {
    // F2.2 (the stuck-build fix): a job that keeps going stale before it can fail gracefully must NOT
    // be re-claimed forever (the stale → running → stale cycle). Once `attempt` reaches `max_attempts`,
    // the next claim moves it to a TERMINAL failed state with an honest reason instead of claiming it.
    let vault = Vault::open_temp().unwrap();
    // A full-rebuild index job has max_attempts = 2.
    let queued = enqueue_index_job(
        &vault,
        &AiIndexRequest { full_rebuild: true, ..AiIndexRequest::default() },
        false,
    )
    .expect("enqueue");
    assert_eq!(queued.max_attempts, 2);

    // Drive it stale exactly `max_attempts` times (each claim increments `attempt`, then we age the
    // heartbeat so `mark_stale_jobs` flips it back to stale on the next claim).
    for _ in 0..2 {
        let claimed = claim_next_ai_job(&vault, 1).expect("claim").expect("claimable");
        assert_eq!(claimed.id, queued.id);
        execute(
            &vault,
            "UPDATE ai_jobs SET heartbeat_at = ?1 WHERE id = ?2",
            params!["2000-01-01T00:00:00+00:00", queued.id],
        )
        .expect("age heartbeat");
    }

    // The budget is now spent (attempt == max_attempts): the next claim TERMINATES it rather than
    // recycling — the loop is broken, and `claim_next_ai_job` finds no other runnable work.
    let reclaim = claim_next_ai_job(&vault, 1).expect("claim past budget");
    assert!(reclaim.is_none(), "an attempt-exhausted job is not re-claimed (no infinite cycle)");
    let job = load_ai_job(&vault, queued.id).expect("load job");
    assert_eq!(job.state, "failed", "the wedged job reaches a TERMINAL failed state");
    assert_eq!(job.error_code.as_deref(), Some("max-attempts-exhausted"));
    assert!(job.finished_at.is_some());
}

#[test]
fn clear_index_jobs_cancels_every_nonterminal_index_job_and_leaves_others() {
    // F2.2 reset primitive: clearing index jobs moves EVERY non-terminal index-build/index-clear job
    // to a terminal cancelled state (recovering a wedged build) while leaving terminal jobs and
    // non-index (assistant) jobs untouched.
    let vault = Vault::open_temp().unwrap();
    let queued_build =
        enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue queued build");
    // A second build, claimed into `running`, then a stale one.
    let running_build = enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue running build");
    claim_ai_job_by_id(&vault, running_build.id, 300).expect("claim").expect("running");
    // An assistant job that must SURVIVE the index reset.
    let assistant = enqueue_assistant_job(
        &vault,
        &AiAssistantRequest { question: "keep me".to_string(), profile_id: None, domain: None },
        "llm-primary",
        None,
        false,
    )
    .expect("enqueue assistant");

    // A build that failed earlier keeps its failure: the user saw it and it says what went wrong.
    let failed_build =
        enqueue_index_job(&vault, &AiIndexRequest::default(), false).expect("enqueue failed build");
    execute(&vault, "UPDATE ai_jobs SET state = 'failed' WHERE id = ?1", params![failed_build.id])
        .expect("mark failed");

    let cleared = clear_index_jobs(&vault).expect("clear");
    assert_eq!(cleared, 2, "both the queued and running index jobs were cleared");
    assert_eq!(load_ai_job(&vault, failed_build.id).expect("failed").state, "failed");
    assert_eq!(load_ai_job(&vault, queued_build.id).expect("queued").state, "cancelled");
    assert_eq!(load_ai_job(&vault, running_build.id).expect("running").state, "cancelled");
    // The assistant job is untouched (still queued and claimable).
    assert_eq!(load_ai_job(&vault, assistant.id).expect("assistant").state, "queued");
    // Re-running over an already-cleared queue is a no-op (idempotent recovery).
    assert_eq!(clear_index_jobs(&vault).expect("clear again"), 0);
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
fn query<T: Send + 'static>(
    vault: &Vault,
    sql: &'static str,
    params: impl vault_store::IntoParams,
    map: impl FnOnce(&vault_store::Row) -> vault_store::Result<T> + Send + 'static,
) -> vault_store::Result<T> {
    let params = params.into_params().unwrap();
    vault.store().read(Db::Intelligence, move |s| s.query_one(sql, params, map))
}
fn count_jobs_for_types(vault: &Vault, types: &[AiQueueJobType], states: &[&str]) -> Result<i64> {
    let types = types.to_vec();
    let states = states.iter().map(|v| v.to_string()).collect::<Vec<_>>();
    vault.read(Db::Intelligence, move |s| {
        store::count_jobs_for_types(
            s,
            &types,
            &states.iter().map(String::as_str).collect::<Vec<_>>(),
        )
    })
}
