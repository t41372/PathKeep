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
            ("list_data_kinds", json!({"request":{"includeArchiveBytes":true}})),
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
            ("get_task_summary", json!({})),
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
