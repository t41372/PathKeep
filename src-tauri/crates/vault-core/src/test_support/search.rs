//! Responsibilities: build canonical search facts for transport integration tests.
//! Not responsible for: returning expected results or bypassing the owner indexer.
//! Failure modes: transport fixtures never index nonempty data, miss old records,
//! or use a different storage mode from their owning Vault.
use crate::Vault;
use vault_store::{Db, Priority, params};

pub fn seed_pages(vault: &Vault, count: i64) {
    vault.store().write(Db::Archive, Priority::Batch, move |s| {
        s.execute("INSERT INTO source_profiles(id,profile_key,browser_kind,profile_name,profile_path,discovered_at) VALUES(1,'chrome:Default','chrome','Default','/fixture','now')", ())?;
        for id in 1..=count {
            let url = format!("https://transport.test/page/{id}");
            let title = if id == 1 {"common ancienttransport"} else {"common transport"};
            s.execute("INSERT INTO pages(id,url,canonical_url) VALUES(?1,?2,?2)", params![id,url.as_str()])?;
            s.execute("INSERT INTO urls(id,page_id,url,title,source_profile_id,source_url_id,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,created_by_run_id) VALUES(?1,?1,?2,?3,1,?1,?1,'then',?1,'then',0)", params![id,url,title])?;
            s.execute("INSERT INTO visits(id,url_id,source_profile_id,visit_time_ms,visit_time_iso,created_by_run_id) VALUES(?1,?1,1,?1,'then',0)", [id])?;
            crate::archive::search_documents::refresh(s,id)?;
        }
        Ok(())
    }).unwrap();
    vault.search().cycle().unwrap();
}
