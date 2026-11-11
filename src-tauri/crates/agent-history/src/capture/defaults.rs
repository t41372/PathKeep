//! Responsibilities: suggest known local source roots without changing process configuration.
//! Not responsible for: scanning projects, reading credentials or inferring cloud history.

use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::model::{Harness, SourceRoot};

/// Uses documented homes and overrides; absent defaults are omitted while explicit
/// overrides remain visible so discovery can report a missing configured source.
pub fn default_sources() -> Vec<SourceRoot> {
    let Some(base) = directories::BaseDirs::new() else { return Vec::new() };
    let home = base.home_dir();
    let mut sources = Vec::new();
    preferred(&mut sources, Harness::ClaudeCode, "CLAUDE_CONFIG_DIR", home.join(".claude"), home);
    preferred(&mut sources, Harness::Codex, "CODEX_HOME", home.join(".codex"), home);
    if let Some(path) = environment_path("CODEX_SQLITE_HOME", home) {
        append(&mut sources, Harness::Codex, path, true);
    }
    let pi_root = environment_path("PI_CODING_AGENT_DIR", home);
    let pi_root =
        pi_root.map(|path| (path, true)).unwrap_or_else(|| (home.join(".pi/agent"), false));
    append(&mut sources, Harness::Pi, pi_root.0.clone(), pi_root.1);
    if let Some(path) = environment_path("PI_CODING_AGENT_SESSION_DIR", home) {
        // The agent-root reader already enters `sessions`; don't give the same
        // physical files a second source identity just because an override names it.
        if !same_location(&path, &pi_root.0.join("sessions")) {
            append(&mut sources, Harness::Pi, path, true);
        }
    }
    preferred(&mut sources, Harness::Deepseek, "DSH_HOME", home.join(".dsh"), home);

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let desktop = base.data_dir().join("Claude");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let desktop = base.config_dir().join("Claude");
    append(&mut sources, Harness::ClaudeCode, desktop, false);
    sources
}

fn preferred(
    sources: &mut Vec<SourceRoot>,
    harness: Harness,
    variable: &str,
    default: PathBuf,
    home: &Path,
) {
    match environment_path(variable, home) {
        Some(path) => append(sources, harness, path, true),
        None => append(sources, harness, default, false),
    }
}

fn environment_path(variable: &str, home: &Path) -> Option<PathBuf> {
    let value = env::var_os(variable)?;
    if value.is_empty() {
        return None;
    }
    // DeepSeek documents a blank override as absent. Other harnesses receive
    // their exact native path value, including significant whitespace.
    if variable == "DSH_HOME" && value.to_str().is_some_and(|value| value.trim().is_empty()) {
        return None;
    }
    Some(expand_home(value, home))
}

fn expand_home(value: OsString, home: &Path) -> PathBuf {
    match value.to_str() {
        Some("~") => home.to_path_buf(),
        Some(value) if value.starts_with("~/") || value.starts_with("~\\") => {
            home.join(&value[2..])
        }
        _ => PathBuf::from(value),
    }
}

fn append(sources: &mut Vec<SourceRoot>, harness: Harness, root: PathBuf, explicit: bool) {
    if !explicit && !root.exists() {
        return;
    }
    if sources.iter().any(|source| source.harness == harness && same_location(&source.root, &root))
    {
        return;
    }
    sources.push(SourceRoot { harness, root });
}

fn same_location(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
