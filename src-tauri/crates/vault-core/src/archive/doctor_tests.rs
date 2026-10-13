//! Responsibilities: verify native health findings and conservative repairs.
//! Not responsible for: legacy intelligence or file recovery.
//! Failure modes: corruption reported healthy, schema checks omitted, cached totals
//! drift, missing search documents/outbox notifications, incomplete artifact checks,
//! repair restores hidden visits or records success after an artifact error,
//! a concurrent indexer consumes intentionally damaged outbox fixtures before diagnosis.
use super::*;
use vault_store::{Db, Priority};

#[test]
fn repairs_counters_missing_documents_and_visibility_provenance_without_unhiding() {
    for key in [None, Some([79; 32])] {
        let vault = crate::archive::source_stats::tests::seeded_archive(key);
        // Keep the corrupt fixture stable until doctor has diagnosed and repaired it.
        // The live consumer is resumed below to verify that the repaired handoff applies.
        let paused = vault.search().pause();
        vault
            .store()
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute("DELETE FROM search_document_profiles", ())?;
                s.execute("DELETE FROM search_documents", ())?;
                s.execute("INSERT INTO search_outbox(page_id,change) VALUES(999,'upsert')", ())?;
                s.execute("UPDATE archive_totals SET value=999", ())?;
                Ok(())
            })
            .unwrap();
        let before = doctor(&vault).unwrap();
        for code in ["cached-totals", "search-outbox", "broken-visibility-references"] {
            assert!(before.checks.iter().any(|c| c.code == code && !c.ok), "{code}");
        }
        let repaired = repair_health_issues(&vault).unwrap();
        assert!(repaired.run_id.is_some());
        assert_eq!(repaired.repaired_visibility_rows, 1);
        assert_eq!(repaired.repaired_total_counters, 8);
        assert_eq!(repaired.refreshed_search_pages, 1);
        assert!(repaired.queued_search_updates >= 2);
        let after = doctor(&vault).unwrap();
        for code in ["cached-totals", "search-outbox", "broken-visibility-references"] {
            assert!(after.checks.iter().any(|c| c.code == code && c.ok), "{code}: {after:?}");
        }
        vault
            .store()
            .read(Db::Archive, |s| {
                assert_eq!(
                    s.query_one(
                        "SELECT COUNT(*) FROM visits WHERE reverted_at IS NOT NULL",
                        (),
                        |r| Ok(r.get::<i64>(0)?)
                    )?,
                    1
                );
                assert_eq!(
                    s.query_one("SELECT value FROM archive_totals WHERE name='visits'", (), |r| {
                        Ok(r.get::<i64>(0)?)
                    })?,
                    3
                );
                Ok(())
            })
            .unwrap();
        drop(paused);
        vault.search().cycle().unwrap();
        let result =
            crate::search::page(&vault, crate::search::SearchRequest::default(), None).unwrap();
        assert_eq!(result.count.value, 1);
        assert_eq!(result.visits.value, 3);
        assert_eq!(result.items[0].page_id, 1);
    }
}
