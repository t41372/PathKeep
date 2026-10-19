//! Host launcher/file-manager adapter.
//!
//! These helpers validate user-provided paths/URLs before handing them to the
//! OS shell. The goal is modest but important: PathKeep should only ask the
//! platform to open real local folders, HTTP(S) URLs, one fixed macOS settings
//! pane, or a `file://` URL the caller has checked is a page the user visited,
//! and should fail with a straightforward message otherwise. Opening a
//! `file://` URL can launch an application, so it never accepts one on its own.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Fixed macOS System Settings target for granting Safari History.db access.
pub const MACOS_FULL_DISK_ACCESS_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// Opens a path in the host file manager and returns the opened target directory.
pub fn open_path_in_file_manager(path: String) -> Result<String, String> {
    let target = resolve_file_manager_target(&path)?;
    spawn_file_manager_for_target(&target)?;
    Ok(target.display().to_string())
}

/// Opens an HTTP(S) URL, or the approved settings URL, in the host launcher.
pub fn open_external_url(url: String) -> Result<String, String> {
    let target = resolve_external_url_target(&url)?;
    let (program, arguments) = external_url_command(&target);
    spawn_external_url_command(program, arguments, &target)?;
    Ok(target)
}

/// Opens a `file://` URL. Only for one the caller found in the user's own history: anything else
/// (a link a model wrote, a page's content) could name an application and run it.
pub fn open_visited_file_url(url: String) -> Result<String, String> {
    let target = url.trim().to_string();
    if !target.to_ascii_lowercase().starts_with("file://") {
        return Err(format!("Not a file:// URL: {target}"));
    }
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

/// Validates that a user-supplied URL is a trusted launcher target.
pub fn resolve_external_url_target(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL does not exist.".to_string());
    }

    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("https://")
        || lower.starts_with("http://")
        || is_allowed_macos_settings_url(trimmed))
    {
        return Err(
            "Only http://, https://, and the macOS Full Disk Access settings URL can be opened."
                .to_string(),
        );
    }

    Ok(trimmed.to_string())
}

fn is_allowed_macos_settings_url(candidate: &str) -> bool {
    candidate.eq_ignore_ascii_case(MACOS_FULL_DISK_ACCESS_SETTINGS_URL)
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

    #[test]
    fn resolve_external_url_target_accepts_trusted_launcher_urls() {
        // A file:// URL can name an application; only visited ones open, through
        // open_visited_file_url after the caller has looked them up.
        resolve_external_url_target("file:///Applications/Calculator.app")
            .expect_err("reject file urls");
        assert_eq!(
            resolve_external_url_target("https://example.com").expect("https target"),
            "https://example.com"
        );
        assert_eq!(
            resolve_external_url_target("http://example.com").expect("http target"),
            "http://example.com"
        );
        assert_eq!(
            resolve_external_url_target(MACOS_FULL_DISK_ACCESS_SETTINGS_URL)
                .expect("Full Disk Access settings target"),
            MACOS_FULL_DISK_ACCESS_SETTINGS_URL
        );

        let unsupported =
            resolve_external_url_target("ftp://example.com").expect_err("reject ftp urls");
        assert!(unsupported.contains("macOS Full Disk Access settings URL"));

        let missing = resolve_external_url_target("   ").expect_err("reject blank urls");
        assert!(missing.contains("does not exist"));
    }
}
