//! Responsibilities: discover retained evidence without consulting live sources.
//! Not responsible for: filesystem discovery, native parsing or mirroring deletion.

use super::{Page, SourceQuery, cursor, page_limit};
use crate::{
    Result,
    archive::{Archive, SourceInfo},
    model::ArtifactKind,
};
use vault_store::{Db, Row, Value};

const PAGE_BYTES: usize = 4 * 1024 * 1024;

impl Archive {
    /// Walks retained source identities, including unparseable transcripts and
    /// raw-only attachments. A continuation excludes sources registered after its
    /// first page; head revisions and promoted roles reflect current archive facts.
    /// An unavailable source can have no complete revision yet and is still listed.
    pub fn list_sources(&self, query: &SourceQuery) -> Result<Page<SourceInfo>> {
        let _job = self.jobs.enter(None)?;
        let limit = page_limit(query.limit)?;
        let scope = cursor::scope(&(&query.harness, &query.kind))?;
        let after =
            query.after.as_deref().map(cursor::decode::<cursor::SourceCursor>).transpose()?;
        if let Some(after) = &after {
            cursor::valid_version(after.version)?;
            if after.scope != scope || after.snapshot_rowid < 0 || after.after_id.len() > 256 {
                return Err(cursor::invalid());
            }
        }
        let harness = query.harness.map(|value| value.to_string());
        let kind = query.kind.map(kind_name);
        let (snapshot_rowid, items, has_more) =
            self.store.read(Db::AgentHistoryEvidence, move |session| {
                let snapshot = match &after {
                    Some(cursor) => cursor.snapshot_rowid,
                    None => session.query_one(
                        "SELECT coalesce(max(rowid), 0) FROM ah_sources",
                        (),
                        |row| Ok(row.get::<i64>(0)?),
                    )?,
                };
                let mut sql = "SELECT id, harness, root, relative_path, kind, latest_revision_id,
                        observed_at, retention, last_seen_at, missing_since  FROM ah_sources
                        WHERE rowid<=? AND id>?"
                    .to_owned();
                let mut values = vec![
                    Value::Integer(snapshot),
                    Value::Text(after.map_or_else(String::new, |cursor| cursor.after_id)),
                ];
                if let Some(harness) = harness {
                    sql.push_str(" AND harness=?");
                    values.push(Value::Text(harness));
                }
                if let Some(kind) = kind {
                    sql.push_str(" AND kind=?");
                    values.push(Value::Text(kind.to_owned()));
                }
                sql.push_str(" ORDER BY id LIMIT ?");
                values.push(Value::Integer(limit + 1));
                let mut statement = session.prepare(&sql)?;
                let mut rows = statement.query(values)?;
                let mut items = Vec::new();
                let mut bytes = 0usize;
                let mut has_more = false;
                while let Some(row) = rows.next()? {
                    if items.len() == limit as usize {
                        has_more = true;
                        break;
                    }
                    let item = source(&row)?;
                    let size =
                        [&item.id, &item.harness, &item.root, &item.relative_path, &item.kind]
                            .into_iter()
                            .fold(0usize, |total, value| total.saturating_add(value.len()));
                    if !items.is_empty() && bytes.saturating_add(size) > PAGE_BYTES {
                        has_more = true;
                        break;
                    }
                    bytes = bytes.saturating_add(size);
                    items.push(item);
                }
                Ok((snapshot, items, has_more))
            })?;
        let next_cursor = if has_more {
            Some(cursor::encode(&cursor::SourceCursor {
                version: cursor::version(),
                scope,
                snapshot_rowid,
                after_id: items
                    .last()
                    .expect("a source page always admits its first record")
                    .id
                    .clone(),
            })?)
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }
}

fn source(row: &Row) -> vault_store::Result<SourceInfo> {
    Ok(SourceInfo {
        id: row.get(0)?,
        harness: row.get(1)?,
        root: row.get(2)?,
        relative_path: row.get(3)?,
        kind: row.get(4)?,
        latest_revision_id: row.get(5)?,
        observed_at: row.get(6)?,
        retention: row.get(7)?,
        last_seen_at: row.get(8)?,
        missing_since: row.get(9)?,
    })
}

fn kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Transcript => "transcript",
        ArtifactKind::Attachment => "attachment",
        ArtifactKind::Metadata => "metadata",
        ArtifactKind::Sqlite => "sqlite",
    }
}
