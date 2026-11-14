//! Responsibilities: exercise development keyring, login and scheduler paths in real file seams.
//! Not responsible for: touching the installed app or proving native OS registration on CI hosts.
use std::{ffi::OsString, fs, sync::Mutex};
static ENV: Mutex<()> = Mutex::new(());
struct Sandbox {
    root: tempfile::TempDir,
    saved: Vec<(&'static str, Option<OsString>)>,
}
impl Sandbox {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let values = [
            ("CHB_PROJECT_ROOT", Some(root.path().join("data").into_os_string())),
            ("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", Some(root.path().join("os").into_os_string())),
            (
                "PATHKEEP_PLATFORM_TEST_KEYRING_DIR",
                Some(root.path().join("keyring").into_os_string()),
            ),
            ("PATHKEEP_PLATFORM_TEST_KEYRING_SERVICE", None),
            ("PATHKEEP_PLATFORM_TEST_SCHEDULE_LABEL", None),
            ("PATHKEEP_PLATFORM_TEST_WINDOWS_USER_ID", Some("Fixture\\User".into())),
        ];
        let saved = values.iter().map(|(k, _)| (*k, std::env::var_os(k))).collect();
        for (key, value) in values {
            unsafe {
                if let Some(v) = value {
                    std::env::set_var(key, v)
                } else {
                    std::env::remove_var(key)
                }
            }
        }
        Self { root, saved }
    }
    fn paths(&self) -> vault_core::ProjectPaths {
        vault_core::project_paths_with_root(&self.root.path().join("data"))
    }
    fn plan(&self, platform: &str) -> vault_core::SchedulePlan {
        vault_platform::preview_schedule(
            Some(platform),
            &self.root.path().join("pathkeep-worker"),
            &self.paths(),
            &vault_platform::ScheduleParameters { due_after_hours: 24., check_interval_hours: 6. },
        )
        .unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        for (k, v) in &self.saved {
            unsafe {
                if let Some(v) = v { std::env::set_var(k, v) } else { std::env::remove_var(k) }
            }
        }
    }
}

#[test]
fn development_key_operations_do_not_read_or_clear_the_production_service() {
    let _lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let s = Sandbox::new();
    let keys = s.root.path().join("keyring");
    fs::create_dir_all(&keys).unwrap();
    let prod = keys.join("com.yi-ting.pathkeep-database-key.secret");
    let provider = keys.join("com.yi-ting.pathkeep-ai-provider::fixture.secret");
    fs::write(&prod, "production archive key").unwrap();
    fs::write(&provider, "production provider key").unwrap();
    assert_eq!(vault_platform::keyring_get_database_key().unwrap(), None);
    assert_eq!(vault_platform::keyring_get_provider_api_key("fixture").unwrap(), None);
    vault_platform::keyring_set_database_key("development key").unwrap();
    vault_platform::keyring_set_provider_api_key("fixture", "development provider").unwrap();
    vault_platform::keyring_clear_database_key().unwrap();
    vault_platform::keyring_clear_provider_api_key("fixture").unwrap();
    assert_eq!(fs::read_to_string(prod).unwrap(), "production archive key");
    assert_eq!(fs::read_to_string(provider).unwrap(), "production provider key");
}

#[test]
fn login_item_changes_leave_the_production_registration_untouched() {
    let _lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let s = Sandbox::new();
    let folder = s.root.path().join("os/login-items");
    fs::create_dir_all(&folder).unwrap();
    let prod = folder.join("com.yi-ting.pathkeep.login.plist");
    fs::write(&prod, "production login").unwrap();
    let store = vault_platform::login_item_store().unwrap().unwrap();
    assert_eq!(store.plist_path(), folder.join("com.yi-ting.pathkeep.dev.login.plist"));
    assert!(!store.is_enabled());
    store.enable(&s.root.path().join("development-app")).unwrap();
    store.refresh(&s.root.path().join("moved-development-app")).unwrap();
    store.disable().unwrap();
    assert_eq!(fs::read_to_string(prod).unwrap(), "production login");
}

#[test]
fn schedule_plans_and_removal_cannot_reuse_a_production_label_or_plist() {
    let _lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let s = Sandbox::new();
    for platform in ["macos", "windows", "linux"] {
        let plan = s.plan(platform);
        assert_eq!(plan.label, "com.yi-ting.pathkeep.dev.backup");
        assert!(
            plan.generated_files
                .iter()
                .all(|f| f.relative_path.contains("com.yi-ting.pathkeep.dev.backup"))
        );
        if platform == "linux" {
            continue;
        }
        let mut old = plan.clone();
        old.label = "com.yi-ting.pathkeep.backup".into();
        assert!(
            vault_platform::apply_schedule(&old, &s.paths())
                .unwrap_err()
                .to_string()
                .contains("schedule-identity-mismatch")
        );
        assert!(
            vault_platform::remove_schedule(&old, &s.paths())
                .unwrap_err()
                .to_string()
                .contains("schedule-identity-mismatch")
        );
        if platform == "macos" {
            assert!(vault_platform::repair_schedule(&old, &s.paths()).is_err());
            let mut disguised = plan.clone();
            disguised.generated_files[0].relative_path =
                "launchd/com.yi-ting.pathkeep.backup.plist".into();
            assert!(vault_platform::apply_schedule(&disguised, &s.paths()).is_err());
            let mut payload = plan.clone();
            payload.generated_files[0].contents = payload.generated_files[0].contents.replace(
                "<string>com.yi-ting.pathkeep.dev.backup</string>",
                "<string>com.yi-ting.pathkeep.backup</string>",
            );
            assert!(vault_platform::apply_schedule(&payload, &s.paths()).is_err());
        }
    }
}

#[test]
fn development_status_and_repair_ignore_production_legacy_schedules() {
    let _lock = ENV.lock().unwrap_or_else(|e| e.into_inner());
    let s = Sandbox::new();
    let folder = s.root.path().join("os/LaunchAgents");
    fs::create_dir_all(&folder).unwrap();
    let legacy = folder.join("dev.codex.pathkeep.backup.plist");
    fs::write(&legacy, "production legacy schedule").unwrap();
    let plan = s.plan("macos");
    let status = vault_platform::schedule_status(
        Some("macos"),
        &s.root.path().join("pathkeep-worker"),
        &s.paths(),
        &vault_platform::ScheduleParameters { due_after_hours: 24., check_interval_hours: 6. },
    )
    .unwrap();
    assert!(!status.detected_files.iter().any(|p| p.contains("dev.codex.pathkeep.backup")));
    vault_platform::repair_schedule(&plan, &s.paths()).unwrap();
    assert_eq!(fs::read_to_string(legacy).unwrap(), "production legacy schedule");
}
