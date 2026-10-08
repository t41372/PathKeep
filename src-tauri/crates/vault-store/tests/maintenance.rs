//! Responsibilities: prove maintenance uses committed, complete, recoverable files.
//! Not responsible for: application schema, passwords or file installation.
//! Failure modes: missed rows/sequence marks, changed values, plaintext output,
//! ignored cancellation, queued writes crossing a snapshot, failed copies stranding admission.
use std::sync::{Arc, mpsc};
use vault_store::{CancelToken, Config, Db, Migration, Priority, Store};

fn config(root: &std::path::Path, key: Option<[u8; 32]>) -> Config {
    let mut config = Config::new(root);
    config.keys = [key; 5];
    config.migrations.register(Db::Archive, vec![Migration::rust(1, |s| {
        s.execute("CREATE TABLE facts(id INTEGER PRIMARY KEY AUTOINCREMENT, body TEXT, bytes BLOB, number REAL)", ())?;
        s.execute("CREATE VIEW named_facts AS SELECT id,body FROM facts WHERE body IS NOT NULL", ())?;
        Ok(())
    })]);
    config
}

#[test]
fn attached_copy_preserves_values_and_deleted_sequence_high_water_in_both_modes() {
    for key in [None, Some([19; 32])] {
        let source_root = tempfile::tempdir().unwrap();
        let target_root = tempfile::tempdir().unwrap();
        let source = Store::open(config(source_root.path(), key)).unwrap();
        source
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("INSERT INTO facts VALUES(1,'private sentinel',X'0001FF',1.25)", ())?;
                s.execute("INSERT INTO facts VALUES(2,NULL,NULL,NULL)", ())?;
                s.execute("INSERT INTO facts(id) VALUES(9000)", ())?;
                s.execute("DELETE FROM facts WHERE id=9000", ())?;
                Ok(())
            })
            .unwrap();
        let target_key = if key.is_some() { None } else { Some([37; 32]) };
        let target_config = config(target_root.path(), target_key);
        std::fs::create_dir_all(target_root.path().join("archive")).unwrap();
        let destination = target_config.paths.archive.clone();
        let pause = source.pause().unwrap();
        let proof = pause
            .copy_database(Db::Archive, destination.clone(), target_key, CancelToken::new(), |_| {})
            .unwrap();
        assert!(proof.iter().any(|t| t.table == "facts" && t.rows == 2));
        drop(pause);
        let target = Store::open(target_config).unwrap();
        assert_eq!(
            target
                .read(Db::Archive, |s| s.query_one(
                    "SELECT body,bytes,number FROM facts WHERE id=1",
                    (),
                    |r| Ok((r.get::<String>(0)?, r.get::<Vec<u8>>(1)?, r.get::<f64>(2)?))
                ))
                .unwrap(),
            ("private sentinel".into(), vec![0, 1, 255], 1.25)
        );
        // Views hold no rows, but readers depend on them as much as on tables.
        assert_eq!(
            target
                .read(Db::Archive, |s| s
                    .query("SELECT id FROM named_facts", (), |r| Ok(r.get::<i64>(0)?)))
                .unwrap(),
            [1]
        );
        let id = target
            .write(Db::Archive, Priority::Interactive, |s| {
                s.query_one("INSERT INTO facts(body) VALUES('next') RETURNING id", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })
            })
            .unwrap();
        assert_eq!(id, 9001);
        if target_key.is_some() {
            target.close().unwrap();
            let bytes = std::fs::read(destination).unwrap();
            assert!(!bytes.windows(16).any(|b| b == b"private sentinel"));
        }
    }
}

#[test]
fn pause_checkpoint_copy_and_resume_land_between_write_batches() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(config(root.path(), Some([11; 32]))).unwrap());
    store
        .write(Db::Archive, Priority::Batch, |s| {
            s.execute("INSERT INTO facts(id) VALUES(1)", ())?;
            Ok(())
        })
        .unwrap();
    let pause = store.pause().unwrap();
    let (started, ready) = mpsc::channel();
    let other = store.clone();
    let next = std::thread::spawn(move || {
        started.send(()).unwrap();
        other
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("INSERT INTO facts(id) VALUES(2)", ())?;
                Ok(())
            })
            .unwrap();
    });
    ready.recv().unwrap();
    pause.snapshot(snapshot.path()).unwrap();
    drop(pause);
    next.join().unwrap();
    let restored = Store::open(config(snapshot.path(), Some([11; 32]))).unwrap();
    assert_eq!(
        restored
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM facts", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM facts", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        2
    );
}

#[test]
fn cancelled_copy_releases_attachment_and_leaves_source_usable() {
    let root = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let store = Store::open(config(root.path(), None)).unwrap();
    store
        .write(Db::Archive, Priority::Batch, |s| {
            s.execute_rows(
                "INSERT INTO facts(id) VALUES(?1)",
                (1..=9000).map(|id| vec![vault_store::Value::Integer(id)]).collect(),
            )?;
            Ok(())
        })
        .unwrap();
    let cancel = CancelToken::new();
    let stop = cancel.clone();
    let pause = store.pause().unwrap();
    assert!(matches!(
        pause.copy_database(
            Db::Archive,
            target.path().join("cancelled.db"),
            Some([3; 32]),
            cancel,
            move |progress| {
                if progress.copied >= 4096 {
                    stop.cancel();
                }
            }
        ),
        Err(vault_store::Error::Cancelled { .. })
    ));
    pause
        .copy_database(
            Db::Archive,
            target.path().join("retry.db"),
            Some([3; 32]),
            CancelToken::new(),
            |_| {},
        )
        .unwrap();
    drop(pause);
    store
        .write(Db::Archive, Priority::Interactive, |s| {
            s.execute("INSERT INTO facts(id) VALUES(9001)", ())?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn repeated_insert_rebinds_nulls_and_rolls_back_the_whole_failed_batch() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(config(root.path(), None)).unwrap();
    use vault_store::Value as V;
    store
        .write(Db::Archive, Priority::Batch, |s| {
            s.execute_rows(
                "INSERT INTO facts(id,body,bytes,number) VALUES(?1,?2,?3,?4)",
                vec![
                    vec![V::Integer(1), V::Text("first".into()), V::Blob(vec![1]), V::Real(1.5)],
                    vec![V::Integer(2), V::Null, V::Null, V::Null],
                ],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        store
            .read(Db::Archive, |s| s.query_one(
                "SELECT body,bytes,number FROM facts WHERE id=2",
                (),
                |r| Ok((
                    r.get::<Option<String>>(0)?,
                    r.get::<Option<Vec<u8>>>(1)?,
                    r.get::<Option<f64>>(2)?
                ))
            ))
            .unwrap(),
        (None, None, None)
    );
    let error = store.write(Db::Archive, Priority::Batch, |s| {
        s.execute_rows(
            "INSERT INTO facts(id) VALUES(?1)",
            vec![vec![V::Integer(3)], vec![V::Integer(3)]],
        )?;
        Ok(())
    });
    assert!(error.is_err());
    assert_eq!(
        store
            .read(Db::Archive, |s| s
                .query_one("SELECT count(*) FROM facts", (), |r| Ok(r.get::<i64>(0)?)))
            .unwrap(),
        2
    );
}
