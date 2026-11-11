//! Responsibilities: bounded event pages from one published interpretation.
//! Not responsible for: flattening branches, following native lineage or reparsing.

use super::{EventQuery, Page, cursor, operation, page_limit};
use crate::{Error, Result, archive::Archive, model::NativeEvent};
use vault_store::{Db, Row};

// A single native record may exceed this budget; returning it alone makes forward
// progress without retaining a page full of large tool outputs in memory.
const PAGE_BYTES: usize = 4 * 1024 * 1024;

impl Archive {
    /// Walks physical event order without converting a branch graph into an
    /// invented linear conversation. Native parent IDs and payloads remain intact.
    pub fn events(&self, query: &EventQuery) -> Result<Page<NativeEvent>> {
        let limit = page_limit(query.limit)?;
        let after =
            query.after.as_deref().map(cursor::decode::<cursor::EventCursor>).transpose()?;
        if let Some(after) = &after {
            cursor::valid_version(after.version)?;
            if after.session_id != query.session_id || after.ordinal > i64::MAX as u64 {
                return Err(cursor::invalid());
            }
        }
        let session_id = query.session_id.clone();
        let (projection_id, items, has_more) = self.store.read(Db::Archive, move |session| {
            let projection = session
                .query("SELECT projection_id FROM ah_sessions WHERE id=?1", [session_id.clone()], |row| {
                    Ok(row.get::<i64>(0)?)
                })?
                .pop();
            let Some(projection) = projection else {
                return Ok(Err(Error::NotFound(session_id)));
            };
            if after.as_ref().is_some_and(|cursor| cursor.projection_id != projection) {
                return Ok(Err(cursor::stale()));
            }
            let after_ordinal = after.as_ref().map_or(-1, |cursor| cursor.ordinal as i64);
            let mut statement = session.prepare(
                "SELECT ordinal,native_id,parent_id,kind,role,timestamp,text,payload FROM ah_events \
                 WHERE session_id=?1 AND projection_id=?2 AND ordinal>?3 ORDER BY ordinal LIMIT ?4",
            )?;
            let mut rows = statement.query(vault_store::params![session_id, projection, after_ordinal, limit + 1])?;
            let mut items = Vec::new();
            let mut bytes = 0usize;
            let mut has_more = false;
            while let Some(row) = rows.next()? {
                if items.len() == limit as usize {
                    has_more = true;
                    break;
                }
                let payload = row.get::<String>(7)?;
                let text = row.get::<String>(6)?;
                let size = payload.len().saturating_add(text.len());
                if !items.is_empty() && bytes.saturating_add(size) > PAGE_BYTES {
                    has_more = true;
                    break;
                }
                bytes = bytes.saturating_add(size);
                items.push(event(&row, text, payload)?);
            }
            Ok(Ok((projection, items, has_more)))
        })??;
        let next_cursor = if has_more {
            let last = items.last().expect("an event page always admits its first record");
            Some(cursor::encode(&cursor::EventCursor {
                version: cursor::version(),
                session_id: query.session_id.clone(),
                projection_id,
                ordinal: last.ordinal,
            })?)
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }
}

fn event(row: &Row, text: String, payload: String) -> vault_store::Result<NativeEvent> {
    Ok(NativeEvent {
        ordinal: u64::try_from(row.get::<i64>(0)?).map_err(operation)?,
        native_id: row.get(1)?,
        parent_id: row.get(2)?,
        kind: row.get(3)?,
        role: row.get(4)?,
        timestamp: row.get(5)?,
        text,
        payload: serde_json::from_str(&payload).map_err(operation)?,
    })
}
