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
