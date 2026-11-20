//! Project and child cards from captured Claude transcripts and metadata.
use super::*;

#[test]
fn tools_home_beneath_system_temp_keeps_projects_distinct() {
    const RUN_FIXTURE: &str = "PATHKEEP_TEST_GROUPING_HOME";
    if let Some(home) = std::env::var_os(RUN_FIXTURE) {
        let home = std::path::PathBuf::from(home);
        let root = home.join(".claude");
        let archive = Archive::create_unencrypted(&home.join("archive")).unwrap();
        for (id, cwd) in
            [("one", home.join("repo")), ("two", home.join("tmp/scratch")), ("three", home.clone())]
        {
            native::claude_main(
                &root,
                id,
                &[native::claude_user(id, "u", None, cwd.to_str().unwrap(), id)],
            );
        }
        capture(&archive, agent_history::Harness::ClaudeCode, &root);
        let projects = archive.project_list().unwrap();
        assert_eq!(projects["homeFolder"], json!(home));
        let rows = projects["items"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{projects}");
        assert!(rows.iter().any(|r| r["kind"] == "scratch" && r["conversationCount"] == 2));
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "capture_fixtures::tools_home_beneath_system_temp_keeps_projects_distinct",
            "--nocapture",
        ])
        .env("CHB_AGENT_TOOLS_HOME", temp.path())
        .env(RUN_FIXTURE, temp.path())
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn saved_subagent_metadata_links_the_launch_and_names_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&dir.path().join("archive")).unwrap();
    let root = dir.path().join("claude");
    let mut task = native::claude_tool_call(
        "parent",
        "launch",
        "u",
        "/repo",
        json!({"description":"Inspect migrations","prompt":"Review each migration"}),
    );
    task["message"]["content"][0]["name"] = json!("Task");
    native::claude_main(
        &root,
        "parent",
        &[native::claude_user("parent", "u", None, "/repo", "Start review"), task],
    );
    let child = native::claude_subagent(
        &root,
        "parent",
        "agent-kid.jsonl",
        "kid",
        "/repo",
        "Review each migration",
    );
    write(&child.with_extension("meta.json"), br#"{"toolUseId":"call-launch"}"#);
    native::claude_subagent(
        &root,
        "parent",
        "agent-other.jsonl",
        "other",
        "/repo",
        "Check the schema",
    );
    capture(&archive, agent_history::Harness::ClaudeCode, &root);
    std::fs::remove_dir_all(root).unwrap();
    let id = all(&archive)[0]["id"].clone();
    let branches = archive
        .chat_branches(&serde_json::from_value(json!({"id":id})).unwrap(), &Default::default())
        .unwrap();
    let children = branches["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    let launched = children
        .iter()
        .find(|c| c["launch"]["callId"] == "call-launch")
        .expect("saved sidecar identifies launch");
    assert_eq!(launched["launch"]["ordinal"], 1);
    assert_eq!(launched["title"], "Inspect migrations");
    assert_eq!(launched["titleSource"], "tool");
    assert_eq!(
        archive.chat_header(launched["id"].as_str().unwrap()).unwrap()["summary"]["title"],
        "Inspect migrations"
    );
    assert!(children.iter().any(|c| c["launch"].is_null() && c["title"] == "Check the schema"));
}

#[test]
fn branch_cards_use_the_matching_launch_type_when_the_child_has_none() {
    use std::io::Write;

    let dir = tempfile::tempdir().unwrap();
    let archive = Archive::create_unencrypted(&dir.path().join("archive")).unwrap();
    let root = dir.path().join("claude");
    let mut calls = native::claude_tool_call("parent", "dispatch", "u", "/repo", json!({}));
    calls["message"]["content"] = json!([
        {"type":"tool_use","id":"task","name":"Task","input":{"description":"Survey code","subagent_type":"general-purpose"}},
        {"type":"tool_use","id":"agent","name":"Agent","input":{"subagent_type":"Explore"}},
        {"type":"tool_use","id":"typed","name":"Agent","input":{"subagent_type":"requested-type"}}
    ]);
    native::claude_main(
        &root,
        "parent",
        &[native::claude_user("parent", "u", None, "/repo", "Review code"), calls],
    );
    for id in ["task", "agent", "typed"] {
        let path = native::claude_subagent(
            &root,
            "parent",
            &format!("agent-{id}.jsonl"),
            id,
            "/repo",
            "Investigate",
        );
        write(
            &path.with_extension("meta.json"),
            &serde_json::to_vec(&json!({"toolUseId":id})).unwrap(),
        );
        if id == "task" || id == "typed" {
            let mut title = native::claude_title("parent", "Native title");
            title["agentId"] = json!(id);
            if id == "typed" {
                title["agentType"] = json!("recorded-type");
            }
            writeln!(std::fs::OpenOptions::new().append(true).open(path).unwrap(), "{title}")
                .unwrap();
        }
    }
    capture(&archive, agent_history::Harness::ClaudeCode, &root);
    std::fs::remove_dir_all(root).unwrap();
    let branches = archive
        .chat_branches(
            &serde_json::from_value(json!({"id":all(&archive)[0]["id"]})).unwrap(),
            &Default::default(),
        )
        .unwrap();
    let children = branches["children"].as_array().unwrap();
    assert_eq!(children.len(), 3);
    for (call, title, kind) in [
        ("typed", "Native title", "recorded-type"),
        ("task", "Native title", "general-purpose"),
        ("agent", "Investigate", "Explore"),
    ] {
        let card = children.iter().find(|card| card["launch"]["callId"] == call).unwrap();
        assert_eq!(card["title"], title, "{call}");
        assert_eq!(card["agentType"], kind, "{call}");
        assert_eq!(archive.chat_info(card["id"].as_str().unwrap()).unwrap()["agentType"], kind);
    }
}
