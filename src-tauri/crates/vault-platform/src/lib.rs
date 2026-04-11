mod biometric;

use anyhow::{Context, Result};
#[cfg(all(not(coverage), target_os = "macos"))]
use apple_native_keyring_store::keychain::Store as NativeKeyringStore;
use chrono::Utc;
#[cfg(all(not(coverage), any(target_os = "linux", target_os = "freebsd")))]
use dbus_secret_service_keyring_store::Store as NativeKeyringStore;
#[cfg(not(any(test, coverage)))]
use directories::UserDirs;
#[cfg(not(coverage))]
use keyring_core::{Entry, get_default_store, set_default_store};
use serde::Serialize;
#[cfg(all(not(coverage), target_os = "macos"))]
use std::collections::HashMap;
#[cfg(not(any(test, coverage)))]
use std::process::Command;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use vault_core::{
    ProjectPaths,
    models::{
        ApplyResult, GeneratedFile, KeyringStatusReport, S3CredentialInput, SchedulePlan,
        ScheduleStatus,
    },
};
#[cfg(all(not(coverage), target_os = "windows"))]
use windows_native_keyring_store::Store as NativeKeyringStore;

const KEYRING_SERVICE: &str = "com.yi-ting.pathkeep";
const KEYRING_DATABASE_USER: &str = "database-key";
const KEYRING_S3_USER: &str = "remote-s3";
const MACOS_LABEL: &str = "com.yi-ting.pathkeep.backup";
const TEST_KEYRING_DIR_ENV: &str = "CHB_TEST_KEYRING_DIR";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleParameters {
    pub due_after_hours: u64,
    pub check_interval_hours: u64,
}

pub fn current_platform_name() -> String {
    compiled_platform_name().to_string()
}

pub use biometric::{app_lock_biometric_state, authenticate_app_lock_biometric};

#[cfg(not(coverage))]
fn keyring_entry(user: &str) -> Result<Entry> {
    ensure_native_keyring_store()?;
    Ok(Entry::new(KEYRING_SERVICE, user)?)
}

fn provider_keyring_user(provider_id: &str) -> String {
    format!("ai-provider::{provider_id}")
}

#[cfg(all(
    not(coverage),
    any(target_os = "macos", target_os = "windows", target_os = "linux", target_os = "freebsd",)
))]
fn ensure_native_keyring_store() -> Result<()> {
    if get_default_store().is_none() {
        set_default_store(
            NativeKeyringStore::new().context("initializing native keyring backend")?,
        );
    }
    Ok(())
}

#[cfg(all(
    not(coverage),
    not(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd",
    ))
))]
fn ensure_native_keyring_store() -> Result<()> {
    anyhow::bail!("A native keyring backend is not available on this machine.")
}

#[cfg(all(not(coverage), target_os = "macos"))]
fn keyring_entry_exists_for_service(service: &str, user: &str) -> bool {
    ensure_native_keyring_store().ok();
    Entry::search(&HashMap::from([("service", service), ("user", user)]))
        .map(|entries| !entries.is_empty())
        .unwrap_or(false)
}

#[cfg(all(not(coverage), not(target_os = "macos")))]
fn keyring_entry_exists_for_service(service: &str, user: &str) -> bool {
    ensure_native_keyring_store().ok();
    Entry::new(service, user).ok().and_then(|entry| entry.get_password().ok()).is_some()
}

#[cfg(not(coverage))]
fn keyring_entry_exists(user: &str) -> bool {
    keyring_entry_exists_for_service(KEYRING_SERVICE, user)
}

pub fn preview_schedule(
    platform: Option<&str>,
    executable_path: &Path,
    paths: &ProjectPaths,
    params: &ScheduleParameters,
) -> Result<SchedulePlan> {
    let platform = platform.unwrap_or(compiled_platform_name());

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

    let launch_agents_dir = launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;

    let mut written_files = Vec::new();
    for file in &plan.generated_files {
        let target_path = launch_agents_dir
            .join(PathBuf::from(&file.relative_path).file_name().unwrap_or_default());
        fs::write(&target_path, &file.contents)?;
        written_files.push(target_path.display().to_string());
    }

    let uid = scheduler_uid()?;
    let plist_path =
        written_files.first().context("missing plist file for macOS schedule apply")?.clone();

    let bootstrap = bootstrap_launch_agent(&uid, &plist_path)?;
    let audit_path = write_schedule_apply_audit(paths, plan, &plist_path, &bootstrap)?;

    Ok(ApplyResult {
        applied: bootstrap.success,
        platform: plan.platform.clone(),
        files: written_files,
        audit_path: Some(audit_path.display().to_string()),
        message: if bootstrap.success {
            "LaunchAgent installed and bootstrapped.".to_string()
        } else {
            "LaunchAgent files were written, but launchctl bootstrap did not report success."
                .to_string()
        },
    })
}

pub fn remove_schedule(plan: &SchedulePlan, paths: &ProjectPaths) -> Result<ApplyResult> {
    if plan.platform != "macos" {
        return Ok(ApplyResult {
            applied: false,
            platform: plan.platform.clone(),
            files: Vec::new(),
            audit_path: None,
            message:
                "Remove is only implemented on macOS in v1. Use the Manual rollback steps on other platforms."
                    .to_string(),
        });
    }

    let launch_agents_dir = launch_agents_dir()?;
    fs::create_dir_all(&launch_agents_dir)?;

    let uid = scheduler_uid()?;
    let current_path = generated_plist_target_path(plan, &launch_agents_dir)?;
    let current_unload = bootout_launch_agent(&uid, MACOS_LABEL)?;

    let mut removed_files = Vec::new();
    if current_path.exists() {
        fs::remove_file(&current_path)?;
        removed_files.push(current_path.display().to_string());
    }

    let audit_path = write_schedule_remove_audit(paths, plan, &removed_files, &[current_unload])?;
    let applied = !removed_files.is_empty();

    Ok(ApplyResult {
        applied,
        platform: plan.platform.clone(),
        files: removed_files,
        audit_path: Some(audit_path.display().to_string()),
        message: if applied {
            "Installed LaunchAgent files were removed.".to_string()
        } else {
            "No installed PathKeep LaunchAgent files were found to remove.".to_string()
        },
    })
}

pub fn schedule_status(
    platform: Option<&str>,
    executable_path: &Path,
    paths: &ProjectPaths,
    params: &ScheduleParameters,
) -> Result<ScheduleStatus> {
    let plan = preview_schedule(platform, executable_path, paths, params)?;
    let mut status = ScheduleStatus {
        platform: plan.platform.clone(),
        label: plan.label.clone(),
        due_after_hours: params.due_after_hours,
        check_interval_hours: params.check_interval_hours,
        apply_supported: plan.apply_supported,
        install_state: if plan.platform == "macos" {
            "not-installed".to_string()
        } else {
            "manual-review".to_string()
        },
        manual_steps: plan.manual_steps.clone(),
        audit_path: latest_schedule_audit_path(paths),
        ..ScheduleStatus::default()
    };

    if plan.platform != "macos" {
        status.warnings.push(
            "Automatic install-status detection is only implemented on macOS in v1. Use the manual verification steps for this platform.".to_string(),
        );
        return Ok(status);
    }

    let launch_agents_dir = launch_agents_dir()?;
    let target_path = generated_plist_target_path(&plan, &launch_agents_dir)?;

    if target_path.exists() {
        status.detected_files.push(target_path.display().to_string());
        match fs::read_to_string(&target_path) {
            Ok(contents) => {
                status.install_state = if contents == plan.generated_files[0].contents {
                    "installed".to_string()
                } else {
                    status
                        .warnings
                        .push("Installed LaunchAgent content no longer matches the current PathKeep schedule plan.".to_string());
                    "mismatch".to_string()
                };
            }
            Err(error) => {
                status.install_state = "permission-warning".to_string();
                status.warnings.push(format!(
                    "PathKeep could not read the installed LaunchAgent at {}: {error}",
                    target_path.display()
                ));
            }
        }
    }

    Ok(status)
}

fn generated_plist_target_path(plan: &SchedulePlan, launch_agents_dir: &Path) -> Result<PathBuf> {
    let generated =
        plan.generated_files.first().context("missing plist file for macOS schedule plan")?;
    Ok(launch_agents_dir
        .join(PathBuf::from(&generated.relative_path).file_name().unwrap_or_default()))
}

fn write_schedule_apply_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    plist_path: &str,
    bootstrap: &LaunchctlOutcome,
) -> Result<PathBuf> {
    let audit_path = paths
        .audit_repo_path
        .join("scheduler")
        .join(format!("apply-{}.json", Utc::now().to_rfc3339().replace(':', "-")));
    ensure_parent_dir(&audit_path)?;

    let contents = serde_json::to_string_pretty(&BTreeMap::from([
        ("platform".to_string(), plan.platform.clone()),
        ("plistPath".to_string(), plist_path.to_string()),
        ("status".to_string(), bootstrap.status_description.clone()),
    ]))?;
    fs::write(&audit_path, contents)?;
    Ok(audit_path)
}

fn write_schedule_remove_audit(
    paths: &ProjectPaths,
    plan: &SchedulePlan,
    removed_files: &[String],
    unloads: &[LaunchctlOutcome],
) -> Result<PathBuf> {
    let audit_path = paths
        .audit_repo_path
        .join("scheduler")
        .join(format!("remove-{}.json", Utc::now().to_rfc3339().replace(':', "-")));
    ensure_parent_dir(&audit_path)?;

    let contents = serde_json::to_string_pretty(&BTreeMap::from([
        ("action".to_string(), "remove".to_string()),
        ("platform".to_string(), plan.platform.clone()),
        ("label".to_string(), plan.label.clone()),
        ("removedFiles".to_string(), serde_json::to_string(removed_files)?),
        (
            "launchctl".to_string(),
            serde_json::to_string(
                &unloads
                    .iter()
                    .map(|outcome| outcome.status_description.clone())
                    .collect::<Vec<_>>(),
            )?,
        ),
    ]))?;
    fs::write(&audit_path, contents)?;
    Ok(audit_path)
}

fn latest_schedule_audit_path(paths: &ProjectPaths) -> Option<String> {
    let scheduler_dir = paths.audit_repo_path.join("scheduler");
    let mut newest = fs::read_dir(&scheduler_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect::<Vec<_>>();
    newest.sort_by_key(|(modified, _)| *modified);
    newest.last().map(|(_, path)| path.display().to_string())
}

#[rustfmt::skip]
fn ensure_parent_dir(path: &Path) -> Result<()> { if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; } Ok(()) }

#[cfg(coverage)]
pub fn keyring_status() -> KeyringStatusReport {
    let path = test_keyring_dir().expect("coverage keyring dir");
    KeyringStatusReport {
        available: true,
        backend: "File-backed test keyring".to_string(),
        stored_secret: test_keyring_path(&path, KEYRING_DATABASE_USER).exists(),
        message: None,
    }
}

#[cfg(not(coverage))]
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

    let backend = keyring_backend_name();
    let available = ensure_native_keyring_store().is_ok();
    let stored_secret = available && keyring_entry_exists(KEYRING_DATABASE_USER);

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

#[cfg(coverage)]
pub fn keyring_get_database_key() -> Result<Option<String>> {
    test_keyring_get(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_DATABASE_USER)
}

#[cfg(not(coverage))]
pub fn keyring_get_database_key() -> Result<Option<String>> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_get(&path, KEYRING_DATABASE_USER);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

#[cfg(coverage)]
pub fn keyring_set_database_key(key: &str) -> Result<()> {
    test_keyring_set(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_DATABASE_USER, key)
}

#[cfg(not(coverage))]
pub fn keyring_set_database_key(key: &str) -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, KEYRING_DATABASE_USER, key);
    }

    ensure_native_keyring_store()?;
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    entry.set_password(key)?;
    Ok(())
}

#[cfg(coverage)]
pub fn keyring_clear_database_key() -> Result<()> {
    test_keyring_clear(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_DATABASE_USER)
}

#[cfg(not(coverage))]
pub fn keyring_clear_database_key() -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, KEYRING_DATABASE_USER);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(KEYRING_DATABASE_USER)?;
    let _ = entry.delete_credential();
    Ok(())
}

#[cfg(coverage)]
pub fn keyring_get_s3_credentials() -> Result<Option<S3CredentialInput>> {
    test_keyring_get(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_S3_USER)?
        .map(|value| serde_json::from_str(&value).context("parsing stored S3 credentials"))
        .transpose()
}

#[cfg(not(coverage))]
pub fn keyring_get_s3_credentials() -> Result<Option<S3CredentialInput>> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_get(&path, KEYRING_S3_USER)?
            .map(|value| serde_json::from_str(&value).context("parsing stored S3 credentials"))
            .transpose();
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(KEYRING_S3_USER)?;
    match entry.get_password() {
        Ok(value) => {
            Ok(Some(serde_json::from_str(&value).context("parsing stored S3 credentials")?))
        }
        Err(_) => Ok(None),
    }
}

#[cfg(coverage)]
pub fn keyring_set_s3_credentials(credentials: &S3CredentialInput) -> Result<()> {
    test_keyring_set(
        &test_keyring_dir().expect("coverage keyring dir"),
        KEYRING_S3_USER,
        &serde_json::to_string(credentials)?,
    )
}

#[cfg(not(coverage))]
pub fn keyring_set_s3_credentials(credentials: &S3CredentialInput) -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, KEYRING_S3_USER, &serde_json::to_string(credentials)?);
    }

    ensure_native_keyring_store()?;
    let entry = keyring_entry(KEYRING_S3_USER)?;
    entry.set_password(&serde_json::to_string(credentials)?)?;
    Ok(())
}

#[cfg(coverage)]
pub fn keyring_clear_s3_credentials() -> Result<()> {
    test_keyring_clear(&test_keyring_dir().expect("coverage keyring dir"), KEYRING_S3_USER)
}

#[cfg(not(coverage))]
pub fn keyring_clear_s3_credentials() -> Result<()> {
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, KEYRING_S3_USER);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(KEYRING_S3_USER)?;
    let _ = entry.delete_credential();
    Ok(())
}

pub fn s3_credentials_saved() -> bool {
    #[cfg(coverage)]
    {
        keyring_get_s3_credentials().ok().flatten().is_some()
    }

    #[cfg(not(coverage))]
    {
        if let Some(path) = test_keyring_dir() {
            return test_keyring_path(&path, KEYRING_S3_USER).exists();
        }

        keyring_entry_exists_for_service(KEYRING_SERVICE, KEYRING_S3_USER)
    }
}

#[cfg(coverage)]
pub fn keyring_get_provider_api_key(provider_id: &str) -> Result<Option<String>> {
    let user = provider_keyring_user(provider_id);
    test_keyring_get(&test_keyring_dir().expect("coverage keyring dir"), &user)
}

#[cfg(not(coverage))]
pub fn keyring_get_provider_api_key(provider_id: &str) -> Result<Option<String>> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_get(&path, &user);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(&user)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

#[cfg(coverage)]
pub fn keyring_set_provider_api_key(provider_id: &str, api_key: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    test_keyring_set(&test_keyring_dir().expect("coverage keyring dir"), &user, api_key)
}

#[cfg(not(coverage))]
pub fn keyring_set_provider_api_key(provider_id: &str, api_key: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_set(&path, &user, api_key);
    }

    ensure_native_keyring_store()?;
    let entry = keyring_entry(&user)?;
    entry.set_password(api_key)?;
    Ok(())
}

#[cfg(coverage)]
pub fn keyring_clear_provider_api_key(provider_id: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    test_keyring_clear(&test_keyring_dir().expect("coverage keyring dir"), &user)
}

#[cfg(not(coverage))]
pub fn keyring_clear_provider_api_key(provider_id: &str) -> Result<()> {
    let user = provider_keyring_user(provider_id);
    if let Some(path) = test_keyring_dir() {
        return test_keyring_clear(&path, &user);
    }

    ensure_native_keyring_store().ok();
    let entry = keyring_entry(&user)?;
    let _ = entry.delete_credential();
    Ok(())
}

pub fn provider_api_key_saved(provider_id: &str) -> bool {
    #[cfg(coverage)]
    {
        keyring_get_provider_api_key(provider_id).ok().flatten().is_some()
    }

    #[cfg(not(coverage))]
    {
        let user = provider_keyring_user(provider_id);
        if let Some(path) = test_keyring_dir() {
            return test_keyring_path(&path, &user).exists();
        }

        keyring_entry_exists_for_service(KEYRING_SERVICE, &user)
    }
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
            relative_path: "windows/com.yi-ting.pathkeep.task.xml".to_string(),
            absolute_path: None,
            purpose: "Import into Task Scheduler or register with schtasks.exe".to_string(),
            contents: xml,
        }],
        manual_steps: vec![
            "Save the XML file and import it in Task Scheduler.".to_string(),
            "Alternatively run `schtasks /Create /TN com.yi-ting.pathkeep.backup /XML com.yi-ting.pathkeep.task.xml`.".to_string(),
        ],
        apply_commands: Vec::new(),
        rollback_commands: vec![vec![
            "schtasks".to_string(),
            "/Delete".to_string(),
            "/TN".to_string(),
            MACOS_LABEL.to_string(),
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
        label: MACOS_LABEL.to_string(),
        executable_path: executable_path.display().to_string(),
        generated_files: vec![
            GeneratedFile {
                relative_path: "systemd/com.yi-ting.pathkeep.service".to_string(),
                absolute_path: None,
                purpose: "User service entry for the worker mode".to_string(),
                contents: service,
            },
            GeneratedFile {
                relative_path: "systemd/com.yi-ting.pathkeep.timer".to_string(),
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
            "Run `systemctl --user enable --now com.yi-ting.pathkeep.timer`.".to_string(),
            "Run `systemctl --user list-timers com.yi-ting.pathkeep.timer` to verify the next scheduled run."
                .to_string(),
        ],
        apply_commands: Vec::new(),
        rollback_commands: vec![vec![
            "systemctl".to_string(),
            "--user".to_string(),
            "disable".to_string(),
            "--now".to_string(),
            "com.yi-ting.pathkeep.timer".to_string(),
        ]],
        apply_supported: false,
    })
}

fn linux_on_calendar(hours: u64) -> String {
    if hours <= 1 { "*-*-* *:00:00".to_string() } else { format!("*-*-* 00/{hours}:00:00") }
}

fn xml_escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

struct LaunchctlOutcome {
    success: bool,
    status_description: String,
}

#[cfg(target_os = "macos")]
fn compiled_platform_name() -> &'static str {
    "macos"
}

#[cfg(target_os = "windows")]
fn compiled_platform_name() -> &'static str {
    "windows"
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn compiled_platform_name() -> &'static str {
    "linux"
}

#[cfg(all(target_os = "macos", not(coverage)))]
fn keyring_backend_name() -> &'static str {
    "macOS Keychain"
}

#[cfg(all(target_os = "windows", not(coverage)))]
fn keyring_backend_name() -> &'static str {
    "Windows Credential Manager"
}

#[cfg(all(not(any(target_os = "macos", target_os = "windows")), not(coverage)))]
fn keyring_backend_name() -> &'static str {
    "Linux Secret Service / keyutils"
}

#[cfg(not(any(test, coverage)))]
fn launch_agents_dir() -> Result<PathBuf> {
    Ok(UserDirs::new().context("resolving home dir")?.home_dir().join("Library/LaunchAgents"))
}

#[cfg(any(test, coverage))]
fn launch_agents_dir() -> Result<PathBuf> {
    Ok(std::env::var_os("CHB_TEST_LAUNCH_AGENTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("pathkeep-launch-agents")))
}

#[cfg(not(any(test, coverage)))]
fn scheduler_uid() -> Result<String> {
    let uid = Command::new("id").arg("-u").output().context("running id -u")?;
    Ok(String::from_utf8_lossy(&uid.stdout).trim().to_string())
}

#[cfg(any(test, coverage))]
fn scheduler_uid() -> Result<String> {
    Ok("501".to_string())
}

#[cfg(not(any(test, coverage)))]
fn bootstrap_launch_agent(uid: &str, plist_path: &str) -> Result<LaunchctlOutcome> {
    let _ = bootout_launch_agent(uid, MACOS_LABEL);
    let status = Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), plist_path])
        .status()
        .context("bootstrapping launch agent")?;
    Ok(LaunchctlOutcome { success: status.success(), status_description: format!("{status:?}") })
}

#[cfg(any(test, coverage))]
fn bootstrap_launch_agent(uid: &str, plist_path: &str) -> Result<LaunchctlOutcome> {
    let success =
        std::env::var("CHB_TEST_LAUNCHCTL_SUCCESS").unwrap_or_else(|_| "1".to_string()) != "0";
    Ok(LaunchctlOutcome {
        success,
        status_description: format!("stub bootstrap gui/{uid} {plist_path}"),
    })
}

#[cfg(not(any(test, coverage)))]
fn bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
    let status = Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}"), label])
        .status()
        .context("unloading launch agent")?;
    Ok(LaunchctlOutcome {
        success: status.success(),
        status_description: format!("bootout {label}: {status:?}"),
    })
}

#[cfg(any(test, coverage))]
fn bootout_launch_agent(uid: &str, label: &str) -> Result<LaunchctlOutcome> {
    let success =
        std::env::var("CHB_TEST_LAUNCHCTL_SUCCESS").unwrap_or_else(|_| "1".to_string()) != "0";
    Ok(LaunchctlOutcome { success, status_description: format!("stub bootout gui/{uid} {label}") })
}

#[cfg(coverage)]
fn test_keyring_dir() -> Option<PathBuf> {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    Some(std::env::var_os(TEST_KEYRING_DIR_ENV).map(PathBuf::from).unwrap_or_else(|| {
        ROOT.get_or_init(|| {
            std::env::temp_dir().join(format!("pathkeep-coverage-keyring-{}", std::process::id()))
        })
        .clone()
    }))
}

#[cfg(not(coverage))]
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
    use std::{
        ffi::OsStr,
        sync::{Mutex, OnceLock},
    };
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn restore_env_var(name: &str, value: Option<&OsStr>) {
        unsafe {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }

    fn sample_paths(root: &Path) -> ProjectPaths {
        vault_core::config::project_paths_with_root(root)
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
        assert!(mac.apply_supported);
        assert!(!windows.apply_supported);
        assert!(!linux.apply_supported);
        assert!(windows.generated_files[0].contents.contains("<Task"));
        assert!(linux.generated_files[1].contents.contains("Persistent=true"));
        assert!(linux.generated_files[1].contents.contains("OnCalendar="));
        assert!(!linux.generated_files[1].contents.contains("OnUnitActiveSec"));
    }

    #[test]
    fn preview_schedule_uses_current_platform_when_unspecified() {
        let dir = tempdir().expect("tempdir");
        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let preview = preview_schedule(None, Path::new("/tmp/bhb"), &paths, &params)
            .expect("default preview");
        assert_eq!(preview.platform, current_platform_name());
        assert!(!preview.manual_steps.is_empty());
    }

    #[test]
    fn macos_schedule_status_detects_installed_content() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os("CHB_TEST_LAUNCH_AGENTS_DIR");
        unsafe {
            std::env::set_var("CHB_TEST_LAUNCH_AGENTS_DIR", &launch_agents_dir);
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        fs::create_dir_all(&launch_agents_dir).expect("create launch agents dir");
        fs::write(
            launch_agents_dir.join(format!("{MACOS_LABEL}.plist")),
            &plan.generated_files[0].contents,
        )
        .expect("write current plist");

        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var("CHB_TEST_LAUNCH_AGENTS_DIR", original_launch_agents.as_deref());

        assert_eq!(status.install_state, "installed");
        assert_eq!(status.detected_files.len(), 1);
    }

    #[test]
    fn macos_schedule_status_detects_mismatch() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os("CHB_TEST_LAUNCH_AGENTS_DIR");
        unsafe {
            std::env::set_var("CHB_TEST_LAUNCH_AGENTS_DIR", &launch_agents_dir);
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        fs::create_dir_all(&launch_agents_dir).expect("create launch agents dir");
        fs::write(
            launch_agents_dir.join(format!("{MACOS_LABEL}.plist")),
            "<plist>outdated</plist>",
        )
        .expect("write mismatched plist");

        let status =
            schedule_status(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("status");

        restore_env_var("CHB_TEST_LAUNCH_AGENTS_DIR", original_launch_agents.as_deref());

        assert_eq!(status.install_state, "mismatch");
        assert!(status.warnings.iter().any(|warning| warning.contains("no longer matches")));
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
    fn non_macos_remove_is_manual_only() {
        let dir = tempdir().expect("tempdir");
        let result = remove_schedule(
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
        .expect("remove");

        assert!(!result.applied);
        assert!(result.message.contains("Manual"));
    }

    #[test]
    fn macos_apply_schedule_writes_files_and_audit_report() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os("CHB_TEST_LAUNCH_AGENTS_DIR");
        let original_launchctl_success = std::env::var_os("CHB_TEST_LAUNCHCTL_SUCCESS");
        unsafe {
            std::env::set_var("CHB_TEST_LAUNCH_AGENTS_DIR", &launch_agents_dir);
            std::env::set_var("CHB_TEST_LAUNCHCTL_SUCCESS", "1");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = apply_schedule(&plan, &paths).expect("apply macos plan");

        restore_env_var("CHB_TEST_LAUNCH_AGENTS_DIR", original_launch_agents.as_deref());
        restore_env_var("CHB_TEST_LAUNCHCTL_SUCCESS", original_launchctl_success.as_deref());

        assert!(result.applied);
        assert_eq!(result.files.len(), 1);
        assert!(Path::new(&result.files[0]).exists());
        assert!(Path::new(result.audit_path.as_deref().expect("audit path")).exists());
    }

    #[test]
    fn macos_apply_schedule_reports_bootstrap_failures_without_erroring() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os("CHB_TEST_LAUNCH_AGENTS_DIR");
        let original_launchctl_success = std::env::var_os("CHB_TEST_LAUNCHCTL_SUCCESS");
        unsafe {
            std::env::set_var("CHB_TEST_LAUNCH_AGENTS_DIR", &launch_agents_dir);
            std::env::set_var("CHB_TEST_LAUNCHCTL_SUCCESS", "0");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let result = apply_schedule(&plan, &paths).expect("apply failing macos plan");

        restore_env_var("CHB_TEST_LAUNCH_AGENTS_DIR", original_launch_agents.as_deref());
        restore_env_var("CHB_TEST_LAUNCHCTL_SUCCESS", original_launchctl_success.as_deref());

        assert!(!result.applied);
        assert!(result.message.contains("did not report success"));
    }

    #[test]
    fn macos_remove_schedule_removes_files_and_writes_audit_report() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let launch_agents_dir = dir.path().join("LaunchAgents");
        let original_launch_agents = std::env::var_os("CHB_TEST_LAUNCH_AGENTS_DIR");
        let original_launchctl_success = std::env::var_os("CHB_TEST_LAUNCHCTL_SUCCESS");
        unsafe {
            std::env::set_var("CHB_TEST_LAUNCH_AGENTS_DIR", &launch_agents_dir);
            std::env::set_var("CHB_TEST_LAUNCHCTL_SUCCESS", "1");
        }

        let paths = sample_paths(dir.path());
        let params = ScheduleParameters { due_after_hours: 72, check_interval_hours: 6 };
        let plan =
            preview_schedule(Some("macos"), Path::new("/tmp/chb"), &paths, &params).expect("plan");
        let applied = apply_schedule(&plan, &paths).expect("apply macos plan");
        assert!(applied.applied);

        let removed = remove_schedule(&plan, &paths).expect("remove macos plan");

        restore_env_var("CHB_TEST_LAUNCH_AGENTS_DIR", original_launch_agents.as_deref());
        restore_env_var("CHB_TEST_LAUNCHCTL_SUCCESS", original_launchctl_success.as_deref());

        assert!(removed.applied);
        assert_eq!(removed.files.len(), 1);
        assert!(!Path::new(&removed.files[0]).exists());
        assert!(Path::new(removed.audit_path.as_deref().expect("audit path")).exists());
    }

    #[test]
    fn macos_remove_schedule_rejects_missing_generated_files() {
        let dir = tempdir().expect("tempdir");
        let result = remove_schedule(
            &SchedulePlan {
                platform: "macos".to_string(),
                label: MACOS_LABEL.to_string(),
                executable_path: "/tmp/pathkeep".to_string(),
                generated_files: Vec::new(),
                manual_steps: Vec::new(),
                apply_commands: Vec::new(),
                rollback_commands: Vec::new(),
                apply_supported: true,
            },
            &sample_paths(dir.path()),
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing plist file"));
    }

    #[test]
    fn file_backed_test_keyring_roundtrips_secrets() {
        let _guard = env_lock().lock().expect("env lock");
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

        keyring_set_provider_api_key("openai-primary", "provider-secret").expect("set provider");
        assert_eq!(
            keyring_get_provider_api_key("openai-primary").expect("get provider"),
            Some("provider-secret".to_string())
        );
        assert!(provider_api_key_saved("openai-primary"));

        keyring_clear_database_key().expect("clear db");
        keyring_clear_s3_credentials().expect("clear s3");
        keyring_clear_provider_api_key("openai-primary").expect("clear provider");
        assert!(!provider_api_key_saved("openai-primary"));
        unsafe {
            std::env::remove_var(TEST_KEYRING_DIR_ENV);
        }
    }

    #[test]
    fn file_backed_test_keyring_handles_missing_entries_and_helpers() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        unsafe {
            std::env::set_var(TEST_KEYRING_DIR_ENV, dir.path());
        }

        assert_eq!(keyring_get_database_key().expect("missing db key"), None);
        assert_eq!(keyring_get_s3_credentials().expect("missing s3"), None);
        assert_eq!(
            keyring_get_provider_api_key("missing-provider").expect("missing provider"),
            None
        );
        assert!(
            test_keyring_path(dir.path(), "sample-user")
                .display()
                .to_string()
                .contains("sample-user")
        );
        keyring_clear_database_key().expect("clear empty db key");
        keyring_clear_s3_credentials().expect("clear empty s3 creds");
        keyring_clear_provider_api_key("missing-provider").expect("clear empty provider key");

        unsafe {
            std::env::remove_var(TEST_KEYRING_DIR_ENV);
        }
    }

    #[test]
    fn provider_keyring_user_and_file_backed_helpers_cover_extra_paths() {
        let _guard = env_lock().lock().expect("env lock");
        let dir = tempdir().expect("tempdir");
        let original_test_dir = std::env::var_os(TEST_KEYRING_DIR_ENV);
        unsafe {
            std::env::set_var(TEST_KEYRING_DIR_ENV, dir.path());
        }

        assert_eq!(provider_keyring_user("openai"), "ai-provider::openai");
        assert_eq!(keyring_status().backend, "File-backed test keyring");

        restore_env_var(TEST_KEYRING_DIR_ENV, original_test_dir.as_deref());
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_helpers_cover_default_keyring_root_and_restore_set_branch() {
        let _guard = env_lock().lock().expect("env lock");
        let original_test_dir = std::env::var_os(TEST_KEYRING_DIR_ENV);
        let seeded_value = std::ffi::OsString::from("/tmp/chb-platform-keyring");
        unsafe {
            std::env::remove_var(TEST_KEYRING_DIR_ENV);
        }

        let default_root = test_keyring_dir().expect("default keyring dir");
        assert!(default_root.to_string_lossy().contains("pathkeep-coverage-keyring"));

        restore_env_var(TEST_KEYRING_DIR_ENV, Some(seeded_value.as_os_str()));
        assert_eq!(std::env::var_os(TEST_KEYRING_DIR_ENV), Some(seeded_value));

        restore_env_var(TEST_KEYRING_DIR_ENV, original_test_dir.as_deref());
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
