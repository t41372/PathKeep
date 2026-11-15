//! Responsibilities: every standalone root command rejects product control before opening stores.
//! Not responsible for: capturing real harness sources or deciding a product root's compatibility.
use sha2::{Digest, Sha256};
use std::{fs, process::Command};
#[path = "../../../../tests/root_admission/support/tree.rs"]
mod tree;
const PRODUCT: &str = include_str!("../../../../tests/root_admission/fixtures/reader-floor.json");
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn standalone_commands_reject_product_control_even_beside_a_valid_standalone_marker() {
    let mut failures = Vec::new();
    // Explicit sources and output locations are always in this synthetic sandbox.
    for args in [
        vec!["verify"],
        vec!["list"],
        vec!["sources"],
        vec!["folders"],
        vec!["events", "--session", "fixture"],
        vec!["revisions", "--source-id", "fixture"],
        vec!["search", "--query", "fixture"],
        vec!["sync-search"],
        vec!["mark", "--conversation", "fixture", "--star", "true", "--execute"],
        vec!["reparse", "--source-id", "fixture", "--execute"],
        vec!["backup", "--execute", "--no-git"],
        vec!["export", "--session", "fixture", "--format", "jsonl", "--execute"],
        vec!["raw-export", "--revision", "1", "--execute"],
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        drop(agent_history::Archive::create_unencrypted(&root).unwrap());
        fs::write(root.join("config.json"), PRODUCT).unwrap();
        let input = temp.path().join("harness");
        fs::create_dir(&input).unwrap();
        let before = tree::census(temp.path(), hash);
        let mut command = Command::new(env!("CARGO_BIN_EXE_pathkeep-agent-history"));
        command.args(&args).arg("--archive").arg(&root);
        if args[0] == "backup" {
            command.arg("--source").arg(format!("codex={}", input.display()));
        }
        if matches!(args[0], "export" | "raw-export") {
            command.arg("--output").arg(temp.path().join("output"));
        }
        let result = command.output().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        let after = tree::census(temp.path(), hash);
        if result.status.code() != Some(1)
            || value["error"]["code"] != "invalid_archive"
            || before != after
        {
            failures.push(format!(
                "{args:?}: exit={:?}, response={value}, before={before:?}, after={after:?}",
                result.status.code()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn standalone_initialization_never_overwrites_a_product_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("config.json"), PRODUCT).unwrap();
    let before = tree::census(&root, hash);
    let output = Command::new(env!("CARGO_BIN_EXE_pathkeep-agent-history"))
        .args(["init", "--no-encryption", "--archive"])
        .arg(&root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], "already_exists");
    assert_eq!(before, tree::census(&root, hash));
}
