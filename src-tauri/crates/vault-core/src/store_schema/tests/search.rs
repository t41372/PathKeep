//! Responsibilities: pin page identity, revertable membership and durable index handoff.
//! Not responsible for: production search writers, tokenization or Tantivy.
//! Failure modes: cross-profile duplicates, stale membership after last revert,
//! non-atomic document/outbox changes, reused sequences and lost tombstones.
use super::*;

fn shared_page(tx: &Session<'_>) -> vault_store::Result<()> {
    tx.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at) VALUES (1,'chrome','one','/one','now'),(2,'firefox','two','/two','now')", ())?;
    tx.execute("INSERT INTO pages(id,url,registrable_domain,canonical_url) VALUES(1,'https://example.com/page','example.com','https://example.com/page')", ())?;
    tx.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) VALUES(1,1,'https://example.com/page','Chrome title',10,'first',20,'last',1,0),(2,1,'https://example.com/page','Firefox title',30,'first',30,'last',2,0)", ())?;
    tx.execute("INSERT INTO visits(id,url_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(1,1,10,'first',1,0),(2,1,20,'last',1,0),(3,2,30,'last',2,0)", ())?;
    tx.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,domain,newest_visit_ms,updated_at) VALUES(1,'https://example.com/page','Shared title','','example com page','shared title','','','examplecompage','example.com',30,'now')", ())?;
    tx.execute("INSERT INTO search_document_profiles VALUES(1,1,2,20),(1,2,1,30)", ())?;
    Ok(())
}

// Test transaction describes the writer contract; the schema does not hide it in triggers.
fn refresh_visibility(tx: &Session<'_>) -> vault_store::Result<()> {
    tx.execute("DELETE FROM search_document_profiles WHERE page_id=1", ())?;
    tx.execute("INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms) SELECT u.page_id,v.source_profile_id,count(*),max(v.visit_time_ms) FROM urls u JOIN visits v ON v.url_id=u.id WHERE u.page_id=1 AND v.reverted_at IS NULL GROUP BY u.page_id,v.source_profile_id", ())?;
    tx.execute("UPDATE search_documents SET newest_visit_ms=(SELECT max(newest_visit_ms) FROM search_document_profiles WHERE page_id=1) WHERE page_id=1", ())?;
    tx.execute("INSERT INTO search_outbox(page_id,change) VALUES(1,'upsert')", ())?;
    Ok(())
}
#[test]
fn pages_and_visibility() {
    each_store(|store| {
        store.write(Db::Archive, Priority::Batch, shared_page).unwrap();
        assert_eq!(number(store, Db::Archive, "SELECT count(*) FROM pages"), 1);
        assert_eq!(number(store, Db::Archive, "SELECT count(*) FROM urls WHERE page_id=1"), 2);
        assert_eq!(
            number(store, Db::Archive, "SELECT count(DISTINCT title) FROM urls WHERE page_id=1"),
            2
        );
        for (visit, profiles, newest) in [(2, 2, 30), (3, 1, 10), (1, 0, 0)] {
            store
                .write(Db::Archive, Priority::Interactive, move |tx| {
                    tx.execute("UPDATE visits SET reverted_at='now' WHERE id=?1", [visit])?;
                    refresh_visibility(tx)
                })
                .unwrap();
            assert_eq!(
                number(store, Db::Archive, "SELECT count(*) FROM search_document_profiles"),
                profiles
            );
            assert_eq!(
                number(
                    store,
                    Db::Archive,
                    "SELECT coalesce(newest_visit_ms,0) FROM search_documents"
                ),
                newest
            );
        }
        store
            .write(Db::Archive, Priority::Interactive, |tx| {
                tx.execute("UPDATE visits SET reverted_at=NULL WHERE id=3", ())?;
                refresh_visibility(tx)
            })
            .unwrap();
        assert_eq!(
            number(store, Db::Archive, "SELECT source_profile_id FROM search_document_profiles"),
            2
        );
        assert_eq!(
            number(store, Db::Archive, "SELECT visible_visit_count FROM search_document_profiles"),
            1
        );
        assert_eq!(number(store, Db::Archive, "SELECT newest_visit_ms FROM search_documents"), 30);
    });
}

#[test]
fn search_outbox() {
    each_store(|store| {
        store.write(Db::Archive, Priority::Batch, shared_page).unwrap();
        store
            .write(Db::Archive, Priority::Interactive, |tx| {
                tx.execute(
                    "UPDATE search_documents SET notes_text='committed' WHERE page_id=1",
                    (),
                )?;
                tx.execute("INSERT INTO search_outbox(page_id,change) VALUES(1,'upsert')", ())?;
                Ok(())
            })
            .unwrap();
        let result: vault_store::Result<()> =
            store.write(Db::Archive, Priority::Interactive, |tx| {
                tx.execute(
                    "UPDATE search_documents SET notes_text='rolled back' WHERE page_id=1",
                    (),
                )?;
                tx.execute("INSERT INTO search_outbox(page_id,change) VALUES(1,'upsert')", ())?;
                Err(Error::cancelled())
            });
        assert!(matches!(result, Err(Error::Cancelled { .. })));
        assert_eq!(
            number(
                store,
                Db::Archive,
                "SELECT count(*) FROM search_documents WHERE notes_text='committed'"
            ),
            1
        );
        assert_eq!(number(store, Db::Archive, "SELECT count(*) FROM search_outbox"), 1);
        let acknowledged = number(store, Db::Archive, "SELECT max(seq) FROM search_outbox");
        store
            .write(Db::Archive, Priority::Interactive, |tx| {
                tx.execute("DELETE FROM search_outbox", ())?;
                tx.execute("DELETE FROM visits", ())?;
                tx.execute("DELETE FROM urls", ())?;
                tx.execute("DELETE FROM pages WHERE id=1", ())?;
                tx.execute("INSERT INTO search_outbox(page_id,change) VALUES(1,'delete')", ())?;
                Ok(())
            })
            .unwrap();
        assert_eq!(number(store, Db::Archive, "SELECT count(*) FROM search_documents"), 0);
        assert_eq!(number(store, Db::Archive, "SELECT count(*) FROM search_document_profiles"), 0);
        assert_eq!(
            number(store, Db::Archive, "SELECT page_id FROM search_outbox WHERE change='delete'"),
            1
        );
        assert!(number(store, Db::Archive, "SELECT seq FROM search_outbox") > acknowledged);
    });
}

#[test]
fn unique_constraints() {
    each_store(|store| {
        store.write(Db::Archive, Priority::Batch, shared_page).unwrap();
        for sql in [
            "INSERT INTO pages(url,canonical_url) VALUES('https://example.com/page','https://example.com/page')",
            "INSERT INTO search_document_profiles VALUES(1,1,1,10)",
            "UPDATE search_document_profiles SET visible_visit_count=0 WHERE page_id=1 AND source_profile_id=2",
            "INSERT INTO search_outbox(page_id,change) VALUES(1,'unknown')",
        ] {
            assert!(
                store
                    .write(Db::Archive, Priority::Interactive, move |tx| {
                        tx.execute(sql, ())?;
                        Ok(())
                    })
                    .is_err(),
                "{sql}"
            );
        }
    });
}

#[test]
fn natural_keys() {
    each_store(|store| {
        for (db, rows, sql) in [
            (
                Db::Archive,
                archive::ROWS,
                "INSERT INTO urls(page_id,url,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id) VALUES(1,'sample',1,'now',1,'now',1,1,1)",
            ),
            (
                Db::Intelligence,
                intelligence::ROWS,
                "INSERT INTO ai_embeddings(history_id,profile_id,url,domain,visited_at,content_hash,content_bytes,provider_id,model,indexed_at) VALUES(1,'sample','sample','sample','sample','sample',1,'sample','sample','sample')",
            ),
        ] {
            store.write(db, Priority::Batch, move |tx| seed(tx, rows)).unwrap();
            let result = store.write(db, Priority::Interactive, move |tx| {
                tx.execute(sql, ())?;
                Ok(())
            });
            let error = result.expect_err("duplicate natural key");
            assert!(error.to_string().to_ascii_lowercase().contains("unique"), "{error}");
        }
    });
}
