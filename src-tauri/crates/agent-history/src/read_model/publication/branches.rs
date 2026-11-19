//! Resolves native anchors and Pi parent chains using bounded pages. A run of
//! adjacent parent ordinals needs only its first and last entries to navigate.
use super::*;
use crate::{Archive, query::page_limit};
use vault_store::{OptionalRow, params};

impl Archive {
    pub(super) fn member_version(
        &self,
        h: &ReaderPublication,
        source: &str,
    ) -> Result<(String, u64)> {
        self.check_head(h)?;
        let token = h.publication.0.clone();
        let source = source.to_owned();
        self.store
            .read(DB, move |tx| {
                let member=tx.query_one(
                    "SELECT m.version,v.records FROM ah_reader_member m JOIN ah_reader_version v ON
                     v.token=m.version WHERE m.publication=?1 AND m.source=?2",
                    params![token.clone(), source.clone()],
                    |r| Ok((r.get::<String>(0)?,r.get::<i64>(1)? as u64)),
                ).optional()?;
                if member.is_some() {
                    return Ok(member);
                }
                // A fork can inherit a prefix from a different conversation. Its address
                // authorizes expansion only through the largest included cut.
                tx.query_one(
                    "SELECT version,max(records) FROM ah_reader_line WHERE publication=?1
                    AND source=?2 GROUP BY version",
                    params![token, source],
                    |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)? as u64)),
                )
                .optional()
            })?
            .ok_or_else(|| Error::NotFound("reader member".into()))
    }

    /// Resolves exact native IDs. Duplicate occurrences remain separately pageable.
    pub fn reader_lookup(
        &self,
        h: &ReaderPublication,
        source: &str,
        id: &str,
        after: Option<u64>,
        limit: usize,
    ) -> Result<Vec<u64>> {
        page_limit(limit)?;
        let (owner, end) = self.member_version(h, source)?;
        let _identity = self.version_info(&owner)?;
        let mut rows = self.lookup(&owner, id, after, limit)?;
        rows.retain(|ordinal| *ordinal < end);
        self.check_head(h)?;
        Ok(rows)
    }
    fn lookup(&self, owner: &str, id: &str, after: Option<u64>, limit: usize) -> Result<Vec<u64>> {
        let hash = build::key(id);
        let (token, k) = (owner.to_owned(), hash.clone());
        let start = self
            .store
            .read(DB, move |tx| {
                tx.query_one(
                    "SELECT page FROM ah_reader_lookup WHERE owner=?1 AND (key<?2 OR (key=?2 AND
                 ordinal<=?3)) ORDER BY key DESC, ordinal DESC LIMIT 1",
                    params![token, k, after.unwrap_or(0) as i64],
                    |r| Ok(r.get::<i64>(0)?),
                )
                .optional()
            })?
            .unwrap_or(0) as u64;
        let mut pos = start;
        let mut result = Vec::new();
        loop {
            let token = owner.to_owned();
            let exists = self.store.read(DB, move |tx| tx.query_one("SELECT 1 FROM ah_reader_page WHERE owner=?1 AND family=?2 AND first_rank=?3 AND part=0", params![token, LOOKUP, pos as i64], |_| Ok(())).optional())?.is_some();
            if !exists {
                break;
            }
            let (_, entries) = pages::load::<(String, u64)>(self, owner, LOOKUP, pos)?;
            pos += entries.len() as u64;
            for (key, n) in entries {
                if key > hash {
                    return Ok(result);
                }
                if key == hash
                    && after.is_none_or(|v| n > v)
                    && self.entry(owner, n)?.id.as_deref() == Some(id)
                {
                    result.push(n);
                    if result.len() == limit {
                        return Ok(result);
                    }
                }
            }
        }
        Ok(result)
    }
    fn entry(&self, owner: &str, n: u64) -> Result<Entry> {
        let (first, mut page) = pages::load::<Entry>(self, owner, OUTLINE, n)?;
        Ok(page.swap_remove((n - first) as usize))
    }
    /// Exact native expansion always uses evidence, including a member with rendered pages.
    pub fn reader_record_bytes(
        &self,
        h: &ReaderPublication,
        source: &str,
        ordinal: u64,
        offset: u64,
        limit: usize,
        job: &JobControl,
    ) -> Result<BytePage> {
        let _guard = self.jobs.enter_reader(&job.cancel)?;
        let (owner, count) = self.member_version(h, source)?;
        if ordinal >= count {
            return Err(Error::NotFound("record ordinal".into()));
        }
        let (info, _) = self.version_info(&owner)?;
        let entry =
            self.source_outline(&owner, &info, ordinal, 1, job)?.pop().ok_or(Error::StaleCursor)?;
        let page = self.record_bytes(&info, &entry.locator, offset, limit, job)?;
        self.check_head(h)?;
        Ok(page)
    }
    /// Restores a recorded leaf. Parent identity must resolve uniquely to an earlier physical record.
    pub fn reader_leaf_window(
        &self,
        h: &ReaderPublication,
        source: &str,
        leaf: u64,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<DisplayRow>> {
        read::window_limit(first, count)?;
        let _guard = self.jobs.enter_reader(&job.cancel)?;
        let (owner, total) = self.member_version(h, source)?;
        if leaf >= total {
            return Err(Error::NotFound("leaf ordinal".into()));
        }
        let mut depth = 0;
        self.walk_leaf(&owner, leaf, job, |lo, hi| {
            depth += hi - lo;
            Ok(true)
        })?;
        let mut remaining = depth;
        let mut runs = Vec::new();
        self.walk_leaf(&owner, leaf, job, |lo, hi| {
            let start = remaining - (hi - lo);
            let begin = first.max(start);
            let end = (first + count as u64).min(remaining);
            if begin < end {
                runs.push((lo + begin - start, (end - begin) as usize));
            }
            remaining = start;
            Ok(remaining > first)
        })?;
        let mut rows = Vec::new();
        for (start, len) in runs.into_iter().rev() {
            rows.extend(self.display_source(&owner, start, len, job)?);
        }
        self.check_head(h)?;
        Ok(rows)
    }
    pub(super) fn walk_leaf(
        &self,
        owner: &str,
        mut last: u64,
        job: &JobControl,
        mut visit: impl FnMut(u64, u64) -> Result<bool>,
    ) -> Result<()> {
        loop {
            job.cancel.check().map_err(|_| Error::Cancelled)?;
            let entry = self.entry(owner, last)?;
            let first = entry.run_start;
            if first > last {
                return Err(pages::invalid("backwards branch run"));
            }
            if !visit(first, last + 1)? || first == 0 {
                break;
            }
            let head = if first == last { entry } else { self.entry(owner, first)? };
            if let Some(parent) = head.parent {
                let ordinals = self.lookup(owner, &parent, None, 2)?;
                if ordinals.len() != 1 || ordinals[0] >= first {
                    return Err(Error::InvalidInput("unresolved branch parent".into()));
                }
                last = ordinals[0];
            } else {
                last = 0
            }
        }
        Ok(())
    }
}
