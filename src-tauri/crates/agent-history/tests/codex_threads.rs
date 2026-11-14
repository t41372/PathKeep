//! Responsibilities: pin how the files of one Codex thread become one conversation
//! (contract §8): rollouts that each record their own time, chains of inherited
//! history, rollouts that inherit nothing, which rollout is current, and how a fork
//! and a spawned thread differ.
//! Not responsible for: retention, run observations or the other tools' identity rules.
//! Failure modes: every revert becomes its own conversation because its session record
//! carries a new time; a chain reads only one step back or repeats inherited records;
//! a rollout that continues another outranks one with later records; a tie is decided
//! by something other than the file name; trashing a thread hides its fork; a fork is
//! listed as a child; a page cursor outlives a republished base rollout.
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::path::Path;
use support::harness::{assert_conversation_shape, by_native, current_source};
use support::native::*;
use support::{Case, args, items, string, write};

const THREAD: &str = "33333333-3333-4333-8333-333333333333";
const FIRST: &str = "44444444-4444-4444-8444-444444444441";
const SECOND: &str = "44444444-4444-4444-8444-444444444442";

fn source_arg(root: &Path) -> Vec<String> {
    vec!["--source".into(), format!("codex={}", root.display())]
}

fn backup(case: &mut Case, root: &Path) -> Value {
    let mut arguments = source_arg(root);
    arguments.push("--execute".into());
    case.call("backup", arguments)
}

fn base(case: &Case) -> std::path::PathBuf {
    std::fs::canonicalize(&case.root).unwrap()
}

fn plain<'a>(thread: &'a str, hour: u8, turns: &'a [&'a str]) -> RolloutSpec<'a> {
    RolloutSpec {
        thread,
        rollout: None,
        name_hour: hour,
        clock_hour: hour,
        cwd: "/synthetic/codex-thread",
        base: None,
        meta: json!({}),
        turns,
    }
}

/// `(relative path, ordinal)` of every event on the conversation's current line.
fn line(case: &mut Case, conversation: &Value) -> Vec<(String, u64)> {
    let index = case.source_index();
    case.all("events", args(&["--session", string(conversation, "id"), "--limit", "2"]))
        .iter()
        .map(|event| {
            let source = &index[event["source_id"].as_str().unwrap()];
            (string(source, "relative_path").to_owned(), event["ordinal"].as_u64().unwrap())
        })
        .collect()
}

fn relative(root: &Path, rollout: &Rollout) -> String {
    rollout.path.strip_prefix(root).unwrap().to_str().unwrap().to_owned()
}

fn rows(root: &Path, rollout: &Rollout, ordinals: std::ops::Range<u64>) -> Vec<(String, u64)> {
    ordinals.map(|ordinal| (relative(root, rollout), ordinal)).collect()
}

fn current_path(case: &mut Case, conversation: &Value) -> String {
    let index = case.source_index();
    let id = current_source(conversation)["source_id"].as_str().unwrap();
    string(&index[id], "relative_path").to_owned()
}

#[test]
fn a_chain_of_reverts_reads_through_every_rollout_it_inherits_from() {
    let mut case = Case::new("contract-codex-chain");
    let root = base(&case).join("codex");
    let first = codex_thread_rollout(
        &root,
        plain(THREAD, 12, &["chainfirstneedle", "chainrevertedoneneedle"]),
    );
    let second = codex_thread_rollout(
        &root,
        RolloutSpec {
            rollout: Some(FIRST),
            base: Some((&first, 3)),
            ..plain(THREAD, 13, &["chainsecondneedle", "chainrevertedtwoneedle"])
        },
    );
    let third = codex_thread_rollout(
        &root,
        RolloutSpec {
            rollout: Some(SECOND),
            base: Some((&second, 3)),
            ..plain(THREAD, 14, &["chainthirdneedle"])
        },
    );
    case.fixture(json!({"seed":"contract-codex-chain-v1","rollouts":[first.rows, second.rows,
        third.rows],"private_data":false}));
    case.init();
    let report = backup(&mut case, &root);
    case.equal("backup captures three rollouts", report["captured"].clone(), json!(3));
    let conversations = case.conversations();
    case.equal(
        "one conversation, though each rollout records its own time",
        json!(conversations.len()),
        json!(1),
    );
    let conversation = conversations[0].clone();
    assert_conversation_shape(&mut case, &conversation);
    case.equal("three sources", json!(conversation["sources"].as_array().unwrap().len()), json!(3));
    case.equal(
        "a thread whose whole chain is in the archive is complete",
        conversation["complete"].clone(),
        json!(true),
    );
    case.equal(
        "the boundary is the tool's own number, counted along the base's whole history",
        current_source(&conversation)["base_end_ordinal"].clone(),
        json!(6),
    );
    let current = current_path(&mut case, &conversation);
    case.equal("the last rollout is current", json!(current), json!(relative(&root, &third)));
    let relations = conversation["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|source| source["relation"] == json!("continues"))
        .count();
    case.equal("two rollouts continue another", json!(relations), json!(2));

    let mut expected = rows(&root, &first, 0..3);
    expected.extend(rows(&root, &second, 0..3));
    expected.extend(rows(&root, &third, 0..3));
    let read = line(&mut case, &conversation);
    case.equal("the line runs through both inherited parts", json!(read), json!(expected));
    let events = case.all("events", args(&["--session", string(&conversation, "id")]));
    case.truth(
        "no reverted turn is on the current line",
        events.iter().all(|event| !event.to_string().contains("reverted")),
    );
}

#[test]
fn a_continuation_captured_before_its_base_is_complete_once_the_base_arrives() {
    let mut case = Case::new("contract-codex-base-arrives-later");
    let early = base(&case).join("codex-early");
    let late = base(&case).join("codex-late");
    // The base lives under a root that is only added on the second run.
    let first =
        codex_thread_rollout(&late, plain(THREAD, 12, &["latebaseneedle", "reverted turn"]));
    let second = codex_thread_rollout(
        &early,
        RolloutSpec {
            rollout: Some(FIRST),
            base: Some((&first, 3)),
            ..plain(THREAD, 13, &["earlycontinuationneedle"])
        },
    );
    case.fixture(json!({"seed":"contract-codex-base-arrives-later-v1","private_data":false}));
    case.init();
    backup(&mut case, &early);
    let before = case.conversations();
    case.equal("one conversation", json!(before.len()), json!(1));
    case.equal(
        "without its base the thread is not fully read",
        before[0]["complete"].clone(),
        json!(false),
    );
    let id = string(&before[0], "id").to_owned();

    let mut both = source_arg(&early);
    both.extend(source_arg(&late));
    both.push("--execute".into());
    let report = case.call("backup", both);
    case.equal("the second run captures only the base", report["captured"].clone(), json!(1));
    let after = case.conversations();
    case.equal("still one conversation", json!(after.len()), json!(1));
    case.equal("under the same id", after[0]["id"].clone(), json!(id));
    case.equal(
        "complete once the base is in the archive, without reparsing the continuation",
        after[0]["complete"].clone(),
        json!(true),
    );
    let mut expected = rows(&late, &first, 0..3);
    expected.extend(rows(&early, &second, 0..3));
    let read = line(&mut case, &after[0]);
    case.equal("the line begins with the inherited part", json!(read), json!(expected));
}

#[test]
fn the_rollout_with_the_latest_last_record_is_current_whatever_it_inherits() {
    let mut case = Case::new("contract-codex-current-rollout");
    let root = base(&case).join("codex");
    // A rollout that inherits nothing, with the latest records, beside one that continues.
    let first = codex_thread_rollout(&root, plain(THREAD, 12, &["currentfirstneedle"]));
    codex_thread_rollout(
        &root,
        RolloutSpec {
            rollout: Some(FIRST),
            base: Some((&first, 3)),
            ..plain(THREAD, 13, &["currentcontinuedneedle"])
        },
    );
    let alone = codex_thread_rollout(
        &root,
        RolloutSpec { rollout: Some(SECOND), ..plain(THREAD, 14, &["currentaloneneedle"]) },
    );
    // A second thread where the earlier-named rollout kept being written to.
    let other = "55555555-5555-4555-8555-555555555555";
    let other_first = codex_thread_rollout(&root, plain(other, 12, &["otherfirstneedle"]));
    let still_used = codex_thread_rollout(
        &root,
        RolloutSpec {
            rollout: Some(FIRST),
            base: Some((&other_first, 3)),
            clock_hour: 16,
            ..plain(other, 13, &["otherstillusedneedle"])
        },
    );
    codex_thread_rollout(
        &root,
        RolloutSpec { rollout: Some(SECOND), ..plain(other, 14, &["otherlaternameneedle"]) },
    );
    case.fixture(json!({"seed":"contract-codex-current-rollout-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &root);
    case.equal("backup captures six rollouts", report["captured"].clone(), json!(6));
    let conversations = case.conversations();
    case.equal("one conversation per thread", json!(conversations.len()), json!(2));

    let thread = by_native(&conversations, THREAD)[0].clone();
    assert_conversation_shape(&mut case, &thread);
    let current = current_path(&mut case, &thread);
    case.equal(
        "the rollout with the latest last record is current",
        json!(current),
        json!(relative(&root, &alone)),
    );
    case.equal(
        "a rollout with no history_base inherits nothing",
        current_source(&thread)["relation"].clone(),
        json!("whole"),
    );
    let read = line(&mut case, &thread);
    case.equal("its line is its own records", json!(read), json!(rows(&root, &alone, 0..3)));

    let second = by_native(&conversations, other)[0].clone();
    let current = current_path(&mut case, &second);
    case.equal(
        "a later file name does not outrank later records",
        json!(current),
        json!(relative(&root, &still_used)),
    );
}

#[test]
fn rollouts_that_end_at_the_same_time_are_decided_by_the_later_file_name() {
    let mut case = Case::new("contract-codex-current-tie");
    let root = base(&case).join("codex");
    let threads = [
        "66666666-6666-4666-8666-666666666661",
        "66666666-6666-4666-8666-666666666662",
        "66666666-6666-4666-8666-666666666663",
    ];
    let mut later = vec![];
    for (index, thread) in threads.iter().enumerate() {
        let text = format!("tie thread {index}");
        let turns = [text.as_str()];
        codex_thread_rollout(&root, plain(thread, 12, &turns));
        codex_thread_rollout(
            &root,
            RolloutSpec { rollout: Some(FIRST), clock_hour: 15, ..plain(thread, 13, &turns) },
        );
        later.push(codex_thread_rollout(
            &root,
            RolloutSpec { rollout: Some(SECOND), clock_hour: 15, ..plain(thread, 14, &turns) },
        ));
    }
    case.fixture(json!({"seed":"contract-codex-current-tie-v1","private_data":false}));
    case.init();
    backup(&mut case, &root);
    let conversations = case.conversations();
    case.equal("one conversation per thread", json!(conversations.len()), json!(3));
    for (thread, expected) in threads.iter().zip(&later) {
        let conversation = by_native(&conversations, thread)[0].clone();
        let current = current_path(&mut case, &conversation);
        case.equal(
            "a tie goes to the later time in the file name",
            json!(current),
            json!(relative(&root, expected)),
        );
    }
}

#[test]
fn trashing_a_thread_hides_the_threads_it_spawned_and_leaves_its_fork() {
    let mut case = Case::new("contract-codex-fork-and-spawn");
    let root = base(&case).join("codex");
    let fork = "77777777-7777-4777-8777-77777777777f";
    let child = "88888888-8888-4888-8888-88888888888c";
    let origin = codex_thread_rollout(&root, plain(THREAD, 12, &["forkoriginneedle"]));
    let forked = codex_thread_rollout(
        &root,
        RolloutSpec {
            base: Some((&origin, 3)),
            meta: json!({"forked_from_id":THREAD,"forked_from_ordinal_exclusive":3}),
            ..plain(fork, 13, &["forkedthreadneedle"])
        },
    );
    codex_thread_rollout(
        &root,
        RolloutSpec {
            meta: json!({"parent_thread_id":THREAD}),
            ..plain(child, 13, &["spawnedthreadneedle"])
        },
    );
    case.fixture(json!({"seed":"contract-codex-fork-and-spawn-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &root);
    case.equal("backup captures three rollouts", report["captured"].clone(), json!(3));
    let conversations = case.conversations();
    case.equal("three conversations", json!(conversations.len()), json!(3));
    for item in &conversations {
        assert_conversation_shape(&mut case, item);
    }
    let id = |native: &str| string(by_native(&conversations, native)[0], "id").to_owned();
    let (origin_id, fork_id, child_id) = (id(THREAD), id(fork), id(child));

    let forked_item = by_native(&conversations, fork)[0].clone();
    case.equal(
        "a fork has no parent",
        json!([forked_item["parent_native_id"], forked_item["parent_id"]]),
        json!([null, null]),
    );
    case.equal(
        "a fork names the thread it was forked from",
        json!([forked_item["forked_from_native_id"], forked_item["forked_from_id"]]),
        json!([THREAD, origin_id]),
    );
    let mut expected = rows(&root, &origin, 0..3);
    expected.extend(rows(&root, &forked, 0..3));
    let read = line(&mut case, &forked_item);
    case.equal(
        "a fork reads what it inherited from the other thread",
        json!(read),
        json!(expected),
    );
    let spawned = by_native(&conversations, child)[0].clone();
    case.equal(
        "a spawned thread names its parent",
        json!([spawned["parent_native_id"], spawned["parent_id"]]),
        json!([THREAD, origin_id]),
    );
    case.equal(
        "a spawned thread was not forked",
        spawned["forked_from_native_id"].clone(),
        json!(null),
    );

    let mut arguments = args(&["--conversation", &origin_id, "--trash", "true"]);
    arguments.push("--execute".into());
    case.call("mark", arguments);
    let listed = case
        .every_conversation()
        .iter()
        .map(|item| string(item, "id").to_owned())
        .collect::<Vec<_>>();
    case.equal("only the fork is still listed", json!(listed), json!([fork_id]));
    case.truth("the spawned thread left with its parent", !listed.contains(&child_id));
    let found = case.call("search", args(&["--query", "forkedthreadneedle"]));
    case.truth("the fork is still found", !items(&found).is_empty());
    let found = case.call("search", args(&["--query", "spawnedthreadneedle"]));
    case.equal("the spawned thread is not", json!(items(&found)), json!([]));
    let read = line(&mut case, &forked_item);
    case.equal("the fork still reads its inherited part", json!(read), json!(expected));
}

#[test]
fn a_cursor_goes_stale_when_a_rollout_on_its_line_is_republished() {
    let mut case = Case::new("contract-codex-cursor-on-a-chain");
    let root = base(&case).join("codex");
    let first = codex_thread_rollout(&root, plain(THREAD, 12, &["cursorfirstneedle"]));
    codex_thread_rollout(
        &root,
        RolloutSpec {
            rollout: Some(FIRST),
            base: Some((&first, 3)),
            ..plain(THREAD, 13, &["cursorsecondneedle"])
        },
    );
    case.fixture(json!({"seed":"contract-codex-cursor-on-a-chain-v1","private_data":false}));
    case.init();
    backup(&mut case, &root);
    let id = string(&case.conversations()[0], "id").to_owned();
    let page = case.call("events", args(&["--session", &id, "--limit", "2"]));
    let cursor = page["next_cursor"].as_str().expect("a second page").to_owned();

    // The inherited rollout grows after the boundary, so its view is published again.
    let mut grown = first.rows.clone();
    grown.push(codex_user_at("2026-11-10T12:30:00.000Z", "written after the boundary"));
    write(&first.path, &support::fixtures::jsonl(&grown));
    backup(&mut case, &root);
    let stale = case.fail("events", args(&["--session", &id, "--limit", "2", "--after", &cursor]));
    case.equal("the old cursor is refused", stale["error"]["code"].clone(), json!("stale_cursor"));
    let fresh = case.all("events", args(&["--session", &id, "--limit", "2"]));
    case.equal("a fresh read still gives the whole line", json!(fresh.len()), json!(6));
}
