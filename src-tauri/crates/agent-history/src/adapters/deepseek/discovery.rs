//! Responsibilities: inventory DeepSeek generations and known attachment trees.
//! Not responsible for: decoding records, repairing sources, or granting later
//! filesystem reads. Capture revalidates every discovered path against races.

use crate::adapters::common;
use crate::{Artifact, ArtifactKind, Diagnostic, SourceRoot};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_DIRECTORY_DEPTH: usize = 16;

/// Keep retained generations raw while identifying one unambiguous query source.
pub(super) fn discover(
    source: &SourceRoot,
    emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
    issues: &mut Vec<Diagnostic>,
) -> crate::Result<()> {
    if !directory_is_regular(&source.root, issues, false) {
        return Ok(());
    }
    let default_sessions = source.root.join("sessions");
    let sessions = match fs::symlink_metadata(&default_sessions) {
        Ok(metadata) if metadata.is_dir() => default_sessions,
        Ok(_) => {
            discovery_issue(issues, &default_sessions, "deepseek-session-root-invalid");
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => source.root.clone(),
        Err(_) => {
            discovery_issue(issues, &default_sessions, "deepseek-session-root-unreadable");
            return Ok(());
        }
    };
    discover_sessions(&source.root, &sessions, 0, emit, issues)?;
    if sessions != source.root {
        let attachments = source.root.join("attachments");
        if !directory_is_regular(&attachments, issues, true)
            || !directory_is_regular(&attachments.join("v1"), issues, true)
        {
            return Ok(());
        }
        for name in ["objects", "file-objects"] {
            let directory = attachments.join("v1").join(name);
            match fs::symlink_metadata(&directory) {
                Ok(metadata) if metadata.is_dir() => {
                    common::walk_files(
                        &directory,
                        &mut |path| {
                            emit(artifact(&source.root, path, ArtifactKind::Attachment, None, true))
                        },
                        issues,
                    )?;
                }
                Ok(_) => discovery_issue(issues, &directory, "deepseek-attachment-root-invalid"),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    discovery_issue(issues, &directory, "deepseek-attachment-root-unreadable")
                }
            }
        }
    }
    Ok(())
}

fn artifact(
    root: &Path,
    path: PathBuf,
    kind: ArtifactKind,
    generation: Option<u64>,
    current: bool,
) -> Artifact {
    let relative_path = path
        .strip_prefix(root)
        .unwrap_or(&path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    Artifact { path, relative_path, kind, project_hint: None, generation, current }
}

fn discovery_issue(issues: &mut Vec<Diagnostic>, path: &Path, code: &str) {
    common::push_diagnostic(
        issues,
        Diagnostic {
            code: code.into(),
            path: Some(path.to_string_lossy().into_owned()),
            ordinal: None,
            detail: None,
        },
    );
}

/// Check each chosen path component before probing below it. Capture repeats this
/// authorization against races; discovery must not hide an intermediate symlink.
fn directory_is_regular(path: &Path, issues: &mut Vec<Diagnostic>, optional: bool) -> bool {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => true,
        Ok(_) => {
            discovery_issue(issues, path, "deepseek-directory-not-regular");
            false
        }
        Err(error) if optional && error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => {
            discovery_issue(issues, path, "deepseek-directory-unreadable");
            false
        }
    }
}

fn directory_entries(path: &Path, issues: &mut Vec<Diagnostic>) -> Option<fs::ReadDir> {
    match fs::read_dir(path) {
        Ok(entries) => Some(entries),
        Err(_) => {
            discovery_issue(issues, path, "deepseek-directory-unreadable");
            None
        }
    }
}

fn directory_entry(
    entry: std::io::Result<fs::DirEntry>,
    directory: &Path,
    issues: &mut Vec<Diagnostic>,
) -> Option<fs::DirEntry> {
    match entry {
        Ok(entry) => Some(entry),
        Err(_) => {
            discovery_issue(issues, directory, "deepseek-directory-entry-unreadable");
            None
        }
    }
}

/// Select generations within one directory, keeping memory independent of corpus size.
fn discover_sessions(
    root: &Path,
    directory: &Path,
    depth: usize,
    emit: &mut dyn FnMut(Artifact) -> crate::Result<()>,
    issues: &mut Vec<Diagnostic>,
) -> crate::Result<()> {
    if depth > MAX_DIRECTORY_DEPTH {
        discovery_issue(issues, directory, "deepseek-directory-depth-exceeded");
        return Ok(());
    }
    if !directory_is_regular(directory, issues, false) {
        return Ok(());
    }
    let Some(entries) = directory_entries(directory, issues) else {
        return Ok(());
    };
    // Two directory passes bound memory even for malformed or legacy flat roots
    // containing millions of files. No corpus-sized path list is retained.
    let mut highest = None;
    let mut highest_count = 0_u64;
    for entry in entries {
        let Some(entry) = directory_entry(entry, directory, issues) else { continue };
        if entry.file_type().is_ok_and(|kind| kind.is_file()) {
            if let Some(version) = generation_number(&entry.file_name().to_string_lossy()) {
                match highest {
                    Some(previous) if previous > version => {}
                    Some(previous) if previous == version => {
                        highest_count = highest_count.saturating_add(1)
                    }
                    _ => {
                        highest = Some(version);
                        highest_count = 1;
                    }
                }
            }
        }
    }
    let ambiguous = highest_count > 1;
    if ambiguous {
        discovery_issue(issues, directory, "deepseek-ambiguous-generation");
    }
    let Some(entries) = directory_entries(directory, issues) else { return Ok(()) };
    for entry in entries {
        let Some(entry) = directory_entry(entry, directory, issues) else { continue };
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => discover_sessions(root, &path, depth + 1, emit, issues)?,
            Ok(kind) if kind.is_file() => {
                if let Some(version) = generation_number(&entry.file_name().to_string_lossy()) {
                    if path.to_str().is_some() {
                        if Some(version) > highest {
                            discovery_issue(issues, directory, "deepseek-generation-changed");
                        }
                        emit(artifact(
                            root,
                            path,
                            ArtifactKind::Transcript,
                            Some(version),
                            !ambiguous && Some(version) == highest,
                        ))?;
                    } else {
                        discovery_issue(issues, &path, "non_utf8_path");
                    }
                }
            }
            Ok(kind) if kind.is_symlink() => {
                discovery_issue(issues, &path, "source-symlink-skipped")
            }
            Ok(_) => {}
            Err(_) => discovery_issue(issues, &path, "deepseek-directory-entry-unreadable"),
        }
    }
    Ok(())
}

pub(crate) fn generation_number(name: &str) -> Option<u64> {
    let name = name.strip_suffix(".zstd").unwrap_or(name);
    if name == "session.jsonl" {
        return Some(0);
    }
    let digits = name.strip_prefix("session.v")?.strip_suffix(".jsonl")?;
    if digits.starts_with('0') || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let version = digits.parse::<u64>().ok()?;
    (version <= 9_007_199_254_740_991).then_some(version)
}
