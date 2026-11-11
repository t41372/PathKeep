//! Responsibilities: authorize structured native references against selected source roots.
//! Not responsible for: reading payloads, following network URLs, or replaying lineage.

use crate::{Artifact, ArtifactKind, Diagnostic, Reference, Result, SourceRoot};
use std::path::{Component, Path, PathBuf};

/// An adapter supplies evidence of a reference, never authority to read its target.
pub(super) enum Resolution {
    Local { source: SourceRoot, artifact: Artifact },
    Unresolved(Diagnostic),
    Inline,
}

/// Resolves only declared native pointers; returned files still undergo capture's
/// open-handle validation because filesystem state may change after this check.
pub(super) fn resolve(
    source: &SourceRoot,
    roots: &[SourceRoot],
    reference: &Reference,
) -> Result<Resolution> {
    let target = reference.target.as_str();
    if target.is_empty() || target.contains('\0') {
        return Ok(unresolved(reference, "invalid"));
    }
    if !matches!(reference.kind.as_str(), "attachment" | "tool-output" | "lineage") {
        return Ok(unresolved(reference, "unsupported"));
    }
    if target.starts_with("data:") && reference.kind == "attachment" {
        return Ok(Resolution::Inline);
    }
    if reference.kind == "lineage" && opaque_identity(target) {
        return Ok(unresolved(reference, "unresolved"));
    }
    let path = match local_path(target) {
        Ok(path) => path,
        Err(reason) => return Ok(unresolved(reference, reason)),
    };
    let base = if source.root.is_file() {
        source.root.parent().unwrap_or(&source.root)
    } else {
        &source.root
    };
    let selected_path = if path.is_absolute() { path } else { base.join(path) };
    let candidate = absolute(&selected_path)?;
    let mut authorized_location = false;
    let mut unavailable_authority = false;
    let mut missing = false;
    for authority in std::iter::once(source).chain(roots) {
        let canonical_root = match std::fs::canonicalize(&authority.root) {
            Ok(root) => root,
            Err(_) => {
                unavailable_authority = true;
                continue;
            }
        };
        let declared = absolute(&authority.root)?;
        // Resolve only caller-approved roots. Probing an unapproved target can
        // itself access a network share on Windows, even before opening the file.
        if !within_root(&candidate, &declared, &canonical_root, canonical_root.is_file()) {
            continue;
        }
        authorized_location = true;
        let canonical = match super::files::ensure_source_path(&selected_path, &authority.root) {
            Ok(path) => path,
            Err(crate::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                missing = true;
                continue;
            }
            Err(_) => continue,
        };
        let relative = if canonical_root.is_file() {
            Path::new(canonical.file_name().unwrap_or_default())
        } else {
            match canonical.strip_prefix(&canonical_root) {
                Ok(relative) => relative,
                Err(_) => continue,
            }
        };
        if sensitive(relative) {
            return Ok(unresolved(reference, "sensitive"));
        }
        let Some(relative_path) = portable_path(relative) else {
            return Ok(unresolved(reference, "path_encoding"));
        };
        return Ok(Resolution::Local {
            source: SourceRoot { harness: authority.harness, root: canonical_root },
            artifact: Artifact {
                path: canonical,
                relative_path,
                kind: if reference.kind == "lineage" {
                    ArtifactKind::Metadata
                } else {
                    ArtifactKind::Attachment
                },
                project_hint: None,
                generation: None,
                current: false,
            },
        });
    }
    let reason = if authorized_location {
        if missing { "missing" } else { "unsafe_path" }
    } else if unavailable_authority {
        "authority_unavailable"
    } else {
        "outside_roots"
    };
    Ok(unresolved(reference, reason))
}

fn local_path(target: &str) -> std::result::Result<PathBuf, &'static str> {
    if network_or_device_path(target) {
        return Err("network_or_device_path");
    }
    if target.starts_with("file:") {
        if file_uri_network_path(target) {
            return Err("network_or_device_path");
        }
        let uri = url::Url::parse(target).map_err(|_| "invalid_uri")?;
        if uri.host_str().is_some_and(|host| !host.is_empty() && host != "localhost") {
            return Err("remote_unavailable");
        }
        if uri.query().is_some() || uri.fragment().is_some() || !uri.username().is_empty() {
            return Err("invalid_uri");
        }
        if network_or_device_path(uri.path()) {
            return Err("network_or_device_path");
        }
        #[cfg(not(windows))]
        if windows_path(uri.path().trim_start_matches('/')) {
            return Err("foreign_path");
        }
        let path = uri.to_file_path().map_err(|_| "invalid_uri")?;
        if path.as_os_str().to_string_lossy().contains('\0') {
            return Err("invalid_uri");
        }
        if path.to_str().is_some_and(network_or_device_path) {
            return Err("network_or_device_path");
        }
        if path.to_str().is_some_and(alternate_stream_path) {
            return Err("alternate_stream");
        }
        return Ok(path);
    }
    #[cfg(not(windows))]
    if windows_path(target) {
        return Err("foreign_path");
    }
    // A colon after an ASCII scheme is not a native relative path. On Windows
    // drive letters are handled by Path; network/device references are never used.
    if target.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1 && scheme.bytes().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
    }) {
        return Err("remote_unavailable");
    }
    if alternate_stream_path(target) {
        return Err("alternate_stream");
    }
    Ok(PathBuf::from(target))
}

// URL normalization is allowed to collapse repeated separators. Inspect the
// original path after its authority before that can hide a UNC-shaped spelling.
fn file_uri_network_path(value: &str) -> bool {
    let Some(tail) = value.strip_prefix("file:") else { return false };
    let path = if let Some(authority) = tail.strip_prefix("//") {
        authority.find(['/', '\\']).map_or("", |offset| &authority[offset..])
    } else {
        tail
    };
    path.chars().take_while(|value| matches!(*value, '/' | '\\')).take(2).count() == 2
}

fn alternate_stream_path(value: &str) -> bool {
    value.split(['/', '\\']).enumerate().any(|(index, part)| {
        part.contains(':')
            && !(index == 0
                && part.len() == 2
                && part.as_bytes()[0].is_ascii_alphabetic()
                && part.as_bytes()[1] == b':')
    })
}

/// Pure classification happens before any target metadata lookup, on every OS.
fn network_or_device_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.chars().take_while(|value| matches!(*value, '/' | '\\')).take(2).count() == 2
        || lower.starts_with("\\??\\")
        || lower.starts_with("\\device\\")
        || lower.split(['/', '\\']).any(|part| {
            let stem = part.trim_end_matches([' ', '.']).split('.').next().unwrap_or("");
            matches!(stem, "con" | "prn" | "aux" | "nul" | "conin$" | "conout$")
                || ((stem.starts_with("com") || stem.starts_with("lpt"))
                    && stem.len() == 4
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        })
}

fn within_root(candidate: &Path, declared: &Path, canonical: &Path, is_file: bool) -> bool {
    if is_file {
        candidate == declared || candidate == canonical
    } else {
        candidate.starts_with(declared) || candidate.starts_with(canonical)
    }
}

#[cfg(not(windows))]
fn windows_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn opaque_identity(value: &str) -> bool {
    !value.contains(['/', '\\']) && Path::new(value).extension().is_none()
}

fn sensitive(relative: &Path) -> bool {
    relative.components().any(|component| {
        let Component::Normal(name) = component else { return false };
        let name = name.to_string_lossy().to_ascii_lowercase();
        matches!(
            name.as_str(),
            ".ssh" | ".aws" | ".azure" | ".gnupg" | "credentials" | "secrets" | "keychains"
        ) || name == ".env"
            || name.starts_with(".env.")
            || ["auth", "credential", "config", "setting"].iter().any(|prefix| {
                name.starts_with(prefix)
                    && [".json", ".toml", ".yaml", ".yml"]
                        .iter()
                        .any(|suffix| name.ends_with(suffix))
            })
            || [".pem", ".key", ".p12", ".pfx"].iter().any(|suffix| name.ends_with(suffix))
    })
}

fn portable_path(path: &Path) -> Option<String> {
    path.components()
        .map(|part| part.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

fn absolute(path: &Path) -> Result<PathBuf> {
    let path =
        if path.is_absolute() { path.to_owned() } else { std::env::current_dir()?.join(path) };
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

fn unresolved(reference: &Reference, reason: &str) -> Resolution {
    let category = if reference.kind == "lineage" { "lineage" } else { "attachment" };
    Resolution::Unresolved(Diagnostic {
        code: format!("{category}_{reason}"),
        // References can contain URL credentials or arbitrary malformed text.
        // Their exact values remain in encrypted ParseReport evidence, never in
        // diagnostics returned to a CLI, log sink or future notification surface.
        path: None,
        ordinal: None,
        detail: None,
    })
}

#[cfg(test)]
mod tests {
    //! Failure modes: a transcript grants outside-root access, URI decoding changes
    //! authority, credential files become attachments, or a selected file grants its
    //! entire parent directory. These boundary tests precede the resolver.
    use super::*;
    use crate::Harness;
    use std::fs;

    fn source(root: &std::path::Path) -> SourceRoot {
        SourceRoot { harness: Harness::Pi, root: root.to_owned() }
    }

    fn reference(kind: &str, target: &str) -> Reference {
        Reference {
            kind: kind.into(),
            target: target.into(),
            end_byte_offset: None,
            end_ordinal_exclusive: None,
        }
    }

    fn rejected(value: Resolution) -> String {
        match value {
            Resolution::Unresolved(issue) => issue.code,
            _ => panic!("an unsafe reference was accepted"),
        }
    }

    #[test]
    fn explicit_roots_allow_local_artifacts_but_not_neighboring_files() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("sessions");
        let extra = temporary.path().join("outputs");
        fs::create_dir_all(root.join("attachments")).unwrap();
        fs::create_dir_all(&extra).unwrap();
        fs::write(root.join("attachments/picture.png"), b"picture").unwrap();
        fs::write(extra.join("result.txt"), b"result").unwrap();
        let declared = source(&root);
        let Resolution::Local { artifact, .. } =
            resolve(&declared, &[], &reference("attachment", "attachments/picture.png")).unwrap()
        else {
            panic!()
        };
        assert_eq!(artifact.relative_path, "attachments/picture.png");
        assert!(!artifact.current);
        assert_eq!(artifact.kind, ArtifactKind::Attachment);
        let external = reference("tool-output", extra.join("result.txt").to_str().unwrap());
        assert!(rejected(resolve(&declared, &[], &external).unwrap()).contains("outside"));
        assert!(matches!(
            resolve(&declared, &[source(&extra)], &external).unwrap(),
            Resolution::Local { .. }
        ));
    }

    #[test]
    fn file_uri_decoding_preserves_local_authority_and_rejects_remote_hosts() {
        let temporary = tempfile::tempdir().unwrap();
        let file = temporary.path().join("a b.png");
        fs::write(&file, b"image").unwrap();
        let declared = source(temporary.path());
        let uri = url::Url::from_file_path(&file).unwrap();
        assert!(matches!(
            resolve(&declared, &[], &reference("attachment", uri.as_str())).unwrap(),
            Resolution::Local { .. }
        ));
        for target in [
            "https://example.test/image.png",
            "file://remote.test/share/a.png",
            "file:///%00/bad/path/opaque",
            "data:image/png;base64,eA==",
        ] {
            let result = resolve(&declared, &[], &reference("attachment", target)).unwrap();
            if target.starts_with("data:") {
                assert!(matches!(result, Resolution::Inline));
            } else {
                assert!(!rejected(result).is_empty());
            }
        }
        #[cfg(not(windows))]
        assert!(
            rejected(
                resolve(
                    &declared,
                    &[],
                    &reference("attachment", "file:///C:/Users/person/image.png")
                )
                .unwrap()
            )
            .contains("foreign")
        );
    }

    #[test]
    fn references_never_grant_credential_or_source_parent_access() {
        let temporary = tempfile::tempdir().unwrap();
        let transcript = temporary.path().join("chat.jsonl");
        fs::write(&transcript, b"{}\n").unwrap();
        fs::write(temporary.path().join("picture.png"), b"picture").unwrap();
        let selected_file = source(&transcript);
        assert!(
            rejected(
                resolve(&selected_file, &[], &reference("attachment", "picture.png")).unwrap()
            )
            .contains("outside")
        );
        let declared = source(temporary.path());
        for path in ["auth.json", "config.toml", "settings.local.json", ".env", "credentials/token"]
        {
            let file = temporary.path().join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, b"credential").unwrap();
            assert!(
                rejected(resolve(&declared, &[], &reference("attachment", path)).unwrap())
                    .contains("sensitive")
            );
        }
    }

    #[test]
    fn opaque_lineage_is_not_invented_as_a_local_path() {
        let temporary = tempfile::tempdir().unwrap();
        let declared = source(temporary.path());
        assert!(
            rejected(
                resolve(
                    &declared,
                    &[],
                    &reference("lineage", "12345678-abcd-4321-8765-123456789abc")
                )
                .unwrap()
            )
            .contains("lineage")
        );
        assert!(
            rejected(resolve(&declared, &[], &reference("attachment", "not-present.png")).unwrap())
                .contains("missing")
        );
    }

    #[test]
    fn network_and_device_references_are_rejected_without_filesystem_probes() {
        // This calls only the pure classifier: no UNC name is passed to an OS API.
        for target in [
            r"\\unreachable.invalid\share\image.png",
            "//unreachable.invalid/share/image.png",
            r"/\unreachable.invalid/share/image.png",
            r"\/unreachable.invalid/share/image.png",
            r"\\?\UNC\unreachable.invalid\share\image.png",
            r"\\.\C:\image.png",
            r"\??\C:\image.png",
            r"\Device\HarddiskVolume1\image.png",
            "file:////unreachable.invalid/share/image.png",
            "file://localhost//unreachable.invalid/share/image.png",
            "file:///%5C%5Cunreachable.invalid%5Cshare%5Cimage.png",
            "attachments/NUL.txt",
        ] {
            assert!(local_path(target).is_err(), "unsafe path was accepted: {target}");
        }
    }

    #[test]
    fn alternate_streams_cannot_hide_credential_or_device_names() {
        for target in [
            r"C:\approved\auth.json::$DATA",
            "folder/auth.json::$DATA",
            "folder/NUL::$DATA",
            "folder/image.png:credential-stream",
            "file:///C:/approved/auth.json::$DATA",
            "file:///approved/auth.json%3A%3A%24DATA",
        ] {
            assert!(local_path(target).is_err(), "alternate stream accepted: {target}");
        }
        assert!(!alternate_stream_path(r"C:\approved\image.png"));
        assert!(!alternate_stream_path("folder/image.png"));
    }

    #[test]
    fn unresolved_diagnostics_do_not_expose_untrusted_reference_contents() {
        let temporary = tempfile::tempdir().unwrap();
        let declared = source(temporary.path());
        for (kind, target) in [
            (
                "attachment",
                "https://account:private-password@example.test/image?token=private-token",
            ),
            ("attachment", "private conversational text\0"),
            ("lineage", "private-unresolved-session-id"),
            ("unsupported", "data:text/plain,private-inline-content"),
            ("attachment", "missing-private-local-name.png"),
        ] {
            let reference = reference(kind, target);
            let Resolution::Unresolved(issue) = resolve(&declared, &[], &reference).unwrap() else {
                panic!("expected an unresolved reference")
            };
            assert!(issue.path.is_none(), "untrusted targets do not belong in diagnostics");
            assert!(issue.detail.is_none());
            assert!(!serde_json::to_string(&issue).unwrap().contains("private"));
            assert_eq!(
                reference.target, target,
                "exact evidence stays available to encrypted reports"
            );
        }
    }

    #[test]
    fn lexical_authority_does_not_need_to_probe_an_unapproved_target() {
        let declared = Path::new("/selected/sessions");
        let canonical = Path::new("/private/selected/sessions");
        assert!(within_root(Path::new("/selected/sessions/image.png"), declared, canonical, false));
        assert!(within_root(
            Path::new("/private/selected/sessions/image.png"),
            declared,
            canonical,
            false
        ));
        assert!(!within_root(Path::new("/unapproved/image.png"), declared, canonical, false));
        assert!(!within_root(
            Path::new("/selected/sessions-other/image.png"),
            declared,
            canonical,
            false
        ));
        assert!(!within_root(
            Path::new("/selected/sessions/another.jsonl"),
            Path::new("/selected/sessions/chat.jsonl"),
            Path::new("/selected/sessions/chat.jsonl"),
            true
        ));
    }

    #[cfg(unix)]
    #[test]
    fn attachment_symlinks_cannot_borrow_another_files_authority() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("sessions");
        fs::create_dir_all(&root).unwrap();
        let outside = temporary.path().join("private.png");
        fs::write(&outside, b"private").unwrap();
        symlink(&outside, root.join("image.png")).unwrap();
        assert!(matches!(
            resolve(&source(&root), &[], &reference("attachment", "image.png")).unwrap(),
            Resolution::Unresolved(_)
        ));
    }
}
