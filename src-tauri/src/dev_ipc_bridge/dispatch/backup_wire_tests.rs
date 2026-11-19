//! Pin the actual dev-bridge JSON. Canonical examples replace machine paths,
//! generated identifiers, clock readings and physical database size only.
use super::*;
use crate::dev_ipc_bridge::preview;
use std::time::{Duration, Instant};

async fn invoke(
    state: &DevIpcBridgeState,
    examples: &mut Value,
    name: &str,
    request: Value,
) -> Value {
    let wire = json!({"request":request});
    let response = dispatch_command(state, name, wire.clone()).await.unwrap();
    examples[name] = json!({"request":wire,"response":response});
    response
}

fn canonical(
    value: &mut Value,
    key: &str,
    path: &str,
    pointer: &str,
    names: &mut std::collections::HashMap<String, String>,
) {
    match value {
        Value::Object(fields) => {
            for (k, v) in fields {
                canonical(v, k, path, &format!("{pointer}/{k}"), names);
            }
        }
        Value::Array(items) => {
            for (index, v) in items.iter_mut().enumerate() {
                canonical(v, key, path, &format!("{pointer}/{index}"), names);
            }
        }
        Value::Number(_) if matches!(key, "startedAt" | "finishedAt" | "endedAt") => {
            *value = json!(1794355200000i64)
        }
        Value::Number(_) if key == "durationMs" => *value = json!(0),
        Value::String(s) if key == "archiveBytes" => *s = "409600".into(),
        Value::String(s) if s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()) => {
            *s = names
                .entry(s.clone())
                .or_insert_with(|| vault_core::utils::sha256_hex(pointer.as_bytes()))
                .clone();
        }
        Value::String(s)
            if s.starts_with("agent-history:") || (s.len() == 36 && s.as_bytes()[8] == b'-') =>
        {
            let (prefix, id) = s
                .strip_prefix("agent-history:")
                .map_or(("", s.as_str()), |id| ("agent-history:", id));
            let name = names.entry(id.to_owned()).or_insert_with(|| {
                let h = vault_core::utils::sha256_hex(pointer.as_bytes());
                format!("{}-{}-4{}-8{}-{}", &h[..8], &h[8..12], &h[13..16], &h[17..20], &h[20..32])
            });
            *s = format!("{prefix}{name}");
        }
        Value::String(s) => *s = s.replace(path, "/fixtures/pi"),
        _ => {}
    }
}

#[test]
fn conversation_backup_json_examples_match_dispatch() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    let home = tempdir().unwrap();
    let directory = home.path().canonicalize().unwrap();
    let file = directory.join("sessions/project/session.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        b"{\"type\":\"session\",\"version\":1000,\"id\":\"one\",\"cwd\":\"/synthetic\"}\n",
    )
    .unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let observe = state.events.observe();
        let settings =
            dispatch_command(&state, "get_agent_settings", json!({"request":{}})).await.unwrap();
        let stream = dispatch_command(
            &state,
            "preview_agent_sources",
            json!({"request":{"purpose":"enable","pickedFolder":{"tool":"pi","path":directory}}}),
        )
        .await
        .unwrap();
        let mut frames = preview::subscribe(&state, stream["streamId"].as_str().unwrap()).unwrap();
        let inventory = loop {
            let frame = frames.next_frame().await.unwrap();
            if frame["type"] == "finished" {
                break frame["payload"].clone();
            }
        };
        let mut examples = json!({});
        let receipt=invoke(&state,&mut examples,"enable_agent_history",json!({"requestId":"30000000-0000-4000-8000-000000000003","previewId":inventory["previewId"],
            "selectedRootIds":[inventory["rows"][0]["rootId"]],"expectedPolicyToken":settings["policyToken"],"readGit":false})).await;
        let mut events = json!({});
        let start = Instant::now();
        loop {
            for (name, payload) in observe() {
                events[name] = payload;
            }
            if !events["pathkeep://kind-backup-finished"].is_null() {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(events["pathkeep://kind-backup-finished"]["result"], "partial");
        examples["events"] = events;
        invoke(&state, &mut examples, "list_data_kinds", json!({})).await;
        invoke(&state, &mut examples, "get_agent_settings", json!({})).await;
        invoke(&state, &mut examples, "get_agent_overview", json!({"purpose":"backup"})).await;
        invoke(&state, &mut examples, "list_agent_sources", json!({"limit":10})).await;
        invoke(
            &state,
            &mut examples,
            "list_archive_runs",
            json!({"space":"agent-history","includeArchiveWide":true,"limit":10}),
        )
        .await;
        invoke(
            &state,
            &mut examples,
            "get_archive_run",
            json!({"runRef":receipt["firstSave"]["runRef"]}),
        )
        .await;
        invoke(&state, &mut examples, "get_task_summary", json!({})).await;
        let tasks = invoke(
            &state,
            &mut examples,
            "list_background_tasks",
            json!({"group":"attention","limit":10}),
        )
        .await;
        let task = &tasks["items"][0];
        assert_eq!(task["attention"], true);
        invoke(&state,&mut examples,"control_background_task",json!({"taskId":task["taskId"],"action":"dismiss","expectedToken":task["snapshotToken"],"requestId":"40000000-0000-4000-8000-000000000004"})).await;
        let choices=invoke(&state,&mut examples,"update_agent_sources",json!({"selectedRootIds":[inventory["rows"][0]["rootId"]],"readGit":false,"toolChanges":{"pi":true},"expectedToken":receipt["policyToken"]})).await;
        invoke(&state,&mut examples,"set_data_kind_state",json!({"kind":"agent-history","state":"paused","expectedPolicyToken":choices["policyToken"]})).await;
        let manual=invoke(&state,&mut examples,"start_backup",json!({"scope":{"type":"tool","kind":"agent-history","tool":"pi"},"requestId":"50000000-0000-4000-8000-000000000005"})).await;
        assert_eq!(manual["kinds"][0]["decision"], "started");
        finished_task(&state, &manual["kinds"][0]["taskId"]).await;
        examples["variants"] = controls(&state, &file).await;
        state.session.close_vault().unwrap();
        canonical(&mut examples, "", directory.to_str().unwrap(), "", &mut Default::default());
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/agent-backup.json");
        if std::env::var_os("PATHKEEP_WRITE_BACKUP_FIXTURE").is_some() {
            std::fs::write(&fixture, serde_json::to_string_pretty(&examples).unwrap() + "\n")
                .unwrap();
        }
        let expected: Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
        assert_eq!(examples, expected);
    })
}

async fn finished_task(state: &DevIpcBridgeState, id: &Value) -> Value {
    let start = Instant::now();
    loop {
        let list = dispatch_command(
            state,
            "list_background_tasks",
            json!({"request":{"group":"today","limit":20}}),
        )
        .await
        .unwrap();
        if let Some(row) = list["items"].as_array().unwrap().iter().find(|r| r["taskId"] == *id) {
            let live =
                dispatch_command(state, "get_task_summary", json!({"request":{}})).await.unwrap();
            if live["runningCount"] == 0 {
                return row.clone();
            }
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn controls(state: &DevIpcBridgeState, file: &std::path::Path) -> Value {
    use std::sync::{Arc, Mutex, mpsc};
    use vault_worker::data_kinds::backups;
    let mut examples = json!({});
    let (arrived, at_first_file) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    let events: backups::Events = Arc::new(move |name, p| {
        if name == "pathkeep://kind-backup-progress" && p["phase"] == "discovery" {
            arrived.send(()).unwrap();
            released.lock().unwrap().recv_timeout(Duration::from_secs(10)).unwrap();
        }
    });
    let kind = json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":"60000000-0000-4000-8000-000000000006"});
    let admitted =
        backups::start(&state.session, serde_json::from_value(kind.clone()).unwrap(), events)
            .unwrap();
    at_first_file.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(invoke(state, &mut examples["kind"], "start_backup", kind).await, admitted);
    let busy=invoke(state,&mut examples["busy"],"start_backup",json!({"scope":{"type":"tool","kind":"agent-history","tool":"pi"},"requestId":"70000000-0000-4000-8000-000000000007"})).await;
    assert_eq!(busy["kinds"][0]["decision"], "already-running");
    let running = invoke(state, &mut examples["running"], "get_task_summary", json!({})).await;
    let row = &running["running"][0];
    let cancel = json!({"taskId":row["taskId"],"action":"cancel","expectedToken":row["snapshotToken"],"requestId":"80000000-0000-4000-8000-000000000008"});
    let stopped = invoke(state, &mut examples["cancel"], "control_background_task", cancel).await;
    assert_eq!(stopped["task"]["state"], "cancelling");
    release.send(()).unwrap();
    let old = finished_task(state, &row["taskId"]).await;
    assert_eq!(old["state"], "interrupted");
    let settings =
        dispatch_command(state, "get_agent_settings", json!({"request":{}})).await.unwrap();
    let changed = dispatch_command(
        state,
        "update_agent_sources",
        json!({"request":{"expectedToken":settings["policyToken"],"readGit":true}}),
    )
    .await
    .unwrap();
    let mut retry = json!({"taskId":old["taskId"],"action":"retry","expectedToken":old["snapshotToken"],"requestId":"90000000-0000-4000-8000-000000000009"});
    assert_eq!(
        invoke(state, &mut examples["review"], "control_background_task", retry.clone()).await["previewRequired"],
        true
    );
    dispatch_command(
        state,
        "update_agent_sources",
        json!({"request":{"expectedToken":changed["policyToken"],"readGit":false}}),
    )
    .await
    .unwrap();
    std::fs::write(
        file,
        b"{\"type\":\"session\",\"version\":3,\"id\":\"one\",\"cwd\":\"/synthetic\"}\n",
    )
    .unwrap();
    retry["requestId"] = json!("a0000000-0000-4000-8000-00000000000a");
    let accepted =
        dispatch_command(state, "control_background_task", json!({"request":retry})).await.unwrap();
    assert_eq!(finished_task(state, &accepted["replacementTaskId"]).await["state"], "succeeded");
    let replay = invoke(state, &mut examples["retry"], "control_background_task", retry).await;
    assert_eq!(replay["replacementTaskId"], accepted["replacementTaskId"]);
    let denied = json!({"request":{"scope":{"type":"tool","kind":"agent-history","tool":"codex"},"requestId":"b0000000-0000-4000-8000-00000000000b"}});
    let error = dispatch_command(state, "start_backup", denied.clone()).await.unwrap_err();
    assert_eq!(error.code.as_deref(), Some("scope-denied"));
    examples["scopeDenied"] = json!({"command":"start_backup","request":denied,"error":error});
    examples
}
