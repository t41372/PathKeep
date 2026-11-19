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
    locations(false)
}

/// Includes absent defaults so the picker can explain where presence was checked.
pub fn known_sources() -> Vec<SourceRoot> {
    locations(true)
}

fn locations(include_missing: bool) -> Vec<SourceRoot> {
    if let Some(folder) = env::var_os("CHB_AGENT_TOOLS_HOME") {
        if folder.is_empty() {
            return Vec::new();
        }
        let home = Path::new(&folder);
        return sources_under(home, home, include_missing, |_| None);
    }

    let Some(base) = directories::BaseDirs::new() else { return Vec::new() };
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let desktop_base = base.data_dir();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let desktop_base = base.config_dir();
    sources_under(base.home_dir(), desktop_base, include_missing, |name| {
        environment_path(name, base.home_dir())
    })
}

/// The CLI and the owner use the same layout. Only the native environment adds
/// alternative or supplemental roots to it.
fn sources_under(
    home: &Path,
    desktop_base: &Path,
    include_missing: bool,
    tool_setting: impl Fn(&str) -> Option<PathBuf>,
) -> Vec<SourceRoot> {
    let mut sources = Vec::new();
    for (harness, folder, variable) in [
        (Harness::ClaudeCode, ".claude", "CLAUDE_CONFIG_DIR"),
        (Harness::Codex, ".codex", "CODEX_HOME"),
        (Harness::Pi, ".pi/agent", "PI_CODING_AGENT_DIR"),
        (Harness::Deepseek, ".dsh", "DSH_HOME"),
    ] {
        let configured = tool_setting(variable);
        let keep_missing = include_missing || configured.is_some();
        let root = configured.unwrap_or_else(|| home.join(folder));
        append(&mut sources, harness, &root, keep_missing);
        match harness {
            Harness::Codex => {
                if let Some(path) = tool_setting("CODEX_SQLITE_HOME") {
                    append(&mut sources, harness, &path, true);
                }
            }
            Harness::Pi => {
                // The agent-root reader already enters sessions. Preserve one
                // source identity if both overrides name that same directory.
                if let Some(path) = tool_setting("PI_CODING_AGENT_SESSION_DIR")
                    && !same_location(&path, &root.join("sessions"))
                {
                    append(&mut sources, harness, &path, true);
                }
            }
            _ => {}
        }
    }
    append(&mut sources, Harness::ClaudeCode, &desktop_base.join("Claude"), include_missing);
    sources
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

fn append(sources: &mut Vec<SourceRoot>, harness: Harness, root: &Path, keep_missing: bool) {
    if !keep_missing && !root.exists() {
        return;
    }
    if sources.iter().any(|source| source.harness == harness && same_location(&source.root, root)) {
        return;
    }
    sources.push(SourceRoot { harness, root: root.to_owned() });
}

fn same_location(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise native override precedence without asking the OS for the
    /// account's roaming, XDG or Claude Desktop location on the test host.
    #[test]
    fn native_settings_use_the_supplied_home_and_desktop_base() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let desktop = temp.path().join("application-data");
        let tool_dirs = temp.path().join("native-settings");
        let roots = sources_under(&home, &desktop, true, |variable| Some(tool_dirs.join(variable)));
        let expected = [
            (Harness::ClaudeCode, tool_dirs.join("CLAUDE_CONFIG_DIR")),
            (Harness::Codex, tool_dirs.join("CODEX_HOME")),
            (Harness::Codex, tool_dirs.join("CODEX_SQLITE_HOME")),
            (Harness::Pi, tool_dirs.join("PI_CODING_AGENT_DIR")),
            (Harness::Pi, tool_dirs.join("PI_CODING_AGENT_SESSION_DIR")),
            (Harness::Deepseek, tool_dirs.join("DSH_HOME")),
            (Harness::ClaudeCode, desktop.join("Claude")),
        ];
        assert_eq!(roots.into_iter().map(|r| (r.harness, r.root)).collect::<Vec<_>>(), expected);
        let defaults = sources_under(&home, &desktop, true, |_| None);
        assert_eq!(
            defaults.into_iter().map(|r| r.root).collect::<Vec<_>>(),
            [
                home.join(".claude"),
                home.join(".codex"),
                home.join(".pi/agent"),
                home.join(".dsh"),
                desktop.join("Claude"),
            ]
        );
    }
}
