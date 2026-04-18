//! Host launcher/file-manager adapter.
//!
//! These helpers validate user-provided paths/URLs before handing them to the
//! OS shell. The goal is modest but important: PathKeep should only ask the
//! platform to open real local folders, trusted `file://` URLs, or HTTP(S)
//! URLs and should fail with a straightforward message otherwise.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Opens a path in the host file manager and returns the opened target directory.
pub fn open_path_in_file_manager(path: String) -> Result<String, String> {
    let target = resolve_file_manager_target(&path)?;
    spawn_file_manager_for_target(&target)?;
    Ok(target.display().to_string())
}

/// Opens an HTTP(S) or trusted file URL in the host browser and returns the normalized URL.
pub fn open_external_url(url: String) -> Result<String, String> {
    let target = resolve_external_url_target(&url)?;
    let (program, arguments) = external_url_command(&target);
    spawn_external_url_command(program, arguments, &target)?;
    Ok(target)
}

/// Resolves a user-supplied path into the directory the file manager should open.
pub fn resolve_file_manager_target(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    let target = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir().map_err(|error| error.to_string())?.join(candidate)
    };

    if target.is_dir() {
        return Ok(target);
    }
    if target.is_file() {
        return target
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("Unable to open a parent directory for {}", target.display()));
    }
    if target.extension().is_some() {
        let mut current = target.parent();
        while let Some(parent) = current {
            if parent.is_dir() {
                return Ok(parent.to_path_buf());
            }
            current = parent.parent();
        }
    }

    Err(format!("Path does not exist: {}", target.display()))
}

/// Validates that a user-supplied URL is an HTTP(S) or file target we can hand to the shell.
pub fn resolve_external_url_target(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL does not exist.".to_string());
    }

    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("file://"))
    {
        return Err("Only file://, http://, and https:// URLs can be opened.".to_string());
    }

    Ok(trimmed.to_string())
}

#[cfg(target_os = "macos")]
/// Returns the macOS file-manager command and arguments for one target.
pub fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("open", vec![target.display().to_string()])
}

#[cfg(target_os = "windows")]
/// Returns the Windows file-manager command and arguments for one target.
pub fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("explorer", vec![target.display().to_string()])
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
/// Returns the Linux file-manager command and arguments for one target.
pub fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("xdg-open", vec![target.display().to_string()])
}

#[cfg(target_os = "macos")]
/// Returns the macOS browser-launch command and arguments for one URL.
pub fn external_url_command(target: &str) -> (&'static str, Vec<String>) {
    ("open", vec![target.to_string()])
}

#[cfg(target_os = "windows")]
/// Returns the Windows browser-launch command and arguments for one URL.
pub fn external_url_command(target: &str) -> (&'static str, Vec<String>) {
    ("cmd", vec!["/C".to_string(), "start".to_string(), String::new(), target.to_string()])
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
/// Returns the Linux browser-launch command and arguments for one URL.
pub fn external_url_command(target: &str) -> (&'static str, Vec<String>) {
    ("xdg-open", vec![target.to_string()])
}

/// Spawns the host file manager for a validated target path.
pub fn spawn_file_manager_for_target(target: &Path) -> Result<(), String> {
    let (program, arguments) = file_manager_command(target);
    spawn_file_manager_command(program, arguments, target)
}

fn spawn_file_manager_command(
    program: &str,
    arguments: Vec<String>,
    target: &Path,
) -> Result<(), String> {
    Command::new(program)
        .args(arguments)
        .spawn()
        .map_err(|error| format!("Failed to open {}: {error}", target.display()))?;
    Ok(())
}

fn spawn_external_url_command(
    program: &str,
    arguments: Vec<String>,
    target: &str,
) -> Result<(), String> {
    Command::new(program)
        .args(arguments)
        .spawn()
        .map_err(|error| format!("Failed to open {target}: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[cfg(target_os = "macos")]
    fn expected_file_manager_program() -> &'static str {
        "open"
    }

    #[cfg(target_os = "windows")]
    fn expected_file_manager_program() -> &'static str {
        "explorer"
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    fn expected_file_manager_program() -> &'static str {
        "xdg-open"
    }

    #[cfg(target_os = "macos")]
    fn expected_external_url_program() -> &'static str {
        "open"
    }

    #[cfg(target_os = "windows")]
    fn expected_external_url_program() -> &'static str {
        "cmd"
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    fn expected_external_url_program() -> &'static str {
        "xdg-open"
    }

    #[test]
    fn resolve_file_manager_target_prefers_directory_and_parent_folder() {
        let dir = tempdir().expect("tempdir");
        let nested_dir = dir.path().join("nested");
        fs::create_dir_all(&nested_dir).expect("create nested dir");
        let file_path = nested_dir.join("archive.sqlite");
        fs::write(&file_path, "sqlite").expect("write file");

        assert_eq!(
            resolve_file_manager_target(&nested_dir.display().to_string()).expect("resolve dir"),
            nested_dir
        );
        assert_eq!(
            resolve_file_manager_target(&file_path.display().to_string())
                .expect("resolve file parent"),
            nested_dir
        );
    }

    #[test]
    fn resolve_file_manager_target_rejects_missing_paths() {
        let error = resolve_file_manager_target("/tmp/pathkeep-does-not-exist")
            .expect_err("missing path should fail");
        assert!(error.contains("Path does not exist"));
    }

    #[test]
    fn resolve_external_url_target_accepts_file_http_and_https() {
        assert_eq!(
            resolve_external_url_target("file:///tmp/pathkeep/index.html").expect("file target"),
            "file:///tmp/pathkeep/index.html"
        );
        assert_eq!(
            resolve_external_url_target("https://example.com").expect("https target"),
            "https://example.com"
        );
        assert_eq!(
            resolve_external_url_target("http://example.com").expect("http target"),
            "http://example.com"
        );

        let unsupported =
            resolve_external_url_target("ftp://example.com").expect_err("reject ftp urls");
        assert!(unsupported.contains("file://, http://, and https://"));

        let missing = resolve_external_url_target("   ").expect_err("reject blank urls");
        assert!(missing.contains("does not exist"));
    }

    #[test]
    fn resolve_file_manager_target_allows_preview_file_paths_to_open_existing_parent_dir() {
        let dir = tempdir().expect("tempdir");
        let preview_file = dir.path().join("integrations/pathkeep-mcp.json");

        assert_eq!(
            resolve_file_manager_target(&preview_file.display().to_string())
                .expect("resolve preview file parent"),
            dir.path()
        );
    }

    #[test]
    fn launcher_helpers_cover_command_resolution() {
        let dir = tempdir().expect("tempdir");
        let relative_target =
            resolve_file_manager_target("src/lib.rs").expect("resolve relative file");
        assert!(relative_target.ends_with("src"));

        let (program, arguments) = file_manager_command(dir.path());
        assert_eq!(program, expected_file_manager_program());
        assert_eq!(arguments, vec![dir.path().display().to_string()]);

        let (program, arguments) = external_url_command("https://example.com/pathkeep");
        assert_eq!(program, expected_external_url_program());
        assert!(arguments.join(" ").contains("https://example.com/pathkeep"));
    }

    #[cfg(unix)]
    #[test]
    fn launcher_spawn_uses_host_commands_via_path_shim() {
        let dir = tempdir().expect("tempdir");
        let shim_dir = dir.path().join("bin");
        let capture_path = dir.path().join("capture.log");
        let target_dir = dir.path().join("open-me");
        fs::create_dir_all(&shim_dir).expect("create shim dir");
        fs::create_dir_all(&target_dir).expect("create target dir");

        let file_manager_shim = shim_dir.join(expected_file_manager_program());
        fs::write(
            &file_manager_shim,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n", capture_path.display()),
        )
        .expect("write file manager shim");
        let mut permissions = fs::metadata(&file_manager_shim).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&file_manager_shim, permissions).expect("chmod shim");

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

        open_path_in_file_manager(target_dir.display().to_string()).expect("open path");
        for _ in 0..50 {
            if capture_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let captured = fs::read_to_string(&capture_path).expect("read capture");

        unsafe {
            if let Some(value) = original_path {
                std::env::set_var("PATH", value);
            } else {
                std::env::remove_var("PATH");
            }
        }

        assert!(captured.contains(&target_dir.display().to_string()));
    }
}
