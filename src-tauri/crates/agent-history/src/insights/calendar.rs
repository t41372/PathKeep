//! Year cards use the filtered conversation pool independently of the selected report range.
use super::{
    clock::{self, Axis, Span},
    facts::{Brief, Fact},
};
use crate::{Error, Result};
use chrono::{Datelike, NaiveDate};
use chrono_tz::Tz;
use serde_json::{Value, json};

pub(super) struct Calendar {
    axis: Axis,
    dates: Vec<NaiveDate>,
    active: Vec<bool>,
    timed: Vec<bool>,
    spans: Vec<Span>,
    closed_ms: Vec<i64>,
    year: i32,
    today: NaiveDate,
    tz: Tz,
}
impl Calendar {
    pub fn new(year: i32, today: NaiveDate, tz: Tz) -> Result<Self> {
        let date = |y| {
            NaiveDate::from_ymd_opt(y, 1, 1)
                .ok_or_else(|| Error::InvalidInput("invalid calendar year".into()))
        };
        let mut day = date(year)?;
        let next = date(
            year.checked_add(1)
                .ok_or_else(|| Error::InvalidInput("invalid calendar year".into()))?,
        )?;
        let mut dates = vec![];
        let mut starts = vec![];
        while day < next {
            dates.push(day);
            starts.push(clock::midnight(tz, day)?);
            day = day.succ_opt().unwrap();
        }
        let n = dates.len();
        Ok(Self {
            axis: Axis { starts, end: clock::midnight(tz, next)?, unit: "day", all: false },
            dates,
            active: vec![false; n],
            timed: vec![false; n],
            spans: vec![],
            closed_ms: vec![0; n],
            year,
            today,
            tz,
        })
    }
    pub fn bounds(&self) -> Span {
        self.axis.bounds()
    }
    pub fn bulk(&mut self, b: &Brief, lo: i64) {
        if let Some(i) = self.axis.bin(lo) {
            self.active[i] = true;
            self.timed[i] |= b.summed.is_some();
        }
        self.spans.extend(&b.spans);
    }
    pub fn wall_total(&mut self, at: i64, ms: i64, known: bool) {
        if let Some(i) = self.axis.bin(at) {
            self.active[i] = true;
            self.timed[i] |= known;
            self.closed_ms[i] += ms;
        }
    }
    pub fn wall_span(&mut self, lo: i64, hi: i64) {
        self.spans.push((lo, hi));
    }
    pub fn wall_point(&mut self, at: i64, known: bool) {
        if let Some(i) = self.axis.bin(at) {
            self.active[i] = true;
            self.timed[i] |= known;
        }
    }
    pub fn single_day(&self, lo: i64, hi: i64) -> bool {
        self.axis.bin(lo).is_some() && self.axis.bin(lo) == self.axis.bin(hi)
    }
    pub fn detail(&mut self, events: &[Fact]) {
        for f in events {
            if let Some(i) = f.at().and_then(|t| self.axis.bin(t)) {
                self.active[i] = true;
            }
            if let Fact::Work { at: Some(s), end: Some(e), .. } = f {
                self.spans.push((*s, *e));
                if let Some(i) = self.axis.bin(*s) {
                    self.timed[i] = true;
                }
            }
        }
    }
    pub fn finish(mut self, first_year: Option<i32>) -> Value {
        let mut ms = std::mem::take(&mut self.closed_ms);
        for (s, e) in clock::union(self.spans) {
            self.axis.pieces(s, e, |i, n| {
                self.active[i] = true;
                self.timed[i] = true;
                ms[i] += n;
            });
        }
        let mut streak = 0;
        let mut best = 0;
        let mut end = 0;
        let mut peak = None;
        let mut days = vec![];
        let mut months = [0i64; 12];
        let mut month_timed = [false; 12];
        let mut month_active = [false; 12];
        let (mut active, mut measured, mut elapsed) = (0, 0, 0);
        for (i, &date) in self.dates.iter().enumerate() {
            let future = date > self.today;
            let value = (!future && (!self.active[i] || self.timed[i])).then_some(ms[i]);
            days.push(
                json!({"day":date,"busyMs":value,"active":!future&&self.active[i],"future":future}),
            );
            if future {
                continue;
            }
            elapsed += 1;
            if self.active[i] {
                active += 1;
                streak += 1;
                if streak > best {
                    best = streak;
                    end = i;
                }
                month_active[date.month0() as usize] = true;
                if self.timed[i] {
                    measured += 1;
                    month_timed[date.month0() as usize] = true;
                    if peak.is_none_or(|p: usize| ms[p] < ms[i]) {
                        peak = Some(i);
                    }
                }
            } else {
                streak = 0
            }
            months[date.month0() as usize] += ms[i];
        }
        let total: i64 = months.iter().sum();
        let month = months
            .iter()
            .enumerate()
            .filter(|(i, _)| month_timed[*i])
            .max_by_key(|(i, n)| (**n, std::cmp::Reverse(*i)))
            .map(|(i, _)| i + 1);
        let bars:Vec<_>=months.iter().enumerate().map(|(i,&n)|{
            let future=self.year==self.today.year() && i>self.today.month0() as usize;
            json!({"month":i+1,"busyMs":(!future && (!month_active[i]||month_timed[i])).then_some(n),"future":future})
        }).collect();
        json!({"year":self.year,"timeZone":self.tz.name(),"firstYearWithData":first_year,"currentYear":self.today.year(),"days":days,"months":bars,
            "busyMs":(active==0||measured>0).then_some(total),"elapsedDays":elapsed,"activeDays":active,"timedActiveDays":measured,"averageBusyMs":(measured>0).then(||total as f64/measured as f64),
            "longestStreak":best,"streakStart":(best>0).then(||self.dates[end+1-best]),"streakEnd":(best>0).then(||self.dates[end]),
            "busiestDay":peak.map(|i|self.dates[i]),"busiestMonth":month})
    }
}
