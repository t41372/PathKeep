//! Responsibilities: keyset reads over published session metadata.
//! Not responsible for: native session identity merging or source discovery.

use super::{
    Page, SessionQuery, SessionSort, SessionView, cursor, generation, operation, page_limit,
};
use crate::{Error, Result, archive::Archive};
use vault_store::{Db, Row, Value};

fn columns() -> String {
    let parent = super::parents::native_link("parent_session_id", "parent_path");
    let fork = super::parents::native_link("forked_from_native_id", "fork_path");
    format!(
        "s.conversation_id,s.source_id,s.native_id,s.harness,s.project_path,
         coalesce((SELECT title FROM ah_user_state WHERE conversation_id=s.conversation_id),
           s.title,(SELECT substr(e.text,1,160) FROM ah_events e
           WHERE e.session_id=s.id AND e.projection_id=s.projection_id
             AND e.role='user' AND e.text<>'' ORDER BY e.ordinal LIMIT 1)),
         s.created_at,s.updated_at,s.origin,s.harness_version,s.parent_session_id,s.model,
         s.revision_id,CASE WHEN s.complete=1 AND coalesce(
           (SELECT complete FROM ah_coverage WHERE source_id=s.source_id),1)=1
           THEN 1 ELSE 0 END,s.projection_id,s.parser_version,{parent},
         coalesce((SELECT starred FROM ah_user_state WHERE conversation_id=s.conversation_id),0),
         coalesce((SELECT put_away FROM ah_user_state WHERE conversation_id=s.conversation_id),0),
         (SELECT trashed_at FROM ah_user_state WHERE conversation_id=s.conversation_id),
         CASE WHEN (SELECT title FROM ah_user_state WHERE conversation_id=s.conversation_id)
           IS NOT NULL THEN 'user' ELSE s.title_source END,s.forked_from_native_id,{fork}"
    )
}
const PAGE_BYTES: usize = 4 * 1024 * 1024;

impl Archive {
    /// Reads one current interpretation; raw revisions remain independently
    /// addressable when a newer source cannot be interpreted.
    pub fn session(&self, id: &str) -> Result<SessionView> {
        let _job = self.jobs.enter(None)?;
        let id = id.to_owned();
        let requested = id.clone();
        let columns = columns();
        let found = self.store.read(Db::AgentHistoryReadModel, move |session| {
            let mut items = session.query(
                &format!(
                    "SELECT {columns} FROM ah_sessions s
                    WHERE conversation_id=?1 AND conversation_current=1"
                ),
                [id],
                view,
            )?;
            super::conversation_sources::attach(session, &mut items).map_err(operation)?;
            Ok(items.pop())
        })?;
        let mut item = found.ok_or(Error::NotFound(requested))?;
        self.source_presence(std::slice::from_mut(&mut item))?;
        Ok(item)
    }

    /// Lists sources in a stable order, with missing native timestamps last.
    /// Restart the query if any projection was published between its pages.
    pub fn list_sessions(&self, query: &SessionQuery) -> Result<Page<SessionView>> {
        let _job = self.jobs.enter(None)?;
        let limit = page_limit(query.limit)?;
        if !["exclude", "only", "include"].contains(&query.put_away.as_str())
            || !["exclude", "only"].contains(&query.trash.as_str())
        {
            return Err(Error::InvalidInput("invalid conversation filters".into()));
        }
        let put_away = query.put_away.clone();
        let trash = query.trash.clone();
        let starred = query.starred;
        let scope = cursor::scope(&(
            &query.harness,
            &query.project_path,
            query.sort,
            &query.put_away,
            &query.trash,
            query.starred,
        ))?;
        let after =
            query.after.as_deref().map(cursor::decode::<cursor::SessionCursor>).transpose()?;
        if let Some(after) = &after {
            cursor::valid_version(after.version)?;
            if after.scope != scope {
                return Err(cursor::invalid());
            }
        }
        let harness = query.harness.as_ref().map(ToString::to_string);
        let project = query.project_path.clone();
        let sort = query.sort;
        let hidden = super::parents::hidden_query();
        let columns = columns();
        let (generation, page) = self.store.read(Db::AgentHistoryReadModel, move |session| {
            let current = generation(session)?;
            if after.as_ref().is_some_and(|cursor| cursor.generation != current) {
                return Ok(Err(cursor::stale()));
            }
            let column = match sort {
                SessionSort::Updated => "updated_at",
                SessionSort::Created => "created_at",
            };
            let mut base = format!(
                "
                SELECT {columns} FROM ah_sessions s WHERE conversation_current=1"
            );
            let mut filters = Vec::new();
            if trash == "only" {
                base.push_str(
                    " AND EXISTS(SELECT 1 FROM ah_user_state u WHERE
                    u.conversation_id=s.conversation_id AND u.trashed_at IS NOT NULL)",
                );
            } else {
                base.push_str(&format!(" AND s.conversation_id NOT IN ({hidden})"));
                match put_away.as_str() {
                    "only" => base.push_str(
                        " AND EXISTS(SELECT 1 FROM ah_user_state u WHERE
                        u.conversation_id=s.conversation_id AND u.put_away=1)",
                    ),
                    "exclude" => base.push_str(
                        " AND NOT EXISTS(SELECT 1 FROM ah_user_state u WHERE
                        u.conversation_id=s.conversation_id AND u.put_away=1)",
                    ),
                    _ => {}
                }
            }
            if starred {
                base.push_str(
                    " AND EXISTS(SELECT 1 FROM ah_user_state u WHERE
                    u.conversation_id=s.conversation_id AND u.starred=1)",
                );
            }
            if let Some(harness) = harness {
                base.push_str(" AND harness=?");
                filters.push(Value::Text(harness));
            }
            if let Some(project) = project {
                base.push_str(" AND project_path=?");
                filters.push(Value::Text(project));
            }
            let mut page = MetadataPage {
                items: Vec::new(),
                bytes: 0,
                has_more: false,
                limit: limit as usize,
            };
            // Two index seeks keep unknown timestamps last without an OR that
            // can force each later page to scan all previously visited rows.
            if after.as_ref().is_none_or(|after| after.timestamp.is_some()) {
                let mut sql = format!("{base} AND {column} IS NOT NULL");
                let mut values = filters.clone();
                if let Some(after) = &after {
                    sql.push_str(&format!(" AND ({column}, conversation_id)<(?, ?)"));
                    values.extend([
                        Value::Integer(after.timestamp.expect("the timestamp phase has a value")),
                        Value::Text(after.id.clone()),
                    ]);
                }
                sql.push_str(&format!(" ORDER BY {column} DESC, conversation_id DESC LIMIT ?"));
                values.push(Value::Integer(limit + 1));
                page.read(session, &sql, values)?;
            }
            if !page.has_more {
                let mut sql = format!("{base} AND {column} IS NULL");
                if let Some(after) = &after
                    && after.timestamp.is_none()
                {
                    sql.push_str(" AND conversation_id<?");
                    filters.push(Value::Text(after.id.clone()));
                }
                sql.push_str(" ORDER BY conversation_id DESC LIMIT ?");
                filters.push(Value::Integer(limit + 1 - page.items.len() as i64));
                page.read(session, &sql, filters)?;
            }
            super::conversation_sources::attach(session, &mut page.items).map_err(operation)?;
            Ok(Ok((current, page)))
        })??;
        let next_cursor = if page.has_more {
            let last = page.items.last().expect("a metadata page always admits its first record");
            Some(cursor::encode(&cursor::SessionCursor {
                version: cursor::version(),
                scope,
                generation,
                timestamp: match query.sort {
                    SessionSort::Updated => last.updated_at,
                    SessionSort::Created => last.created_at,
                },
                id: last.id.clone(),
            })?)
        } else {
            None
        };
        let mut items = page.items;
        self.source_presence(&mut items)?;
        Ok(Page { items, next_cursor })
    }
}

struct MetadataPage {
    items: Vec<SessionView>,
    bytes: usize,
    has_more: bool,
    limit: usize,
}

impl MetadataPage {
    fn read(
        &mut self,
        session: &vault_store::Session<'_>,
        sql: &str,
        values: Vec<Value>,
    ) -> vault_store::Result<()> {
        let mut statement = session.prepare(sql)?;
        let mut rows = statement.query(values)?;
        while let Some(row) = rows.next()? {
            if self.items.len() == self.limit {
                self.has_more = true;
                break;
            }
            let item = view(&row)?;
            let size = metadata_bytes(&item);
            if !self.items.is_empty() && self.bytes.saturating_add(size) > PAGE_BYTES {
                self.has_more = true;
                break;
            }
            self.bytes = self.bytes.saturating_add(size);
            self.items.push(item);
        }
        Ok(())
    }
}

fn metadata_bytes(item: &SessionView) -> usize {
    [
        Some(item.id.as_str()),
        Some(item.source_id.as_str()),
        Some(item.native_id.as_str()),
        item.project_path.as_deref(),
        item.title.as_deref(),
        Some(item.origin.as_str()),
        item.harness_version.as_deref(),
        item.parent_session_id.as_deref(),
        item.model.as_deref(),
        Some(item.parser_version.as_str()),
    ]
    .into_iter()
    .flatten()
    .fold(0usize, |total, value| total.saturating_add(value.len()))
}

fn view(row: &Row) -> vault_store::Result<SessionView> {
    Ok(SessionView {
        id: row.get(0)?,
        source_id: row.get(1)?,
        native_id: row.get(2)?,
        harness: row.get::<String>(3)?.parse().map_err(operation)?,
        project_path: row.get(4)?,
        title: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        origin: row.get(8)?,
        harness_version: row.get(9)?,
        parent_session_id: row.get(10)?,
        model: row.get(11)?,
        revision_id: row.get(12)?,
        complete: row.get::<i64>(13)? != 0,
        projection_id: row.get(14)?,
        parser_version: row.get(15)?,
        sources: Vec::new(),
        parent_native_id: row.get(10)?,
        parent_id: row.get(16)?,
        only_in_archive: false,
        attachments_not_kept: 0,
        starred: row.get::<i64>(17)? != 0,
        put_away: row.get::<i64>(18)? != 0,
        trashed: row.get::<Option<i64>>(19)?.is_some(),
        trashed_at: row.get(19)?,
        title_source: row.get(20)?,
        forked_from_native_id: row.get(21)?,
        forked_from_id: row.get(22)?,
    })
}
