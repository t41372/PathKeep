//! Responsibilities: expose a bounded, read-only source inventory and stable source identities.
//! Not responsible for: reading conversation bytes, authorizing references or executing backup.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    Error, Result,
    adapters::{adapter, common},
    model::*,
};

const PREVIEW_SAMPLE_LIMIT: usize = 256;

/// A source artifact can be selected and inspected before any archive is created.
#[derive(Clone, Debug, Serialize)]
pub struct PlannedArtifact {
    pub source_id: String,
    pub harness: Harness,
    pub root: PathBuf,
    pub path: PathBuf,
    pub relative_path: String,
    pub kind: ArtifactKind,
    pub byte_len: u64,
    pub current: bool,
}

/// Counts cover the inventory; `sources` is a bounded preview, never an execution manifest.
#[derive(Clone, Debug, Serialize)]
pub struct BackupPlan {
    pub roots: Vec<SourceRoot>,
    pub sources: Vec<PlannedArtifact>,
    pub total_files: u64,
    pub total_bytes: u64,
    pub truncated: bool,
    pub issues: Vec<Diagnostic>,
}

/// Enumerates known artifact locations without opening their contents or writing an archive.
/// Execution rediscovers these approved roots and validates every file again.
pub fn preview(sources: &[SourceRoot]) -> Result<BackupPlan> {
    let mut plan = BackupPlan {
        roots: Vec::with_capacity(sources.len()),
        sources: Vec::new(),
        total_files: 0,
        total_bytes: 0,
        truncated: false,
        issues: Vec::new(),
    };
    for source in sources {
        if root_identity(source).is_err() {
            issue(&mut plan.issues, "source_root_unrepresentable", &source.root, None);
            continue;
        }
        plan.roots.push(source.clone());
        let mut discovery_issues = Vec::new();
        let outcome = adapter(source.harness).discover(
            source,
            &mut |artifact| {
                let source_id = match source_identity(source, &artifact) {
                    Ok(id) => id,
                    Err(_) => {
                        issue(
                            &mut plan.issues,
                            "source_path_unrepresentable",
                            &artifact.path,
                            None,
                        );
                        return Ok(());
                    }
                };
                let metadata = match fs::symlink_metadata(&artifact.path) {
                    Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                        metadata
                    }
                    Ok(_) => {
                        issue(&mut plan.issues, "source_not_regular_file", &artifact.path, None);
                        return Ok(());
                    }
                    Err(error) => {
                        issue(
                            &mut plan.issues,
                            "source_metadata_unavailable",
                            &artifact.path,
                            Some(format!("{:?}", error.kind())),
                        );
                        return Ok(());
                    }
                };
                plan.total_files = plan.total_files.saturating_add(1);
                plan.total_bytes = plan.total_bytes.saturating_add(metadata.len());
                if plan.sources.len() < PREVIEW_SAMPLE_LIMIT {
                    plan.sources.push(PlannedArtifact {
                        source_id,
                        harness: source.harness,
                        root: source.root.clone(),
                        path: artifact.path,
                        relative_path: artifact.relative_path,
                        kind: artifact.kind,
                        byte_len: metadata.len(),
                        current: artifact.current,
                    });
                } else {
                    plan.truncated = true;
                }
                Ok(())
            },
            &mut discovery_issues,
        );
        for diagnostic in discovery_issues {
            common::push_diagnostic(&mut plan.issues, diagnostic);
        }
        if let Err(error) = outcome {
            issue(
                &mut plan.issues,
                "source_discovery_failed",
                &source.root,
                Some(error.code().into()),
            );
        }
    }
    Ok(plan)
}

/// Names physical evidence independently of native session IDs, which can be reused or shared.
pub fn source_identity(source: &SourceRoot, artifact: &Artifact) -> Result<String> {
    let root = normalized_root(&source.root)?;
    // A native filename must not be silently folded through replacement characters.
    if artifact.path.to_str().is_none() {
        return Err(Error::InvalidInput("source path is not UTF-8".into()));
    }
    let root = path_text(&root)?;
    Ok(identity(
        b"pathkeep-agent-source-v1",
        &[source.harness.as_str().as_bytes(), root.as_bytes(), artifact.relative_path.as_bytes()],
    ))
}

/// Coverage for an unreadable root has an identity even when no artifacts can be listed.
pub(crate) fn root_identity(source: &SourceRoot) -> Result<String> {
    let root = normalized_root(&source.root)?;
    Ok(identity(
        b"pathkeep-agent-root-v1",
        &[source.harness.as_str().as_bytes(), path_text(&root)?.as_bytes()],
    ))
}

fn identity(domain: &[u8], parts: &[&[u8]]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    hex::encode(hash.finalize())
}

fn path_text(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| Error::InvalidInput("source root is not UTF-8".into()))
}

/// Resolve ambient aliases (such as macOS `/tmp`) even when a source disappeared.
fn normalized_root(root: &Path) -> Result<PathBuf> {
    path_text(root)?;
    if let Ok(canonical) = fs::canonicalize(root) {
        path_text(&canonical)?;
        return Ok(canonical);
    }
    // Do not collapse `alias/..` lexically: the operating system first follows
    // the alias, then selects the target's parent. The deepest existing ancestor
    // preserves that meaning when only the selected root itself is absent.
    let normalized = std::path::absolute(root)?;
    let mut ancestor = normalized.as_path();
    loop {
        if let Ok(canonical) = fs::canonicalize(ancestor) {
            let suffix = normalized
                .strip_prefix(ancestor)
                .map_err(|_| Error::InvalidInput("source root cannot be normalized".into()))?;
            let resolved = canonical.join(suffix);
            path_text(&resolved)?;
            return Ok(resolved);
        }
        match ancestor.parent() {
            Some(parent) => ancestor = parent,
            None => {
                path_text(&normalized)?;
                return Ok(normalized);
            }
        }
    }
}

fn issue(issues: &mut Vec<Diagnostic>, code: &str, path: &Path, detail: Option<String>) {
    common::push_diagnostic(
        issues,
        Diagnostic {
            code: code.into(),
            path: path.to_str().map(str::to_owned),
            ordinal: None,
            detail,
        },
    );
}

#[cfg(test)]
mod tests {
    //! Failure modes: disappearance changes coverage identity; parent components
    //! are collapsed before an ambient alias resolves; a non-UTF8 root prevents
    //! serializing the diagnostic that was meant to describe it.
    use super::*;

    #[test]
    fn disappearing_root_keeps_its_coverage_identity() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("native-history");
        fs::create_dir(&root).unwrap();
        let source = SourceRoot { harness: Harness::Pi, root: root.clone() };
        let before = root_identity(&source).unwrap();
        fs::remove_dir(&root).unwrap();
        assert_eq!(root_identity(&source).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn missing_root_resolves_parent_components_after_ambient_aliases() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        fs::create_dir_all(temporary.path().join("target/child")).unwrap();
        symlink(temporary.path().join("target/child"), temporary.path().join("alias")).unwrap();
        let source = SourceRoot {
            harness: Harness::Pi,
            root: temporary.path().join("alias/../missing-history"),
        };
        let before = root_identity(&source).unwrap();
        fs::create_dir(temporary.path().join("target/missing-history")).unwrap();
        assert_eq!(root_identity(&source).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_root_produces_a_serializable_plan_diagnostic() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let temporary = tempfile::tempdir().unwrap();
        let source = SourceRoot {
            harness: Harness::Pi,
            root: temporary.path().join(OsString::from_vec(vec![0xff])),
        };
        let plan = preview(&[source]).unwrap();
        assert!(plan.roots.is_empty());
        assert!(plan.issues.iter().any(|issue| issue.code == "source_root_unrepresentable"));
        assert!(serde_json::to_vec(&plan).is_ok());
    }
}
