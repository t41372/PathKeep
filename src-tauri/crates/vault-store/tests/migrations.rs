//! Responsibilities: verify ordered, atomic startup migrations on real Turso databases.
//! Not responsible for: defining application schema.
//! Failure modes: newer schema opens; partial failed/panicked steps persist or advance;
//! migrations rerun during reads.
mod support;
use vault_store::{Db, Error, Migration, Store};
#[test]
fn newer_schema_is_refused_without_running_older_code() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = support::config(dir.path());
    config.migrations.register(
        Db::Archive,
        vec![
            Migration::sql(1, "CREATE TABLE visits(id INTEGER)"),
            Migration::sql(3, "CREATE TABLE newer(id INTEGER)"),
        ],
    );
    drop(Store::open(config).unwrap());
    assert!(matches!(
        Store::open(support::config(dir.path())),
        Err(Error::SchemaTooNew { found: 3, supported: 1, .. })
    ));
}
#[test]
fn each_step_is_atomic_and_only_runs_once_at_open() {
    let dir = tempfile::tempdir().unwrap();
    let steps = || {
        vec![
            Migration::sql(1, "CREATE TABLE first_step(id INTEGER)"),
            Migration::rust(2, |tx| {
                tx.execute("CREATE TABLE second_step(id INTEGER)", ())?;
                Err(Error::cancelled())
            }),
        ]
    };
    let mut config = support::config(dir.path());
    config.migrations.register(Db::Archive, steps());
    assert!(matches!(Store::open(config), Err(Error::Cancelled { .. })));
    let mut config = support::config(dir.path());
    config.migrations.register(
        Db::Archive,
        vec![
            Migration::sql(1, "CREATE TABLE first_step(id INTEGER)"),
            Migration::sql(2, "CREATE TABLE second_step(id INTEGER)"),
        ],
    );
    let store = Store::open(config).unwrap();
    for _ in 0..3 {
        assert_eq!(
            store
                .read(Db::Archive, |c| c
                    .query_one("SELECT count(*) FROM second_step", (), |r| Ok(r.get::<i64>(0)?)))
                .unwrap(),
            0
        );
    }
}
#[test]
fn migration_panic_rolls_back_and_releases_owner() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = support::config(dir.path());
    config.migrations.register(
        Db::Archive,
        vec![Migration::rust(1, |tx| {
            tx.execute("CREATE TABLE visits(id INTEGER)", ())?;
            panic!("migration fixture")
        })],
    );
    assert!(matches!(Store::open(config), Err(Error::JobPanicked { .. })));
    let store = Store::open(support::config(dir.path())).unwrap();
    assert_eq!(support::count(&store), 0);
}

/// Failure modes: a failed batch rolls back earlier progress, reopen skips a version's unfinished
/// backfill, readers start with partial data, or observers report an uncommitted batch.
#[test]
fn resumable_backfill_finishes_before_readers_and_restarts_after_the_last_committed_batch() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use vault_store::BackfillProgress;
    let root = tempfile::tempdir().unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let registry = |fail: bool| {
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed = observed.clone();
        vec![Migration::rust(1,|s| {
            s.execute("CREATE TABLE old_rows(id INTEGER PRIMARY KEY)",())?;
            for id in 1..=5 { s.execute("INSERT INTO old_rows VALUES(?1)",[id])?; }
            Ok(())
        }),Migration::rust(2,|s| {s.execute("CREATE TABLE new_rows(id INTEGER PRIMARY KEY)",())?;Ok(())})
        .with_backfill(|s| Ok(s.query_one("SELECT COUNT(*) FROM old_rows WHERE id>COALESCE((SELECT MAX(id) FROM new_rows),0)",(),|r| r.get::<i64>(0).map_err(Into::into))? > 0), move |s| {
            let next=s.query("SELECT id FROM old_rows WHERE id>COALESCE((SELECT MAX(id) FROM new_rows),0) ORDER BY id LIMIT 1",(),|r| r.get::<i64>(0).map_err(Into::into))?;
            let Some(id)=next.first().copied() else {return Ok(None)};
            s.execute("INSERT INTO new_rows VALUES(?1)",[id])?;
            if fail && attempts.fetch_add(1,Ordering::SeqCst)==2 { return Err(Error::cancelled()); }
            Ok(Some(BackfillProgress {processed:id as u64,total:5,complete:id==5,table:Some("new_rows".into())}))
        }).on_backfill_progress(move |p| observed.lock().unwrap().push(p.processed))]
    };
    let mut cfg = support::config(root.path());
    cfg.migrations.register(Db::Archive, registry(true));
    assert!(Store::open(cfg).is_err());
    assert_eq!(*observed.lock().unwrap(), [1, 2]);
    let mut cfg = support::config(root.path());
    cfg.migrations.register(Db::Archive, registry(false));
    let store = Store::open(cfg).unwrap();
    assert_eq!(*observed.lock().unwrap(), [1, 2, 3, 4, 5]);
    assert_eq!(
        store
            .read(Db::Archive, |s| s.query_one("SELECT COUNT(*) FROM new_rows", (), |r| r
                .get::<i64>(0)
                .map_err(Into::into)))
            .unwrap(),
        5
    );
}

/// Failure modes: an update goes untold, so the owner lets someone quit during it without asking
/// (new versions for a database that already has a schema, or a backfill an earlier open left
/// unfinished); the owner hears of it only after the change; a database the open creates, or one
/// already up to date, is reported as updating; one open reports one database twice.
#[test]
fn an_open_tells_the_owner_before_it_updates_a_database_and_at_no_other_time() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use vault_store::BackfillProgress;
    let root = tempfile::tempdir().unwrap();
    let log = Arc::new(Mutex::new(Vec::<String>::new()));
    let say = |log: &Arc<Mutex<Vec<String>>>, line: String| log.lock().unwrap().push(line);
    // Version 1 holds three rows; version 2 marks them one batch at a time; version 3 changes
    // nothing. A batch can fail once `fail_at` batches committed, leaving the backfill unfinished.
    let steps = |versions: i64, fail_at: Option<usize>| {
        let (one, two, batch) = (log.clone(), log.clone(), log.clone());
        let batches = AtomicUsize::new(0);
        let mut steps = vec![
            Migration::rust(1, move |s| {
                say(&one, "apply 1".into());
                s.execute("CREATE TABLE rows(id INTEGER PRIMARY KEY, moved INTEGER NOT NULL)", ())?;
                s.execute("INSERT INTO rows VALUES(1,0),(2,0),(3,0)", ())?;
                Ok(())
            }),
            Migration::rust(2, move |_| {
                say(&two, "apply 2".into());
                Ok(())
            })
            .with_backfill(
                |s| {
                    let left = s.query_one("SELECT COUNT(*) FROM rows WHERE moved=0", (), |r| {
                        r.get::<i64>(0).map_err(Into::into)
                    })?;
                    Ok(left > 0)
                },
                move |s| {
                    let next = s.query(
                        "SELECT id FROM rows WHERE moved=0 ORDER BY id LIMIT 1",
                        (),
                        |r| r.get::<i64>(0).map_err(Into::into),
                    )?;
                    let Some(id) = next.first().copied() else { return Ok(None) };
                    if fail_at.is_some_and(|at| batches.fetch_add(1, Ordering::SeqCst) == at) {
                        return Err(Error::cancelled());
                    }
                    s.execute("UPDATE rows SET moved=1 WHERE id=?1", [id])?;
                    say(&batch, format!("batch {id}"));
                    Ok(Some(BackfillProgress {
                        processed: id as u64,
                        total: 3,
                        table: None,
                        complete: false,
                    }))
                },
            ),
        ];
        if versions == 3 {
            let three = log.clone();
            steps.push(Migration::rust(3, move |_| {
                say(&three, "apply 3".into());
                Ok(())
            }));
        }
        steps
    };
    let open = |steps: Vec<Migration>| {
        let mut config = support::config(root.path());
        config.migrations.register(Db::Archive, steps);
        let told = log.clone();
        config.migrations.on_update(move |db| say(&told, format!("told {db:?}")));
        let opened = Store::open(config);
        (opened.map(drop), std::mem::take(&mut *log.lock().unwrap()))
    };
    let created = open(steps(2, Some(1)));
    assert!(created.0.is_err(), "the second batch fails");
    assert_eq!(
        created.1,
        ["apply 1", "apply 2", "batch 1"],
        "creating a database is not updating it"
    );
    let resumed = open(steps(2, None));
    assert!(resumed.0.is_ok());
    assert_eq!(resumed.1, ["told Archive", "batch 2", "batch 3"], "the unfinished backfill");
    let current = open(steps(2, None));
    assert!(current.0.is_ok());
    assert_eq!(current.1, [] as [&str; 0], "nothing to do, nothing told");
    let newer = open(steps(3, None));
    assert!(newer.0.is_ok());
    assert_eq!(newer.1, ["told Archive", "apply 3"], "told once, before the new version");
    let again = open(steps(3, None));
    assert_eq!(again.1, [] as [&str; 0]);
}
