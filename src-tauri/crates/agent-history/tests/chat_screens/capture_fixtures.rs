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
