//! Responsibilities: prove backfill coverage beyond the former candidate-pool limit.
//! Not responsible for: provider networking or query ranking.
//! Failure modes: the 50,000-page pool truncates a build, a batch size stops the walk,
//! or durable metadata and vector maps disagree about completed pages.
use super::*;

/// One encrypted archive, as users keep theirs: whether the walk is capped does not depend on
/// encryption. Batches of 1,000 pages still end on a partial one (50,021 pages).
#[test]
fn backfill_reaches_every_distinct_page_beyond_fifty_thousand() {
    const TOTAL: i64 = 50_021;
    {
        let vault =
            crate::Vault::open_temp_with_key(Some([101; 32])).unwrap().with_config(base_config());
        vault.write(Db::Archive,Priority::Batch,|s| {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,profile_key,discovered_at) VALUES(1,'chrome','Default','/fixture','chrome:Default','')",())?;Ok(())
        }).unwrap();
        for first in (1..=TOTAL).step_by(512) {
            let last = (first + 511).min(TOTAL);
            vault.write(Db::Archive,Priority::Batch,move |s| {
                s.execute("WITH RECURSIVE ids(id) AS (VALUES(?1) UNION ALL SELECT id+1 FROM ids WHERE id<?2) INSERT INTO pages(id,url,canonical_url,registrable_domain) SELECT id,'https://example.com/'||id,'https://example.com/'||id,'example.com' FROM ids",params![first,last])?;
                s.execute("INSERT INTO urls(id,page_id,url,title,source_profile_id,created_by_run_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso) SELECT id,id,url,'Page',1,0,1700000000000,'',1700000000000,'' FROM pages WHERE id>=?1 AND id<=?2",params![first,last])?;
                s.execute("INSERT INTO visits(id,url_id,source_visit_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) SELECT id,id,CAST(id AS TEXT),1,0,1700000000000,'' FROM pages WHERE id>=?1 AND id<=?2",params![first,last])?;Ok(())
            }).unwrap();
        }
        vault
            .write(Db::Archive, Priority::Batch, |s| {
                s.execute(
                    "UPDATE archive_totals SET value=?1 WHERE name IN ('visits','urls')",
                    [TOTAL],
                )?;
                s.execute(
                    "INSERT INTO archive_totals(name,value) VALUES('sourceVisits:1',?1)",
                    [TOTAL],
                )?;
                Ok(())
            })
            .unwrap();
        let runtime = Runtime::new().unwrap();
        let provider = embedding_provider();
        let report = runtime
            .block_on(build_ai_index(
                &vault,
                &provider,
                &AiIndexRequest { limit: Some(1_000), ..Default::default() },
            ))
            .unwrap();
        assert_eq!(report.indexed_items, TOTAL as usize);
        let map = VisitContentMap::for_provider(
            vault.paths(),
            &provider.config.id,
            &provider.config.default_model,
        );
        let ids = map.mapped_history_ids().unwrap();
        assert_eq!(ids.len(), TOTAL as usize);
        assert!((1..=TOTAL).all(|id| ids.contains(&id)));
        let count = vault
            .read(Db::Intelligence, |s| {
                Ok(s.query_one("SELECT COUNT(DISTINCT history_id) FROM ai_embeddings", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })?)
            })
            .unwrap();
        assert_eq!(count, TOTAL);
        assert_eq!(
            VectorStore::for_provider(
                vault.paths(),
                &provider.config.id,
                &provider.config.default_model
            )
            .count()
            .unwrap(),
            TOTAL as u64
        );
    }
}
