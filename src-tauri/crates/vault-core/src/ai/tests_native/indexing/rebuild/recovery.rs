//! Responsibilities: reproduce optional-index recovery against real files and Turso.
//! Not responsible for: frontend or embedding transport. Failure cases are recorded in
//! semantic-index-control.md before these tests and their implementation.
use super::*;
use crate::ai::generations;

#[test]
fn unavailable_generation_degrades_status_and_search_and_cannot_authorize_gc() {
    for damage in ["missing", "metadata", "plane", "truncated", "map"] {
        let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
        let serving = lease(&vault, &provider);
        let serving_path = serving.paths().vectors_dir.clone();
        drop(serving);
        let build =
            generations::BuildGeneration::open(&vault, &provider, None, 998001, true, false)
                .unwrap();
        let retained = build.output().vectors_dir.parent().unwrap().to_owned();
        drop(build);
        match damage {
            "missing" => std::fs::remove_dir_all(&serving_path).unwrap(),
            "metadata" => std::fs::write(serving_path.join("serving.json"), "torn").unwrap(),
            _ => {
                let mut paths = vault.paths().clone();
                paths.vectors_dir = serving_path;
                let plane = crate::ai::vector_planes::Int8Plane::for_provider(
                    &paths,
                    &provider.config.id,
                    &provider.config.default_model,
                );
                if damage == "truncated" {
                    let file = std::fs::OpenOptions::new().write(true).open(plane.path()).unwrap();
                    file.set_len(file.metadata().unwrap().len() - 1).unwrap();
                } else if damage == "map" {
                    let reverse = ReverseVisitMap::for_provider(
                        &paths,
                        &provider.config.id,
                        &provider.config.default_model,
                    );
                    std::fs::write(reverse.forward_path(), "torn").unwrap();
                } else {
                    std::fs::remove_file(plane.path()).unwrap();
                }
            }
        }
        assert!(
            generations::resolve(&vault, &provider.config.id, &provider.config.default_model)
                .unwrap()
                .is_none()
        );
        assert!(!ai_index_status(&vault).unwrap().ready);
        assert!(
            finds(&runtime, &vault, &provider, "kestrel").contains(&"https://example.com/a".into())
        );
        for _ in 0..2 {
            generations::collect(&vault, true).unwrap();
        }
        assert!(retained.exists(), "dangling pointers cannot justify deleting other lineages");
        for clear_only in [false, true] {
            assert!(
                runtime
                    .block_on(build_ai_index_with_control(
                        &vault,
                        &provider,
                        &AiIndexRequest { clear_only, ..Default::default() },
                        Some(Arc::new(FailBeforeSource)),
                        0,
                        0,
                        None
                    ))
                    .is_err()
            );
            generations::collect(&vault, true).unwrap();
            assert!(
                retained.exists(),
                "failed build/clear must preserve damaged-pointer recovery ownership"
            );
        }

        runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).unwrap();
        assert!(ai_index_status(&vault).unwrap().ready);
        generations::collect(&vault, true).unwrap();
        assert!(!retained.exists());
    }
}

#[test]
fn success_transaction_records_the_final_summary() {
    let (runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let job = running_rebuild_job(&vault, &provider);
    let report = runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &rebuild_request(),
            None,
            0,
            0,
            Some(JobLedger::new(job, None)),
        ))
        .unwrap();
    let job = crate::ai_queue::load_ai_job(&vault, job).unwrap();
    assert_eq!(job.state, "succeeded");
    assert_eq!(
        job.summary,
        Some(format!(
            "Indexed {} new / {} updated row(s).",
            report.indexed_items, report.updated_items
        ))
    );
}

#[test]
fn resumed_incremental_scan_restarts_when_serving_lineage_changes() {
    let (runtime, vault, provider, archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let request = AiIndexRequest {
        provider_id: Some(provider.config.id.clone()),
        limit: Some(1),
        ..Default::default()
    };
    let job = enqueue_index_job(&vault, &request, false).unwrap();
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    let paused = JobLedger::new(job.id, Some(1));
    assert!(
        runtime
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &request,
                None,
                0,
                0,
                Some(paused.clone()),
            ))
            .is_err()
    );
    let cursor = paused.progress();
    runtime.block_on(build_ai_index(&vault, &provider, &rebuild_request())).unwrap();
    archive.execute_batch("UPDATE urls SET title='albatross changed' WHERE id=1").unwrap();
    runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &request,
            None,
            cursor.next_history_id,
            cursor.embedded_so_far,
            Some(JobLedger::new(job.id, None)),
        ))
        .unwrap();
    let live = lease(&vault, &provider);
    let lineage = live.lineage().to_owned();
    assert_eq!(
        vault
            .read(Db::Intelligence, move |s| Ok(s.query_one(
                "SELECT title FROM ai_embeddings WHERE history_id=1 AND lineage=?1",
                [lineage],
                |r| Ok(r.get::<String>(0)?),
            )?))
            .unwrap(),
        "albatross changed"
    );
}

#[test]
fn torn_legacy_sources_without_projections_move_without_building_during_open() {
    use std::io::Write;
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_embedding(&connection, 1, &provider, "hash");
    seed_successful_index_ledger(&connection, &provider, 1);
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let store = VectorStore::create_stamped(&paths, &fp).unwrap();
    store.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
    let map =
        VisitContentMap::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    map.ensure_created(&paths).unwrap();
    map.append(&[(1, 11)]).unwrap();
    let search = VectorStore::for_provider(
        &paths,
        &crate::ai::search_vectors::identity_of(&provider.config.id),
        &provider.config.default_model,
    )
    .create_stamped_at(&paths, &fp, "searches".into())
    .unwrap();
    search.append_vectors(&[(33, vec![0., 0., 1.])]).unwrap();
    let clean_search = std::fs::read(search.path()).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(search.path())
        .unwrap()
        .write_all(&[1, 2, 3])
        .unwrap();
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(store.path()).unwrap().ino()
    };
    let clean = std::fs::read(store.path()).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(store.path())
        .unwrap()
        .write_all(&[1, 2, 3])
        .unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(map.path())
        .unwrap()
        .write_all(&[1, 2, 3])
        .unwrap();
    generations::migrate_flat_indexes(&connection.vault).unwrap();
    assert!(!store.exists());
    assert!(
        generations::resolve(
            &connection.vault,
            &provider.config.id,
            &provider.config.default_model
        )
        .unwrap()
        .is_none()
    );
    let dir = paths
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name(
            &provider.config.id,
            &provider.config.default_model,
        ))
        .join("legacy");
    assert_eq!(std::fs::read(dir.join(store.path().file_name().unwrap())).unwrap(), clean);
    assert_eq!(std::fs::read(dir.join(search.path().file_name().unwrap())).unwrap(), clean_search);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            std::fs::metadata(dir.join(store.path().file_name().unwrap())).unwrap().ino(),
            inode,
            "migration must retain the same inode, not copy its bytes"
        );
    }
    assert!(
        std::fs::read_dir(dir.join("g0")).unwrap().next().is_none(),
        "open cannot build projections"
    );
    generations::collect(&connection.vault, false).unwrap();
    assert!(dir.exists(), "next normal pass needs the migrated sources");
}

#[test]
fn committed_old_copy_finishes_before_flat_migration_and_uncommitted_rows_are_deferred() {
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_embedding(&connection, 1, &provider, "old");
    seed_embedding(&connection, 2, &provider, "abandoned");
    connection.execute_batch("UPDATE ai_embeddings SET indexed_at=CASE WHEN history_id=1 THEN '2026-01-01' ELSE '2026-02-01' END").unwrap();
    seed_successful_index_ledger(&connection, &provider, 1);
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let old = VectorStore::create_stamped(&paths, &fp).unwrap();
    old.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
    let committed = paths.vectors_dir.join("rebuild/job-900.publish");
    std::fs::create_dir_all(&committed).unwrap();
    let mut staged = paths.clone();
    staged.vectors_dir = committed.clone();
    let new = VectorStore::create_stamped(&staged, &fp).unwrap();
    new.append_vectors(&[(22, vec![0., 1., 0.])]).unwrap();
    let map =
        VisitContentMap::for_provider(&staged, &provider.config.id, &provider.config.default_model);
    map.ensure_created(&staged).unwrap();
    map.append(&[(1, 22)]).unwrap();
    let files = vec![
        new.path().file_name().unwrap().to_str().unwrap(),
        map.path().file_name().unwrap().to_str().unwrap(),
    ];
    let manifest = serde_json::json!({"jobId":900,"providerId":provider.config.id,"model":provider.config.default_model,"rowsSince":"2026-01-01","files":files,"sourceWatermark":1});
    std::fs::write(committed.join("manifest.json"), manifest.to_string()).unwrap();
    let obsolete = old.path().with_extension("obsolete");
    std::fs::write(&obsolete, "old projection").unwrap();
    let abandoned = paths.vectors_dir.join("rebuild/job-901");
    std::fs::create_dir_all(&abandoned).unwrap();
    let mut manifest = manifest;
    manifest["jobId"] = 901.into();
    manifest["rowsSince"] = "2026-02-01".into();
    std::fs::write(abandoned.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(abandoned.join("partial"), "uncommitted").unwrap();
    generations::migrate_flat_indexes(&connection.vault).unwrap();
    assert!(!obsolete.exists());
    assert!(!committed.exists());
    assert!(!abandoned.join("partial").exists());
    assert!(abandoned.join("manifest.json").exists(), "open defers row scans");
    let mut source = paths.clone();
    source.vectors_dir = paths
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name(
            &provider.config.id,
            &provider.config.default_model,
        ))
        .join("legacy");
    assert_eq!(
        VectorStore::for_provider(&source, &provider.config.id, &provider.config.default_model)
            .read_all()
            .unwrap()[0]
            .0,
        22
    );
    generations::collect(&connection.vault, true).unwrap();
    assert!(!abandoned.exists());
    assert_eq!(
        connection
            .query_one("SELECT COUNT(*) FROM ai_embeddings WHERE history_id=2", (), |r| Ok(
                r.get::<i64>(0)?
            ))
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_one("SELECT COUNT(*) FROM ai_embeddings WHERE history_id=1", (), |r| Ok(
                r.get::<i64>(0)?
            ))
            .unwrap(),
        1
    );
}

#[test]
fn legacy_migration_failure_does_not_fail_archive_open_and_retries() {
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_embedding(&connection, 1, &provider, "hash");
    seed_successful_index_ledger(&connection, &provider, 1);
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let store = VectorStore::create_stamped(&paths, &fp).unwrap();
    store.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
    let target = paths.vectors_dir.join(crate::ai_sidecar::provider_table_name(
        &provider.config.id,
        &provider.config.default_model,
    ));
    // A real filesystem failure in mkdir, independent of root/permission behavior.
    std::fs::write(&target, "blocks the lineage directory").unwrap();
    let config = connection.vault.config().clone();
    let _root_owner = connection.vault.clone();
    connection.vault.close().unwrap();
    drop(connection);
    let reopened = crate::Vault::open(paths.clone(), config.clone(), None).unwrap();
    assert!(store.exists());
    assert!(
        generations::resolve(&reopened, &provider.config.id, &provider.config.default_model)
            .unwrap()
            .is_none()
    );
    reopened.close().unwrap();
    drop(reopened);
    std::fs::remove_file(target).unwrap();
    let retried = crate::Vault::open(paths, config, None).unwrap();
    assert!(!store.exists());
    retried.close().unwrap();
}

#[test]
fn resumed_legacy_destination_wins_over_a_stale_flat_duplicate() {
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_embedding(&connection, 1, &provider, "hash");
    seed_successful_index_ledger(&connection, &provider, 1);
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let dir = paths
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name(
            &provider.config.id,
            &provider.config.default_model,
        ))
        .join("legacy");
    let mut source = paths.clone();
    source.vectors_dir = dir.clone();
    let store = VectorStore::create_stamped(&source, &fp).unwrap();
    store.append_vectors(&[(22, vec![0., 1., 0.])]).unwrap();
    let before = std::fs::read(store.path()).unwrap();
    let flat =
        VectorStore::for_provider(&paths, &provider.config.id, &provider.config.default_model);
    std::fs::write(flat.path(), "stale invalid source must not be read").unwrap();
    generations::migrate_flat_indexes(&connection.vault).unwrap();
    assert!(!flat.exists());
    assert_eq!(std::fs::read(store.path()).unwrap(), before);
    generations::migrate_flat_indexes(&connection.vault).unwrap();
    assert_eq!(std::fs::read(store.path()).unwrap(), before);
}

#[test]
fn paused_mixed_job_summarizes_new_and_updated_rows_across_both_attempts() {
    let (runtime, vault, provider, archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    archive.execute_batch("UPDATE urls SET title='albatross changed' WHERE id=1").unwrap();
    seed_visit(&archive, 4, "chrome:Default", "https://example.com/new", Some("nebula comet"), 4);
    let request = AiIndexRequest {
        provider_id: Some(provider.config.id.clone()),
        limit: Some(1),
        ..Default::default()
    };
    let job = enqueue_index_job(&vault, &request, false).unwrap();
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    let paused = JobLedger::new(job.id, Some(1));
    assert!(
        runtime
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &request,
                None,
                0,
                0,
                Some(paused.clone())
            ))
            .is_err()
    );
    let progress = paused.progress();
    let report = runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &request,
            None,
            progress.next_history_id,
            progress.embedded_so_far,
            Some(JobLedger::new(job.id, None)),
        ))
        .unwrap();
    assert_eq!((report.indexed_items, report.updated_items), (2, 1));
    assert_eq!(
        crate::ai_queue::load_ai_job(&vault, job.id).unwrap().summary.as_deref(),
        Some("Indexed 1 new / 1 updated row(s).")
    );
}

#[test]
fn simultaneous_status_resolve_and_gc_do_not_deadlock_the_two_reader_pool() {
    let (_runtime, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let (done, rx) = std::sync::mpsc::channel();
    for operation in 0..3 {
        let (vault, provider, barrier, done) =
            (vault.clone(), provider.clone(), barrier.clone(), done.clone());
        std::thread::spawn(move || {
            barrier.wait();
            for _ in 0..50 {
                match operation {
                    0 => {
                        ai_index_status(&vault).unwrap();
                    }
                    1 => {
                        drop(
                            generations::resolve(
                                &vault,
                                &provider.config.id,
                                &provider.config.default_model,
                            )
                            .unwrap(),
                        );
                    }
                    _ => {
                        generations::collect(&vault, false).unwrap();
                    }
                }
            }
            done.send(()).unwrap();
        });
    }
    barrier.wait();
    for _ in 0..3 {
        rx.recv_timeout(std::time::Duration::from_secs(15))
            .expect("status/resolve/lease drop/GC must all finish");
    }
}

#[test]
fn stopped_initial_fills_serve_partial_and_next_job_embeds_only_missing_visits() {
    for stop in ["pause", "cancel", "failure"] {
        let runtime = Runtime::new().unwrap();
        let (_, _, connection) = prepared_archive();
        let config = static_config(DEFAULT_STATIC_MODEL_REPO);
        let vault = connection.vault.with_config(config.clone());
        let provider = provider_for(&config);
        for id in 1..=3 {
            seed_visit(
                &connection,
                id,
                "chrome:Default",
                &format!("https://example.com/{id}"),
                Some(if id == 1 { "kestrel falcon" } else { "banana zebra" }),
                id,
            );
        }
        let request = AiIndexRequest {
            provider_id: Some(provider.config.id.clone()),
            limit: Some(1),
            ..Default::default()
        };
        let job = enqueue_index_job(&vault, &request, false).unwrap();
        claim_next_ai_job(&vault, 300).unwrap().unwrap();
        let control =
            Arc::new(InitialStop { vault: vault.clone(), job: job.id, kind: stop.into() });
        assert!(
            runtime
                .block_on(build_ai_index_with_control(
                    &vault,
                    &provider,
                    &request,
                    None,
                    0,
                    0,
                    Some(control)
                ))
                .is_err()
        );
        if stop == "pause" {
            crate::ai_queue::pause_ai_job(&vault, job.id).unwrap();
        }
        if stop != "failure" {
            mark_running_ai_job_cancelled(&vault, job.id, None).unwrap();
        } else {
            crate::ai_queue::mark_ai_job_failed(
                &vault,
                job.id,
                None,
                &crate::ai_queue::AiJobFailure {
                    error_message: "fixture local error".into(),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        }
        generations::collect(&vault, true).unwrap();
        let partial = lease(&vault, &provider);
        let lineage = partial.lineage().to_owned();
        assert_eq!(partial.metadata().unwrap().vector_count, 1);
        assert!(
            finds(&runtime, &vault, &provider, "kestrel").contains(&"https://example.com/1".into())
        );
        let status = ai_index_status(&vault).unwrap();
        assert_eq!(status.indexed_items, 1);
        assert!(status.ready);
        assert!(!crate::semantic_index::load_state(&vault).unwrap().chosen_complete);
        if stop == "pause" {
            cancel_ai_job(&vault, job.id).unwrap();
        }
        let report = runtime
            .block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default()))
            .unwrap();
        assert_eq!(report.indexed_items, 2, "only the two missing visits may be embedded again");
        assert_eq!(lease(&vault, &provider).lineage(), lineage);
        assert!(crate::semantic_index::load_state(&vault).unwrap().chosen_complete);
    }
}
struct InitialStop {
    vault: crate::Vault,
    job: i64,
    kind: String,
}
impl IndexBackfillLedger for InitialStop {
    fn job_id(&self) -> Option<i64> {
        Some(self.job)
    }
    fn record(&self, progress: IndexBackfillProgress) -> Result<()> {
        if progress.next_history_id > 1 {
            if self.kind == "cancel" {
                cancel_ai_job(&self.vault, self.job)?;
            }
            if self.kind == "failure" {
                anyhow::bail!("fixture local failure after durable chunk")
            }
            return Err(AiRunCancelled::new("fixture stopped initial fill").into());
        }
        Ok(())
    }
}

struct FailBeforeSource;
impl AiRunControl for FailBeforeSource {
    fn checkpoint(&self, _detail: &str) -> Result<()> {
        anyhow::bail!("fixture failure before source work")
    }
}

#[test]
fn interrupted_totals_only_legacy_move_retains_sources_until_retry() {
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    seed_embedding(&connection, 1, &provider, "hash");
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let store = VectorStore::create_stamped(&paths, &fp).unwrap();
    store.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
    assert!(generations::interrupt_after_move(&connection.vault).is_err());
    let dir = paths
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name(
            &provider.config.id,
            &provider.config.default_model,
        ))
        .join("legacy");
    generations::collect(&connection.vault, true).unwrap();
    assert!(dir.join(store.path().file_name().unwrap()).exists());
    generations::migrate_flat_indexes(&connection.vault).unwrap();
    assert!(dir.join(store.path().file_name().unwrap()).exists());
}

#[cfg(unix)]
#[test]
fn legacy_ancestor_barrier_failure_never_unlinks_the_flat_source() {
    use std::os::unix::fs::MetadataExt;
    // Crash contract: every newly-created ancestor must be durable before the first source
    // rename/unlink. Fail each of the four barriers; retry must preserve the original inode.
    for barrier in 0..4 {
        let (paths, _, connection) = prepared_archive();
        let provider = embedding_provider();
        seed_embedding(&connection, 1, &provider, "hash");
        let fp = EmbeddingFingerprint::new(
            provider.config.id.clone(),
            provider.config.default_model.clone(),
            3,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Unknown,
            None,
        );
        let store = VectorStore::create_stamped(&paths, &fp).unwrap();
        store.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
        let inode = std::fs::metadata(store.path()).unwrap().ino();
        let mut faults = vec![None; barrier];
        faults.push(Some(libc::EIO));
        crate::durable_io::inject_fsync_faults(faults);
        assert!(generations::migrate_flat_indexes(&connection.vault).is_err());
        assert!(store.exists(), "ancestor barrier {barrier} must precede any source rename");
        generations::migrate_flat_indexes(&connection.vault).unwrap();
        let destination = paths
            .vectors_dir
            .join(crate::ai_sidecar::provider_table_name(
                &provider.config.id,
                &provider.config.default_model,
            ))
            .join("legacy")
            .join(store.path().file_name().unwrap());
        assert_eq!(std::fs::metadata(destination).unwrap().ino(), inode);
        assert!(!store.exists());
    }
}

fn write_legacy_identity(paths: &ProjectPaths, provider: &AiProviderRuntime) -> std::path::PathBuf {
    let dir = paths
        .vectors_dir
        .join(crate::ai_sidecar::provider_table_name(
            &provider.config.id,
            &provider.config.default_model,
        ))
        .join("legacy");
    std::fs::create_dir_all(&dir).unwrap();
    let identity = serde_json::json!({"provider_id":provider.config.id,"model":provider.config.default_model,"job_id":null,"source_watermark":0,"upper_history_id":0,"sources_valid":true});
    std::fs::write(dir.join("identity.json"), identity.to_string()).unwrap();
    dir
}

fn ledger_row(connection: &TestDatabase) -> (String, String, Option<String>) {
    connection
        .read(|s| {
            Ok(s.query_one(
                "SELECT sidecar_table,state,failure_reason FROM ai_index_ledger",
                (),
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<Option<String>>(2)?)),
            )?)
        })
        .unwrap()
}

/// Failure modes: v0.4.0 projected planes only at the end of a pass, so a stopped pass left more
/// source vectors than projected ones; the move publishes the source count, every reader rejects
/// it, and semantic search is gone after upgrading, with the "was ready" fact lost.
#[test]
fn a_pass_v040_stopped_keeps_serving_the_planes_it_had_projected() {
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    let (id, model) = (&provider.config.id, &provider.config.default_model);
    seed_embedding(&connection, 1, &provider, "hash");
    seed_successful_index_ledger(&connection, &provider, 1);
    let fp = EmbeddingFingerprint::new(
        id.clone(),
        model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let store = VectorStore::create_stamped(&paths, &fp).unwrap();
    store.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
    let map = VisitContentMap::for_provider(&paths, id, model);
    map.ensure_created(&paths).unwrap();
    map.append(&[(1, 11)]).unwrap();
    crate::ai::vector_planes::build_planes_from_store(&paths, id, model).unwrap();
    let hash = store.read_header().unwrap().unwrap().fingerprint_hash;
    ReverseVisitMap::for_provider(&paths, id, model)
        .build_from_visit_map(&paths, &map, &hash)
        .unwrap();
    // The next pass embedded another visit, then stopped before projecting anything.
    store.append_vectors(&[(22, vec![0., 1., 0.])]).unwrap();
    map.append(&[(2, 22)]).unwrap();

    generations::migrate_flat_indexes(&connection.vault).unwrap();
    generations::collect(&connection.vault, false).unwrap();
    let lease = generations::resolve(&connection.vault, id, model)
        .unwrap()
        .expect("the planes v0.4.0 served still serve");
    assert_eq!(lease.metadata().unwrap().vector_count, 1);
    let (_, state, note) = ledger_row(&connection);
    assert_eq!((state.as_str(), note), ("ready", None));
}

/// Failure modes: a legacy folder without flat files (a Clear whose row cleanup was cut short
/// by quitting, or one GC recreated from leftover rows) is taken for a move in progress, and
/// open publishes an empty index in its place: a cleared index comes back, and the next update
/// fails because the "serving" lineage has rows but no vectors. With the move's own note, an
/// empty source whose rows say visits were embedded is lost, not empty, and is not published.
#[test]
fn a_legacy_folder_without_vectors_is_never_published() {
    let note = generations::LEGACY_WAS_READY;
    for (state, reason) in [("cleared", None), ("incomplete", None), ("incomplete", Some(note))] {
        let (paths, _, connection) = prepared_archive();
        let provider = embedding_provider();
        seed_embedding(&connection, 1, &provider, "hash");
        seed_successful_index_ledger(&connection, &provider, 1);
        connection
            .execute(
                "UPDATE ai_index_ledger SET sidecar_table='',state=?1,failure_reason=?2",
                params![state, reason],
            )
            .unwrap();
        let dir = write_legacy_identity(&paths, &provider);
        generations::migrate_flat_indexes(&connection.vault).unwrap();
        generations::migrate_flat_indexes(&connection.vault).unwrap();
        assert_eq!(
            ledger_row(&connection),
            (String::new(), state.to_owned(), reason.map(str::to_owned)),
            "{state} {reason:?}"
        );
        if reason.is_none() {
            assert!(!dir.join("g0").exists(), "{state}");
        }
    }
}

/// Failure modes: after a Clear, the next update finds the moved v0.4.0 sources still on disk
/// (GC had to skip them while a search read them) and builds on them, bringing back the
/// vectors the user cleared.
#[test]
fn cleared_legacy_sources_are_not_reused_by_the_next_build() {
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    write_legacy_identity(&paths, &provider);
    connection
        .execute(
            "INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state) VALUES(?1,?2,'','v','cleared')",
            params![provider.config.id.as_str(), provider.config.default_model.as_str()],
        )
        .unwrap();
    let build =
        generations::BuildGeneration::open(&connection.vault, &provider, None, 7, false, false)
            .unwrap();
    assert_eq!(build.lineage(), "r7");
}

/// Failure modes: a build that meets GC holding its lineage fails for good ("lineage cleanup is
/// in progress" ends a queued job as needing manual review), or proceeds while GC decides.
#[test]
fn a_build_waits_for_gc_to_release_its_lineage_instead_of_failing() {
    let (paths, _, connection) = prepared_archive();
    let provider = embedding_provider();
    let vault = connection.vault.clone();
    // Moved and not yet served: GC reserves it, finds the note and keeps it.
    let dir = write_legacy_identity(&paths, &provider);
    connection
        .execute(
            "INSERT INTO ai_index_ledger(provider_id,model,sidecar_table,index_version,state,failure_reason) VALUES(?1,?2,'','v','incomplete',?3)",
            params![
                provider.config.id.as_str(),
                provider.config.default_model.as_str(),
                generations::LEGACY_NEEDS_PASS
            ],
        )
        .unwrap();
    let (waiting, waited) = std::sync::mpsc::channel();
    vault.vector_generations.set_test_hook("build_waits_for_gc", move || waiting.send(()).unwrap());
    let (done, result) = std::sync::mpsc::channel();
    let (building, pair) = (vault.clone(), provider.clone());
    vault.vector_generations.set_test_hook("after_reservation", move || {
        std::thread::spawn(move || {
            let build = generations::BuildGeneration::open(&building, &pair, None, 7, false, false);
            done.send(build.map(|b| b.lineage().to_owned()).map_err(|e| format!("{e:#}"))).unwrap();
        });
        waited.recv_timeout(std::time::Duration::from_secs(10)).expect("the build waits for GC");
    });
    generations::collect(&vault, true).unwrap();
    assert_eq!(result.recv().unwrap(), Ok("legacy".to_owned()));
    assert!(dir.join("identity.json").exists());
}

/// A torn resume can append a key the lineage source already holds. The planes keep one record
/// per key, so a count taken from the source would not match them, readers would reject the
/// generation, and pointer repair would clear it: every visit embedded again at the next build.
#[test]
fn a_source_holding_a_key_twice_still_publishes_a_generation_readers_accept() {
    let (runtime, vault, provider, archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
    let source = source_paths(&vault, &provider);
    let store = crate::ai::VectorStore::for_provider(
        &source,
        &provider.config.id,
        &provider.config.default_model,
    );
    let mut records = Vec::new();
    store
        .read_records_streaming(|key, vector| {
            records.push((key, vector.to_vec()));
            Ok(())
        })
        .unwrap();
    store.append_vectors(&records[..1]).unwrap();
    assert_eq!(store.count().unwrap(), records.len() as u64 + 1);
    seed_visit(&archive, 4, "chrome:Default", "https://example.com/d", Some("velvet comet"), 4);
    runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).unwrap();
    let serving = lease(&vault, &provider);
    assert_eq!(serving.metadata().unwrap().vector_count, records.len() as u64 + 1);
    drop(serving);
    generations::collect(&vault, true).unwrap();
    assert!(
        generations::resolve(&vault, &provider.config.id, &provider.config.default_model)
            .unwrap()
            .is_some(),
        "pointer repair keeps a generation readers accept"
    );
    assert!(
        finds(&runtime, &vault, &provider, "kestrel").contains(&"https://example.com/a".into())
    );
}

/// A rebuild asked for while an initial fill is paused, and itself stopped after one chunk, must
/// not take the place of the partial index that covers more. That one keeps serving; the
/// rebuild's lineage waits for its job.
#[test]
fn a_rebuild_stopped_after_one_chunk_keeps_the_partial_that_covers_more() {
    let runtime = Runtime::new().unwrap();
    let (_, _, connection) = prepared_archive();
    let config = static_config(DEFAULT_STATIC_MODEL_REPO);
    let vault = connection.vault.with_config(config.clone());
    let provider = provider_for(&config);
    for (id, title) in [(1, "kestrel falcon"), (2, "banana zebra"), (3, "quantum otter")] {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some(title),
            id,
        );
    }
    let request = AiIndexRequest {
        provider_id: Some(provider.config.id.clone()),
        limit: Some(1),
        ..Default::default()
    };
    let fill = enqueue_index_job(&vault, &request, false).unwrap();
    claim_next_ai_job(&vault, 300).unwrap().unwrap();
    assert!(
        runtime
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &request,
                None,
                0,
                0,
                Some(JobLedger::new(fill.id, Some(2)))
            ))
            .is_err()
    );
    // The worker's ending for a paused job: it stays paused, its lineage kept.
    crate::ai_queue::pause_ai_job(&vault, fill.id).unwrap();
    mark_running_ai_job_cancelled(&vault, fill.id, None).unwrap();
    generations::collect(&vault, true).unwrap();
    let partial = lease(&vault, &provider);
    let lineage = partial.lineage().to_owned();
    assert_eq!(partial.metadata().unwrap().vector_count, 2);
    drop(partial);
    cancel_ai_job(&vault, fill.id).unwrap();
    let rebuild = running_rebuild_job(&vault, &provider);
    assert!(
        runtime
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &AiIndexRequest {
                    provider_id: Some(provider.config.id.clone()),
                    ..rebuild_request()
                },
                None,
                0,
                0,
                Some(JobLedger::new(rebuild, Some(1)))
            ))
            .is_err()
    );
    crate::ai_queue::pause_ai_job(&vault, rebuild).unwrap();
    mark_running_ai_job_cancelled(&vault, rebuild, None).unwrap();
    generations::collect(&vault, true).unwrap();
    let serving = lease(&vault, &provider);
    assert_eq!(serving.lineage(), lineage);
    assert_eq!(serving.metadata().unwrap().vector_count, 2);
    assert!(finds(&runtime, &vault, &provider, "banana").contains(&"https://example.com/2".into()));
}

/// A rebuild that failed after some chunks keeps what it built while Replay can carry it on, and
/// gives it up once a later rebuild replaced the index it was building: nothing else ever removes
/// a failed job, so its index-sized files would stay for good.
#[test]
fn a_failed_rebuild_keeps_its_lineage_only_until_a_later_rebuild_replaces_it() {
    let runtime = Runtime::new().unwrap();
    let (_, _, connection) = prepared_archive();
    let config = static_config(DEFAULT_STATIC_MODEL_REPO);
    let vault = connection.vault.with_config(config.clone());
    let provider = provider_for(&config);
    for id in 1..=3 {
        seed_visit(
            &connection,
            id,
            "chrome:Default",
            &format!("https://example.com/{id}"),
            Some("kestrel falcon"),
            id,
        );
    }
    runtime.block_on(build_ai_index(&vault, &provider, &AiIndexRequest::default())).unwrap();
    let serving = lease(&vault, &provider).paths().vectors_dir.clone();
    let table = serving.parent().and_then(std::path::Path::parent).unwrap().to_path_buf();

    let failed = running_rebuild_job(&vault, &provider);
    let request =
        AiIndexRequest { provider_id: Some(provider.config.id.clone()), ..rebuild_request() };
    let stop = Arc::new(InitialStop { vault: vault.clone(), job: failed, kind: "failure".into() });
    assert!(
        runtime
            .block_on(build_ai_index_with_control(
                &vault,
                &provider,
                &request,
                None,
                0,
                0,
                Some(stop)
            ))
            .is_err()
    );
    crate::ai_queue::mark_ai_job_failed(
        &vault,
        failed,
        None,
        &crate::ai_queue::AiJobFailure {
            error_message: "fixture local error".into(),
            ..Default::default()
        },
        false,
    )
    .unwrap();
    generations::collect(&vault, true).unwrap();
    let failed_lineage = table.join(format!("j{failed}"));
    assert!(failed_lineage.exists(), "Replay can carry the failed rebuild on");

    let later = running_rebuild_job(&vault, &provider);
    runtime
        .block_on(build_ai_index_with_control(
            &vault,
            &provider,
            &request,
            None,
            0,
            0,
            Some(JobLedger::new(later, None)),
        ))
        .unwrap();
    assert_eq!(crate::ai_queue::load_ai_job(&vault, later).unwrap().state, "succeeded");
    generations::collect(&vault, true).unwrap();
    assert!(
        !failed_lineage.exists(),
        "the later rebuild replaced what the failed one was building"
    );
}
