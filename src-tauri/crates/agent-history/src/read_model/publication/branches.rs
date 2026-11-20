//! Resolves native anchors and Pi parent chains using bounded pages. A run of
//! adjacent parent ordinals needs only its first and last entries to navigate.
use super::*;
use crate::{Archive, query::page_limit};
use vault_store::{OptionalRow, params};

impl Archive {
    pub(crate) fn member_version(
        &self,
        h: &ReaderPublication,
        source: &str,
    ) -> Result<(String, u64)> {
        let token = h.publication.0.clone();
        let source = source.to_owned();
        self.store
            .read(DB, move |tx| {
                if let Some(member)=manifest::member(tx,&token,&source)?{
                    if member.1.is_empty(){return Err(crate::query::operation(Error::ReaderRebuildRequired))}
                    manifest::verify_version(tx,&member.1)?;
                    let records=tx.query_one("SELECT records FROM ah_reader_version WHERE token=?1",[member.1.as_str()],|r|Ok(r.get::<i64>(0)?))?;
                    return Ok(Some((member.1,records as u64)))
                }
                // A fork can inherit a prefix from a different conversation. Its address
                // authorizes expansion only through the largest included cut.
                tx.query_one("SELECT branch,first_rank,version,source,records,first_visible,visible,stamp FROM ah_reader_line
                    WHERE publication=?1 AND source=?2 ORDER BY records DESC LIMIT 1",params![token.clone(),source],|r|{
                    let line=manifest::decode_line(r,&token)?;Ok((line.2,line.4 as u64))
                }).optional()
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
        job: &JobControl,
    ) -> Result<Vec<u64>> {
        page_limit(limit)?;
        let _worker = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            let (owner, end) = self.member_version(h, source)?;
            let _identity = self.version_info(&owner)?;
            let mut rows = self.lookup(&owner, id, after, limit, job)?;
            rows.retain(|ordinal| *ordinal < end);
            Ok(rows)
        })
    }
    fn lookup(
        &self,
        owner: &str,
        id: &str,
        after: Option<u64>,
        limit: usize,
        job: &JobControl,
    ) -> Result<Vec<u64>> {
        let hash = build::key(id);
        let nav = Extents::load(self, owner, job)?;
        let (extent, postings) = (nav.lookup, nav.postings);
        if nav.directories != extent.div_ceil(256) || (extent == 0) != (postings == 0) {
            return Err(Error::ReaderRebuildRequired);
        }
        let directory = |rank: u64| -> Result<(String, u64, u64)> {
            let (first, mut rows) =
                pages::load::<(String, u64, u64)>(self, owner, DIRECTORY, rank, job)
                    .map_err(navigation_error)?;
            Ok(rows.swap_remove((rank - first) as usize))
        };
        if extent > 0 {
            let (last_key, last_ordinal, tail) = directory(extent - 1)?;
            let (first, rows) = pages::load::<(String, u64)>(self, owner, LOOKUP, tail, job)
                .map_err(navigation_error)?;
            if first != tail
                || rows.first() != Some(&(last_key, last_ordinal))
                || first.checked_add(rows.len() as u64) != Some(postings)
            {
                return Err(Error::ReaderRebuildRequired);
            }
        }
        let (mut lo, mut hi) = (0, extent);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let (k, n, _) = directory(mid)?;
            if (k.as_str(), n) <= (hash.as_str(), after.unwrap_or(0)) {
                lo = mid + 1
            } else {
                hi = mid
            }
        }
        let mut pos = if extent == 0 { 0 } else { directory(lo.saturating_sub(1))?.2 };
        let mut result = Vec::new();
        while pos < postings {
            let (first, entries) = pages::load::<(String, u64)>(self, owner, LOOKUP, pos, job)
                .map_err(navigation_error)?;
            if first != pos || entries.len() as u64 > postings - pos {
                return Err(Error::ReaderRebuildRequired);
            }
            pos += entries.len() as u64;
            for (key, n) in entries {
                if key > hash {
                    return Ok(result);
                }
                if key == hash
                    && after.is_none_or(|v| n > v)
                    && self.entry(owner, n, job)?.id.as_deref() == Some(id)
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
    fn entry(&self, owner: &str, n: u64, job: &JobControl) -> Result<Entry> {
        let (first, mut page) =
            pages::load::<Entry>(self, owner, OUTLINE, n, job).map_err(navigation_error)?;
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
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            let (owner, count) = self.member_version(h, source)?;
            if ordinal >= count {
                return Err(Error::NotFound("record ordinal".into()));
            }
            let (info, _) = self.version_info(&owner)?;
            let entry = self
                .source_outline(&owner, &info, ordinal, 1, job)?
                .pop()
                .ok_or(Error::StaleCursor)?;
            let page = self.record_bytes_admitted(&info, &entry.locator, offset, limit, job)?;
            Ok(page)
        })
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
        let _guard = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            let (owner, total) = self.member_version(h, source)?;
            if leaf >= total {
                return Err(Error::NotFound("leaf ordinal".into()));
            }
            let runs = self.leaf_page(&owner, leaf, first, count, job)?;
            let mut window = Window::default();
            for (start, len) in runs {
                window.physical(&owner, start..start + len);
            }
            let rows = window.read(self, job)?;
            Ok(rows)
        })
    }
    pub(super) fn walk_leaf(
        &self,
        owner: &str,
        mut last: u64,
        job: &JobControl,
        mut visit: impl FnMut(u64, u64) -> Result<bool>,
    ) -> Result<()> {
        let token = owner.to_owned();
        let header = self.store.read(DB, move |tx| {
            tx.query_one(
                "SELECT header_ordinal FROM ah_reader_version WHERE token=?1",
                [token],
                |r| Ok(r.get::<Option<i64>>(0)?.map(|n| n as u64)),
            )
        })?;
        let extent = Extents::load(self, owner, job)?;
        loop {
            job.cancel.check().map_err(|_| Error::Cancelled)?;
            let entry = self.entry(owner, last, job)?;
            let (mut lo, mut hi) = (0, extent.ambiguous);
            let ambiguous = |rank| -> Result<u64> {
                let (first, rows) = pages::load::<u64>(self, owner, AMBIGUOUS, rank, job)
                    .map_err(navigation_error)?;
                Ok(rows[(rank - first) as usize])
            };
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if ambiguous(mid)? <= last { lo = mid + 1 } else { hi = mid }
            }
            let boundary = if lo > 0 {
                Some(ambiguous(lo - 1)?).filter(|n| *n >= entry.run_start)
            } else {
                None
            };
            let first = boundary.unwrap_or(entry.run_start);
            if first > last {
                return Err(Error::ReaderRebuildRequired);
            }
            if !visit(first, last + 1)? || Some(first) == header {
                break;
            }
            if boundary.is_some() {
                return Err(Error::InvalidInput("unresolved branch parent".into()));
            }
            let head = if first == last { entry } else { self.entry(owner, first, job)? };
            if let Some(parent) = head.parent {
                let ordinals = self.lookup(owner, &parent, None, 2, job)?;
                let mut earlier = ordinals.into_iter().take_while(|n| *n < first);
                let parent = earlier.next();
                if parent.is_none() || earlier.next().is_some() {
                    return Err(Error::InvalidInput("unresolved branch parent".into()));
                }
                last = parent.expect("one eligible parent");
            } else {
                if !head.declared_root {
                    return Err(Error::InvalidInput("unresolved branch parent".into()));
                }
                let Some(root) = header.filter(|root| *root < first) else { break };
                last = root;
            }
        }
        Ok(())
    }
}
