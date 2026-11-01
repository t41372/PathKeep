//! Archive-independent scheduled-backup attempt ledger.
//!
//! ## Responsibilities
//! - Durably record every native-scheduler worker invocation before keyring,
//!   config, App Lock state, or archive open can fail.
//! - Keep each attempt in its own atomically replaced file so concurrent worker
//!   invocations cannot overwrite one another's evidence.
//! - Bound both retained files and returned DTOs independently of archive size.
//!
//! ## Not responsible for
//! - Native scheduler installation or host-state inspection.
//! - Canonical `runs` rows once the archive can be opened safely.
//! - Translating stable reason/issue codes into user-visible copy.
//!
//! ## Performance notes
//! - At most 64 small JSON records are retained. Reads and pruning therefore
//!   remain O(64), regardless of the 14.4M-row archive size.

use crate::{
    ProjectPaths,
    durable_io::atomic_durable_write,
    models::{
        ScheduledBackupAttempt, ScheduledBackupAttemptOutcome, ScheduledBackupAttemptPhase,
        ScheduledBackupHealthIssue,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const ATTEMPT_FILE_PREFIX: &str = "attempt-";
const ATTEMPT_FILE_SUFFIX: &str = ".json";
const LAST_SUCCESS_FILE: &str = "last-success.json";
const MAX_RETAINED_ATTEMPTS: usize = 64;
const MIN_STALE_RUNNING_MINUTES: i64 = 15;
/// How long a scheduler that works may take to start the worker once the computer wakes or
/// starts: launchd runs a job it missed on waking, Task Scheduler's "start when available" can
/// take some minutes.
const CATCH_UP_MINUTES: i64 = 15;
static ATTEMPT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Read result used by schedule status without making one malformed record hide
/// every other attempt.
#[derive(Debug, Default)]
pub struct ScheduleAttemptLedgerSnapshot {
    pub attempts: Vec<ScheduledBackupAttempt>,
    pub last_success_at: Option<String>,
    pub health_issues: Vec<ScheduledBackupHealthIssue>,
}

/// Handle used to advance and finalize one durable attempt.
#[derive(Debug)]
pub struct ScheduleAttemptRecorder {
    path: PathBuf,
    attempt: ScheduledBackupAttempt,
}

impl ScheduleAttemptRecorder {
    /// Creates the first durable evidence before any fallible backup setup.
    pub fn begin(paths: &ProjectPaths) -> Result<Self> {
        let directory = attempt_directory(paths);
        fs::create_dir_all(&directory)
            .with_context(|| format!("creating schedule attempt ledger {}", directory.display()))?;
        finalize_orphaned_running_attempts(&directory)?;
        prune_attempt_files(&directory, MAX_RETAINED_ATTEMPTS.saturating_sub(1))?;

        let id = new_attempt_id();
        let path = directory.join(format!("{ATTEMPT_FILE_PREFIX}{id}{ATTEMPT_FILE_SUFFIX}"));
        let recorder = Self {
            path,
            attempt: ScheduledBackupAttempt {
                id,
                started_at: now_rfc3339(),
                finished_at: None,
                outcome: ScheduledBackupAttemptOutcome::Running,
                phase: ScheduledBackupAttemptPhase::WorkerEntry,
                reason_code: None,
                detail: None,
                run_id: None,
            },
        };
        recorder.persist()?;
        prune_attempt_files(&directory, MAX_RETAINED_ATTEMPTS)?;
        Ok(recorder)
    }

    /// Persists the next setup/execution boundary before entering it.
    pub fn advance(&mut self, phase: ScheduledBackupAttemptPhase) -> Result<()> {
        self.attempt.phase = phase;
        self.persist()
    }

    /// Finalizes the attempt with a stable result and optional canonical run id.
    pub fn finish(
        &mut self,
        outcome: ScheduledBackupAttemptOutcome,
        reason_code: Option<&str>,
        detail: Option<&str>,
        run_id: Option<i64>,
    ) -> Result<()> {
        self.attempt.outcome = outcome;
        self.attempt.phase = ScheduledBackupAttemptPhase::Finished;
        self.attempt.finished_at = Some(now_rfc3339());
        self.attempt.reason_code = reason_code.map(str::to_string);
        self.attempt.detail = detail.map(str::to_string);
        self.attempt.run_id = run_id;
        self.persist()?;
        if outcome == ScheduledBackupAttemptOutcome::Success {
            let timestamp = self
                .attempt
                .finished_at
                .as_ref()
                .expect("a finalized attempt always has a finished timestamp");
            atomic_durable_write(
                &self
                    .path
                    .parent()
                    .expect("attempt file always has a ledger directory")
                    .join(LAST_SUCCESS_FILE),
                serde_json::to_string_pretty(timestamp)?.as_bytes(),
            )
            .context("persisting latest scheduled-backup success")?;
        }
        Ok(())
    }

    fn persist(&self) -> Result<()> {
        let payload = serde_json::to_vec_pretty(&self.attempt)?;
        atomic_durable_write(&self.path, &payload)
            .with_context(|| format!("persisting schedule attempt {}", self.attempt.id))
    }
}

/// Loads newest-first bounded attempt evidence and derives execution-health issues.
/// `awake_since` is when the computer last woke or started (`vault_platform::awake_since`).
pub fn load_schedule_attempt_ledger(
    paths: &ProjectPaths,
    check_interval_hours: f64,
    awake_since: Option<DateTime<Utc>>,
) -> Result<ScheduleAttemptLedgerSnapshot> {
    let directory = attempt_directory(paths);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ScheduleAttemptLedgerSnapshot::default());
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!("reading schedule attempt ledger {}", directory.display())
            });
        }
    };

    let detected_at = now_rfc3339();
    let mut attempts = Vec::new();
    let mut health_issues = Vec::new();
    let last_success_at = match fs::read(directory.join(LAST_SUCCESS_FILE)) {
        Ok(bytes) => match serde_json::from_slice::<String>(&bytes) {
            Ok(timestamp) => Some(timestamp),
            Err(error) => {
                health_issues.push(unreadable_issue(
                    &detected_at,
                    format!("parsing latest schedule success: {error}"),
                ));
                None
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            health_issues.push(unreadable_issue(
                &detected_at,
                format!("reading latest schedule success: {error}"),
            ));
            None
        }
    };
    let mut attempt_paths = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("reading entry in {}", directory.display()))?;
        let path = entry.path();
        if is_attempt_path(&path) {
            attempt_paths.push(path);
        }
    }
    attempt_paths.sort();
    attempt_paths.reverse();
    attempt_paths.truncate(MAX_RETAINED_ATTEMPTS);

    for path in attempt_paths {
        match fs::read(&path).with_context(|| format!("reading {}", path.display())).and_then(
            |bytes| {
                serde_json::from_slice::<ScheduledBackupAttempt>(&bytes)
                    .with_context(|| format!("parsing {}", path.display()))
            },
        ) {
            Ok(attempt) => attempts.push(attempt),
            Err(error) => {
                health_issues.push(unreadable_issue(&detected_at, error.to_string()));
            }
        }
    }
    attempts.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    materialize_interrupted_attempts(
        &mut attempts,
        check_interval_hours,
        &detected_at,
        awake_since,
    );

    derive_health_issues(&attempts, &detected_at, &mut health_issues);
    Ok(ScheduleAttemptLedgerSnapshot { attempts, last_success_at, health_issues })
}

fn unreadable_issue(detected_at: &str, evidence: String) -> ScheduledBackupHealthIssue {
    ScheduledBackupHealthIssue {
        code: "attempt-ledger-unreadable".to_string(),
        severity: "error".to_string(),
        detected_at: detected_at.to_string(),
        related_attempt_id: None,
        evidence: vec![evidence],
    }
}

fn derive_health_issues(
    attempts: &[ScheduledBackupAttempt],
    detected_at: &str,
    issues: &mut Vec<ScheduledBackupHealthIssue>,
) {
    let Some(latest) = attempts.first() else {
        return;
    };
    if latest.outcome == ScheduledBackupAttemptOutcome::Partial {
        issues.push(ScheduledBackupHealthIssue {
            code: "latest-attempt-incomplete".into(),
            severity: "warning".into(),
            detected_at: detected_at.into(),
            related_attempt_id: Some(latest.id.clone()),
            evidence: Vec::new(),
        });
    }
    if latest.outcome == ScheduledBackupAttemptOutcome::Failed {
        issues.push(ScheduledBackupHealthIssue {
            code: "latest-attempt-failed".to_string(),
            severity: "error".to_string(),
            detected_at: detected_at.to_string(),
            related_attempt_id: Some(latest.id.clone()),
            evidence: latest.detail.clone().into_iter().collect(),
        });
    }

    if latest.outcome == ScheduledBackupAttemptOutcome::Interrupted {
        issues.push(ScheduledBackupHealthIssue {
            code: "worker-interrupted".to_string(),
            severity: "error".to_string(),
            detected_at: detected_at.to_string(),
            related_attempt_id: Some(latest.id.clone()),
            evidence: vec![latest.started_at.clone()],
        });
    }
}

/// Whether the scheduler has had its chance since the computer last woke or started. Nothing
/// runs while it sleeps, and on waking the scheduler runs what it missed; until it has had time
/// to, neither a silence nor a run the sleep paused means something is wrong. An unknown wake
/// time counts as a chance had, so a schedule that stopped is never hidden for want of one.
fn caught_up(awake_since: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    awake_since
        .is_none_or(|since| now.signed_duration_since(since) > Duration::minutes(CATCH_UP_MINUTES))
}

/// The system has stopped starting scheduled backups: no attempt of any outcome for three of
/// its checks, counted from the newest attempt or from when the schedule was installed,
/// whichever is later. A "not due yet" run counts, since it shows the scheduler calls; failed
/// runs have an issue of their own. `attempts` is newest first, as the ledger returns them.
/// Not reported right after the computer wakes or starts (`awake_since`), before a scheduler
/// that works has caught up.
pub fn overdue_issue(
    attempts: &[ScheduledBackupAttempt],
    installed_at: Option<&str>,
    check_interval_hours: f64,
    awake_since: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<ScheduledBackupHealthIssue> {
    if !caught_up(awake_since, now) {
        return None;
    }
    let parse = |timestamp: &str| {
        DateTime::parse_from_rfc3339(timestamp).ok().map(|value| value.with_timezone(&Utc))
    };
    let since = attempts
        .first()
        .and_then(|attempt| parse(&attempt.started_at))
        .into_iter()
        .chain(installed_at.and_then(parse))
        .max()?;
    let check_minutes = if check_interval_hours.is_finite() && check_interval_hours > 0.0 {
        (check_interval_hours * 60.0).ceil() as i64
    } else {
        60
    };
    (now.signed_duration_since(since) > Duration::minutes(check_minutes.saturating_mul(3))).then(
        || ScheduledBackupHealthIssue {
            code: "scheduled-backup-overdue".to_string(),
            severity: "warning".to_string(),
            detected_at: now.to_rfc3339(),
            related_attempt_id: attempts.first().map(|attempt| attempt.id.clone()),
            evidence: vec![since.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)],
        },
    )
}

fn materialize_interrupted_attempts(
    attempts: &mut [ScheduledBackupAttempt],
    check_interval_hours: f64,
    detected_at: &str,
    awake_since: Option<DateTime<Utc>>,
) {
    let now = DateTime::parse_from_rfc3339(detected_at)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    // A run the computer slept through finishes once it wakes.
    if !caught_up(awake_since, now) {
        return;
    }
    let stale_after = stale_after(check_interval_hours);
    for attempt in attempts {
        if attempt.outcome == ScheduledBackupAttemptOutcome::Running
            && timestamp_is_older_than(&attempt.started_at, now, stale_after)
        {
            attempt.outcome = ScheduledBackupAttemptOutcome::Interrupted;
            attempt.reason_code = Some("interrupted".to_string());
        }
    }
}

fn stale_after(check_interval_hours: f64) -> Duration {
    let cadence_minutes = if check_interval_hours.is_finite() && check_interval_hours > 0.0 {
        (check_interval_hours * 60.0).ceil() as i64
    } else {
        MIN_STALE_RUNNING_MINUTES
    };
    Duration::minutes((cadence_minutes.saturating_mul(2)).max(MIN_STALE_RUNNING_MINUTES))
}

fn timestamp_is_older_than(timestamp: &str, now: DateTime<Utc>, age: Duration) -> bool {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|value| now.signed_duration_since(value.with_timezone(&Utc)) > age)
        .unwrap_or(false)
}

fn attempt_directory(paths: &ProjectPaths) -> PathBuf {
    paths.schedule_dir.join("attempts")
}

fn is_attempt_path(path: &Path) -> bool {
    path.is_file()
        && path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
            name.starts_with(ATTEMPT_FILE_PREFIX) && name.ends_with(ATTEMPT_FILE_SUFFIX)
        })
}

fn prune_attempt_files(directory: &Path, retain: usize) -> Result<()> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("reading schedule attempt ledger {}", directory.display()))?
    {
        let path =
            entry.with_context(|| format!("reading entry in {}", directory.display()))?.path();
        if is_attempt_path(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    let remove_count = paths.len().saturating_sub(retain);
    for path in paths.into_iter().take(remove_count) {
        fs::remove_file(&path)
            .with_context(|| format!("pruning old schedule attempt {}", path.display()))?;
    }
    Ok(())
}

/// A new native invocation can durably close an old `running` attempt that
/// never reached its terminal write. The 15-minute floor avoids racing a
/// genuinely active duplicate/legacy scheduler invocation; older records are
/// materialized as interrupted before publishing the new attempt. Each rewrite
/// remains crash-atomic.
fn finalize_orphaned_running_attempts(directory: &Path) -> Result<()> {
    let finished_at = now_rfc3339();
    let now = DateTime::parse_from_rfc3339(&finished_at)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    for entry in fs::read_dir(directory)
        .with_context(|| format!("reading schedule attempt ledger {}", directory.display()))?
    {
        let path =
            entry.with_context(|| format!("reading entry in {}", directory.display()))?.path();
        if !is_attempt_path(&path) {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            // Status loading turns this into a typed ledger-unreadable issue;
            // one bad historical record must not prevent recording a new tick.
            continue;
        };
        let Ok(mut attempt) = serde_json::from_slice::<ScheduledBackupAttempt>(&bytes) else {
            continue;
        };
        if attempt.outcome != ScheduledBackupAttemptOutcome::Running
            || !timestamp_is_older_than(
                &attempt.started_at,
                now,
                Duration::minutes(MIN_STALE_RUNNING_MINUTES),
            )
        {
            continue;
        }
        attempt.outcome = ScheduledBackupAttemptOutcome::Interrupted;
        attempt.phase = ScheduledBackupAttemptPhase::Finished;
        attempt.finished_at = Some(finished_at.clone());
        attempt.reason_code = Some("interrupted".to_string());
        attempt.detail = Some("The worker exited before recording a terminal outcome.".to_string());
        atomic_durable_write(&path, &serde_json::to_vec_pretty(&attempt)?)
            .with_context(|| format!("finalizing interrupted schedule attempt {}", attempt.id))?;
    }
    Ok(())
}

fn new_attempt_id() -> String {
    let epoch_nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let sequence = ATTEMPT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{epoch_nanos:039}-{:010}-{sequence:020}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_paths_with_root;

    #[test]
    fn recorder_persists_pre_archive_failure_and_success_transitions() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let mut failed = ScheduleAttemptRecorder::begin(&paths).expect("begin");
        failed.advance(ScheduledBackupAttemptPhase::Keyring).expect("advance");
        failed
            .finish(
                ScheduledBackupAttemptOutcome::Failed,
                Some("keyring-read-failed"),
                Some("native secret store unavailable"),
                None,
            )
            .expect("finish");

        let mut success = ScheduleAttemptRecorder::begin(&paths).expect("begin second");
        success
            .finish(
                ScheduledBackupAttemptOutcome::Success,
                Some("backup-succeeded"),
                None,
                Some(42),
            )
            .expect("finish success");

        let snapshot = load_schedule_attempt_ledger(&paths, 1.0, None).expect("load");
        assert_eq!(snapshot.attempts.len(), 2);
        assert_eq!(snapshot.attempts[0].outcome, ScheduledBackupAttemptOutcome::Success);
        assert_eq!(snapshot.attempts[0].run_id, Some(42));
        assert_eq!(snapshot.last_success_at, snapshot.attempts[0].finished_at);
        assert_eq!(snapshot.attempts[1].phase, ScheduledBackupAttemptPhase::Finished);
        assert_eq!(snapshot.attempts[1].reason_code.as_deref(), Some("keyring-read-failed"));
        assert_eq!(snapshot.health_issues.len(), 0);
    }

    #[test]
    fn next_invocation_durably_finalizes_an_orphaned_running_attempt() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let first = ScheduleAttemptRecorder::begin(&paths).expect("first begin");
        let first_path = first.path.clone();
        drop(first);
        let mut orphan: ScheduledBackupAttempt =
            serde_json::from_slice(&fs::read(&first_path).expect("read first"))
                .expect("parse first");
        orphan.started_at = "2020-01-01T00:00:00Z".to_string();
        atomic_durable_write(&first_path, &serde_json::to_vec_pretty(&orphan).expect("serialize"))
            .expect("age running attempt");

        let _second = ScheduleAttemptRecorder::begin(&paths).expect("second begin");
        let persisted: ScheduledBackupAttempt =
            serde_json::from_slice(&fs::read(first_path).expect("read first"))
                .expect("parse first");
        assert_eq!(persisted.outcome, ScheduledBackupAttemptOutcome::Interrupted);
        assert_eq!(persisted.phase, ScheduledBackupAttemptPhase::Finished);
        assert_eq!(persisted.reason_code.as_deref(), Some("interrupted"));
        assert!(persisted.finished_at.is_some());
    }

    // Failure modes right after the computer wakes or starts: a night asleep taken for a
    // scheduler that stopped, before launchd has caught up; a backup the sleep paused taken for
    // one that was interrupted; a scheduler that really stopped, on a laptop that sleeps often,
    // not reported until it has been awake for hours; an unknown wake time hiding a stopped
    // schedule.
    #[test]
    fn right_after_waking_the_scheduler_gets_its_chance() {
        let at =
            |timestamp: &str| DateTime::parse_from_rfc3339(timestamp).unwrap().with_timezone(&Utc);
        let attempt = |started_at: &str, outcome| ScheduledBackupAttempt {
            id: started_at.to_string(),
            started_at: started_at.to_string(),
            finished_at: (outcome != ScheduledBackupAttemptOutcome::Running)
                .then(|| started_at.to_string()),
            outcome,
            phase: ScheduledBackupAttemptPhase::Finished,
            reason_code: None,
            detail: None,
            run_id: None,
        };
        // Hourly checks; the last one ran at ten in the evening, then the laptop slept.
        let last = [attempt("2026-10-09T22:00:00+00:00", ScheduledBackupAttemptOutcome::Success)];
        let woke = Some(at("2026-10-10T08:00:00+00:00"));
        assert!(overdue_issue(&last, None, 1.0, woke, at("2026-10-10T08:05:00+00:00")).is_none());
        // Twenty minutes awake and still nothing: stopped, counted from the last run.
        let issue = overdue_issue(&last, None, 1.0, woke, at("2026-10-10T08:20:00+00:00"))
            .expect("overdue once it has had its chance");
        assert_eq!(issue.evidence, vec!["2026-10-09T22:00:00+00:00"]);
        // Without a wake time, as before.
        assert!(overdue_issue(&last, None, 1.0, None, at("2026-10-10T08:05:00+00:00")).is_some());

        // A backup running when the laptop went to sleep is still running when it wakes.
        let paused =
            || vec![attempt("2026-10-09T23:30:00+00:00", ScheduledBackupAttemptOutcome::Running)];
        let mut attempts = paused();
        materialize_interrupted_attempts(&mut attempts, 1.0, "2026-10-10T08:05:00+00:00", woke);
        assert_eq!(attempts[0].outcome, ScheduledBackupAttemptOutcome::Running);
        let mut issues = Vec::new();
        derive_health_issues(&attempts, "2026-10-10T08:05:00+00:00", &mut issues);
        assert!(issues.is_empty());
        // One that has not finished once the scheduler has had its chance was interrupted.
        let mut attempts = paused();
        materialize_interrupted_attempts(&mut attempts, 1.0, "2026-10-10T08:20:00+00:00", woke);
        assert_eq!(attempts[0].outcome, ScheduledBackupAttemptOutcome::Interrupted);
    }

    #[cfg(unix)]
    #[test]
    fn orphan_finalization_skips_noise_corrupt_and_unreadable_records() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let directory = attempt_directory(&paths);
        fs::create_dir_all(&directory).expect("attempt directory");
        fs::write(directory.join("noise.txt"), b"noise").expect("noise record");
        fs::write(
            directory.join(format!("{ATTEMPT_FILE_PREFIX}corrupt{ATTEMPT_FILE_SUFFIX}")),
            b"{not-json",
        )
        .expect("corrupt attempt");
        let unreadable_path =
            directory.join(format!("{ATTEMPT_FILE_PREFIX}unreadable{ATTEMPT_FILE_SUFFIX}"));
        fs::write(&unreadable_path, b"{}").expect("unreadable attempt");
        fs::set_permissions(&unreadable_path, fs::Permissions::from_mode(0o000))
            .expect("remove read permission");

        let result = ScheduleAttemptRecorder::begin(&paths);

        fs::set_permissions(&unreadable_path, fs::Permissions::from_mode(0o600))
            .expect("restore read permission");
        result.expect("bad historical records do not block a new attempt");
    }
}
