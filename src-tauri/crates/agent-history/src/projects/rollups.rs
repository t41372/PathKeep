//! Maintain all-time folder/tool counts at publication and mark boundaries. Regrouping
//! moves these counts between projects without rereading transcripts or turn facts.
use super::D;
use crate::{Archive, Result};
use vault_store::{Priority, Session};

pub(crate) fn dirty_conversation(tx: &Session<'_>, id: &str) -> vault_store::Result<()> {
    tx.execute(
        "WITH RECURSIVE lineage(id,parent,folder,root) AS (
        SELECT id,parent,folder,root FROM ah_chat_summary WHERE id=?1 AND live=1 UNION ALL
        SELECT s.id,s.parent,s.folder,s.root FROM lineage JOIN ah_chat_summary s ON s.id=lineage.parent AND s.live=1)
        INSERT OR IGNORE INTO ah_project_dirty SELECT folder FROM lineage WHERE root=1 AND folder IS NOT NULL",
        [id],
    )?;
    Ok(())
}
// The published root already includes its children's attributed tokens. Most archives can use
// those totals directly. An independently trashed child requires the visible family walk.
const FROM_ROOTS: &str = "WITH totals AS MATERIALIZED (
    SELECT s.harness,count(*) n,sum(s.children) ch,sum(s.agent_ms) ms,sum(s.tokens_total) tokens,count(s.tokens_total) known,
        min(s.started) first_at,max(s.last_activity) last_at,max(s.recent_key) latest
    FROM ah_chat_summary s INDEXED BY ah_chat_folder LEFT JOIN ah_user_state u ON u.conversation_id=s.id
    WHERE s.live=1 AND s.root=1 AND s.folder=?1 AND u.trashed_at IS NULL GROUP BY s.harness)";
const FROM_FAMILIES: &str = "WITH RECURSIVE roots AS MATERIALIZED (
    SELECT s.id,s.harness,s.children,s.tokens_total,s.agent_ms,s.started,s.last_activity,s.recent_key
    FROM ah_chat_summary s LEFT JOIN ah_user_state u ON u.conversation_id=s.id
    WHERE s.live=1 AND s.root=1 AND s.folder=?1 AND u.trashed_at IS NULL),
    family(root,id,own) AS (
        SELECT r.id,r.id,s.own FROM roots r JOIN ah_chat_summary s ON s.id=r.id AND s.live=1 WHERE r.children>0
        UNION ALL SELECT f.root,c.id,c.own FROM family f JOIN ah_chat_summary c ON c.parent=f.id AND c.live=1
        WHERE NOT EXISTS(SELECT 1 FROM ah_user_state u WHERE u.conversation_id=c.id AND u.trashed_at IS NOT NULL)),
    families AS (SELECT root,count(*)-1 children,sum(json_extract(own,'$.tokens.input')+json_extract(own,'$.tokens.output')
        +json_extract(own,'$.tokens.cacheRead')+json_extract(own,'$.tokens.cacheWrite')) tokens FROM family GROUP BY root),
    amounts AS (SELECT r.*,coalesce(f.children,0) subagents,CASE WHEN r.children=0 THEN r.tokens_total ELSE f.tokens END known_tokens
        FROM roots r LEFT JOIN families f ON f.root=r.id),
    totals AS (SELECT harness,count(*) n,sum(subagents) ch,sum(agent_ms) ms,sum(known_tokens) tokens,count(known_tokens) known,
        min(started) first_at,max(last_activity) last_at,max(recent_key) latest FROM amounts GROUP BY harness)";
const SAVE: &str = "INSERT INTO ah_project_rollup SELECT ?1,t.harness,t.n,t.ch,t.ms,t.tokens,t.known,t.first_at,t.last_at,t.latest,
    (SELECT coalesce(u.title,s.title) FROM ah_chat_summary s LEFT JOIN ah_user_state u ON u.conversation_id=s.id
     WHERE s.live=1 AND s.root=1 AND s.recent_key=t.latest LIMIT 1) FROM totals t";
impl Archive {
    pub(crate) fn refresh_project_rollups(&self) -> Result<()> {
        loop {
            if self.store.read(D, |s| {
                s.query_one("SELECT NOT EXISTS(SELECT 1 FROM ah_project_dirty)", (), |r| {
                    Ok(r.get::<i64>(0)? != 0)
                })
            })? {
                return Ok(());
            }
            self.store.write(D, Priority::Batch, |tx| {
                let paths = tx.query("SELECT path FROM ah_project_dirty ORDER BY path LIMIT 32", (), |r| {
                    Ok(r.get::<String>(0)?)
                })?;
                let hidden = tx.query_one(
                    "SELECT EXISTS(SELECT 1 FROM ah_user_state u WHERE u.trashed_at IS NOT NULL
                    AND EXISTS(SELECT 1 FROM ah_chat_summary s WHERE s.id=u.conversation_id AND s.live=1 AND s.parent IS NOT NULL))",
                    (),
                    |r| Ok(r.get::<i64>(0)? != 0),
                )?;
                let query = format!("{} {SAVE}", if hidden { FROM_FAMILIES } else { FROM_ROOTS });
                let mut insert = tx.prepare(&query)?;
                for path in paths {
                    tx.execute("DELETE FROM ah_project_rollup WHERE path=?1", [path.as_str()])?;
                    insert.execute([path.as_str()])?;
                    tx.execute("DELETE FROM ah_project_dirty WHERE path=?1", [path])?;
                }
                Ok(())
            })?;
        }
    }
}
