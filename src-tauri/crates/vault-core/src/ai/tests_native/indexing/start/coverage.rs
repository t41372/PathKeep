//! Exercises visibility, persisted checkpoints and automatic admission at publication.
//! Uses real archive and queue transactions, without driving the embedding worker thread.
use super::*;
use crate::ai::{native_ledger, search_store};
use crate::ai_queue;

/// Each test visit belongs to its own reversible import. Exercise the actual undo and
/// restore entry points, including their archive counters and search-document updates.
fn seed_imports(vault: &crate::Vault, ids: std::ops::RangeInclusive<i64>) {
    seed_visits(vault, pages(ids.clone()));
    vault.write(Db::Archive, Priority::Interactive, move |s| {
        for id in ids {
            s.execute("INSERT INTO import_batches(id,source_kind,source_path,profile_id,created_at,status,summary_json,visible_items)
                VALUES(?1,'takeout','fixture','chrome:Default','2026-11-18T00:00:00Z','imported','{}',1)", [id])?;
            s.execute("UPDATE visits SET import_batch_id=?1 WHERE id=?1", [id])?;
        }
        Ok(())
    }).unwrap();
}

fn assert_freshness(vault: &crate::Vault, stale: bool) {
    let current = native_ledger::current_source_watermark(vault).unwrap();
    let reason = vault
        .read(Db::Intelligence, move |s| {
            let row = native_ledger::load_index_ledger(s, "embed", "text-embedding-3-small")?;
            if !stale {
                assert_eq!(
                    row.source_watermark, current,
                    "clean scan publishes exactly its capture"
                );
            }
            let indexed = s.query_one(
                "SELECT COALESCE(SUM(item_count),0) FROM ai_embedding_totals",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?;
            native_ledger::semantic_index_staleness_reason(s, &row, current, false, indexed)
        })
        .unwrap();
    let expected = stale.then_some(crate::AiSemanticStaleness::Watermark);
    assert_eq!(reason, expected, "status freshness");
    assert_eq!(
        search_store::staleness(vault, "embed", "text-embedding-3-small").unwrap(),
        expected,
        "search freshness"
    );
}

#[test]
fn semantic_scope_visibility_changed_while_paused_stays_stale_after_restore() {
    let runtime = Runtime::new().unwrap();
    let (_, config, connection) = prepared_archive();
    let vault = connection.vault.with_config(config.clone());
    let provider = embedding_provider();
    seed_imports(&vault, 1..=4);
    let (_, paused) = run_backfill_chunk(
        &runtime,
        &vault,
        &config,
        &provider,
        &AiIndexRequest { limit: Some(1), ..Default::default() },
        0,
    );
    crate::revert_import_batch(&vault, 3).unwrap();
    run_backfill_chunk(
        &runtime,
        &vault,
        &config,
        &provider,
        &AiIndexRequest::default(),
        paused.next_history_id,
    );
    assert_freshness(&vault, true);
    let published = vault
        .read(Db::Intelligence, |s| {
            Ok(native_ledger::load_index_ledger(s, "embed", "text-embedding-3-small")?
                .source_watermark)
        })
        .unwrap();
    assert_eq!(published, -1, "mixed visibility must remain explicitly uncertified");
    crate::restore_import_batch(&vault, 3).unwrap();
    assert_freshness(&vault, true);
}

#[test]
fn semantic_scope_same_count_swap_cannot_certify_the_wrong_visits() {
    let runtime = Runtime::new().unwrap();
    let (_, config, connection) = prepared_archive();
    let vault = connection.vault.with_config(config.clone());
    let provider = embedding_provider();
    seed_imports(&vault, 1..=4);
    crate::revert_import_batch(&vault, 2).unwrap();
    let (_, paused) = run_backfill_chunk(
        &runtime,
        &vault,
        &config,
        &provider,
        &AiIndexRequest { limit: Some(1), ..Default::default() },
        0,
    );
    crate::restore_import_batch(&vault, 2).unwrap();
    crate::revert_import_batch(&vault, 3).unwrap();
    run_backfill_chunk(
        &runtime,
        &vault,
        &config,
        &provider,
        &AiIndexRequest::default(),
        paused.next_history_id,
    );
    assert_freshness(&vault, true);
    run_backfill_chunk(&runtime, &vault, &config, &provider, &AiIndexRequest::default(), 0);
    assert_freshness(&vault, false);
}

#[test]
fn semantic_scope_empty_capture_detects_the_first_visible_visit() {
    let runtime = Runtime::new().unwrap();
    let (_, config, connection) = prepared_archive();
    let vault = connection.vault.with_config(config.clone());
    run_backfill_chunk(
        &runtime,
        &vault,
        &config,
        &embedding_provider(),
        &AiIndexRequest::default(),
        0,
    );
    assert_freshness(&vault, false);
    seed_visits(&vault, pages([1]));
    assert_freshness(&vault, true);
}

#[test]
fn semantic_scope_late_backup_admits_one_successor_then_converges() {
    for keep_automatic in [true, false] {
        late_backup(keep_automatic);
    }
}

fn late_backup(keep_automatic: bool) {
    let runtime = Runtime::new().unwrap();
    let (paths, mut config, connection) = prepared_archive();
    config.ai.auto_index_after_backup = true;
    crate::save_config(&crate::config::ControlGate::acquire(&paths), &config).unwrap();
    let vault = connection.vault.with_config(config.clone());
    let provider = embedding_provider();
    seed_visits(&vault, pages([1]));
    let request = AiIndexRequest::default();
    run_backfill_chunk(&runtime, &vault, &config, &provider, &request, 0);
    seed_visits(&vault, pages([2, 3]));
    let (_, paused) = run_backfill_chunk(
        &runtime,
        &vault,
        &config,
        &provider,
        &AiIndexRequest { limit: Some(1), ..Default::default() },
        0,
    );
    let job = crate::semantic_index::load_state(&vault).unwrap().build.unwrap().job_id;
    ai_queue::pause_ai_job(&vault, job).unwrap();
    ai_queue::mark_running_ai_job_cancelled(&vault, job, None).unwrap();
    seed_visits(&vault, pages([4]));
    assert!(ai_queue::request_index_job(&vault, &request, true).unwrap().is_none());
    assert!(ai_queue::claim_next_ai_job(&vault, 300).unwrap().is_none());
    if !keep_automatic {
        let mut changed = config.clone();
        changed.ai.auto_index_after_backup = false;
        crate::save_config(&crate::config::ControlGate::acquire(&paths), &changed).unwrap();
    }
    ai_queue::replay_ai_job(&vault, job, false).unwrap();
    let resumed = ai_queue::claim_next_ai_job(&vault, 300).unwrap().unwrap();
    assert_eq!(resumed.id, job);
    run_backfill_chunk(&runtime, &vault, &config, &provider, &request, paused.next_history_id);
    assert_freshness(&vault, true);
    let next = ai_queue::claim_next_ai_job(&vault, 300).unwrap();
    if !keep_automatic {
        assert!(next.is_none(), "current automatic-index consent wins at publication");
        return;
    }
    let next = next.expect("successor for the late visit");
    let (ids, _) =
        run_backfill_chunk(&runtime, &vault, &config, &provider, &AiIndexRequest::default(), 0);
    assert_eq!(ids, [1, 2, 3, 4]);
    assert_eq!(ai_queue::load_ai_job(&vault, next.id).unwrap().state, "succeeded");
    assert_freshness(&vault, false);
    assert!(ai_queue::claim_next_ai_job(&vault, 300).unwrap().is_none());
}

#[test]
fn semantic_scope_resume_keeps_revision_and_empty_capture_cannot_cover_restored_rows() {
    for initially_visible in [false, true] {
        let runtime = Runtime::new().unwrap();
        let (_, config, db) = prepared_archive();
        let vault = db.vault.with_config(config.clone());
        seed_imports(&vault, 1..=3);
        if !initially_visible {
            for id in 1..=3 {
                crate::revert_import_batch(&vault, id).unwrap();
            }
        }
        let provider = embedding_provider();
        let start = if initially_visible {
            let (_, paused) = run_backfill_chunk(
                &runtime,
                &vault,
                &config,
                &provider,
                &AiIndexRequest { limit: Some(1), ..Default::default() },
                0,
            );
            paused.next_history_id
        } else {
            struct StopBeforeVisits;
            impl AiRunControl for StopBeforeVisits {
                fn checkpoint(&self, detail: &str) -> Result<()> {
                    if detail.contains("before the next chunk") {
                        return Err(AiRunCancelled::new("pause before the first visit").into());
                    }
                    Ok(())
                }
            }
            let request = AiIndexRequest::default();
            let job = ai_queue::request_index_job(&vault, &request, false).unwrap().unwrap().id;
            ai_queue::claim_next_ai_job(&vault, 300).unwrap().unwrap();
            let ledger = Arc::new(PauseAfterBatch {
                job_id: job,
                progress: Default::default(),
                pause: false,
                start_history_id: 0,
            });
            let error = runtime
                .block_on(build_ai_index_with_control(
                    &vault,
                    &provider,
                    &request,
                    Some(Arc::new(StopBeforeVisits)),
                    0,
                    0,
                    Some(ledger),
                ))
                .unwrap_err();
            assert!(error.is::<AiRunCancelled>());
            0
        };
        if !initially_visible {
            for id in 2..=3 {
                crate::restore_import_batch(&vault, id).unwrap();
            }
            seed_visits(&vault, pages([4]));
        }
        ai_queue::recover_owner_claims(vault.store(), false).unwrap();
        let claimed = ai_queue::claim_next_ai_job(&vault, 300).unwrap().unwrap();
        let ai_queue::AiJobPayload::Index { cursor, .. } = claimed.payload else { unreachable!() };
        assert_eq!(cursor.next_history_id, start);
        let (_, finished) = run_backfill_chunk(
            &runtime,
            &vault,
            &config,
            &provider,
            &AiIndexRequest::default(),
            start,
        );
        let finished_scan = finished.scan.unwrap();
        assert_eq!(finished_scan.source_watermark, cursor.scan.unwrap().source_watermark);
        assert_eq!((finished_scan.examined, finished_scan.total), (3, 3));
        assert_freshness(&vault, !initially_visible);
    }
}

#[test]
fn semantic_scope_followup_drifted_total_does_not_requeue_a_stable_scan() {
    let runtime = Runtime::new().unwrap();
    for (revised, visits) in [(true, 3), (false, 3), (false, 0)] {
        let (paths, mut config, db) = prepared_archive();
        config.ai.auto_index_after_backup = true;
        crate::save_config(&crate::config::ControlGate::acquire(&paths), &config).unwrap();
        let vault = db.vault.with_config(config.clone());
        if visits > 0 {
            seed_visits(&vault, pages(1..=visits));
        }
        vault
            .write(Db::Archive, Priority::Interactive, move |s| {
                if !revised {
                    s.execute("DELETE FROM archive_totals WHERE name='visitMembershipToken'", ())?;
                }
                s.execute("UPDATE archive_totals SET value=19 WHERE name='visits'", ())?;
                Ok(())
            })
            .unwrap();
        let captured = native_ledger::current_source_watermark(&vault).unwrap();
        let (_, progress) = run_backfill_chunk(
            &runtime,
            &vault,
            &config,
            &embedding_provider(),
            &AiIndexRequest::default(),
            0,
        );
        let scan = progress.scan.unwrap();
        assert_eq!(scan.examined, visits as u64);
        assert_eq!(scan.source_watermark, captured);
        assert_freshness(&vault, false);
        assert!(ai_queue::claim_next_ai_job(&vault, 300).unwrap().is_none());
    }
}

#[test]
fn semantic_scope_followup_orphans_are_checked_and_skipped() {
    let runtime = Runtime::new().unwrap();
    let (paths, config, db) = prepared_archive();
    seed_visits(&db.vault, pages(1..=4));
    db.vault.close().unwrap();
    // Create a damaged historical fixture offline. Normal writers enforce foreign keys.
    {
        let archive = rusqlite::Connection::open(&paths.archive_db).unwrap();
        archive
            .execute_batch(
                "PRAGMA foreign_keys=OFF;
            UPDATE visits SET url_id=99999 WHERE id=2;
            UPDATE visits SET source_profile_id=99999 WHERE id=3;",
            )
            .unwrap();
    }
    let vault = crate::Vault::open(paths, config.clone(), None).unwrap();
    let (mapped, progress) = run_backfill_chunk(
        &runtime,
        &vault,
        &config,
        &embedding_provider(),
        &AiIndexRequest::default(),
        0,
    );
    let scan = progress.scan.unwrap();
    assert_eq!((scan.examined, scan.total), (4, 4));
    assert_eq!(mapped, [1, 4]);
    assert_freshness(&vault, false);
}

#[test]
fn semantic_scope_followup_text_format_precedes_mixed_coverage_in_both_readers() {
    let runtime = Runtime::new().unwrap();
    let (_, config, db) = prepared_archive();
    let vault = db.vault.with_config(config.clone());
    let provider = embedding_provider();
    seed_visits(&vault, pages([1]));
    run_backfill_chunk(&runtime, &vault, &config, &provider, &AiIndexRequest::default(), 0);
    let model = &provider.config.default_model;
    let lease =
        crate::ai::generations::resolve(&vault, &provider.config.id, model).unwrap().unwrap();
    let mut metadata = lease.metadata().unwrap();
    metadata.pages.as_mut().unwrap().embedded_text = None;
    std::fs::write(
        lease.paths().vectors_dir.join("serving.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    db.execute("UPDATE ai_index_ledger SET source_watermark=-1", ()).unwrap();
    let current = native_ledger::current_source_watermark(&vault).unwrap();
    let status = vault
        .read(Db::Intelligence, move |s| {
            let row = native_ledger::load_index_ledger(s, "embed", "text-embedding-3-small")?;
            native_ledger::semantic_index_staleness_reason(s, &row, current, true, 1)
        })
        .unwrap();
    let search = search_store::staleness(&vault, &provider.config.id, model).unwrap();
    use crate::AiSemanticStaleness::TextFormat;
    assert_eq!((status, search), (Some(TextFormat), Some(TextFormat)));
}
