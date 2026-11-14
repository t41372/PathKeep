//! Responsibilities: list immutable, complete raw captures by source and sequence.
//! Not responsible for: mirroring source deletion or exposing unfinished attempts.

use super::{Page, cursor, operation, page_limit};
use crate::{
    Result,
    archive::{Archive, RevisionInfo},
};
use vault_store::Db;

impl Archive {
    /// Older complete captures remain available after rewrite, deletion or a
    /// parser failure. Newly completed captures do not disturb an existing walk.
    pub fn revisions(
        &self,
        source_id: &str,
        limit: usize,
        after: Option<String>,
    ) -> Result<Page<RevisionInfo>> {
        let count = page_limit(limit)?;
        let after = after.as_deref().map(cursor::decode::<cursor::RevisionCursor>).transpose()?;
        if let Some(after) = &after {
            cursor::valid_version(after.version)?;
            if after.source_id != source_id || after.before_id <= 0 {
                return Err(cursor::invalid());
            }
        }
        let before = after.map_or(i64::MAX, |cursor| cursor.before_id);
        let source = source_id.to_owned();
        let mut items = self.store.read(Db::SourceEvidence, move |session| {
            session.query(
                "SELECT id, source_id, digest, byte_len, captured_at, capture_format,
                    bytes_retained FROM ah_revisions  WHERE source_id=?1 AND state='complete' AND
                    id<?2 ORDER BY id DESC LIMIT ?3",
                vault_store::params![source, before, count + 1],
                |row| {
                    Ok(RevisionInfo {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        digest: row.get(2)?,
                        byte_len: u64::try_from(row.get::<i64>(3)?).map_err(operation)?,
                        captured_at: row.get(4)?,
                        capture_format: row.get(5)?,
                        bytes_retained: row.get::<i64>(6)? != 0,
                    })
                },
            )
        })?;
        let has_more = items.len() > limit;
        items.truncate(limit);
        let next_cursor = if has_more {
            Some(cursor::encode(&cursor::RevisionCursor {
                version: cursor::version(),
                source_id: source_id.to_owned(),
                before_id: items.last().expect("a bounded revision page is nonempty").id,
            })?)
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }
}
