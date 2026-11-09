//! Responsibilities: compare the due-check wake interval with each platform's generated file.
//! Not responsible for: installing a schedule or measuring operating-system jitter.
//! Failure modes: the worker uses the backup interval rather than the health-check cap, or
//! uses Linux's requested interval instead of the shorter OnCalendar grid.

use super::*;
use std::{path::Path, time::Duration};

#[test]
fn the_worker_uses_the_wake_interval_written_by_each_platform() {
    let dir = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(dir.path());
    for (due_hours, mac_seconds, windows_interval, linux_grid, linux_seconds) in [
        (1.0, 3_600, "PT1H", "*-*-* *:00:00", 3_600),
        (24.0, 21_600, "PT6H", "*-*-* 00/6:00:00", 21_600),
        (1.5, 5_400, "PT1H30M", "*-*-* *:00/30:00", 1_800),
        (7.0 / 60.0, 420, "PT7M", "*-*-* *:*:00", 60),
    ] {
        let config = vault_core::AppConfig { due_after_hours: due_hours, ..Default::default() };
        let params = ScheduleParameters::from(&config);
        for platform in ["macos", "windows", "linux"] {
            let plan =
                preview_schedule(Some(platform), Path::new("/fixture/pathkeep"), &paths, &params)
                    .unwrap();
            let expected_seconds = if platform == "linux" { linux_seconds } else { mac_seconds };
            assert_eq!(plan.wake_interval_seconds, expected_seconds, "preview wake interval");
            match platform {
                "macos" => assert!(plan.generated_files[0].contents.contains(&format!(
                    "<key>StartInterval</key>\n\t<integer>{expected_seconds}</integer>"
                ))),
                "windows" => assert!(plan.generated_files[0].contents.contains(&format!(
                    "<Repetition>\n        <Interval>{windows_interval}</Interval>"
                ))),
                _ => assert!(
                    plan.generated_files[1]
                        .contents
                        .contains(&format!("OnCalendar={linux_grid}\n"))
                ),
            }
            let worker_wake = params.wake_interval(Some(platform));
            assert_eq!(
                worker_wake,
                Duration::from_secs(expected_seconds),
                "{platform}: {due_hours}h"
            );
        }
    }
}
