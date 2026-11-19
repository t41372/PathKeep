//! Pi leaf selection follows native parent edges. The latest recorded leaf is
//! distinct from a live UI choice, which the evidence cannot establish.
use super::*;
use crate::Archive;
use vault_store::{OptionalRow, Priority, params};

/// A page of recorded Pi alternatives, with the most recent terminal entry as a reader default.
#[derive(Clone, Debug, Serialize)]
pub struct ReaderLeaves {
    pub ordinals: Vec<u64>,
    pub total: u64,
    pub latest_recorded: Option<u64>,
}

impl Archive {
    /// Enumerates leaf addresses without walking every parent chain or scanning raw events.
    pub fn reader_leaves(
        &self,
        h: &ReaderPublication,
        source: &str,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<ReaderLeaves> {
        read::window_limit(first, count)?;
        let _worker = self.jobs.enter_window(&job.cancel)?;
        self.at_head(h, || {
            let (owner, _) = self.member_version(h, source)?;
            self.version_info(&owner)?;
            let key = owner.clone();
            let selection = self.store.read(DB, move |tx| selection(tx, &key))?;
            let (total, latest_recorded) = selection.map_or((0, None), |s| (s.total, s.latest));
            let n = count.min(total.saturating_sub(first) as usize);
            let ordinals =
                read::range(self, &owner, LEAVES, first, n, job).map_err(navigation_error)?;
            Ok(ReaderLeaves { ordinals, total, latest_recorded })
        })
    }

    pub(super) fn stage_selected_leaf(
        &self,
        owner: &str,
        total: u64,
        latest: Option<u64>,
        job: &JobControl,
    ) -> Result<()> {
        let mut reverse_rank = 0i64;
        let mut oldest = None;
        let mut unresolved = None;
        if let Some(leaf) = latest {
            let walked = self.walk_leaf(owner, leaf, job, |lo, hi| {
                oldest = Some(lo as i64);
                let token = owner.to_owned();
                let n = self.store.write(DB, Priority::Batch, move |tx| {
                    build::require_staging(tx, &token)?;
                    let start = visible::prefix(tx, &token, lo as i64)?;
                    let count = visible::prefix(tx, &token, hi as i64)? - start;
                    if count > 0 {
                        runs::save(tx, &token, runs::SELECTED, reverse_rank, start, count)?;
                    }
                    Ok(count)
                })?;
                reverse_rank += n;
                Ok(true)
            });
            match walked {
                Ok(()) => {}
                Err(Error::InvalidInput(_)) => unresolved = oldest,
                Err(error) => return Err(error),
            }
        }
        let token = owner.to_owned();
        self.store.write(DB, Priority::Batch, move |tx| {
            build::require_staging(tx, &token)?;
            let visible = reverse_rank + i64::from(unresolved.is_some());
            let stamp = selection_stamp(
                &token,
                total,
                latest,
                visible as u64,
                unresolved.map(|n| n as u64),
            );
            tx.execute(
                "INSERT INTO ah_reader_leaf VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    token,
                    total as i64,
                    latest.map(|n| n as i64),
                    reverse_rank + i64::from(unresolved.is_some()),
                    unresolved,
                    stamp
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub(super) fn selected_visible_ranges(
        &self,
        owner: &str,
        lo: u64,
        hi: u64,
    ) -> Result<(Option<u64>, Vec<(u64, u64)>)> {
        let token = owner.to_owned();
        Ok(self.store.read(DB, move |tx| {
            let Some(selected) = selection(tx, &token)? else { return Ok((None, vec![(lo, hi)])) };
            let shift = u64::from(selected.boundary.is_some());
            let total = selected.visible - shift;
            let marker = selected.boundary.filter(|_| lo == 0 && hi > 0);
            let (first, end) = (lo.saturating_sub(shift), hi.saturating_sub(shift));
            let runs =
                runs::page(tx, &token, runs::SELECTED, total as i64, first as i64, end as i64)?;
            Ok((marker, runs.into_iter().map(|(first, count)| (first, first + count)).collect()))
        })?)
    }
}

impl Archive {
    /// A completed depth publishes reverse-ranked runs. Concurrent or interrupted first
    /// walks produce the same private rows for this immutable version.
    pub(super) fn leaf_page(
        &self,
        owner: &str,
        leaf: u64,
        first: u64,
        count: usize,
        job: &JobControl,
    ) -> Result<Vec<(u64, u64)>> {
        let token = owner.to_owned();
        let leaf = leaf as i64;
        let depth = self.store.read(DB, move |tx| {
            tx.query_one(
                "SELECT depth,stamp FROM ah_reader_walk WHERE owner=?1 AND leaf=?2",
                params![token, leaf],
                |r| Ok((r.get::<i64>(0)?, r.get::<Vec<u8>>(1)?)),
            )
            .optional()
        })?;
        let depth = match depth {
            Some((n, stamp)) if n > 0 && stamp == runs::depth_stamp(owner, leaf, n) => n,
            Some(_) => return Err(Error::ReaderRebuildRequired),
            None => {
                let mut depth = 0i64;
                self.walk_leaf(owner, leaf as u64, job, |lo, hi| {
                    let n = (hi - lo) as i64;
                    let token = owner.to_owned();
                    self.store.write(DB, Priority::Interactive, move |tx| {
                        runs::save(tx, &token, leaf, depth, lo as i64, n)
                    })?;
                    depth += n;
                    Ok(true)
                })?;
                let token = owner.to_owned();
                let stamp = runs::depth_stamp(owner, leaf, depth);
                self.store.write(DB, Priority::Interactive, move |tx| {
                    tx.execute(
                        "INSERT OR IGNORE INTO ah_reader_walk VALUES(?1,?2,?3,?4)",
                        params![token, leaf, depth, stamp],
                    )
                })?;
                depth
            }
        };
        let first = first as i64;
        if first >= depth {
            return Ok(vec![]);
        }
        let end = (first + count as i64).min(depth);
        let token = owner.to_owned();
        Ok(self.store.read(DB, move |tx| runs::page(tx, &token, leaf, depth, first, end))?)
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct Selection {
    pub total: u64,
    pub latest: Option<u64>,
    pub visible: u64,
    pub boundary: Option<u64>,
}
const SELECTION_BINDING: i64 = 7;
fn selection_stamp(
    owner: &str,
    total: u64,
    latest: Option<u64>,
    visible: u64,
    boundary: Option<u64>,
) -> Vec<u8> {
    pages::binding(
        owner,
        SELECTION_BINDING,
        total as i64,
        latest.map_or(-1, |n| n as i64),
        visible as i64,
        &boundary.map_or(-1, |n| n as i64).to_le_bytes(),
    )
}
pub(super) fn selection(
    tx: &vault_store::Session<'_>,
    owner: &str,
) -> vault_store::Result<Option<Selection>> {
    tx.query_one("SELECT v.leaf_count,l.total,l.latest,l.selected_visible,l.unresolved_at,l.stamp FROM ah_reader_version v LEFT JOIN ah_reader_leaf l ON l.owner=v.token WHERE v.token=?1",[owner],|r|{
        let expected:i64=r.get(0)?;
        if expected == -1{return Ok(None)}
        let total:Option<i64>=r.get(1)?;let latest:Option<i64>=r.get(2)?;
        let visible:Option<i64>=r.get(3)?;let boundary:Option<i64>=r.get(4)?;
        let invalid=||crate::query::operation(Error::ReaderRebuildRequired);
        if expected<0 || total!=Some(expected) || latest.is_some_and(|n|n<0) || boundary.is_some_and(|n|n<0){return Err(invalid())}
        let visible=visible.filter(|n|*n>=i64::from(boundary.is_some())).ok_or_else(invalid)? as u64;
        let total=expected as u64;let latest=latest.map(|n|n as u64);let boundary=boundary.map(|n|n as u64);
        if r.get::<Option<Vec<u8>>>(5)?!=Some(selection_stamp(owner,total,latest,visible,boundary)){return Err(invalid())}
        Ok(Some(Selection{total,latest,visible,boundary}))
    })
}
