//! Responsibilities: prove archive status, cached dashboard totals and complete run paging.
//! Not responsible for: legacy initialization or whole-file maintenance.
//! Failure modes: totals recounted/stale, seed run leaked, tied timestamps drop runs,
//! running batch hidden, manifest or warnings missing from audit detail.
use crate::Vault;
use crate::archive::run_support::{EARLIEST_VISIT_SQL, LATEST_VISIT_SQL};
use vault_store::Db;

#[test]
fn overview_bounds_seek_the_visible_time_index() {
    let vault = Vault::open_temp().unwrap();
    vault
        .store()
        .read(Db::Archive, |s| {
            for sql in [EARLIEST_VISIT_SQL, LATEST_VISIT_SQL] {
                let plan = s
                    .query(&format!("EXPLAIN QUERY PLAN {sql}"), (), |r| Ok(r.get::<String>(3)?))?
                    .join("\n");
                assert!(
                    plan.contains("SEARCH visits") && plan.contains("idx_visits_visible_time_id"),
                    "{plan}"
                );
                assert!(!plan.contains("TEMP B-TREE") && !plan.contains("SORTER"), "{plan}");
            }
            Ok(())
        })
        .unwrap();
}
