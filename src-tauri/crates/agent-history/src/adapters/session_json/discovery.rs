//! Responsibilities: enumerate only native Continue sessions and classic Cline/Roo task bundles.
//! Not responsible for opening records, following tool paths, SDK formats, or mutating native indexes.
//! Discovery failures are specified first in `tests/additional_adapters.rs`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    Result,
    adapters::common,
    model::{Artifact, ArtifactKind, Diagnostic, SourceRoot},
};

type ArtifactSink<'a> = &'a mut dyn FnMut(Artifact) -> Result<()>;

pub(super) fn discover_continue(
    source: &SourceRoot,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    let Some(tree) = select_tree(&source.root, "sessions", issues) else {
        return Ok(());
    };
    entries(&tree, issues, &mut |path, kind, issues| {
        // Session files are direct children. Extension caches and arbitrary
        // nested JSON are not conversation evidence.
        if !kind.is_file() {
            return Ok(());
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            issue(issues, "non_utf8_path", &path);
            return Ok(());
        };
        if !name.ends_with(".json") || sensitive_name(name) {
            return Ok(());
        }
        let kind =
            if name == "sessions.json" { ArtifactKind::Metadata } else { ArtifactKind::Transcript };
        artifact(&source.root, &path, kind, emit, issues)
    })
}

pub(super) fn discover_tasks(
    source: &SourceRoot,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
    legacy_roo: bool,
) -> Result<()> {
    let Some(tree) = select_tree(&source.root, "tasks", issues) else {
        return Ok(());
    };
    entries(&tree, issues, &mut |path, kind, issues| {
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if kind.is_file() && name == "_index.json" {
            artifact(&source.root, &path, ArtifactKind::Metadata, emit, issues)
        } else if kind.is_dir() && !matches!(name, "state" | "settings" | "auth") {
            task(&source.root, &path, legacy_roo, emit, issues)
        } else {
            if kind.is_file()
                && !sensitive_name(name)
                && matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("json" | "jsonl" | "db" | "sqlite" | "sqlite3")
                )
            {
                issue(issues, "task_layout_unrecognized", &path);
            }
            Ok(())
        }
    })?;
    if tree != source.root {
        optional_metadata(&source.root, &source.root.join("_index.json"), emit, issues)?;
    }
    let state = source.root.join("state");
    if file_type(&state, true, issues).is_some_and(|kind| kind.is_dir()) {
        optional_metadata(&source.root, &state.join("taskHistory.json"), emit, issues)?;
    }
    Ok(())
}

fn task(
    root: &Path,
    directory: &Path,
    legacy_roo: bool,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    let primary = directory.join("api_conversation_history.json");
    // An inaccessible primary cannot prove that the legacy copy is the current
    // conversation. Preserve the fallback as metadata in that case.
    let has_primary = match fs::symlink_metadata(&primary) {
        Ok(metadata) => metadata.is_file(),
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    };
    let mut transcript_found = false;
    entries(directory, issues, &mut |path, file_kind, issues| {
        if !file_kind.is_file() {
            return Ok(());
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        let kind = match name {
            "api_conversation_history.json" => ArtifactKind::Transcript,
            "claude_messages.json" if legacy_roo && !has_primary => ArtifactKind::Transcript,
            "claude_messages.json" if legacy_roo => ArtifactKind::Metadata,
            "ui_messages.json"
            | "context_history.json"
            | "task_metadata.json"
            | "history_item.json" => ArtifactKind::Metadata,
            _ => return Ok(()),
        };
        transcript_found |= kind == ArtifactKind::Transcript;
        artifact(root, &path, kind, emit, issues)
    })?;
    if !transcript_found {
        // This includes new SDK layouts: their presence must not look like a
        // complete empty backup, and unknown files are not safe to collect.
        issue(issues, "task_transcript_unavailable", directory);
    }
    Ok(())
}

fn sensitive_name(name: &str) -> bool {
    name.to_ascii_lowercase().split(['.', '-', '_']).any(|part| {
        matches!(
            part,
            "config"
                | "configuration"
                | "auth"
                | "authentication"
                | "settings"
                | "credential"
                | "credentials"
                | "secret"
                | "secrets"
                | "token"
                | "tokens"
                | "cookie"
                | "cookies"
        )
    })
}

fn select_tree(root: &Path, child: &str, issues: &mut Vec<Diagnostic>) -> Option<PathBuf> {
    let kind = file_type(root, false, issues)?;
    if !kind.is_dir() {
        issue(issues, "source_not_directory", root);
        return None;
    }
    if root.file_name().and_then(|name| name.to_str()) == Some(child) {
        return Some(root.to_owned());
    }
    let nested = root.join(child);
    match fs::symlink_metadata(&nested) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(root.to_owned()),
        _ => {
            let kind = file_type(&nested, false, issues)?;
            if kind.is_dir() {
                Some(nested)
            } else {
                issue(issues, "source_not_directory", &nested);
                None
            }
        }
    }
}

fn optional_metadata(
    root: &Path,
    path: &Path,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    if file_type(path, true, issues).is_some_and(|kind| kind.is_file()) {
        artifact(root, path, ArtifactKind::Metadata, emit, issues)?;
    }
    Ok(())
}

fn entries(
    directory: &Path,
    issues: &mut Vec<Diagnostic>,
    visit: &mut dyn FnMut(PathBuf, fs::FileType, &mut Vec<Diagnostic>) -> Result<()>,
) -> Result<()> {
    // Validate each directory before entering it, including the selected root
    // and task/state parents that a leaf-only walker would not see.
    if !file_type(directory, false, issues).is_some_and(|kind| kind.is_dir()) {
        return Ok(());
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => {
            issue(issues, "source_unavailable", directory);
            return Ok(());
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if let Some(kind) = file_type(&path, false, issues) {
                    visit(path, kind, issues)?;
                }
            }
            Err(_) => issue(issues, "source_unavailable", directory),
        }
    }
    Ok(())
}

fn file_type(path: &Path, optional: bool, issues: &mut Vec<Diagnostic>) -> Option<fs::FileType> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            issue(issues, "symlink_skipped", path);
            None
        }
        Ok(metadata) if metadata.is_file() || metadata.is_dir() => Some(metadata.file_type()),
        Ok(_) => {
            issue(issues, "non_regular_file", path);
            None
        }
        Err(error) if optional && error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            issue(issues, "source_unavailable", path);
            None
        }
    }
}

fn artifact(
    root: &Path,
    path: &Path,
    kind: ArtifactKind,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    if path.to_str().is_none() {
        issue(issues, "non_utf8_path", path);
        return Ok(());
    }
    let Some(file_kind) = file_type(path, false, issues) else {
        return Ok(());
    };
    if !file_kind.is_file() {
        issue(issues, "non_regular_file", path);
        return Ok(());
    }
    let selected = path.to_owned();
    let mut emitted = false;
    common::walk_files(
        path,
        &mut |path| {
            // A selected file replaced by a directory must never broaden capture.
            if path != selected {
                return Ok(());
            }
            let relative = path.strip_prefix(root).ok().and_then(|path| {
                path.components().map(|part| part.as_os_str().to_str()).collect::<Option<Vec<_>>>()
            });
            let Some(parts) = relative else {
                return Err(crate::Error::InvalidInput(
                    "artifact is outside its UTF-8 source root".into(),
                ));
            };
            let relative_path = parts.join("/");
            emit(Artifact {
                path,
                relative_path,
                kind,
                project_hint: None,
                generation: None,
                current: true,
            })?;
            emitted = true;
            Ok(())
        },
        issues,
    )?;
    if !emitted {
        issue(issues, "source_changed", &selected);
    }
    Ok(())
}

fn issue(issues: &mut Vec<Diagnostic>, code: &str, path: &Path) {
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
