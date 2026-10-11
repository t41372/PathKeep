//! Responsibilities: every statement that links the same visit seeks an index.
//! Not responsible for: what linking does (T3, T8–T10 in `ingest/dedup_scenarios_takeout.rs`).
//! Failure modes: a statement run once per copy reading every visit. The follower update did:
//! `same_as=?` alone does not let the partial index on `same_as IS NOT NULL` answer, and linking
//! the 12,400 copies on the real-data copy read its 387k visits once per copy.
use super::*;
use crate::archive::source_stats::tests::seeded_archive;
use vault_store::{Db, Priority, Value};

#[test]
fn linking_statements_seek_indexes() {
    for key in [None, Some([72; 32])] {
        let vault = seeded_archive(key);
        vault
            .store()
            // Updates are explained in a write session; a read one refuses them.
            .write(Db::Archive, Priority::Batch, |s| {
                let n = Value::Integer;
                for (sql, args) in [
                    (OVERLAPS_SQL.to_owned(), vec![n(1), n(0), n(1)]),
                    (
                        stand_in_sql("1,2"),
                        vec![Value::Text("https://example.com/a".into()), n(0), n(1), n(0)],
                    ),
                    (HIDE_FROM_BATCH_SQL.to_owned(), vec![n(1)]),
                    (HIDE_SQL.to_owned(), vec![Value::Text(String::new()), n(1), n(2), n(1)]),
                    (FOLLOW_SQL.to_owned(), vec![n(2), n(1)]),
                    (COPIES_SQL.to_owned(), vec![n(1), n(1)]),
                    (PROMOTE_SQL.to_owned(), vec![n(1)]),
                    (SHOW_IN_BATCH_SQL.to_owned(), vec![n(1)]),
                ] {
                    let plan = s
                        .query(&format!("EXPLAIN QUERY PLAN {sql}"), args, |r| {
                            Ok(r.get::<String>(3)?)
                        })?
                        .join("\n");
                    let scans = plan
                        .lines()
                        .filter(|line| line.starts_with("SCAN") && !line.contains("CONSTANT ROW"))
                        .count();
                    assert!(plan.contains("SEARCH") && scans == 0, "{sql}\n{plan}");
                }
                Ok(())
            })
            .unwrap();
    }
}
