//! Responsibilities: share the counted backup anchor and due timing with schedule estimates.
//! Not responsible for: native scheduler inspection, installation, or localized presentation.
//! The archive lookup pages through the run ledger; estimating a future wake is constant time.

use crate::{ScheduledBackupAttempt, Vault};
use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use std::time::Duration;
use vault_store::Db;

/// Keeps due-only execution and the displayed estimate on the same archive clock.
/// Successes and partials that read a profile to the end count, including manual backups.
pub fn backup_due_anchor(vault: &Vault) -> Result<Option<DateTime<Utc>>> {
    let latest = vault.store().read(Db::Archive, |s| {
        let mut before = i64::MAX;
        loop {
            let runs = s.query(
                "SELECT id,started_at,status,stats_json FROM runs
                 WHERE run_type='backup' AND status IN ('success','partial') AND id<?1
                 ORDER BY id DESC LIMIT 16",
                [before],
                |r| {
                    Ok((
                        r.get::<i64>(0)?,
                        r.get::<String>(1)?,
                        r.get::<String>(2)?,
                        r.get::<String>(3)?,
                    ))
                },
            )?;
            let Some((last, ..)) = runs.last() else { return Ok(None) };
            before = *last;
            if let Some((_, started, ..)) = runs
                .into_iter()
                .find(|(_, _, status, stats)| status == "success" || read_one_to_the_end(stats))
            {
                return Ok(Some(started));
            }
        }
    })?;
    Ok(latest.as_deref().and_then(parse_timestamp))
}

/// Estimates the first wake strictly after `now` that passes the execution due rule.
///
/// `attempts` is newest first, as returned by the schedule ledger. Every outcome is a wake.
/// Without attempts, installation sets the phase; without either, use anchor + interval.
/// With no evidence at all, estimate one wake interval from now. Login restarts the grid and
/// sleep shifts it, so this predicts the current grid rather than an OS-guaranteed start time.
pub fn next_scheduled_backup_at(
    anchor: Option<DateTime<Utc>>,
    attempts: &[ScheduledBackupAttempt],
    installed_at: Option<&str>,
    due_after_hours: f64,
    wake_interval: Duration,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    // Native schedules use whole minutes. Milliseconds retain the observed phase and allow
    // direct arithmetic even when the computer has missed years of wakes.
    let step = i64::try_from(wake_interval.as_millis()).ok().filter(|step| *step > 0)?;
    let wake = TimeDelta::from_std(wake_interval).ok()?;
    let phase = attempts
        .first()
        .and_then(|attempt| parse_timestamp(&attempt.started_at))
        .or_else(|| installed_at.and_then(parse_timestamp))
        .or_else(|| {
            anchor?.checked_add_signed(TimeDelta::from_std(backup_interval(due_after_hours)).ok()?)
        })
        .unwrap_or(now);
    let due = match anchor {
        Some(anchor) => anchor.checked_add_signed(
            TimeDelta::from_std(backup_due_delay(due_after_hours, wake_interval)).ok()?,
        )?,
        None => now,
    };
    let target = due.max(now);
    let steps = target.signed_duration_since(phase).num_milliseconds().div_euclid(step).max(0);
    let next = phase.checked_add_signed(TimeDelta::milliseconds(steps.checked_mul(step)?))?;
    // Equality with the due boundary passes; equality with now needs the following wake.
    if next <= now || next < due { next.checked_add_signed(wake) } else { Some(next) }
}

/// The execution rule's half-wake allowance, shared with the displayed estimate.
pub(super) fn backup_due_delay(due_after_hours: f64, wake_interval: Duration) -> Duration {
    backup_interval(due_after_hours).saturating_sub(wake_interval / 2)
}

fn backup_interval(due_after_hours: f64) -> Duration {
    let minutes = (due_after_hours * 60.0).round();
    let minutes = if minutes.is_finite() && minutes >= 1.0 && minutes <= u64::MAX as f64 {
        minutes as u64
    } else {
        1
    };
    Duration::from_secs(minutes.saturating_mul(60))
}

fn parse_timestamp(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text).ok().map(|at| at.with_timezone(&Utc))
}

fn read_one_to_the_end(stats: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(stats).ok().is_some_and(|stats| {
        stats["profiles"].as_array().is_some_and(|profiles| {
            profiles.iter().any(|profile| profile["outcome"]["status"] == "complete")
        })
    })
}
