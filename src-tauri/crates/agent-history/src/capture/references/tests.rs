//! Responsibilities: prove reference authority against synthetic filesystem boundaries.
//! Not responsible for: discovering or reading any real agent history.
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
            resolve(&declared, &[], &reference("attachment", "file:///C:/Users/person/image.png"))
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
        rejected(resolve(&selected_file, &[], &reference("attachment", "picture.png")).unwrap())
            .contains("outside")
    );
    let declared = source(temporary.path());
    for path in ["auth.json", "config.toml", "settings.local.json", ".env", "credentials/token"] {
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
            resolve(&declared, &[], &reference("lineage", "12345678-abcd-4321-8765-123456789abc"))
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
        ("attachment", "https://account:private-password@example.test/image?token=private-token"),
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
        assert!(matches!(
            issue.detail.as_deref(),
            Some("outside_roots" | "missing" | "unsafe_path" | "authority_unavailable" | "remote")
        ));
        assert!(!serde_json::to_string(&issue).unwrap().contains("private"));
        assert_eq!(reference.target, target, "exact evidence stays available to encrypted reports");
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
