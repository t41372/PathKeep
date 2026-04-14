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
        if path.exists() {
            let _ = fs::remove_file(path);
            fs::rename(&temp_path, path)
                .with_context(|| format!("replacing crash report {}", path.display()))?;
        } else {
            return Err(error).with_context(|| format!("writing crash report {}", path.display()));
        }
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
    use tempfile::tempdir;

    fn sample_paths(root: &Path) -> ProjectPaths {
        ProjectPaths {
            app_root: root.to_path_buf(),
            config_path: root.join("config.json"),
            archive_database_path: root.join("archive/history-vault.sqlite"),
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
    fn latest_crash_prefers_most_recent_report() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        ensure_paths(&paths).expect("ensure paths");

        write_report(
            &paths.rust_panic_report_path,
            &StoredCrashReport {
                source: "rust-panic".to_string(),
                recorded_at: "2026-04-10T00:00:00Z".to_string(),
                fatal: true,
                message: "panic".to_string(),
                location: None,
                path: paths.rust_panic_report_path.display().to_string(),
                stack: None,
                thread: None,
                url: None,
                line: None,
                column: None,
            },
        )
        .expect("write rust report");
        write_report(
            &paths.frontend_error_report_path,
            &StoredCrashReport {
                source: "window-error".to_string(),
                recorded_at: "2026-04-10T01:00:00Z".to_string(),
                fatal: true,
                message: "frontend".to_string(),
                location: None,
                path: paths.frontend_error_report_path.display().to_string(),
                stack: None,
                thread: None,
                url: None,
                line: None,
                column: None,
            },
        )
        .expect("write frontend report");

        let diagnostics = load_runtime_diagnostics(&paths).expect("diagnostics");
        assert_eq!(diagnostics.latest_crash_report.expect("latest crash").source, "window-error");
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
}
