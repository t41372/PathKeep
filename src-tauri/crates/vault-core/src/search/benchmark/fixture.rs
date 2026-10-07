//! Responsibilities: bounded synthetic canonical batches with known search frequencies.
//! Not responsible for: benchmark timing or changing storage/indexer defaults.
//! Failure modes: visit/page counts drift; data is plaintext; fixtures fill RAM/disk.
//! The ignored owner benchmark checks physical row counts and the 40 GB ceiling.
use super::{KEY, PAGES, VISITS, measure};
use crate::{AppConfig, ArchiveMode, Vault};
use vault_store::{Db, Priority, params};
pub(super) const BASE: i64 = 1_700_000_000_000;
pub(super) fn open(root: &std::path::Path) -> Vault {
    Vault::open(
        crate::project_paths_with_root(root),
        AppConfig {
            initialized: true,
            git_enabled: false,
            archive_mode: ArchiveMode::Encrypted,
            ..Default::default()
        },
        Some(KEY),
    )
    .unwrap()
}
pub(super) fn seed(vault: &Vault) {
    vault.store().write(Db::Archive,Priority::Batch,|s| {
        for id in 1..=3 {
            s.execute("INSERT INTO source_profiles(id,browser_kind,profile_key,profile_name,profile_path,discovered_at) VALUES(?1,'chrome',?2,'bench','bench','now')",params![id,format!("bench:{id}")])?;
        }Ok(())
    }).unwrap();
    let start = std::time::Instant::now();
    for first in (1..=PAGES).step_by(2000) {
        let last = (first + 1999).min(PAGES);
        vault.store().write(Db::Archive,Priority::Batch,move|s| {
            let ids="WITH RECURSIVE ids(id) AS (SELECT ?1 UNION ALL SELECT id+1 FROM ids WHERE id<?2)";
            s.execute(&format!("{ids} INSERT INTO pages(id,url,canonical_url,registrable_domain) SELECT id,'https://site'||(id%20000)||'.example/articles/'||id,'https://site'||(id%20000)||'.example/articles/'||id,'site'||(id%20000)||'.example' FROM ids"),params![first,last])?;
            s.execute("INSERT INTO urls(id,page_id,url,title,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id) SELECT id,id,url,'archive guide topic '||(id%16)||CASE WHEN id%317=0 THEN ' webassembly' ELSE '' END||CASE WHEN id=1200001 THEN ' raresentinel' ELSE '' END,0,'',?3+(id*5-MIN(id,100000))*1000,'',id%3+1,0 FROM pages WHERE id>=?1 AND id<=?2",params![first,last,BASE])?;
            // First 100k pages have four visits, every later page five: exactly 14.4M.
            for number in 1..=5 {
                s.execute("INSERT INTO visits(id,url_id,source_profile_id,created_by_run_id,visit_time_ms,visit_time_iso) SELECT (id-1)*5-MIN(id-1,100000)+?3,id,source_profile_id,0,?4+((id-1)*5-MIN(id-1,100000)+?3)*1000,'' FROM urls WHERE id>=?1 AND id<=?2 AND (?3<5 OR id>100000)",params![first,last,number,BASE])?;
            }
            s.execute("INSERT INTO search_documents(page_id,url,title,search_terms,normalized_url,normalized_title,normalized_search_terms,cjk_grams,compact_text,domain,newest_visit_ms,updated_at,enrichment_text,notes_text,tags_text) SELECT id,url,title,'',url,title,'','',replace(url||title,' ',''),'site'||(id%20000)||'.example',last_visit_ms,'now',CASE WHEN id%7=0 THEN 'reference documentation examples community tutorials' ELSE '' END,CASE WHEN id%13=0 THEN 'read again compare project design' ELSE '' END,CASE WHEN id%11=0 THEN 'reference reading' ELSE '' END FROM urls WHERE id>=?1 AND id<=?2",params![first,last])?;
            s.execute("INSERT INTO search_document_profiles(page_id,source_profile_id,visible_visit_count,newest_visit_ms) SELECT id,source_profile_id,CASE WHEN id<=100000 THEN 4 ELSE 5 END,last_visit_ms FROM urls WHERE id>=?1 AND id<=?2",params![first,last])?;
            s.execute("INSERT INTO search_outbox(page_id,change) SELECT id,'upsert' FROM pages WHERE id>=?1 AND id<=?2 ORDER BY id",params![first,last])?;
            Ok(())
        }).unwrap();
        if last % 100000 == 0 || last == PAGES {
            assert!(measure::disk(&vault.paths().app_root) < 40_000_000_000);
            println!(
                "{}",
                serde_json::json!({"case":"seed-progress","pages":last,"seconds":start.elapsed().as_secs_f64(),"indexer":vault.search().state(),"rssMiB":measure::rss(),"uptime":measure::uptime()})
            );
        }
    }
    vault
        .store()
        .write(Db::Archive, Priority::Batch, |s| {
            assert_eq!(
                s.query_one("SELECT COUNT(*) FROM visits", (), |r| Ok(r.get::<i64>(0)?))?,
                VISITS
            );
            assert_eq!(
                s.query_one("SELECT COUNT(*) FROM pages", (), |r| Ok(r.get::<i64>(0)?))?,
                PAGES
            );
            for (name, value) in [("visits", VISITS), ("urls", PAGES), ("sourceProfiles", 3)] {
                s.execute(
                    "UPDATE archive_totals SET value=?2 WHERE name=?1",
                    params![name, value],
                )?;
            }
            Ok(())
        })
        .unwrap();
}
