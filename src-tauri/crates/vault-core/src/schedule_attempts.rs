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
pub fn load_schedule_attempt_ledger(
    paths: &ProjectPaths,
    check_interval_hours: f64,
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
    materialize_interrupted_attempts(&mut attempts, check_interval_hours, &detected_at);

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

fn materialize_interrupted_attempts(
    attempts: &mut [ScheduledBackupAttempt],
    check_interval_hours: f64,
    detected_at: &str,
) {
    let now = DateTime::parse_from_rfc3339(detected_at)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
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

        let snapshot = load_schedule_attempt_ledger(&paths, 1.0).expect("load");
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

    #[test]
    fn next_invocation_does_not_interrupt_a_recently_started_worker() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let first = ScheduleAttemptRecorder::begin(&paths).expect("first begin");
        let first_path = first.path.clone();
        drop(first);

        let _second = ScheduleAttemptRecorder::begin(&paths).expect("second begin");
        let persisted: ScheduledBackupAttempt =
            serde_json::from_slice(&fs::read(first_path).expect("read first"))
                .expect("parse first");
        assert_eq!(persisted.outcome, ScheduledBackupAttemptOutcome::Running);
        assert!(persisted.finished_at.is_none());
    }

    #[test]
    fn ledger_is_bounded_and_one_malformed_record_does_not_hide_valid_evidence() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        for _ in 0..(MAX_RETAINED_ATTEMPTS + 3) {
            ScheduleAttemptRecorder::begin(&paths).expect("begin");
        }
        let directory = attempt_directory(&paths);
        fs::write(
            directory.join(format!("{ATTEMPT_FILE_PREFIX}999999999999999999999999999999999999999-bad{ATTEMPT_FILE_SUFFIX}")),
            b"{not-json",
        )
        .expect("malformed record");

        let snapshot = load_schedule_attempt_ledger(&paths, 1.0).expect("load");
        assert!(snapshot.attempts.len() <= MAX_RETAINED_ATTEMPTS);
        assert!(
            snapshot.health_issues.iter().any(|issue| issue.code == "attempt-ledger-unreadable")
        );
        assert!(
            fs::read_dir(directory)
                .expect("read dir")
                .filter_map(Result::ok)
                .filter(|entry| is_attempt_path(&entry.path()))
                .count()
                <= MAX_RETAINED_ATTEMPTS + 1
        );
    }

    #[test]
    fn health_derivation_flags_failed_and_stale_or_interrupted_workers() {
        let detected_at = "2026-07-23T12:00:00+00:00";
        let failed = ScheduledBackupAttempt {
            id: "failed".to_string(),
            started_at: "2026-07-23T11:59:00+00:00".to_string(),
            finished_at: Some("2026-07-23T11:59:01+00:00".to_string()),
            outcome: ScheduledBackupAttemptOutcome::Failed,
            phase: ScheduledBackupAttemptPhase::Finished,
            reason_code: Some("config-load-failed".to_string()),
            detail: Some("bad config".to_string()),
            run_id: None,
        };
        let mut issues = Vec::new();
        derive_health_issues(&[failed], detected_at, &mut issues);
        assert_eq!(issues[0].code, "latest-attempt-failed");
        assert_eq!(issues[0].evidence, vec!["bad config"]);

        let running = ScheduledBackupAttempt {
            id: "running".to_string(),
            started_at: "2026-07-23T08:00:00+00:00".to_string(),
            finished_at: None,
            outcome: ScheduledBackupAttemptOutcome::Running,
            phase: ScheduledBackupAttemptPhase::Backup,
            reason_code: None,
            detail: None,
            run_id: None,
        };
        let mut attempts = vec![running];
        materialize_interrupted_attempts(&mut attempts, 1.0, detected_at);
        assert_eq!(attempts[0].outcome, ScheduledBackupAttemptOutcome::Interrupted);
        assert_eq!(attempts[0].reason_code.as_deref(), Some("interrupted"));
        let mut issues = Vec::new();
        derive_health_issues(&attempts, detected_at, &mut issues);
        assert!(issues.iter().any(|issue| issue.code == "worker-interrupted"));
    }

    #[test]
    fn empty_health_and_invalid_cadence_have_safe_defaults() {
        let mut issues = Vec::new();
        derive_health_issues(&[], "2026-07-23T12:00:00+00:00", &mut issues);
        assert!(issues.is_empty());
        assert_eq!(stale_after(f64::NAN), Duration::minutes(MIN_STALE_RUNNING_MINUTES * 2));
    }

    #[test]
    fn malformed_or_unreadable_last_success_is_a_typed_health_issue() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let directory = attempt_directory(&paths);
        fs::create_dir_all(&directory).expect("attempt directory");
        let last_success = directory.join(LAST_SUCCESS_FILE);

        fs::write(&last_success, b"{not-json").expect("malformed last success");
        let malformed = load_schedule_attempt_ledger(&paths, 1.0).expect("load malformed marker");
        assert!(malformed.last_success_at.is_none());
        assert!(
            malformed.health_issues.iter().any(|issue| issue.code == "attempt-ledger-unreadable")
        );

        fs::remove_file(&last_success).expect("remove malformed marker");
        fs::create_dir(&last_success).expect("replace marker with unreadable directory");
        let unreadable = load_schedule_attempt_ledger(&paths, 1.0).expect("load unreadable marker");
        assert!(unreadable.last_success_at.is_none());
        assert!(
            unreadable.health_issues.iter().any(|issue| issue.code == "attempt-ledger-unreadable")
        );
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

    #[test]
    fn invalid_or_missing_ledger_is_safe() {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        assert!(
            load_schedule_attempt_ledger(&paths, f64::NAN)
                .expect("missing ledger")
                .attempts
                .is_empty()
        );
        fs::create_dir_all(&paths.schedule_dir).expect("schedule dir");
        fs::write(paths.schedule_dir.join("attempts"), b"not-a-directory").expect("block dir");
        let error = load_schedule_attempt_ledger(&paths, 1.0).expect_err("invalid ledger");
        assert!(error.to_string().contains("reading schedule attempt ledger"));
    }
}
