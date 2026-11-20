//! Project identity, grouping and reversible user choices over native Pi backups.
#[allow(dead_code)]
mod support;
use agent_history::{
    Archive, Harness, SourceRoot,
    capture::BackupOptions,
    chats::ListQuery,
    projects::{Grouping, ProjectEdit},
};
use serde_json::{Value, json};
use std::{fs, path::Path};
use support::{
    fixtures::pi,
    native::{
        claude_main, claude_reply, claude_subagent, claude_user, git_repository, git_worktree,
    },
};

fn list(a: &Archive) -> Value {
    a.project_list().unwrap()
}
fn edit(a: &Archive, action: Value) {
    let request: ProjectEdit =
        serde_json::from_value(json!({"version":list(a)["version"],"action":action})).unwrap();
    a.edit_project(request).unwrap();
}
fn project<'a>(list: &'a Value, path: &Path) -> &'a Value {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["mainFolder"] == path.to_str().unwrap())
        .unwrap()
}

#[test]
fn project_pages_include_worktrees_and_archived_roots_and_remember_manual_moves() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    let native = base.join("pi");
    let archive = base.join("archive");
    let a = Archive::create_unencrypted(&archive).unwrap();
    let policy = Grouping {
        read_git: true,
        home: Some("/home/project-fixture".into()),
        temporary: vec!["/scratch-fixture".into()],
    };
    a.set_project_grouping(policy.clone());
    let repo = base.join("old/api");
    let moved = base.join("new/api");
    let work = base.join("checkouts/feature");
    let sub = repo.join("deleted");
    let remote = "https://example.invalid/team/api.git";
    git_repository(&repo, "main", &[("origin", remote)]);
    git_worktree(&repo.join(".git"), "feature", &work, "feature");
    git_repository(&moved, "main", &[("origin", remote)]);
    for (n, path) in [
        &repo,
        &work,
        &sub,
        &moved,
        Path::new("/home/project-fixture"),
        Path::new("/scratch-fixture/job"),
    ]
    .into_iter()
    .enumerate()
    {
        pi(&native, path.to_str().unwrap(), &format!("project-{n}"), "saved");
    }
    let files = [SourceRoot { harness: Harness::Pi, root: native.clone() }];
    a.backup(&files, &Default::default()).unwrap();
    let first = list(&a);
    assert_eq!(first["items"].as_array().unwrap().len(), 3);
    let old = project(&first, &repo);
    let id = old["id"].as_str().unwrap();
    let other = project(&first, &moved)["id"].as_str().unwrap();
    assert_eq!(old["conversationCount"], 3);
    assert_eq!(old["folderCount"], 3);
    let folders = a.project_detail(id).unwrap();
    assert!(
        folders["folders"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["path"] == work.to_str().unwrap() && v["role"] == "worktree")
    );
    let q = |id: &str| ListQuery { project_id: Some(id.into()), ..Default::default() };
    assert_eq!(a.chat_list(&q(id)).unwrap()["items"].as_array().unwrap().len(), 3);
    let single = a.chat_list(&q(other)).unwrap();
    a.mark_conversation(
        single["items"][0]["id"].as_str().unwrap(),
        &agent_history::archive::ConversationMark { put_away: Some(true), ..Default::default() },
        true,
    )
    .unwrap();
    assert_eq!(a.chat_list(&q(other)).unwrap()["items"], json!([]));
    assert_eq!(project(&list(&a), &moved)["conversationCount"], 1);

    a.backup(&files, &BackupOptions { read_git: false, ..Default::default() }).unwrap();
    assert_eq!(list(&a)["items"].as_array().unwrap().len(), 5);
    a.set_project_git(true);
    assert_eq!(project(&list(&a), &repo)["conversationCount"], 3);
    let suggestions = a.project_suggestions(None).unwrap();
    assert!(suggestions["items"].as_array().unwrap().iter().any(|s| s["kind"] == "sameRemote"));
    edit(&a, json!({"kind":"rename","id":other,"name":"Moved API"}));
    edit(&a, json!({"kind":"createSection","name":"Work","parent":null}));
    let section = list(&a)["arrangement"][0]["id"].clone();
    edit(&a, json!({"kind":"move","id":other,"parent":section,"index":0}));
    edit(&a, json!({"kind":"join","from":other,"into":id}));
    assert_eq!(a.project_detail(other).unwrap()["status"], "addedInto");
    assert_eq!(project(&list(&a), &repo)["conversationCount"], 4);
    edit(&a, json!({"kind":"undo"}));
    assert_eq!(project(&list(&a), &moved)["displayName"], "Moved API");
    edit(&a, json!({"kind":"detach","id":id,"path":work}));
    fs::remove_dir_all(&repo).unwrap();
    a.backup(&files, &BackupOptions::default()).unwrap();
    assert_eq!(project(&list(&a), &repo)["conversationCount"], 2);
    assert_eq!(project(&list(&a), &repo)["gone"], true);
    assert_eq!(project(&list(&a), &work)["conversationCount"], 1);
    drop(a);
    let reopened = Archive::open(&archive, b"").unwrap();
    reopened.set_project_grouping(policy);
    assert_eq!(project(&list(&reopened), &moved)["id"], other);
    assert!(
        list(&reopened)["arrangement"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["id"] == other && n["parent"] == section)
    );
}

#[test]
fn a_missing_cwd_is_unknown_and_a_move_requires_the_saved_decision() {
    use support::fixtures::{jsonl, pi_message};
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("archive");
    let model = vault_layout::path(&root, vault_layout::ArtifactId::AgentHistoryReadModel);
    drop(Archive::create_unencrypted(&root).unwrap());
    let empty = fs::read(&model).unwrap();
    let a = Archive::open(&root, b"").unwrap();
    let policy = Grouping { read_git: true, temporary: vec![], home: None };
    a.set_project_grouping(policy.clone());
    let native = t.path().join("pi");
    let old = t.path().join("original/repo");
    let new = t.path().join("replacement/repo");
    git_repository(&old, "main", &[("origin", "https://example.invalid/renamed")]);
    pi(&native, old.to_str().unwrap(), "before", "before move");
    fs::write(
        native.join("sessions/unknown.jsonl"),
        jsonl(&[
            json!({"type":"session","version":3,"id":"no-cwd","timestamp":"2026-11-10T12:00:00Z"}),
            pi_message("without", None, "user", "no folder here"),
        ]),
    )
    .unwrap();
    let sources = [SourceRoot { harness: Harness::Pi, root: native.clone() }];
    a.backup(&sources, &Default::default()).unwrap();
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::rename(&old, &new).unwrap();
    pi(&native, new.to_str().unwrap(), "after", "after move");
    a.backup(&sources, &Default::default()).unwrap();
    let records = a.chat_list(&ListQuery::default()).unwrap();
    assert_eq!(records["items"].as_array().unwrap().len(), 3);
    assert_eq!(
        records["items"].as_array().unwrap().iter().filter(|c| c["project"].is_null()).count(),
        1
    );
    assert_eq!(list(&a)["items"].as_array().unwrap().len(), 2);
    let suggestions = a.project_suggestions(None).unwrap();
    let moved =
        suggestions["items"].as_array().unwrap().iter().find(|v| v["kind"] == "moved").unwrap();
    assert_eq!(moved["evidence"]["oldPath"], old.to_str().unwrap());
    assert_eq!(moved["evidence"]["newPath"], new.to_str().unwrap());
    let id = moved["id"].as_str().unwrap();
    for decision in ["later", "declined", "pending", "added"] {
        edit(&a, json!({"kind":"answer","id":id,"decision":decision}));
        assert_eq!(a.project_suggestions(None).unwrap()["items"][0]["decision"], decision);
    }
    let into = moved["intoProjectId"].as_str().unwrap();
    let q = ListQuery { project_id: Some(into.into()), ..Default::default() };
    assert_eq!(a.chat_list(&q).unwrap()["items"].as_array().unwrap().len(), 2);
    let query =
        serde_json::from_value(json!({"query":"move","filters":{"projectId":into}})).unwrap();
    let mut searched = None;
    a.stream_conversations(None, &query, &Default::default(), |ty, v| {
        if ty == "finished" {
            searched = v["conversations"].as_i64();
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(searched, Some(2));
    edit(&a, json!({"kind":"rename","id":into,"name":"Team repo"}));
    let saved = a.project_detail(into).unwrap();
    assert_eq!(saved["project"]["conversationCount"], 2);
    drop(a);
    fs::remove_dir_all(native).unwrap();
    fs::remove_dir_all(vault_layout::path(&root, vault_layout::ArtifactId::AgentHistorySearch))
        .unwrap();
    fs::write(model, empty).unwrap();
    let rebuilt = Archive::open(&root, b"").unwrap();
    rebuilt.set_project_grouping(policy);
    rebuilt.rebuild_from_retained_evidence(&Default::default()).unwrap();
    let restored = rebuilt.project_detail(into).unwrap();
    assert_eq!(restored["project"], saved["project"]);
    assert_eq!(restored["folders"], saved["folders"]);
    assert_eq!(rebuilt.project_suggestions(None).unwrap()["items"][0]["decision"], "added");
    edit(&rebuilt, json!({"kind":"answer","id":id,"decision":"pending"}));
    assert_eq!(list(&rebuilt)["items"].as_array().unwrap().len(), 2);
}

#[test]
fn project_picker_tree_pins_and_undo_share_one_visible_pool() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let native = t.path().join("pi");
    for (id, path) in [
        ("api", "/home/project-fixture/code/work/api"),
        ("web", "/home/project-fixture/code/work/web"),
        ("book", "/home/project-fixture/books"),
    ] {
        pi(&native, path, id, "fixture");
    }
    a.set_project_grouping(Grouping {
        home: Some("/home/project-fixture".into()),
        temporary: vec![],
        read_git: true,
    });
    a.backup(&[SourceRoot { harness: Harness::Pi, root: native }], &Default::default()).unwrap();
    let page = |q: Value| a.project_page(&serde_json::from_value(q).unwrap()).unwrap();
    let one = page(json!({"limit":1}));
    let rest = page(json!({"cursor":one["nextCursor"],"limit":3}));
    assert_eq!(rest["items"].as_array().unwrap().len(), 2);
    assert_ne!(one["items"][0]["id"], rest["items"][0]["id"]);
    let selected = page(json!({"text":"/code/","folderTree":true}));
    assert_eq!(selected["matching"], 2);
    assert_eq!(selected["tree"]["groups"].as_array().unwrap().len(), 1);
    assert_eq!(selected["tree"]["groups"][0]["conversationCount"], 2);
    let api = &selected["items"][0]["id"];
    assert_eq!(page(json!({"candidateFor":api}))["matching"], 2);
    edit(&a, json!({"kind":"createSection","name":"Work"}));
    let work = list(&a)["arrangement"][0]["id"].clone();
    edit(&a, json!({"kind":"createSection","parent":work,"name":"Apps"}));
    let apps = list(&a)["arrangement"][1]["id"].clone();
    edit(&a, json!({"kind":"move","id":api,"parent":apps,"index":0}));
    assert_eq!(page(json!({"otherOnly":true}))["matching"], 2);
    let tree = list(&a);
    assert_eq!(tree["arrangement"][0]["conversationCount"], 1);
    let cycle = serde_json::from_value(
        json!({"version":tree["version"],"action":{"kind":"move","id":work,"parent":apps}}),
    )
    .unwrap();
    assert_eq!(a.edit_project(cycle).unwrap_err().code(), "invalid_input");
    edit(&a, json!({"kind":"collapse","collapsed":true}));
    assert_eq!(list(&a)["arrangement"][1]["collapsed"], true);
    edit(&a, json!({"kind":"deleteSection","id":work}));
    assert_eq!(list(&a)["arrangement"], json!([]));
    edit(&a, json!({"kind":"undo"}));
    assert_eq!(list(&a)["arrangement"][1]["collapsed"], true);
    let stale =
        serde_json::from_value(json!({"version":0,"action":{"kind":"unpin","id":api}})).unwrap();
    assert_eq!(a.edit_project(stale).unwrap_err().code(), "stale_cursor");
}

#[test]
fn child_usage_leaves_the_project_when_the_child_is_trashed() {
    use agent_history::archive::ConversationMark;
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    let root = t.path().join("claude");
    let cwd = "/work/project-tokens";
    claude_main(
        &root,
        "parent",
        &[
            claude_user("parent", "u", None, cwd, "question"),
            claude_reply("parent", "a", "u", cwd, "answer"),
        ],
    );
    claude_subagent(&root, "parent", "agent-kid.jsonl", "kid", cwd, "subtask");
    a.backup(&[SourceRoot { harness: Harness::ClaudeCode, root }], &Default::default()).unwrap();
    let all = a.chat_list(&ListQuery::default()).unwrap();
    let parent = a.reader_header(all["items"][0]["id"].as_str().unwrap()).unwrap();
    let child = a.reader_children(&parent, None, 4).unwrap().pop().unwrap();
    let count = || {
        let p = list(&a);
        let row = &p["items"][0];
        (
            row["subagentCount"].as_i64().unwrap(),
            row["tokensTotal"].as_i64().unwrap(),
            row["tokensKnownConversations"].as_i64().unwrap(),
        )
    };
    assert_eq!(count(), (1, 18, 1));
    a.mark_conversation(
        &child,
        &ConversationMark { trash: Some(true), ..Default::default() },
        true,
    )
    .unwrap();
    assert_eq!(count(), (0, 12, 1));
    a.mark_conversation(
        &child,
        &ConversationMark { trash: Some(false), ..Default::default() },
        true,
    )
    .unwrap();
    assert_eq!(count(), (1, 18, 1));
    a.mark_conversation(
        &parent.conversation_id,
        &ConversationMark { trash: Some(true), ..Default::default() },
        true,
    )
    .unwrap();
    assert_eq!(list(&a)["total"], 0);
}

#[test]
fn renaming_a_repository_folder_preserves_both_locations_for_review() {
    let t = tempfile::tempdir().unwrap();
    let a = Archive::create_unencrypted(&t.path().join("archive")).unwrap();
    a.set_project_grouping(Grouping { read_git: true, home: None, temporary: vec![] });
    let native = t.path().join("pi");
    let old = t.path().join("old-name");
    let renamed = t.path().join("new-name");
    git_repository(&old, "main", &[("origin", "https://example.invalid/team/project")]);
    let roots = [SourceRoot { harness: Harness::Pi, root: native.clone() }];
    pi(&native, old.to_str().unwrap(), "one", "before rename");
    a.backup(&roots, &Default::default()).unwrap();
    let id = project(&list(&a), &old)["id"].clone();
    fs::rename(&old, &renamed).unwrap();
    pi(&native, renamed.to_str().unwrap(), "two", "after rename");
    a.backup(&roots, &Default::default()).unwrap();
    let projects = list(&a);
    assert_eq!(projects["total"], 2);
    assert_eq!(project(&projects, &old)["id"], id);
    assert_eq!(project(&projects, &old)["gone"], true);
    assert_eq!(project(&projects, &renamed)["gone"], false);
    let review = a.project_suggestions(None).unwrap();
    assert_eq!(review["items"][0]["kind"], "sameRemote");
    assert_eq!(review["items"][0]["fromProjectId"], id);
    assert_eq!(review["items"][0]["evidence"]["remote"], "https://example.invalid/team/project");
}
