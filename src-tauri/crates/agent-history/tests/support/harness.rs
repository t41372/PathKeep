//! Responsibilities: the CLI calls the evidence-format contract tests need beyond plain
//! `call`/`fail`: archives without encryption, explicit passwords, extra environment
//! variables, today's-contract control runs, paged reads and shared fixture checks.
//! Not responsible for: interpreting native formats or knowing how the archive is laid out.
#![allow(dead_code)]

use super::{Case, PASSWORD, args, items, string};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What the control run saw: today's CLI on an encrypted archive of the same fixture.
pub struct Control {
    pub backup: Value,
    pub sources: Vec<Value>,
}

impl Case {
    /// An archive created with `init --no-encryption`. Its commands send no password,
    /// so its files can be searched for plaintext markers.
    pub fn unencrypted(name: &str) -> Self {
        let mut case = Self::new(name);
        case.encrypted = false;
        case
    }

    /// Runs a command with an explicit password choice, independent of the case's mode.
    pub fn invoke_as(
        &mut self,
        command: &str,
        arguments: Vec<String>,
        password: Option<&str>,
        expect_success: bool,
    ) -> Value {
        self.invoke(command, arguments, password, expect_success)
    }

    /// Runs a successful command with extra environment variables (for example `PATH`).
    pub fn call_with_env(
        &mut self,
        command: &str,
        arguments: Vec<String>,
        environment: &[(&str, &str)],
    ) -> Value {
        let password = self.password();
        let archive = self.archive.clone();
        let run = self.run(&archive, command, arguments, password, environment, Some(true));
        serde_json::from_str(&run.stdout).expect("each CLI invocation returns one JSON document")
    }

    /// Backs the fixture up into a separate encrypted archive with commands that work
    /// today and keep working under the new contract (`init`, `backup --execute`,
    /// `sources`), so a malformed fixture shows up before the first unbuilt step.
    pub fn control(&mut self, sources: Vec<String>) -> Control {
        let archive = self.root.join("control-archive");
        self.run(&archive, "init", vec![], Some(PASSWORD), &[], Some(true));
        let mut backup_args = sources;
        backup_args.push("--execute".into());
        let run = self.run(&archive, "backup", backup_args, Some(PASSWORD), &[], Some(true));
        let backup = serde_json::from_str(&run.stdout).expect("backup report");
        let mut listed = vec![];
        let mut after: Option<String> = None;
        loop {
            let mut arguments = args(&["--limit", "500"]);
            if let Some(cursor) = &after {
                arguments.extend(["--after".into(), cursor.clone()]);
            }
            let run = self.run(&archive, "sources", arguments, Some(PASSWORD), &[], Some(true));
            let page: Value = serde_json::from_str(&run.stdout).expect("sources page");
            listed.extend_from_slice(items(&page));
            after = page["next_cursor"].as_str().map(str::to_owned);
            if after.is_none() {
                break;
            }
        }
        Control { backup, sources: listed }
    }

    /// Every item of a paged listing, following `next_cursor` with `--after`.
    pub fn all(&mut self, command: &str, arguments: Vec<String>) -> Vec<Value> {
        let mut collected = vec![];
        let mut after: Option<String> = None;
        loop {
            let mut page_args = arguments.clone();
            if let Some(cursor) = &after {
                page_args.extend(["--after".into(), cursor.clone()]);
            }
            let page = self.call(command, page_args);
            collected.extend_from_slice(items(&page));
            after = page["next_cursor"].as_str().map(str::to_owned);
            if after.is_none() {
                break;
            }
            assert!(collected.len() < 100_000, "pagination must make progress");
        }
        collected
    }

    /// Every conversation `list` shows by default (put-away ones are hidden).
    pub fn conversations(&mut self) -> Vec<Value> {
        self.all("list", args(&["--limit", "500"]))
    }

    /// Every listed conversation, whatever its user state.
    pub fn every_conversation(&mut self) -> Vec<Value> {
        self.all("list", args(&["--put-away", "include", "--limit", "500"]))
    }

    /// The archive's source inventory keyed by source id.
    pub fn source_index(&mut self) -> BTreeMap<String, Value> {
        self.all("sources", args(&["--limit", "500"]))
            .into_iter()
            .map(|source| (string(&source, "id").to_owned(), source))
            .collect()
    }

    pub fn revisions_of(&mut self, source_id: &str) -> Vec<Value> {
        self.all("revisions", args(&["--source-id", source_id, "--limit", "500"]))
    }

    /// Exports one revision with `raw-export --execute` and returns the bytes.
    pub fn raw_export(&mut self, revision_id: &str) -> Vec<u8> {
        let output = self.unused_path("raw-export");
        self.call(
            "raw-export",
            args(&["--revision", revision_id, "--output", output.to_str().unwrap(), "--execute"]),
        );
        fs::read(output).unwrap()
    }

    pub fn unused_path(&self, prefix: &str) -> PathBuf {
        for index in 0.. {
            let path = self.root.join(format!("{prefix}-{index}.out"));
            if !path.exists() {
                return path;
            }
        }
        unreachable!()
    }
}

/// Checks the list item shape the contract fixes, so every test that reads a
/// conversation also pins its fields.
pub fn assert_conversation_shape(case: &mut Case, item: &Value) {
    let fields = [
        "id",
        "harness",
        "native_id",
        "parent_id",
        "parent_native_id",
        "forked_from_id",
        "forked_from_native_id",
        "sources",
        "only_in_archive",
        "attachments_not_kept",
        "starred",
        "put_away",
        "trashed",
        "title",
        "title_source",
        "project_path",
        "created_at",
        "updated_at",
        "model",
        "complete",
    ];
    let missing = fields.iter().filter(|field| item.get(**field).is_none()).collect::<Vec<_>>();
    case.equal("list item carries every contract field", json!(missing), json!([]));
    let id = string(item, "id");
    case.truth(
        "conversation id is lowercase hex SHA-256",
        id.len() == 64
            && id.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
    );
    case.truth(
        "parent_id is present",
        item["parent_id"].is_null() || item["parent_id"].is_string(),
    );
    case.truth(
        "a fork link is not a parent link",
        item["forked_from_native_id"].is_null() || item["parent_native_id"].is_null(),
    );
    let sources = item["sources"].as_array().expect("sources array");
    case.truth("a conversation has at least one source", !sources.is_empty());
    let current = sources.iter().filter(|source| source["current"] == json!(true)).count();
    case.equal("exactly one source is current", json!(current), json!(1));
    for source in sources {
        case.truth("source_id is a string", source["source_id"].is_string());
        match source["relation"].as_str() {
            Some("whole") => {}
            Some("continues") => case.truth(
                "continues names its base and boundary",
                source["base_source_id"].is_string() && source["base_end_ordinal"].is_u64(),
            ),
            Some("copy") => {
                case.truth("copy names its original", source["of_source_id"].is_string())
            }
            other => panic!("unknown relation {other:?}: {source}"),
        }
    }
    case.truth("only_in_archive is a boolean", item["only_in_archive"].is_boolean());
    case.truth("starred is a boolean", item["starred"].is_boolean());
    case.truth("put_away is a boolean", item["put_away"].is_boolean());
    case.truth("trashed is a boolean", item["trashed"].is_boolean());
    if item["trashed"] == json!(true) {
        case.truth("a trashed item carries trashed_at", !item["trashed_at"].is_null());
    }
    if item["title"].is_string() {
        case.truth(
            "title_source names where the title came from",
            matches!(item["title_source"].as_str(), Some("user" | "tool" | "first-message")),
        );
    }
}

pub fn current_source(item: &Value) -> &Value {
    item["sources"]
        .as_array()
        .expect("sources array")
        .iter()
        .find(|source| source["current"] == json!(true))
        .unwrap_or_else(|| panic!("no current source: {item}"))
}

pub fn by_native<'a>(items: &'a [Value], native_id: &str) -> Vec<&'a Value> {
    items.iter().filter(|item| item["native_id"] == json!(native_id)).collect()
}

/// Deterministic, incompressible bytes. zstd stores such data as a raw block, so a
/// marker placed inside it stays byte-searchable in an archive without encryption.
pub fn noise(seed: &str, length: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(length + 32);
    let mut counter = 0_u64;
    while bytes.len() < length {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(counter.to_be_bytes());
        bytes.extend_from_slice(&hasher.finalize());
        counter += 1;
    }
    bytes.truncate(length);
    bytes
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// `case.root` resolved, so roots built under it compare equal to what the archive stores.
pub fn canonical_root(case: &Case) -> PathBuf {
    fs::canonicalize(&case.root).unwrap()
}

/// Folder observations from `folders`, optionally filtered with `--path`.
pub fn folders(case: &mut Case, path: Option<&Path>) -> Vec<Value> {
    let arguments = match path {
        Some(path) => args(&["--path", path.to_str().unwrap()]),
        None => vec![],
    };
    items(&case.call("folders", arguments)).to_vec()
}

/// Whether a reported directory is `expected`, however the path is spelled.
pub fn same_place(actual: &Value, expected: &Path) -> bool {
    actual.as_str().and_then(|path| fs::canonicalize(path).ok()) == fs::canonicalize(expected).ok()
}

/// A `git` first on `PATH` that leaves a file behind if anything runs it.
#[cfg(unix)]
pub fn fake_git(case: &Case) -> (String, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let bin = case.root.join("fake-bin");
    let sentinel = case.root.join("git-was-run");
    let script = bin.join("git");
    super::write(&script, format!("#!/bin/sh\necho ran > '{}'\n", sentinel.display()).as_bytes());
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    (format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()), sentinel)
}
