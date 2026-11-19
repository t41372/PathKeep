//! Failure modes: the second kind loses its last-on protection, changing unrelated settings
//! manufactures consent, previews attach or write a store, or cancellation loses its terminal.
#[allow(dead_code)]
#[path = "root_admission/support.rs"]
mod support;
use serde_json::{from_value, json, to_value};
use support::Fixture;
use vault_core::{CapturePolicy, ControlGate, DataKind, KindState, load_config, save_config};
use vault_worker::data_kinds::{self as kinds, Refusal, preview};

#[test]
fn either_kind_as_last_on_is_protected_with_every_other_state() {
    let fixture = Fixture::new(true);
    for (kind, other) in
        [(DataKind::Browsing, DataKind::AgentHistory), (DataKind::AgentHistory, DataKind::Browsing)]
    {
        for other_state in [KindState::On, KindState::Paused, KindState::Off] {
            for state in [KindState::Off, KindState::Paused] {
                let mut config = load_config(&fixture.paths()).unwrap();
                config.capture_policy = match kind {
                    DataKind::Browsing => {
                        CapturePolicy { agent_history: other_state, ..Default::default() }
                    }
                    DataKind::AgentHistory => CapturePolicy {
                        agent_history: KindState::On,
                        browsing: other_state,
                        ..Default::default()
                    },
                };
                save_config(&ControlGate::acquire(&fixture.paths()), &config).unwrap();
                let request = kinds::SetState {
                    kind,
                    state,
                    expected_policy_token: config.capture_policy.token(),
                };
                let mut changed = Vec::new();
                let result =
                    kinds::set_state(&fixture.session, request, |event| changed.push(event));
                let current = kinds::settings().unwrap();
                if other_state == KindState::On {
                    let response = result.unwrap();
                    assert_eq!(response.snapshot.policy_token, current.policy_token);
                    assert_eq!(current.policy.state(kind), state);
                    assert_eq!(changed.len(), 1);
                    assert_eq!(
                        to_value(&changed[0]).unwrap(),
                        json!({"policyToken":current.policy_token,"changedKinds":[kind]})
                    );
                } else {
                    assert_eq!(
                        result.unwrap_err().downcast::<Refusal>().unwrap().code(),
                        "last-active-kind"
                    );
                    assert_eq!(current.policy, config.capture_policy);
                    assert!(changed.is_empty());
                }
                assert_eq!(current.policy.state(other), other_state);
            }
        }
    }
}

#[test]
fn preview_reads_on_the_target_volume_and_writes_no_capture_rows() {
    use vault_store::Db;
    let fixture = Fixture::new(true);
    let source = fixture.temp.path().join("pi-sessions");
    std::fs::create_dir(&source).unwrap();
    for i in 0..270 {
        std::fs::write(source.join(format!("{i}.jsonl")), b"ignored").unwrap()
    }
    let before = fixture.observe();
    let request = || {
        from_value(json!({"purpose":"onboarding","pickedFolder":{"tool":"pi","path":source}}))
            .unwrap()
    };
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let before_free = fs4::available_space(&fixture.root).unwrap();
        let mut frames = preview::start(&fixture.session, request()).unwrap();
        let mut finished = false;
        while let Some(frame) = frames.next_frame().await {
            if frame["type"] == "finished" {
                let payload = &frame["payload"];
                assert_eq!(payload["totals"]["files"], "270");
                let available = payload["availableBytes"].as_str().unwrap().parse::<u64>().unwrap();
                let after_free = fs4::available_space(&fixture.root).unwrap();
                // Target-volume readings can change while the rest of the machine writes.
                assert!(
                    available.abs_diff(before_free).min(available.abs_diff(after_free))
                        < 64 * 1024 * 1024
                );
                let id = payload["previewId"].as_str().unwrap();
                let root = payload["rows"][0]["rootId"].as_str().unwrap().to_owned();
                assert_eq!(preview::reviewed_sources(id, &[root]).unwrap()[0].root, source);
                let root = payload["rows"][0]["rootId"].as_str().unwrap().to_owned();
                std::fs::rename(&source, source.with_extension("previous")).unwrap();
                std::fs::create_dir(&source).unwrap();
                assert_eq!(
                    preview::reviewed_sources(id, &[root])
                        .unwrap_err()
                        .downcast::<Refusal>()
                        .unwrap()
                        .code(),
                    "root-changed"
                );
                finished = true;
            }
        }
        assert!(finished);
        let mut cancelled = preview::start(&fixture.session, request()).unwrap();
        let ack = preview::cancel(
            from_value(json!({"requestIdOrStreamId":cancelled.started.stream_id})).unwrap(),
        )
        .unwrap();
        assert_eq!(ack, json!({"state":"cancelled"}));
        assert_eq!(cancelled.next_frame().await.unwrap()["type"], "cancelled");
        assert!(cancelled.next_frame().await.is_none());
    });
    let rows=fixture.vault().store().read(Db::AgentHistoryEvidence,|s|s.query_one(
        "SELECT (SELECT count(*) FROM ah_sources)+(SELECT count(*) FROM ah_revisions)+(SELECT count(*) FROM ah_backup_runs)",(),|r|Ok(r.get::<i64>(0)?))).unwrap();
    assert_eq!(rows, 0);
    assert_eq!(before, fixture.observe());
}

#[test]
fn damaged_role_has_unknown_inventory_while_browsing_stays_readable() {
    use vault_store::Db;
    let mut fixture = Fixture::new(true);
    fixture.session.close_vault().unwrap();
    drop(fixture.vault.take());
    std::fs::remove_file(fixture.root.join(vault_layout::database_path(Db::AgentHistoryEvidence)))
        .unwrap();
    let result = to_value(kinds::list(&fixture.session).unwrap()).unwrap();
    assert_eq!(result["kinds"][0]["availability"], "available");
    let agent = &result["kinds"][1];
    assert_eq!(agent["state"], "off");
    assert_eq!(agent["availability"], "unavailable");
    assert_eq!(agent["hasSavedData"], serde_json::Value::Null);
    assert!(agent.get("archiveCount").is_none());
    assert_eq!(agent["failure"]["role"], "evidence");
    assert_eq!(agent["failure"]["reason"], "archive-store-missing");
}

#[allow(dead_code)]
#[path = "../../agent-history/tests/support/fixtures.rs"]
mod fixtures;
#[path = "../../agent-history/tests/support/native.rs"]
mod native;

fn write(path: &std::path::Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn native_home_presence_and_one_complete_four_tool_preview() {
    const CHILD: &str = "PATHKEEP_NATIVE_PREVIEW_FIXTURE";
    let Some(home) = std::env::var_os(CHILD) else {
        let home = tempfile::tempdir().unwrap();
        let fixture = fixtures::Fixture::four_harnesses(home.path());
        for i in 0..260 {
            native::claude_main(&fixture.roots[0].1, &format!("case-{i}"), &[]);
        }
        native::claude_upload(&fixture.roots[0].1, "case-0", "proof", b"ancillary");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .current_dir(home.path())
            .args([
                "--exact",
                "native_home_presence_and_one_complete_four_tool_preview",
                "--nocapture",
            ])
            .env_remove("CHB_AGENT_TOOLS_HOME")
            .env(CHILD, home.path())
            .env("HOME", home.path())
            .env("USERPROFILE", home.path())
            .env("CLAUDE_CONFIG_DIR", &fixture.roots[0].1)
            .env("CODEX_HOME", &fixture.roots[1].1)
            .env("PI_CODING_AGENT_DIR", &fixture.roots[2].1)
            .env("DSH_HOME", &fixture.roots[3].1)
            .env("CODEX_SQLITE_HOME", home.path().join("missing"))
            .env_remove("PI_CODING_AGENT_SESSION_DIR");
        #[cfg(unix)]
        let denied = {
            use std::os::unix::fs::PermissionsExt;
            let path = home.path().join("denied");
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
            child.env("PI_CODING_AGENT_SESSION_DIR", &path);
            path
        };
        let result = child.status().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(denied, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(result.success());
        return;
    };
    let archive = Fixture::new(true);
    let before = archive.observe();
    let detected =
        preview::detect(from_value(json!({"kinds":["agent-history"]})).unwrap()).unwrap();
    let home = std::path::PathBuf::from(home);
    let roots: Vec<_> = detected["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            std::path::Path::new(row["path"].as_str().unwrap()).starts_with(home.join("sources"))
                || std::path::Path::new(row["path"].as_str().unwrap()) == home.join("denied")
                || std::path::Path::new(row["path"].as_str().unwrap()) == home.join("missing")
        })
        .collect();
    let root_count = if cfg!(unix) { 6 } else { 5 };
    assert_eq!(roots.len(), root_count);
    assert_eq!(roots.iter().filter(|r| r["found"] == true && r["readable"] == true).count(), 4);
    assert!(roots.iter().any(|r| r["reason"] == "root-missing" && r["found"] == false));
    let wire: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/data-kinds-preview.json"))
            .unwrap();
    let expected = wire["detect_data_sources"]["response"]["sources"].as_array().unwrap();
    for actual in &roots {
        let mut row = expected
            .iter()
            .find(|r| r["tool"] == actual["tool"] && r["reason"] == actual["reason"])
            .unwrap()
            .clone();
        assert!(actual["path"].as_str().is_some());
        assert_eq!(actual["rootIds"].as_array().unwrap().len(), 1);
        assert_eq!(actual["rootIds"][0].as_str().unwrap().len(), 64);
        row["rootIds"] = actual["rootIds"].clone();
        row["path"] = actual["path"].clone();
        assert_eq!(**actual, row);
    }
    let ids: Vec<_> = roots.iter().map(|r| &r["rootIds"][0]).collect();
    let mut frames = preview::start(
        &archive.session,
        from_value(json!({"purpose":"enable","rootIds":ids})).unwrap(),
    )
    .unwrap();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        while let Some(frame) = frames.next_frame().await {
            if frame["type"] == "finished" {
                let report = &frame["payload"];
                assert_eq!(report["rows"].as_array().unwrap().len(), root_count);
                #[cfg(unix)]
                assert_eq!(report["byTool"]["pi"]["reasons"], json!(["root-unreadable"]));
                assert_eq!(report["byTool"]["claude-code"]["files"], "262");
                assert_eq!(report["byTool"]["claude-code"]["ancillaryFiles"], "1");
                assert_eq!(report["byTool"]["codex"]["readableRoots"], 1);
                assert_eq!(report["byTool"]["codex"]["roots"], 2);
                assert!(
                    report["byTool"]["deepseek"]["compressedBytes"]
                        .as_str()
                        .unwrap()
                        .parse::<u64>()
                        .unwrap()
                        > 0
                );
                assert!(report["totals"]["files"].as_str().unwrap().parse::<u64>().unwrap() > 262);
                return;
            }
        }
        panic!("missing final inventory")
    });
    assert_eq!(before, archive.observe());
}
