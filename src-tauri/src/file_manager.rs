use std::path::{Path, PathBuf};

#[cfg(not(test))]
use std::process::Command;

pub(crate) fn resolve_file_manager_target(path: &str) -> Result<PathBuf, String> {
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

pub(crate) fn resolve_external_url_target(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL does not exist.".to_string());
    }

    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("Only http:// and https:// URLs can be opened.".to_string());
    }

    Ok(trimmed.to_string())
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(target_os = "macos")]
pub(crate) fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("open", vec![target.display().to_string()])
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(target_os = "windows")]
pub(crate) fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("explorer", vec![target.display().to_string()])
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn file_manager_command(target: &Path) -> (&'static str, Vec<String>) {
    ("xdg-open", vec![target.display().to_string()])
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(target_os = "macos")]
pub(crate) fn external_url_command(target: &str) -> (&'static str, Vec<String>) {
    ("open", vec![target.to_string()])
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(target_os = "windows")]
pub(crate) fn external_url_command(target: &str) -> (&'static str, Vec<String>) {
    ("cmd", vec!["/C".to_string(), "start".to_string(), String::new(), target.to_string()])
}

#[cfg_attr(test, allow(dead_code))]
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
pub(crate) fn external_url_command(target: &str) -> (&'static str, Vec<String>) {
    ("xdg-open", vec![target.to_string()])
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn open_path_in_file_manager_impl(path: String) -> Result<String, String> {
    let target = resolve_file_manager_target(&path)?;
    spawn_file_manager_for_target(&target)?;
    Ok(target.display().to_string())
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn open_external_url_impl(url: String) -> Result<String, String> {
    let target = resolve_external_url_target(&url)?;
    let (program, arguments) = external_url_command(&target);
    spawn_external_url_command(program, arguments, &target)?;
    Ok(target)
}

pub(crate) fn spawn_file_manager_for_target(target: &Path) -> Result<(), String> {
    let (program, arguments) = file_manager_command(target);
    spawn_file_manager_command(program, arguments, target)
}

#[cfg(not(test))]
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

#[cfg(not(test))]
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
fn spawn_file_manager_command(
    program: &str,
    arguments: Vec<String>,
    target: &Path,
) -> Result<(), String> {
    if program.is_empty() || arguments.is_empty() || !target.exists() {
        return Err(format!("Failed to open {}", target.display()));
    }
    Ok(())
}

#[cfg(test)]
fn spawn_external_url_command(
    program: &str,
    arguments: Vec<String>,
    target: &str,
) -> Result<(), String> {
    if program.is_empty() || arguments.is_empty() || !target.starts_with("http") {
        return Err(format!("Failed to open {target}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
    fn resolve_external_url_target_accepts_http_and_https_only() {
        assert_eq!(
            resolve_external_url_target("https://example.com").expect("https target"),
            "https://example.com"
        );
        assert_eq!(
            resolve_external_url_target("http://example.com").expect("http target"),
            "http://example.com"
        );

        let unsupported =
            resolve_external_url_target("file:///tmp/example").expect_err("reject file urls");
        assert!(unsupported.contains("http:// and https://"));

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
    fn file_manager_helpers_cover_command_resolution_and_opening() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("archive.sqlite");
        fs::write(&file_path, "sqlite").expect("write file");

        let relative_target =
            resolve_file_manager_target("src/lib.rs").expect("resolve relative file");
        assert!(relative_target.ends_with("src"));

        let (program, arguments) = file_manager_command(dir.path());
        assert_eq!(program, expected_file_manager_program());
        assert_eq!(arguments, vec![dir.path().display().to_string()]);

        assert_eq!(
            open_path_in_file_manager_impl(file_path.display().to_string())
                .expect("open file manager"),
            dir.path().display().to_string()
        );
        spawn_file_manager_for_target(dir.path()).expect("spawn file manager");

        let (external_program, external_arguments) = external_url_command("https://example.com");
        assert_eq!(external_program, expected_external_url_program());
        assert!(!external_arguments.is_empty());
        assert_eq!(
            open_external_url_impl("https://example.com".to_string()).expect("open external url"),
            "https://example.com"
        );
    }

    #[test]
    fn spawn_file_manager_command_rejects_invalid_invocations() {
        let dir = tempdir().expect("tempdir");
        let missing_target = dir.path().join("missing");

        let error = spawn_file_manager_command("", Vec::new(), &missing_target)
            .expect_err("invalid invocation should fail");
        assert!(error.contains("Failed to open"));
    }

    #[test]
    fn spawn_external_url_command_rejects_invalid_invocations() {
        let error = spawn_external_url_command("", Vec::new(), "mailto:test@example.com")
            .expect_err("invalid invocation should fail");
        assert!(error.contains("Failed to open"));
    }
}
