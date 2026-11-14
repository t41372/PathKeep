//! Responsibilities: page one native line without depending on unrelated publications.
//! Not responsible for: changing evidence, source discovery or holding publication locks.
use super::{EventQuery, EventView, Page, cursor, line, operation, page_limit};
use crate::{Archive, Error, NativeEvent, Result};
use vault_store::{Db, Row, Session, params};

const PAGE_BYTES: usize = 4 * 1024 * 1024;

impl Archive {
    /// Line resolution and rows share one read snapshot. First pages need no
    /// retry or global staleness check; cursors bind only their own publications.
    pub fn events(&self, query: &EventQuery) -> Result<Page<EventView>> {
        let limit = page_limit(query.limit)? as usize;
        let after =
            query.after.as_deref().map(cursor::decode::<cursor::EventCursor>).transpose()?;
        if let Some(after) = &after {
            cursor::valid_version(after.version)?;
            if after.session_id != query.session_id || after.ordinal > i64::MAX as u64 {
                return Err(cursor::invalid());
            }
        }
        let id = query.session_id.clone();
        self.store.read(Db::Archive, move |tx| Ok(page(tx, &id, limit, after)))?
    }
}

fn page(
    tx: &Session<'_>,
    id: &str,
    limit: usize,
    after: Option<cursor::EventCursor>,
) -> Result<Page<EventView>> {
    let Some(segments) = line::snapshot(tx, id)? else {
        return Err(if after.is_some() { Error::StaleCursor } else { Error::NotFound(id.into()) });
    };
    let stamp = cursor::scope(&segments)?;
    if let Some(after) = &after {
        if after.line != stamp {
            return Err(cursor::stale());
        }
        if after.segment >= segments.len() {
            return Err(cursor::invalid());
        }
    }
    let mut items = Vec::new();
    let mut bytes = 0;
    let mut more = false;
    let mut last_segment = 0;
    for (index, segment) in
        segments.iter().enumerate().skip(after.as_ref().map_or(0, |cursor| cursor.segment))
    {
        let ordinal = after
            .as_ref()
            .filter(|cursor| cursor.segment == index)
            .map_or(-1, |cursor| cursor.ordinal as i64);
        let mut statement = tx.prepare(
            "SELECT ordinal, native_id, parent_id, kind, role, timestamp, text, payload FROM
                ah_events WHERE session_id=?1 AND projection_id=?2 AND ordinal>?3 AND ordinal<?4
                ORDER BY ordinal LIMIT ?5",
        )?;
        let mut rows = statement.query(params![
            segment.source.clone(),
            segment.projection,
            ordinal,
            segment.end.map_or(i64::MAX, |value| value as i64),
            (limit + 1 - items.len()) as i64,
        ])?;
        while let Some(row) = rows.next()? {
            let text = row.get::<String>(6).map_err(vault_store::Error::from)?;
            let payload = row.get::<String>(7).map_err(vault_store::Error::from)?;
            let cost = text.len().saturating_add(payload.len());
            if items.len() == limit || (!items.is_empty() && bytes + cost > PAGE_BYTES) {
                more = true;
                break;
            }
            bytes += cost;
            items.push(EventView {
                source_id: segment.source.clone(),
                event: event(&row, text, payload)?,
            });
            last_segment = index;
        }
        if more {
            break;
        }
    }
    let next_cursor = if more {
        Some(cursor::encode(&cursor::EventCursor {
            version: cursor::version(),
            session_id: id.into(),
            line: stamp,
            ordinal: items.last().unwrap().ordinal,
            segment: last_segment,
        })?)
    } else {
        None
    };
    Ok(Page { items, next_cursor })
}

fn event(row: &Row, text: String, payload: String) -> vault_store::Result<NativeEvent> {
    Ok(NativeEvent {
        references: Vec::new(),
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
