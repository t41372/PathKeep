//! Assembles one requested window. Physical ranks from different runs share a source
//! decoder and page lookup; presentation order is restored after the bounded reads.
use super::*;
use crate::{Archive, ParseContext, archive::RevisionInfo};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use vault_store::params;

#[derive(Default)]
pub(super) struct Window {
    rows: Vec<Option<DisplayRow>>,
    by_version: BTreeMap<String, Vec<(u64, usize)>>,
}
impl Window {
    pub fn physical(&mut self, owner: &str, ordinals: impl IntoIterator<Item = u64>) {
        let selected = self.by_version.entry(owner.to_owned()).or_default();
        for n in ordinals {
            selected.push((n, self.rows.len()));
            self.rows.push(None);
        }
    }
    pub fn marker(&mut self, row: DisplayRow) {
        self.rows.push(Some(row));
    }
    pub fn read(mut self, a: &Archive, job: &JobControl) -> Result<Vec<DisplayRow>> {
        let mut budget = job.clone();
        for (owner, mut positions) in self.by_version {
            positions.sort_unstable();
            let mut ordinals: Vec<_> = positions.iter().map(|p| p.0).collect();
            ordinals.dedup();
            let (info, selective) = a.version_info(&owner)?;
            let rows = if selective {
                match selected::<DisplayRow>(a, &owner, RENDERED, &ordinals, &budget) {
                    Ok(rows)
                        if rows
                            .iter()
                            .zip(&ordinals)
                            .all(|(r, n)| r.ordinal.0 == *n && r.source_id == info.source_id) =>
                    {
                        Some(rows)
                    }
                    Ok(_) | Err(Error::Integrity(_)) => None,
                    Err(e) => return Err(e),
                }
            } else {
                None
            };
            let rows = match rows {
                Some(rows) => rows,
                None => {
                    let entries = a
                        .selected_outline(&owner, &info, &ordinals, &mut budget)?
                        .into_iter()
                        .map(|e| (e.locator, e.context))
                        .collect::<Vec<_>>();
                    let mut rows = Vec::with_capacity(entries.len());
                    let work = a.read_window_admitted(
                        &info,
                        &entries,
                        pages::TEXT_BYTES,
                        &budget,
                        &mut |r| {
                            rows.push(r.preview.unwrap_or_else(|| {
                                let mut display = DisplayRow::from_event(
                                    &r.source_id.0,
                                    &r.record,
                                    r.interpretation,
                                    pages::TEXT_BYTES,
                                );
                                display.truncated |= r.raw.next.is_some();
                                display
                            }));
                            Ok(())
                        },
                    )?;
                    budget.max_decoded_bytes -= work;
                    rows
                }
            };
            let mut positions = positions.into_iter().peekable();
            for row in rows {
                let mut next = positions.next_if(|(n, _)| *n == row.ordinal.0);
                while let Some((_, index)) = next {
                    next = positions.next_if(|(n, _)| *n == row.ordinal.0);
                    if next.is_none() {
                        self.rows[index] = Some(row);
                        break;
                    }
                    self.rows[index] = Some(row.clone());
                }
            }
        }
        self.rows.into_iter().map(|row| row.ok_or(Error::ReaderRebuildRequired)).collect()
    }
}

/// Requested ranks are sorted. Only the last authenticated page needs to stay resident.
fn selected<T: serde::de::DeserializeOwned + Clone>(
    a: &Archive,
    owner: &str,
    family: i64,
    ranks: &[u64],
    job: &JobControl,
) -> Result<Vec<T>> {
    let mut page: Vec<T> = Vec::new();
    let mut first = 0;
    let mut rows = Vec::with_capacity(ranks.len());
    for &rank in ranks {
        if rank >= first + page.len() as u64 {
            (first, page) = pages::load(a, owner, family, rank, job)?;
        }
        rows.push(page[(rank - first) as usize].clone());
    }
    Ok(rows)
}
impl Archive {
    pub(super) fn selected_outline(
        &self,
        owner: &str,
        info: &RevisionInfo,
        ordinals: &[u64],
        job: &mut JobControl,
    ) -> Result<Vec<Entry>> {
        match selected::<Entry>(self, owner, OUTLINE, ordinals, job) {
            Ok(mut rows) if rows.iter().zip(ordinals).all(|(e, n)| e.locator.ordinal.0 == *n) => {
                let compressed = rows.first().is_some_and(|e| e.locator.frame.is_some());
                let owner = Arc::new(LocatorOwner::new(info, compressed));
                for entry in &mut rows {
                    entry.locator.owner = Some(owner.clone());
                }
                return Ok(rows);
            }
            Ok(_) | Err(Error::Integrity(_)) => {}
            Err(e) => return Err(e),
        }
        let token = owner.to_owned();
        let (limit, decoded) = self.store.read(DB, move |tx| {
            tx.query_one(
                "SELECT record_limit,decoded_limit FROM ah_reader_version WHERE token=?1",
                params![token],
                |r| Ok((r.get::<i64>(0)? as usize, r.get::<i64>(1)? as u64)),
            )
        })?;
        let mut scan = job.clone();
        scan.max_decoded_bytes = scan.max_decoded_bytes.min(decoded);
        let work = Arc::new(AtomicU64::new(0));
        let meter = work.clone();
        let observer = job.progress.clone();
        scan.progress = Some(Arc::new(move |p| {
            if p.unit == ProgressUnit::DecodedBytes {
                meter.fetch_max(p.done, Ordering::Relaxed);
            }
            if let Some(report) = &observer {
                report(p)
            }
        }));
        let mut rows = Vec::new();
        let mut previous = None;
        let mut start = 0;
        let result = self.scan_revision_admitted(
            info.id,
            &ParseContext { max_record_bytes: limit, ..Default::default() },
            &scan,
            &mut |loc, ctx, v| {
                let entry = Entry::interpret(loc, ctx, v, &mut previous, &mut start)?;
                if ordinals.get(rows.len()) == Some(&entry.locator.ordinal.0) {
                    rows.push(entry);
                }
                if rows.len() == ordinals.len() {
                    return Err(Error::Cancelled);
                }
                Ok(())
            },
        );
        match result {
            Ok(_) => {}
            Err(Error::Cancelled) if rows.len() == ordinals.len() => {}
            Err(e) => return Err(e),
        }
        job.cancel.check().map_err(|_| Error::Cancelled)?;
        job.max_decoded_bytes -= work.load(Ordering::Relaxed);
        if rows.len() != ordinals.len() {
            return Err(Error::ReaderRebuildRequired);
        }
        Ok(rows)
    }
}
