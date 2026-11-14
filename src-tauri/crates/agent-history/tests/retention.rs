//! Responsibilities: pin archives without encryption (contract §1) and what is kept of
//! each retention kind (contract §5): appended, document, write-once, state, not captured.
//! Not responsible for: conversation identity or observations.
//! Failure modes: `--no-encryption` accepts a password or an encrypted archive opens
//! without one; a retained revision stops exporting byte-exact in either mode; an
//! appended file leaves one partial slice per backup; a rewritten document loses a
//! revision; a write-once file is stored twice; a state source keeps old snapshots or
//! drops their records; SQLite text stays hex; Codex's `thread_history_N.sqlite` is
//! planned or captured.
#[allow(dead_code)]
mod support;

use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use support::fixtures::{jsonl, pi, pi_message};
use support::harness::{noise, sha256_hex};
use support::native::*;
use support::{Case, PASSWORD, args, identifier, items, string, write};

fn source_arg(harness: &str, root: &Path) -> Vec<String> {
    vec!["--source".into(), format!("{harness}={}", root.display())]
}

fn backup(case: &mut Case, sources: &[String]) -> Value {
    let mut arguments = sources.to_vec();
    arguments.push("--execute".into());
    case.call("backup", arguments)
}

fn source_at(case: &mut Case, relative_path: &str) -> Value {
    let found = case
        .source_index()
        .into_values()
        .filter(|source| source["relative_path"] == json!(relative_path))
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "one source at {relative_path}");
    found[0].clone()
}

fn by_digest<'a>(revisions: &'a [Value], bytes: &[u8]) -> &'a Value {
    let digest = sha256_hex(bytes);
    revisions.iter().find(|revision| revision["digest"] == json!(digest)).expect("revision listed")
}

#[test]
fn an_archive_without_encryption_takes_no_password_and_refuses_one() {
    let mut case = Case::unencrypted("contract-no-encryption-passwords");
    case.fixture(json!({"seed":"contract-no-encryption-passwords-v1","private_data":false}));
    let refused = case.invoke_as("init", args(&["--no-encryption"]), Some(PASSWORD), false);
    case.equal(
        "--no-encryption with a password",
        refused["error"]["code"].clone(),
        json!("invalid_input"),
    );
    case.truth("the refused init creates nothing", !case.archive.exists());
    case.init();
    let listed = case.call("list", vec![]);
    case.equal("commands run without a password", json!(items(&listed).len()), json!(0));
    let given = case.invoke_as("list", vec![], Some(PASSWORD), false);
    case.equal(
        "a password for an unencrypted archive",
        given["error"]["code"].clone(),
        json!("archive_not_encrypted"),
    );
}

#[test]
fn an_encrypted_archive_opened_without_a_password_fails_with_password_required() {
    let mut case = Case::new("contract-password-required");
    case.fixture(json!({"seed":"contract-password-required-v1","private_data":false}));
    case.init();
    let missing = case.invoke_as("list", vec![], None, false);
    case.equal(
        "no password for an encrypted archive",
        missing["error"]["code"].clone(),
        json!("password_required"),
    );
}

#[test]
fn appended_revisions_are_all_listed_and_export_byte_exact_in_both_encryption_modes() {
    for encrypted in [true, false] {
        let name = format!(
            "contract-appended-revisions-{}",
            if encrypted { "encrypted" } else { "plain" }
        );
        let mut case = if encrypted { Case::new(&name) } else { Case::unencrypted(&name) };
        let root = case.root.join("pi");
        let file = pi(&root, "/synthetic/pi", "pi-appended", "appendedoriginalneedle");
        case.fixture(json!({"seed":"contract-appended-revisions-v1","encrypted":encrypted,"private_data":false}));
        case.init();
        let sources = source_arg("pi", &root);
        let mut versions = vec![fs::read(&file).unwrap()];
        backup(&mut case, &sources);
        let mut grown = versions[0].clone();
        grown.extend(jsonl(&[pi_message(
            "00000009",
            Some("00000003"),
            "user",
            "appendedlaterneedle",
        )]));
        let rewritten = String::from_utf8(grown.clone())
            .unwrap()
            .replace("appendedlaterneedle", "rewrittenhereneedle");
        let header_end = grown.iter().position(|byte| *byte == b'\n').unwrap() + 1;
        for next in [grown.clone(), rewritten.into_bytes(), grown[..header_end].to_vec()] {
            write(&file, &next);
            backup(&mut case, &sources);
            versions.push(next);
        }
        let source = case.source_index().into_values().next().expect("one source");
        let revisions = case.revisions_of(string(&source, "id"));
        case.equal("every revision is listed", json!(revisions.len()), json!(4));
        for version in &versions {
            let revision = identifier(by_digest(&revisions, version), "id");
            let exported = case.raw_export(&revision);
            case.truth(
                "a prefix, a rewrite and a truncation all export byte-exact",
                &exported == version,
            );
        }
        case.equal("a transcript is appended", source["retention"].clone(), json!("appended"));
    }
}

/// `verify` reports `slices.count`. An archive that backed a transcript up after each of
/// ten appends must hold as many slices as one that backed the final file up once: one
/// partial slice per superseded prefix revision would make it larger.
#[test]
fn an_appended_file_leaves_no_trail_of_partial_slices() {
    let mut case = Case::new("contract-appended-slice-trail");
    let root = case.root.join("pi");
    let file = pi(&root, "/synthetic/pi", "pi-slice-trail", "slicetrailneedle");
    case.fixture(
        json!({"seed":"contract-appended-slice-trail-v1","appends":10,"private_data":false}),
    );
    case.init();
    let sources = source_arg("pi", &root);
    let mut versions = vec![fs::read(&file).unwrap()];
    backup(&mut case, &sources);
    for index in 0..10 {
        let mut next = versions.last().unwrap().clone();
        let text = format!("appended line {index} of the slice trail");
        next.extend(jsonl(&[pi_message(
            &format!("append-{index}"),
            Some("00000003"),
            "user",
            &text,
        )]));
        write(&file, &next);
        let report = backup(&mut case, &sources);
        case.equal("each append is captured", report["captured"].clone(), json!(1));
        versions.push(next);
    }
    let source = case.source_index().into_values().next().expect("one source");
    let revisions = case.revisions_of(string(&source, "id"));
    case.equal("every revision is listed", json!(revisions.len()), json!(11));
    for version in &versions {
        let exported = case.raw_export(&identifier(by_digest(&revisions, version), "id"));
        case.truth("each prefix revision exports byte-exact", &exported == version);
    }
    let appended = case.call("verify", vec![])["slices"].clone();
    case.truth(
        "verify reports slice count and stored bytes",
        appended["count"].as_u64() > Some(0) && appended["bytes"].as_u64() > Some(0),
    );

    let mut once = Case::new("contract-appended-slice-trail-once");
    let root = once.root.join("pi");
    write(&root.join(file.strip_prefix(case.root.join("pi")).unwrap()), versions.last().unwrap());
    once.init();
    backup(&mut once, &source_arg("pi", &root));
    let single = once.call("verify", vec![])["slices"].clone();
    case.equal(
        "ten appends leave as many slices as one backup",
        appended["count"].clone(),
        single["count"].clone(),
    );
}

#[test]
fn a_rewritten_document_is_kept_whole_at_every_revision() {
    let mut case = Case::new("contract-document-retention");
    let root = case.root.join("continue");
    let id = "continue-document";
    let question = continue_entry("user", "documentquestionneedle");
    let versions = [
        vec![question.clone()],
        vec![question.clone(), continue_entry("assistant", "first answer that is replaced")],
        vec![question.clone(), continue_entry("assistant", "replacement answer")],
    ];
    case.fixture(
        json!({"seed":"contract-document-retention-v1","versions":versions,"private_data":false}),
    );
    case.init();
    let mut bytes = vec![];
    for history in &versions {
        let file = continue_session(&root, id, history);
        backup(&mut case, &source_arg("continue", &root));
        bytes.push(fs::read(file).unwrap());
    }
    let source = source_at(&mut case, &format!("sessions/{id}.json"));
    let revisions = case.revisions_of(string(&source, "id"));
    case.equal("every revision is listed", json!(revisions.len()), json!(3));
    for kept in &bytes {
        let exported = case.raw_export(&identifier(by_digest(&revisions, kept), "id"));
        case.truth("each revision exports byte-exact", &exported == kept);
    }
    case.equal("a Continue session is a document", source["retention"].clone(), json!("document"));
}

#[test]
fn a_write_once_attachment_has_one_revision_and_a_changed_file_is_a_new_one() {
    let mut case = Case::new("contract-write-once");
    let root = case.root.join("claude");
    let session = "ffffffff-6666-4666-8666-00000000000f";
    let first = noise("write-once-first", 4096);
    let upload = claude_upload(&root, session, "diagram.png", &first);
    claude_main(
        &root,
        session,
        &[
            claude_user(session, "u-1", None, "/synthetic/claude", "see the diagram"),
            claude_attachment(session, "att-1", &upload),
        ],
    );
    case.fixture(json!({"seed":"contract-write-once-v1","private_data":false}));
    case.init();
    let sources = source_arg("claude-code", &root);
    backup(&mut case, &sources);
    backup(&mut case, &sources);
    let relative = format!("uploads/{session}/diagram.png");
    let source = source_at(&mut case, &relative);
    let unchanged = case.revisions_of(string(&source, "id")).len();
    case.equal("an unchanged file has one revision", json!(unchanged), json!(1));
    let second = noise("write-once-second", 4096);
    write(&upload, &second);
    backup(&mut case, &sources);
    let revisions = case.revisions_of(string(&source, "id"));
    case.equal("a changed file is a new revision", json!(revisions.len()), json!(2));
    for bytes in [&first, &second] {
        let exported = case.raw_export(&identifier(by_digest(&revisions, bytes), "id"));
        case.truth("each revision exports byte-exact", &exported == bytes);
    }
    case.equal("an upload is write-once", source["retention"].clone(), json!("write-once"));
}

fn state_database(path: &Path, title: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS threads(id TEXT PRIMARY KEY, title TEXT, raw TEXT);
             INSERT OR IGNORE INTO threads VALUES('thread-1','',CAST(X'80FF0041' AS TEXT));",
        )
        .unwrap();
    connection.execute("UPDATE threads SET title=?1", [title]).unwrap();
}

#[test]
fn a_state_source_keeps_only_its_latest_verified_snapshot_in_the_v2_format() {
    let mut case = Case::new("contract-state-retention");
    let root = case.root.join("codex");
    let database = root.join("state_5.sqlite");
    state_database(&database, "first synthetic title");
    case.fixture(json!({"seed":"contract-state-retention-v1","private_data":false}));
    case.init();
    let sources = source_arg("codex", &root);
    let report = backup(&mut case, &sources);
    case.equal("backup captures the database", report["captured"].clone(), json!(1));
    state_database(&database, "second synthetic title");
    backup(&mut case, &sources);
    let source = source_at(&mut case, "state_5.sqlite");
    case.equal("a state database is state", source["retention"].clone(), json!("state"));
    let revisions = case.revisions_of(string(&source, "id"));
    let kept = revisions
        .iter()
        .filter(|revision| revision["bytes_retained"] == json!(true))
        .collect::<Vec<_>>();
    case.equal("one revision with bytes", json!(kept.len()), json!(1));
    let older =
        revisions.iter().map(|revision| revision["bytes_retained"].clone()).collect::<Vec<_>>();
    case.equal("the older snapshot keeps its record without bytes", json!(older.len()), json!(2));
    case.truth("and it is marked so", older.contains(&json!(false)));
    let exported = case.raw_export(&identifier(kept[0], "id"));
    let records = String::from_utf8(exported).unwrap();
    let records = records
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    case.equal("format name", records[0]["format"].clone(), json!("pathkeep-sqlite-evidence-v2"));
    let row = records
        .iter()
        .find(|record| record["type"] == "row" && record["table"] == "threads")
        .unwrap();
    case.equal(
        "the kept snapshot is the latest",
        row["values"][1].clone(),
        json!("second synthetic title"),
    );
    case.equal("UTF-8 text is a JSON string", row["values"][0].clone(), json!("thread-1"));
    case.equal(
        "other text stays hex with its class",
        row["values"][2].clone(),
        json!({"type":"text","hex":"80ff0041"}),
    );
}

#[test]
fn codex_thread_history_is_never_planned_or_captured() {
    let mut case = Case::new("contract-thread-history-not-captured");
    let root = case.root.join("codex");
    state_database(&root.join("state_5.sqlite"), "state title");
    let history = Connection::open(root.join("thread_history_1.sqlite")).unwrap();
    history.execute_batch("CREATE TABLE items(rollout_ordinal INTEGER, body TEXT);").unwrap();
    drop(history);
    codex_rollout(
        &root,
        "2026-11-10T12-00-00",
        "77777777-7777-4777-8777-777777777777",
        &[
            codex_meta("77777777-7777-4777-8777-777777777777", "/synthetic/codex"),
            codex_user("threadhistoryneedle"),
        ],
    );
    case.fixture(json!({"seed":"contract-thread-history-not-captured-v1","private_data":false}));
    let sources = source_arg("codex", &root);
    let plan = case.call("plan", sources.clone());
    let planned = plan["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| string(s, "relative_path").to_owned())
        .collect::<Vec<_>>();
    case.truth("the state database is planned", planned.contains(&"state_5.sqlite".to_owned()));
    case.truth(
        "thread_history_N.sqlite is not planned",
        !planned.iter().any(|path| path.starts_with("thread_history_")),
    );
    case.init();
    backup(&mut case, &sources);
    let captured = case
        .source_index()
        .into_values()
        .map(|s| string(&s, "relative_path").to_owned())
        .collect::<Vec<_>>();
    case.truth("nor captured", !captured.iter().any(|path| path.starts_with("thread_history_")));
}
