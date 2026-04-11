//! Tiny shell-launcher wrappers for the desktop support commands.
//!
//! The command facade keeps file-manager/URL opening isolated here so tests can
//! exercise the wrapper without pulling in the whole command module.

/// Opens one local path through the platform launcher and returns the same path on success.
pub(crate) fn open_path_in_file_manager_impl(path: String) -> Result<String, String> {
    vault_platform::open_path_in_file_manager(path)
}

/// Opens one external HTTP(S) URL through the platform launcher and returns it on success.
pub(crate) fn open_external_url_impl(url: String) -> Result<String, String> {
    vault_platform::open_external_url(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env() -> MutexGuard<'static, ()> {
        env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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

    #[test]
    fn wrapper_surfaces_validation_errors() {
        let error = open_path_in_file_manager_impl("/tmp/pathkeep-does-not-exist".to_string())
            .expect_err("missing path should fail");
        assert!(error.contains("Path does not exist"));

        let error = open_external_url_impl("file:///tmp/pathkeep".to_string())
            .expect_err("file urls should fail");
        assert!(error.contains("http:// and https://"));
    }

    #[cfg(unix)]
    #[test]
    fn wrapper_opens_targets_through_platform_launcher() {
        let _env_lock = lock_env();
        let dir = tempdir().expect("tempdir");
        let shim_dir = dir.path().join("bin");
        let target_dir = dir.path().join("target");
        let capture_path = dir.path().join("capture.log");
        fs::create_dir_all(&shim_dir).expect("create shim dir");
        fs::create_dir_all(&target_dir).expect("create target dir");

        #[cfg(target_os = "macos")]
        let program = "open";
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        let program = "xdg-open";

        let shim_path = shim_dir.join(program);
        fs::write(
            &shim_path,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" >> {}\n", capture_path.display()),
        )
        .expect("write shim");
        let mut permissions = fs::metadata(&shim_path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim_path, permissions).expect("chmod shim");

        let original_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var(
                "PATH",
                format!(
                    "{}:{}",
                    shim_dir.display(),
                    original_path
                        .as_ref()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ),
            );
        }

        let opened =
            open_path_in_file_manager_impl(target_dir.display().to_string()).expect("open path");
        let opened_url =
            open_external_url_impl("https://example.com/pathkeep".to_string()).expect("open url");
        for _ in 0..50 {
            if capture_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let captured = fs::read_to_string(&capture_path).expect("read capture");

        restore_env_var("PATH", original_path.as_deref());

        assert_eq!(opened, target_dir.display().to_string());
        assert_eq!(opened_url, "https://example.com/pathkeep");
        assert!(captured.contains(&target_dir.display().to_string()));
        assert!(captured.contains("https://example.com/pathkeep"));
    }

    #[test]
    fn restore_env_var_handles_missing_and_present_values() {
        let _env_lock = lock_env();
        let env_name = "PATHKEEP_FILE_MANAGER_TEST_ENV";

        unsafe {
            std::env::set_var(env_name, "present");
        }
        restore_env_var(env_name, None);
        assert!(std::env::var_os(env_name).is_none());

        restore_env_var(env_name, Some(OsStr::new("restored")));
        assert_eq!(std::env::var(env_name).expect("restored env value"), "restored");

        restore_env_var(env_name, None);
        assert!(std::env::var_os(env_name).is_none());
    }
}
