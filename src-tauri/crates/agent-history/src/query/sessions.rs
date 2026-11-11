//! Responsibilities: keyset reads over published session metadata.
//! Not responsible for: native session identity merging or source discovery.

use super::{
    Page, SessionQuery, SessionSort, SessionView, cursor, generation, operation, page_limit,
};
use crate::{Error, Result, archive::Archive};
use vault_store::{Db, Row, Value};

const COLUMNS: &str = "id,source_id,native_id,harness,project_path,title,created_at,updated_at,origin,harness_version,parent_session_id,model,revision_id,\
    CASE WHEN ah_sessions.complete=1 AND coalesce(\
    (SELECT ah_coverage.complete FROM ah_coverage WHERE ah_coverage.source_id=ah_sessions.source_id),1)=1 \
    THEN 1 ELSE 0 END,projection_id,parser_version";
const PAGE_BYTES: usize = 4 * 1024 * 1024;

impl Archive {
    /// Reads one current interpretation; raw revisions remain independently
    /// addressable when a newer source cannot be interpreted.
    pub fn session(&self, id: &str) -> Result<SessionView> {
        let id = id.to_owned();
        let requested = id.clone();
        let found = self.store.read(Db::Archive, move |session| {
            Ok(session
                .query(&format!("SELECT {COLUMNS} FROM ah_sessions WHERE id=?1"), [id], view)?
                .pop())
        })?;
        found.ok_or(Error::NotFound(requested))
    }

    /// Lists sources in a stable order, with missing native timestamps last.
    /// Restart the query if any projection was published between its pages.
    pub fn list_sessions(&self, query: &SessionQuery) -> Result<Page<SessionView>> {
        let limit = page_limit(query.limit)?;
        let scope = cursor::scope(&(&query.harness, &query.project_path, query.sort))?;
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
        let (generation, page) = self.store.read(Db::Archive, move |session| {
            let current = generation(session)?;
            if after.as_ref().is_some_and(|cursor| cursor.generation != current) {
                return Ok(Err(cursor::stale()));
            }
            let column = match sort {
                SessionSort::Updated => "updated_at",
                SessionSort::Created => "created_at",
            };
            let mut base = format!("SELECT {COLUMNS} FROM ah_sessions WHERE 1=1");
            let mut filters = Vec::new();
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
                    sql.push_str(&format!(" AND ({column},id)<(?,?)"));
                    values.extend([
                        Value::Integer(after.timestamp.expect("the timestamp phase has a value")),
                        Value::Text(after.id.clone()),
                    ]);
                }
                sql.push_str(&format!(" ORDER BY {column} DESC,id DESC LIMIT ?"));
                values.push(Value::Integer(limit + 1));
                page.read(session, &sql, values)?;
            }
            if !page.has_more {
                let mut sql = format!("{base} AND {column} IS NULL");
                if let Some(after) = &after
                    && after.timestamp.is_none()
                {
                    sql.push_str(" AND id<?");
                    filters.push(Value::Text(after.id.clone()));
                }
                sql.push_str(" ORDER BY id DESC LIMIT ?");
                filters.push(Value::Integer(limit + 1 - page.items.len() as i64));
                page.read(session, &sql, filters)?;
            }
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
        Ok(Page { items: page.items, next_cursor })
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
    })
}
