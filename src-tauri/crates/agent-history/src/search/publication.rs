//! Publish the source and card selectors paired with a complete Tantivy commit. Retain old
//! selectors and their event/summary rows while a search is paging through that epoch.
use crate::{Archive, Result};
use vault_store::{Db, Priority, params};
const DB: Db = Db::AgentHistoryReadModel;

impl Archive {
    pub(super) fn search_cards_dirty(&self) -> Result<bool> {
        Ok(self.store.read(DB, |s| {
            s.query_one("SELECT EXISTS(SELECT 1 FROM ah_search_dirty)", (), |r| {
                Ok(r.get::<i64>(0)? != 0)
            })
        })?)
    }
    pub(super) fn publish_search_sources(
        &self,
        previous: i64,
        target: i64,
        rebuilt: bool,
    ) -> Result<()> {
        self.store.write(DB, Priority::Batch, move |tx| {
            let epoch = tx.query_one(
                "SELECT CAST(value AS INTEGER)+1 FROM ah_meta WHERE key='search_epoch'",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?;
            let changes = "SELECT session_id FROM ah_outbox WHERE seq>?2 AND seq<=?3";
            let mut retire = String::from("UPDATE ah_search_source SET retired=?1 WHERE retired IS NULL");
            let mut insert = String::from(
                "INSERT INTO ah_search_source SELECT id,projection_id,conversation_id,harness,
                CASE WHEN harness='deepseek' THEN (SELECT json_extract(payload,'$.version') FROM ah_events WHERE session_id=s.id AND projection_id=s.projection_id AND ordinal=0) END,
                ?1,NULL FROM ah_sessions s WHERE (current_only=0 OR conversation_current=1)",
            );
            if rebuilt {
                tx.execute(&retire, [epoch])?;
                tx.execute(&insert, [epoch])?;
            } else {
                retire.push_str(&format!(" AND source IN ({changes})"));
                insert.push_str(&format!(" AND id IN ({changes})"));
                tx.execute(&retire, params![epoch, previous, target])?;
                tx.execute(&insert, params![epoch, previous, target])?;
            }
            if rebuilt {
                tx.execute(
                    "INSERT OR IGNORE INTO ah_search_dirty SELECT id FROM ah_search_card WHERE retired IS NULL",
                    (),
                )?;
                tx.execute(
                    "INSERT OR IGNORE INTO ah_search_dirty SELECT id FROM ah_chat_summary WHERE live=1",
                    (),
                )?;
            }
            let old_roots = tx.query_one(
                "SELECT count(*) FROM ah_search_dirty d JOIN ah_search_card h ON h.id=d.id WHERE h.root=1 AND h.retired IS NULL",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?;
            let new_roots = tx.query_one(
                "SELECT count(*) FROM ah_search_dirty d JOIN ah_chat_summary s ON s.id=d.id AND s.live=1 WHERE s.root=1",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )?;
            tx.execute(
                "INSERT INTO ah_meta(key,value) VALUES('search_roots',CAST(?1 AS TEXT))
                ON CONFLICT(key) DO UPDATE SET value=CAST(CAST(ah_meta.value AS INTEGER)+?1 AS TEXT)",
                [new_roots - old_roots],
            )?;
            tx.execute(
                "UPDATE ah_search_card SET retired=?1 WHERE id IN (SELECT id FROM ah_search_dirty) AND retired IS NULL",
                [epoch],
            )?;
            tx.execute(
                "INSERT INTO ah_search_card SELECT s.publication,s.id,s.root,s.parent,s.recent_key,?1,NULL
                FROM ah_chat_summary s WHERE s.live=1 AND s.id IN (SELECT id FROM ah_search_dirty)",
                [epoch],
            )?;
            tx.execute("DELETE FROM ah_search_dirty", ())?;
            tx.execute("UPDATE ah_meta SET value=CAST(?1 AS TEXT) WHERE key='search_epoch'", [epoch])?;
            Ok(())
        })?;
        Ok(())
    }
    fn collect_retired(&self, table: &'static str) -> Result<()> {
        loop {
            let Ok(_unused) = self.search_reads.try_write() else {
                self.jobs.request_collection();
                return Ok(());
            };
            if self.jobs.stopping() {
                return Err(crate::Error::Cancelled);
            }
            let removed = self.store.write(DB, Priority::Batch, move |s| {
                s.execute(
                    &format!("DELETE FROM {table} WHERE rowid IN (SELECT rowid FROM {table} WHERE retired IS NOT NULL LIMIT 256)"),
                    (),
                )
            })?;
            if removed < 256 {
                return Ok(());
            }
        }
    }
    pub(crate) fn collect_search_selectors(&self) -> Result<()> {
        self.collect_retired("ah_search_card")?;
        self.collect_retired("ah_search_source")
    }
}
