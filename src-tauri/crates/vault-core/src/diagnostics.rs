//! Crash-report and runtime-diagnostics helpers.
//!
//! Diagnostics in PathKeep are local-first artifacts. This module records
//! frontend and Rust failures into structured files and assembles the shell's
//! runtime diagnostics snapshot from those local reports.

use crate::{
    config::{ProjectPaths, ensure_paths},
    models::{CrashReportSummary, FrontendErrorReportRequest, RuntimeDiagnostics},
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{backtrace::Backtrace, fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// On-disk crash report payload used for frontend and Rust failures.
struct StoredCrashReport {
    source: String,
    recorded_at: String,
    fatal: bool,
    message: String,
    location: Option<String>,
    path: String,
    stack: Option<String>,
    thread: Option<String>,
    url: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
}

/// Loads runtime diagnostics paths plus the latest persisted crash report.
pub fn load_runtime_diagnostics(paths: &ProjectPaths) -> Result<RuntimeDiagnostics> {
    ensure_paths(paths)?;
    Ok(RuntimeDiagnostics {
        log_directory: paths.logs_dir.display().to_string(),
        rust_log_path: paths.rust_log_path.display().to_string(),
        frontend_log_path: paths.frontend_log_path.display().to_string(),
        crash_reports_directory: paths.crash_reports_dir.display().to_string(),
        latest_crash_report: latest_crash_report(paths)?,
    })
}

/// Records one frontend error report and returns its shell-facing summary.
pub fn record_frontend_error(
    paths: &ProjectPaths,
    request: &FrontendErrorReportRequest,
) -> Result<CrashReportSummary> {
    ensure_paths(paths)?;
    let report = StoredCrashReport {
        source: request.source.clone(),
        recorded_at: now_rfc3339(),
        fatal: request.fatal,
        message: request.message.clone(),
        location: frontend_location(request),
        path: paths.frontend_error_report_path.display().to_string(),
        stack: request.stack.clone(),
        thread: None,
        url: request.url.clone(),
        line: request.line,
        column: request.column,
    };
    write_report(&paths.frontend_error_report_path, &report)?;
    Ok(to_summary(report))
}

/// Records one Rust panic into the local crash-report area.
pub fn record_rust_panic(
    paths: &ProjectPaths,
    panic_info: &std::panic::PanicHookInfo<'_>,
) -> Result<CrashReportSummary> {
    ensure_paths(paths)?;
    let payload = panic_payload(panic_info);
    let location = panic_info
        .location()
        .map(|location| format!("{}:{}:{}", location.file(), location.line(), location.column()));
    let report = StoredCrashReport {
        source: "rust-panic".to_string(),
        recorded_at: now_rfc3339(),
        fatal: true,
        message: payload,
        location,
        path: paths.rust_panic_report_path.display().to_string(),
        stack: Some(Backtrace::force_capture().to_string()),
        thread: std::thread::current()
            .name()
            .map(ToString::to_string)
            .or_else(|| Some("unnamed".to_string())),
        url: None,
        line: None,
        column: None,
    };
    write_report(&paths.rust_panic_report_path, &report)?;
    Ok(to_summary(report))
}

fn latest_crash_report(paths: &ProjectPaths) -> Result<Option<CrashReportSummary>> {
    let rust_report = read_report(&paths.rust_panic_report_path)?;
    let frontend_report = read_report(&paths.frontend_error_report_path)?;
    Ok(match (rust_report, frontend_report) {
        (Some(left), Some(right)) => {
            if left.recorded_at >= right.recorded_at {
                Some(to_summary(left))
            } else {
                Some(to_summary(right))
            }
        }
        (Some(report), None) | (None, Some(report)) => Some(to_summary(report)),
        (None, None) => None,
    })
}

fn read_report(path: &Path) -> Result<Option<StoredCrashReport>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };
    let report = match serde_json::from_str::<StoredCrashReport>(&content) {
        Ok(report) => report,
        Err(_) => return Ok(None),
    };
    Ok(Some(report))
}

fn write_report(path: &Path, report: &StoredCrashReport) -> Result<()> {
    let content = serde_json::to_string_pretty(report)?;
    let suffix = now_rfc3339().replace([':', '.'], "-");
    let temp_path = path.with_extension(format!(
        "{}.{suffix}.tmp",
        path.extension().and_then(|value| value.to_str()).unwrap_or("json")
    ));
    fs::write(&temp_path, content)
        .with_context(|| format!("writing crash report {}", temp_path.display()))?;
    if let Err(error) = fs::rename(&temp_path, path) {
        recover_report_rename(path, &temp_path, error)?;
    }
    Ok(())
}

fn recover_report_rename(path: &Path, temp_path: &Path, error: std::io::Error) -> Result<()> {
    if path.exists() {
        let _ = fs::remove_file(path);
        fs::rename(temp_path, path)
            .with_context(|| format!("replacing crash report {}", path.display()))?;
    } else {
        return Err(error).with_context(|| format!("writing crash report {}", path.display()));
    }
    Ok(())
}

fn to_summary(report: StoredCrashReport) -> CrashReportSummary {
    CrashReportSummary {
        source: report.source,
        recorded_at: report.recorded_at,
        fatal: report.fatal,
        message: report.message,
        location: report.location,
        path: report.path,
    }
}

fn frontend_location(request: &FrontendErrorReportRequest) -> Option<String> {
    match (&request.url, request.line, request.column) {
        (Some(url), Some(line), Some(column)) => Some(format!("{url}:{line}:{column}")),
        (Some(url), Some(line), None) => Some(format!("{url}:{line}")),
        (Some(url), None, None) => Some(url.clone()),
        _ => None,
    }
}

fn panic_payload(panic_info: &std::panic::PanicHookInfo<'_>) -> String {
    if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = panic_info.payload().downcast_ref::<String>() {
        return message.clone();
    }
    "Rust panic with non-string payload".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, PanicHookInfo, UnwindSafe},
        sync::{
            Arc, Mutex, OnceLock,
            atomic::{AtomicBool, Ordering},
        },
    };
    use tempfile::tempdir;

    fn sample_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
            source_evidence_database_path: root.join("archive/source-evidence.sqlite"),
            derived_dir: root.join("derived"),
            search_database_path: root.join("derived/history-search.sqlite"),
            intelligence_database_path: root.join("derived/history-intelligence.sqlite"),
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            sidecars_dir: root.join("sidecars"),
            semantic_index_dir: root.join("sidecars/semantic-index"),
            intelligence_blobs_dir: root.join("sidecars/intelligence-blobs"),
            vectors_dir: root.join("derived/vectors"),
            agent_database_path: root.join("derived/agent.sqlite"),
            models_dir: root.join("models"),
            logs_dir: root.join("logs"),
            rust_log_path: root.join("logs/rust.log"),
            frontend_log_path: root.join("logs/frontend.log"),
            crash_reports_dir: root.join("diagnostics/crash-reports"),
            rust_panic_report_path: root.join("diagnostics/crash-reports/rust-panic-latest.json"),
            frontend_error_report_path: root
                .join("diagnostics/crash-reports/frontend-error-latest.json"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
    }

    fn stored_report(
        source: &str,
        recorded_at: &str,
        message: &str,
        path: &Path,
    ) -> StoredCrashReport {
        StoredCrashReport {
            source: source.to_string(),
            recorded_at: recorded_at.to_string(),
            fatal: true,
            message: message.to_string(),
            location: None,
            path: path.display().to_string(),
            stack: None,
            thread: None,
            url: None,
            line: None,
            column: None,
        }
    }

    /// Process-wide serialization for the diagnostics tests that install a GLOBAL panic hook.
    ///
    /// The panic hook is a single process-global slot: only one custom hook can be installed at a
    /// time. This mutex keeps the two hook-installing helpers below from clobbering each other's
    /// install/restore. It does NOT — and cannot — stop the rest of the suite (fault_inject /
    /// migration / restore crash tests) from PANICKING on their own threads while a hook is
    /// installed; that cross-test contamination is fenced off by [`PANIC_CAPTURE_ACTIVE`] instead.
    fn panic_hook_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    thread_local! {
        /// Opt-in flag: `true` ONLY on the thread whose panic a `capture_*` helper intends to
        /// record. The installed hook consults it (see [`install_selective_panic_hook`]) so a
        /// CONCURRENT panic raised by any OTHER test thread — the `arm_panic_at` migration/restore
        /// crash tests, the `fault_inject` panic tests — is FORWARDED to the previous hook instead of
        /// being mis-captured into this helper's slot/file. That mis-capture was the root cause of the
        /// historical `rust_panic_payloads_keep_owned_strings_and_fallback_text` flake (~3/8 full-suite
        /// runs): a foreign panic overwrote the captured summary/report before the helper read it back.
        static PANIC_CAPTURE_ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    /// Marks the current thread as the intended capturer for the duration of a scope, clearing the
    /// flag on drop even if the guarded body panics/unwinds — so a leaked `true` can never make a
    /// LATER test's panic on this reused worker thread get captured.
    struct CaptureScope;

    impl CaptureScope {
        fn enter() -> Self {
            PANIC_CAPTURE_ACTIVE.with(|active| active.set(true));
            CaptureScope
        }
    }

    impl Drop for CaptureScope {
        fn drop(&mut self) {
            PANIC_CAPTURE_ACTIVE.with(|active| active.set(false));
        }
    }

    /// Installs a GLOBAL panic hook that records ONLY panics raised on a thread that opted in via
    /// [`PANIC_CAPTURE_ACTIVE`]; every other thread's panic is forwarded to the `previous` hook. The
    /// caller holds [`panic_hook_lock`] for the whole install→panic→restore window, so at most one
    /// such hook is ever installed. Returns the shared slot the intended panic is recorded into.
    ///
    /// Restore with [`restore_default_panic_hook`]: because the lock guarantees no OTHER custom hook
    /// coexists, the hook this replaces is always the std default, so `take_hook` restores it exactly.
    fn install_selective_panic_hook(
        hook_paths: ProjectPaths,
    ) -> Arc<Mutex<Option<CrashReportSummary>>> {
        let captured = Arc::new(Mutex::new(None));
        let captured_for_hook = Arc::clone(&captured);
        let previous: Arc<dyn Fn(&PanicHookInfo<'_>) + Sync + Send> =
            Arc::from(std::panic::take_hook());
        std::panic::set_hook(Box::new(move |info| {
            if PANIC_CAPTURE_ACTIVE.with(Cell::get) {
                let summary = record_rust_panic(&hook_paths, info).expect("record rust panic");
                *captured_for_hook.lock().expect("captured panic summary") = Some(summary);
            } else {
                // A foreign test's panic: forward to the previous hook (normal reporting) — never
                // touch this helper's capture slot / report file. This is the isolation that kills
                // the cross-test flake.
                previous(info);
            }
        }));
        captured
    }

    /// Drops the currently-installed custom hook and resets to the std default. Correct because the
    /// [`panic_hook_lock`] guarantees the replaced hook was always the std default.
    fn restore_default_panic_hook() {
        let _ = std::panic::take_hook();
    }

    fn capture_panic_summary<F>(paths: &ProjectPaths, panic_fn: F) -> CrashReportSummary
    where
        F: FnOnce() + UnwindSafe,
    {
        let _guard = panic_hook_lock().lock().expect("panic hook lock");
        let captured = install_selective_panic_hook(paths.clone());

        // `panic_fn` runs on THIS thread; opt this thread in so the hook captures its panic while
        // forwarding any concurrent foreign panic. The scope guard clears the flag on the way out.
        let panic_result = {
            let _capture = CaptureScope::enter();
            std::panic::catch_unwind(panic_fn)
        };
        restore_default_panic_hook();

        assert!(panic_result.is_err());
        captured
            .lock()
            .expect("captured panic summary")
            .take()
            .expect("panic hook recorded summary")
    }

    fn capture_unnamed_thread_panic_report(paths: &ProjectPaths) -> StoredCrashReport {
        let _guard = panic_hook_lock().lock().expect("panic hook lock");
        let _captured = install_selective_panic_hook(paths.clone());

        // The panic happens on a SPAWNED thread, so that thread (not this one) must opt in before it
        // panics. The hook runs on the panicking thread, sees its flag, and records to `hook_paths`.
        let panic_result = std::thread::spawn(|| {
            let _capture = CaptureScope::enter();
            panic!("unnamed thread panic fixture");
        })
        .join();
        restore_default_panic_hook();

        assert!(panic_result.is_err());
        // Read the on-disk report the (flagged) hook wrote for THIS panic; foreign panics were
        // forwarded and never wrote here, so the read is race-free.
        read_report(&paths.rust_panic_report_path)
            .expect("read rust panic report")
            .expect("stored rust panic report")
    }

    #[test]
    fn diagnostics_report_paths_even_before_crashes() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());

        let diagnostics = load_runtime_diagnostics(&paths).expect("diagnostics");
        assert_eq!(diagnostics.log_directory, paths.logs_dir.display().to_string());
        assert!(diagnostics.latest_crash_report.is_none());
        assert!(paths.logs_dir.exists());
        assert!(paths.crash_reports_dir.exists());
    }

    #[test]
    fn frontend_location_formats_partial_source_positions() {
        let mut request = FrontendErrorReportRequest {
            source: "window-error".to_string(),
            message: "boom".to_string(),
            stack: None,
            url: Some("app://main".to_string()),
            line: Some(12),
            column: Some(4),
            fatal: false,
        };
        assert_eq!(frontend_location(&request).as_deref(), Some("app://main:12:4"));

        request.column = None;
        assert_eq!(frontend_location(&request).as_deref(), Some("app://main:12"));

        request.line = None;
        assert_eq!(frontend_location(&request).as_deref(), Some("app://main"));

        request.url = None;
        assert!(frontend_location(&request).is_none());
    }

    #[test]
    fn frontend_report_roundtrips_and_becomes_latest_crash() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());

        let summary = record_frontend_error(
            &paths,
            &FrontendErrorReportRequest {
                source: "unhandledrejection".to_string(),
                message: "boom".to_string(),
                stack: Some("stack".to_string()),
                url: Some("app://main".to_string()),
                line: Some(10),
                column: Some(3),
                fatal: true,
            },
        )
        .expect("record frontend error");
        assert_eq!(summary.source, "unhandledrejection");
        assert!(paths.frontend_error_report_path.exists());

        let diagnostics = load_runtime_diagnostics(&paths).expect("diagnostics");
        let latest = diagnostics.latest_crash_report.expect("latest crash");
        assert_eq!(latest.message, "boom");
        assert!(latest.location.expect("location").contains("app://main:10:3"));
    }

    #[test]
    fn record_frontend_error_reports_unwritable_existing_report_path() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        fs::create_dir_all(&paths.frontend_error_report_path).expect("directory at report path");

        let error = record_frontend_error(
            &paths,
            &FrontendErrorReportRequest {
                source: "window-error".to_string(),
                message: "boom".to_string(),
                stack: None,
                url: None,
                line: None,
                column: None,
                fatal: true,
            },
        )
        .expect_err("directory report path is not writable as a report file");

        let message = format!("{error:#}");
        assert!(message.contains("replacing crash report"));
    }

    #[test]
    fn recover_report_rename_replaces_existing_report_file() {
        let dir = tempdir().expect("tempdir");
        let report_path = dir.path().join("report.json");
        let temp_path = dir.path().join("report.tmp");
        fs::write(&report_path, "old-report").expect("write old report");
        fs::write(&temp_path, "new-report").expect("write temp report");

        recover_report_rename(&report_path, &temp_path, std::io::Error::other("rename failed"))
            .expect("recover rename");

        assert_eq!(fs::read_to_string(&report_path).expect("read replaced report"), "new-report");
        assert!(!temp_path.exists());
    }

    #[test]
    fn recover_report_rename_reports_missing_target_write_failure() {
        let dir = tempdir().expect("tempdir");
        let report_path = dir.path().join("missing").join("report.json");
        let temp_path = dir.path().join("report.tmp");
        fs::write(&temp_path, "new-report").expect("write temp report");

        let error =
            recover_report_rename(&report_path, &temp_path, std::io::Error::other("rename failed"))
                .expect_err("missing target cannot be recovered by replacement");

        let message = format!("{error:#}");
        assert!(message.contains("writing crash report"));
        assert!(temp_path.exists());
    }

    #[test]
    fn rust_panic_reports_capture_payload_location_and_thread() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());

        let summary = capture_panic_summary(
            &paths,
            AssertUnwindSafe(|| {
                panic!("diagnostic panic fixture");
            }),
        );

        assert_eq!(summary.source, "rust-panic");
        assert!(summary.fatal);
        assert_eq!(summary.message, "diagnostic panic fixture");
        assert!(summary.location.as_deref().expect("panic location").contains("diagnostics.rs"));
        assert!(paths.rust_panic_report_path.exists());
    }

    #[test]
    fn rust_panic_payloads_keep_owned_strings_and_fallback_text() {
        let string_dir = tempdir().expect("tempdir");
        let string_paths = sample_paths(string_dir.path());
        let string_summary = capture_panic_summary(
            &string_paths,
            AssertUnwindSafe(|| {
                std::panic::panic_any(String::from("owned diagnostic panic"));
            }),
        );
        assert_eq!(string_summary.message, "owned diagnostic panic");

        let fallback_dir = tempdir().expect("tempdir");
        let fallback_paths = sample_paths(fallback_dir.path());
        let fallback_summary = capture_panic_summary(
            &fallback_paths,
            AssertUnwindSafe(|| {
                std::panic::panic_any(42_u8);
            }),
        );
        assert_eq!(fallback_summary.message, "Rust panic with non-string payload");
    }

    #[test]
    fn rust_panic_report_uses_unnamed_thread_fallback() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());

        let report = capture_unnamed_thread_panic_report(&paths);

        assert_eq!(report.message, "unnamed thread panic fixture");
        assert_eq!(report.thread.as_deref(), Some("unnamed"));
    }

    #[test]
    fn selective_panic_hook_forwards_untagged_thread_panics_to_the_previous_hook() {
        // The isolation contract that KILLS the historical flake: while a `capture_*` helper's hook is
        // installed, a panic on a thread that did NOT opt in (every OTHER test's panic-injecting
        // thread) must be FORWARDED to the previous hook and must NOT be recorded into this helper's
        // slot. Proven deterministically with a sentinel "previous" hook + an un-opted-in panicking
        // thread — no reliance on the real concurrent tests actually racing.
        let _guard = panic_hook_lock().lock().expect("panic hook lock");
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());

        // A sentinel "previous" hook whose invocation we can observe.
        let original = std::panic::take_hook();
        let forwarded = Arc::new(AtomicBool::new(false));
        let forwarded_for_hook = Arc::clone(&forwarded);
        std::panic::set_hook(Box::new(move |_info| {
            forwarded_for_hook.store(true, Ordering::SeqCst);
        }));

        // Install the selective hook ON TOP; it captures the sentinel as its `previous`.
        let captured = install_selective_panic_hook(paths.clone());

        // A panic on a thread that never opts in (PANIC_CAPTURE_ACTIVE stays false there).
        let outcome = std::thread::spawn(|| {
            panic!("untagged foreign-thread panic");
        })
        .join();
        assert!(outcome.is_err(), "the spawned thread must have panicked");

        restore_default_panic_hook();
        std::panic::set_hook(original);

        assert!(
            forwarded.load(Ordering::SeqCst),
            "an untagged thread's panic must be forwarded to the previous hook",
        );
        assert!(
            captured.lock().expect("captured slot").is_none(),
            "an untagged thread's panic must NOT be captured into this helper's slot",
        );
        // The forwarded panic must never have written this helper's report file.
        assert!(
            !paths.rust_panic_report_path.exists(),
            "a forwarded foreign panic must not write the capturing helper's report file",
        );
    }

    #[test]
    fn latest_crash_prefers_most_recent_report() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        write_report(
            &paths.rust_panic_report_path,
            &stored_report(
                "rust-panic",
                "2026-04-10T00:00:00Z",
                "panic",
                &paths.rust_panic_report_path,
            ),
        )
        .expect("write rust report");
        write_report(
            &paths.frontend_error_report_path,
            &stored_report(
                "window-error",
                "2026-04-10T01:00:00Z",
                "frontend",
                &paths.frontend_error_report_path,
            ),
        )
        .expect("write frontend report");

        let diagnostics = load_runtime_diagnostics(&paths).expect("diagnostics");
        assert_eq!(diagnostics.latest_crash_report.expect("latest crash").source, "window-error");
    }

    #[test]
    fn latest_crash_prefers_rust_report_when_timestamps_tie() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        write_report(
            &paths.rust_panic_report_path,
            &stored_report(
                "rust-panic",
                "2026-04-10T01:00:00Z",
                "panic",
                &paths.rust_panic_report_path,
            ),
        )
        .expect("write rust report");
        write_report(
            &paths.frontend_error_report_path,
            &stored_report(
                "window-error",
                "2026-04-10T01:00:00Z",
                "frontend",
                &paths.frontend_error_report_path,
            ),
        )
        .expect("write frontend report");

        let diagnostics = load_runtime_diagnostics(&paths).expect("diagnostics");
        assert_eq!(diagnostics.latest_crash_report.expect("latest crash").source, "rust-panic");
    }

    #[test]
    fn diagnostics_ignore_malformed_crash_reports() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        fs::write(&paths.frontend_error_report_path, "{not-json")
            .expect("write malformed crash report");

        let diagnostics = load_runtime_diagnostics(&paths).expect("diagnostics");
        assert!(diagnostics.latest_crash_report.is_none());
    }

    #[test]
    fn diagnostics_ignore_unreadable_crash_report_paths() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");
        fs::create_dir_all(&paths.frontend_error_report_path).expect("directory report path");

        let diagnostics = load_runtime_diagnostics(&paths).expect("diagnostics");

        assert!(diagnostics.latest_crash_report.is_none());
    }
}
