//! Page the selected root summaries in activity order. A commit epoch binds every source
//! and child lookup. SQL transactions end before text matching or consumer backpressure.
use super::{schema::EventSource, stream::ScreenQuery};
use crate::{
    Archive, Error, Result,
    chats::{
        Flag,
        list::{Filter, project_chip},
    },
};
use serde_json::{Value, json};
use std::collections::HashMap;
use vault_store::{Db, Session, Value as Sql};
const DB: Db = Db::AgentHistoryReadModel;
pub(super) struct Card {
    pub id: String,
    pub title: Option<String>,
    key: String,
    harness: String,
    project: Value,
    active: Option<i64>,
    missing: bool,
    archived: bool,
}
impl Card {
    pub fn fields(&self) -> Value {
        json!({"id":self.id,"title":self.title,"harness":self.harness,"project":self.project,
            "lastActivityAt":self.active,"onlyInPathKeep":self.missing,"archived":self.archived})
    }
}
#[derive(Default)]
pub(super) struct Batch {
    pub roots: Vec<Card>,
    pub children: HashMap<String, Vec<(String, Option<String>)>>,
    pub sources: HashMap<String, Vec<EventSource>>,
}
pub(super) struct Catalog {
    epoch: i64,
    predicate: String,
    args: Vec<Sql>,
    after: Option<String>,
    done: bool,
    include_trashed: bool,
    pub total: i64,
    pub lag: i64,
}
fn selected(alias: &str, epoch: i64) -> String {
    format!("{alias}.born<={epoch} AND ({alias}.retired IS NULL OR {alias}.retired>{epoch})")
}
const ROOTS: &str = "FROM ah_search_card h JOIN ah_chat_summary s ON s.publication=h.publication
    LEFT JOIN ah_user_state u ON u.conversation_id=h.id";
fn slots(count: usize) -> String {
    vec!["?"; count].join(",")
}
impl Archive {
    pub(super) fn search_catalog(&self, request: &ScreenQuery, watermark: i64) -> Result<Catalog> {
        let filters = request.filters.clone();
        self.store
            .read(DB, move |tx| {
                let epoch = tx.query_one("SELECT CAST(value AS INTEGER) FROM ah_meta WHERE key='search_epoch'", (), |r| {
                    Ok(r.get(0)?)
                })?;
                let Filter{sql,args}=Filter::search(&filters,epoch).map_err(crate::query::operation)?;
                let unrestricted=args.is_empty() && filters.flags.is_empty();
                let include_trashed=filters.flags.contains(&Flag::Trash);
                let predicate=format!("{} AND h.root=1 AND {sql}",selected("h",epoch));
                let total = if unrestricted {
                    tx.query_one(
                        "SELECT coalesce((SELECT CAST(value AS INTEGER) FROM ah_meta WHERE key='search_roots'),0)
                    -(SELECT count(*) FROM ah_user_state u WHERE u.trashed_at IS NOT NULL AND EXISTS(
                    SELECT 1 FROM ah_search_card h WHERE h.id=u.conversation_id AND h.root=1 AND h.retired IS NULL))",
                        (),
                        |r| Ok(r.get(0)?),
                    )?
                } else {
                    tx.query_one(&format!("SELECT count(*) {ROOTS} WHERE {predicate}"), args.clone(), |r| {
                        Ok(r.get(0)?)
                    })?
                };
                let lag = tx.query_one(
                    "SELECT count(*) FROM (SELECT id FROM ah_search_dirty UNION
                SELECT coalesce(s.conversation_id,i.conversation,o.session_id) FROM ah_outbox o
                LEFT JOIN ah_sessions s ON s.id=o.session_id LEFT JOIN ah_search_source i ON i.source=o.session_id AND i.retired IS NULL WHERE o.seq>?1)",
                    [watermark],
                    |r| Ok(r.get(0)?),
                )?;
                Ok(Catalog { epoch, predicate, args, after: None, done: false, include_trashed, total, lag })
            })
            .map_err(Into::into)
    }
    pub fn search_marks_generation(&self) -> Result<i64> {
        self.ensure_state_visible()?;
        Ok(self.store.read(DB, |s| {
            s.query_one("SELECT generation+(SELECT CAST(value AS INTEGER) FROM ah_meta WHERE key='project_epoch') FROM ah_state_generation WHERE id=1", (), |r| {
                Ok(r.get(0)?)
            })
        })?)
    }
}
impl Catalog {
    /// Narrow selective searches to the ancestors of body/title candidates. A live user title
    /// always participates, even when edited after the serving index commit.
    pub fn restrict(
        &mut self,
        archive: &Archive,
        projections: Vec<i64>,
        titles: Vec<String>,
    ) -> Result<()> {
        let epoch = self.epoch;
        let ids = archive.store.read(DB, move |tx| {
            let anchors = format!(
                "SELECT conversation FROM ah_search_source i WHERE {} AND projection IN ({})
                UNION SELECT conversation_id FROM ah_user_state WHERE title IS NOT NULL
                UNION SELECT id FROM ah_search_card WHERE id IN ({})",
                selected("i", epoch),
                slots(projections.len()),
                slots(titles.len())
            );
            let sql = format!(
                "WITH RECURSIVE ancestors(id) AS ({anchors} UNION
                SELECT h.parent FROM ancestors a JOIN ah_search_card h ON h.id=a.id WHERE h.parent IS NOT NULL AND {})
                SELECT DISTINCT id FROM ancestors LIMIT 257",
                selected("h", epoch)
            );
            let args = projections.into_iter().map(Sql::Integer).chain(titles.into_iter().map(Sql::Text)).collect::<Vec<_>>();
            tx.query(&sql, args, |r| Ok(r.get::<String>(0)?))
        })?;
        if ids.len() <= 256 {
            if ids.is_empty() {
                self.done = true;
            } else {
                self.predicate.push_str(&format!(" AND h.id IN ({})", slots(ids.len())));
                self.args.extend(ids.into_iter().map(Sql::Text));
            }
        }
        Ok(())
    }
    pub fn next(&mut self, archive: &Archive) -> Result<Option<Batch>> {
        if self.done {
            return Ok(None);
        }
        let mut predicate = self.predicate.clone();
        let mut args = self.args.clone();
        if let Some(after) = self.after.take() {
            predicate.push_str(" AND h.recent_key<?");
            args.push(Sql::Text(after));
        }
        let epoch = self.epoch;
        let include_trashed = self.include_trashed;
        let batch = archive.store.read(DB, move |tx| {
            let sql = format!(
                "SELECT h.id,coalesce(u.title,s.title),s.harness,(SELECT json_object('id',p.project,'label',p.label,'kind',p.kind) FROM ah_project_folder p WHERE p.path=s.folder),s.last_activity,s.missing,coalesce(u.put_away,0),h.recent_key
                {ROOTS} WHERE {predicate} ORDER BY h.recent_key DESC LIMIT 96"
            );
            let roots = tx.query(&sql, args, card_row)?;
            let mut batch = Batch { roots, ..Default::default() };
            if !batch.roots.is_empty() {
                load_members(tx, &mut batch, epoch, include_trashed)?
            }
            Ok(batch)
        })?;
        self.done = batch.roots.len() < 96;
        self.after = batch.roots.last().map(|r| r.key.clone());
        Ok((!batch.roots.is_empty()).then_some(batch))
    }
}
fn load_members(
    tx: &Session<'_>,
    batch: &mut Batch,
    epoch: i64,
    include_trashed: bool,
) -> vault_store::Result<()> {
    let args = batch.roots.iter().map(|r| Sql::Text(r.id.clone())).collect::<Vec<_>>();
    let starts = vec!["(?)"; args.len()].join(",");
    let allowed = if include_trashed {
        ""
    } else {
        " AND NOT EXISTS(SELECT 1 FROM ah_user_state u WHERE u.conversation_id=c.id AND u.trashed_at IS NOT NULL)"
    };
    let tree = format!(
        "WITH RECURSIVE tree(id) AS (VALUES {starts} UNION SELECT c.id FROM tree JOIN ah_search_card c ON c.parent=tree.id WHERE {}{allowed})",
        selected("c", epoch)
    );
    tx.for_each(
        &format!(
            "{tree} SELECT c.parent,c.id,coalesce(u.title,s.title) FROM tree t JOIN ah_search_card c ON c.id=t.id
        JOIN ah_chat_summary s ON s.publication=c.publication LEFT JOIN ah_user_state u ON u.conversation_id=c.id
        WHERE c.parent IS NOT NULL AND {} ORDER BY s.started,c.id",
            selected("c", epoch)
        ),
        args.clone(),
        |r| {
            batch.children.entry(r.get(0)?).or_default().push((r.get(1)?, r.get(2)?));
            Ok(())
        },
    )?;
    tx.for_each(
        &format!(
            "{tree} SELECT i.conversation,i.source,i.projection,i.harness,i.format,c.publication FROM tree JOIN ah_search_source i ON i.conversation=tree.id JOIN ah_search_card c ON c.id=tree.id AND {cards}
        WHERE {} AND NOT EXISTS(SELECT 1 FROM ah_reader_member m WHERE m.publication=c.publication AND m.source=i.source AND json_extract(m.descriptor,'$.copy_of') IS NOT NULL) ORDER BY i.conversation,i.source",
            selected("i", epoch),
            cards = selected("c", epoch)
        ),
        args,
        |r| {
            batch.sources.entry(r.get(0)?).or_default().push(EventSource {
                session_id: r.get(1)?,
                projection_id: r.get(2)?,
                harness: r.get(3)?,
                format: r.get(4)?,
                publication: r.get(5)?,
                project: String::new(),
            });
            Ok(())
        },
    )
}

impl Archive {
    /// Reader find follows the publications visible in the open conversation. A backup can
    /// publish the conversation before it publishes its global search index.
    pub(super) fn find_tree(&self, id: &str) -> Result<(Batch, bool)> {
        use vault_store::OptionalRow;
        let root = self.reader_header(id)?;
        let publication = root.publication.0;
        Ok(self.store.read(DB, move |tx| {
            let card = tx
                .query_one(
                    "SELECT s.id,coalesce(u.title,s.title),s.harness,(SELECT json_object('id',p.project,'label',p.label,'kind',p.kind) FROM ah_project_folder p WHERE p.path=s.folder),s.last_activity,s.missing,coalesce(u.put_away,0),s.recent_key
                FROM ah_chat_summary s LEFT JOIN ah_user_state u ON u.conversation_id=s.id WHERE s.publication=?1",
                    [publication.as_str()],
                    card_row,
                )
                .optional()?
                .ok_or_else(|| crate::query::operation(Error::ReaderRebuildRequired))?;
            let mut batch = Batch { roots: vec![card], ..Default::default() };
            let tree = "WITH RECURSIVE tree(id,pub) AS (
                SELECT id,publication FROM ah_chat_summary WHERE publication=?1
                UNION SELECT c.id,c.publication FROM tree JOIN ah_chat_summary c ON c.parent=tree.id AND c.live=1
                WHERE NOT EXISTS(SELECT 1 FROM ah_user_state u WHERE u.conversation_id=c.id AND u.trashed_at IS NOT NULL))";
            tx.for_each(
                &format!(
                    "{tree} SELECT s.parent,s.id,coalesce(u.title,s.title) FROM tree JOIN ah_chat_summary s ON s.publication=tree.pub
                LEFT JOIN ah_user_state u ON u.conversation_id=s.id WHERE s.publication<>?1 ORDER BY s.started,s.id"
                ),
                [publication.as_str()],
                |r| {
                    batch.children.entry(r.get(0)?).or_default().push((r.get(1)?, r.get(2)?));
                    Ok(())
                },
            )?;
            let mut complete = root.complete;
            tx.for_each(
                &format!(
                    "{tree} SELECT tree.id,m.source,v.attempt,s.harness,CAST(v.format_version AS INTEGER),tree.pub,m.complete
                FROM tree JOIN ah_chat_summary s ON s.publication=tree.pub JOIN ah_reader_head h ON h.conversation=tree.id
                JOIN ah_reader_member m ON m.publication=tree.pub LEFT JOIN ah_reader_version v ON v.token=m.version
                WHERE json_extract(m.descriptor,'$.copy_of') IS NULL AND (s.harness<>'deepseek' OR m.source=h.current_source)
                ORDER BY tree.id,m.source"
                ),
                [publication.as_str()],
                |r| {
                    complete &= r.get::<i64>(6)? != 0;
                    let Some(attempt) = r.get::<Option<i64>>(2)? else {
                        complete = false;
                        return Ok(());
                    };
                    batch.sources.entry(r.get(0)?).or_default().push(EventSource {
                        session_id: r.get(1)?,
                        projection_id: attempt,
                        harness: r.get(3)?,
                        format: r.get(4)?,
                        publication: r.get(5)?,
                        project: String::new(),
                    });
                    Ok(())
                },
            )?;
            Ok((batch, complete))
        })?)
    }
}

fn card_row(r: &vault_store::Row) -> vault_store::Result<Card> {
    Ok(Card {
        id: r.get(0)?,
        title: r.get(1)?,
        harness: r.get(2)?,
        project: project_chip(r, 3)?,
        active: r.get(4)?,
        missing: r.get::<i64>(5)? != 0,
        archived: r.get::<i64>(6)? != 0,
        key: r.get(7)?,
    })
}
