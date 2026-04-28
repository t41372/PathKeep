//! Regression coverage for persistent AI queue storage.
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

use super::*;

/// Builds the minimum canonical tables needed by queue jobs that reference runs.
fn connection() -> Connection {
    let connection = Connection::open_in_memory().expect("open in-memory db");
    connection
        .execute_batch(
            "CREATE TABLE runs (
               id INTEGER PRIMARY KEY AUTOINCREMENT
             );
             INSERT INTO runs (id) VALUES (42);",
        )
        .expect("create runs table");
    ensure_ai_queue_schema(&connection).expect("ensure queue schema");
    connection
}

/// Protects the shell-facing status counts and recent-job ordering used by Settings and Jobs.
#[test]
fn enqueue_and_load_queue_status_tracks_counts_and_recent_jobs() {
    let connection = connection();
    enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue index");
    enqueue_assistant_job(
        &connection,
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

    let status = load_ai_queue_status(&connection, true, 2, 8).expect("status");
    assert!(status.paused);
    assert_eq!(status.concurrency, 2);
    assert_eq!(status.queued, 2);
    assert_eq!(status.running, 0);
    assert_eq!(status.failed, 0);
    assert_eq!(status.recent_jobs.len(), 2);
    assert_eq!(status.recent_jobs[0].state, "paused");
}

/// Locks down the ordinary success lifecycle from queued work through persisted run metadata.
#[test]
fn claim_and_complete_job_advances_lifecycle() {
    let connection = connection();
    let queued =
        enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);
    heartbeat_ai_job(&connection, claimed.id).expect("heartbeat");

    assert!(
        mark_ai_job_succeeded(&connection, claimed.id, Some(42), Some("Index refreshed"))
            .expect("succeeded"),
        "success transition should win while no cancellation is pending"
    );
    let finished = load_ai_job(&connection, claimed.id).expect("load finished job");
    assert_eq!(finished.state, "succeeded");
    assert_eq!(finished.run_id, Some(42));
    assert_eq!(finished.summary.as_deref(), Some("Index refreshed"));
    assert!(finished.finished_at.is_some());
}

/// Ensures retryable failures respect the attempt budget before becoming terminal.
#[test]
fn retryable_failures_requeue_until_attempt_budget_is_exhausted() {
    let connection = connection();
    let queued =
        enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);

    let retried = mark_ai_job_failed(
        &connection,
        claimed.id,
        None,
        &AiJobFailure {
            error_code: Some("network-error".to_string()),
            error_message: "connection timed out".to_string(),
            retryable: true,
            retry_after_seconds: 5,
            summary: Some("Will retry after timeout".to_string()),
        },
        false,
    )
    .expect("mark retryable failure");
    assert_eq!(retried.state, "queued");
    assert_eq!(retried.error_code.as_deref(), Some("network-error"));

    connection
        .execute(
            "UPDATE ai_jobs SET available_at = ?1 WHERE id = ?2",
            params![now_rfc3339(), retried.id],
        )
        .expect("release retry delay");
    let claimed_again =
        claim_next_ai_job(&connection, 60).expect("claim second attempt").expect("second job");
    assert_eq!(claimed_again.attempt, 2);

    let retried_again = mark_ai_job_failed(
        &connection,
        claimed_again.id,
        None,
        &AiJobFailure {
            error_code: Some("network-error".to_string()),
            error_message: "connection timed out".to_string(),
            retryable: true,
            retry_after_seconds: 5,
            summary: Some("Retry budget exhausted".to_string()),
        },
        false,
    )
    .expect("mark second retry");
    assert_eq!(retried_again.state, "queued");
    connection
        .execute(
            "UPDATE ai_jobs SET available_at = ?1 WHERE id = ?2",
            params![now_rfc3339(), retried_again.id],
        )
        .expect("release final retry delay");
    let final_claim =
        claim_next_ai_job(&connection, 60).expect("claim final attempt").expect("final job");
    assert_eq!(final_claim.attempt, 3);

    let terminal = mark_ai_job_failed(
        &connection,
        final_claim.id,
        None,
        &AiJobFailure {
            error_code: Some("network-error".to_string()),
            error_message: "connection timed out".to_string(),
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
    let connection = connection();
    let queued = enqueue_assistant_job(
        &connection,
        &AiAssistantRequest {
            question: "Summarize MCP research".to_string(),
            profile_id: None,
            domain: None,
        },
        "llm-primary",
        Some("embed-primary"),
        false,
    )
    .expect("enqueue assistant");
    let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);
    connection
        .execute(
            "UPDATE ai_jobs SET heartbeat_at = ?1 WHERE id = ?2",
            params!["2000-01-01T00:00:00+00:00", claimed.id],
        )
        .expect("age heartbeat");

    let reclaimed = claim_next_ai_job(&connection, 1).expect("claim stale").expect("stale job");
    assert_eq!(reclaimed.id, claimed.id);
    let failed = mark_ai_job_failed(
        &connection,
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
    let replayed = replay_ai_job(&connection, failed.id, true).expect("replay");
    assert_eq!(replayed.state, "paused");
    let cancelled = cancel_ai_job(&connection, replayed.id).expect("cancel");
    assert_eq!(cancelled.state, "cancelled");
}

/// Ensures cancellation remains cooperative while a worker owns the running job.
#[test]
fn running_cancel_sets_stop_request_until_worker_finishes_cancel() {
    let connection = connection();
    let queued =
        enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed = claim_next_ai_job(&connection, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);

    let running = cancel_ai_job(&connection, claimed.id).expect("request cancellation");
    assert_eq!(running.state, "running");
    assert!(ai_job_stop_requested(&connection, claimed.id).expect("stop flag"));

    let cancelled =
        mark_running_ai_job_cancelled(&connection, claimed.id, Some("Cancelled while running."))
            .expect("finalize cancellation");
    assert_eq!(cancelled.state, "cancelled");
    assert_eq!(cancelled.summary.as_deref(), Some("Cancelled while running."));
}

/// Guards targeted job claims plus pause/resume controls used by manual queue review.
#[test]
fn claim_by_id_and_pause_resume_cover_targeted_orchestration_paths() {
    let connection = connection();
    let first = enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("first");
    let second = enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("second");

    let claimed =
        claim_ai_job_by_id(&connection, second.id, 60).expect("claim by id").expect("job");
    assert_eq!(claimed.id, second.id);
    assert_eq!(claimed.attempt, 1);

    let untouched = load_ai_job(&connection, first.id).expect("first remains queued");
    assert_eq!(untouched.state, "queued");

    let paused = pause_queued_jobs(&connection).expect("pause queued jobs");
    assert_eq!(paused, 1);
    assert_eq!(load_ai_job(&connection, first.id).expect("paused job").state, "paused");

    let resumed = resume_paused_jobs(&connection).expect("resume paused jobs");
    assert_eq!(resumed, 1);
    assert_eq!(load_ai_job(&connection, first.id).expect("resumed job").state, "queued");
    assert!(
        claim_ai_job_by_id(&connection, 9_999, 60).expect("missing job is not claimable").is_none()
    );
}

/// Protects the compare-and-set claim path that prevents duplicate worker ownership.
#[test]
fn compare_and_set_claim_prevents_double_claims() {
    let root = tempfile::tempdir().expect("tempdir");
    let database_path = root.path().join("ai-queue.sqlite");
    let first_connection = Connection::open(&database_path).expect("open first db");
    let second_connection = Connection::open(&database_path).expect("open second db");
    first_connection
        .execute_batch("CREATE TABLE runs (id INTEGER PRIMARY KEY AUTOINCREMENT);")
        .expect("create runs table");
    ensure_ai_queue_schema(&first_connection).expect("ensure schema");
    enqueue_index_job(&first_connection, &AiIndexRequest::default(), false).expect("enqueue");

    let first_claim = claim_next_ai_job(&first_connection, 60).expect("first claim");
    let second_claim = claim_next_ai_job(&second_connection, 60).expect("second claim");
    assert!(first_claim.is_some() ^ second_claim.is_some());
}

/// Covers queue failure edges that used to be invisible to the broad green test run.
#[test]
fn malformed_payloads_and_terminal_edges_stay_observable() {
    let connection = connection();
    assert_eq!(
        count_jobs_for_types(&connection, &[], &[encode_job_state(AiQueueJobState::Queued)])
            .expect("empty types count"),
        0
    );
    assert_eq!(
        count_jobs_for_types(&connection, &[AiQueueJobType::IndexBuild], &[])
            .expect("empty states count"),
        0
    );
    assert!(!ai_job_stop_requested(&connection, 99_999).expect("missing stop flag"));

    let queued =
        enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue");
    let claimed =
        claim_ai_job_by_id(&connection, queued.id, 60).expect("claim by id").expect("claimed");
    cancel_ai_job(&connection, claimed.id).expect("request stop");
    assert!(
        !mark_ai_job_succeeded(&connection, claimed.id, Some(42), Some("too late"))
            .expect("success should lose after stop request")
    );
    let cancelled =
        mark_running_ai_job_cancelled(&connection, claimed.id, Some("worker observed stop"))
            .expect("cancel running");
    assert_eq!(cancelled.state, encode_job_state(AiQueueJobState::Cancelled));
    let replayed = replay_ai_job(&connection, cancelled.id, false).expect("replay cancelled");
    assert_eq!(replayed.state, encode_job_state(AiQueueJobState::Queued));

    let paused_failure = enqueue_assistant_job(
        &connection,
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
    let claimed_failure = claim_ai_job_by_id(&connection, paused_failure.id, 60)
        .expect("claim paused failure")
        .expect("claimed paused failure");
    let paused = mark_ai_job_failed(
        &connection,
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
        enqueue_index_job(&connection, &AiIndexRequest::default(), false).expect("enqueue success");
    let claimed_success = claim_ai_job_by_id(&connection, succeeded.id, 60)
        .expect("claim success")
        .expect("claimed success");
    assert!(
        mark_ai_job_succeeded(&connection, claimed_success.id, None, Some("done"))
            .expect("mark succeeded")
    );
    let replay_error =
        replay_ai_job(&connection, succeeded.id, false).expect_err("succeeded replay is invalid");
    assert!(replay_error.to_string().contains("Only failed"));
    let cancel_error =
        cancel_ai_job(&connection, succeeded.id).expect_err("succeeded cancel is invalid");
    assert!(cancel_error.to_string().contains("cannot be cancelled"));

    let now = now_rfc3339();
    connection
        .execute(
            "INSERT INTO ai_jobs (
               job_type, state, priority, attempt, max_attempts, payload_json,
               available_at, created_at, updated_at
             )
             VALUES ('assistant', 'queued', 99, 0, 1, '{not-json', ?1, ?1, ?1)",
            params![now],
        )
        .expect("insert malformed queued job");
    let malformed_id = connection.last_insert_rowid();
    let malformed_payload =
        load_ai_job_payload(&connection, malformed_id).expect_err("malformed payload load");
    assert!(malformed_payload.to_string().contains("loading AI job payload"));
    let malformed_claim =
        claim_ai_job_by_id(&connection, malformed_id, 60).expect_err("malformed targeted claim");
    assert!(format!("{malformed_claim:#}").contains("line 1 column"));
    let malformed_next_claim =
        claim_next_ai_job(&connection, 60).expect_err("malformed next claim");
    assert!(format!("{malformed_next_claim:#}").contains("line 1 column"));

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
