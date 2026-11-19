//! Responsibilities: enumerate Claude-owned transcript and artifact trees without opening records.
//! Not responsible for collecting arbitrary referenced paths or treating cloud caches as history.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{Result, adapters::common, model::*};

type ArtifactSink<'a> = &'a mut dyn FnMut(Artifact) -> Result<()>;

pub(super) fn discover(
    cancel: &crate::capture::Discovery,
    source: &SourceRoot,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    let root = &source.root;
    let name = root.file_name().and_then(|name| name.to_str()).unwrap_or("");
    match name {
        "projects" => return projects(cancel, root, root, emit, issues).map(|_| ()),
        "claude-code-sessions" => {
            return collect(cancel, root, root, ArtifactKind::Metadata, emit, issues);
        }
        "local-agent-mode-sessions" => return cowork(cancel, root, root, emit, issues),
        "file-history" | "uploads" | "image-cache" | "paste-cache" | "images" => {
            return collect(cancel, root, root, ArtifactKind::Attachment, emit, issues);
        }
        _ => {}
    }
    let mut recognized = false;
    let projects_root = root.join("projects");
    if projects_root.exists() {
        recognized = true;
        projects(cancel, root, &projects_root, emit, issues)?;
    }
    for directory in ["file-history", "uploads", "image-cache", "paste-cache", "plans"] {
        let path = root.join(directory);
        if path.exists() {
            recognized = true;
            collect(cancel, root, &path, ArtifactKind::Attachment, emit, issues)?;
        }
    }
    for name in ["tasks", "history.jsonl"] {
        let path = root.join(name);
        if path.exists() {
            recognized = true;
            collect(cancel, root, &path, ArtifactKind::Metadata, emit, issues)?;
        }
    }
    let metadata = root.join("claude-code-sessions");
    if metadata.exists() {
        recognized = true;
        collect(cancel, root, &metadata, ArtifactKind::Metadata, emit, issues)?;
    }
    let local_agent = root.join("local-agent-mode-sessions");
    if local_agent.exists() {
        recognized = true;
        cowork(cancel, root, &local_agent, emit, issues)?;
    }
    if !recognized {
        issue(issues, "claude-source-layout-unrecognized", root);
    }
    Ok(())
}

fn transcript_name(name: &str) -> bool {
    name.ends_with(".jsonl") || name.contains(".jsonl.superseded-")
}

fn historical_transcript(name: &str) -> bool {
    name.contains(".jsonl.superseded-") || (name.contains(".orphaned-") && name.ends_with(".jsonl"))
}

fn projects(
    cancel: &crate::capture::Discovery,
    root: &Path,
    tree: &Path,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<u64> {
    if !tree_is_direct(root, tree, issues) {
        return Ok(0);
    }
    let mut current_main_transcripts = 0;
    // Traversal owns its diagnostics so the callback may also report path conversion failures.
    let mut traversal_issues = Vec::new();
    common::walk_files_cancellable(
        cancel,
        tree,
        &mut |path| {
            let Ok(relative) = path.strip_prefix(tree) else {
                return Ok(());
            };
            let parts: Vec<_> = relative.components().map(|part| part.as_os_str()).collect();
            let Some(filename) = path.file_name().and_then(|value| value.to_str()) else {
                issue(issues, "claude-non-utf8-path", &path);
                return Ok(());
            };
            let kind = if parts.len() == 2 && transcript_name(filename) {
                Some(ArtifactKind::Transcript)
            } else if parts.len() == 2 {
                Some(ArtifactKind::Metadata)
            } else if parts.len() >= 3 && parts[1] != "memory" {
                if parts.get(2).is_some_and(|part| *part == "subagents") {
                    if filename.starts_with("agent-") && transcript_name(filename) {
                        Some(ArtifactKind::Transcript)
                    } else {
                        Some(ArtifactKind::Metadata)
                    }
                } else {
                    Some(ArtifactKind::Attachment)
                }
            } else {
                None
            };
            if let Some(kind) = kind {
                if parts.len() == 2
                    && matches!(kind, ArtifactKind::Transcript)
                    && !historical_transcript(filename)
                    && !filename.starts_with("agent-")
                {
                    current_main_transcripts += 1;
                }
                artifact(root, path, kind, emit, issues)?;
            }
            Ok(())
        },
        &mut traversal_issues,
    )?;
    for diagnostic in traversal_issues {
        common::push_diagnostic(issues, diagnostic);
    }
    Ok(current_main_transcripts)
}

fn collect(
    cancel: &crate::capture::Discovery,
    root: &Path,
    tree: &Path,
    kind: ArtifactKind,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    if !tree_is_direct(root, tree, issues) {
        return Ok(());
    }
    let mut traversal_issues = Vec::new();
    common::walk_files_cancellable(
        cancel,
        tree,
        &mut |path| artifact(root, path, kind, emit, issues),
        &mut traversal_issues,
    )?;
    for diagnostic in traversal_issues {
        common::push_diagnostic(issues, diagnostic);
    }
    Ok(())
}

fn cowork(
    cancel: &crate::capture::Discovery,
    root: &Path,
    tree: &Path,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    // Enumerating account/org directories keeps plugin caches outside the session collection.
    children(cancel, tree, issues, &mut |account, issues| {
        if !account.is_dir() {
            return Ok(());
        }
        children(cancel, &account, issues, &mut |workspace, issues| {
            if !workspace.is_dir() {
                return Ok(());
            }
            children(cancel, &workspace, issues, &mut |path, issues| {
                let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                    issue(issues, "claude-non-utf8-path", &path);
                    return Ok(());
                };
                if !name.starts_with("local_") {
                    return Ok(());
                }
                if path.is_file() && name.ends_with(".json") {
                    artifact(root, path, ArtifactKind::Metadata, emit, issues)
                } else if path.is_dir() {
                    cowork_body(cancel, root, &path, emit, issues)
                } else {
                    Ok(())
                }
            })
        })
    })
}

fn cowork_body(
    cancel: &crate::capture::Discovery,
    root: &Path,
    body: &Path,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    let projects_root = body.join(".claude/projects");
    let transcript_count = if projects_root.exists() {
        projects(cancel, root, &projects_root, emit, issues)?
    } else {
        0
    };
    let audit = body.join("audit.jsonl");
    if audit.exists() {
        let kind =
            if transcript_count == 0 { ArtifactKind::Transcript } else { ArtifactKind::Metadata };
        collect(cancel, root, &audit, kind, emit, issues)?;
    }
    let audit_key = body.join(".audit-key");
    if audit_key.exists() {
        collect(cancel, root, &audit_key, ArtifactKind::Metadata, emit, issues)?;
    }
    for directory in [
        "uploads",
        "outputs",
        ".claude/file-history",
        ".claude/paste-cache",
        ".claude/image-cache",
        ".claude/uploads",
        ".claude/plans",
    ] {
        let path = body.join(directory);
        if path.exists() {
            collect(cancel, root, &path, ArtifactKind::Attachment, emit, issues)?;
        }
    }
    let tasks = body.join(".claude/tasks");
    if tasks.exists() {
        collect(cancel, root, &tasks, ArtifactKind::Metadata, emit, issues)?;
    }
    Ok(())
}

fn children(
    cancel: &crate::capture::Discovery,
    root: &Path,
    issues: &mut Vec<Diagnostic>,
    visit: &mut dyn FnMut(PathBuf, &mut Vec<Diagnostic>) -> Result<()>,
) -> Result<()> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(_) => {
            issue(issues, "source-directory-unreadable", root);
            return Ok(());
        }
    };
    if metadata.file_type().is_symlink() {
        issue(issues, "source-symlink-skipped", root);
        return Ok(());
    }
    let entries = match cancel.read_dir(root) {
        Ok(entries) => entries,
        Err(_) => {
            issue(issues, "source-directory-unreadable", root);
            return Ok(());
        }
    };
    for entry in entries {
        cancel.check()?;
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                issue(issues, "source-directory-unreadable", root);
                continue;
            }
        };
        match entry.file_type() {
            Ok(kind) if kind.is_symlink() => issue(issues, "source-symlink-skipped", &entry.path()),
            Ok(kind) if kind.is_file() || kind.is_dir() => visit(entry.path(), issues)?,
            Ok(_) => issue(issues, "source-special-file-skipped", &entry.path()),
            Err(_) => issue(issues, "source-entry-unreadable", &entry.path()),
        }
    }
    Ok(())
}

fn artifact(
    root: &Path,
    path: PathBuf,
    kind: ArtifactKind,
    emit: ArtifactSink<'_>,
    issues: &mut Vec<Diagnostic>,
) -> Result<()> {
    let relative = path.strip_prefix(root).ok().and_then(|path| {
        path.components().map(|part| part.as_os_str().to_str()).collect::<Option<Vec<_>>>()
    });
    let Some(parts) = relative else {
        issue(issues, "claude-non-utf8-path", &path);
        return Ok(());
    };
    let relative_path = parts.join("/");
    let current =
        !path.file_name().and_then(|name| name.to_str()).is_some_and(historical_transcript);
    emit(Artifact { path, relative_path, kind, project_hint: None, generation: None, current })
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

fn tree_is_direct(root: &Path, tree: &Path, issues: &mut Vec<Diagnostic>) -> bool {
    let Ok(relative) = tree.strip_prefix(root) else {
        issue(issues, "source-path-outside-root", tree);
        return false;
    };
    let mut path = root.to_path_buf();
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(component) = component {
            path.push(component);
        }
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                issue(issues, "source-symlink-skipped", &path);
                return false;
            }
            Ok(_) => {}
            Err(_) => {
                issue(issues, "source-entry-unreadable", &path);
                return false;
            }
        }
    }
    true
}
