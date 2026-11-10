//! Responsibilities: pin due-only cadence with fixed times and real run-ledger rows.
//! Not responsible for: native scheduler installation or browser discovery.
//! Failure modes: finish time adds backup duration to the cadence; a faster startup skips
//! the next wake; rounding half a minute runs early; using Linux's requested interval skips
//! its calendar grid; a recent manual run is ignored; partial runs get called successful.

use super::*;
use chrono::{DateTime, Duration, Utc};

fn start() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-09T00:00:00Z").unwrap().with_timezone(&Utc)
}

fn skip_after(interval_minutes: u64, wake_minutes: u64, elapsed: Duration) -> Option<String> {
    let config =
        crate::AppConfig { due_after_hours: interval_minutes as f64 / 60.0, ..Default::default() };
    super::super::run_support::backup_due_skip_reason_at(
        start(),
        &config,
        std::time::Duration::from_secs(wake_minutes * 60),
        start() + elapsed,
    )
}

#[test]
fn hourly_backups_run_at_each_wake_but_wait_after_a_recent_manual_backup() {
    assert!(skip_after(60, 60, Duration::minutes(10)).is_some());
    assert!(skip_after(60, 60, Duration::minutes(30) - Duration::milliseconds(1)).is_some());
    assert_eq!(skip_after(60, 60, Duration::hours(1)), None);
    assert_eq!(skip_after(60, 60, Duration::minutes(30)), None, "ties run at this wake");
}

#[test]
fn daily_backups_skip_six_twelve_and_eighteen_hours() {
    for hours in [6, 12, 18] {
        assert!(skip_after(24 * 60, 6 * 60, Duration::hours(hours)).is_some());
    }
    assert_eq!(skip_after(24 * 60, 6 * 60, Duration::hours(24)), None);
    assert!(skip_after(24 * 60, 6 * 60, Duration::hours(21) - Duration::milliseconds(1)).is_some());
    assert_eq!(skip_after(24 * 60, 6 * 60, Duration::hours(21)), None);
}

#[test]
fn linux_ninety_minute_backups_use_the_thirty_minute_grid() {
    assert!(skip_after(90, 30, Duration::minutes(60)).is_some());
    assert_eq!(skip_after(90, 30, Duration::minutes(90)), None);
    assert!(skip_after(90, 30, Duration::minutes(75) - Duration::milliseconds(1)).is_some());
    assert_eq!(skip_after(90, 30, Duration::minutes(75)), None);
}

#[test]
fn linux_seven_minute_backups_keep_the_half_minute_boundary() {
    assert!(skip_after(7, 1, Duration::minutes(6)).is_some());
    assert_eq!(skip_after(7, 1, Duration::minutes(7)), None);
    assert!(skip_after(7, 1, Duration::seconds(390) - Duration::milliseconds(1)).is_some());
    assert_eq!(skip_after(7, 1, Duration::seconds(390)), None);
}

#[test]
fn a_next_wake_with_a_thirty_second_faster_startup_is_due() {
    assert_eq!(skip_after(60, 60, Duration::hours(1) - Duration::seconds(30)), None);
}

#[test]
fn the_skip_reason_names_the_counted_start_without_claiming_success() {
    assert_eq!(
        skip_after(60, 60, Duration::minutes(10)).as_deref(),
        Some("last counted backup started only 10 minutes ago")
    );
}

#[test]
fn a_forty_minute_backup_does_not_delay_the_next_hourly_start() {
    let vault = crate::Vault::open_temp().unwrap();
    let vault = vault.with_config(crate::AppConfig {
        initialized: true,
        due_after_hours: 1.0,
        ..Default::default()
    });
    let now = Utc::now();
    let started = (now - Duration::hours(1)).to_rfc3339();
    let finished = (now - Duration::minutes(20)).to_rfc3339();
    let probe = crate::test_support::store::Probe::new(&vault);
    for (status, trigger, stats) in [
        ("success", "schedule", "{}"),
        ("partial", "manual", r#"{"profiles":[{"outcome":{"status":"complete"}}]}"#),
    ] {
        probe.execute("DELETE FROM runs", ()).unwrap();
        probe
            .execute(
                "INSERT INTO runs (run_type,trigger,started_at,finished_at,timezone,status,
                   profile_scope_json,warnings_json,stats_json,due_only)
                 VALUES ('backup',?1,?2,?3,'UTC',?4,'[]','[]',?5,0)",
                params![trigger, started.as_str(), finished.as_str(), status, stats],
            )
            .unwrap();
        assert_eq!(
            super::due_skip_reason(&vault, std::time::Duration::from_secs(3_600)).unwrap(),
            None,
            "{status} {trigger}"
        );
    }
}

#[test]
fn schedule_anchor_counts_manual_success_and_completed_partial_profiles_only() {
    let vault = crate::Vault::open_temp().unwrap();
    let probe = crate::test_support::store::Probe::new(&vault);
    assert_eq!(crate::backup_due_anchor(&vault).unwrap(), None);
    let mut expected = None;
    for (index, (run_type, status, stats, counts)) in [
        ("backup", "success", "{}", true),
        ("backup", "partial", r#"{"profiles":[{"outcome":{"status":"complete"}}]}"#, true),
        ("backup", "partial", r#"{"profiles":[{"outcome":{"status":"failed"}}]}"#, false),
        ("backup", "partial", r#"{"profiles":[]}"#, false),
        ("backup", "partial", "invalid JSON", false),
        ("backup", "failed", "{}", false),
        ("backup", "running", "{}", false),
        ("import", "success", "{}", false),
    ]
    .into_iter()
    .enumerate()
    {
        let started = start() + Duration::minutes(index as i64);
        probe
            .execute(
                "INSERT INTO runs (run_type,trigger,started_at,finished_at,timezone,status,
                   profile_scope_json,warnings_json,stats_json,due_only)
                 VALUES (?1,'manual',?2,?3,'UTC',?4,'[]','[]',?5,0)",
                params![
                    run_type,
                    started.to_rfc3339(),
                    (started + Duration::seconds(40)).to_rfc3339(),
                    status,
                    stats
                ],
            )
            .unwrap();
        if counts {
            expected = Some(started);
        }
        assert_eq!(crate::backup_due_anchor(&vault).unwrap(), expected, "{run_type} {status}");
    }
    // More than one query page of non-counting partials must not hide the anchor.
    for _ in 0..20 {
        probe
            .execute(
                "INSERT INTO runs (run_type,trigger,started_at,timezone,status,
               profile_scope_json,warnings_json,stats_json,due_only)
             VALUES ('backup','schedule',?1,'UTC','partial','[]','[]','{}',1)",
                [start().to_rfc3339()],
            )
            .unwrap();
    }
    assert_eq!(crate::backup_due_anchor(&vault).unwrap(), expected);
}
