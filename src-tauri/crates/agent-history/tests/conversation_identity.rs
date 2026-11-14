//! Responsibilities: pin how native files become conversations (contract §2): ids, parent
//! links, sources and their relations, and the current line `events` reads.
//! Not responsible for: user state, run observations or retention.
//! Failure modes: a copied beginning renames a Claude transcript; a subagent takes its
//! parent's id or turns incomplete over an unresolved link; a reverted Codex rollout
//! becomes a second conversation or `events` mixes in reverted records; a copy under a
//! second root becomes a second conversation or the shorter copy wins by capture order;
//! strangers with one native id merge; files without a native id merge; a lower DeepSeek
//! generation is current; an id depends on the root path, a reparse or an index rebuild.
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use support::fixtures::{SHARED_ID, jsonl, pi};
use support::harness::{assert_conversation_shape, by_native, current_source};
use support::native::*;
use support::{Case, args, items, string, write};

const ALPHA: &str = "aaaaaaaa-1111-4111-8111-00000000000a";
const BETA: &str = "bbbbbbbb-2222-4222-8222-00000000000b";
const THREAD: &str = "33333333-3333-4333-8333-333333333333";
const ROLLOUT: &str = "44444444-4444-4444-8444-444444444444";

fn source_arg(harness: &str, root: &std::path::Path) -> Vec<String> {
    vec!["--source".into(), format!("{harness}={}", root.display())]
}

fn backup(case: &mut Case, sources: &[String]) -> Value {
    let mut arguments = sources.to_vec();
    arguments.push("--execute".into());
    case.call("backup", arguments)
}

/// A canonical base directory, so source roots compare equal to what the archive stores.
fn base(case: &Case) -> std::path::PathBuf {
    fs::canonicalize(&case.root).unwrap()
}

#[test]
fn a_claude_transcript_is_keyed_by_its_file_name_even_when_it_begins_with_another_session() {
    let mut case = Case::new("contract-claude-copied-beginning");
    let root = base(&case).join("claude");
    let cwd = "/synthetic/claude-project";
    let beginning = [
        claude_user(ALPHA, "u-1", None, cwd, "shared beginning"),
        claude_reply(ALPHA, "a-1", "u-1", cwd, "shared answer"),
    ];
    let mut original = beginning.to_vec();
    original.push(claude_user(ALPHA, "u-2", Some("a-1"), cwd, "claudeoriginalneedle"));
    claude_main(&root, ALPHA, &original);
    let mut continued = beginning.to_vec();
    continued.push(claude_user(BETA, "u-3", Some("a-1"), cwd, "claudecontinuedneedle"));
    claude_main(&root, BETA, &continued);
    case.fixture(json!({"seed":"contract-claude-copied-beginning-v1","private_data":false}));
    let sources = source_arg("claude-code", &root);
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures both transcripts", report["captured"].clone(), json!(2));
    let conversations = case.conversations();
    case.equal("two conversations", json!(conversations.len()), json!(2));
    for item in &conversations {
        assert_conversation_shape(&mut case, item);
    }
    case.equal(
        "the original keeps its id",
        json!(by_native(&conversations, ALPHA).len()),
        json!(1),
    );
    case.equal(
        "the continued file is named after its own session, not its copied beginning",
        json!(by_native(&conversations, BETA).len()),
        json!(1),
    );
    let ids = conversations.iter().map(|item| string(item, "id")).collect::<BTreeSet<_>>();
    case.equal("distinct conversation ids", json!(ids.len()), json!(2));
    let original = by_native(&conversations, ALPHA)[0];
    let continued = by_native(&conversations, BETA)[0];
    case.equal(
        "a copied beginning is a fork link, never a parent",
        json!([
            continued["forked_from_native_id"],
            continued["forked_from_id"],
            continued["parent_native_id"]
        ]),
        json!([ALPHA, original["id"], null]),
    );
    case.equal(
        "the original was not forked",
        original["forked_from_native_id"].clone(),
        json!(null),
    );
}

#[test]
fn a_claude_subagent_is_its_own_conversation_under_its_parent() {
    let mut case = Case::new("contract-claude-subagents");
    let root = base(&case).join("claude");
    let cwd = "/synthetic/claude-project";
    claude_main(&root, ALPHA, &[claude_user(ALPHA, "u-1", None, cwd, "parent work")]);
    claude_subagent(&root, ALPHA, "agent-alpha.jsonl", "alpha", cwd, "first subtask");
    claude_subagent(&root, ALPHA, "workflows/run-1/agent-beta.jsonl", "beta", cwd, "nested task");
    case.fixture(json!({"seed":"contract-claude-subagents-v1","private_data":false}));
    let sources = source_arg("claude-code", &root);
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures three transcripts", report["captured"].clone(), json!(3));
    let conversations = case.conversations();
    case.equal("parent and both subagents are conversations", json!(conversations.len()), json!(3));
    for item in &conversations {
        assert_conversation_shape(&mut case, item);
    }
    let parent = conversations.iter().find(|item| item["parent_id"].is_null()).unwrap().clone();
    let children = conversations
        .iter()
        .filter(|item| item["parent_id"] == parent["id"])
        .cloned()
        .collect::<Vec<_>>();
    case.equal("both subagents name the parent conversation", json!(children.len()), json!(2));
    let ids = conversations.iter().map(|item| string(item, "id")).collect::<BTreeSet<_>>();
    case.equal("a subagent never shares its parent's id", json!(ids.len()), json!(3));
    for child in &children {
        case.equal(
            "a subagent whose records parse is complete",
            child["complete"].clone(),
            json!(true),
        );
    }
}

#[test]
fn a_subagent_names_its_parent_before_the_parent_is_captured_and_links_to_it_after() {
    let mut case = Case::new("contract-claude-orphan-subagent");
    let root = base(&case).join("claude");
    let cwd = "/synthetic/claude-project";
    claude_subagent(&root, ALPHA, "agent-alpha.jsonl", "alpha", cwd, "orphaned subtask");
    case.fixture(json!({"seed":"contract-claude-orphan-subagent-v1","private_data":false}));
    let sources = source_arg("claude-code", &root);
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures the subagent", report["captured"].clone(), json!(1));
    let before = case.conversations();
    case.equal("the subagent is listed alone", json!(before.len()), json!(1));
    let child = before[0].clone();
    assert_conversation_shape(&mut case, &child);
    case.equal(
        "the parent is named by its native id while it is not in the archive",
        child["parent_native_id"].clone(),
        json!(ALPHA),
    );
    case.truth(
        "parent_id is null until the parent is in the archive",
        child["parent_id"].is_null(),
    );
    case.equal(
        "an unresolved link to the parent does not make the subagent incomplete",
        child["complete"].clone(),
        json!(true),
    );

    claude_main(&root, ALPHA, &[claude_user(ALPHA, "u-1", None, cwd, "parent arrives later")]);
    backup(&mut case, &sources);
    let after = case.conversations();
    let parent = after
        .iter()
        .find(|item| item["native_id"] == json!(ALPHA) && item["parent_native_id"].is_null())
        .expect("parent listed");
    let child_after = after
        .iter()
        .find(|item| item["id"] == child["id"])
        .expect("the subagent keeps its id when the parent arrives");
    case.equal(
        "the link resolves once the parent is captured, without reparsing the subagent",
        child_after["parent_id"].clone(),
        parent["id"].clone(),
    );
}

#[test]
fn codex_rollouts_of_one_reverted_thread_are_one_conversation_on_the_continuation() {
    let mut case = Case::new("contract-codex-reverted-thread");
    let root = base(&case).join("codex");
    let thread = codex_reverted_thread(
        &root,
        THREAD,
        ROLLOUT,
        "/synthetic/codex-before",
        "/synthetic/codex-after",
    );
    case.fixture(json!({"seed":"contract-codex-reverted-thread-v1","boundary":thread.boundary,
        "base_rows":thread.base_rows,"continuation_rows":thread.continuation_rows,"private_data":false}));
    let sources = source_arg("codex", &root);
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures both rollouts", report["captured"].clone(), json!(2));
    let conversations = case.conversations();
    case.equal("one conversation for the thread", json!(conversations.len()), json!(1));
    let conversation = conversations[0].clone();
    assert_conversation_shape(&mut case, &conversation);
    case.equal("native id is the thread id", conversation["native_id"].clone(), json!(THREAD));
    case.equal(
        "a continuation whose base is in the archive is complete",
        conversation["complete"].clone(),
        json!(true),
    );
    let index = case.source_index();
    let path_of = |id: &Value| string(&index[id.as_str().unwrap()], "relative_path").to_owned();
    let relative =
        |path: &std::path::Path| path.strip_prefix(&root).unwrap().to_str().unwrap().to_owned();
    let listed = conversation["sources"].as_array().unwrap().clone();
    case.equal("two sources", json!(listed.len()), json!(2));
    let current = current_source(&conversation).clone();
    case.equal(
        "the later rollout is current",
        json!(path_of(&current["source_id"])),
        json!(relative(&thread.continuation)),
    );
    case.equal(
        "the later rollout continues the first",
        current["relation"].clone(),
        json!("continues"),
    );
    let other = listed.iter().find(|source| source["current"] == json!(false)).unwrap();
    case.equal(
        "the first rollout is the base",
        current["base_source_id"].clone(),
        other["source_id"].clone(),
    );
    case.equal("the base rollout is whole", other["relation"].clone(), json!("whole"));
    case.equal(
        "project comes from the current source",
        conversation["project_path"].clone(),
        json!("/synthetic/codex-after"),
    );

    let events =
        case.all("events", args(&["--session", string(&conversation, "id"), "--limit", "2"]));
    let line = events
        .iter()
        .map(|event| (path_of(&event["source_id"]), event["ordinal"].as_u64().unwrap()))
        .collect::<Vec<_>>();
    let boundary = current["base_end_ordinal"].as_u64().unwrap();
    case.equal("the boundary is the native exclusive end", json!(boundary), json!(thread.boundary));
    let mut expected =
        (0..boundary).map(|ordinal| (relative(&thread.base), ordinal)).collect::<Vec<_>>();
    expected.extend((0..3).map(|ordinal| (relative(&thread.continuation), ordinal)));
    case.equal(
        "events read the inherited part, then the continuation's own records",
        json!(line),
        json!(expected),
    );
    case.truth(
        "the reverted turn is not on the current line",
        events.iter().all(|event| !event.to_string().contains("codexrevertedneedle")),
    );
}

#[test]
fn a_byte_identical_copy_under_a_second_root_is_a_copy_of_one_conversation() {
    let mut case = Case::new("contract-identical-copy");
    let first = base(&case).join("pi-first");
    let second = base(&case).join("pi-second");
    let original = pi(&first, "/synthetic/pi-project", SHARED_ID, "identicalcopyneedle");
    let copy = second.join(original.strip_prefix(&first).unwrap());
    write(&copy, &fs::read(&original).unwrap());
    case.fixture(json!({"seed":"contract-identical-copy-v1","private_data":false}));
    let mut sources = source_arg("pi", &first);
    sources.extend(source_arg("pi", &second));
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures both files", report["captured"].clone(), json!(2));
    let conversations = case.conversations();
    case.equal("a copy is not a second conversation", json!(conversations.len()), json!(1));
    assert_conversation_shape(&mut case, &conversations[0]);
    let listed = conversations[0]["sources"].as_array().unwrap().clone();
    case.equal("both files are sources", json!(listed.len()), json!(2));
    let copies =
        listed.iter().filter(|source| source["relation"] == json!("copy")).collect::<Vec<_>>();
    case.equal("one source is the copy", json!(copies.len()), json!(1));
    let original_source = listed.iter().find(|source| source["relation"] != json!("copy")).unwrap();
    case.equal(
        "the copy names the other source",
        copies[0]["of_source_id"].clone(),
        original_source["source_id"].clone(),
    );
}

#[test]
fn the_longer_copy_is_current_whichever_root_was_captured_first() {
    for long_first in [false, true] {
        let mut case = Case::new(&format!("contract-longer-copy-current-{long_first}"));
        let long = base(&case).join("pi-long");
        let short = base(&case).join("pi-short");
        let original = pi(&long, "/synthetic/pi-project", SHARED_ID, "longercopyneedle");
        let bytes = fs::read(&original).unwrap();
        let second_line =
            bytes.iter().enumerate().filter(|(_, byte)| **byte == b'\n').nth(1).unwrap().0;
        write(&short.join(original.strip_prefix(&long).unwrap()), &bytes[..=second_line]);
        case.fixture(
            json!({"seed":"contract-longer-copy-v1","long_first":long_first,"private_data":false}),
        );
        let (first, later) = if long_first { (&long, &short) } else { (&short, &long) };
        case.init();
        let report = backup(&mut case, &source_arg("pi", first));
        case.equal("backup captures the first root", report["captured"].clone(), json!(1));
        let mut both = source_arg("pi", first);
        both.extend(source_arg("pi", later));
        let report = backup(&mut case, &both);
        case.equal("backup captures the second root", report["captured"].clone(), json!(1));
        let conversations = case.conversations();
        case.equal("one conversation", json!(conversations.len()), json!(1));
        assert_conversation_shape(&mut case, &conversations[0]);
        let index = case.source_index();
        let current = current_source(&conversations[0]);
        let root = string(&index[current["source_id"].as_str().unwrap()], "root").to_owned();
        case.equal("the longer copy is current", json!(root), json!(long.to_str().unwrap()));
        case.truth(
            "the shorter copy is listed as a copy",
            conversations[0]["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source["relation"] == json!("copy")),
        );
    }
}

#[test]
fn the_same_native_id_with_a_different_first_record_stays_two_conversations() {
    let mut case = Case::new("contract-native-id-strangers");
    let first = base(&case).join("pi-first");
    let second = base(&case).join("pi-second");
    pi(&first, "/synthetic/one", SHARED_ID, "strangeroneneedle");
    pi(&second, "/synthetic/two", SHARED_ID, "strangertwoneedle");
    case.fixture(json!({"seed":"contract-native-id-strangers-v1","private_data":false}));
    let mut sources = source_arg("pi", &first);
    sources.extend(source_arg("pi", &second));
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures both files", report["captured"].clone(), json!(2));
    let conversations = case.conversations();
    case.equal("strangers stay apart", json!(conversations.len()), json!(2));
    for item in &conversations {
        assert_conversation_shape(&mut case, item);
        case.equal(
            "native id without the origin mark",
            item["native_id"].clone(),
            json!(SHARED_ID),
        );
        let listed = item["sources"].as_array().unwrap();
        case.equal(
            "each has one whole source",
            json!(listed.iter().map(|s| s["relation"].clone()).collect::<Vec<_>>()),
            json!(["whole"]),
        );
    }
}

#[test]
fn files_without_a_native_id_are_one_conversation_each() {
    let mut case = Case::new("contract-no-native-id");
    let root = base(&case).join("codex");
    let rows = [json!({"type":"future_record","payload":{"preserve":"nativeidlessneedle"}})];
    for name in
        ["sessions/2026/11/10/rollout-first.jsonl", "sessions/2026/11/10/rollout-second.jsonl"]
    {
        write(&root.join(name), &jsonl(&rows));
    }
    case.fixture(json!({"seed":"contract-no-native-id-v1","rows":rows,"private_data":false}));
    let sources = source_arg("codex", &root);
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures both files", report["captured"].clone(), json!(2));
    let conversations = case.conversations();
    case.equal("identical files without an id stay apart", json!(conversations.len()), json!(2));
    for item in &conversations {
        assert_conversation_shape(&mut case, item);
        case.equal("one source each", json!(item["sources"].as_array().unwrap().len()), json!(1));
    }
}

#[test]
fn deepseek_generations_of_one_session_are_one_conversation_on_the_highest() {
    let mut case = Case::new("contract-deepseek-generations");
    let root = base(&case).join("deepseek");
    let (_, current) =
        deepseek_generations(&root, SHARED_ID, "/synthetic/deepseek", "generationneedle");
    case.fixture(json!({"seed":"contract-deepseek-generations-v1","private_data":false}));
    let sources = source_arg("deepseek", &root);
    case.init();
    let report = backup(&mut case, &sources);
    case.equal("backup captures both generations", report["captured"].clone(), json!(2));
    let conversations = case.conversations();
    case.equal("one conversation", json!(conversations.len()), json!(1));
    assert_conversation_shape(&mut case, &conversations[0]);
    case.equal(
        "both generations are sources",
        json!(conversations[0]["sources"].as_array().unwrap().len()),
        json!(2),
    );
    let index = case.source_index();
    let chosen = current_source(&conversations[0])["source_id"].as_str().unwrap().to_owned();
    case.equal(
        "the highest generation is current",
        json!(string(&index[&chosen], "relative_path")),
        json!(current.strip_prefix(&root).unwrap().to_str().unwrap()),
    );
}

#[test]
fn conversation_ids_survive_reparse_a_rebuild_and_another_root_path() {
    let mut case = Case::new("contract-derived-ids");
    let root = base(&case).join("pi");
    pi(&root, "/synthetic/pi-project", SHARED_ID, "derivedidneedle");
    pi(&root, "/synthetic/pi-project", "second-derived-id", "seconddocumentneedle");
    case.fixture(json!({"seed":"contract-derived-ids-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal("backup captures both files", report["captured"].clone(), json!(2));
    let ids = |items: &[Value]| {
        items.iter().map(|item| string(item, "id").to_owned()).collect::<BTreeSet<_>>()
    };
    let before = case.conversations();
    case.equal("two conversations", json!(before.len()), json!(2));
    for item in &before {
        assert_conversation_shape(&mut case, item);
    }
    let original = ids(&before);
    for source in case.source_index().into_keys() {
        case.call("reparse", args(&["--source-id", &source, "--execute"]));
    }
    let reparsed = ids(&case.conversations());
    case.equal("ids are the same after reparse", json!(reparsed), json!(original));
    case.call("sync-search", args(&["--rebuild"]));
    let rebuilt = ids(&case.conversations());
    case.equal("ids are the same after a rebuild", json!(rebuilt), json!(original));
    let found = case.call("search", args(&["--query", "derivedidneedle"]));
    case.truth("the rebuilt index still finds the text", !items(&found).is_empty());

    let mut elsewhere = Case::new("contract-derived-ids-elsewhere");
    let moved = base(&elsewhere).join("another/place/for/pi");
    pi(&moved, "/synthetic/pi-project", SHARED_ID, "derivedidneedle");
    pi(&moved, "/synthetic/pi-project", "second-derived-id", "seconddocumentneedle");
    elsewhere.init();
    backup(&mut elsewhere, &source_arg("pi", &moved));
    let derived_elsewhere = ids(&elsewhere.conversations());
    case.equal(
        "another archive under another root path derives the same ids",
        json!(derived_elsewhere),
        json!(original),
    );
}
