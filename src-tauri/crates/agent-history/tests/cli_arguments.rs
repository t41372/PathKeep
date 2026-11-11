//! Responsibilities: prove the real CLI refuses ambiguous or unsafe instructions.
//! Not responsible for: emulating the archive or duplicating the feature E2E suite.
//! Failure modes specified before implementation: unknown flags silently execute;
//! duplicate singleton flags change intent; passwords leak through argv diagnostics;
//! an absent execution flag writes data; numeric limits wrap; localized help is missing.
//! A recovery flag must not be silently ignored or accepted by unrelated commands.

use serde_json::Value;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn call(arguments: &[&str], input: &[u8]) -> (Output, Value) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pathkeep-agent-history"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    let value = serde_json::from_slice(&output.stdout).expect("one JSON response");
    (output, value)
}

#[test]
fn unknown_and_repeated_singleton_arguments_never_create_an_archive() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("archive");
    let path = archive.to_str().unwrap();
    for arguments in [
        vec!["init", "--archive", path, "--password-stdin", "--surprise"],
        vec!["init", "--archive", path, "--archive", path, "--password-stdin"],
        vec!["init", "--archive", path, "--password-stdin", "--password-stdin"],
    ] {
        let (output, value) = call(&arguments, b"synthetic-password");
        assert!(!output.status.success());
        assert_eq!(value["error"]["code"], "invalid_input");
        assert!(!archive.exists());
    }
}

#[test]
fn a_password_in_arguments_is_rejected_without_echoing_its_value() {
    let secret = "must-never-be-printed-password";
    let (output, value) = call(&["init", "--archive", "unused", "--password", secret], b"");
    assert!(!output.status.success());
    assert_eq!(value["error"]["code"], "invalid_input");
    assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
}

#[test]
fn invalid_page_limits_are_rejected_before_password_or_archive_access() {
    for limit in ["0", "-1", "1.5", "10000000000000000000000"] {
        let (output, value) = call(&["list", "--archive", "missing", "--limit", limit], b"");
        assert!(!output.status.success());
        assert_eq!(value["error"]["code"], "invalid_input");
    }
}

#[test]
fn help_and_errors_have_all_three_shipped_locales() {
    let mut usage = Vec::new();
    let mut messages = Vec::new();
    for locale in ["en", "zh-CN", "zh-TW"] {
        let (output, value) = call(&["help", "--locale", locale], b"");
        assert!(output.status.success());
        assert!(value["usage"].as_str().unwrap().contains("backup"));
        usage.push(value["description"].as_str().unwrap().to_owned());
        let (output, value) = call(&["unknown-command", "--locale", locale], b"");
        assert!(!output.status.success());
        assert_eq!(value["error"]["code"], "invalid_input");
        messages.push(value["error"]["message"].as_str().unwrap().to_owned());
    }
    assert!(usage.windows(2).all(|pair| pair[0] != pair[1]));
    assert!(messages.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn init_requires_a_nonempty_password_from_standard_input() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("archive");
    let path = archive.to_str().unwrap();
    for input in [b"".as_slice(), b"\n", b"\r\n"] {
        let (output, value) = call(&["init", "--archive", path, "--password-stdin"], input);
        assert!(!output.status.success());
        assert_eq!(value["error"]["code"], "invalid_input");
        assert!(!archive.exists());
    }
    let (output, value) = call(&["init", "--archive", path], b"synthetic-password");
    assert!(!output.status.success());
    assert_eq!(value["error"]["code"], "invalid_input");
    assert!(!archive.exists());
}

#[test]
fn rebuild_is_an_explicit_search_recovery_flag_not_a_general_option() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("missing-archive");
    let path = archive.to_str().unwrap();
    let (output, value) = call(
        &["sync-search", "--rebuild", "--archive", path, "--password-stdin"],
        b"synthetic-password",
    );
    assert!(!output.status.success());
    // A recognized recovery request reaches archive validation. This test never
    // creates an archive; the search suite proves the rebuild's real behavior.
    assert_eq!(value["error"]["code"], "invalid_archive");
    for arguments in [
        vec!["sync-search", "--rebuild", "--rebuild", "--archive", path],
        vec!["backup", "--rebuild", "--archive", path, "--execute"],
        vec!["search", "--rebuild", "--archive", path, "--query", "needle"],
    ] {
        let (output, value) = call(&arguments, b"");
        assert!(!output.status.success());
        assert_eq!(value["error"]["code"], "invalid_input");
    }
    assert!(!archive.exists());
}

#[test]
fn reparsing_requires_a_retained_source_id_and_explicit_execution() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("missing-archive");
    let path = archive.to_str().unwrap();
    let (output, value) = call(
        &["reparse", "--source-id", "retained-source", "--archive", path, "--password-stdin"],
        b"synthetic-password",
    );
    assert!(!output.status.success());
    assert_eq!(value["error"]["code"], "execute_required");
    let (output, value) = call(
        &["reparse", "--execute", "--archive", path, "--password-stdin"],
        b"synthetic-password",
    );
    assert!(!output.status.success());
    assert_eq!(value["error"]["code"], "invalid_input");
    let (output, value) = call(
        &[
            "reparse",
            "--source-id",
            "retained-source",
            "--execute",
            "--archive",
            path,
            "--password-stdin",
        ],
        b"synthetic-password",
    );
    assert!(!output.status.success());
    assert_eq!(value["error"]["code"], "invalid_archive");
    let (output, value) = call(
        &["reparse", "--source", "pi=unapproved-live-source", "--execute", "--archive", path],
        b"",
    );
    assert!(!output.status.success());
    assert_eq!(value["error"]["code"], "invalid_input");
    assert!(!archive.exists());
}

#[test]
fn retained_source_inventory_accepts_only_typed_filters_and_read_options() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("missing-archive");
    let path = archive.to_str().unwrap();
    for kind in ["transcript", "attachment", "metadata", "sqlite"] {
        let (output, value) = call(
            &[
                "sources",
                "--kind",
                kind,
                "--harness",
                "codex",
                "--limit",
                "37",
                "--archive",
                path,
                "--password-stdin",
            ],
            b"synthetic-password",
        );
        assert!(!output.status.success());
        assert_eq!(value["error"]["code"], "invalid_archive");
    }
    for arguments in [
        vec!["sources", "--kind", "unknown", "--archive", path],
        vec!["sources", "--kind", "transcript", "--kind", "metadata", "--archive", path],
        vec!["sources", "--limit", "501", "--archive", path],
        vec!["sources", "--execute", "--archive", path],
        vec!["sources", "--source", "codex=live-input", "--archive", path],
        vec!["list", "--kind", "transcript", "--archive", path],
    ] {
        let (output, value) = call(&arguments, b"");
        assert!(!output.status.success());
        assert_eq!(value["error"]["code"], "invalid_input");
    }
    assert!(!archive.exists());
}
