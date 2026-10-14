//! Responsibilities: verify source counts, visibility, bounds and index seeks.
//! Not responsible for: discovery or ingestion fixtures.
//! Failure modes: reverted visits counted, empty profiles hidden, stale caches, full scans.
use super::*;
use crate::Vault;
use vault_store::{Db, Priority};

pub(crate) fn seeded_archive(key: Option<[u8; 32]>) -> Vault {
    let vault = Vault::open_temp_with_key(key).expect("open");
    vault.store().write(Db::Archive, Priority::Batch, |s| {
            let sql =                 "INSERT INTO runs (id, run_type, trigger, started_at, status, due_only)
                   VALUES (1, 'backup', 'manual', '2026-01-01T00:00:00Z', 'success', 0);
                 INSERT INTO source_profiles (id, browser_kind, profile_name, profile_path, discovered_at, enabled, profile_key)
                   VALUES (1, 'chrome', 'Default', '/p/1', '2026-01-01T00:00:00Z', 1, 'chrome:Default'),
                          (2, 'firefox', 'dev', '/p/2', '2026-01-01T00:00:00Z', 1, 'firefox:dev'),
                          (3, 'edge', 'Empty', '/p/3', '2026-01-01T00:00:00Z', 1, 'edge:Empty');
                 INSERT INTO pages (id,url,canonical_url,registrable_domain) VALUES (1,'https://example.com/a','https://example.com/a','example.com');
                 INSERT INTO urls (id, page_id, url, title, visit_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso, source_profile_id, created_by_run_id, source_url_id)
                   VALUES (1, 1, 'https://example.com/a', 'A', 3, 0, '', 0, '', 1, 1, 1),
                          (2, 1, 'https://example.com/a', 'A (ff)', 1, 0, '', 0, '', 2, 1, 1);
                 INSERT INTO visits (url_id, visit_time_ms, visit_time_iso, source_profile_id, created_by_run_id, reverted_at)
                   VALUES (1, 1000, '', 1, 1, NULL),
                          (1, 5000, '', 1, 1, NULL),
                          (1, 9000, '', 1, 1, '2026-01-02T00:00:00Z'),
                          (2, 2000, '', 2, 1, NULL);";
            for statement in sql.split(';').filter(|s| !s.trim().is_empty()) {
                s.execute(statement, ())?;
            }
            crate::archive::search_documents::refresh(s, 1)?;
        s.execute("INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',2),('sourceVisits:2',1)",())?;
            Ok(())
        }).expect("seed");
    vault
}

#[test]
fn stats_use_index_seeks_without_visits_or_urls_scans() {
    for key in [None, Some([71; 32])] {
        let vault = seeded_archive(key);
        vault
            .store()
            .read(Db::Archive, |s| {
                for sql in [PROFILE_COUNT_SQL, PROFILE_FIRST_SQL, PROFILE_LAST_SQL] {
                    let plan = s
                        .query(&format!("EXPLAIN QUERY PLAN {sql}"), [1], |r| {
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
