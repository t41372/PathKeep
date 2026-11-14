//! Responsibilities: cover identity publication failures beyond the contract's initial captures.
//! Not responsible for: duplicating the contract's copy, parent-arrival or state scenarios.
//! Failure modes recorded before the fixes: a current source changing identity hides its
//! old conversation's surviving copy; identical parent native IDs bind children to strangers.
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::native::*;
use support::{Case, args, string};

#[test]
fn changing_one_copys_origin_keeps_the_original_conversation_and_its_state_visible() {
    let mut case = Case::unencrypted("identity-reassignment");
    let base = support::harness::canonical_root(&case);
    let first = base.join("first");
    let second = base.join("second");
    let id = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    let rows = [claude_user(id, "first", None, "/synthetic/project", "initial origin")];
    let a = claude_main(&first, id, &rows);
    claude_main(&second, id, &rows);
    let roots = args(&[
        "--source",
        &format!("claude-code={}", first.display()),
        "--source",
        &format!("claude-code={}", second.display()),
        "--execute",
    ]);
    case.fixture(json!({"seed":"identity-reassignment-v1","private_data":false}));
    case.init();
    case.call("backup", roots.clone());
    let before = case.conversations()[0].clone();
    let conversation = string(&before, "id").to_owned();
    case.call("mark", args(&["--conversation", &conversation, "--star", "true", "--execute"]));
    let current =
        support::harness::current_source(&before)["source_id"].as_str().unwrap().to_owned();
    let index = case.source_index();
    let changed =
        if index[&current]["root"] == json!(first.to_str().unwrap()) { &first } else { &second };
    claude_main(
        changed,
        id,
        &[claude_user(id, "different", None, "/synthetic/project", "different origin")],
    );
    case.call("backup", roots);
    let after = case.conversations();
    case.equal("both conversations remain visible", json!(after.len()), json!(2));
    let original = after.iter().find(|item| item["id"] == json!(conversation)).unwrap();
    case.equal(
        "original state stays with its surviving source",
        original["starred"].clone(),
        json!(true),
    );
    case.truth(
        "original has a current source",
        support::harness::current_source(original)["current"] == true,
    );
    case.truth("fixture used an actual native file", a.exists());
}

#[test]
fn children_resolve_reused_parent_native_ids_to_their_own_root_and_layout() {
    let mut case = Case::unencrypted("parent-root-disambiguation");
    let base = support::harness::canonical_root(&case);
    let first = base.join("first");
    let second = base.join("second");
    let id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    for (root, text) in [(&first, "first"), (&second, "second")] {
        claude_main(root, id, &[claude_user(id, text, None, "/synthetic/project", text)]);
        claude_subagent(root, id, "agent-worker.jsonl", "worker", "/synthetic/project", text);
    }
    case.fixture(json!({"seed":"parent-root-disambiguation-v1","private_data":false}));
    case.init();
    case.call(
        "backup",
        args(&[
            "--source",
            &format!("claude-code={}", first.display()),
            "--source",
            &format!("claude-code={}", second.display()),
            "--execute",
        ]),
    );
    let all = case.conversations();
    let sources = case.source_index();
    case.equal("strangers stay separate", json!(all.len()), json!(4));
    for child in all.iter().filter(|item| item["parent_native_id"].is_string()) {
        let parent = all.iter().find(|item| item["id"] == child["parent_id"]).unwrap();
        let child_source = support::harness::current_source(child)["source_id"].as_str().unwrap();
        let parent_source = support::harness::current_source(parent)["source_id"].as_str().unwrap();
        case.equal(
            "parent comes from the child's root",
            sources[child_source]["root"].clone(),
            sources[parent_source]["root"].clone(),
        );
    }
}
