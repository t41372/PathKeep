//! Regressions at the owner boundary. All discovery stays inside this fixture.
use super::*;
use serde_json::Value;
use vault_core::AgentTool;

fn walk(f: &Fixture, req: Value) -> anyhow::Result<Value> {
    let mut frames = preview::start(&f.session, from_value(req)?)?;
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(async {
        while let Some(frame) = frames.next_frame().await {
            if frame["type"] == "finished" {
                return Ok(frame["payload"].clone());
            }
            anyhow::ensure!(frame["type"] != "error", "{frame}");
        }
        anyhow::bail!("no terminal inventory")
    })
}

fn custom(f: &Fixture) -> (Value, String) {
    let source = f.temp.path().join("chosen-pi");
    write(&source.join("sessions/test.jsonl"), b"{}");
    let report =
        walk(f, json!({"purpose":"enable","pickedFolder":{"tool":"pi","path":source}})).unwrap();
    let id = report["rows"][0]["rootId"].as_str().unwrap().to_owned();
    (report, id)
}

pub(super) fn grant(f: &Fixture) -> String {
    use agent_history::capture::{directory_handle, directory_identity};
    let (report, id) = custom(f);
    let path = std::path::PathBuf::from(report["rows"][0]["path"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    let directory = directory_identity(&directory_handle(&path).unwrap()).unwrap();
    let binding = json!([{"id":id,"tool":"pi","path":path,"directory":directory}]).to_string();
    f.vault()
        .store()
        .write(
            vault_store::Db::AgentHistoryEvidence,
            vault_store::Priority::Interactive,
            move |s| {
                s.execute(
                    "INSERT INTO ah_capture_selections(id,descriptor_json) VALUES('chosen',?1)",
                    [binding],
                )?;
                Ok(())
            },
        )
        .unwrap();
    let mut config = load_config(&f.paths()).unwrap();
    config.capture_policy.agent_history = KindState::On;
    config.capture_policy.enabled_tools.insert(AgentTool::Pi);
    config.capture_policy.selection_ref = Some("chosen".into());
    save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
    id
}

#[test]
fn rescan_approved_custom_root_checks_original_identity() {
    let f = Fixture::new(true);
    let id = grant(&f);
    let req = json!({"purpose":"rescan","tools":["pi"],"rootIds":[id]});
    let summary = walk(&f, req.clone()).unwrap();
    assert_eq!(summary["rows"][0]["rootId"], id);
    let mut mixed = req.clone();
    let default = f.temp.path().join("agent-tools/.pi/agent");
    write(&default.join("sessions/native.jsonl"), b"{}");
    let default_id = agent_history::capture::root_identity(&agent_history::SourceRoot {
        harness: agent_history::Harness::Pi,
        root: default,
    })
    .unwrap();
    mixed["rootIds"] = json!([id, default_id]);
    let report = walk(&f, mixed.clone()).unwrap();
    let ids = report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["rootId"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids, std::collections::BTreeSet::from([id.as_str(), default_id.as_str()]));
    mixed["rootIds"] = json!([id, "unapproved"]);
    assert_eq!(walk(&f, mixed).unwrap_err().to_string(), "scope-denied");
    let path = f.temp.path().join("chosen-pi");
    std::fs::rename(&path, path.with_extension("previous")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_eq!(walk(&f, req).unwrap_err().to_string(), "root-changed");
}
#[test]
fn enable_ticket_survives_only_reviewed_choice_changes() {
    let f = Fixture::new(true);
    let (report, id) = custom(&f);
    let ticket = report["previewId"].as_str().unwrap();
    let (other, _) = custom(&f);
    let info = kinds::settings().unwrap();
    let patch = json!({
        "expectedToken":info.policy_token,
        "previewId":ticket,
        "readGit":false,
        "toolChanges":{"pi":true}
    });
    let saved = kinds::update_sources(&f.session, from_value(patch).unwrap(), |_| {}).unwrap();
    assert!(
        preview::reviewed_sources(other["previewId"].as_str().unwrap(), std::slice::from_ref(&id))
            .is_err()
    );
    preview::reviewed_sources(ticket, std::slice::from_ref(&id)).unwrap();
    kinds::set_state(
        &f.session,
        from_value(
            json!({"expectedPolicyToken":saved.policy_token,"kind":"agent-history","state":"off"}),
        )
        .unwrap(),
        |_| {},
    )
    .unwrap();
    assert!(preview::reviewed_sources(ticket, &[id]).is_err());
}

#[test]
fn only_on_agent_cannot_lose_its_last_selected_tool_or_root() {
    let f = Fixture::new(true);
    grant(&f);
    let mut config = load_config(&f.paths()).unwrap();
    config.capture_policy.browsing = KindState::Paused;
    save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
    for mut patch in [json!({"toolChanges":{"pi":false}}), json!({"selectedRootIds":[]})] {
        patch["expectedToken"] = json!(config.capture_policy.token());
        let error =
            kinds::update_sources(&f.session, from_value(patch).unwrap(), |_| {}).unwrap_err();
        assert_eq!(error.to_string(), "source-selection-empty");
        assert_eq!(load_config(&f.paths()).unwrap().capture_policy, config.capture_policy);
    }
}
#[test]
fn only_on_browser_cannot_lose_its_last_profile() {
    let f = Fixture::new(true);
    let mut config = load_config(&f.paths()).unwrap();
    config.selected_profile_ids = vec!["chrome:Default".into()];
    save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
    let mut next = config.clone();
    next.selected_profile_ids.clear();
    let err =
        vault_worker::save_user_config_with_vault(Some(f.vault()), &next, Some(&config), None)
            .unwrap_err();
    assert_eq!(err.to_string(), "source-selection-empty");
    assert_eq!(load_config(&f.paths()).unwrap().selected_profile_ids, config.selected_profile_ids);
}
#[test]
fn scheduled_browsing_obeys_paused_and_off() {
    let f = Fixture::new(true);
    for state in [KindState::Paused, KindState::Off] {
        let mut cfg = load_config(&f.paths()).unwrap();
        cfg.capture_policy.browsing = state;
        cfg.capture_policy.agent_history = KindState::On;
        save_config(&ControlGate::acquire(&f.paths()), &cfg).unwrap();
        let report = vault_worker::run_backup_now(f.vault(), None, true).unwrap();
        assert_eq!(
            report.reason_code.as_deref(),
            Some(if state == KindState::Paused { "kind-paused" } else { "kind-off" })
        );
        assert!(report.due_skipped && report.run.is_none());
    }
}

// Use the actual stream consumed by the preview. Equal counts or file sizes
// cannot distinguish a forgotten task from a different extra file.
fn assert_layout(home: &str, harness: agent_history::Harness, paths: &[&str], expected: &[&str]) {
    use agent_history::{
        SourceRoot,
        capture::{BackupOptions, InventoryObserver, inventory_root},
    };
    use std::collections::BTreeSet;
    struct Files<'a>(&'a mut BTreeSet<String>);
    impl InventoryObserver for Files<'_> {
        fn progress(&mut self, _: u64) {}
        fn candidate(&mut self, file: &agent_history::Artifact) {
            self.0.insert(file.relative_path.clone());
        }
    }
    let f = Fixture::new(true);
    let source = SourceRoot { root: f.temp.path().join(home), harness };
    for path in paths {
        write(&source.root.join(path), b"{}");
    }
    let mut seen = BTreeSet::new();
    let report = inventory_root(&source, &Default::default(), Files(&mut seen)).unwrap();
    assert_eq!(report.counts.files, seen.len() as u64);
    f.vault()
        .agent_history()
        .unwrap()
        .backup(&[source], &BackupOptions { read_git: false, ..Default::default() })
        .unwrap();
    let saved: BTreeSet<_> = f
        .vault()
        .store()
        .read(vault_store::Db::AgentHistoryEvidence, |s| {
            s.query(
                "SELECT relative_path FROM ah_sources WHERE latest_revision_id IS NOT NULL",
                (),
                |r| Ok(r.get::<String>(0)?),
            )
        })
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(seen, saved, "reviewed vs saved file paths");
    assert_eq!(seen, expected.iter().map(|p| p.to_string()).collect());
}
#[test]
fn pi_home_without_sessions_matches_capture() {
    assert_layout(".pi/agent", agent_history::Harness::Pi, &["extensions/private.jsonl"], &[])
}
#[test]
fn deepseek_home_without_sessions_matches_capture() {
    assert_layout(".dsh", agent_history::Harness::Deepseek, &["plugin/session.jsonl"], &[])
}
#[test]
fn cowork_tasks_match_capture() {
    let yes = "local-agent-mode-sessions/account/org/local_main/.claude/tasks/task.json";
    let no = "local-agent-mode-sessions/account/org/local_main/tasks/other.json";
    assert_layout("Claude", agent_history::Harness::ClaudeCode, &[yes, no], &[yes])
}

#[test]
fn shared_agent_trigger_skips_paused_or_off_before_claiming_a_slot() {
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(true);
    grant(&f);
    for state in [KindState::Off, KindState::Paused] {
        let mut config = load_config(&f.paths()).unwrap();
        config.capture_policy.agent_history = state;
        save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
        let req=from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"shared":true,"requestId":uuid::Uuid::new_v4().to_string()})).unwrap();
        let result = backups::start(
            &f.session,
            req,
            std::sync::Arc::new(|_, _| panic!("skipped run emitted event")),
        )
        .unwrap();
        assert_eq!(result["kinds"][0]["decision"], "skipped");
        assert_eq!(
            result["kinds"][0]["reason"],
            if state == KindState::Off { "kind-off" } else { "kind-paused" }
        );
    }
}

#[test]
fn losing_the_other_on_kind_requires_an_approved_source_even_if_unplugged() {
    let f = Fixture::new(true);
    grant(&f);
    let mut config = load_config(&f.paths()).unwrap();
    config.capture_policy.selection_ref = Some("missing".into());
    save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
    let pause = || {
        kinds::set_state(&f.session,from_value(json!({"kind":"browsing","state":"paused","expectedPolicyToken":kinds::settings().unwrap().policy_token})).unwrap(),|_|{})
    };
    assert_eq!(pause().unwrap_err().to_string(), "source-selection-empty");
    config.capture_policy.selection_ref = Some("chosen".into());
    save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
    std::fs::rename(f.temp.path().join("chosen-pi"), f.temp.path().join("offline")).unwrap();
    pause().unwrap();
}

#[test]
fn move_over_agent_only_destination_requires_source_review_after_reopen() {
    let f = Fixture::new(true);
    let bundle = f.temp.path().join("before-selection.pathkeep");
    vault_worker::export_app_data(f.vault(), bundle.clone(), "move password").unwrap();
    grant(&f);
    kinds::set_state(&f.session,from_value(json!({
        "kind":"browsing","state":"paused","expectedPolicyToken":kinds::settings().unwrap().policy_token
    })).unwrap(),|_|{}).unwrap();

    let current = f.session.vault().unwrap();
    let lease = f.session.begin_maintenance().unwrap();
    let (_, installed) = vault_worker::apply_import(
        &f.paths(),
        Some(&current),
        f.session.owner(),
        bundle,
        vault_core::ApplyImportOptions {
            confirm_overwrite: true,
            source_archive_key: Some("move password".into()),
        },
    )
    .unwrap();
    f.session
        .install_archive(
            installed,
            Some("move password".into()),
            vault_worker::KeychainAfterInstall::AsSaved,
        )
        .unwrap();
    drop(lease);
    let list = to_value(kinds::list(&f.session).unwrap()).unwrap();
    assert_eq!(list["kinds"][0]["state"], "paused");
    assert_eq!(list["kinds"][1]["state"], "on");
    assert_eq!(list["kinds"][1]["hasBackupSource"], false);
    assert_eq!(list["kinds"][1]["sourceReviewRequired"], true);
    assert!(kinds::settings().unwrap().policy.selection_ref.is_none());
}
