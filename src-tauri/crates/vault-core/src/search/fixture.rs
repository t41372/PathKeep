//! Responsibilities: port legacy search fixtures to real canonical Turso writes.
//! Not responsible for: simulating FTS or changing expected query semantics.
//! Failure modes: profile copies become pages, zero timestamps imply visibility, fixtures
//! bypass outbox. The ported suites verify every generated page through the owner indexer.
use crate::{HistoryEntry, HistoryQuery, HistoryQueryResponse, Vault};
use vault_store::{Db, Priority, params};
pub(super) const CHROME: i64 = 1;
pub(super) const FIREFOX: i64 = 2;
pub(super) struct Archive {
    pub vault: Vault,
}
impl Archive {
    pub fn new() -> Self {
        Self::with_key(None)
    }
    pub fn with_key(key: Option<[u8; 32]>) -> Self {
        let vault = Vault::open_temp_with_key(key).unwrap();
        vault.store().write(Db::Archive,Priority::Batch,|s| {
            for (id,kind,key) in [(1,"chrome","chrome:Default"),(2,"firefox","firefox:main"),(3,"chrome","chrome:Work")] {
                s.execute("INSERT INTO source_profiles(id,browser_kind,profile_name,profile_path,discovered_at,profile_key) VALUES(?1,?2,'profile','/tmp/profile','now',?3)",params![id,kind,key])?;
            }Ok(())
        }).unwrap();
        Self { vault }
    }
    pub fn url(&self, id: i64, profile: i64, url: &str, title: &str, times: &[i64]) -> &Self {
        let url = url.to_owned();
        let title = title.to_owned();
        let times = times.to_vec();
        self.vault.store().write(Db::Archive,Priority::Batch,move |s| {
            s.execute("INSERT INTO pages(url,canonical_url) VALUES(?1,?1) ON CONFLICT(url) DO NOTHING",[url.as_str()])?;
            let page=s.query_one("SELECT id FROM pages WHERE url=?1",[url.as_str()],|r|Ok(r.get::<i64>(0)?))?;
            let newest=times.iter().max().copied().unwrap_or(0);
            s.execute("INSERT INTO urls(id,page_id,url,title,visit_count,typed_count,first_visit_ms,first_visit_iso,last_visit_ms,last_visit_iso,source_profile_id,created_by_run_id,source_url_id,hidden) VALUES(?1,?2,?3,?4,0,0,0,'',?5,'',?6,0,?1,0)",params![id,page,url,title,newest,profile])?;
            for time in times {s.execute("INSERT INTO visits(url_id,source_visit_id,visit_time_ms,visit_time_iso,source_profile_id,created_by_run_id) VALUES(?1,?2,?3,'',?4,0)",params![id,id*1_000_000+time,time,profile])?;}
            crate::archive::search_documents::refresh(s,page)
        }).unwrap();
        self
    }
    pub fn index(&self) {
        self.vault.search().cycle().unwrap();
    }
    pub fn search(&self, query: HistoryQuery) -> HistoryQueryResponse {
        super::history(&self.vault, query).unwrap()
    }
    pub fn words(&self, q: &str) -> HistoryQueryResponse {
        self.search(HistoryQuery { q: Some(q.into()), ..Default::default() })
    }
    pub fn walk(&self, mut query: HistoryQuery, limit: u32) -> Vec<HistoryEntry> {
        query.limit = Some(limit);
        let mut rows = Vec::new();
        loop {
            let page = self.search(query.clone());
            rows.extend(page.items);
            if page.next_cursor.is_none() {
                break;
            }
            query.cursor = page.next_cursor;
        }
        rows
    }
    pub fn tag(&self, url: &str, tag: &str) -> &Self {
        let mut tags =
            crate::get_annotation(&self.vault, url).unwrap().map(|a| a.tags).unwrap_or_default();
        tags.push(tag.into());
        crate::replace_tags(
            &self.vault,
            crate::ReplaceTagsRequest { url: url.into(), tags, source_profile: None },
        )
        .unwrap();
        self.index();
        self
    }
}
