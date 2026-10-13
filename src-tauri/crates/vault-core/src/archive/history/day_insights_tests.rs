//! Responsibilities: verify full-day counts, transitions, sessions, dates and index seeks.
//! Not responsible for: intelligence projections.
//! Failure modes: scroll-dependent totals, profile leakage, reverted visits, DST gaps, full scans.
use super::*;
use vault_store::{Db, Priority, params};

fn fresh_vault(key: Option<[u8; 32]>) -> Vault {
    let vault = Vault::open_temp_with_key(key).unwrap();
    vault.store().write(Db::Archive, Priority::Batch, |s| {
            s.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,profile_name,profile_path,discovered_at) VALUES (1,'chrome:Default','chromium','Default','/chrome','2026-05-25'),(2,'firefox:Default','firefox','Default','/firefox','2026-05-25')",())?;
            Ok(())
        }).unwrap();
    vault
}

#[test]
fn day_queries_seek_time_indexes() {
    for key in [None, Some([73; 32])] {
        let vault = fresh_vault(key);
        vault
            .store()
            .read(Db::Archive, |s| {
                for profile in [None, Some(1)] {
                    let sql = day_visits_sql(profile.is_some(), false, true);
                    let plan = s
                        .query(
                            &format!("EXPLAIN QUERY PLAN {sql}"),
                            params![0, i64::MAX, profile, i64::MAX, 4096],
                            |r| Ok(r.get::<String>(3)?),
                        )?
                        .join("\n");
                    assert!(plan.contains("SEARCH visits"), "{plan}");
                    assert!(!plan.contains("SCAN visits") && !plan.contains("SCAN urls"), "{plan}");
                }
                Ok(())
            })
            .unwrap();
    }
}
