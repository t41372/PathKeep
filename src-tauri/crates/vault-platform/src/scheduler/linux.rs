//! Linux systemd schedule preview owner.
//!
//! ## Responsibilities
//! - Generate manual-review systemd user service/timer artifacts.
//! - Keep Linux schedule setup explicit because v1 does not auto-install timers.
//!
//! ## Not responsible for
//! - Running `systemctl`.
//! - Detecting installed Linux timers.
//!
//! ## Dependencies
//! - `ScheduleParameters` from the scheduler facade.
//! - `vault_core` schedule DTOs for generated artifacts and typed manual steps.
//!
//! ## Performance notes
//! - Pure string generation only; no filesystem or process work.

use anyhow::Result;
use std::path::Path;
use vault_core::models::{GeneratedFile, ScheduleManualStep, SchedulePlan};

use super::ScheduleParameters;

pub(super) fn linux_schedule_plan(
    label: &str,
    executable_path: &Path,
    worker_args: &[String],
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let service = format!(
        "[Unit]\nDescription=PathKeep backup worker\n\n[Service]\nType=oneshot\nExecStart={} {}\n",
        executable_path.display(),
        worker_args[1..].join(" ")
    );
    let timer = format!(
        "[Unit]\nDescription=PathKeep periodic backup\n\n[Timer]\nOnCalendar={}\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
        linux_on_calendar(params.check_interval_hours)
    );
    Ok(SchedulePlan {
        platform: "linux".to_string(),
        label: label.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![
            GeneratedFile {
                relative_path: format!("systemd/{label}.service"),
                absolute_path: None,
                purpose: "User service entry for the worker mode".to_string(),
                contents: service.clone(),
            },
            GeneratedFile {
                relative_path: format!("systemd/{label}.timer"),
                absolute_path: None,
                purpose: format!(
                    "Persistent user timer that wakes every {} hours.",
                    params.check_interval_hours
                ),
                contents: timer.clone(),
            },
        ],
        manual_steps: vec![
            "Copy the files to ~/.config/systemd/user/.".to_string(),
            "Run `systemctl --user daemon-reload`.".to_string(),
            format!("Run `systemctl --user enable --now {label}.timer`."),
            format!(
                "Run `systemctl --user list-timers {label}.timer` to verify the next scheduled run."
            ),
        ],
        manual_step_details: vec![
            ScheduleManualStep {
                id: "linux-copy-service".to_string(),
                title_key: "schedule.manualLinuxCopyServiceTitle".to_string(),
                summary_key: "schedule.manualLinuxCopyServiceSummary".to_string(),
                why_key: "schedule.manualLinuxCopyServiceWhy".to_string(),
                command: None,
                file_path: Some(format!("~/.config/systemd/user/{label}.service")),
                file_contents: Some(service),
                directory_path: Some("~/.config/systemd/user".to_string()),
                can_auto_run: false,
                can_verify: true,
            },
            ScheduleManualStep {
                id: "linux-copy-timer".to_string(),
                title_key: "schedule.manualLinuxCopyTimerTitle".to_string(),
                summary_key: "schedule.manualLinuxCopyTimerSummary".to_string(),
                why_key: "schedule.manualLinuxCopyTimerWhy".to_string(),
                command: None,
                file_path: Some(format!("~/.config/systemd/user/{label}.timer")),
                file_contents: Some(timer),
                directory_path: Some("~/.config/systemd/user".to_string()),
                can_auto_run: false,
                can_verify: true,
            },
            ScheduleManualStep {
                id: "linux-enable-timer".to_string(),
                title_key: "schedule.manualLinuxEnableTitle".to_string(),
                summary_key: "schedule.manualLinuxEnableSummary".to_string(),
                why_key: "schedule.manualLinuxEnableWhy".to_string(),
                command: Some(vec![
                    "systemctl".to_string(),
                    "--user".to_string(),
                    "enable".to_string(),
                    "--now".to_string(),
                    format!("{label}.timer"),
                ]),
                file_path: None,
                file_contents: None,
                directory_path: None,
                can_auto_run: false,
                can_verify: true,
            },
        ],
        apply_commands: Vec::new(),
        rollback_commands: vec![vec![
            "systemctl".to_string(),
            "--user".to_string(),
            "disable".to_string(),
            "--now".to_string(),
            format!("{label}.timer"),
        ]],
        apply_supported: false,
    })
}

fn linux_on_calendar(hours: u64) -> String {
    if hours <= 1 { "*-*-* *:00:00".to_string() } else { format!("*-*-* 00/{hours}:00:00") }
}
