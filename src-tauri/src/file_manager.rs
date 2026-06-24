//! Tiny shell-launcher wrappers for the desktop support commands.
//!
//! The command facade keeps file-manager/URL opening isolated here so tests can
//! exercise the wrapper without pulling in the whole command module.

/// Opens one local path through the platform launcher and returns the same path on success.
pub(crate) fn open_path_in_file_manager_impl(path: String) -> Result<String, String> {
    vault_platform::open_path_in_file_manager(path)
}

/// Opens one trusted launcher URL through the platform launcher and returns it on success.
pub(crate) fn open_external_url_impl(url: String) -> Result<String, String> {
    vault_platform::open_external_url(url)
}

/// Writes a UTF-8 text document to a user-chosen path and returns the byte count written.
///
/// Backs the AI assistant's "Export conversation" affordance: the frontend builds the Markdown /
/// JSON string off the main thread, the native save dialog returns the target path, and this
/// helper performs the actual disk write off the WebView thread (the command runs it on the
/// blocking pool). It refuses an empty path so a cancelled / malformed dialog never writes to the
/// process working directory, and surfaces the OS error verbatim so the UI can show it.
pub(crate) fn export_conversation_file_impl(
    target_path: String,
    contents: String,
) -> Result<u64, String> {
    if target_path.trim().is_empty() {
        return Err("Export path is empty".to_string());
    }
    std::fs::write(&target_path, contents.as_bytes())
        .map_err(|error| format!("Failed to write {target_path}: {error}"))?;
    Ok(contents.len() as u64)
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

    fn wait_for_capture(path: &std::path::Path, expected: &[&str]) -> String {
        let mut last_capture = String::new();
        for _ in 0..250 {
            if let Ok(captured) = fs::read_to_string(path) {
                if expected.iter().all(|value| captured.contains(value)) {
                    return captured;
                }
                last_capture = captured;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!(
            "launcher capture did not contain expected values {:?}; last capture: {:?}",
            expected, last_capture
        );
    }

    #[test]
    fn wrapper_surfaces_validation_errors() {
        let error = open_path_in_file_manager_impl("/tmp/pathkeep-does-not-exist".to_string())
            .expect_err("missing path should fail");
        assert!(error.contains("Path does not exist"));

        let error = open_external_url_impl("ftp://example.com/pathkeep".to_string())
            .expect_err("ftp urls should fail");
        assert!(error.contains("macOS Full Disk Access settings URL"));
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
        let captured = wait_for_capture(
            &capture_path,
            &[&target_dir.display().to_string(), "https://example.com/pathkeep"],
        );

        restore_env_var("PATH", original_path.as_deref());

        assert_eq!(opened, target_dir.display().to_string());
        assert_eq!(opened_url, "https://example.com/pathkeep");
        assert!(captured.contains(&target_dir.display().to_string()));
        assert!(captured.contains("https://example.com/pathkeep"));
    }

    #[test]
    fn export_conversation_file_writes_contents_and_returns_byte_count() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("conversation.md");
        let body = "# PathKeep conversation\n\nhello world\n";

        let written = export_conversation_file_impl(target.display().to_string(), body.to_string())
            .expect("write should succeed");

        assert_eq!(written, body.len() as u64);
        let read_back = fs::read_to_string(&target).expect("read back");
        assert_eq!(read_back, body);
    }

    #[test]
    fn export_conversation_file_rejects_empty_path() {
        let error = export_conversation_file_impl("   ".to_string(), "x".to_string())
            .expect_err("empty path should fail");
        assert!(error.contains("Export path is empty"));
    }

    #[test]
    fn export_conversation_file_surfaces_write_errors() {
        let dir = tempdir().expect("tempdir");
        // A path whose parent directory does not exist forces std::fs::write to fail, exercising
        // the error-mapping branch.
        let target = dir.path().join("missing-subdir").join("conversation.json");
        let error = export_conversation_file_impl(target.display().to_string(), "{}".to_string())
            .expect_err("write into a missing directory should fail");
        assert!(error.contains("Failed to write"));
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
