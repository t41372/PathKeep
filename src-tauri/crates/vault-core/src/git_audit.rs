use anyhow::{Context, Result};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn ensure_repo(repo_path: &Path) -> Result<()> {
    fs::create_dir_all(repo_path).with_context(|| format!("creating {}", repo_path.display()))?;
    if !repo_path.join(".git").exists() {
        run_git(repo_path, ["init"])?;
    }

    let readme_path = repo_path.join("README.md");
    if !readme_path.exists() {
        fs::write(
            &readme_path,
            "# PathKeep Audit Repo\n\nThis repository stores manifests, scheduler artifacts, and health reports.\n",
        )
        .with_context(|| format!("writing {}", readme_path.display()))?;
    }

    let gitignore_path = repo_path.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "*.tmp\n")
            .with_context(|| format!("writing {}", gitignore_path.display()))?;
    }

    run_git(repo_path, ["config", "user.name", "PathKeep"])?;
    run_git(repo_path, ["config", "user.email", "vault@localhost"])?;
    run_git(repo_path, ["config", "commit.gpgsign", "false"])?;

    Ok(())
}

pub fn write_audit_file(repo_path: &Path, relative_path: &str, contents: &str) -> Result<PathBuf> {
    let full_path = repo_path.join(relative_path);
    ensure_parent_dir(&full_path)?;
    fs::write(&full_path, contents).context(format!("writing {}", full_path.display()))?;
    Ok(full_path)
}

#[rustfmt::skip]
fn ensure_parent_dir(path: &Path) -> Result<()> { if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; } Ok(()) }

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

fn run_git<const N: usize>(repo_path: &Path, args: [&str; N]) -> Result<()> {
    run_repo_command(OsStr::new("git"), repo_path, args)
}

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
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    fn install_fake_git(dir: &Path, body: &str) -> PathBuf {
        let script_path = dir.join("git");
        fs::create_dir_all(dir).expect("create fake git dir");
        fs::write(&script_path, body).expect("write fake git");
        let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions).expect("chmod");
        script_path
    }

    #[test]
    fn ensure_repo_bootstraps_git_metadata() {
        let dir = tempdir().expect("tempdir");
        ensure_repo(dir.path()).expect("ensure repo");

        assert!(dir.path().join(".git").exists());
        assert!(dir.path().join("README.md").exists());
        assert!(dir.path().join(".gitignore").exists());
    }

    #[test]
    fn write_and_commit_audit_files() {
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
    }

    #[test]
    fn run_git_reports_failures_with_and_without_stderr() {
        let dir = tempdir().expect("tempdir");
        let bin_dir = dir.path().join("fake-bin");
        let empty_stderr_git = install_fake_git(&bin_dir, "#!/bin/sh\nexit 1\n");
        let empty_stderr = run_repo_command(&empty_stderr_git, dir.path(), ["status"])
            .expect_err("git should fail");
        assert_eq!(
            empty_stderr.to_string(),
            format!("git command failed in {}", dir.path().display())
        );

        let stderr_git = install_fake_git(&bin_dir, "#!/bin/sh\necho 'boom' >&2\nexit 2\n");
        let stderr_error = run_repo_command(&stderr_git, dir.path(), ["status"])
            .expect_err("git should fail with stderr");
        assert_eq!(
            stderr_error.to_string(),
            format!("git command failed in {}: boom", dir.path().display())
        );
    }
}
