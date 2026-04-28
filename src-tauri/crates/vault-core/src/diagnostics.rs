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
        panic::{AssertUnwindSafe, UnwindSafe},
        sync::{Arc, Mutex, OnceLock},
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

    fn panic_hook_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn capture_panic_summary<F>(paths: &ProjectPaths, panic_fn: F) -> CrashReportSummary
    where
        F: FnOnce() + UnwindSafe,
    {
        let _guard = panic_hook_lock().lock().expect("panic hook lock");
        let captured = Arc::new(Mutex::new(None));
        let hook_paths = paths.clone();
        let captured_for_hook = Arc::clone(&captured);
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let summary = record_rust_panic(&hook_paths, info).expect("record rust panic");
            *captured_for_hook.lock().expect("captured panic summary") = Some(summary);
        }));

        let panic_result = std::panic::catch_unwind(panic_fn);
        std::panic::set_hook(default_hook);

        assert!(panic_result.is_err());
        captured
            .lock()
            .expect("captured panic summary")
            .take()
            .expect("panic hook recorded summary")
    }

    fn capture_unnamed_thread_panic_report(paths: &ProjectPaths) -> StoredCrashReport {
        let _guard = panic_hook_lock().lock().expect("panic hook lock");
        let captured = Arc::new(Mutex::new(None));
        let hook_paths = paths.clone();
        let captured_for_hook = Arc::clone(&captured);
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            record_rust_panic(&hook_paths, info).expect("record rust panic");
            let report = read_report(&hook_paths.rust_panic_report_path)
                .expect("read rust panic report")
                .expect("stored rust panic report");
            *captured_for_hook.lock().expect("captured panic report") = Some(report);
        }));

        let panic_result = std::thread::spawn(|| {
            panic!("unnamed thread panic fixture");
        })
        .join();
        std::panic::set_hook(default_hook);

        assert!(panic_result.is_err());
        captured.lock().expect("captured panic report").take().expect("panic hook recorded report")
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
