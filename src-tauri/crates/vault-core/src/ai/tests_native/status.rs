//! Responsibilities: native AI status regression coverage.
//! Not responsible for: search or network transport.
//! Failure modes: omitted records, stale state, non-durable progress, and incorrect provider scope.
use super::*;

#[test]
fn ai_index_status_warns_when_archive_is_missing() {
    let owner = crate::Vault::open_temp().unwrap();
    let _paths = owner.paths().clone();
    let vault = owner;
    let mut config = base_config();
    config.initialized = false;
    config.ai.mcp_enabled = true;
    config.ai.skill_enabled = true;
    config.ai.job_queue_paused = true;
    config.ai.job_queue_concurrency = 7;

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    assert!(status.enabled);
    assert!(status.assistant_enabled);
    assert!(status.mcp_enabled);
    assert!(status.skill_enabled);
    assert_eq!(status.state, "blocked");
    assert_eq!(status.llm_provider_id.as_deref(), Some("llm"));
    assert_eq!(status.embedding_provider_id.as_deref(), Some("embed"));
    assert!(status.queue_paused);
    assert_eq!(status.queue_concurrency, 7);
    assert_eq!(status.queued_jobs, 0);
    assert_eq!(status.running_jobs, 0);
    assert_eq!(status.failed_jobs, 0);
    assert!(status.recent_jobs.is_empty());
    assert_eq!(
        status.warning.as_deref(),
        Some("Initialize the archive before using AI analysis features.")
    );
    assert!(!status.ready);
    assert_eq!(status.indexed_items, 0);
    assert!(status.last_indexed_at.is_none());
}

#[test]
fn ai_index_status_reports_ready_with_existing_embeddings() {
    let (paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ready");
    // F3: readiness now requires REAL vectors on the `.pkvec` plane, not just SQLite metadata rows.
    seed_vector_planes(&paths, &provider, &[(1, 0xA1, vec![1.0, 0.0, 0.0])]);

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    assert!(status.ready);
    assert_eq!(status.state, "ready");
    assert_eq!(status.indexed_items, 1);
    assert_eq!(status.semantic_vector_count, 1, "the real .pkvec vector count is surfaced");
    assert!(status.last_indexed_at.is_some());
}

#[test]
fn ai_index_status_flags_metadata_rows_with_an_empty_vector_store() {
    // F3 (0-byte honesty): SQLite `ai_embeddings` rows exist (a build counted rows) but NO vectors
    // landed on the `.pkvec` plane. The status must NOT report "ready, N indexed" — it surfaces the
    // honest `IndexVectorsMissing` warning, a degraded state, and the real (zero) vector count.
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ghost", Some("Ghost"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ghost");
    // Deliberately do NOT seed vector planes — the dishonest "indexed N with an empty sidecar" case.

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    assert!(!status.ready, "metadata rows without vectors are NOT ready");
    assert_eq!(status.state, "degraded");
    assert_eq!(status.indexed_items, 1, "the SQLite metadata count is still reported honestly");
    assert_eq!(status.semantic_vector_count, 0, "the REAL vector count is zero");
    assert_eq!(status.warning_code, Some(AiIndexWarning::IndexVectorsMissing));
}

#[test]
fn ai_index_status_requires_initialized_archive_even_if_embeddings_exist() {
    let (_paths, mut config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ready");
    config.initialized = false;

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    assert!(!status.ready);
    assert_eq!(status.indexed_items, 1);
    assert!(status.last_indexed_at.is_some());
    assert_eq!(
        status.warning.as_deref(),
        Some("Initialize the archive before using AI analysis features.")
    );
}

#[test]
fn ai_index_status_requires_indexed_rows_and_respects_warning_gate() {
    let (_paths, config, _connection) = prepared_archive();
    let vault = _connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    assert!(!status.ready);
    assert_eq!(status.state, "empty");
    assert_eq!(status.indexed_items, 0);
    assert!(status.last_indexed_at.is_none());
    assert_eq!(
        status.warning.as_deref(),
        Some("Run Build index after configuring an embedding provider to enable semantic search.")
    );

    let mut disabled = config.clone();
    disabled.ai.enabled = false;
    let disabled_status =
        ai_index_status(&vault.with_config(disabled.clone())).expect("disabled status");
    assert!(!disabled_status.ready);
    assert_eq!(disabled_status.state, "disabled");
    assert_eq!(disabled_status.warning, None);
}

#[test]
fn ai_index_status_treats_selected_model_without_embeddings_as_empty() {
    let (_paths, mut config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let provider = embedding_provider();
    seed_visit(&connection, 1, "chrome:Default", "https://example.com/ready", Some("Ready"), 1);
    seed_embedding(&connection, 1, &provider, "hash-ready");
    config.ai.embedding_providers[0].default_model = "text-embedding-3-large".to_string();

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    assert!(!status.ready);
    assert_eq!(status.state, "empty");
    assert_eq!(status.indexed_items, 0);
    assert!(status.last_indexed_at.is_none());
}

#[test]
fn ai_index_status_covers_degraded_queued_paused_rebuilding_and_failed_states() {
    let provider = embedding_provider();

    let (_paths, mut degraded_config, _connection) = prepared_archive();
    let vault = _connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    degraded_config.ai.embedding_provider_id = None;
    let degraded =
        ai_index_status(&vault.with_config(degraded_config.clone())).expect("degraded status");
    assert_eq!(degraded.state, "degraded");
    assert!(!degraded.ready);
    assert_eq!(degraded.indexed_items, 0);
    assert!(degraded.warning.is_some());

    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    let queued = ai_index_status(&vault.with_config(config.clone())).expect("queued status");
    assert_eq!(queued.state, "queued");
    assert_eq!(queued.queued_jobs, 1);
    assert_eq!(queued.running_jobs, 0);
    assert_eq!(queued.failed_jobs, 0);
    assert_eq!(queued.recent_jobs.len(), 1);

    let (_paths, mut paused_config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    paused_config.ai.job_queue_paused = true;
    ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), true)
        .expect("enqueue paused job");
    let paused = ai_index_status(&vault.with_config(paused_config.clone())).expect("paused status");
    assert_eq!(paused.state, "paused");
    assert!(paused.queue_paused);
    assert_eq!(paused.queued_jobs, 1);

    let (_paths, mut paused_empty_config, _connection) = prepared_archive();
    let vault = _connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    paused_empty_config.ai.job_queue_paused = true;
    let paused_empty = ai_index_status(&vault.with_config(paused_empty_config.clone()))
        .expect("paused empty status");
    assert_eq!(paused_empty.state, "empty");

    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let queued_job = ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue job for rebuild");
    let running_job = ai_queue::claim_ai_job_by_id(&vault, queued_job.id, 300)
        .expect("claim job")
        .expect("running job");
    let rebuilding =
        ai_index_status(&vault.with_config(config.clone())).expect("rebuilding status");
    assert_eq!(rebuilding.state, "rebuilding");
    assert_eq!(rebuilding.running_jobs, 1);
    assert!(rebuilding.recent_jobs.iter().any(|job| job.id == running_job.id));

    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    seed_failed_index_ledger(&connection, &provider, "embedding run failed");
    let failed = ai_index_status(&vault.with_config(config.clone())).expect("failed status");
    assert_eq!(failed.state, "failed");
    assert_eq!(failed.warning.as_deref(), Some("embedding run failed"));
}

#[test]
fn ai_queue_status_reflects_config_and_recent_jobs() {
    let owner = crate::Vault::open_temp().unwrap();
    let _paths = owner.paths().clone();
    let vault = owner;
    let mut config = base_config();
    config.ai.job_queue_paused = true;
    config.ai.job_queue_concurrency = 3;
    let missing_status =
        ai_queue_status(&vault.with_config(config.clone())).expect("missing queue status");
    assert!(missing_status.paused);
    assert_eq!(missing_status.concurrency, 3);
    assert_eq!(missing_status.queued, 0);
    assert!(missing_status.recent_jobs.is_empty());

    let (_paths, mut config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    config.ai.job_queue_paused = false;
    config.ai.job_queue_concurrency = 2;
    ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    ai_queue::enqueue_assistant_job(
        &vault,
        &AiAssistantRequest {
            question: "What changed?".to_string(),
            profile_id: None,
            domain: None,
        },
        "llm",
        Some("embed"),
        true,
    )
    .expect("enqueue paused assistant job");
    let status = ai_queue_status(&vault.with_config(config.clone())).expect("queue status");
    assert!(!status.paused);
    assert_eq!(status.concurrency, 2);
    assert_eq!(status.queued, 2);
    assert_eq!(status.running, 0);
    assert_eq!(status.failed, 0);
    assert_eq!(status.recent_jobs.len(), 2);
    assert!(status.recent_jobs.iter().any(|job| job.state == "queued"));
    assert!(status.recent_jobs.iter().any(|job| job.state == "paused"));

    let (_paths, mut uninitialized, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    uninitialized.initialized = false;
    uninitialized.ai.job_queue_paused = true;
    ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue job for uninitialized queue");
    let still_default = ai_queue_status(&vault.with_config(uninitialized.clone()))
        .expect("uninitialized queue status");
    assert!(still_default.paused);
    assert_eq!(still_default.queued, 1);
    assert_eq!(still_default.recent_jobs.len(), 1);
}

/// `recoverable_ai_jobs` is the drain-kick signal that fixes the wedged-build bug: it must see
/// queued work AND a job orphaned in `running` by a dead worker, while reporting 0 for a missing /
/// uninitialized archive (no work to recover) and 0 for a job a live worker is actively heartbeating.
#[test]
fn recoverable_ai_jobs_sees_queued_and_orphaned_running_work() {
    // No archive on disk yet → nothing to recover (the early guard).
    let missing_vault = crate::Vault::open_temp().unwrap();
    let config = base_config();
    assert_eq!(
        recoverable_ai_jobs(&missing_vault.with_config(config.clone()))
            .expect("missing archive recoverable"),
        0
    );

    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);

    // An empty queue has nothing to recover.
    assert_eq!(
        recoverable_ai_jobs(&vault.with_config(config.clone())).expect("empty recoverable"),
        0
    );

    // A queued job is recoverable work (the drain should be kicked to run it).
    let queued = ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    assert_eq!(
        recoverable_ai_jobs(&vault.with_config(config.clone())).expect("queued recoverable"),
        1
    );

    // Once claimed and actively heartbeating, it is NOT recoverable (a live worker owns it) — so a
    // status poll won't spuriously kick a second drain at it.
    let claimed = ai_queue::claim_next_ai_job(&vault, 60).expect("claim").expect("job");
    assert_eq!(claimed.id, queued.id);
    assert_eq!(
        recoverable_ai_jobs(&vault.with_config(config.clone())).expect("running recoverable"),
        0
    );

    // Orphan it (the worker died → stale heartbeat). Now it is recoverable again, so the next poll
    // re-kicks the drain and the build resumes from its cursor.
    connection
        .execute(
            "UPDATE ai_jobs SET heartbeat_at = ?1 WHERE id = ?2",
            params!["2000-01-01T00:00:00+00:00", claimed.id],
        )
        .expect("age heartbeat");
    assert_eq!(
        recoverable_ai_jobs(&vault.with_config(config.clone())).expect("orphan recoverable"),
        1
    );

    // An uninitialized config does not hide durable work in the already-open Vault.
    let mut uninitialized = config.clone();
    uninitialized.initialized = false;
    assert_eq!(
        recoverable_ai_jobs(&vault.with_config(uninitialized.clone()))
            .expect("uninitialized recoverable"),
        1
    );
}

#[test]
fn reconcile_ai_queue_controls_pauses_resumes_and_noops() {
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let queued_job = ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");

    let mut paused = config.clone();
    paused.ai.job_queue_paused = true;
    reconcile_ai_queue_controls(&vault, &paused, &paused).expect("no-op reconcile");
    let unchanged = ai_queue::load_ai_queue_status(&vault, false, 1, 5).expect("load jobs");
    assert!(
        unchanged.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "queued")
    );

    reconcile_ai_queue_controls(&vault, &config, &paused).expect("pause reconcile");
    let paused_status =
        ai_queue::load_ai_queue_status(&vault, true, 1, 5).expect("load paused jobs");
    assert!(
        paused_status
            .recent_jobs
            .iter()
            .any(|job| job.id == queued_job.id && job.state == "paused")
    );

    reconcile_ai_queue_controls(&vault, &paused, &config).expect("resume reconcile");
    let resumed = ai_queue::load_ai_queue_status(&vault, false, 1, 5).expect("load resumed jobs");
    assert!(resumed.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "queued"));

    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let queued_job = ai_queue::enqueue_index_job(&vault, &AiIndexRequest::default(), false)
        .expect("enqueue queued job");
    let mut not_initialized = config.clone();
    not_initialized.initialized = false;
    let mut paused_next = config.clone();
    paused_next.initialized = false;
    paused_next.ai.job_queue_paused = true;
    reconcile_ai_queue_controls(&vault, &not_initialized, &paused_next)
        .expect("apply queue control to the open Vault");
    let skipped = ai_queue::load_ai_queue_status(&vault, false, 1, 5).expect("load skipped jobs");
    assert!(skipped.recent_jobs.iter().any(|job| job.id == queued_job.id && job.state == "paused"));
}

#[test]
fn persisted_assistant_run_loader_reports_malformed_json_columns() {
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    connection
        .execute(
            "INSERT INTO ai_assistant_runs
             (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at)
             VALUES (1, 'question', 'answer', 'llm', 'embed', '{not-json', '[]', 'now')",
            (),
        )
        .expect("insert malformed citations");
    let citation_error = load_assistant_run_response(&vault.with_config(config.clone()), 1)
        .expect_err("citations json error");
    assert!(format!("{citation_error:#}").contains("invalid"));

    connection
        .execute(
            "INSERT INTO ai_assistant_runs
             (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at)
             VALUES (2, 'question', 'answer', 'llm', 'embed', '[]', '{not-json', 'now')",
            (),
        )
        .expect("insert malformed notes");
    let notes_error = load_assistant_run_response(&vault.with_config(config.clone()), 2)
        .expect_err("notes json error");
    assert!(format!("{notes_error:#}").contains("invalid"));
}

#[test]
fn persisted_assistant_run_loader_marks_cited_answers_completed() {
    let (_paths, config, connection) = prepared_archive();
    let vault = connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let citations_json = serde_json::to_string(&vec![AiCitation {
        history_id: 42,
        profile_id: "chrome:Default".to_string(),
        url: "https://example.com/report".to_string(),
        title: Some("Example report".to_string()),
        visited_at: "2026-04-27T00:00:00Z".to_string(),
        score: Some(0.91),
        canonical_url: None,
    }])
    .expect("serialize citation");
    connection
        .execute(
            "INSERT INTO ai_assistant_runs
             (run_id, question, answer, provider_id, embedding_provider_id, citations_json, notes_json, created_at)
             VALUES (3, 'question', 'answer', 'llm', 'embed', ?1, ?2, 'now')",
            params![
                citations_json,
                serde_json::to_string(&vec!["grounded by one visit"]).expect("serialize notes")
            ],
        )
        .expect("insert cited assistant run");

    let response =
        load_assistant_run_response(&vault.with_config(config.clone()), 3).expect("load run");

    assert_eq!(response.state, "completed");
    assert_eq!(response.run_id, Some(3));
    assert_eq!(response.citations.len(), 1);
    assert_eq!(response.citations[0].history_id, 42);
    assert_eq!(response.notes, vec!["grounded by one visit"]);
}

#[test]
fn ai_index_status_surfaces_the_builtin_static_provider_for_the_ui() {
    // F1 surfacing: the status read model carries the built-in static provider's id, model repo,
    // download state, selection, and default flag so the Settings selector can render them.
    let (_paths, mut config, _connection) = prepared_archive();
    let vault = _connection.vault.clone();
    let _archive = TestDatabase::new(&vault, Db::Archive);
    let static_provider = crate::models::built_in_static_embedding_provider();
    config.ai.embedding_providers = vec![static_provider.clone()];
    config.ai.embedding_provider_id = Some(static_provider.id.clone());

    let status = ai_index_status(&vault.with_config(config.clone())).expect("status");
    let surfaced = status.static_embedding.expect("static embedding surfaced");
    assert_eq!(surfaced.provider_id, crate::models::BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID);
    assert_eq!(surfaced.model_repo, DEFAULT_STATIC_MODEL_REPO);
    assert!(!surfaced.model_downloaded, "no static model files on disk in tests");
    assert!(surfaced.selected, "the built-in static provider is the active selection");
    assert!(surfaced.is_default);

    // The empty-default-model branch resolves the repo to the canonical default.
    config.ai.embedding_providers[0].default_model = String::new();
    let status_empty =
        ai_index_status(&vault.with_config(config.clone())).expect("status empty model");
    assert_eq!(
        status_empty.static_embedding.expect("static").model_repo,
        DEFAULT_STATIC_MODEL_REPO
    );
}

#[test]
fn assistant_completion_uses_the_native_run_and_artifact_ledgers() {
    let runtime = Runtime::new().unwrap();
    for key in [None, Some([109; 32])] {
        let config = base_config();
        let vault = crate::Vault::open_temp_with_key(key).unwrap().with_config(config.clone());
        let response = runtime
            .block_on(crate::ai::assistant::answer_history_question(
                &vault,
                vault.paths(),
                &config,
                None,
                &llm_provider(),
                None,
                &AiAssistantRequest {
                    question: "What is in this empty archive?".into(),
                    profile_id: None,
                    domain: None,
                },
            ))
            .unwrap();
        let run_id = response.run_id.unwrap();
        let state = vault
            .read(Db::Archive, move |s| {
                Ok(s.query_one(
                    "SELECT status FROM runs WHERE id=?1 AND run_type='assistant'",
                    [run_id],
                    |r| Ok(r.get::<String>(0)?),
                )?)
            })
            .unwrap();
        assert_eq!(state, "success");
        let stored = crate::ai::load_assistant_run_response(&vault, run_id).unwrap();
        assert_eq!(stored.answer, response.answer);
        assert_eq!(stored.state, "insufficient-evidence");
    }
}

#[test]
fn semantic_freshness_uses_native_visibility_and_canonical_enrichment() {
    for key in [None, Some([110; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        let db = TestDatabase::new(&vault, Db::Intelligence);
        let provider = embedding_provider();
        seed_visit(&db, 1, "chrome:Default", "https://example.com/docs", Some("Docs"), 1);
        seed_embedding(&db, 1, &provider, "hash");
        db.execute("INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state,source_watermark,last_indexed_at) VALUES(?1,?2,'ai_embeddings','v1','ready',1,'2026-04-01T00:00:00Z')",params![provider.config.id,provider.config.default_model]).unwrap();
        assert_eq!(
            crate::ai::search_store::staleness(
                &vault,
                &provider.config.id,
                &provider.config.default_model
            )
            .unwrap(),
            Some(AiSemanticStaleness::Watermark)
        );
        let watermark = crate::ai::native_ledger::current_source_watermark(&vault).unwrap();
        db.execute("UPDATE ai_index_ledger SET source_watermark=?1", [watermark]).unwrap();
        db.execute("INSERT INTO page_content_enrichments(canonical_url,content_source,fetch_status,fetched_at,snippet_json,extraction_json,pipeline_version) VALUES('https://example.com/docs','readable-content','success','2026-04-05T00:00:00Z','[]','{}','test')",()).unwrap();
        assert_eq!(
            crate::ai::search_store::staleness(
                &vault,
                &provider.config.id,
                &provider.config.default_model
            )
            .unwrap(),
            Some(AiSemanticStaleness::Enrichment)
        );
        let provider = provider.clone();
        let reason = vault
            .read(Db::Intelligence, move |s| {
                crate::ai::native_ledger::semantic_index_staleness_reason(
                    s,
                    &provider.config.id,
                    &provider.config.default_model,
                    watermark,
                    watermark,
                    Some("2026-04-01T00:00:00Z"),
                )
            })
            .unwrap();
        assert_eq!(reason, Some(AiSemanticStaleness::Enrichment));
    }
}
