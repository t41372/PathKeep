//! Responsibilities: verify ordered, atomic startup migrations on real Turso databases.
//! Not responsible for: defining application schema.
//! Failure modes: newer schema opens; partial failed/panicked steps persist or advance;
//! registry accepts duplicate/reversed versions; migrations rerun during reads.
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
#[test]
fn duplicate_and_reversed_versions_are_rejected() {
    for versions in [[1, 1], [2, 1], [0, 1]] {
        let dir = tempfile::tempdir().unwrap();
        let mut config = support::config(dir.path());
        config.migrations.register(
            Db::Archive,
            versions.into_iter().map(|v| Migration::sql(v, "SELECT 1")).collect(),
        );
        assert!(matches!(Store::open(config), Err(Error::InvalidConfig(_))));
    }
}
