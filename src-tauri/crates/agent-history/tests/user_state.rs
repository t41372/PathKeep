//! Responsibilities: pin what the user writes about a conversation (contract §3): `mark`,
//! the `list` filters over it, and that it lives with the evidence, not the projection.
//! Not responsible for: how conversations are identified (conversation_identity.rs).
//! Failure modes: `mark` without `--execute` writes; a reparse or an index rebuild drops
//! stars, put-away or trash flags or typed titles; a moved file or an added copy under a
//! second root starts a fresh conversation without its state; a cleared title does not
//! fall back to the tool's; put-away hides from search or its hits are not marked; a
//! trashed conversation or its subagent still shows in `list` or `search`, stops being
//! backed up, loses its evidence or its state when restored; an unknown id is accepted.
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use support::fixtures::{jsonl, pi, pi_message};
use support::harness::{assert_conversation_shape, by_native};
use support::native::*;
use support::{Case, args, items, string, write};

const SESSION: &str = "cccccccc-3333-4333-8333-00000000000c";

fn backup(case: &mut Case, sources: &[String]) -> Value {
    let mut arguments = sources.to_vec();
    arguments.push("--execute".into());
    case.call("backup", arguments)
}

fn source_arg(harness: &str, root: &Path) -> Vec<String> {
    vec!["--source".into(), format!("{harness}={}", root.display())]
}

fn mark(case: &mut Case, id: &str, flags: &[&str]) -> Value {
    let mut arguments = args(&["--conversation", id]);
    arguments.extend(args(flags));
    arguments.push("--execute".into());
    case.call("mark", arguments)
}

fn ids(items: &[Value]) -> BTreeSet<String> {
    items.iter().map(|item| string(item, "id").to_owned()).collect()
}

/// Ids `list` returns with these extra flags.
fn listed(case: &mut Case, flags: &[&str]) -> BTreeSet<String> {
    let mut arguments = args(&["--limit", "500"]);
    arguments.extend(args(flags));
    ids(&case.all("list", arguments))
}

fn hits(case: &mut Case, query: &str) -> Vec<Value> {
    items(&case.call("search", args(&["--query", query]))).to_vec()
}

fn id_of(items: &[Value], native_id: &str) -> String {
    let found = by_native(items, native_id);
    assert_eq!(found.len(), 1, "one conversation for {native_id}");
    string(found[0], "id").to_owned()
}

/// A Pi session whose `session_info` record names it, so its title comes from the tool.
fn pi_named(root: &Path, id: &str, name: &str, text: &str) -> PathBuf {
    let path =
        root.join(format!("sessions/--not-a-reversible-path--/2026-11-10T12-00-00Z_{id}.jsonl"));
    write(
        &path,
        &jsonl(&[
            json!({"type":"session","version":3,"id":id,"timestamp":TIME,"cwd":"/synthetic/pi"}),
            pi_message("00000001", None, "user", text),
            json!({"type":"session_info","id":"00000002","parentId":"00000001","timestamp":TIME,
                "name":name}),
        ]),
    );
    path
}

#[test]
fn mark_returns_the_conversation_and_list_filters_by_star_and_put_away() {
    let mut case = Case::new("contract-mark-and-filters");
    let root = case.root.join("pi");
    for (id, text) in
        [("pi-one", "starredneedle"), ("pi-two", "putawayneedle"), ("pi-three", "plainneedle")]
    {
        pi(&root, "/synthetic/pi", id, text);
    }
    case.fixture(json!({"seed":"contract-mark-and-filters-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal("backup captures three files", report["captured"].clone(), json!(3));
    let all = case.conversations();
    let (one, two, three) = (id_of(&all, "pi-one"), id_of(&all, "pi-two"), id_of(&all, "pi-three"));

    let preview = case.call("mark", args(&["--conversation", &one, "--star", "true"]));
    case.truth("a preview answers with a document", preview.is_object());
    let after_preview = case.conversations();
    let item = after_preview.iter().find(|item| item["id"] == json!(one)).unwrap();
    case.equal("mark without --execute changes nothing", item["starred"].clone(), json!(false));

    let starred = mark(&mut case, &one, &["--star", "true"]);
    assert_conversation_shape(&mut case, &starred);
    case.equal("mark returns the conversation item", starred["id"].clone(), json!(one));
    case.equal("the star is set", starred["starred"].clone(), json!(true));
    let hidden = mark(&mut case, &two, &["--put-away", "true"]);
    case.equal("put away is set", hidden["put_away"].clone(), json!(true));

    let set =
        |values: &[&String]| values.iter().map(|value| (*value).clone()).collect::<BTreeSet<_>>();
    let default = listed(&mut case, &[]);
    case.equal(
        "put-away conversations are excluded by default",
        json!(default),
        json!(set(&[&one, &three])),
    );
    let exclude = listed(&mut case, &["--put-away", "exclude"]);
    case.equal("--put-away exclude", json!(exclude), json!(set(&[&one, &three])));
    let only = listed(&mut case, &["--put-away", "only"]);
    case.equal("--put-away only", json!(only), json!(set(&[&two])));
    let include = listed(&mut case, &["--put-away", "include"]);
    case.equal("--put-away include", json!(include), json!(set(&[&one, &two, &three])));
    let star = listed(&mut case, &["--starred"]);
    case.equal("--starred", json!(star), json!(set(&[&one])));

    let found = case.call("search", args(&["--query", "putawayneedle"]));
    case.truth("search still finds a put-away conversation", !items(&found).is_empty());
    case.truth(
        "each of its hits is marked put_away: true",
        items(&found).iter().all(|hit| hit["put_away"] == json!(true)),
    );
    let plain = case.call("search", args(&["--query", "plainneedle"]));
    case.truth(
        "hits of a conversation that is not put away are not marked",
        !items(&plain).is_empty()
            && items(&plain).iter().all(|hit| hit["put_away"] == json!(false)),
    );

    let unstarred = mark(&mut case, &one, &["--star", "false"]);
    case.equal("the star can be removed", unstarred["starred"].clone(), json!(false));
}

#[test]
fn a_typed_title_replaces_the_tool_title_until_it_is_cleared() {
    let mut case = Case::new("contract-typed-title");
    let root = case.root.join("pi");
    pi_named(&root, "pi-named", "Title the tool wrote", "namedsessionneedle");
    pi(&root, "/synthetic/pi", "pi-unnamed", "unnamedsessionneedle");
    case.fixture(json!({"seed":"contract-typed-title-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal("backup captures both files", report["captured"].clone(), json!(2));
    let all = case.conversations();
    let named = by_native(&all, "pi-named")[0].clone();
    case.equal("the tool's title", named["title"].clone(), json!("Title the tool wrote"));
    assert_conversation_shape(&mut case, &named);
    case.equal("comes from the tool", named["title_source"].clone(), json!("tool"));
    let unnamed = by_native(&all, "pi-unnamed")[0].clone();
    case.equal("a session without a name", unnamed["title_source"].clone(), json!("first-message"));

    let typed = mark(&mut case, string(&named, "id"), &["--title", "Typed by the user"]);
    case.equal("the typed title is shown", typed["title"].clone(), json!("Typed by the user"));
    case.equal("and comes from the user", typed["title_source"].clone(), json!("user"));
    let cleared = mark(&mut case, string(&named, "id"), &["--clear-title"]);
    case.equal(
        "clearing restores the tool's title",
        cleared["title"].clone(),
        json!("Title the tool wrote"),
    );
    case.equal("from the tool again", cleared["title_source"].clone(), json!("tool"));

    mark(&mut case, string(&unnamed, "id"), &["--title", "Second typed title"]);
    let cleared = mark(&mut case, string(&unnamed, "id"), &["--clear-title"]);
    case.equal(
        "clearing falls back to the first message",
        cleared["title_source"].clone(),
        json!("first-message"),
    );
}

#[test]
fn user_state_survives_reparse_of_every_source_and_a_search_rebuild() {
    let mut case = Case::new("contract-state-survives-reparse");
    let root = case.root.join("pi");
    pi(&root, "/synthetic/pi", "pi-kept", "keptstateneedle");
    case.fixture(json!({"seed":"contract-state-survives-reparse-v1","private_data":false}));
    case.init();
    backup(&mut case, &source_arg("pi", &root));
    let id = id_of(&case.conversations(), "pi-kept");
    mark(&mut case, &id, &["--star", "true", "--put-away", "true", "--title", "Kept title"]);

    let state = |case: &mut Case| {
        let all = case.every_conversation();
        let item =
            all.iter().find(|item| item["id"] == json!(id)).expect("conversation listed").clone();
        json!([item["starred"], item["put_away"], item["title"], item["title_source"]])
    };
    let expected = json!([true, true, "Kept title", "user"]);
    let marked = state(&mut case);
    case.equal("state is set", marked, expected.clone());
    let item = case.every_conversation().into_iter().find(|item| item["id"] == json!(id)).unwrap();
    for source in item["sources"].as_array().unwrap() {
        case.call(
            "reparse",
            args(&["--source-id", source["source_id"].as_str().unwrap(), "--execute"]),
        );
    }
    let reparsed = state(&mut case);
    case.equal("state survives reparse of every source", reparsed, expected.clone());
    case.call("sync-search", args(&["--rebuild"]));
    let rebuilt = state(&mut case);
    case.equal("state survives a search rebuild", rebuilt, expected);
}

#[test]
fn user_state_follows_the_conversation_when_the_tool_moves_its_file() {
    let mut case = Case::new("contract-state-follows-move");
    let root = case.root.join("claude");
    let cwd = "/synthetic/claude-project";
    let file = claude_main(
        &root,
        SESSION,
        &[
            claude_user(SESSION, "u-1", None, cwd, "movedfileneedle"),
            claude_reply(SESSION, "a-1", "u-1", cwd, "an answer"),
        ],
    );
    case.fixture(json!({"seed":"contract-state-follows-move-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("claude-code", &root));
    case.equal("backup captures the transcript", report["captured"].clone(), json!(1));
    let id = id_of(&case.conversations(), SESSION);
    mark(&mut case, &id, &["--star", "true", "--title", "Follows the file"]);

    let moved = root.join(format!("projects/--another-project-folder--/{SESSION}.jsonl"));
    fs::create_dir_all(moved.parent().unwrap()).unwrap();
    fs::rename(&file, &moved).unwrap();
    backup(&mut case, &source_arg("claude-code", &root));
    let all = case.conversations();
    case.equal("a moved file is the same conversation", json!(ids(&all)), json!([id.clone()]));
    let item = &all[0];
    case.equal("the star stays", item["starred"].clone(), json!(true));
    case.equal("the typed title stays", item["title"].clone(), json!("Follows the file"));
}

#[test]
fn user_state_stays_when_a_second_root_with_a_copy_is_added() {
    let mut case = Case::new("contract-state-with-copy");
    let first = case.root.join("pi-first");
    let second = case.root.join("pi-second");
    let original = pi(&first, "/synthetic/pi", "pi-copied", "copiedstateneedle");
    case.fixture(json!({"seed":"contract-state-with-copy-v1","private_data":false}));
    case.init();
    backup(&mut case, &source_arg("pi", &first));
    let id = id_of(&case.conversations(), "pi-copied");
    mark(&mut case, &id, &["--star", "true", "--title", "Copied title"]);

    write(&second.join(original.strip_prefix(&first).unwrap()), &fs::read(&original).unwrap());
    let mut both = source_arg("pi", &first);
    both.extend(source_arg("pi", &second));
    let report = backup(&mut case, &both);
    case.equal("the copy is captured", report["captured"].clone(), json!(1));
    let all = case.conversations();
    case.equal("still one conversation", json!(ids(&all)), json!([id.clone()]));
    case.equal("with two sources", json!(all[0]["sources"].as_array().map(Vec::len)), json!(2));
    case.equal("the star stays", all[0]["starred"].clone(), json!(true));
    case.equal("the typed title stays", all[0]["title"].clone(), json!("Copied title"));
}

#[test]
fn mark_on_an_unknown_conversation_fails_with_conversation_not_found() {
    let mut case = Case::new("contract-mark-unknown");
    let root = case.root.join("pi");
    pi(&root, "/synthetic/pi", "pi-known", "knownneedle");
    case.fixture(json!({"seed":"contract-mark-unknown-v1","private_data":false}));
    case.init();
    backup(&mut case, &source_arg("pi", &root));
    let before = case.conversations();
    case.equal("one conversation", json!(before.len()), json!(1));
    let unknown = "0".repeat(64);
    let failure =
        case.fail("mark", args(&["--conversation", &unknown, "--star", "true", "--execute"]));
    case.equal("unknown id", failure["error"]["code"].clone(), json!("conversation_not_found"));
    let after = case.conversations();
    case.equal("nothing changed", json!(after), json!(before));
}

#[test]
fn a_trashed_conversation_and_its_subagent_leave_list_and_search_until_restored() {
    let mut case = Case::new("contract-trash-and-restore");
    let root = case.root.join("claude");
    let cwd = "/synthetic/claude-trash";
    let other = "dddddddd-4444-4444-8444-00000000000d";
    claude_main(&root, SESSION, &[claude_user(SESSION, "u-1", None, cwd, "trashedmainneedle")]);
    claude_subagent(&root, SESSION, "agent-helper.jsonl", "helper", cwd, "trashedsubagentneedle");
    claude_main(&root, other, &[claude_user(other, "u-9", None, cwd, "untouchedneedle")]);
    case.fixture(json!({"seed":"contract-trash-and-restore-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("claude-code", &root));
    case.equal("backup captures three transcripts", report["captured"].clone(), json!(3));
    let all = case.conversations();
    // The subagent's records carry the parent's session id, so pick the main transcript
    // by its missing parent rather than by native id alone.
    let main =
        all.iter().find(|item| item["native_id"] == json!(SESSION) && item["parent_id"].is_null());
    let id = string(main.expect("main transcript"), "id").to_owned();
    let other_id = id_of(&all, other);
    let child = all.iter().find(|item| item["parent_id"] == json!(id));
    case.truth("the subagent names its parent", child.is_some());
    let child = string(child.unwrap(), "id").to_owned();
    mark(
        &mut case,
        &id,
        &["--star", "true", "--put-away", "true", "--title", "Typed before trash"],
    );
    let source =
        all.iter().find(|item| item["id"] == json!(id)).unwrap()["sources"][0]["source_id"]
            .as_str()
            .unwrap()
            .to_owned();

    case.call("mark", args(&["--conversation", &id, "--trash", "true"]));
    let preview = listed(&mut case, &["--trash", "only"]);
    case.equal("trash without --execute changes nothing", json!(preview), json!([]));
    let trashed = mark(&mut case, &id, &["--trash", "true"]);
    assert_conversation_shape(&mut case, &trashed);
    case.equal("the item is trashed", trashed["trashed"].clone(), json!(true));
    for flags in [&[][..], &["--put-away", "include"], &["--put-away", "only"], &["--starred"]] {
        let shown = listed(&mut case, flags);
        case.truth(
            "no list shows it or its subagent",
            !shown.contains(&id) && !shown.contains(&child),
        );
    }
    let bin = listed(&mut case, &["--trash", "only"]);
    case.truth("--trash only shows it", bin.contains(&id));
    let shown_1 = listed(&mut case, &["--trash", "only", "--put-away", "exclude"]);
    case.truth("--trash only ignores --put-away", shown_1.contains(&id));
    for word in ["trashedmainneedle", "trashedsubagentneedle"] {
        let found = hits(&mut case, word);
        case.equal("search returns nothing of it", json!(found), json!([]));
    }
    let shown_2 = listed(&mut case, &[]);
    case.truth("other conversations are untouched", shown_2.contains(&other_id));
    let events = case.all("events", args(&["--session", &id]));
    case.truth("events still answer by id", !events.is_empty());
    let revisions = case.revisions_of(&source);
    case.equal("its revision stays", json!(revisions.len()), json!(1));

    let restored = mark(&mut case, &id, &["--trash", "false"]);
    case.equal(
        "restoring brings back its state exactly",
        json!([
            restored["trashed"],
            restored["starred"],
            restored["put_away"],
            restored["title"],
            restored["title_source"]
        ]),
        json!([false, true, true, "Typed before trash", "user"]),
    );
    let back = listed(&mut case, &["--put-away", "include"]);
    case.truth("it and its subagent are listed again", back.contains(&id) && back.contains(&child));
    let found = hits(&mut case, "trashedsubagentneedle");
    case.truth("and searchable again", !found.is_empty());
}

#[test]
fn a_trashed_conversation_keeps_being_backed_up() {
    let mut case = Case::new("contract-trash-still-backed-up");
    let root = case.root.join("pi");
    let file = pi(&root, "/synthetic/pi", "pi-trashed", "trashedpineedle");
    case.fixture(json!({"seed":"contract-trash-still-backed-up-v1","private_data":false}));
    case.init();
    backup(&mut case, &source_arg("pi", &root));
    let id = id_of(&case.conversations(), "pi-trashed");
    mark(&mut case, &id, &["--trash", "true"]);
    let mut grown = fs::read(&file).unwrap();
    grown.extend(jsonl(&[pi_message("00000009", Some("00000003"), "user", "afterthetrashneedle")]));
    write(&file, &grown);
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal("new bytes are captured", report["captured"].clone(), json!(1));
    let source = case.source_index().into_values().next().expect("one source");
    let revisions = case.revisions_of(string(&source, "id"));
    case.equal("both revisions are kept", json!(revisions.len()), json!(2));
    let newest = revisions
        .iter()
        .find(|revision| revision["byte_len"] == json!(grown.len()))
        .expect("the new revision");
    let exported = case.raw_export(&support::identifier(newest, "id"));
    case.truth("the new revision exports byte-exact", exported == grown);
    let shown_3 = listed(&mut case, &["--trash", "only"]);
    case.truth("it is still in the trash", shown_3.contains(&id));
}

#[test]
fn trash_survives_reparse_and_a_search_rebuild() {
    let mut case = Case::new("contract-trash-survives-reparse");
    let root = case.root.join("pi");
    pi(&root, "/synthetic/pi", "pi-trash-kept", "trashkeptneedle");
    case.fixture(json!({"seed":"contract-trash-survives-reparse-v1","private_data":false}));
    case.init();
    backup(&mut case, &source_arg("pi", &root));
    let id = id_of(&case.conversations(), "pi-trash-kept");
    mark(&mut case, &id, &["--trash", "true"]);
    for source in case.source_index().into_keys() {
        case.call("reparse", args(&["--source-id", &source, "--execute"]));
    }
    case.call("sync-search", args(&["--rebuild"]));
    let shown_4 = listed(&mut case, &["--trash", "only"]);
    case.truth("still in the trash", shown_4.contains(&id));
    let shown_5 = listed(&mut case, &["--put-away", "include"]);
    case.truth("still out of the list", !shown_5.contains(&id));
    let found = hits(&mut case, "trashkeptneedle");
    case.equal("still out of search", json!(found), json!([]));
}
