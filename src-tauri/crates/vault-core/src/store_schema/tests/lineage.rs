//! Responsibilities: prove released Intelligence v1 survives a failed upgrade and v2 retains data.
//! Not responsible for: vector-file migration or application indexing.
//! Failure modes: copy loses rows/ids/totals, uniqueness still crosses lineages, failed DDL leaves
//! renamed tables or a version 2 marker, and a large copy materializes the corpus in Rust memory.
use super::*;

fn v1_config(root: &Path) -> Config {
    let mut cfg = config(root, None);
    cfg.migrations.register(Db::Intelligence, vec![super::super::intelligence::migration()]);
    cfg
}
fn seed_v1(store: &Store, count: i64) {
    for start in (1..=count).step_by(4096) {
        store.write(Db::Intelligence, Priority::Batch, move |s| {
            let mut insert = s.prepare("INSERT INTO ai_embeddings(history_id,profile_id,url,title,domain,visited_at,content_hash,content_bytes,metadata_bytes,provider_id,model,indexed_at) VALUES(?1,'chrome:Default','https://example.com/page','A page','example.com','2026-10-29T00:00:00Z','0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',128,256,'service','model','2026-10-29T00:00:00Z')")?;
            for id in start..=(start + 4095).min(count) { insert.execute([id])?; }
            Ok(())
        }).unwrap();
    }
    store.write(Db::Intelligence, Priority::Batch, move |s| {
        s.execute("INSERT INTO ai_embedding_totals VALUES('service','model',?1,?2,?3,'2026-10-29T00:00:00Z')", vault_store::params![count,count*128,count*256])?;
        Ok(())
    }).unwrap();
}
#[test]
fn intelligence_v2_keeps_every_row_and_total_and_isolates_lineages() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(v1_config(root.path())).unwrap();
    seed_v1(&store, 1025);
    drop(store);
    let store = Store::open(config(root.path(), None)).unwrap();
    store.read(Db::Intelligence, |s| {
        assert_eq!(s.schema_version()?, 2);
        assert_eq!(s.query_one("SELECT COUNT(*),SUM(id),MIN(lineage),MAX(lineage) FROM ai_embeddings", (), |r| Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<String>(2)?,r.get::<String>(3)?)))?, (1025,1025*1026/2,"legacy".into(),"legacy".into()));
        assert_eq!(s.query_one("SELECT item_count,content_bytes,metadata_bytes,lineage FROM ai_embedding_totals", (), |r| Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?,r.get::<String>(3)?)))?, (1025,1025*128,1025*256,"legacy".into()));
        Ok(())
    }).unwrap();
    store.write(Db::Intelligence, Priority::Batch, |s| {
        s.execute("INSERT INTO ai_embeddings(history_id,profile_id,url,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at,lineage) SELECT history_id,profile_id,url,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at,'j2' FROM ai_embeddings WHERE id=1", ())?;
        s.execute("INSERT INTO ai_embedding_totals SELECT provider_id,model,item_count,content_bytes,metadata_bytes,last_indexed_at,'j2' FROM ai_embedding_totals WHERE lineage='legacy'", ())?;
        Ok(())
    }).unwrap();
}
#[test]
fn intelligence_v2_failure_rolls_back_ddl_and_version() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(v1_config(root.path())).unwrap();
    seed_v1(&store, 33);
    drop(store);
    let mut failed = v1_config(root.path());
    failed.migrations.register(
        Db::Intelligence,
        vec![
            super::super::intelligence::migration(),
            Migration::rust(2, |s| {
                super::super::intelligence_lineage::apply(s)?;
                s.execute("INSERT INTO deliberately_absent VALUES(1)", ())?;
                Ok(())
            }),
        ],
    );
    assert!(Store::open(failed).is_err());
    let store = Store::open(v1_config(root.path())).unwrap();
    store
        .read(Db::Intelligence, |s| {
            assert_eq!(s.schema_version()?, 1);
            assert_eq!(
                s.query_one("SELECT COUNT(*) FROM ai_embeddings", (), |r| r
                    .get::<i64>(0)
                    .map_err(Into::into))?,
                33
            );
            assert!(!s.table_columns("ai_embeddings")?.contains(&"lineage".to_string()));
            Ok(())
        })
        .unwrap();
}

/// Run each size in its own process under /usr/bin/time -l; the tempdir is removed on success.
#[test]
#[ignore = "explicit migration scaling measurement"]
fn measure_intelligence_v2_migration() {
    let count: i64 = std::env::var("PATHKEEP_MIGRATION_ROWS").unwrap().parse().unwrap();
    let root =
        std::env::var("PATHKEEP_MIGRATION_ROOT").expect("separate seed and migration processes");
    let path = Path::new(&root);
    if std::env::var("PATHKEEP_MIGRATION_PHASE").as_deref() == Ok("seed") {
        let store = Store::open(v1_config(path)).unwrap();
        seed_v1(&store, count);
        return;
    }
    let start = std::time::Instant::now();
    let store = Store::open(config(path, None)).unwrap();
    eprintln!(
        "Intelligence v2: rows={count} migration_seconds={:.3}",
        start.elapsed().as_secs_f64()
    );
    store
        .read(Db::Intelligence, move |s| {
            assert_eq!(
                s.query_one(
                    "SELECT COUNT(*) FROM ai_embeddings WHERE lineage='legacy'",
                    (),
                    |r| r.get::<i64>(0).map_err(Into::into)
                )?,
                count
            );
            Ok(())
        })
        .unwrap();
}

/// Failure modes: a batch's copy commits without its delete (or the reverse), so a crash repeats
/// or loses rows; or a resumed run starts over from rows that already moved.
#[test]
fn intelligence_backfill_resumes_committed_move_batches_before_open() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    // Rolls back the first visit batch, the last one, the totals batch and the final drop.
    for stop in [1, 2, 4, 6] {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(v1_config(root.path())).unwrap();
        seed_v1(&store, 50_003);
        drop(store);
        let mut cfg = config(root.path(), None);
        let calls = Arc::new(AtomicUsize::new(0));
        cfg.migrations.register(
            Db::Intelligence,
            vec![
                super::super::intelligence::migration(),
                Migration::rust(2, super::super::intelligence_lineage::apply).with_backfill(
                    move |s| {
                        let result = super::super::intelligence_lineage::backfill(s)?;
                        if calls.fetch_add(1, Ordering::SeqCst) == stop {
                            return Err(Error::cancelled());
                        }
                        Ok(result)
                    },
                ),
            ],
        );
        assert!(Store::open(cfg).is_err());
        let store = Store::open(config(root.path(), None)).unwrap();
        store
            .read(Db::Intelligence, |s| {
                assert_eq!(
                    s.query_one(
                        "SELECT COUNT(*),SUM(history_id) FROM ai_embeddings WHERE lineage='legacy'",
                        (),
                        |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?))
                    )?,
                    (50_003, 50_003 * 50_004 / 2)
                );
                assert_eq!(
                    s.query_one("SELECT item_count,lineage FROM ai_embedding_totals", (), |r| Ok(
                        (r.get::<i64>(0)?, r.get::<String>(1)?)
                    ))?,
                    (50_003, "legacy".to_owned())
                );
                assert_eq!(
                    s.query_one("SELECT phase FROM ai_embedding_upgrade", (), |r| r
                        .get::<String>(0)
                        .map_err(Into::into))?,
                    "done"
                );
                assert!(
                    !s.query_one(
                        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='ai_embeddings_v1')",
                        (),
                        |r| r.get::<i64>(0).map_err(Into::into)
                    )?
                    .eq(&1)
                );
                Ok(())
            })
            .unwrap();
        drop(store);
        drop(Store::open(config(root.path(), None)).unwrap());
    }
}

#[test]
fn native_startup_reports_committed_batches_through_the_archive_upgrade_contract() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(v1_config(root.path())).unwrap();
    seed_v1(&store, 33);
    drop(store);
    let paths = crate::project_paths_with_root(root.path());
    let mut events = Vec::new();
    let vault = crate::with_archive_upgrade_progress(
        |event| events.push(event),
        move || Ok(crate::Vault::open(paths, crate::AppConfig::default(), None)?),
    )
    .unwrap();
    assert!(events.iter().any(|event| event.processed==33 && event.database.as_deref()==Some("Intelligence")));
    assert!(events.last().unwrap().done);
    assert!(events.iter().filter(|event| !event.done).map(|event| event.processed).is_sorted());
    vault.close().unwrap();
}

#[test]
fn migrated_explicit_ids_advance_turso_sequence_for_later_upserts() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(v1_config(root.path())).unwrap();
    seed_v1(&store, 1025);
    drop(store);
    for expected in [1026, 1027] {
        let store = Store::open(config(root.path(), None)).unwrap();
        store.write(Db::Intelligence, Priority::Batch, move |s| {
            s.execute("INSERT OR REPLACE INTO ai_embeddings(history_id,profile_id,url,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at,lineage) VALUES(5000,'chrome:Default','https://new.test','new.test','','new',1,'service','model','','legacy')", ())?;
            assert_eq!(s.query_one("SELECT id FROM ai_embeddings WHERE history_id=5000",(),|r| Ok(r.get::<i64>(0)?))?, expected);
            assert_eq!(s.query_one("SELECT COUNT(*) FROM ai_embeddings WHERE history_id<=1025",(),|r| Ok(r.get::<i64>(0)?))?,1025);
            assert_eq!(s.query_one("SELECT seq FROM sqlite_sequence WHERE name='ai_embeddings'",(),|r| Ok(r.get::<i64>(0)?))?,expected);
            Ok(())
        }).unwrap();
    }
}

#[test]
fn history_prefix_index_keeps_turso_candidate_batches_on_history_seeks() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(config(root.path(), None)).unwrap();
    store.write(Db::Intelligence, Priority::Batch, |s| {
        let batch="EXPLAIN QUERY PLAN WITH requested(history_id) AS (VALUES(1),(2)) SELECT e.history_id,e.content_hash FROM requested CROSS JOIN ai_embeddings e ON e.history_id=requested.history_id WHERE e.provider_id='service' AND e.model='model' AND e.lineage='legacy'";
        s.execute("DROP INDEX idx_ai_embeddings_lineage_history", ())?;
        let direct=s.query("EXPLAIN QUERY PLAN SELECT content_hash FROM ai_embeddings WHERE history_id=1 AND provider_id='service' AND model='model' AND lineage='legacy'",(),|r|Ok(r.get::<String>(3)?))?;
        assert!(direct.iter().any(|s|s.contains("sqlite_autoindex_ai_embeddings") && s.contains("history_id=? AND provider_id=? AND model=? AND lineage=?")),"{direct:?}");
        let unforced=s.query(batch,(),|r|Ok(r.get::<String>(3)?))?;
        assert!(unforced.iter().any(|s|s.contains("idx_ai_embeddings_lineage_id") && !s.contains("history_id=?")),"{unforced:?}");
        s.execute("CREATE INDEX idx_ai_embeddings_lineage_history ON ai_embeddings(history_id,provider_id,model,lineage)", ())?;
        let forced=batch.replace("ai_embeddings e ON", "ai_embeddings e INDEXED BY idx_ai_embeddings_lineage_history ON");
        let protected=s.query(&forced,(),|r|Ok(r.get::<String>(3)?))?;
        assert!(protected.iter().any(|s|s.contains("idx_ai_embeddings_lineage_history") && s.contains("history_id=? AND provider_id=? AND model=? AND lineage=?")),"{protected:?}");
        Ok(())
    }).unwrap();
}
