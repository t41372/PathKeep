//! Responsibilities: pin that a conversation, its user state and its kept revisions stay
//! reachable while the tool keeps changing the files underneath (contract §9).
//! Not responsible for: the identity rules themselves or Codex thread layouts.
//! Failure modes: a transcript captured mid-write gets an id from half a record and a
//! different one afterwards; a rewritten JSON document becomes a new conversation at
//! every rewrite; a DeepSeek conversation vanishes, with its star, while its newest
//! generation cannot be read, or shows the older generation as if it were current;
//! reading a conversation page by page fails because some other file was backed up;
//! a source that is reclassified loses the revisions it had already kept.
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use support::fixtures::{jsonl, pi};
use support::harness::assert_conversation_shape;
use support::native::*;
use support::{Case, args, items, string, write};

const SESSION: &str = "eeeeeeee-5555-4555-8555-00000000000e";

fn source_arg(harness: &str, root: &Path) -> Vec<String> {
    vec!["--source".into(), format!("{harness}={}", root.display())]
}

fn backup(case: &mut Case, sources: &[String]) -> Value {
    let mut arguments = sources.to_vec();
    arguments.push("--execute".into());
    case.call("backup", arguments)
}

fn mark(case: &mut Case, id: &str, flags: &[&str]) -> Value {
    let mut arguments = args(&["--conversation", id]);
    arguments.extend(args(flags));
    arguments.push("--execute".into());
    case.call("mark", arguments)
}

fn base(case: &Case) -> std::path::PathBuf {
    fs::canonicalize(&case.root).unwrap()
}

#[test]
fn a_transcript_caught_mid_write_has_no_conversation_until_its_first_record_is_whole() {
    let mut case = Case::new("contract-partial-first-record");
    let root = base(&case).join("claude");
    let cwd = "/synthetic/claude-partial";
    let rows = [
        claude_user(SESSION, "u-1", None, cwd, "partialfirstrecordneedle"),
        claude_reply(SESSION, "a-1", "u-1", cwd, "an answer"),
    ];
    let whole = jsonl(&rows);
    let first_line = whole.iter().position(|byte| *byte == b'\n').unwrap();
    let path = root.join(format!("{CLAUDE_PROJECT}/{SESSION}.jsonl"));
    case.fixture(
        json!({"seed":"contract-partial-first-record-v1","rows":rows,"private_data":false}),
    );
    case.init();

    // The tool has written half of the first record and not yet its line end.
    write(&path, &whole[..first_line / 2]);
    backup(&mut case, &source_arg("claude-code", &root));
    let kept = case.source_index().len();
    case.equal("the bytes are kept as a source", json!(kept), json!(1));
    let named = case.every_conversation().len();
    case.equal("half a record names no conversation", json!(named), json!(0));

    write(&path, &whole);
    backup(&mut case, &source_arg("claude-code", &root));
    let conversations = case.conversations();
    case.equal("one conversation once the record is whole", json!(conversations.len()), json!(1));
    assert_conversation_shape(&mut case, &conversations[0]);
    case.truth("and it is complete", conversations[0]["complete"] == json!(true));
    let source = case.source_index().into_keys().next().unwrap();
    let revisions = case.revisions_of(&source).len();
    case.equal("both captures are revisions", json!(revisions), json!(2));

    let mut clean = Case::new("contract-partial-first-record-clean");
    let clean_root = base(&clean).join("elsewhere/claude");
    write(&clean_root.join(format!("{CLAUDE_PROJECT}/{SESSION}.jsonl")), &whole);
    clean.init();
    backup(&mut clean, &source_arg("claude-code", &clean_root));
    case.equal(
        "its id is the one a capture of the finished file gives",
        conversations[0]["id"].clone(),
        clean.conversations()[0]["id"].clone(),
    );
}

#[test]
fn a_rewritten_document_stays_one_conversation_with_its_user_state() {
    let mut case = Case::new("contract-document-identity");
    let root = base(&case).join("continue");
    let id = "continue-stable-document";
    let question = continue_entry("user", "stabledocumentneedle");
    case.fixture(json!({"seed":"contract-document-identity-v1","private_data":false}));
    case.init();
    continue_session(&root, id, std::slice::from_ref(&question));
    backup(&mut case, &source_arg("continue", &root));
    let before = case.conversations();
    case.equal("one conversation", json!(before.len()), json!(1));
    let conversation = string(&before[0], "id").to_owned();
    mark(&mut case, &conversation, &["--star", "true"]);

    continue_session(&root, id, &[question, continue_entry("assistant", "a later answer")]);
    backup(&mut case, &source_arg("continue", &root));
    let after = case.conversations();
    case.equal("still one conversation", json!(after.len()), json!(1));
    assert_conversation_shape(&mut case, &after[0]);
    case.equal("with the same id", after[0]["id"].clone(), json!(conversation));
    case.equal("and its star", after[0]["starred"].clone(), json!(true));
}

#[test]
fn a_deepseek_conversation_stays_listed_while_its_newest_generation_cannot_be_read() {
    let mut case = Case::new("contract-deepseek-unreadable-generation");
    let root = base(&case).join("deepseek");
    let id = "deepseek-unreadable-generation";
    let cwd = "/synthetic/deepseek";
    let folder = deepseek_folder(&root, id);
    let sources = source_arg("deepseek", &root);
    case.fixture(json!({"seed":"contract-deepseek-unreadable-generation-v1","private_data":false}));
    case.init();
    write(
        &folder.join("session.v3.jsonl"),
        &jsonl(&[deepseek_header(id, 3, cwd), deepseek_user(0, "oldergenerationneedle")]),
    );
    backup(&mut case, &sources);
    let before = case.conversations();
    case.equal("one conversation", json!(before.len()), json!(1));
    let conversation = string(&before[0], "id").to_owned();
    mark(&mut case, &conversation, &["--star", "true", "--title", "Typed before the upgrade"]);

    // The tool has published a newer generation that this reader cannot decode.
    let newest = folder.join("session.v4.jsonl.zstd");
    write(&newest, b"these bytes are not a zstd frame");
    backup(&mut case, &sources);
    let during = case.conversations();
    case.equal("the conversation is still listed", json!(during.len()), json!(1));
    assert_conversation_shape(&mut case, &during[0]);
    case.equal("under the same id", during[0]["id"].clone(), json!(conversation));
    case.equal(
        "with its user state",
        json!([during[0]["starred"], during[0]["title"], during[0]["title_source"]]),
        json!([true, "Typed before the upgrade", "user"]),
    );
    case.equal("marked as not fully read", during[0]["complete"].clone(), json!(false));
    let events = case.all("events", args(&["--session", &conversation]));
    case.equal("the older generation does not stand in for the newest", json!(events), json!([]));
    let found = case.call("search", args(&["--query", "oldergenerationneedle"]));
    case.equal("nor in search", json!(items(&found)), json!([]));
    let marked = mark(&mut case, &conversation, &["--put-away", "true"]);
    case.equal("mark still reaches it", marked["put_away"].clone(), json!(true));
    mark(&mut case, &conversation, &["--put-away", "false"]);

    let mut bytes = zstd_frame(&[deepseek_header(id, 4, cwd)]);
    bytes.extend(zstd_frame(&[deepseek_user(0, "newestgenerationneedle")]));
    write(&newest, &bytes);
    backup(&mut case, &sources);
    let after = case.conversations();
    case.equal("one conversation once it can be read", json!(after.len()), json!(1));
    case.equal(
        "same id, complete, star kept",
        json!([after[0]["id"], after[0]["complete"], after[0]["starred"]]),
        json!([conversation, true, true]),
    );
    let events = case.all("events", args(&["--session", &conversation]));
    case.truth(
        "the line is the newest generation",
        events.iter().any(|event| event.to_string().contains("newestgenerationneedle"))
            && events.iter().all(|event| !event.to_string().contains("oldergenerationneedle")),
    );
}

#[test]
fn paging_a_conversation_is_not_disturbed_by_a_backup_of_another_file() {
    let mut case = Case::new("contract-cursor-unrelated-publish");
    let root = base(&case).join("pi");
    pi(&root, "/synthetic/pi-paged", "pi-being-read", "pagedconversationneedle");
    case.fixture(json!({"seed":"contract-cursor-unrelated-publish-v1","private_data":false}));
    case.init();
    backup(&mut case, &source_arg("pi", &root));
    let id = string(&case.conversations()[0], "id").to_owned();
    let all = case.all("events", args(&["--session", &id]));
    let first = case.call("events", args(&["--session", &id, "--limit", "1"]));
    let cursor = first["next_cursor"].as_str().expect("a second page").to_owned();

    pi(&root, "/synthetic/pi-other", "pi-written-meanwhile", "unrelatedconversationneedle");
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal("the backup captured the other file", report["captured"].clone(), json!(1));
    let second = case.call("events", args(&["--session", &id, "--limit", "1", "--after", &cursor]));
    case.equal(
        "the cursor still gives the next record",
        items(&second)[0]["ordinal"].clone(),
        all[1]["ordinal"].clone(),
    );
}

#[test]
fn a_reclassified_source_keeps_every_revision_it_already_had() {
    let mut case = Case::new("contract-reclassified-source");
    let root = base(&case).join("claude");
    let body = "local-agent-mode-sessions/account/org/local_reclassified";
    let audit = root.join(format!("{body}/audit.jsonl"));
    let record = |minute: u8, text: &str| {
        json!({"type":"assistant","uuid":format!("audit-{minute}"),"session_id":"cowork-native",
            "message":{"role":"assistant","content":[{"type":"text","text":text}]},
            "_audit_timestamp":format!("2026-11-10T12:{minute:02}:00Z")})
    };
    let mut rows = vec![
        json!({"type":"system","subtype":"init","session_id":"cowork-native",
            "cwd":"/synthetic/cowork","claude_code_version":"2.1.322","model":"synthetic-model",
            "_audit_timestamp":"2026-11-10T12:00:00Z"}),
        record(1, "auditfirstneedle"),
    ];
    let sources = source_arg("claude-code", &root);
    case.fixture(json!({"seed":"contract-reclassified-source-v1","private_data":false}));
    case.init();
    let mut kept = vec![];
    // While the session folder holds no transcript, its audit log is the transcript.
    for minute in [2, 3] {
        write(&audit, &jsonl(&rows));
        backup(&mut case, &sources);
        kept.push(jsonl(&rows));
        rows.push(record(minute, "audit grows"));
    }
    let source = case
        .source_index()
        .into_values()
        .find(|source| string(source, "relative_path").ends_with("audit.jsonl"))
        .expect("the audit log is a source");
    let source_id = string(&source, "id").to_owned();
    case.equal("an audit log is appended to", source["retention"].clone(), json!("appended"));

    // The session's own transcript appears, so the audit log is now only metadata.
    let cwd = "/synthetic/cowork";
    write(
        &root.join(format!("{body}/.claude/projects/p/{SESSION}.jsonl")),
        &jsonl(&[claude_user(SESSION, "u-1", None, cwd, "coworktranscriptneedle")]),
    );
    for _ in 0..2 {
        write(&audit, &jsonl(&rows));
        backup(&mut case, &sources);
        kept.push(jsonl(&rows));
        rows.push(record(rows.len() as u8 + 10, "audit keeps growing"));
    }
    let revisions = case.revisions_of(&source_id);
    case.equal("every capture is still a revision", json!(revisions.len()), json!(kept.len()));
    case.truth(
        "each keeps its bytes",
        revisions.iter().all(|revision| revision["bytes_retained"] == json!(true)),
    );
    for bytes in &kept {
        let digest = support::harness::sha256_hex(bytes);
        let revision = revisions
            .iter()
            .find(|revision| revision["digest"] == json!(digest))
            .expect("a revision for each captured state");
        let exported = case.raw_export(&support::identifier(revision, "id"));
        case.truth("and exports byte-exact", &exported == bytes);
    }
}

#[test]
fn an_attachment_that_was_not_kept_does_not_make_the_conversation_incomplete() {
    let mut case = Case::new("contract-attachment-not-kept");
    let root = base(&case).join("claude");
    let cwd = "/synthetic/claude-attachment";
    // A file the transcript refers to that lies outside every root PathKeep may read.
    let outside = base(&case).join("desktop/screenshot.png");
    write(&outside, b"not under the tool's folder");
    let other = "ffffffff-6666-4666-8666-00000000000f";
    claude_main(
        &root,
        SESSION,
        &[
            claude_user(SESSION, "u-1", None, cwd, "attachmentneedle"),
            claude_attachment(SESSION, "att-1", &outside),
        ],
    );
    claude_main(&root, other, &[claude_user(other, "u-2", None, cwd, "plainneedle")]);
    case.fixture(json!({"seed":"contract-attachment-not-kept-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("claude-code", &root));
    let issues = report["issues"].as_array().unwrap();
    let left_out =
        issues.iter().filter(|issue| issue["detail"] == json!("outside_roots")).collect::<Vec<_>>();
    case.equal("the run says one reference was outside its roots", json!(left_out.len()), json!(1));
    case.truth(
        "and does not repeat where it pointed",
        !report.to_string().contains("screenshot.png"),
    );
    let conversations = case.conversations();
    case.equal("two conversations", json!(conversations.len()), json!(2));
    for item in &conversations {
        assert_conversation_shape(&mut case, item);
        case.equal(
            "its records were read, so it is complete",
            item["complete"].clone(),
            json!(true),
        );
        let expected = if item["native_id"] == json!(SESSION) { 1 } else { 0 };
        case.equal(
            "the attachment that was not kept is counted on its own",
            item["attachments_not_kept"].clone(),
            json!(expected),
        );
    }
}
