//! Responsibilities: isolated process-test archive, environment and result artifact.
//! Not responsible for: replacing the app, scheduler or worker implementation.
//! Failure modes: tests access developer data/LaunchAgents; orphan owner processes;
//! unrepeatable evidence. Used by owner_process.rs.
#![allow(dead_code)]
use std::{path::PathBuf, process::Command};
use vault_core::{AppConfig, ArchiveMode};
pub struct Fixture {
    pub root: tempfile::TempDir,
    executable: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        // A short root keeps the owner's socket path under the Unix limit; macOS's
        // own temp directory is too deep for it.
        let base =
            if cfg!(target_os = "macos") { "/private/tmp".into() } else { std::env::temp_dir() };
        let root = tempfile::Builder::new().prefix("pk-owner-").tempdir_in(base).unwrap();
        let paths = vault_core::project_paths_with_root(root.path());
        vault_core::config::ensure_paths(&paths).unwrap();
        let mut config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            git_enabled: false,
            selected_profile_ids: vec!["chrome:Default".into()],
            due_after_hours: 72.0,
            ..Default::default()
        };
        config.og_image.fetch_enabled = false;
        config.ai.enabled = true;
        config.ai.mcp_enabled = true;
        config.ai.skill_enabled = true;
        vault_core::save_config(&paths, &config).unwrap();
        vault_core::ensure_archive_initialized(&paths, &config, None).unwrap();
        let browser = root.path().join("chrome/Default");
        std::fs::create_dir_all(&browser).unwrap();
        let fixture = browser_history_fixtures::chromium::ChromiumHistoryFixture::new()
            .add_url(browser_history_fixtures::chromium::ChromiumUrlRow {
                id: 1,
                url: "https://example.test/owner".into(),
                title: Some("Owner process".into()),
                visit_count: 1,
                typed_count: 0,
                last_visit_unix_ms: 1_800_000_000_000,
                hidden: false,
            })
            .add_visit(browser_history_fixtures::chromium::ChromiumVisitRow {
                id: 1,
                url_id: 1,
                visit_time_unix_ms: 1_800_000_000_000,
                from_visit: None,
                transition: Some(1),
                visit_duration_micros: Some(1000),
                is_known_to_sync: false,
                visited_link_id: None,
                external_referrer_url: None,
                app_id: None,
            });
        fixture.write(&browser.join("History")).unwrap();
        let executable = bundle_executable(root.path());
        Self { root, executable }
    }
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        command
            .env("CHB_PROJECT_ROOT", self.root.path())
            .env("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", self.root.path().join("platform"))
            .env("CHB_TEST_KEYRING_DIR", self.root.path().join("keyring"))
            .env("CHB_CHROME_USER_DATA_DIR", self.root.path().join("chrome"))
            .env("CHB_FIREFOX_PROFILES_DIR", self.root.path().join("no-firefox"))
            .env("CHB_SAFARI_ROOT", self.root.path().join("no-safari"));
        command
    }
    pub fn database_denied_command(&self) -> Command {
        #[cfg(target_os = "macos")]
        {
            let base = self.command();
            let mut command = Command::new("/usr/bin/sandbox-exec");
            let profile = format!(
                "(version 1)(allow default)(deny file-read* file-write* (subpath \"{}/archive\") (subpath \"{}/derived\"))",
                self.root.path().display(),
                self.root.path().display()
            );
            command.args(["-p", &profile]).arg(base.get_program());
            for (key, value) in base.get_envs() {
                if let Some(value) = value {
                    command.env(key, value);
                }
            }
            command
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.command()
        }
    }
    pub fn artifact(&self, value: serde_json::Value) {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/owner-process");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("result.json"), serde_json::to_vec_pretty(&value).unwrap())
            .unwrap();
    }
}
// This fixture links vault-store, which requires Rust 1.96 for native file locks.
#[clippy::msrv = "1.96"]
impl Drop for Fixture {
    fn drop(&mut self) {
        let path = self.root.path().join("archive/.pk-owner.lock");
        let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(&path) else {
            return;
        };
        if file.try_lock().is_ok() {
            return;
        }
        let Some(pid) =
            std::fs::read_to_string(&path).ok().and_then(|text| text.trim().parse::<u32>().ok())
        else {
            return;
        };
        if pid == std::process::id() {
            return;
        }
        let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while file.try_lock().is_err() {
            if std::time::Instant::now() >= deadline {
                let _ = Command::new("kill").args(["-KILL", &pid.to_string()]).status();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
pub fn open_files(pid: u32) -> String {
    let output = Command::new("lsof").args(["-p", &pid.to_string(), "-Fn"]).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

/// The macOS cold-start test uses the same Launch Services path as an installed app.
fn bundle_executable(root: &std::path::Path) -> PathBuf {
    if let Some(bundle) = std::env::var_os("PATHKEEP_OWNER_TEST_APP") {
        return PathBuf::from(bundle).join("Contents/MacOS/pathkeep-desktop");
    }
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_pathkeep-desktop"));
    if !cfg!(target_os = "macos") {
        return executable;
    }
    let contents = root.join("PathKeep Test.app/Contents");
    let binary = contents.join("MacOS/pathkeep-desktop");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::hard_link(executable, &binary).unwrap();
    std::fs::write(contents.join("Info.plist"), r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>pathkeep-desktop</string>
<key>CFBundleIdentifier</key><string>com.yi-ting.pathkeep.owner-test</string>
<key>CFBundleName</key><string>PathKeep Owner Test</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
</dict></plist>"#).unwrap();
    binary
}
