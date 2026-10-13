//! Responsibilities: the time zone Insights puts visits on days and hours in
//! (`docs/architecture/insights-time-zone.md`): a named IANA zone of its own, chosen once and kept
//! when the computer's zone changes, and the conversions every build and read use with it.
//! Not responsible for: which zone that is (the build's state and the published generation say),
//! History and search, which follow the computer's zone, or storing anything (the archive keeps
//! UTC instants only).
//! Failure modes: a day key or hour worked out in the computer's zone instead (a backup after the
//! zone changed then drops or double-counts a day's visits); a daylight-saving change ignored
//! (23- and 25-hour days, a repeated hour); a day whose midnight does not exist (some zones skip
//! it) failing instead of starting at its first instant.

use crate::Vault;
use crate::models::{AppConfig, DateRange, InsightsTimeZone};
use anyhow::{Context, Result};
use chrono::{DateTime, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use vault_store::{Db, Session};

/// The zone of the Insights on screen and the zone of the next build, for Settings and for the
/// screens that put dates in Insights' zone.
pub fn insights_time_zone(vault: &Vault) -> Result<InsightsTimeZone> {
    let config = vault.config().clone();
    vault.read(Db::Intelligence, move |s| {
        let next = InsightsZone::for_build(s, &config)?;
        Ok(InsightsTimeZone {
            shown: InsightsZone::for_reading(s)?.name().to_owned(),
            next: next.name().to_owned(),
            rebuilding_from: InsightsZone::published_besides(s, next)?,
        })
    })
}

/// Today in the zone of the Insights on screen, `YYYY-MM-DD`.
pub fn insights_today(vault: &Vault) -> Result<String> {
    vault.read(Db::Intelligence, |s| Ok(InsightsZone::for_reading(s)?.today_key()))
}

/// Whether Insights can be put in the zone `name`: an IANA name, such as `Asia/Taipei`.
pub fn is_insights_time_zone(name: &str) -> bool {
    InsightsZone::named(name).is_some()
}

/// The zone Insights works in: a named IANA zone, so its daylight-saving rules apply to every
/// instant on its own date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InsightsZone(Tz);

impl InsightsZone {
    /// The zone with this IANA name, if it is one.
    pub(crate) fn named(name: &str) -> Option<Self> {
        name.parse::<Tz>().ok().map(Self)
    }

    /// The computer's zone now (`crate::utils::computer_time_zone`).
    pub(crate) fn system() -> Self {
        Self(crate::utils::computer_time_zone())
    }

    /// The zone a build puts days in: the one chosen in Settings; else the one the data was built
    /// in, so a change of the computer's zone alone never moves the days; else the computer's.
    pub(crate) fn for_build(s: &Session<'_>, config: &AppConfig) -> Result<Self> {
        if let Some(zone) = config.insights_time_zone.as_deref().and_then(Self::named) {
            return Ok(zone);
        }
        Ok(Self::published(s)?.unwrap_or_else(Self::system))
    }

    /// The zone the published data's days are in, which reads interpret dates in (until a
    /// rebuild in a newly chosen zone is published); the computer's before the first build.
    pub(crate) fn for_reading(s: &Session<'_>) -> Result<Self> {
        Ok(Self::published(s)?.unwrap_or_else(Self::system))
    }

    /// The zone recorded with the published data. Every profile is built in the same one, except
    /// while a rebuild in a new zone has published some profiles and not yet the others: then the
    /// first profile's, so that every read in the meantime agrees.
    pub(crate) fn published(s: &Session<'_>) -> Result<Option<Self>> {
        let names = s.query(
            "SELECT time_zone FROM intelligence_generations WHERE time_zone IS NOT NULL
             ORDER BY profile_id LIMIT 1",
            (),
            |r| Ok(r.get::<String>(0)?),
        )?;
        Ok(names.first().and_then(|name| Self::named(name)))
    }

    /// A zone some profile's published data is in other than `zone`. A rebuild in a new zone
    /// publishes one profile at a time, and reads move to the new zone with the first one
    /// (`published`): until the last one, this says the rebuild is not done.
    pub(crate) fn published_besides(s: &Session<'_>, zone: Self) -> Result<Option<String>> {
        let names = s.query(
            "SELECT time_zone FROM intelligence_generations WHERE time_zone IS NOT NULL
               AND time_zone<>?1 ORDER BY profile_id LIMIT 1",
            [zone.name()],
            |r| Ok(r.get::<String>(0)?),
        )?;
        Ok(names.into_iter().next())
    }

    /// Its IANA name, which builds record and settings show.
    pub(crate) fn name(self) -> &'static str {
        self.0.name()
    }

    /// The instant in this zone.
    pub(crate) fn datetime(self, ms: i64) -> DateTime<Tz> {
        Utc.timestamp_millis_opt(ms).single().unwrap_or_default().with_timezone(&self.0)
    }

    /// Today here.
    pub(crate) fn today(self) -> NaiveDate {
        Utc::now().with_timezone(&self.0).date_naive()
    }

    /// Today here, `YYYY-MM-DD`.
    pub(crate) fn today_key(self) -> String {
        self.today().format("%Y-%m-%d").to_string()
    }

    /// The day of the instant here, `YYYY-MM-DD`.
    pub(crate) fn date_key(self, ms: i64) -> String {
        self.datetime(ms).format("%Y-%m-%d").to_string()
    }

    /// The first instant of `date` here: its midnight, or when a zone skips midnight that day,
    /// the first time that exists; at a repeated midnight, the earlier one.
    pub(crate) fn start_of(self, date: NaiveDate) -> Result<DateTime<Tz>> {
        let mut time: NaiveDateTime = date.and_hms_opt(0, 0, 0).expect("midnight");
        // A skipped hour is at most two hours long anywhere; step to the first minute that exists.
        for _ in 0..=180 {
            match self.0.from_local_datetime(&time) {
                LocalResult::Single(value) => return Ok(value),
                LocalResult::Ambiguous(earlier, _) => return Ok(earlier),
                LocalResult::None => time += chrono::Duration::minutes(1),
            }
        }
        anyhow::bail!("{date} has no start in {}", self.name())
    }

    /// An inclusive range of days here as `[start_ms, end_ms)`.
    pub(crate) fn range_bounds(self, range: &DateRange) -> Result<(i64, i64)> {
        let day = |text: &str| {
            NaiveDate::parse_from_str(text, "%Y-%m-%d").with_context(|| format!("parsing {text}"))
        };
        let (start, end) = (day(&range.start)?, day(&range.end)?);
        let after = end.succ_opt().unwrap_or(end);
        Ok((self.start_of(start)?.timestamp_millis(), self.start_of(after)?.timestamp_millis()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn zone(name: &str) -> InsightsZone {
        InsightsZone::named(name).expect("an IANA zone")
    }

    fn ms(rfc3339: &str) -> i64 {
        DateTime::parse_from_rfc3339(rfc3339).unwrap().timestamp_millis()
    }

    fn range(start: &str, end: &str) -> DateRange {
        DateRange { start: start.into(), end: end.into() }
    }

    /// New York's spring day has 23 hours and its autumn day 25; an hour bucket on the repeated
    /// hour gets both of its instants.
    #[test]
    fn daylight_saving_days_have_their_real_length() {
        let new_york = zone("America/New_York");
        let (start, end) = new_york.range_bounds(&range("2026-03-08", "2026-03-08")).unwrap();
        assert_eq!(start, ms("2026-03-08T05:00:00Z"));
        assert_eq!(end - start, 23 * 3_600_000);
        let (start, end) = new_york.range_bounds(&range("2026-11-01", "2026-11-01")).unwrap();
        assert_eq!(start, ms("2026-11-01T04:00:00Z"));
        assert_eq!(end - start, 25 * 3_600_000);
        assert_eq!(new_york.datetime(ms("2026-11-01T05:30:00Z")).hour(), 1);
        assert_eq!(new_york.datetime(ms("2026-11-01T06:30:00Z")).hour(), 1);
    }

    /// São Paulo moved its clocks from midnight to one o'clock on 2018-11-04: that day starts at
    /// one, and the day before ends there.
    #[test]
    fn a_day_whose_midnight_is_skipped_starts_at_its_first_instant() {
        let sao_paulo = zone("America/Sao_Paulo");
        let (start, end) = sao_paulo.range_bounds(&range("2018-11-04", "2018-11-04")).unwrap();
        assert_eq!(start, ms("2018-11-04T03:00:00Z"));
        assert_eq!(end, ms("2018-11-05T02:00:00Z"));
        let (_, previous_end) = sao_paulo.range_bounds(&range("2018-11-03", "2018-11-03")).unwrap();
        assert_eq!(previous_end, start);
        assert_eq!(sao_paulo.date_key(ms("2018-11-04T03:00:00Z")), "2018-11-04");
    }
}
