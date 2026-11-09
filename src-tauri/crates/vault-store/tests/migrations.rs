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
        .with_backfill(move |s| {
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
