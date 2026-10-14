use std::{
    fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::tempdir;
use vault_core::ProjectPaths;
#[cfg(target_os = "macos")]
use vault_platform::test_support::TEST_LAUNCH_AGENTS_DIR_ENV;
use vault_platform::test_support::{TEST_KEYRING_SERVICE_ENV, TEST_SCHEDULE_LABEL_ENV};
use vault_platform::{
    ScheduleParameters, keyring_clear_database_key, keyring_clear_provider_api_key,
    keyring_get_database_key, keyring_get_provider_api_key, keyring_set_database_key,
    keyring_set_provider_api_key, keyring_status, preview_schedule,
};
#[cfg(target_os = "macos")]
use vault_platform::{apply_schedule, remove_schedule, schedule_status};

fn unique_suffix() -> String {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).expect("system time").as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn sample_paths(root: &Path) -> ProjectPaths {
    vault_core::config::project_paths_with_root(root)
}

fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    unsafe {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}

fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

fn host_denied(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("operation not permitted")
        || normalized.contains("permission denied")
        || normalized.contains("bootstrap failed: 5")
        || normalized.contains("input/output error")
        || normalized.contains("secret service: no result found")
        // gnome-keyring on a headless box returns this when the default
        // collection isn't unlocked (gcr-prompter has no display to ask
        // the user). It's the post-timeout "give up" path for the same
        // dev-VM scenario the wall-clock budget guards against.
        || normalized.contains("locked collection")
}

fn native_keyring_roundtrip(service: &str) {
    let _guard = env_lock().lock().expect("env lock");
    let original_service = std::env::var_os(TEST_KEYRING_SERVICE_ENV);
    unsafe {
        std::env::set_var(TEST_KEYRING_SERVICE_ENV, service);
    }

    // Wall-clock cap on the whole roundtrip. On headless Linux dev VMs
    // (XDG_SESSION_TYPE=tty, DISPLAY set but unreachable),
    // gnome-keyring-daemon activates on first secret-service write and
    // spawns gcr-prompter to ask the user to unlock the default
    // collection. The prompter dies trying to open the display, but the
    // daemon waits for it for ~2 hours before returning an error — long
    // enough to wreck any reasonable per-commit gate. macOS Keychain and
    // Windows Credential Manager both finish their roundtrip in well
    // under a second, so 30 s is a generous upper bound for a healthy
    // backend.
    let result = run_keyring_body_with_timeout(std::time::Duration::from_secs(30));

    restore_env(TEST_KEYRING_SERVICE_ENV, original_service);

    match result {
        Some(Ok(())) => {}
        Some(Err(error)) if host_denied(&error.to_string()) => {
            eprintln!(
                "Skipping native keyring smoke because the host denied secure-store access: {error:#}"
            );
        }
        Some(Err(error)) => panic!("native keyring roundtrip failed: {error:#}"),
        None => {
            eprintln!(
                "Skipping native keyring smoke: native backend did not complete within 30s — likely a headless host where gnome-keyring-daemon is waiting on an unreachable unlock prompt."
            );
        }
    }
}

/// Runs the roundtrip body on a detached worker thread and waits up to
/// `budget` for it to finish. Returns `None` if the budget elapses (the
/// worker keeps running and is reaped when the test binary exits — fine
/// for an integration-only test). The caller still holds `env_lock` and
/// `TEST_KEYRING_SERVICE_ENV` for the full budget, which serializes
/// against the other tests here that change the environment.
fn run_keyring_body_with_timeout(budget: std::time::Duration) -> Option<anyhow::Result<()>> {
    use std::sync::mpsc;

    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(keyring_roundtrip_body());
    });
    receiver.recv_timeout(budget).ok()
}

fn keyring_roundtrip_body() -> anyhow::Result<()> {
    keyring_clear_database_key()?;
    keyring_clear_provider_api_key("integration-openai")?;

    let status = keyring_status();
    if !status.available {
        anyhow::bail!("native keyring backend is unavailable on this host");
    }

    keyring_set_database_key("native-db-secret")?;
    assert_eq!(keyring_get_database_key()?, Some("native-db-secret".to_string()));

    keyring_set_provider_api_key("integration-openai", "provider-secret")?;
    assert_eq!(
        keyring_get_provider_api_key("integration-openai")?,
        Some("provider-secret".to_string())
    );

    keyring_clear_database_key()?;
    keyring_clear_provider_api_key("integration-openai")?;
    assert_eq!(keyring_get_database_key()?, None);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "writes to the real OS secret store of this machine; run by check:platform"]
fn macos_keychain_roundtrip_uses_a_unique_service_namespace() {
    native_keyring_roundtrip(&format!("com.yi-ting.pathkeep.tests.{}", unique_suffix()));
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
#[ignore = "writes to the real OS secret store of this machine; run by check:platform"]
fn linux_secret_service_roundtrip_uses_a_unique_service_namespace() {
    native_keyring_roundtrip(&format!("com.yi-ting.pathkeep.tests.{}", unique_suffix()));
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "writes to the real OS secret store of this machine; run by check:platform"]
fn windows_credential_manager_roundtrip_uses_a_unique_service_namespace() {
    native_keyring_roundtrip(&format!("com.yi-ting.pathkeep.tests.{}", unique_suffix()));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "installs a real scheduler entry on this machine; run by check:platform"]
fn macos_scheduler_apply_bootstrap_status_and_cleanup_work() {
    let _guard = env_lock().lock().expect("env lock");
    let dir = tempdir().expect("tempdir");
    let launch_agents_dir = dir.path().join("LaunchAgents");
    let label = format!("com.yi-ting.pathkeep.tests.{}", unique_suffix());
    let original_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
    let original_launch_agents = std::env::var_os(TEST_LAUNCH_AGENTS_DIR_ENV);
    unsafe {
        std::env::set_var(TEST_SCHEDULE_LABEL_ENV, &label);
        std::env::set_var(TEST_LAUNCH_AGENTS_DIR_ENV, &launch_agents_dir);
    }

    let paths = sample_paths(dir.path());
    let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
    let plan =
        preview_schedule(Some("macos"), Path::new("/usr/bin/true"), &paths, &params).expect("plan");

    let plist_path = launch_agents_dir.join(format!("{label}.plist"));
    fs::create_dir_all(&launch_agents_dir).expect("launch agents dir");
    fs::write(&plist_path, &plan.generated_files[0].contents).expect("write plist");
    let lint = Command::new("plutil")
        .args(["-lint", plist_path.to_str().expect("plist path")])
        .status()
        .expect("plutil -lint");
    assert!(lint.success(), "expected plutil -lint to accept generated plist");

    let applied = apply_schedule(&plan, &paths).expect("apply schedule");
    if !applied.applied {
        restore_env(TEST_SCHEDULE_LABEL_ENV, original_label);
        restore_env(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents);
        assert!(
            host_denied(&applied.message),
            "expected launchctl bootstrap to succeed or fail with an explicit host-denied reason, got: {}",
            applied.message
        );
        eprintln!(
            "Skipping launchctl bootstrap verification because the host denied LaunchAgent install: {}",
            applied.message
        );
        return;
    }

    let uid = String::from_utf8(Command::new("id").arg("-u").output().expect("id -u").stdout)
        .expect("utf8 uid")
        .trim()
        .to_string();
    let status = Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{label}")])
        .status()
        .expect("launchctl print");
    assert!(status.success(), "expected launchctl print to find installed label");

    let detected = schedule_status(Some("macos"), Path::new("/usr/bin/true"), &paths, &params)
        .expect("status");
    assert_eq!(detected.install_state, "installed");

    let removed = remove_schedule(&plan, &paths).expect("remove schedule");
    assert!(removed.applied);

    restore_env(TEST_SCHEDULE_LABEL_ENV, original_label);
    restore_env(TEST_LAUNCH_AGENTS_DIR_ENV, original_launch_agents);
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
#[ignore = "runs the real scheduler tools of this host; run by check:platform"]
fn linux_scheduler_artifacts_validate_with_systemd_analyze() {
    let _guard = env_lock().lock().expect("env lock");
    let dir = tempdir().expect("tempdir");
    let label = format!("com.yi-ting.pathkeep.tests.{}", unique_suffix());
    let original_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
    unsafe {
        std::env::set_var(TEST_SCHEDULE_LABEL_ENV, &label);
    }

    let paths = sample_paths(dir.path());
    let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
    let plan =
        preview_schedule(Some("linux"), Path::new("/usr/bin/true"), &paths, &params).expect("plan");
    let service_path = dir.path().join(format!("{label}.service"));
    let timer_path = dir.path().join(format!("{label}.timer"));
    fs::write(&service_path, &plan.generated_files[0].contents).expect("write service");
    fs::write(&timer_path, &plan.generated_files[1].contents).expect("write timer");

    let verify = Command::new("systemd-analyze")
        .args([
            "verify",
            service_path.to_str().expect("service path"),
            timer_path.to_str().expect("timer path"),
        ])
        .status()
        .expect("systemd-analyze verify");

    restore_env(TEST_SCHEDULE_LABEL_ENV, original_label);

    assert!(verify.success(), "expected systemd-analyze verify to accept generated units");
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "runs the real scheduler tools of this host; run by check:platform"]
fn windows_scheduler_xml_validates_with_schtasks() {
    let _guard = env_lock().lock().expect("env lock");
    let dir = tempdir().expect("tempdir");
    let label = format!("com.yi-ting.pathkeep.tests.{}", unique_suffix());
    let original_label = std::env::var_os(TEST_SCHEDULE_LABEL_ENV);
    unsafe {
        std::env::set_var(TEST_SCHEDULE_LABEL_ENV, &label);
    }

    let paths = sample_paths(dir.path());
    let params = ScheduleParameters { due_after_hours: 72.0, check_interval_hours: 6.0 };
    let plan = preview_schedule(
        Some("windows"),
        Path::new("C:\\Windows\\System32\\cmd.exe"),
        &paths,
        &params,
    )
    .expect("plan");
    let xml_path = dir.path().join(format!("{label}.xml"));
    fs::write(&xml_path, &plan.generated_files[0].contents).expect("write xml");

    let create = Command::new("schtasks")
        .args(["/Create", "/TN", &label, "/XML", xml_path.to_str().expect("xml path"), "/F"])
        .status()
        .expect("schtasks /Create");
    assert!(create.success(), "expected schtasks /Create to accept XML");

    let delete = Command::new("schtasks")
        .args(["/Delete", "/TN", &label, "/F"])
        .status()
        .expect("schtasks /Delete");

    restore_env(TEST_SCHEDULE_LABEL_ENV, original_label);

    assert!(delete.success(), "expected schtasks /Delete to clean up test task");
}
