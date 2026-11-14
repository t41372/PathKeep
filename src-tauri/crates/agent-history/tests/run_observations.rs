//! Responsibilities: pin what a run records about each known source (contract §4):
//! seen or missing, unreadable roots, and that only the structured working-directory
//! field names a folder.
//! Not responsible for: git facts of folders (git_observations.rs), identity, retention.
//! Failure modes: a missing file is not recorded, or is recorded while its root is
//! unreadable, making a conversation look "only in PathKeep"; a folder named only in
//! message text or tool arguments is observed.
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use support::fixtures::pi;
use support::harness::{canonical_root as base, folders};
use support::native::*;
use support::{Case, write};

fn source_arg(harness: &str, root: &Path) -> Vec<String> {
    vec!["--source".into(), format!("{harness}={}", root.display())]
}

fn backup(case: &mut Case, sources: &[String]) -> Value {
    let mut arguments = sources.to_vec();
    arguments.push("--execute".into());
    case.call("backup", arguments)
}

/// The only source in the archive, from the `sources` command.
fn only_source(case: &mut Case) -> Value {
    let index = case.source_index();
    assert_eq!(index.len(), 1, "one source: {index:?}");
    index.into_values().next().unwrap()
}

#[test]
fn each_run_records_whether_a_source_was_seen() {
    let mut case = Case::new("contract-seen-and-missing");
    let root = case.root.join("pi");
    let file = pi(&root, "/synthetic/pi", "pi-seen", "seenneedle");
    let bytes = fs::read(&file).unwrap();
    case.fixture(json!({"seed":"contract-seen-and-missing-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal("backup captures the file", report["captured"].clone(), json!(1));
    let seen = only_source(&mut case);
    case.truth("a seen source has last_seen_at", !seen["last_seen_at"].is_null());
    case.truth(
        "missing_since is present and null",
        seen.get("missing_since") == Some(&Value::Null),
    );
    let only = |case: &mut Case| case.conversations()[0]["only_in_archive"].clone();
    let present = only(&mut case);
    case.equal("a seen conversation is not only in the archive", present, json!(false));

    fs::remove_file(&file).unwrap();
    backup(&mut case, &source_arg("pi", &root));
    let missing = only_source(&mut case);
    case.truth("a missing source has missing_since", !missing["missing_since"].is_null());
    case.equal(
        "last_seen_at keeps the last sighting",
        missing["last_seen_at"].clone(),
        seen["last_seen_at"].clone(),
    );
    let gone = only(&mut case);
    case.equal("every source missing: only in the archive", gone, json!(true));

    write(&file, &bytes);
    backup(&mut case, &source_arg("pi", &root));
    let back = only_source(&mut case);
    case.equal(
        "a source seen again is no longer missing",
        back["missing_since"].clone(),
        Value::Null,
    );
    let again = only(&mut case);
    case.equal("seen again: not only in the archive", again, json!(false));
}

#[cfg(unix)]
#[test]
fn an_unreadable_root_changes_nothing_about_its_sources() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
        }
    }
    let mut case = Case::new("contract-unreadable-root");
    let vanishing = case.root.join("pi-vanishing");
    let locked = case.root.join("pi-locked");
    pi(&vanishing, "/synthetic/one", "pi-vanishing", "vanishingneedle");
    pi(&locked, "/synthetic/two", "pi-locked", "lockedneedle");
    case.fixture(json!({"seed":"contract-unreadable-root-v1","private_data":false}));
    let mut sources = source_arg("pi", &vanishing);
    sources.extend(source_arg("pi", &locked));
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures both files", report["captured"].clone(), json!(2));
    let before = case.source_index();

    fs::rename(&vanishing, case.root.join("pi-moved-away")).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = Restore(locked.clone());
    let report = backup(&mut case, &sources);
    let unreadable = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|issue| issue["code"] == json!("root_unreadable"))
        .count();
    case.equal("both roots are reported as root-unreadable", json!(unreadable), json!(2));
    let after = case.source_index();
    for (id, source) in &before {
        case.truth("last_seen_at was recorded", !source["last_seen_at"].is_null());
        case.equal(
            "an unreadable root sets no missing_since",
            after[id]["missing_since"].clone(),
            Value::Null,
        );
        case.equal(
            "nor moves last_seen_at",
            after[id]["last_seen_at"].clone(),
            source["last_seen_at"].clone(),
        );
    }
    for item in case.conversations() {
        case.equal("not only in the archive", item["only_in_archive"].clone(), json!(false));
    }
}

#[test]
fn a_path_only_in_message_text_or_tool_arguments_is_never_observed() {
    let mut case = Case::new("contract-structured-cwd-only");
    let projects = base(&case).join("projects");
    let (structured, text_only, tool_only) =
        (projects.join("structured"), projects.join("text-only"), projects.join("tool-only"));
    for folder in [&structured, &text_only, &tool_only] {
        git_repository(folder, "main", &[]);
    }
    let root = base(&case).join("claude");
    let session = "dddddddd-4444-4444-8444-00000000000d";
    let cwd = structured.to_str().unwrap();
    claude_main(
        &root,
        session,
        &[
            claude_user(
                session,
                "u-1",
                None,
                cwd,
                &format!("please look at {}", text_only.display()),
            ),
            claude_tool_call(
                session,
                "a-1",
                "u-1",
                cwd,
                json!({
                    "command":format!("cd {} && ls", tool_only.display()),
                    "cwd":tool_only.to_str().unwrap(),
                    "file_path":tool_only.join("notes.txt").to_str().unwrap(),
                }),
            ),
        ],
    );
    case.fixture(json!({"seed":"contract-structured-cwd-only-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("claude-code", &root));
    case.equal("backup captures the transcript", report["captured"].clone(), json!(1));
    let observed = folders(&mut case, None);
    let paths = observed.iter().map(|entry| entry["path"].clone()).collect::<Vec<_>>();
    case.equal("only the structured working directory is observed", json!(paths), json!([cwd]));
    case.truth("and its git facts are read", observed[0]["git"].is_object());
    for folder in [&text_only, &tool_only] {
        let found = folders(&mut case, Some(folder)).len();
        case.equal("never observed", json!(found), json!(0));
    }
}
