use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");

    let git_commit_full = git_output(["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let git_commit_short =
        git_output(["rev-parse", "--short=8", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let git_dirty = Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .status()
        .map(|status| !status.success())
        .unwrap_or(false);

    println!("cargo:rustc-env=BHB_GIT_COMMIT_FULL={git_commit_full}");
    println!("cargo:rustc-env=BHB_GIT_COMMIT_SHORT={git_commit_short}");
    println!("cargo:rustc-env=BHB_GIT_DIRTY={}", if git_dirty { "true" } else { "false" });

    tauri_build::build()
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
