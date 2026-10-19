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
pub fn commit_all_optional(
    repo_path: &Path,
    message: &str,
) -> (Option<String>, Option<crate::models::BackupWarning>) {
    match commit_all(repo_path, message) {
        Ok(commit) => (commit, None),
        Err(error) => (
            None,
            // Coded so run ledgers localize the skip; the git error chain is
            // evidence and stays verbatim in `diagnostic`.
            Some(
                crate::models::BackupWarning::new(
                    "git-history-skipped",
                    format!(
                        "Audit artifacts were written, but the optional Git history step was skipped: {error}"
                    ),
                )
                .with_diagnostic(format!("{error:#}")),
            ),
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
