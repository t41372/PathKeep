//! UTC intervals carry elapsed milliseconds. Calendar boundaries are built in the published
//! IANA zone so repeated hours remain distinct and a skipped midnight creates no extra work.
use super::Range;
use crate::{Error, Result};
use chrono::{Datelike, Days, Duration, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

pub(super) type Span = (i64, i64);
pub(super) fn union(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_unstable();
    let mut out = Vec::<Span>::new();
    for (s, e) in spans {
        if let Some(last) = out.last_mut()
            && s <= last.1
        {
            last.1 = last.1.max(e)
        } else {
            out.push((s, e));
        }
    }
    out
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub(super) struct Cluster {
    pub first: i64,
    pub last: i64,
    pub end: i64,
}
impl Cluster {
    pub fn new(at: i64, end: Option<i64>) -> Self {
        Self { first: at, last: at, end: end.unwrap_or(at).clamp(at, at.saturating_add(900000)) }
    }
}
pub(super) fn merge_clusters(mut input: Vec<Cluster>) -> Vec<Cluster> {
    input.sort_unstable_by_key(|p| p.first);
    let mut out = Vec::<Cluster>::new();
    for p in input {
        if let Some(t) = out.last_mut()
            && p.first <= t.last.saturating_add(900000)
        {
            if p.last > t.last {
                t.last = p.last;
                t.end = p.end
            } else if p.last == t.last {
                t.end = t.end.max(p.end)
            }
        } else {
            out.push(p)
        }
    }
    out
}
fn invalid() -> Error {
    Error::InvalidInput("date outside supported calendar".into())
}
pub(super) fn day(tz: Tz, at: i64) -> Result<NaiveDate> {
    Ok(Utc.timestamp_millis_opt(at).single().ok_or_else(invalid)?.with_timezone(&tz).date_naive())
}
pub(super) fn hour(tz: Tz, at: i64) -> Result<(NaiveDate, u32, i64)> {
    let local = Utc.timestamp_millis_opt(at).single().ok_or_else(invalid)?.with_timezone(&tz);
    Ok((
        local.date_naive(),
        local.hour(),
        at - i64::from(local.minute()) * 60000
            - i64::from(local.second()) * 1000
            - i64::from(local.timestamp_subsec_millis()),
    ))
}
pub(super) fn midnight(tz: Tz, d: NaiveDate) -> Result<i64> {
    let t = d.and_hms_opt(0, 0, 0).ok_or_else(invalid)?;
    for m in 0..=1440 {
        if let Some(t) = tz.from_local_datetime(&(t + Duration::minutes(m))).earliest() {
            return Ok(t.timestamp_millis());
        }
    }
    Err(invalid())
}
pub(super) struct Axis {
    pub starts: Vec<i64>,
    pub end: i64,
    pub unit: &'static str,
    pub all: bool,
}
impl Axis {
    pub fn for_range(
        range: Range,
        today: NaiveDate,
        tz: Tz,
        first: Option<i64>,
        last: Option<i64>,
    ) -> Result<Self> {
        let (unit, days) = match range {
            Range::Today => ("hour", 1),
            Range::Week => ("day", 7),
            Range::Month => ("day", 30),
            Range::Quarter => ("day", 90),
            Range::Year => ("week", 365),
            Range::All => ("month", 0),
        };
        let mut d = if days == 0 {
            day(tz, first.unwrap_or(midnight(tz, today)?))?.with_day(1).unwrap()
        } else {
            today.checked_sub_days(Days::new(days - 1)).ok_or_else(invalid)?
        };
        let last = if range == Range::All {
            last.map(|v| day(tz, v)).transpose()?.unwrap_or(today)
        } else {
            today
        };
        let end = midnight(tz, last.succ_opt().ok_or_else(invalid)?)?;
        let mut starts = vec![];
        if unit == "hour" {
            // A civil hour can repeat or begin partway through the hour. Walk this
            // single day's actual clock, keeping both occurrences across a transition.
            let mut at = midnight(tz, d)?;
            let mut previous = None;
            while at < end {
                let key = hour(tz, at)?.2;
                if previous != Some(key) {
                    starts.push(at);
                    previous = Some(key);
                }
                at += 60_000;
            }
        } else {
            while d <= last {
                if starts.is_empty()
                    || unit == "day"
                    || unit == "week" && d.weekday() == chrono::Weekday::Mon
                    || unit == "month" && d.day() == 1
                {
                    starts.push(midnight(tz, d)?);
                }
                d = d.succ_opt().ok_or_else(invalid)?;
            }
        }
        starts.retain(|n| *n < end);
        starts.sort_unstable();
        starts.dedup();
        Ok(Self { starts, end, unit, all: range == Range::All })
    }
    pub fn bounds(&self) -> Span {
        (self.starts.first().copied().unwrap_or(self.end), self.end)
    }
    pub fn bin(&self, at: i64) -> Option<usize> {
        if at >= self.end {
            return None;
        }
        self.starts.partition_point(|s| *s <= at).checked_sub(1)
    }
    pub fn pieces(&self, start: i64, end: i64, mut apply: impl FnMut(usize, i64)) {
        let (lo, hi) = self.bounds();
        let mut at = start.max(lo);
        let stop = end.min(hi);
        if at >= stop {
            return;
        }
        let mut index = self.bin(at).expect("clipped interval");
        while at < stop {
            let next = stop.min(self.starts.get(index + 1).copied().unwrap_or(hi));
            apply(index, next - at);
            at = next;
            index += 1;
        }
    }
}
