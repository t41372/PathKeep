use anyhow::{Context, Result};
use std::{
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
            "# Browser History Backup Audit Repo\n\nThis repository stores manifests, scheduler artifacts, and health reports.\n",
        )
        .with_context(|| format!("writing {}", readme_path.display()))?;
    }

    let gitignore_path = repo_path.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "*.tmp\n")
            .with_context(|| format!("writing {}", gitignore_path.display()))?;
    }

    if run_git(repo_path, ["config", "--get", "user.name"]).is_err() {
        run_git(repo_path, ["config", "user.name", "Browser History Backup"])?;
    }
    if run_git(repo_path, ["config", "--get", "user.email"]).is_err() {
        run_git(repo_path, ["config", "user.email", "vault@localhost"])?;
    }

    Ok(())
}

pub fn write_audit_file(repo_path: &Path, relative_path: &str, contents: &str) -> Result<PathBuf> {
    let full_path = repo_path.join(relative_path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&full_path, contents).with_context(|| format!("writing {}", full_path.display()))?;
    Ok(full_path)
}

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
    let status = Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .status()
        .with_context(|| format!("running git in {}", repo_path.display()))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("git command failed in {}", repo_path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
}
