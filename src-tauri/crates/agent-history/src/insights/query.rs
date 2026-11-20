//! Read bounded batches from immutable publications and cache completed sections by view,
//! range, timezone and filters. Slow consumers never hold a database connection.
use super::{
    Part, Preferences, Query,
    calendar::Calendar,
    catalog::Catalog,
    clock::{self, Axis},
    facts::{Brief, Fact},
    report::Report,
};
use crate::{Archive, Error, Result, chats::list::view};
use chrono::Datelike;
use chrono_tz::Tz;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Instant,
};
use vault_store::{CancelToken, Db, Value as Sql};
pub(super) const D: Db = Db::AgentHistoryReadModel;
#[derive(Default)]
pub(crate) struct Cache {
    pub(super) catalog: Option<((i64, i64), Arc<Catalog>)>,
    sections: VecDeque<(String, Value)>,
}
fn check(c: &CancelToken) -> Result<()> {
    c.check().map_err(|_| Error::Cancelled)
}
impl Archive {
    fn cached_insight(&self, key: &str) -> Option<Value> {
        self.insights
            .lock()
            .unwrap()
            .sections
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }
    fn remember_insight(&self, key: String, v: &Value) {
        let mut cache = self.insights.lock().unwrap();
        cache.sections.retain(|(k, _)| k != &key);
        cache.sections.push_back((key, v.clone()));
        if cache.sections.len() > 8 {
            cache.sections.pop_front();
        }
    }
    /// Streams independently reusable calendar and range sections. The owner supplies the
    /// same published IANA zone as browsing Insights and reads the clock once per request.
    pub fn agent_insights(
        &self,
        q: &Query,
        zone: &str,
        now: i64,
        prefs: &Preferences,
        cancel: &CancelToken,
        mut sink: impl FnMut(&str, Value) -> Result<()>,
    ) -> Result<()> {
        let mut send = |ty: &str, p: Value| {
            check(cancel)?;
            sink(ty, p)
        };
        let tz: Tz =
            zone.parse().map_err(|_| Error::InvalidInput("invalid insights time zone".into()))?;
        let today = clock::day(tz, now)?;
        let effective = Preferences {
            estimate_cost: prefs.estimate_cost && q.include_estimates != Some(false),
            ..prefs.clone()
        };
        let prefs = &effective;
        let range = q.range.unwrap_or(prefs.range);
        let year = q.year.unwrap_or(today.year());
        if !(1..=today.year()).contains(&year) {
            return Err(Error::InvalidInput("invalid calendar year".into()));
        }
        let _job = self.jobs.track(cancel)?;
        self.ensure_state_visible()?;
        self.ensure_projects()?;
        let version = self.store.read(D, view)?;
        let mut harnesses = q.harnesses.iter().map(|h| h.as_str()).collect::<Vec<_>>();
        harnesses.sort_unstable();
        harnesses.dedup();
        let base = serde_json::to_string(&(version, today, zone, &harnesses, &q.project_id))?;
        let report_key = format!("{base}/{range:?}/{}/{}", prefs.show_cost, prefs.estimate_cost);
        let year_key = format!("{base}/calendar/{year}");
        send(
            "started",
            json!({"range":range,"year":year,"timeZone":zone,"today":today,"viewToken":format!("{}:{}",version.0,version.1)}),
        )?;
        check(cancel)?;
        let report_wanted = q.part != Part::Calendar;
        let year_wanted = q.part != Part::Report;
        let mut cached_report = report_wanted.then(|| self.cached_insight(&report_key)).flatten();
        let mut cached_year = year_wanted.then(|| self.cached_insight(&year_key)).flatten();
        if let Some(v) = &cached_report {
            sections(v, &mut send)?
        }
        if let Some(v) = &cached_year {
            send("calendar", v.clone())?
        }
        let mut early_work = None;
        let global = q.project_id.is_none() && q.harnesses.is_empty();
        if global {
            let need_year = year_wanted && cached_year.is_none();
            let need_work = report_wanted && cached_report.is_none();
            if need_year || need_work {
                let (work, cal) = self.wall_clock(
                    need_work.then_some(range),
                    need_year.then_some(year),
                    today,
                    tz,
                    cancel,
                    |done| send("progress", json!({"phase":"hours","done":done})),
                )?;
                if self.store.read(D, view)? == version {
                    if let Some(work) = work {
                        send("work", work.clone())?;
                        early_work = Some(work);
                    }
                    if let Some(cal) = cal {
                        self.remember_insight(year_key.clone(), &cal);
                        send("calendar", cal.clone())?;
                        cached_year = Some(cal);
                    }
                }
            }
        }
        if report_wanted && cached_report.is_none() || year_wanted && cached_year.is_none() {
            let _reader = self.jobs.enter_window(cancel)?;
            let _pin = self.jobs.pin_publication()?;
            let mut tick = Instant::now();
            let mut progress = |phase: &str, done: usize| -> Result<()> {
                check(cancel)?;
                if tick.elapsed().as_millis() >= 70 {
                    send("progress", json!({"phase":phase,"done":done}))?;
                    tick = Instant::now();
                }
                Ok(())
            };
            let cat = self.insight_catalog(version, cancel, |n| progress("conversations", n))?;
            if cat.view != version {
                // A backup published during admission. Replace any provisional early sections
                // from the earlier view and use this roster throughout the rest of the scan.
                cached_year = None;
                cached_report = None;
            }
            let cacheable = cat.view == version;
            let mut first = None;
            let mut last = None;
            let mut pool = 0;
            let mut first_usage = std::collections::BTreeMap::new();
            for (i, s) in cat.sources.iter().enumerate() {
                if cat.eligible(i, q) {
                    first = first.into_iter().chain(s.lo).min();
                    last = last.max(s.hi);
                    pool += usize::from(i == s.root);
                    if let Some(at) = s.first_usage {
                        let earliest = first_usage.entry(s.harness.clone()).or_insert(at);
                        *earliest = (*earliest).min(at);
                    }
                }
            }
            let mut report = if report_wanted && cached_report.is_none() {
                Some(Report::new(
                    Axis::for_range(range, today, tz, first, last)?,
                    tz,
                    first_usage,
                    early_work.as_ref().filter(|_| cacheable),
                ))
            } else {
                None
            };
            let mut calendar = if year_wanted && cached_year.is_none() {
                Some(Calendar::new(year, today, tz)?)
            } else {
                None
            };
            self.read_insight_pages(q, &cat, &mut report, &mut calendar, cancel, |n| {
                progress("pages", n)
            })?;
            check(cancel)?;
            if cat.view.1 != self.store.read(D, view)?.1 {
                return Err(Error::StaleCursor);
            }
            if let Some(c) = calendar {
                let value =
                    c.finish(first.map(|t| clock::day(tz, t)).transpose()?.map(|d| d.year()));
                if cacheable {
                    self.remember_insight(year_key, &value);
                }
                send("calendar", value)?;
            }
            if let Some(r) = report {
                let mut value = r.finish(&cat, prefs, cancel, |n| {
                    send("progress", json!({"phase":"totals","done":n}))
                })?;
                value["overview"]["scopeRoots"] = pool.into();
                value["overview"]["savedRoots"] = cat.saved_roots.into();
                value["overview"]["visibleRoots"] =
                    cat.sources.iter().enumerate().filter(|(i, s)| s.root == *i).count().into();
                if cacheable {
                    self.remember_insight(report_key, &value);
                }
                sections(&value, &mut send)?;
            }
        }
        check(cancel)?;
        send("finished", json!({"newerViewAvailable":self.store.read(D,view)?!=version}))
    }
    fn read_insight_pages(
        &self,
        q: &Query,
        cat: &Catalog,
        report: &mut Option<Report>,
        calendar: &mut Option<Calendar>,
        cancel: &CancelToken,
        mut progress: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        let yr = calendar.as_ref().map(Calendar::bounds);
        let window = report.as_ref().map(|r| r.axis.bounds());
        let lo = yr
            .map(|v| v.0)
            .into_iter()
            .chain(window.map(|v| v.0.saturating_sub(900000)))
            .min()
            .unwrap();
        let hi = yr
            .map(|v| v.1)
            .into_iter()
            .chain(window.map(|v| v.1.saturating_add(900000)))
            .max()
            .unwrap();
        let all = report.as_ref().is_some_and(|r| r.axis.all);
        let mut processed = 0;
        self.insight_batches(cat, q, (lo, hi), all, cancel, |batch| {
            processed += batch.len();
            let mut needed = HashMap::new();
            for super::pages::Page {
                id: row,
                publication: pub_id,
                lo: start,
                hi: end,
                undated,
                brief: data,
            } in batch
            {
                let Some(&idx) = cat.publications.get(&pub_id) else { continue };
                if !cat.eligible(idx, q) {
                    continue;
                }
                let src = &cat.sources[idx];
                let b: Brief =
                    postcard::from_bytes(&data).map_err(|_| Error::ReaderRebuildRequired)?;
                let mut slow_report = false;
                let mut slow_year = false;
                if let Some(r) = report {
                    let (l, h) = r.axis.bounds();
                    if r.fast(start, end, undated)? {
                        r.bulk(idx, src, &b, start)?;
                    } else if start.zip(end).is_some_and(|(s, e)| {
                        e >= l.saturating_sub(900000) && s < h.saturating_add(900000)
                    }) || r.axis.all && undated
                    {
                        slow_report = true;
                    }
                }
                if let Some(c) = calendar
                    && idx == src.root
                    && let Some((s, e)) = start.zip(end)
                {
                    if c.single_day(s, e) {
                        c.bulk(&b, s);
                    } else if e >= c.bounds().0 && s < c.bounds().1 {
                        slow_year = true;
                    }
                }
                if slow_report || slow_year {
                    needed.insert(row, (idx, slow_report, slow_year));
                }
            }
            if !needed.is_empty() {
                let args = needed.keys().copied().map(Sql::Integer).collect::<Vec<_>>();
                let details = self.store.read(D, move |s| {
                    s.query(
                        &format!(
                            "SELECT rowid,detail FROM ah_insight_page WHERE rowid IN ({})",
                            vec!["?"; args.len()].join(",")
                        ),
                        args,
                        |r| Ok((r.get::<i64>(0)?, r.get::<Vec<u8>>(1)?)),
                    )
                })?;
                for (id, data) in details {
                    check(cancel)?;
                    let (idx, r, c) = needed[&id];
                    let events: Vec<Fact> =
                        postcard::from_bytes(&data).map_err(|_| Error::ReaderRebuildRequired)?;
                    if r {
                        report.as_mut().unwrap().detail(idx, &cat.sources[idx], &events)?
                    }
                    if c {
                        calendar.as_mut().unwrap().detail(&events);
                    }
                }
            }
            progress(processed)
        })
    }
}
fn sections(v: &Value, send: &mut impl FnMut(&str, Value) -> Result<()>) -> Result<()> {
    for name in ["overview", "series", "breakdowns"] {
        send(name, v[name].clone())?
    }
    Ok(())
}
