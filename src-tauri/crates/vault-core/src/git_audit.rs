//! Optional git-backed audit helper.
//!
//! Archive artifacts must be writable even when the host does not have Git.
//! This module therefore treats the audit directory as the durable surface and
//! the local Git history as an optional review layer on top of it.

use anyhow::{Context, Result};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Ensures the local audit directory exists with baseline review files.
fn ensure_audit_dir(repo_path: &Path) -> Result<()> {
    fs::create_dir_all(repo_path).with_context(|| format!("creating {}", repo_path.display()))?;

    let readme_path = repo_path.join("README.md");
    if !readme_path.exists() {
        fs::write(
            &readme_path,
            "# PathKeep Audit Artifacts\n\nThis directory stores manifests, scheduler artifacts, and health reports. If Git is available, PathKeep may also keep a local commit history here.\n",
        )
        .with_context(|| format!("writing {}", readme_path.display()))?;
    }

    let gitignore_path = repo_path.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "*.tmp\n")
            .with_context(|| format!("writing {}", gitignore_path.display()))?;
    }

    Ok(())
}

/// Ensures the local audit repository exists and has baseline git config.
pub fn ensure_repo(repo_path: &Path) -> Result<()> {
    ensure_audit_dir(repo_path)?;
    if !repo_path.join(".git").exists() {
        run_git(repo_path, ["init"])?;
    }

    run_git(repo_path, ["config", "user.name", "PathKeep"])?;
    run_git(repo_path, ["config", "user.email", "vault@localhost"])?;
    run_git(repo_path, ["config", "commit.gpgsign", "false"])?;

    Ok(())
}

/// Writes one audit artifact into the local audit repository.
pub fn write_audit_file(repo_path: &Path, relative_path: &str, contents: &str) -> Result<PathBuf> {
    ensure_audit_dir(repo_path)?;
    let full_path = repo_path.join(relative_path);
    ensure_parent_dir(&full_path)?;
    fs::write(&full_path, contents).context(format!("writing {}", full_path.display()))?;
    Ok(full_path)
}

#[rustfmt::skip]
/// Creates parent directories for one audit file path if they are missing.
fn ensure_parent_dir(path: &Path) -> Result<()> { if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; } Ok(()) }

/// Stages and commits all current audit-repo changes, returning the new commit hash.
pub fn commit_all(repo_path: &Path, message: &str) -> Result<Option<String>> {
    ensure_repo(repo_path)?;
    run_git(repo_path, ["add", "."])?;
    let status = Command::new("git")
        .current_dir(repo_path)
        .args(["status", "--short"])
        .output()
        .context("reading git status")?;
    if status.stdout.is_empty() {
        return Ok(None);
    }
    run_git(repo_path, ["commit", "-m", message])?;
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("reading git rev-parse")?;
    Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_string()))
}

/// Attempts to commit audit artifacts without making Git a runtime dependency.
///
/// The artifact files are already durable on disk before this helper runs. A
/// missing Git executable, a broken repository, or a local policy failure must
/// not turn a completed backup/import into a failed data operation.
pub fn commit_all_optional(repo_path: &Path, message: &str) -> (Option<String>, Option<String>) {
    match commit_all(repo_path, message) {
        Ok(commit) => (commit, None),
        Err(error) => (
            None,
            Some(format!(
                "Audit artifacts were written, but the optional Git history step was skipped: {error}"
            )),
        ),
    }
}

/// Runs one `git` command inside the audit repository.
fn run_git<const N: usize>(repo_path: &Path, args: [&str; N]) -> Result<()> {
    run_repo_command(OsStr::new("git"), repo_path, args)
}

/// Runs an arbitrary command inside the audit repository and normalizes failures.
fn run_repo_command<S: AsRef<OsStr>, const N: usize>(
    program: S,
    repo_path: &Path,
    args: [&str; N],
) -> Result<()> {
    let output = Command::new(program)
        .current_dir(repo_path)
        .args(args)
        .output()
        .with_context(|| format!("running git in {}", repo_path.display()))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            anyhow::bail!("git command failed in {}", repo_path.display())
        } else {
            anyhow::bail!("git command failed in {}: {}", repo_path.display(), stderr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn git_is_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    fn command_without_stderr(repo_path: &Path) -> anyhow::Error {
        run_repo_command(OsStr::new("sh"), repo_path, ["-c", "exit 1"])
            .expect_err("git should fail")
    }

    #[cfg(windows)]
    fn command_without_stderr(repo_path: &Path) -> anyhow::Error {
        run_repo_command(OsStr::new("cmd"), repo_path, ["/C", "exit /B 1"])
            .expect_err("git should fail")
    }

    #[cfg(unix)]
    fn command_with_stderr(repo_path: &Path) -> anyhow::Error {
        run_repo_command(OsStr::new("sh"), repo_path, ["-c", "echo boom >&2; exit 2"])
            .expect_err("git should fail with stderr")
    }

    #[cfg(windows)]
    fn command_with_stderr(repo_path: &Path) -> anyhow::Error {
        run_repo_command(OsStr::new("cmd"), repo_path, ["/C", "echo boom 1>&2 & exit /B 2"])
            .expect_err("git should fail with stderr")
    }

    #[test]
    fn ensure_repo_bootstraps_git_metadata() {
        if !git_is_available() {
            return;
        }

        let dir = tempdir().expect("tempdir");
        ensure_repo(dir.path()).expect("ensure repo");

        assert!(dir.path().join(".git").exists());
        assert!(dir.path().join("README.md").exists());
        assert!(dir.path().join(".gitignore").exists());
    }

    #[test]
    fn write_and_commit_audit_files() {
        if !git_is_available() {
            return;
        }

        let dir = tempdir().expect("tempdir");
        ensure_repo(dir.path()).expect("ensure repo");
        let file = write_audit_file(dir.path(), "manifests/run-1.json", "{\"ok\":true}")
            .expect("write audit");
        assert!(file.exists());

        let commit = commit_all(dir.path(), "test: commit audit artifact").expect("commit");
        assert!(commit.is_some());

        let second = commit_all(dir.path(), "test: noop").expect("second commit");
        assert!(second.is_none());
    }

    #[test]
    fn write_audit_file_handles_top_level_paths() {
        let dir = tempdir().expect("tempdir");
        let file =
            write_audit_file(dir.path(), "doctor.json", "{\"ok\":true}").expect("write audit file");
        assert_eq!(file, dir.path().join("doctor.json"));
        assert!(file.exists());
        assert!(dir.path().join("README.md").exists());
        assert!(dir.path().join(".gitignore").exists());
        assert!(!dir.path().join(".git").exists());
    }

    #[test]
    fn run_git_reports_failures_with_and_without_stderr() {
        let dir = tempdir().expect("tempdir");
        let empty_stderr = command_without_stderr(dir.path());
        assert_eq!(
            empty_stderr.to_string(),
            format!("git command failed in {}", dir.path().display())
        );

        let stderr_error = command_with_stderr(dir.path());
        assert_eq!(
            stderr_error.to_string(),
            format!("git command failed in {}: boom", dir.path().display())
        );
    }

    #[test]
    fn optional_commit_reports_warning_instead_of_failing() {
        let dir = tempdir().expect("tempdir");
        let blocked_path = dir.path().join("audit-file");
        fs::write(&blocked_path, "not a directory").expect("write blocker file");

        let (commit, warning) = commit_all_optional(&blocked_path, "noop");

        assert!(commit.is_none());
        assert!(warning.expect("warning").contains("optional Git history step was skipped"));
    }
}
