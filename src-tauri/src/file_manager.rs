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

    Err(format!("Path does not exist: {}", target.display()))
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
pub(crate) fn open_path_in_file_manager_impl(path: String) -> Result<String, String> {
    let target = resolve_file_manager_target(&path)?;
    spawn_file_manager_for_target(&target)?;
    Ok(target.display().to_string())
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
    }

    #[test]
    fn spawn_file_manager_command_rejects_invalid_invocations() {
        let dir = tempdir().expect("tempdir");
        let missing_target = dir.path().join("missing");

        let error = spawn_file_manager_command("", Vec::new(), &missing_target)
            .expect_err("invalid invocation should fail");
        assert!(error.contains("Failed to open"));
    }
}
