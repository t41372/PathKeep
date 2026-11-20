//! Executable command examples for the conversation screens. The fixture
//! substitutes installation paths, issued IDs and capture clock readings.
use super::*;
use crate::command_error::CommandError;
async fn sample(
    state: &DevIpcBridgeState,
    examples: &std::cell::RefCell<Value>,
    name: &str,
    request: Value,
) -> Result<Value, CommandError> {
    let wire = json!({"request":request});
    let result = dispatch_command(state, name, wire.clone()).await;
    if let Ok(response) = &result {
        examples.borrow_mut()[name] = json!({"request":wire,"response":response});
    }
    result
}
fn normalize(v: &mut Value, key: &str, replacements: &[(&str, &str)]) {
    match v {
        Value::Object(fields) => {
            for (k, v) in fields {
                normalize(v, k, replacements);
            }
        }
        Value::Array(items) => {
            for v in items {
                normalize(v, key, replacements);
            }
        }
        Value::String(s) => {
            for (from, to) in replacements {
                *s = s.replace(from, to);
            }
        }
        Value::Number(_)
            if matches!(key, "lastSeenAt" | "lastSavedAt" | "savedAt" | "observedAt") =>
        {
            *v = json!(1794355500000_i64)
        }
        _ => {}
    }
}
#[test]
fn conversation_screen_wire_opens_a_saved_pi_turn_and_reads_its_image() {
    let (_guard, platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let native = platform.path().join("pi");
    let dir = native.join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    let records = [
        json!({"type":"session","version":3,"id":"wire-fixture","cwd":"/src/project","timestamp":"2026-11-10T12:00:00Z"}),
        json!({"type":"message","id":"prompt","parentId":null,"timestamp":"2026-11-10T12:01:00Z","message":{"role":"user","content":[{"type":"text","text":"Look at the image"},{"type":"image","mimeType":"image/png","data":"iVBORw0KGgo="}]}}),
        json!({"type":"message","id":"answer","parentId":"prompt","timestamp":"2026-11-10T12:02:00Z","message":{"role":"assistant","model":"synthetic","content":[{"type":"text","text":"Answer"}],"usage":{"input":4,"output":2,"cacheRead":3,"cacheWrite":1,"cost":{"total":0}}}}),
    ];
    std::fs::write(
        dir.join("session.jsonl"),
        records.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let archive = state.session.vault().unwrap().agent_history().unwrap();
        let folders: Vec<_> =
            serde_json::from_value(json!([{"harness":"pi","root":native}])).unwrap();
        archive.backup(&folders, &Default::default()).unwrap();
        let examples = std::cell::RefCell::new(json!({}));
        let call = |name: &'static str, request: Value| sample(&state, &examples, name, request);
        let page =
            call("list_agent_conversations", json!({"sort":"tokens","limit":1})).await.unwrap();
        let id = &page["items"][0]["id"];
        assert_eq!(page["items"][0]["tokensTotal"], 10);
        assert_eq!(
            call("get_agent_conversation_counts", json!({})).await.unwrap()["spaceRoots"],
            1
        );
        for (command, request, expected, frame_type) in [
            (
                "search_agent_conversations",
                json!({"query":"Answer","roles":["agent"]}),
                1,
                "groups",
            ),
            ("find_in_agent_conversation", json!({"id":id,"query":"/im.ge/"}), 1, "matches"),
        ] {
            let receipt =
                dispatch_command(&state, command, json!({"request":request})).await.unwrap();
            let stream = receipt["streamId"].as_str().unwrap();
            let mut frames = crate::dev_ipc_bridge::preview::subscribe(&state, stream).unwrap();
            let mut seen = false;
            while let Some(frame) = frames.next_frame().await {
                assert_eq!(frame["streamId"], stream);
                if frame["type"] == frame_type {
                    seen = true;
                }
                if frame["type"] == "finished" {
                    assert_eq!(frame["payload"]["hits"], expected);
                    break;
                }
                assert_ne!(frame["type"], "error", "{frame}");
            }
            assert!(seen);
        }
        let bad = dispatch_command(
            &state,
            "search_agent_conversations",
            json!({"request":{"query":"/[/"}}),
        )
        .await
        .unwrap_err();
        assert_eq!(bad.code.as_deref(), Some("invalid_regex"));
        let receipt = dispatch_command(
            &state,
            "find_in_agent_conversation",
            json!({"request":{"id":id,"query":"/.*/"}}),
        )
        .await
        .unwrap();
        let stream = receipt["streamId"].as_str().unwrap();
        let mut frames = crate::dev_ipc_bridge::preview::subscribe(&state, stream).unwrap();
        dispatch_command(
            &state,
            "cancel_agent_read",
            json!({"request":{"requestIdOrStreamId":stream}}),
        )
        .await
        .unwrap();
        assert_eq!(frames.next_frame().await.unwrap()["type"], "cancelled");
        assert!(frames.next_frame().await.is_none());
        let h = call("get_agent_conversation", json!({"id":id})).await.unwrap();
        let source = &h["currentSource"];
        let publication = &h["publication"];
        let info =
            call("get_agent_conversation_info", json!({"id":id,"showCost":false})).await.unwrap();
        assert!(info["own"].get("recordedUsd").is_none());
        assert_eq!(info["own"]["agentMs"], 60000);
        let window = call(
            "read_agent_conversation_window",
            json!({"id":id,"publication":publication,"anchor":{"kind":"end"},"limit":2}),
        )
        .await
        .unwrap();
        let chip = &window["rows"][0]["attachments"][0];
        assert_eq!(chip["availability"], "inline");
        let image=call("read_agent_attachment",json!({"id":id,"publication":publication,"sourceId":source,"ordinal":chip["ordinal"],"key":chip["key"],"offset":1,"limit":4})).await.unwrap();
        assert_eq!(image, json!({"bytes":[80,78,71,13],"totalBytes":8,"next":5}));
        let raw=call("read_agent_record",json!({"id":id,"publication":publication,"sourceId":source,"ordinal":0,"offset":0,"limit":4096})).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(
                &serde_json::from_value::<Vec<u8>>(raw["bytes"].clone()).unwrap()
            )
            .unwrap()["id"],
            "wire-fixture"
        );
        assert_eq!(
            call("list_agent_conversation_branches", json!({"id":id})).await.unwrap()["leaves"]["latestRecorded"],
            2
        );
        let changed = call("edit_agent_conversation", json!({"id":id,"action":"star","on":true}))
            .await
            .unwrap();
        assert_eq!(changed["summary"]["starred"], true);
        let error =
            call("read_agent_conversation_window", json!({"id":id,"limit":201})).await.unwrap_err();
        assert_eq!(error.code.as_deref(), Some("invalid_input"));
        let project_id = &page["items"][0]["project"]["id"];
        let projects =
            dispatch_command(&state, "list_agent_projects", json!({"request":{}})).await.unwrap();
        assert_eq!(projects["items"][0]["id"], *project_id);
        let detail =
            dispatch_command(&state, "get_agent_project", json!({"request":{"id":project_id}}))
                .await
                .unwrap();
        assert_eq!(detail["folders"][0]["path"], "/src/project");
        let review = dispatch_command(
            &state,
            "list_agent_project_suggestions",
            json!({"request":{"intoProjectId":project_id}}),
        )
        .await
        .unwrap();
        assert_eq!(review["items"], json!([]));
        dispatch_command(&state,"edit_agent_project",json!({"request":{"version":projects["version"],"action":{"kind":"rename","id":project_id,"name":"Wire project"}}})).await.unwrap();
        let renamed =
            dispatch_command(&state, "get_agent_conversation", json!({"request":{"id":id}}))
                .await
                .unwrap();
        assert_eq!(renamed["summary"]["project"]["label"], "Wire project");
        let settings =
            dispatch_command(&state, "get_agent_insights_preferences", json!({"request":{}}))
                .await
                .unwrap();
        assert_eq!(settings["range"], "all");
        assert_eq!(settings["showCost"], true);
        let prefs = dispatch_command(
            &state,
            "set_agent_insights_preferences",
            json!({"request":{"showCost":false,"range":"30d"}}),
        )
        .await
        .unwrap();
        assert_eq!(prefs["range"], "30d");
        let info = dispatch_command(
            &state,
            "get_agent_conversation_info",
            json!({"request":{"id":id,"showCost":true}}),
        )
        .await
        .unwrap();
        assert!(info["own"].get("recordedUsd").is_none());
        let receipt = dispatch_command(
            &state,
            "get_agent_insights",
            json!({"request":{"range":"all","year":2026}}),
        )
        .await
        .unwrap();
        let mut frames = crate::dev_ipc_bridge::preview::subscribe(
            &state,
            receipt["streamId"].as_str().unwrap(),
        )
        .unwrap();
        let shown_zone =
            dispatch_command(&state, "get_insights_time_zone", json!({})).await.unwrap()["shown"]
                .clone();
        let mut sections = std::collections::BTreeSet::new();
        while let Some(frame) = frames.next_frame().await {
            let kind = frame["type"].as_str().unwrap();
            sections.insert(kind.to_owned());
            if matches!(kind, "started" | "calendar") {
                assert_eq!(frame["payload"]["timeZone"], shown_zone);
            }
            if kind == "overview" {
                assert_eq!(frame["payload"]["tokens"]["output"], 2);
                assert!(frame["payload"].get("cost").is_none());
            }
            assert_ne!(kind, "error", "{frame}");
        }
        for section in ["started", "overview", "series", "calendar", "breakdowns", "finished"] {
            assert!(sections.contains(section));
        }
        let mut examples = examples.into_inner();
        normalize(
            &mut examples,
            "",
            &[
                (page["items"][0]["project"]["id"].as_str().unwrap(), "project-1"),
                (id.as_str().unwrap(), "chat-1"),
                (source.as_str().unwrap(), "source-1"),
                (publication.as_str().unwrap(), "publication-1"),
                (native.to_str().unwrap(), "/fixtures/pi"),
            ],
        );
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/agent-chats.json");
        if std::env::var_os("PATHKEEP_WRITE_CHAT_FIXTURE").is_some() {
            std::fs::write(&fixture, serde_json::to_string_pretty(&examples).unwrap() + "\n")
                .unwrap();
        }
        let expected: Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
        assert_eq!(examples, expected);
    });
}
