//! Responsibilities: prove every batch and visible preview entry remains reachable.
//! Not responsible for: ingestion, search or audit artifact rendering.
//! Failure modes: a fixed recent-batch cap hides older imports, active imports vanish,
//! tied visit times skip rows, reverted rows leak, counts require a fact scan.
use super::*;
use vault_store::Db;

#[test]
fn import_review_seeks_its_batch_time_index() {
    for key in [None, Some([80; 32])] {
        let vault = crate::Vault::open_temp_with_key(key).unwrap();
        vault
            .store()
            .read(Db::Archive, |s| {
                for ties in [false, true] {
                    let sql = batch_visits_sql(ties, true);
                    let plan = s
                        .query(
                            &format!("EXPLAIN QUERY PLAN {sql}"),
                            vault_store::params![1, 5000, 20, 5],
                            |r| Ok(r.get::<String>(3)?),
                        )?
                        .join("\n");
                    assert!(plan.contains("idx_visits_visible_batch_time_id"), "{plan}");
                    assert!(
                        !plan.contains("SCAN visits")
                            && !plan.contains("TEMP B-TREE")
                            && !plan.contains("SORTER"),
                        "{plan}"
                    );
                }
                Ok(())
            })
            .unwrap();
    }
}
