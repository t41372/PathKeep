use anyhow::{Context, Result};
use chrono::Utc;
use directories::UserDirs;
use keyring::use_native_store;
use keyring_core::Entry;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use vault_core::{
    ProjectPaths,
    models::{ApplyResult, GeneratedFile, KeyringStatusReport, S3CredentialInput, SchedulePlan},
};

const KEYRING_SERVICE: &str = "dev.codex.browser-history-backup";
const LEGACY_KEYRING_SERVICES: [&str; 1] = ["dev.codex.chrome-history-backup"];
const KEYRING_DATABASE_USER: &str = "database-key";
const KEYRING_S3_USER: &str = "remote-s3";
const MACOS_LABEL: &str = "dev.codex.browser-history-backup.backup";
const LEGACY_MACOS_LABEL: &str = "dev.codex.chrome-history-backup.backup";
const TEST_KEYRING_DIR_ENV: &str = "CHB_TEST_KEYRING_DIR";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleParameters {
    pub due_after_hours: u64,
    pub check_interval_hours: u64,
}

pub fn current_platform_name() -> String {
    if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else {
        "linux".to_string()
    }
}

fn keyring_entry(user: &str) -> Result<Entry> {
    Ok(Entry::new(KEYRING_SERVICE, user)?)
}

fn legacy_keyring_entries(user: &str) -> Vec<Entry> {
    LEGACY_KEYRING_SERVICES.iter().filter_map(|service| Entry::new(service, user).ok()).collect()
}

pub fn preview_schedule(
    platform: Option<&str>,
    executable_path: &Path,
    paths: &ProjectPaths,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let platform = platform.unwrap_or(if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    });

    let worker_args = vec![
        executable_path.display().to_string(),
        "--worker".to_string(),
        "backup".to_string(),
        "--due-only".to_string(),
    ];
    let log_dir = paths.schedule_dir.join("logs");
    let _ = fs::create_dir_all(&log_dir);

    match platform {
        "macos" => macos_schedule_plan(executable_path, &worker_args, &log_dir, params),
        "windows" => windows_schedule_plan(executable_path, &worker_args, params),
        _ => linux_schedule_plan(executable_path, &worker_args, params),
    }
}

pub fn apply_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform != "macos" {
        return Ok(ApplyResult {
            applied: false,
            platform: plan.platform.clone(),
            files: Vec::new(),
            audit_path: None,
            message:
                "Apply is only implemented on macOS in v1. Use the Manual steps on other platforms."
                    .to_string(),
        });
    }

    let launch_agents_dir =
        UserDirs::new().context("resolving home dir")?.home_dir().join("Library/LaunchAgents");
    fs::create_dir_all(&launch_agents_dir)?;

    let mut written_files = Vec::new();
    for file in &plan.generated_files {
        let target_path = launch_agents_dir
            .join(PathBuf::from(&file.relative_path).file_name().unwrap_or_default());
        fs::write(&target_path, &file.contents)?;
        written_files.push(target_path.display().to_string());
    }

    let uid = Command::new("id").arg("-u").output().context("running id -u")?;
    let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
    let plist_path =
        written_files.first().context("missing plist file for macOS schedule apply")?.clone();

    let _ = Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}"), LEGACY_MACOS_LABEL])
        .status();
    let _ =
        Command::new("launchctl").args(["bootout", &format!("gui/{uid}"), MACOS_LABEL]).status();
    let bootstrap = Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), &plist_path])
        .status()
        .context("bootstrapping launch agent")?;

    let audit_path = paths
        .audit_repo_path
        .join("scheduler")
        .join(format!("apply-{}.json", Utc::now().to_rfc3339().replace(':', "-")));
    if let Some(parent) = audit_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &audit_path,
        serde_json::to_string_pretty(&BTreeMap::from([
            ("platform".to_string(), plan.platform.clone()),
            ("plistPath".to_string(), plist_path.clone()),
            ("status".to_string(), format!("{bootstrap:?}")),
        ]))?,
    )?;

    Ok(ApplyResult {
        applied: bootstrap.success(),
        platform: plan.platform.clone(),
        files: written_files,
        audit_path: Some(audit_path.display().to_string()),
        message: if bootstrap.success() {
            "LaunchAgent installed and bootstrapped.".to_string()
        } else {
            "LaunchAgent files were written, but launchctl bootstrap did not report success."
                .to_string()
        },
    })
}

pub fn keyring_status() -> KeyringStatusReport {
    if let Some(path) = test_keyring_dir() {
        let stored_secret = test_keyring_path(&path, KEYRING_DATABASE_USER).exists();
        return KeyringStatusReport {
            available: true,
            backend: "File-backed test keyring".to_string(),
            stored_secret,
            message: None,
        };
    }

    let backend = if cfg!(target_os = "macos") {
        "macOS Keychain"
    } else if cfg!(target_os = "windows") {
        "Windows Credential Manager"
    } else {
        "Linux Secret Service / keyutils"
    };

    let available = use_native_store(cfg!(target_os = "linux")).is_ok();
    let stored_secret = if available {
        match keyring_entry(KEYRING_DATABASE_USER) {
            Ok(entry) => entry.get_password().is_ok(),
            Err(_) => false,
        }
    } else {
        false
    };

    KeyringStatusReport {
        available,
        backend: backend.to_string(),
        stored_secret,
        message: if available {
            None
        } else {
            Some("A native keyring backend is not available on this machine.".to_string())
        },
    }
}

pub fn keyring_get_database_key() -> Result<Option<String>> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_get(&path, KEYRING_DATABASE_USER);
    }

    use_native_store(cfg!(target_os = "linux")).ok();
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(_) => {
            for legacy_entry in legacy_keyring_entries(KEYRING_DATABASE_USER) {
                if let Ok(value) = legacy_entry.get_password() {
                    return Ok(Some(value));
                }
            }
            Ok(None)
        }
    }
}

pub fn keyring_set_database_key(key: &str) -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, KEYRING_DATABASE_USER, key);
    }

    use_native_store(cfg!(target_os = "linux")).context("initializing keyring backend")?;
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    entry.set_password(key)?;
    Ok(())
}

pub fn keyring_clear_database_key() -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, KEYRING_DATABASE_USER);
    }

    use_native_store(cfg!(target_os = "linux")).ok();
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    let _ = entry.delete_credential();
    for legacy_entry in legacy_keyring_entries(KEYRING_DATABASE_USER) {
        let _ = legacy_entry.delete_credential();
    }
    Ok(())
}

pub fn keyring_get_s3_credentials() -> Result<Option<S3CredentialInput>> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_get(&path, KEYRING_S3_USER)?
            .map(|value| serde_json::from_str(&value).context("parsing stored S3 credentials"))
            .transpose();
    }

    use_native_store(cfg!(target_os = "linux")).ok();
    let entry = keyring_entry(KEYRING_S3_USER)?;
    match entry.get_password() {
        Ok(value) => {
            Ok(Some(serde_json::from_str(&value).context("parsing stored S3 credentials")?))
        }
        Err(_) => {
            for legacy_entry in legacy_keyring_entries(KEYRING_S3_USER) {
                if let Ok(value) = legacy_entry.get_password() {
                    return Ok(Some(
                        serde_json::from_str(&value)
                            .context("parsing stored legacy S3 credentials")?,
                    ));
                }
            }
            Ok(None)
        }
    }
}

pub fn keyring_set_s3_credentials(credentials: &S3CredentialInput) -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, KEYRING_S3_USER, &serde_json::to_string(credentials)?);
    }

    use_native_store(cfg!(target_os = "linux")).context("initializing keyring backend")?;
    let entry = keyring_entry(KEYRING_S3_USER)?;
    entry.set_password(&serde_json::to_string(credentials)?)?;
    Ok(())
}

pub fn keyring_clear_s3_credentials() -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, KEYRING_S3_USER);
    }

    use_native_store(cfg!(target_os = "linux")).ok();
    let entry = keyring_entry(KEYRING_S3_USER)?;
    let _ = entry.delete_credential();
    for legacy_entry in legacy_keyring_entries(KEYRING_S3_USER) {
        let _ = legacy_entry.delete_credential();
    }
    Ok(())
}

pub fn s3_credentials_saved() -> bool {
    keyring_get_s3_credentials().ok().flatten().is_some()
}

fn macos_schedule_plan(
    executable_path: &Path,
    worker_args: &[String],
    log_dir: &Path,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let plist = plist::Value::Dictionary(plist::Dictionary::from_iter([
        ("Label".to_string(), plist::Value::String(MACOS_LABEL.to_string())),
        (
            "ProgramArguments".to_string(),
            plist::Value::Array(worker_args.iter().cloned().map(plist::Value::String).collect()),
        ),
        ("RunAtLoad".to_string(), plist::Value::Boolean(true)),
        (
            "StartInterval".to_string(),
            plist::Value::Integer((params.check_interval_hours * 3600).into()),
        ),
        (
            "StandardOutPath".to_string(),
            plist::Value::String(log_dir.join("worker.stdout.log").display().to_string()),
        ),
        (
            "StandardErrorPath".to_string(),
            plist::Value::String(log_dir.join("worker.stderr.log").display().to_string()),
        ),
    ]));
    let mut buffer = Vec::new();
    plist::to_writer_xml(&mut buffer, &plist)?;
    let contents = String::from_utf8(buffer)?;
    Ok(SchedulePlan {
        platform: "macos".to_string(),
        label: MACOS_LABEL.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![GeneratedFile {
            relative_path: format!("launchd/{MACOS_LABEL}.plist"),
            absolute_path: None,
            purpose: format!(
                "Run worker mode every {} hours and immediately after login or boot.",
                params.check_interval_hours
            ),
            contents,
        }],
        manual_steps: vec![
            format!("Save the plist to ~/Library/LaunchAgents/{MACOS_LABEL}.plist."),
            format!(
                "Run `launchctl bootout gui/$(id -u) {}` to unload an older Chrome History Backup schedule if one still exists.",
                LEGACY_MACOS_LABEL
            ),
            format!(
                "Run `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/{MACOS_LABEL}.plist` to load the new schedule."
            ),
        ],
        apply_commands: vec![
            vec![
                "launchctl".to_string(),
                "bootout".to_string(),
                "gui/$(id -u)".to_string(),
                MACOS_LABEL.to_string(),
            ],
            vec![
                "launchctl".to_string(),
                "bootstrap".to_string(),
                "gui/$(id -u)".to_string(),
                format!("~/Library/LaunchAgents/{MACOS_LABEL}.plist"),
            ],
        ],
        rollback_commands: vec![vec![
            "launchctl".to_string(),
            "bootout".to_string(),
            "gui/$(id -u)".to_string(),
            MACOS_LABEL.to_string(),
        ]],
        apply_supported: true,
    })
}

fn windows_schedule_plan(
    executable_path: &Path,
    worker_args: &[String],
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger />
    <TimeTrigger>
      <Enabled>true</Enabled>
      <Repetition>
        <Interval>PT{}H</Interval>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
      <StartBoundary>2026-01-01T09:00:00</StartBoundary>
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <RunLevel>LeastPrivilege</RunLevel>
      <LogonType>InteractiveToken</LogonType>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <StartWhenAvailable>true</StartWhenAvailable>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>{}</Arguments>
    </Exec>
  </Actions>
</Task>"#,
        params.check_interval_hours,
        xml_escape(&executable_path.display().to_string()),
        xml_escape(&worker_args[1..].join(" "))
    );
    Ok(SchedulePlan {
        platform: "windows".to_string(),
        label: MACOS_LABEL.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![GeneratedFile {
                relative_path: "windows/browser-history-backup-task.xml".to_string(),
            absolute_path: None,
            purpose: "Import into Task Scheduler or register with schtasks.exe".to_string(),
            contents: xml,
        }],
        manual_steps: vec![
            "Save the XML file and import it in Task Scheduler.".to_string(),
            "Alternatively run `schtasks /Create /TN BrowserHistoryBackup /XML browser-history-backup-task.xml`.".to_string(),
        ],
        apply_commands: Vec::new(),
        rollback_commands: vec![vec![
            "schtasks".to_string(),
            "/Delete".to_string(),
            "/TN".to_string(),
            "BrowserHistoryBackup".to_string(),
            "/F".to_string(),
        ]],
        apply_supported: false,
    })
}

fn linux_schedule_plan(
    executable_path: &Path,
    worker_args: &[String],
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let service = format!(
        "[Unit]\nDescription=Browser History Backup backup worker\n\n[Service]\nType=oneshot\nExecStart={} {}\n",
        executable_path.display(),
        worker_args[1..].join(" ")
    );
    let timer = format!(
        "[Unit]\nDescription=Browser History Backup periodic backup\n\n[Timer]\nOnBootSec=2m\nOnUnitActiveSec={}h\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n",
        params.check_interval_hours
    );
    Ok(SchedulePlan {
        platform: "linux".to_string(),
        label: MACOS_LABEL.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![
            GeneratedFile {
                relative_path: "systemd/browser-history-backup.service".to_string(),
                absolute_path: None,
                purpose: "User service entry for the worker mode".to_string(),
                contents: service,
            },
            GeneratedFile {
                relative_path: "systemd/browser-history-backup.timer".to_string(),
                absolute_path: None,
                purpose: format!(
                    "Persistent user timer that wakes every {} hours.",
                    params.check_interval_hours
                ),
                contents: timer,
            },
        ],
        manual_steps: vec![
            "Copy the files to ~/.config/systemd/user/.".to_string(),
            "Run `systemctl --user daemon-reload`.".to_string(),
            "Run `systemctl --user enable --now browser-history-backup.timer`.".to_string(),
        ],
        apply_commands: Vec::new(),
        rollback_commands: vec![vec![
            "systemctl".to_string(),
            "--user".to_string(),
            "disable".to_string(),
            "--now".to_string(),
            "browser-history-backup.timer".to_string(),
        ]],
        apply_supported: false,
    })
}

fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn test_keyring_dir() -> Option<PathBuf> {
    std::env::var_os(TEST_KEYRING_DIR_ENV).map(PathBuf::from)
}

fn test_keyring_path(root: &Path, user: &str) -> PathBuf {
    root.join(format!("{KEYRING_SERVICE}-{user}.secret"))
}

fn test_keyring_get(root: &Path, user: &str) -> Result<Option<String>> {
    let path = test_keyring_path(root, user);
    if !path.exists() {
        return Ok(None);
    }
    let value = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(value))
}

fn test_keyring_set(root: &Path, user: &str, value: &str) -> Result<()> {
    fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
    let path = test_keyring_path(root, user);
    fs::write(&path, value).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

fn test_keyring_clear(root: &Path, user: &str) -> Result<()> {
    let path = test_keyring_path(root, user);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
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
            audit_repo_path: root.join("audit"),
            manifests_dir: root.join("audit/manifests"),
            exports_dir: root.join("exports"),
            raw_snapshots_dir: root.join("raw-snapshots"),
            staging_dir: root.join("staging"),
            quarantine_dir: root.join("quarantine"),
            schedule_dir: root.join("schedule"),
            stronghold_path: root.join("vault.hold"),
            stronghold_salt_path: root.join("stronghold-salt.txt"),
        }
    }

    #[test]
    fn preview_schedule_supports_all_platform_variants() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };

        let mac =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("mac");
        let windows = preview_schedule(Some("windows"), Path::new("C:/chb.exe"), &paths, &params)
            .expect("windows");
        let linux = preview_schedule(Some("linux"), Path::new("/usr/bin/chb"), &paths, &params)
            .expect("linux");

        assert_eq!(mac.platform, "macos");
        assert_eq!(windows.platform, "windows");
        assert_eq!(linux.platform, "linux");
        assert!(!mac.generated_files.is_empty());
        assert!(!windows.generated_files.is_empty());
        assert!(!linux.generated_files.is_empty());
    }

    #[test]
    fn non_macos_apply_is_manual_only() {
        let dir = tempdir().expect("tempdir");
        let result = apply_schedule(
            &SchedulePlan {
                platform: "linux".to_string(),
                label: "example".to_string(),
                executable_path: "/usr/bin/chb".to_string(),
                generated_files: Vec::new(),
                manual_steps: Vec::new(),
                apply_commands: Vec::new(),
                rollback_commands: Vec::new(),
                apply_supported: false,
            },
            &sample_paths(dir.path()),
        )
        .expect("apply");

        assert!(!result.applied);
        assert!(result.message.contains("Manual"));
    }

    #[test]
    fn file_backed_test_keyring_roundtrips_secrets() {
        let dir = tempdir().expect("tempdir");
        unsafe {
            std::env::set_var(TEST_KEYRING_DIR_ENV, dir.path());
        }

        keyring_set_database_key("database-secret").expect("set db");
        assert_eq!(
            keyring_get_database_key().expect("get db"),
            Some("database-secret".to_string())
        );
        assert!(keyring_status().stored_secret);

        let credentials = S3CredentialInput {
            access_key_id: "abc".to_string(),
            secret_access_key: "def".to_string(),
        };
        keyring_set_s3_credentials(&credentials).expect("set s3");
        assert_eq!(keyring_get_s3_credentials().expect("get s3"), Some(credentials));
        assert!(s3_credentials_saved());

        keyring_clear_database_key().expect("clear db");
        keyring_clear_s3_credentials().expect("clear s3");
        unsafe {
            std::env::remove_var(TEST_KEYRING_DIR_ENV);
        }
    }

    #[test]
    fn xml_escape_rewrites_reserved_characters() {
        assert_eq!(
            xml_escape("<tag attr=\"1\">&</tag>"),
            "&lt;tag attr=&quot;1&quot;&gt;&amp;&lt;/tag&gt;"
        );
    }

    #[test]
    fn current_platform_name_is_supported() {
        let value = current_platform_name();
        assert!(matches!(value.as_str(), "macos" | "windows" | "linux"));
    }
}
