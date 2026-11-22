//! Wire contract against the actual owner and the dev dispatch arms. Failures include
//! whole-config writes bypassing policy, replay after lock, and preview writing evidence.
use super::*;
use crate::dev_ipc_bridge::preview;
/// Each simulated installation has its own identity and empty native tools home.
fn isolated_env() -> (crate::test_support::EnvGuard, tempfile::TempDir) {
    let guard = lock_env();
    let platform = tempdir().unwrap();
    unsafe {
        std::env::set_var("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", platform.path());
        std::env::set_var("CHB_AGENT_TOOLS_HOME", platform.path().join("tools"));
    }
    (guard, platform)
}
fn wire() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/data-kinds-preview.json")).unwrap()
}

#[test]
fn app_surface_policy_contract() {
    // Failure modes: omitted request wrapper, a forged enable or pause, stale
    // policy overwriting another window, and lock leaking the management snapshot.
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path()) };
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let chrome = root.path().join("chrome");
    std::fs::create_dir(&chrome).unwrap();
    unsafe {
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, chrome);
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
    }
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let events = state.events.observe();
        let read = json!({"request": {}});
        let snapshot = dispatch_command(&state, "list_data_kinds", read.clone()).await.unwrap();
        assert_eq!(snapshot["kinds"][0]["state"], "on");
        assert_eq!(snapshot["kinds"][1]["state"], "off");
        let contract = wire();
        assert_eq!(snapshot, contract["list_data_kinds"]["response"]);
        let token = snapshot["policyToken"].as_str().unwrap();
        for (kind, next, reason) in [
            ("browsing", "paused", "last-active-kind"),
            ("browsing", "off", "last-active-kind"),
            ("agent-history", "on", "enable-preview-required"),
        ] {
            let error = dispatch_command(
                &state,
                "set_data_kind_state",
                json!({"request":{
                    "kind":kind,"state":next,"expectedPolicyToken":token
                }}),
            )
            .await
            .unwrap_err();
            assert_eq!(error.code.as_deref(), Some(reason));
            if reason == "last-active-kind" {
                assert_eq!(serde_json::to_value(&error).unwrap(), contract["errors"][0]);
            }
        }
        let mut forged = serde_json::to_value(test_config()).unwrap();
        forged["capturePolicy"] =
            json!({"browsing":"paused","agentHistory":"off","enabledTools":[],"readGit":true});
        for cmd in ["save_config", "initialize_archive"] {
            let err = dispatch_command(&state, cmd, json!({"config":forged})).await.unwrap_err();
            assert_eq!(err.code.as_deref(), Some("last-active-kind"), "{cmd}");
        }
        forged["capturePolicy"]["browsing"] = json!("on");
        forged["capturePolicy"]["agentHistory"] = json!("on");
        forged["capturePolicy"]["enabledTools"] = json!(["pi"]);
        forged["capturePolicy"]["readGit"] = json!(false);
        for cmd in ["save_config", "initialize_archive"] {
            dispatch_command(&state, cmd, json!({"config":forged})).await.unwrap();
            state.session.close_vault().unwrap();
            let reopened = state.session.vault().unwrap();
            assert_eq!(
                reopened.config().capture_policy,
                vault_core::CapturePolicy::default(),
                "{cmd} forged consent"
            );
        }
        assert!(events().iter().all(|(name, _)| *name == "pathkeep://archive-upgrade"));
        let settings = dispatch_command(&state, "get_agent_settings", read).await.unwrap();
        assert_eq!(settings, contract["get_agent_settings"]["response"]);
        assert_eq!(settings["policy"]["enabledTools"], json!([]));
        let update = contract["update_agent_sources"]["request"].clone();
        let saved = dispatch_command(&state, "update_agent_sources", update.clone()).await.unwrap();
        assert_eq!(saved, contract["update_agent_sources"]["response"]);
        assert_ne!(saved["policyToken"], token);
        assert_eq!(saved["policy"]["enabledTools"], json!(["pi"]));
        assert_eq!(
            events(),
            vec![(
                contract["event"]["name"].as_str().unwrap(),
                contract["event"]["payload"].clone()
            )]
        );
        let stale = dispatch_command(&state, "update_agent_sources", update).await.unwrap_err();
        assert_eq!(stale.code.as_deref(), Some("policy-changed"));
        let no_op=dispatch_command(&state,"set_data_kind_state",json!({"request":{"kind":"browsing","state":"on","expectedPolicyToken":saved["policyToken"]}})).await.unwrap();
        assert_eq!(no_op, contract["set_data_kind_state"]["response"]);
        assert!(events().is_empty());
        let cancel = json!({"request":{"requestIdOrStreamId":"absent"}});
        assert_eq!(
            dispatch_command(&state, "cancel_agent_read", cancel.clone()).await.unwrap(),
            json!({"state":"cancelled"})
        );
        let preview = json!({"request":{"purpose":"enable","pickedFolder":{"tool":"pi","path":root.path().with_extension("missing")}}});
        let started =
            dispatch_command(&state, "preview_agent_sources", preview.clone()).await.unwrap();
        let mut pending =
            preview::subscribe(&state, started["streamId"].as_str().unwrap()).unwrap();
        let paths = vault_core::project_paths().unwrap();
        {
            let gate = vault_core::ControlGate::acquire(&paths);
            let mut config = vault_core::load_config(&paths).unwrap();
            config.app_lock.enabled = true;
            vault_core::save_config(&gate, &config).unwrap();
        }
        dispatch_command(&state, "lock_app_session", json!({"reason":"manual"})).await.unwrap();
        for (command, request) in [
            ("get_agent_settings", json!({"request":{}})),
            (
                "save_onboarding_choices",
                json!({"request":{"kinds":["browsing"],"agentTools":[],"readGit":true}}),
            ),
            (
                "cancel_agent_export",
                json!({"request":{"exportId":"fd25afcf-5aa6-483b-9d23-fa932b76b9c0"}}),
            ),
            ("list_data_kinds", json!({"request":{"includeArchiveBytes":true}})),
            ("get_agent_viewer_preferences", json!({"request":{}})),
            ("set_agent_viewer_preferences", json!({"request":{"assistantMayRead":true}})),
            ("query_agent_palette", json!({"request":{"text":"query"}})),
            ("export_agent_conversation", json!({"request":{"id":"locked","format":"html"}})),
            ("retry_agent_store", json!({"request":{}})),
            ("dismiss_agent_hint", json!({"request":{"hint":"intro"}})),
            ("detect_data_sources", json!({"request":{"kinds":["browsing","agent-history"]}})),
            ("update_agent_sources", json!({"request":{"expectedToken":token,"readGit":false}})),
            (
                "set_data_kind_state",
                json!({"request":{"expectedPolicyToken":token,"kind":"browsing","state":"paused"}}),
            ),
            ("cancel_agent_read", cancel),
            ("preview_agent_sources", preview),
        ] {
            let err = dispatch_command(&state, command, request).await.unwrap_err();
            assert_eq!(serde_json::to_value(err).unwrap(), contract["errors"][1], "{command}");
        }
        let frame = pending.next_frame().await.unwrap();
        assert_eq!(frame["type"], "error");
        assert_eq!(frame["payload"], json!({"code":"lock-required"}));
        assert!(pending.next_frame().await.is_none());
        assert!(events().is_empty());
    });
}

#[test]
fn app_surface_preview_contract() {
    let (_guard, _platform) = isolated_env();
    let archive = tempdir().unwrap();
    let home = tempdir().unwrap();
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, archive.path()) }
    let folder = home.path().join("claude");
    std::fs::create_dir_all(folder.join("projects/example")).unwrap();
    std::fs::write(folder.join("projects/example/demo.jsonl"), b"not JSON").unwrap();
    let transcript = std::fs::OpenOptions::new()
        .write(true)
        .open(folder.join("projects/example/demo.jsonl"))
        .unwrap();
    transcript
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1700000000)),
        )
        .unwrap();
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        let contract = wire();
        for (payload, code) in [
            (json!({}), "invalid-request"),
            (json!({"request":{"tools":["cline"],"purpose":"enable"}}), "invalid-request"),
            (
                json!({"request":{"purpose":"rescan","pickedFolder":{"tool":"pi","path":archive.path()}}}),
                "scope-denied",
            ),
            (
                json!({"request":{"purpose":"enable","tools":[],"pickedFolder":{"tool":"pi","path":folder}}}),
                "source-selection-empty",
            ),
        ] {
            let err = dispatch_command(&state, "preview_agent_sources", payload).await.unwrap_err();
            assert_eq!(err.code.as_deref(), Some(code));
        }
        let detected =
            dispatch_command(&state, "detect_data_sources", json!({"request":{"kinds":[]}}))
                .await
                .unwrap();
        assert_eq!(detected, json!({"sources":[],"profiles":[]}));
        let start=dispatch_command(&state,"preview_agent_sources",json!({"request":{"purpose":"enable","pickedFolder":{"tool":"claude-code","path":folder}}})).await.unwrap();
        let id = start["streamId"].as_str().unwrap();
        let mut frames = preview::subscribe(&state, id).unwrap();
        assert!(start["viewToken"].is_string());
        assert_eq!(start.as_object().unwrap().len(), 2);
        let mut sequence = 0;
        while let Some(frame) = frames.next_frame().await {
            assert_eq!(frame["streamId"], id);
            assert_eq!(frame["sequence"], sequence.to_string());
            sequence += 1;
            if frame["type"] == "finished" {
                let summary = &frame["payload"];
                assert_eq!(summary["totals"]["files"], "1");
                assert_eq!(summary["rows"][0]["counts"]["sourceBytes"], "8");
                assert!(summary["availableBytes"].as_str().unwrap().parse::<u64>().unwrap() > 0);
                assert!(summary["previewId"].is_string());
                assert!(sequence >= 3);
                let mut expected = contract["frames"][2]["payload"].clone();
                expected["previewId"] = summary["previewId"].clone();
                expected["availableBytes"] = summary["availableBytes"].clone();
                assert!(
                    summary["expiresAt"].as_i64().unwrap() > chrono::Utc::now().timestamp_millis()
                );
                expected["expiresAt"] = summary["expiresAt"].clone();
                expected["rows"][0]["path"] = json!(folder);
                assert_eq!(summary["rows"][0]["rootId"].as_str().unwrap().len(), 64);
                expected["rows"][0]["rootId"] = summary["rows"][0]["rootId"].clone();
                assert_eq!(summary["rows"][0]["directoryToken"].as_str().unwrap().len(), 64);
                expected["rows"][0]["directoryToken"] =
                    summary["rows"][0]["directoryToken"].clone();
                assert_eq!(summary, &expected);
                return;
            }
        }
        panic!("preview closed without a final inventory");
    });
}

/// The window can disappear immediately after enable. Repeating its request
/// must return the same task and there must be exactly one native backup run.
#[test]
fn enable_publishes_one_background_backup_of_the_reviewed_source() {
    let (_guard, _platform) = isolated_env();
    let archive = tempdir().unwrap();
    let home = tempdir().unwrap();
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, archive.path()) };
    let folder = home.path().join(".claude");
    std::fs::create_dir_all(folder.join("projects/example")).unwrap();
    std::fs::write(folder.join("projects/example/demo.jsonl"),
        br#"{"type":"user","uuid":"one","sessionId":"demo","message":{"role":"user","content":"hello"}}
"#).unwrap();
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let read = json!({"request":{}});
        let settings = dispatch_command(&state, "get_agent_settings", read.clone()).await.unwrap();
        let start = dispatch_command(&state,"preview_agent_sources",json!({"request":{"purpose":"enable","pickedFolder":{"tool":"claude-code","path":folder}}})).await.unwrap();
        let mut frames = preview::subscribe(&state, start["streamId"].as_str().unwrap()).unwrap();
        let summary = loop {
            let frame = frames.next_frame().await.unwrap();
            if frame["type"] == "finished" {
                break frame["payload"].clone();
            }
        };
        let request = json!({"request":{"previewId":summary["previewId"],"selectedRootIds":[summary["rows"][0]["rootId"]],"readGit":false,"expectedPolicyToken":settings["policyToken"],"requestId":"7f8ed74e-4f62-4146-9bd2-81f460ffcad8"}});
        let receipt =
            dispatch_command(&state, "enable_agent_history", request.clone()).await.unwrap();
        assert_eq!(receipt["state"], "on");
        assert!(receipt["firstSave"]["taskId"].is_string());
        assert_eq!(receipt["firstSave"]["requestId"], request["request"]["requestId"]);
        assert_eq!(
            dispatch_command(&state, "enable_agent_history", request).await.unwrap(),
            receipt
        );
        let stored = vault_core::load_config(&vault_core::project_paths().unwrap()).unwrap();
        assert_eq!(stored.capture_policy.agent_history, vault_core::KindState::On);
        assert!(!stored.capture_policy.read_git);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            let runs = dispatch_command(
                &state,
                "list_archive_runs",
                json!({"request":{"space":"agent-history","includeArchiveWide":false,"limit":10}}),
            )
            .await
            .unwrap();
            if let Some(row) = runs["items"].get(0) {
                assert_eq!(runs["items"].as_array().unwrap().len(), 1);
                if row["result"] != "running" {
                    assert_eq!(row["result"], "success", "{row}");
                    assert_eq!(row["runRef"], receipt["firstSave"]["runRef"]);
                    break;
                }
            }
            assert!(std::time::Instant::now() < deadline);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });
}

/// Every entry, including an unknown task/run lookup, has to classify the store
/// before inferring that a run, an approval, or a task does not exist.
#[test]
fn damaged_conversation_store_refuses_each_backup_and_tasks_command() {
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path()) };
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        state.session.close_vault().unwrap();
        std::fs::remove_file(root.path().join("archive/agent-history.db")).unwrap();
        let requests = [
            (
                "enable_agent_history",
                json!({"previewId":"p","selectedRootIds":["r"],"readGit":false,"expectedPolicyToken":"p","requestId":"q"}),
            ),
            (
                "start_backup",
                json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":"q"}),
            ),
            (
                "list_archive_runs",
                json!({"space":"agent-history","includeArchiveWide":true,"limit":1}),
            ),
            ("list_archive_runs", json!({"includeArchiveWide":true,"limit":1})),
            ("get_archive_run", json!({"runRef":"agent-history:missing"})),
            ("get_agent_overview", json!({"purpose":"backup"})),
            ("list_agent_sources", json!({"limit":1})),
            ("list_background_tasks", json!({"group":"attention","limit":1})),
            (
                "control_background_task",
                json!({"taskId":"missing","action":"cancel","expectedToken":"p","requestId":"q"}),
            ),
            ("update_agent_sources", json!({"expectedToken":"p","selectedRootIds":[]})),
        ];
        for (command, request) in &requests {
            let error =
                dispatch_command(&state, command, json!({"request":request})).await.unwrap_err();
            assert_eq!(error.code.as_deref(), Some("kind-unavailable"), "{command}: {error:?}");
        }
        let summary =
            dispatch_command(&state, "get_task_summary", json!({"request":{}})).await.unwrap();
        assert_eq!(summary["unavailableKinds"], json!(["agent-history"]));
        assert_eq!(summary["runningCount"], 0);
        let snapshot =
            dispatch_command(&state, "list_data_kinds", json!({"request":{}})).await.unwrap();
        assert_eq!(snapshot["kinds"][0]["availability"], "available");
        let paths = vault_core::project_paths().unwrap();
        {
            let gate = vault_core::ControlGate::acquire(&paths);
            let mut config = vault_core::load_config(&paths).unwrap();
            config.app_lock.enabled = true;
            vault_core::save_config(&gate, &config).unwrap();
        }
        dispatch_command(&state, "lock_app_session", json!({"reason":"manual"})).await.unwrap();
        assert_eq!(
            dispatch_command(&state, "get_task_summary", json!({"request":{}}))
                .await
                .unwrap_err()
                .code
                .as_deref(),
            Some("lock-required")
        );
        for (command, request) in requests {
            let error =
                dispatch_command(&state, command, json!({"request":request})).await.unwrap_err();
            assert_eq!(error.code.as_deref(), Some("lock-required"), "{command}");
        }
    });
}

#[test]
fn followup_sizes_are_opt_in() {
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, root.path().join("chrome"));
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let shell =
            dispatch_command(&state, "list_data_kinds", json!({"request":{}})).await.unwrap();
        assert!(shell["kinds"][0].get("archiveBytes").is_none());
        let mut cards = dispatch_command(
            &state,
            "list_data_kinds",
            wire()["followup"]["sized"]["request"].clone(),
        )
        .await
        .unwrap();
        for row in cards["kinds"].as_array().unwrap() {
            assert!(row["archiveBytes"].as_str().unwrap().parse::<u64>().unwrap() > 0);
        }
        assert_eq!(
            cards["kinds"][1]["archiveBytes"],
            state.session.vault().unwrap().conversation_archive_bytes().unwrap().to_string()
        );
        assert_eq!(
            cards["kinds"][0]["archiveBytes"],
            vault_core::archive::browsing_archive_bytes(&state.session.vault().unwrap())
                .unwrap()
                .to_string()
        );
        // Physical sizes depend on native database pages and WAL checkpoints.
        cards["kinds"][0]["archiveBytes"] = json!("262144");
        cards["kinds"][1]["archiveBytes"] = json!("131072");
        assert_eq!(cards, wire()["followup"]["sized"]["response"]);
    });
}

#[test]
fn followup_retry_rechecks_the_same_owner() {
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, root.path().join("chrome"));
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        state.session.close_vault().unwrap();
        let path = root.path().join("archive/agent-history.db");
        let copy = path.with_extension("held");
        std::fs::rename(&path, &copy).unwrap();
        let read = json!({"request":{}});
        let bad = dispatch_command(&state, "retry_agent_store", read.clone()).await.unwrap();
        assert_eq!(bad["kinds"][1]["availability"], "unavailable");
        let owner = state.session.current_vault().unwrap();
        std::fs::rename(copy, path).unwrap();
        let changes = state.events.observe();
        for _ in 0..2 {
            let good = dispatch_command(&state, "retry_agent_store", read.clone()).await.unwrap();
            assert_eq!(good, wire()["followup"]["retry_agent_store"]["response"]);
            assert!(std::sync::Arc::ptr_eq(
                owner.store(),
                state.session.current_vault().unwrap().store()
            ));
        }
        assert_eq!(
            changes(),
            vec![(
                vault_worker::data_kinds::POLICY_EVENT,
                wire()["followup"]["retry_agent_store"]["event"]["payload"].clone()
            )]
        );
    });
}

#[test]
fn followup_preferences_survive_restart_and_hints_preserve_the_preview_token() {
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, root.path().join("chrome"));
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let read = json!({"request":{}});
        let initial = dispatch_command(&state, "get_agent_settings", read.clone()).await.unwrap();
        assert_eq!(initial["toolsChosen"], false);
        let saved = dispatch_command(
            &state,
            "update_agent_sources",
            json!({"request":{"expectedToken":initial["policyToken"],"toolChanges":{"pi":false}}}),
        )
        .await
        .unwrap();
        assert_eq!(saved, wire()["followup"]["unticked"]["response"]);
        assert_eq!(saved["policy"]["enabledTools"], json!([]));
        let changes = state.events.observe();
        let tool = tempdir().unwrap();
        let start = dispatch_command(
            &state,
            "preview_agent_sources",
            json!({"request":{"purpose":"enable","pickedFolder":{"tool":"pi","path":tool.path()}}}),
        )
        .await
        .unwrap();
        let mut frames = preview::subscribe(&state, start["streamId"].as_str().unwrap()).unwrap();
        let review = loop {
            let frame = frames.next_frame().await.unwrap();
            if frame["type"] == "finished" {
                break frame["payload"].clone();
            }
        };
        for hint in ["intro", "intro", "auto-delete"] {
            let result =
                dispatch_command(&state, "dismiss_agent_hint", json!({"request":{"hint":hint}}))
                    .await
                    .unwrap();
            assert_eq!(result, json!({"hint":hint,"dismissed":true}));
        }
        let events = changes();
        assert_eq!(events.len(), 2);
        for (name, event) in &events {
            assert_eq!(*name, "pathkeep://agent-settings-changed");
            assert_eq!(event["domains"], json!(["hints"]));
        }
        assert_ne!(events[0].1["settingsToken"], events[1].1["settingsToken"]);
        assert_eq!(
            vault_worker::data_kinds::preview::reviewed_sources(
                review["previewId"].as_str().unwrap(),
                &[review["rows"][0]["rootId"].as_str().unwrap().into()]
            )
            .unwrap()
            .len(),
            1
        );
        state.session.close_vault().unwrap();
        let later = dispatch_command(&state, "get_agent_settings", read).await.unwrap();
        assert_eq!(later["policyToken"], saved["policyToken"]);
        assert_eq!(later["toolsChosen"], true);
        assert_eq!(later["dismissedHints"], json!(["intro", "auto-delete"]));
    });
}

/// Captures actual transport JSON for the repair screen, including legacy commands whose
/// success or refusal changes during damage. The only credentials are synthetic test inputs.
#[test]
fn conversation_repair_screen_wire_contract() {
    let _guard = lock_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
        std::env::set_var(TEST_KEYRING_OVERRIDE_ENV, root.path().join("keyring"));
        std::env::set_var(CHROME_USER_DATA_OVERRIDE_ENV, root.path().join("chrome"));
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let mut records = Vec::new();
        macro_rules! invoke { ($cmd:expr,$args:expr,$ok:expr) => {{
            let request=$args;
            let events=state.events.observe();
            let result=dispatch_command(&state,$cmd,request.clone()).await;
            assert_eq!(result.is_ok(),$ok,"{} {:?}",$cmd,result);
            let frames=events().into_iter().map(|(name,payload)|json!({"name":name,"payload":payload})).collect::<Vec<_>>();
            match result {
                Ok(response)=>{records.push(json!({"command":$cmd,"invoke":request,"response":response,"events":frames}));response}
                Err(e)=>{let error=serde_json::to_value(e).unwrap();records.push(json!({"command":$cmd,"invoke":request,"error":error,"events":frames}));error}
            }
        }} }
        let empty = json!({"request":{}});
        let healthy = invoke!("preview_archive_safety_copy", empty.clone(), true);
        let good = invoke!(
            "create_archive_safety_copy",
            json!({"request":{"previewToken":healthy["previewToken"]}}),
            true
        );
        let bundle = root.path().join("exports/pathkeep/format-3/demo.pathkeep");
        dispatch_command(
            &state,
            "export_app_data",
            json!({"targetPath":bundle,"exportPassword":"fixture"}),
        )
        .await
        .unwrap();
        state.session.close_vault().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let evidence = root.path().join("archive/agent-history.db");
            let original = std::fs::metadata(&evidence).unwrap().permissions();
            std::fs::set_permissions(&evidence, std::fs::Permissions::from_mode(0o000)).unwrap();
            if std::fs::File::open(&evidence).is_err() {
                let denied = invoke!("preview_agent_history_repair", empty.clone(), false);
                assert_eq!(denied["code"], "conversation-access-unavailable");
                assert_eq!(denied["actionHint"], "retry_agent_store");
                assert_eq!(denied["affectedFile"], json!({"path": evidence, "role":"evidence"}));
                state.session.close_vault().unwrap();
            }
            std::fs::set_permissions(evidence, original).unwrap();
        }
        std::fs::write(root.path().join("archive/agent-history.db"), b"bad").unwrap();
        let kinds = invoke!("list_data_kinds", empty.clone(), true);
        assert_eq!(kinds["kinds"][1]["availability"], "unavailable");
        invoke!("retry_agent_store", empty.clone(), true);
        let plan = invoke!("preview_archive_safety_copy", empty.clone(), true);
        let marked = invoke!(
            "create_archive_safety_copy",
            json!({"request":{"previewToken":plan["previewToken"]}}),
            true
        );
        let legacy = root.path().join("raw-snapshots/periodic/old.sqlite");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        rusqlite::Connection::open(&legacy)
            .unwrap()
            .execute_batch("CREATE TABLE old_history(id INTEGER PRIMARY KEY)")
            .unwrap();
        let listing = invoke!("list_recovery_snapshots", json!({}), true);
        for (path, eligibility) in [
            (good["path"].clone(), "checkRequired"),
            (marked["path"].clone(), "checkRequired"),
            (json!(legacy), "missingEvidence"),
        ] {
            let row = listing.as_array().unwrap().iter().find(|row| row["path"] == path).unwrap();
            assert_eq!(row["conversationRepairSource"], eligibility);
        }
        #[cfg(unix)]
        for (relative, role) in [
            ("archive/agent-history.db-wal", "evidence"),
            ("derived/agent-history-read-model.db-wal", "readModel"),
        ] {
            // A symlink cannot be preserved as an opaque companion.
            let member = root.path().join(relative);
            let parked = root.path().join("parked-wal");
            if member.exists() {
                std::fs::rename(&member, &parked).unwrap();
            }
            std::os::unix::fs::symlink(&legacy, &member).unwrap();
            let error = invoke!(
                "create_archive_safety_copy",
                json!({"request":{"previewToken":plan["previewToken"]}}),
                false
            );
            std::fs::remove_file(&member).unwrap();
            if parked.exists() {
                std::fs::rename(parked, &member).unwrap();
            }
            assert_eq!(error["code"], "conversation-preservation-failed");
            assert_eq!(error["affectedFile"], json!({"role":role,"path":member}));
        }
        let browsing = state
            .session
            .current_vault()
            .unwrap()
            .store()
            .database_path(vault_store::Db::Archive)
            .to_owned();
        let parked = root.path().join("parked-browsing");
        std::fs::rename(&browsing, &parked).unwrap();
        let error = invoke!("preview_archive_safety_copy", json!({"request":{}}), false);
        std::fs::rename(parked, &browsing).unwrap();
        assert_eq!(error["affectedFile"], json!({"role":"browsing","path":browsing}));
        invoke!("preview_retention_prune", json!({}), true);
        let check = invoke!("reconcile_archive_encryption", json!({}), true);
        assert_eq!(check["conversationsUnreadable"], true);
        for (name, request) in [
            ("rekey_archive", json!({"request":{"newMode":"Encrypted","newKey":"fixture"}})),
            (
                "export_app_data",
                json!({"targetPath":root.path().join("exports/refused.pathkeep"),"exportPassword":"fixture"}),
            ),
            (
                "apply_app_data_import",
                json!({"bundlePath":bundle,"options":{"confirmOverwrite":true,"sourceArchiveKey":"fixture"}}),
            ),
        ] {
            let e = invoke!(name, request, false);
            assert_eq!(
                e["affectedFile"],
                json!({"role":"evidence","path":root.path().join("archive/agent-history.db")})
            );
            assert_eq!(e["code"], "conversation-repair-required");
            assert_eq!(e["actionHint"], "preview_agent_history_repair");
        }
        let restored = invoke!(
            "run_full_archive_restore",
            json!({"request":{"snapshotPath":marked["path"]}}),
            true
        );
        let refused = invoke!(
            "preview_agent_history_repair",
            json!({"request":{"copyPath":marked["path"]}}),
            false
        );
        assert_eq!(refused["code"], "conversation-copy-no-verified-evidence");
        assert_eq!(
            refused["affectedFile"],
            json!({
                "role":"evidence", "path":std::path::Path::new(marked["path"].as_str().unwrap()).join("archive/agent-history.db")
            })
        );
        let good_path = std::path::Path::new(good["path"].as_str().unwrap());
        let model = good_path.join("derived/agent-history-read-model.db");
        let parked_model = root.path().join("model-from-copy");
        std::fs::rename(&model, &parked_model).unwrap();
        let replaced = &restored["quarantineDir"];
        let seal = std::path::Path::new(replaced.as_str().unwrap()).join("snapshot.json");
        let parked_seal = root.path().join("seal-from-copy");
        std::fs::rename(&seal, &parked_seal).unwrap();
        let rows = invoke!("list_recovery_snapshots", json!({}), true);
        assert_eq!(
            rows.as_array().unwrap().iter().find(|r| r["path"] == good["path"]).unwrap()["conversationRepairSource"],
            "missingReadModel"
        );
        let unsealed = rows.as_array().unwrap().iter().find(|r| r["path"] == *replaced).unwrap();
        assert_eq!(unsealed["conversationRepairSource"], "unsealed");
        std::fs::rename(parked_seal, seal).unwrap();
        let incomplete = invoke!(
            "preview_agent_history_repair",
            json!({"request":{"copyPath":good["path"]}}),
            false
        );
        assert_eq!(incomplete["code"], "conversation-copy-incomplete");
        assert_eq!(incomplete["affectedFile"], json!({"role":"readModel","path":model}));
        std::fs::rename(parked_model, model).unwrap();
        let copy = invoke!(
            "preview_agent_history_repair",
            json!({"request":{"copyPath":good["path"]}}),
            true
        );
        invoke!("preview_agent_history_repair", empty.clone(), true);
        let cancelled = invoke!("cancel_maintenance", json!({}), true);
        assert_eq!(cancelled, true);
        let stale = invoke!(
            "repair_agent_history",
            json!({"request":{"copyPath":good["path"],"previewToken":copy["token"]}}),
            false
        );
        assert_eq!(stale["code"], "conversation-repair-preview-stale");
        assert!(stale.get("affectedFile").is_none());
        let copy = invoke!(
            "preview_agent_history_repair",
            json!({"request":{"copyPath":good["path"]}}),
            true
        );
        invoke!(
            "repair_agent_history",
            json!({"request":{"copyPath":good["path"],"previewToken":copy["token"]}}),
            true
        );
        state.session.close_vault().unwrap();
        std::fs::write(root.path().join("derived/agent-history-read-model.db"), b"bad").unwrap();
        let rebuild = invoke!("preview_agent_history_repair", empty.clone(), true);
        invoke!("repair_agent_history", json!({"request":{"previewToken":rebuild["token"]}}), true);
        state.session.close_vault().unwrap();
        std::fs::write(root.path().join("archive/agent-history.db"), b"bad again").unwrap();
        let fresh = invoke!("preview_agent_history_repair", empty, true);
        invoke!("repair_agent_history", json!({"request":{"previewToken":fresh["token"]}}), true);
        let live_model = root.path().join("derived/agent-history-read-model.db");
        let removed = root.path().join("displaced-model.db");
        std::fs::rename(&live_model, &removed).unwrap();
        let observed = state.events.observe();
        let copies = std::path::Path::new(good["path"].as_str().unwrap()).parent().unwrap();
        let parked = root.path().join("parked-copies");
        std::fs::rename(copies, &parked).unwrap();
        std::fs::write(copies, b"publication blocked").unwrap();
        let refused = invoke!("preview_archive_safety_copy", json!({"request":{}}), false);
        std::fs::remove_file(copies).unwrap();
        std::fs::rename(parked, copies).unwrap();
        assert_eq!(refused["affectedFile"], json!({"role":"heldGeneration","path":live_model}));
        invoke!("list_data_kinds", json!({"request":{}}), true);
        let at_rest = invoke!("reconcile_archive_encryption", json!({}), true);
        assert_eq!(at_rest["conversationsUnreadable"], true);
        assert!(
            observed()
                .iter()
                .any(|(name, payload)| *name == vault_worker::data_kinds::POLICY_EVENT
                    && payload["changedKinds"] == json!(["agent-history"]))
        );
        std::fs::rename(removed, live_model).unwrap();
        let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../artifacts/vault-lifecycle/repair-round4");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(
            output.join("command-wire.json"),
            serde_json::to_vec_pretty(&records).unwrap(),
        )
        .unwrap();
        state.session.close_vault().unwrap();
    });
}

#[path = "chat_wire_tests.rs"]
mod chat_wire;

#[test]
fn onboarding_choices_are_a_window_contract_before_setup() {
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        let draft = json!({"kinds":["browsing","agent-history"],"agentTools":["codex","pi"],"readGit":false});
        let saved = dispatch_command(&state, "save_onboarding_choices", json!({"request":draft}))
            .await
            .unwrap();
        assert_eq!(saved, draft);
        let init = dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        assert!(init["config"].get("onboardingChoices").is_none());
        assert_eq!(init["config"]["capturePolicy"]["agentHistory"], "off");
        assert_eq!(init["config"]["capturePolicy"]["browsing"], "on");
        assert!(init["config"]["capturePolicy"]["selectionRef"].is_null());
        assert_eq!(init["config"]["capturePolicy"]["enabledTools"], json!(["codex", "pi"]));
        let err = dispatch_command(&state, "save_onboarding_choices", json!({"request":draft}))
            .await
            .unwrap_err();
        assert_eq!(err.code.as_deref(), Some("archive-already-initialized"));
    });
}

#[test]
fn browsing_queue_commits_reach_the_tasks_event_stream() {
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe {
        std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path());
    }
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    ready_block_on(async {
        dispatch_command(&state, "initialize_archive", json!({"config":test_config()}))
            .await
            .unwrap();
        let observed = state.events.observe();
        let vault = state.session.vault().unwrap();
        let queued = vault_core::ai_queue::enqueue_index_job(
            &vault,
            &serde_json::from_value(json!({"clearOnly":true,"fullRebuild":false})).unwrap(),
            true,
        )
        .unwrap();
        assert!(observed().iter().any(|(n, v)| *n == "pathkeep://background-tasks-changed"
            && *v == json!({"kind":"browsing","taskId":null})));
        let waiting = dispatch_command(
            &state,
            "list_background_tasks",
            json!({"request":{"group":"waiting","kind":"browsing","limit":10}}),
        )
        .await
        .unwrap();
        assert!(
            waiting["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["taskId"] == format!("browsing:index/{}", queued.id))
        );
        vault_core::ai_queue::cancel_ai_job(&vault, queued.id).unwrap();
        assert!(observed().iter().any(|(n, _)| *n == "pathkeep://background-tasks-changed"));
    });
}

#[test]
fn archive_access_reports_read_and_write_floors_without_opening_the_root() {
    let (_guard, _platform) = isolated_env();
    let root = tempdir().unwrap();
    unsafe { std::env::set_var(PROJECT_ROOT_OVERRIDE_ENV, root.path()) };
    let state =
        DevIpcBridgeState::without_app(SessionState::default(), DEFAULT_DEV_IPC_BRIDGE_PORT);
    let read =
        || ready_block_on(dispatch_command(&state, "get_archive_access", json!({}))).unwrap();
    assert_eq!(read()["state"], "notInitialized");
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    let path = root.path().join("config.json");
    for (reader, writer, expected) in [(2, 2, "refusedRead"), (1, 2, "refusedWrite")] {
        let control=json!({"archiveMode":{"formatVersion":2,"mode":"Plaintext","compatibility":{"revision":2,"minReader":reader,"minWriter":writer}}}).to_string();
        std::fs::write(&path, &control).unwrap();
        let report = read();
        assert_eq!(report["state"], expected);
        assert_eq!(report["archive"]["minWriter"], writer);
        assert_eq!(report["buildLevel"], 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), control);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
    std::fs::write(&path, b"broken settings").unwrap();
    assert_eq!(read()["state"], "needsRecovery");
    assert_eq!(read()["code"], "archive-control-invalid");
    std::fs::remove_file(path).unwrap();
    ready_block_on(dispatch_command(&state, "initialize_archive", json!({"config":test_config()})))
        .unwrap();
    assert_eq!(read()["state"], "open");
    let volume = ready_block_on(dispatch_command(
        &state,
        "get_storage_volume",
        json!({"folder":root.path().join("new")}),
    ))
    .unwrap();
    assert!(volume["fat"].is_boolean());
    assert!(!root.path().join("new").exists());
}
