//! Responsibilities: recover durable initial-fill bytes without a provider or a live queue job.
//! Not responsible for: transport. Failure matrix is recorded before implementation in
//! semantic-index-control.md, initial-fill interruption review.
use super::*;
use crate::ai::generations;

#[test]
fn interrupted_initial_projection_recovers_locally_and_a_clear_wins_over_old_intent() {
    for phase in ["before", "prepared", "published", "clear", "complete"] {
        let (_, vault, provider, _archive) = complete_index(DEFAULT_STATIC_MODEL_REPO);
        let old = lease(&vault, &provider);
        let pair = provider.clone();
        vault.write(Db::Intelligence,vault_store::Priority::Batch,move |s| {
            s.execute("UPDATE ai_index_ledger SET sidecar_table='',state='incomplete' WHERE provider_id=?1 AND model=?2",vault_store::params_owned![pair.config.id,pair.config.default_model])?;Ok(())
        }).unwrap();
        // Full reset has a different lineage from the old partial, and no active queue owner.
        let run = crate::ai::native_ledger::begin_ai_run(
            &vault,
            "ai_index",
            "test",
            serde_json::json!({}),
        )
        .unwrap();
        crate::ai::native_ledger::record_index_ledger_start(
            &vault,
            &provider,
            run,
            "now",
            0,
            &rebuild_request(),
        )
        .unwrap();
        let build =
            generations::BuildGeneration::open(&vault, &provider, None, run, true, false).unwrap();
        let fp = EmbeddingFingerprint::new(
            provider.config.id.clone(),
            provider.config.default_model.clone(),
            3,
            EmbeddingDtype::Float32,
            true,
            EmbeddingPooling::Unknown,
            None,
        );
        let store = VectorStore::create_stamped(build.source().paths(), &fp).unwrap();
        store.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
        let map = VisitContentMap::for_provider(
            build.source().paths(),
            &provider.config.id,
            &provider.config.default_model,
        );
        map.ensure_created(build.source().paths()).unwrap();
        map.append(&[(1, 11)]).unwrap();
        if phase == "prepared" {
            build.prepare(&provider).unwrap();
        }
        if phase == "published" {
            build.publish_partial(&provider).unwrap();
        }
        let lineage = build.lineage().to_owned();
        let pending = build.source().paths().vectors_dir.join("partial.json");
        drop(build);
        if phase == "clear" {
            vault.write(Db::Intelligence,vault_store::Priority::Batch,|s| {s.execute("UPDATE ai_index_ledger SET sidecar_table='',state='cleared',last_run_id=999999",())?;Ok(())}).unwrap();
        }
        if phase == "complete" {
            let pointer = old.relative().to_owned();
            vault.write(Db::Intelligence,vault_store::Priority::Batch,move |s| {s.execute("UPDATE ai_index_ledger SET sidecar_table=?1,state='ready',last_run_id=999999",[pointer])?;Ok(())}).unwrap();
        }
        generations::collect(&vault, false).unwrap(); // startup: keep intent, do not project
        if phase != "published" && phase != "complete" {
            assert!(
                generations::resolve(&vault, &provider.config.id, &provider.config.default_model)
                    .unwrap()
                    .is_none()
            );
        }
        generations::recover_partial_indexes(&vault).unwrap(); // local worker, no provider calls
        if phase == "complete" {
            assert_eq!(lease(&vault, &provider).relative(), old.relative());
            assert!(crate::semantic_index::load_state(&vault).unwrap().chosen_complete);
        } else if phase == "clear" {
            assert!(
                generations::resolve(&vault, &provider.config.id, &provider.config.default_model)
                    .unwrap()
                    .is_none()
            );
        } else {
            let live = lease(&vault, &provider);
            assert_eq!(live.lineage(), lineage);
            assert_eq!(live.metadata().unwrap().vector_count, 1);
            assert!(!crate::semantic_index::load_state(&vault).unwrap().chosen_complete);
        }
        assert!(!pending.exists());
        drop(old);
    }
}

fn pending_initial_fill() -> (crate::Vault, AiProviderRuntime) {
    let (_, _, connection) = prepared_archive();
    let config = static_config(DEFAULT_STATIC_MODEL_REPO);
    let vault = connection.vault.with_config(config.clone());
    let provider = provider_for(&config);
    let run =
        crate::ai::native_ledger::begin_ai_run(&vault, "ai_index", "test", serde_json::json!({}))
            .unwrap();
    crate::ai::native_ledger::record_index_ledger_start(
        &vault,
        &provider,
        run,
        "now",
        0,
        &AiIndexRequest::default(),
    )
    .unwrap();
    let build =
        generations::BuildGeneration::open(&vault, &provider, None, run, false, false).unwrap();
    let fp = EmbeddingFingerprint::new(
        provider.config.id.clone(),
        provider.config.default_model.clone(),
        3,
        EmbeddingDtype::Float32,
        true,
        EmbeddingPooling::Unknown,
        None,
    );
    let store = VectorStore::create_stamped(build.source().paths(), &fp).unwrap();
    store.append_vectors(&[(11, vec![1., 0., 0.])]).unwrap();
    let map = VisitContentMap::for_provider(
        build.source().paths(),
        &provider.config.id,
        &provider.config.default_model,
    );
    map.ensure_created(build.source().paths()).unwrap();
    map.append(&[(1, 11)]).unwrap();
    drop(build);
    (vault, provider)
}
#[test]
fn paused_queue_still_recovers_partial_locally_after_archive_reopens() {
    let (vault, provider) = pending_initial_fill();
    let mut config = vault.config().clone();
    config.ai.job_queue_paused = true;
    let paths = vault.paths().clone();
    vault.close().unwrap();
    // Keep the original temp-root owner alive while the independent reopened owner uses its files.
    let reopened = crate::Vault::open(paths, config, None).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(live) =
            generations::resolve(&reopened, &provider.config.id, &provider.config.default_model)
                .unwrap()
        {
            assert_eq!(live.metadata().unwrap().vector_count, 1);
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "local projection recovery cannot depend on queue resume or provider calls"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!crate::semantic_index::load_state(&reopened).unwrap().chosen_complete);
    reopened.close().unwrap();
    drop(vault);
}
#[test]
fn closing_during_partial_recovery_leaves_intent_and_next_owner_finishes() {
    let (vault, provider) = pending_initial_fill();
    let (entered, entered_rx) = std::sync::mpsc::channel();
    let (release, release_rx) = std::sync::mpsc::channel();
    vault.vector_generations.set_test_hook("partial_before_prepare", move || {
        entered.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    let recovering = vault.clone();
    let worker = std::thread::spawn(move || generations::recover_partial_indexes(&recovering));
    entered_rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    let closing = vault.clone();
    let closer = std::thread::spawn(move || closing.vector_generations.stop_reaper());
    while !vault.vector_generations.is_closed() {
        std::thread::yield_now();
    }
    release.send(()).unwrap();
    assert!(worker.join().unwrap().is_err());
    closer.join().unwrap();
    let mut reopened = vault.clone();
    reopened.vector_generations = Arc::default();
    generations::recover_partial_indexes(&reopened).unwrap();
    assert_eq!(lease(&reopened, &provider).metadata().unwrap().vector_count, 1);
}

#[cfg(unix)]
#[test]
fn retry_of_a_prepared_partial_repeats_barriers_before_publication() {
    let (vault, provider) = pending_initial_fill();
    vault.vector_generations.set_test_hook("before_prepared_sync", || {
        crate::durable_io::inject_fsync_faults(vec![Some(libc::ENOSPC)]);
    });
    assert!(generations::recover_partial_indexes(&vault).is_err());
    assert!(
        generations::resolve(&vault, &provider.config.id, &provider.config.default_model)
            .unwrap()
            .is_none()
    );
    // Manifest/planes now exist, but their failed barrier cannot authorize a pointer.
    vault.vector_generations.set_test_hook("before_prepared_sync", || {
        crate::durable_io::inject_fsync_faults(vec![Some(libc::EIO)]);
    });
    assert!(generations::recover_partial_indexes(&vault).is_err());
    assert!(
        generations::resolve(&vault, &provider.config.id, &provider.config.default_model)
            .unwrap()
            .is_none()
    );
    generations::recover_partial_indexes(&vault).unwrap();
    assert_eq!(lease(&vault, &provider).metadata().unwrap().vector_count, 1);
}

#[test]
fn losing_the_previous_partial_pointer_does_not_discard_newer_pending_work() {
    let (vault, provider) = pending_initial_fill();
    generations::recover_partial_indexes(&vault).unwrap();
    let previous = lease(&vault, &provider).paths().vectors_dir.clone();
    let run =
        crate::ai::native_ledger::begin_ai_run(&vault, "ai_index", "test", serde_json::json!({}))
            .unwrap();
    crate::ai::native_ledger::record_index_ledger_start(
        &vault,
        &provider,
        run,
        "now",
        0,
        &AiIndexRequest::default(),
    )
    .unwrap();
    let build =
        generations::BuildGeneration::open(&vault, &provider, None, run, false, false).unwrap();
    let store = VectorStore::for_provider(
        build.source().paths(),
        &provider.config.id,
        &provider.config.default_model,
    );
    store.append_vectors(&[(22, vec![0., 1., 0.])]).unwrap();
    VisitContentMap::for_provider(
        build.source().paths(),
        &provider.config.id,
        &provider.config.default_model,
    )
    .append(&[(2, 22)])
    .unwrap();
    drop(build);
    std::fs::remove_dir_all(previous).unwrap();
    generations::collect(&vault, false).unwrap();
    assert!(
        generations::resolve(&vault, &provider.config.id, &provider.config.default_model)
            .unwrap()
            .is_none()
    );
    generations::recover_partial_indexes(&vault).unwrap();
    assert_eq!(lease(&vault, &provider).metadata().unwrap().vector_count, 2);
}
