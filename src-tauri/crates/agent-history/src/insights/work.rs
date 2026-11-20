//! UTC-hour sweep lines for the unfiltered wall and calendar. Counts between successive
//! edges preserve both elapsed and summed time under arbitrary local calendar cuts.
use super::{
    Range,
    calendar::Calendar,
    clock::{self, Axis},
    facts::Fact,
};
use crate::{Archive, Error, Result, query::operation};
use chrono::{Datelike, NaiveDate};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use vault_store::{CancelToken, Db, Priority, Session, params};
const HOUR: i64 = 3_600_000;
const DB: Db = Db::AgentHistoryReadModel;

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Input {
    spans: Vec<(u32, u32)>,
    points: Vec<(u32, bool)>,
}
pub(super) fn split(facts: &[Fact]) -> BTreeMap<i64, Input> {
    let mut hours = BTreeMap::<i64, Input>::new();
    for f in facts {
        let Some(at) = f.at() else { continue };
        let timed = matches!(f, Fact::Work { end: Some(_), .. });
        hours
            .entry(at.div_euclid(HOUR))
            .or_default()
            .points
            .push((at.rem_euclid(HOUR) as u32, timed));
        if let Fact::Work { end: Some(end), .. } = f {
            let mut t = at;
            while t < *end {
                let hour = t.div_euclid(HOUR);
                let base = hour * HOUR;
                let next = (*end).min(base + HOUR);
                hours
                    .entry(hour)
                    .or_default()
                    .spans
                    .push(((t - base) as u32, (next - base) as u32));
                t = next;
            }
        }
    }
    hours
}
pub(crate) fn switching(s: &Session<'_>) -> vault_store::Result<()> {
    s.execute("INSERT OR IGNORE INTO ah_insight_clock_dirty SELECT i.hour FROM ah_reader_build b JOIN ah_chat_summary c ON c.id=b.conversation
        JOIN ah_insight_clock_input i ON i.publication=c.publication WHERE c.root=1 AND (c.live=1 OR c.publication=b.publication)",())?;
    Ok(())
}
pub(crate) fn visibility(s: &Session<'_>, id: &str, trash: Option<i64>) -> vault_store::Result<()> {
    s.execute("INSERT OR IGNORE INTO ah_insight_clock_dirty SELECT i.hour FROM ah_chat_summary c JOIN ah_insight_clock_input i ON i.publication=c.publication
        LEFT JOIN ah_user_state u ON u.conversation_id=c.id WHERE c.id=?1 AND c.live=1 AND c.root=1 AND (u.trashed_at IS NULL)<>(?2 IS NULL)",params![id,trash])?;
    Ok(())
}
#[derive(Default, Serialize, Deserialize)]
struct Hour {
    runs: Vec<(u32, u32, i64)>,
    points: Vec<(u32, bool)>,
}
fn rebuild(s: &Session<'_>, hour: i64, cancel: &CancelToken) -> vault_store::Result<()> {
    let mut edges = BTreeMap::<u32, i64>::new();
    let mut points = BTreeMap::<u32, bool>::new();
    s.for_each("SELECT i.data FROM ah_insight_clock_input i JOIN ah_chat_summary c ON c.publication=i.publication
        LEFT JOIN ah_user_state u ON u.conversation_id=c.id WHERE i.hour=?1 AND c.root=1 AND c.live=1 AND u.trashed_at IS NULL",[hour],|r|{
        cancel.check()?;
        let input:Input=postcard::from_bytes(&r.get::<Vec<u8>>(0)?).map_err(operation)?;
        for (s,e) in input.spans{*edges.entry(s).or_default()+=1;*edges.entry(e).or_default()-=1;}
        for (t,known) in input.points{*points.entry(t).or_default()|=known;}
        Ok(())
    })?;
    let mut out = Hour::default();
    let (mut prev, mut count) = (0, 0);
    for (t, delta) in edges {
        if delta == 0 {
            continue;
        }
        if count > 0 {
            out.runs.push((prev, t, count));
        }
        count += delta;
        prev = t;
    }
    let mut i = 0;
    for (t, timed) in points {
        while out.runs.get(i).is_some_and(|r| r.1 <= t) {
            i += 1;
        }
        if out.runs.get(i).is_none_or(|r| r.0 > t) {
            out.points.push((t, timed));
        }
    }
    if out.runs.is_empty() && out.points.is_empty() {
        s.execute("DELETE FROM ah_insight_clock_hour WHERE hour=?1", [hour])?;
    } else {
        let lo = out
            .runs
            .first()
            .map(|r| r.0)
            .into_iter()
            .chain(out.points.first().map(|p| p.0))
            .min()
            .unwrap();
        let hi = out
            .runs
            .last()
            .map(|r| r.1 - 1)
            .into_iter()
            .chain(out.points.last().map(|p| p.0))
            .max()
            .unwrap();
        let busy: i64 = out.runs.iter().map(|(a, b, _)| i64::from(b - a)).sum();
        let summed: i64 = out.runs.iter().map(|(a, b, n)| i64::from(b - a) * n).sum();
        let timed = busy > 0 || out.points.iter().any(|p| p.1);
        s.execute(
            "INSERT OR REPLACE INTO ah_insight_clock_hour VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                hour,
                hour * HOUR + i64::from(lo),
                hour * HOUR + i64::from(hi),
                busy,
                summed,
                i64::from(timed),
                postcard::to_allocvec(&out).map_err(operation)?
            ],
        )?;
    }
    s.execute("DELETE FROM ah_insight_clock_dirty WHERE hour=?1", [hour])?;
    Ok(())
}
impl Archive {
    pub(crate) fn refresh_insight_hours(&self, cancel: &CancelToken) -> Result<()> {
        loop {
            cancel.check().map_err(|_| Error::Cancelled)?;
            let hours = self.store.read(DB, |s| {
                s.query("SELECT hour FROM ah_insight_clock_dirty ORDER BY hour LIMIT 32", (), |r| {
                    Ok(r.get(0)?)
                })
            })?;
            if hours.is_empty() {
                return Ok(());
            }
            let cancel = cancel.clone();
            self.store.write(DB, Priority::Batch, move |s| {
                for hour in hours {
                    rebuild(s, hour, &cancel)?;
                }
                Ok(())
            })?;
        }
    }
    pub(super) fn wall_clock(
        &self,
        range: Option<Range>,
        year: Option<i32>,
        today: NaiveDate,
        tz: Tz,
        cancel: &CancelToken,
        mut progress: impl FnMut(usize) -> Result<()>,
    ) -> Result<(Option<Value>, Option<Value>)> {
        self.refresh_insight_hours(cancel)?;
        let (first,last)=self.store.read(DB,|s|s.query_one("SELECT (SELECT lo FROM ah_insight_clock_hour ORDER BY hour LIMIT 1),(SELECT hi FROM ah_insight_clock_hour ORDER BY hour DESC LIMIT 1)",(),|r|Ok((r.get::<Option<i64>>(0)?,r.get::<Option<i64>>(1)?))))?;
        let axis = range.map(|r| Axis::for_range(r, today, tz, first, last)).transpose()?;
        let mut cal = year.map(|y| Calendar::new(y, today, tz)).transpose()?;
        let bounds = axis
            .iter()
            .map(Axis::bounds)
            .chain(cal.iter().map(Calendar::bounds))
            .collect::<Vec<_>>();
        let mut from = bounds.iter().map(|s| s.0).min().unwrap().div_euclid(HOUR);
        let until = (bounds.iter().map(|s| s.1).max().unwrap() - 1).div_euclid(HOUR);
        let mut bins =
            vec![(0_i64, 0_i64, false, false); axis.as_ref().map_or(0, |a| a.starts.len())];
        let mut read = 0;
        loop {
            cancel.check().map_err(|_| Error::Cancelled)?;
            let batch=self.store.read(DB,move|s|s.query("SELECT hour,lo,hi,busy,summed,timed FROM ah_insight_clock_hour WHERE hour BETWEEN ?1 AND ?2 ORDER BY hour LIMIT 256",
                params![from,until],|r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?,r.get::<i64>(2)?,r.get::<i64>(3)?,r.get::<i64>(4)?,r.get::<i64>(5)?!=0))))?;
            if batch.is_empty() {
                break;
            }
            for (hour, lo, hi, busy, summed, timed) in batch {
                from = hour + 1;
                let mut split_wall = false;
                if let Some(a) = &axis {
                    if let Some(i) = a.bin(lo)
                        && a.bin(hi) == Some(i)
                    {
                        bins[i].0 += busy;
                        bins[i].1 += summed;
                        bins[i].2 = true;
                        bins[i].3 |= timed;
                    } else {
                        split_wall = hi >= a.bounds().0 && lo < a.bounds().1;
                    }
                }
                let mut split_year = false;
                if let Some(c) = &mut cal {
                    if c.single_day(lo, hi) {
                        c.wall_total(lo, busy, timed);
                    } else {
                        split_year = hi >= c.bounds().0 && lo < c.bounds().1;
                    }
                }
                if split_wall || split_year {
                    let data = self.store.read(DB, move |s| {
                        s.query_one(
                            "SELECT data FROM ah_insight_clock_hour WHERE hour=?1",
                            [hour],
                            |r| Ok(r.get::<Vec<u8>>(0)?),
                        )
                    })?;
                    let row: Hour =
                        postcard::from_bytes(&data).map_err(|_| Error::ReaderRebuildRequired)?;
                    let base = hour * HOUR;
                    for (s, e, n) in row.runs {
                        let (lo, hi) = (base + i64::from(s), base + i64::from(e));
                        if split_wall {
                            axis.as_ref().unwrap().pieces(lo, hi, |i, ms| {
                                bins[i].0 += ms;
                                bins[i].1 += n * ms;
                                bins[i].2 = true;
                                bins[i].3 = true;
                            });
                        }
                        if split_year {
                            cal.as_mut().unwrap().wall_span(lo, hi);
                        }
                    }
                    for (offset, timed) in row.points {
                        let t = base + i64::from(offset);
                        if split_wall && let Some(i) = axis.as_ref().unwrap().bin(t) {
                            bins[i].2 = true;
                            bins[i].3 |= timed;
                        }
                        if split_year {
                            cal.as_mut().unwrap().wall_point(t, timed);
                        }
                    }
                }
                read += 1;
            }
            progress(read)?;
        }
        let work=axis.map(|a|{
            let rows=bins.into_iter().enumerate().map(|(i,(busy,sum,active,known))|json!({"bucketStart":a.starts[i],"bucketEnd":a.starts.get(i+1).copied().unwrap_or(a.end),"busyMs":(!active||known).then_some(busy),"summedMs":(!active||known).then_some(sum)})).collect::<Vec<_>>();
            json!({"unit":a.unit,"buckets":rows})
        });
        let first_year = first.map(|t| clock::day(tz, t)).transpose()?.map(|d| d.year());
        Ok((work, cal.map(|c| c.finish(first_year))))
    }
}
