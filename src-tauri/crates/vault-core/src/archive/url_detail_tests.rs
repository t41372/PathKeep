//! Responsibilities: verify URL visibility, calendar buckets and query plans.
//! Not responsible for: ingestion.
//! Failure modes: profile loss, reverted rows counted, bad week bounds, unknown URL errors, full scans.
use super::*;
use crate::archive::source_stats::tests::seeded_archive;
use vault_store::Db;

#[test]
fn detail_queries_seek_indexes() {
    for key in [None, Some([72; 32])] {
        let vault = seeded_archive(key);
        vault
            .store()
            .read(Db::Archive, |s| {
                for (sql, args) in [
                    (
                        URL_ROWS_SQL,
                        vault_store::params!["https://example.com/a"]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_COUNTS_SQL,
                        vault_store::params!["https://example.com/a"]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_FIRST_SQL,
                        vault_store::params![1].into_iter().collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_LAST_SQL,
                        vault_store::params![1].into_iter().collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_RANGE_SQL,
                        vault_store::params![1, 0, i64::MAX]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                    (
                        URL_DOMAIN_SQL,
                        vault_store::params!["https://example.com/a"]
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                ] {
                    let plan = s
                        .query(&format!("EXPLAIN QUERY PLAN {sql}"), args, |r| {
                            Ok(r.get::<String>(3)?)
                        })?
                        .join("\n");
                    assert!(plan.contains("SEARCH"), "{plan}");
                    assert!(!plan.contains("SCAN visits") && !plan.contains("SCAN urls"), "{plan}");
                }
                Ok(())
            })
            .unwrap();
    }
}
