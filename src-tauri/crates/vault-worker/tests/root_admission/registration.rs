//! Responsibilities: fail when registered command surfaces outgrow the reviewed literal inventory.
//! Not responsible for: discovering filesystem calls, deriving operation tests or proving admission.
use std::{collections::BTreeSet, fs, path::Path};

fn quoted_arms(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| line.split_once("=>").map(|(left, _)| left))
        .flat_map(|left| {
            left.split('"')
                .enumerate()
                .filter(|(i, _)| i % 2 == 1)
                .map(|(_, name)| name.to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}
fn literal(words: &str) -> BTreeSet<String> {
    words.split_whitespace().map(str::to_owned).collect()
}

#[test]
fn every_desktop_registration_has_a_reviewed_inventory_row() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/lib.rs")).unwrap();
    let body = source.split("tauri::generate_handler![").nth(1).unwrap().split(']').next().unwrap();
    let actual: BTreeSet<_> =
        body.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect();
    let expected = super::support::fixture("desktop-operations.txt");
    let expected =
        expected.lines().filter(|line| !line.starts_with('#')).collect::<Vec<_>>().join(" ");
    assert_eq!(
        actual,
        literal(&expected),
        "add an independently reviewed entry-point row and refusal scenario"
    );
}

#[test]
fn every_owner_and_mcp_registration_has_a_literal_row() {
    let source = include_str!("../../src/owner_requests/mod.rs");
    assert_eq!(
        quoted_arms(source),
        literal(
            "ping backup mcp ai-jobs intelligence-jobs doctor ai-index ai-queue search-history search-conversations read-conversation archive-status usage-guide"
        )
    );
    assert!(source.contains("call.tool == \"initialize\""));
}

#[test]
fn every_worker_cli_registration_has_a_literal_row() {
    let source = include_str!("../../src/cli.rs");
    assert_eq!(
        quoted_arms(source),
        literal("backup doctor ai-index ai-queue ping ai-jobs intelligence-jobs")
    );
    let special: BTreeSet<_> = source
        .split("if command == \"")
        .skip(1)
        .map(|tail| tail.split('"').next().unwrap().to_owned())
        .collect();
    assert_eq!(special, literal("mcp-server"));
    assert!(
        source.contains("arguments == [\"--build-identity\"]"),
        "the identity probe is root-free"
    );
}

#[test]
fn every_standalone_command_has_an_inventory_row() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent-history/src/cli/args.rs"),
    )
    .unwrap();
    let parse = source.split("fn parse(value:").nth(1).unwrap().split("fn accepts").next().unwrap();
    assert_eq!(
        quoted_arms(parse),
        literal(
            "help --help -h init plan backup mark folders reparse list sources events revisions search export raw-export verify sync-search"
        )
    );
}
