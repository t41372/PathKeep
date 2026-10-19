//! Responsibilities: exercise restrictions as executable facade contracts, without grep.
//! Not responsible for: treating trusted business SQL as an adversarial sandbox.
//! Failure modes: comments/quoted identifiers bypass TEMP or VACUUM restrictions;
//! transaction control, PRAGMAs, metadata changes or attached writes bypass owners.
mod support;
use vault_store::{Db, Error, Priority};
#[test]
fn facade_rejects_unsafe_sql_and_transaction_escape() {
    let (_dir, store) = support::open();
    for sql in [
        "CREATE TEMP TABLE leak(id)",
        "CREATE /* note */ TEMPORARY TABLE leak(id)",
        "CREATE TABLE temp.leak(id)",
        "CREATE TABLE \"temp\".leak(id)",
        "CREATE TABLE 'temp'.leak(id)",
        "DELETE FROM '__pk_migrations'",
        "UPDATE OR ABORT '__pk_migrations' SET version=99",
        "REPLACE '__pk_migrations' VALUES (99)",
        "DELETE FROM main.'__pk_migrations'",
        "DELETE FROM main./*comment*/'__pk_migrations'",
        "VACUUM INTO 'leak.db'",
        "VACUUM /* comment */ INTO 'leak.db'",
        "ATTACH 'leak.db' AS leak",
        "PRAGMA journal_mode=mvcc",
        "COMMIT",
        "END",
        "ROLLBACK",
        "SAVEPOINT escape",
        "CREATE TABLE scratch.leak(id)",
        "DELETE FROM __pk_migrations",
        "SELECT 1; CREATE TEMP TABLE leak(id)",
    ] {
        let sql = sql.to_string();
        let result =
            store.write(Db::Archive, Priority::Interactive, move |tx| tx.execute(&sql, ()));
        assert!(matches!(result, Err(Error::RestrictedSql(_))), "{result:?}");
    }
    assert!(
        store
            .read(Db::Archive, |c| c.query_one(
                "INSERT INTO visits(id) VALUES (1) RETURNING id",
                (),
                |r| Ok(r.get::<i64>(0)?)
            ))
            .is_err()
    );
    assert_eq!(support::count(&store), 0);
    assert_eq!(
        store
            .read(Db::Archive, |c| c
                .query_one("SELECT 'TEMP; VACUUM INTO'", (), |r| Ok(r.get::<String>(0)?)))
            .unwrap(),
        "TEMP; VACUUM INTO"
    );
}

// Validation is remembered per text on each connection (statement_cache.rs). What that must not
// change: a write the writer ran is still refused in a read; refused SQL is refused on every call,
// never remembered as allowed.
#[test]
fn remembered_validation_still_refuses() {
    let (_dir, store) = support::open();
    let insert = "INSERT INTO visits(id) VALUES (1) RETURNING id";
    store
        .write(Db::Archive, Priority::Interactive, |tx| {
            tx.query_one(insert, (), |r| Ok(r.get::<i64>(0)?))
        })
        .unwrap();
    for _ in 0..3 {
        let read = store.read(Db::Archive, |c| c.query_one(insert, (), |r| Ok(r.get::<i64>(0)?)));
        assert!(matches!(read, Err(Error::RestrictedSql(_))), "{read:?}");
        let vacuum = store.write(Db::Archive, Priority::Interactive, |tx| tx.execute("VACUUM", ()));
        assert!(matches!(vacuum, Err(Error::RestrictedSql(_))), "{vacuum:?}");
    }
    assert_eq!(support::count(&store), 1);
}
