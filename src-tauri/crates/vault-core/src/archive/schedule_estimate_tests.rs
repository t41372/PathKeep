//! Responsibilities: pin the displayed schedule estimate to the due clock and native wake grid.
//! Not responsible for: installing schedules or simulating OS sleep/login behavior.
//! Failure modes: completion adds seconds or minutes; manual runs move the due clock off-grid;
//! overdue estimates land in the past or on an epoch grid; missing evidence invents a phase;
//! no counted backup waits a full backup interval; Linux uses 90 instead of 30 minutes.

use crate::{ScheduledBackupAttempt, ScheduledBackupAttemptOutcome, ScheduledBackupAttemptPhase};
use chrono::{DateTime, Duration, Utc};

fn at(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text).unwrap().with_timezone(&Utc)
}

struct Case<'a> {
    anchor: Option<&'a str>,
    finished: Option<&'a str>,
    last_wake: Option<&'a str>,
    installed: Option<&'a str>,
    interval_minutes: u64,
    wake_minutes: u64,
    now: &'a str,
    expected: &'a str,
    old_answer: &'a str,
}

/// Reproduces the old TypeScript formula with the default six-hour check cap and its
/// one-hour minimum, including platforms whose actual wake interval is shorter.
fn old_estimate(case: &Case<'_>) -> DateTime<Utc> {
    let now = at(case.now);
    let due = case
        .finished
        .map(|finished| at(finished) + Duration::minutes(case.interval_minutes as i64))
        .unwrap_or(now);
    if due > now {
        due
    } else {
        let step =
            Duration::minutes(case.interval_minutes.clamp(60, 6 * 60) as i64).num_milliseconds();
        DateTime::from_timestamp_millis(((now.timestamp_millis() + step - 1) / step) * step)
            .unwrap()
    }
}

fn check(case: Case<'_>) {
    let expected = at(case.expected);
    let old = old_estimate(&case);
    assert_eq!(old, at(case.old_answer), "document the old TypeScript answer");
    assert_ne!(old, expected, "this case must distinguish the old rule");
    // Each outcome still supplies the newest wake; an older success must not set the phase.
    for outcome in [
        ScheduledBackupAttemptOutcome::Running,
        ScheduledBackupAttemptOutcome::Success,
        ScheduledBackupAttemptOutcome::Partial,
        ScheduledBackupAttemptOutcome::Skipped,
        ScheduledBackupAttemptOutcome::Deferred,
        ScheduledBackupAttemptOutcome::Failed,
        ScheduledBackupAttemptOutcome::Interrupted,
    ] {
        let mut attempts: Vec<_> = case
            .last_wake
            .map(|started| ScheduledBackupAttempt {
                id: "newest-wake".into(),
                started_at: started.into(),
                finished_at: None,
                outcome,
                phase: ScheduledBackupAttemptPhase::WorkerEntry,
                reason_code: None,
                detail: None,
                run_id: None,
            })
            .into_iter()
            .collect();
        if let Some(newest) = attempts.first() {
            attempts.push(ScheduledBackupAttempt {
                id: "older-success".into(),
                started_at: (at(&newest.started_at) - Duration::minutes(17)).to_rfc3339(),
                outcome: ScheduledBackupAttemptOutcome::Success,
                ..newest.clone()
            });
        }
        assert_eq!(
            super::schedule::next_scheduled_backup_at(
                case.anchor.map(at),
                &attempts,
                case.installed,
                case.interval_minutes as f64 / 60.0,
                std::time::Duration::from_secs(case.wake_minutes * 60),
                at(case.now),
            ),
            Some(expected),
            "{outcome:?}"
        );
    }
}

#[test]
fn schedule_estimate_hourly_five_second_backup() {
    check(Case {
        anchor: Some("2026-11-09T00:00:00Z"),
        finished: Some("2026-11-09T00:00:05Z"),
        last_wake: Some("2026-11-09T00:00:00Z"),
        installed: None,
        interval_minutes: 60,
        wake_minutes: 60,
        now: "2026-11-09T00:10:00Z",
        expected: "2026-11-09T01:00:00Z",
        old_answer: "2026-11-09T01:00:05Z",
    });
}

#[test]
fn schedule_estimate_forty_minute_backup() {
    check(Case {
        anchor: Some("2026-11-09T00:00:00Z"),
        finished: Some("2026-11-09T00:40:00Z"),
        last_wake: Some("2026-11-09T00:00:00Z"),
        installed: None,
        interval_minutes: 60,
        wake_minutes: 60,
        now: "2026-11-09T00:45:00Z",
        expected: "2026-11-09T01:00:00Z",
        old_answer: "2026-11-09T01:40:00Z",
    });
}

#[test]
fn schedule_estimate_daily_manual_backup_off_the_six_hour_grid() {
    check(Case {
        anchor: Some("2026-11-08T09:00:00Z"),
        finished: Some("2026-11-08T09:00:05Z"),
        last_wake: Some("2026-11-09T00:00:00Z"),
        installed: None,
        interval_minutes: 24 * 60,
        wake_minutes: 6 * 60,
        now: "2026-11-09T00:15:00Z",
        expected: "2026-11-09T06:00:00Z",
        old_answer: "2026-11-09T09:00:05Z",
    });
}

#[test]
fn schedule_estimate_overdue_after_sleep_uses_the_next_future_wake() {
    for (now, expected) in [
        ("2026-11-09T10:10:00Z", "2026-11-09T10:17:00Z"),
        ("2026-11-09T10:17:00Z", "2026-11-09T11:17:00Z"),
    ] {
        check(Case {
            anchor: Some("2026-11-08T00:00:00Z"),
            finished: Some("2026-11-08T00:00:05Z"),
            last_wake: Some("2026-11-08T03:17:00Z"),
            installed: None,
            interval_minutes: 60,
            wake_minutes: 60,
            now,
            expected,
            old_answer: "2026-11-09T11:00:00Z",
        });
    }
}

#[test]
fn schedule_estimate_without_a_ledger_uses_the_installation_phase() {
    check(Case {
        anchor: Some("2026-11-09T00:00:00Z"),
        finished: Some("2026-11-09T00:00:05Z"),
        last_wake: None,
        installed: Some("2026-11-09T00:17:00Z"),
        interval_minutes: 60,
        wake_minutes: 60,
        now: "2026-11-09T00:20:00Z",
        expected: "2026-11-09T01:17:00Z",
        old_answer: "2026-11-09T01:00:05Z",
    });
}

#[test]
fn schedule_estimate_without_any_phase_uses_anchor_plus_interval() {
    for (now, expected, old_answer) in [
        ("2026-11-09T00:20:00Z", "2026-11-09T01:17:00Z", "2026-11-09T01:17:05Z"),
        ("2026-11-09T10:10:00Z", "2026-11-09T10:17:00Z", "2026-11-09T11:00:00Z"),
    ] {
        check(Case {
            anchor: Some("2026-11-09T00:17:00Z"),
            finished: Some("2026-11-09T00:17:05Z"),
            last_wake: None,
            installed: None,
            interval_minutes: 60,
            wake_minutes: 60,
            now,
            expected,
            old_answer,
        });
    }
}

#[test]
fn schedule_estimate_without_a_counted_backup_uses_the_next_wake() {
    for (last_wake, installed, expected) in [
        (Some("2026-11-09T00:17:00Z"), None, "2026-11-09T06:17:00Z"),
        (None, Some("2026-11-09T00:17:00Z"), "2026-11-09T06:17:00Z"),
        (None, None, "2026-11-09T06:20:00Z"),
    ] {
        check(Case {
            anchor: None,
            finished: None,
            last_wake,
            installed,
            interval_minutes: 24 * 60,
            wake_minutes: 6 * 60,
            now: "2026-11-09T00:20:00Z",
            expected,
            old_answer: "2026-11-09T06:00:00Z",
        });
    }
}

#[test]
fn schedule_estimate_linux_ninety_minutes_on_thirty_minute_wakes() {
    check(Case {
        anchor: Some("2026-11-09T00:00:00Z"),
        finished: Some("2026-11-09T00:05:00Z"),
        last_wake: Some("2026-11-09T00:30:00Z"),
        installed: None,
        interval_minutes: 90,
        wake_minutes: 30,
        now: "2026-11-09T01:20:00Z",
        expected: "2026-11-09T01:30:00Z",
        old_answer: "2026-11-09T01:35:00Z",
    });
}
