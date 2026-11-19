//! Production capture and Tasks contracts against a temporary owner.
use super::*;

/// A second backup of just one Claude root must not observe the project of a
/// different root, even when both roots have previously been backed up.
#[test]
fn capture_only_revisits_folders_from_the_approved_roots() {
    use agent_history::{Harness, SourceRoot, capture::BackupOptions};

    let fixture = Fixture::new(true);
    let archive = fixture.vault().agent_history().unwrap();
    let home = fixture.temp.path();
    let roots = ["alpha", "beta"].map(|name| {
        let source = home.join(format!("claude-{name}"));
        let project = home.join(name);
        std::fs::create_dir(&project).unwrap();
        let record = native::claude_user(name, "message", None, project.to_str().unwrap(), "hello");
        native::claude_main(&source, name, &[record]);
        SourceRoot { harness: Harness::ClaudeCode, root: source }
    });
    let options = BackupOptions { read_git: false, ..Default::default() };
    archive.backup(&roots, &options).unwrap();
    for name in ["alpha", "beta"] {
        std::fs::remove_dir(home.join(name)).unwrap();
    }
    archive.backup(&roots[..1], &options).unwrap();
    let observations = |name: &str| {
        archive
            .folders(home.join(name).to_str(), None, 10)
            .unwrap()
            .items
            .into_iter()
            .map(|entry| entry.exists)
            .collect::<Vec<_>>()
    };
    assert_eq!(observations("alpha"), [false, true]);
    assert_eq!(observations("beta"), [true]);
}

/// Rollouts, revisions and children are three different inventory units. Repeating
/// a capture cannot double any counter, and adding a turn updates the same chat.
#[test]
fn committed_backup_facts_distinguish_new_updated_and_child() {
    use agent_history::{Harness, SourceRoot, capture::BackupOptions};
    use vault_store::Db;
    let fixture = Fixture::new(true);
    let root = fixture.temp.path().join(".claude");
    let session = "primary";
    let mut rows = vec![native::claude_user(session, "u", None, "/synthetic", "first turn")];
    native::claude_main(&root, session, &rows);
    native::claude_subagent(
        &root,
        session,
        "agent-helper.jsonl",
        "helper",
        "/synthetic",
        "child turn",
    );
    let archive = fixture.vault().agent_history().unwrap();
    let source = SourceRoot { harness: Harness::ClaudeCode, root: root.clone() };
    let backup = || {
        let run = archive
            .backup(
                std::slice::from_ref(&source),
                &BackupOptions { read_git: false, ..Default::default() },
            )
            .unwrap()
            .run_id;
        fixture
            .vault()
            .store()
            .read(Db::AgentHistoryEvidence, move |s| {
                s.query(
                    "SELECT outcome FROM ah_run_conversations WHERE run_id=?1 ORDER BY outcome",
                    [run],
                    |r| Ok(r.get::<String>(0)?),
                )
            })
            .unwrap()
    };
    assert_eq!(backup(), ["new-child", "new-root"]);
    assert!(backup().is_empty());
    rows.push(native::claude_reply(session, "answer", "u", "/synthetic", "finished"));
    native::claude_main(&root, session, &rows);
    assert_eq!(backup(), ["updated-root"]);
}

/// Exercise the production preview, owner slot, control policy and committed
/// prefix. A blocked progress consumer gives the test a deterministic commit
/// boundary without adding a worker or bridge command to the application.
#[test]
fn first_save_cancel_restart_and_retry_keep_original_scope() {
    use std::sync::{Arc, Condvar, Mutex, mpsc};
    use std::time::{Duration, Instant};
    use vault_store::Db;
    use vault_worker::data_kinds::backups;
    let fixture = Fixture::new(true);
    let home = fixture.temp.path().join("tools");
    let caller = fixture.session.clone();
    let vault = fixture.vault();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let project = fixture.temp.path().join("project");
    native::claude_main(
        &home.join(".claude"),
        "chat",
        &[native::claude_user("chat", "1", None, project.to_str().unwrap(), "retained")],
    );
    native::codex_rollout(
        &home.join(".codex"),
        "now",
        "roll",
        &[
            native::codex_meta("roll", project.to_str().unwrap()),
            native::codex_user("retained two"),
        ],
    );
    write(&home.join(".codex/sessions/rollout-unstable.jsonl"), br#"{"type":"session_meta","#);
    write(&home.join(".claude/uploads/chat/.git/config"), b"private git sentinel");
    write(&home.join(".pi/agent/sessions/forbidden.jsonl"), b"unselected");
    let untouched = [
        home.join(".pi/agent/sessions/forbidden.jsonl"),
        home.join(".claude/uploads/chat/.git/config"),
    ];
    let sentinel_atime = std::time::UNIX_EPOCH + Duration::from_secs(123);
    for file in &untouched {
        std::fs::OpenOptions::new()
            .write(true)
            .open(file)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_accessed(sentinel_atime))
            .unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(home.join(".pi/agent"), std::fs::Permissions::from_mode(0o000))
            .unwrap();
    }
    let before = std::env::var_os("CHB_AGENT_TOOLS_HOME");
    unsafe {
        std::env::set_var("CHB_AGENT_TOOLS_HOME", &home);
    }
    let mut frames = preview::start(
        &caller,
        from_value(json!({"purpose":"enable","tools":["claude-code","codex"]})).unwrap(),
    )
    .unwrap();
    let summary = runtime.block_on(async {
        loop {
            let f = frames.next_frame().await.unwrap();
            if f["type"] == "finished" {
                break f["payload"].clone();
            }
        }
    });
    match before {
        Some(v) => unsafe { std::env::set_var("CHB_AGENT_TOOLS_HOME", v) },
        None => unsafe { std::env::remove_var("CHB_AGENT_TOOLS_HOME") },
    }
    let roots: Vec<_> = summary["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["readable"] == true)
        .map(|r| r["rootId"].clone())
        .collect();
    assert_eq!(roots.len(), 2);
    let req = json!({"previewId":summary["previewId"],"selectedRootIds":roots,"expectedPolicyToken":kinds::settings().unwrap().policy_token,
        "requestId":uuid::Uuid::new_v4().to_string(),"readGit":false});
    let (tx, rx) = mpsc::sync_channel(1);
    let latch = Arc::new((Mutex::new(false), Condvar::new()));
    let release = latch.clone();
    let events: backups::Events = Arc::new(move |name, payload| {
        if name == "pathkeep://kind-backup-progress" && payload["unit"] == "bytes" {
            tx.try_send(()).ok();
            let (held, _) = release
                .1
                .wait_timeout_while(release.0.lock().unwrap(), Duration::from_secs(10), |go| !*go)
                .unwrap();
            assert!(*held);
        }
    });
    let quiet: backups::Events = Arc::new(|_, _| {});
    let receipt = backups::enable(&caller, from_value(req.clone()).unwrap(), events).unwrap();
    drop(frames);
    drop(caller);
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(kinds::settings().unwrap().policy.agent_history, KindState::On);
    let count = |db: &vault_core::Vault| {
        db.store()
            .read(Db::AgentHistoryEvidence, |s| {
                s.query_one("SELECT COUNT(*) FROM ah_revisions WHERE state='complete'", (), |r| {
                    Ok(r.get::<i64>(0)?)
                })
            })
            .unwrap()
    };
    assert_eq!(count(vault), 1);
    assert_eq!(
        backups::enable(&fixture.session, from_value(req).unwrap(), quiet.clone()).unwrap(),
        receipt
    );
    let busy_request = json!({"scope":{"type":"tool","kind":"agent-history","tool":"codex"},"requestId":uuid::Uuid::new_v4().to_string()});
    let busy =
        backups::start(&fixture.session, from_value(busy_request.clone()).unwrap(), quiet.clone())
            .unwrap();
    assert_eq!(busy["kinds"][0]["taskId"], receipt["firstSave"]["taskId"]);
    assert_eq!(busy["kinds"][0]["decision"], "already-running");
    let task = backups::tasks(
        &fixture.session,
        from_value(json!({"group":"running","limit":10})).unwrap(),
    )
    .unwrap()["items"][0]
        .clone();
    assert!(task["progress"]["total"].is_null());
    let control = |row: &serde_json::Value, action: &str| {
        from_value(json!({"taskId":row["taskId"],"action":action,"expectedToken":row["snapshotToken"],"requestId":uuid::Uuid::new_v4().to_string()})).unwrap()
    };
    assert_eq!(
        backups::control(&fixture.session, control(&task, "cancel"), quiet.clone()).unwrap()["task"]
            ["state"],
        "cancelling"
    );
    *latch.0.lock().unwrap() = true;
    latch.1.notify_one();
    fixture.session.close_vault().unwrap();
    let reopened = fixture.session.vault().unwrap();
    assert_eq!(count(&reopened), 1);
    let today = || {
        backups::tasks(&fixture.session, from_value(json!({"group":"today","limit":10})).unwrap())
            .unwrap()["items"]
            .clone()
    };
    let old = today()[0].clone();
    assert_eq!(old["state"], "interrupted");
    assert_eq!(
        backups::start(&fixture.session, from_value(busy_request).unwrap(), quiet.clone()).unwrap(),
        busy
    );
    let retried =
        backups::control(&fixture.session, control(&old, "retry"), quiet.clone()).unwrap();
    assert_ne!(retried["replacementTaskId"], old["taskId"]);
    let start = Instant::now();
    loop {
        if today().as_array().unwrap().len() == 2 {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(count(&reopened), 3);
    let receipts = reopened
        .store()
        .read(Db::AgentHistoryEvidence, |tx| {
            tx.query_one(
        "SELECT (SELECT COUNT(*) FROM ah_backup_runs),(SELECT COUNT(*) FROM ah_desktop_backups)",(),
        |r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?)))
        })
        .unwrap();
    assert_eq!(receipts, (2, 2), "enable, busy and cancel replays must not allocate runs");
    let retry_row = today()[0].clone();
    assert_eq!(retry_row["state"], "partial");
    let retry_detail = backups::get_run(
        &fixture.session,
        from_value(json!({"runRef":retry_row["runRef"]})).unwrap(),
    )
    .unwrap();
    assert_eq!(retry_detail["summary"]["unstable"], "1", "{retry_detail}");
    let rows = reopened
        .store()
        .read(Db::AgentHistoryEvidence, |s| {
            s.query("SELECT DISTINCT harness FROM ah_sources ORDER BY harness", (), |r| {
                Ok(r.get::<String>(0)?)
            })
        })
        .unwrap();
    assert_eq!(rows, ["claude-code", "codex"]);
    let digests=reopened.store().read(Db::AgentHistoryEvidence,|s|s.query("SELECT s.root,s.relative_path,r.digest FROM ah_sources s JOIN ah_revisions r ON s.latest_revision_id=r.id",(),|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?)))).unwrap();
    for (root, relative, digest) in digests {
        assert_eq!(
            digest,
            vault_core::utils::sha256_hex(
                &std::fs::read(std::path::Path::new(&root).join(relative)).unwrap()
            )
        );
    }
    kinds::update_sources(&fixture.session,from_value(json!({"toolChanges":{"codex":false},"expectedToken":kinds::settings().unwrap().policy_token})).unwrap(),|_|{}).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(home.join(".pi/agent"), std::fs::Permissions::from_mode(0o700))
            .unwrap();
    }
    for file in &untouched {
        assert_eq!(std::fs::metadata(file).unwrap().accessed().unwrap(), sentinel_atime);
    }
    let denied = backups::control(&fixture.session, control(&old, "retry"), quiet).unwrap();
    assert_eq!(denied["previewRequired"], true);
}

/// Losing every selected source refuses activation. A future Pi schema retains
/// raw bytes and raises durable attention. Correcting the source and retrying
/// clears that attempt's attention across restart.
#[test]
fn source_loss_partial_attention_and_successful_retry() {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(true);
    let folder = f.temp.path().join("agent");
    let file = folder.join("sessions/project/one.jsonl");
    let pi = |version: u64| {
        format!(
            "{{\"type\":\"session\",\"version\":{version},\"id\":\"one\",\"timestamp\":\"2026-11-10T12:00:00Z\",\"cwd\":\"/synthetic\"}}\n"
        )
    };
    write(&file, pi(1000).as_bytes());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let request = || {
        let mut frames = preview::start(
            &f.session,
            from_value(json!({"purpose":"enable","pickedFolder":{"tool":"pi","path":folder}}))
                .unwrap(),
        )
        .unwrap();
        let inventory = runtime.block_on(async {
            loop {
                let frame = frames.next_frame().await.unwrap();
                if frame["type"] == "finished" {
                    break frame["payload"].clone();
                }
            }
        });
        from_value(json!({"requestId":uuid::Uuid::new_v4().to_string(),"previewId":inventory["previewId"],"selectedRootIds":[inventory["rows"][0]["rootId"]],"readGit":false,
            "expectedPolicyToken":kinds::settings().unwrap().policy_token})).unwrap()
    };
    let lost = request();
    let other = folder.with_extension("offline");
    std::fs::rename(&folder, &other).unwrap();
    assert!(backups::enable(&f.session, lost, Arc::new(|_, _| {})).is_err());
    assert_eq!(kinds::settings().unwrap().policy.agent_history, KindState::Off);
    std::fs::rename(&other, &folder).unwrap();
    let receipt = backups::enable(&f.session, request(), Arc::new(|_, _| {})).unwrap();
    let wait = |id: &serde_json::Value| {
        let start = Instant::now();
        loop {
            let rows = backups::tasks(
                &f.session,
                from_value(json!({"group":"today","limit":20})).unwrap(),
            )
            .unwrap();
            if let Some(row) = rows["items"].as_array().unwrap().iter().find(|r| r["taskId"] == *id)
            {
                return row.clone();
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let first = wait(&receipt["firstSave"]["taskId"]);
    assert_eq!(first["state"], "partial");
    assert_eq!(first["attention"], true);
    let sources = backups::sources(&f.session, from_value(json!({"limit":20})).unwrap()).unwrap();
    let source = sources["items"].as_array().unwrap().iter().find(|r| r["tool"] == "pi").unwrap();
    assert_eq!(source["availability"], "unknown-format", "{sources}");
    let overview =
        backups::overview(&f.session, from_value(json!({"purpose":"backup"})).unwrap()).unwrap();
    assert_eq!(overview["firstBackup"], "partial");
    let detail =
        backups::get_run(&f.session, from_value(json!({"runRef":first["runRef"]})).unwrap())
            .unwrap();
    assert_eq!(detail["readGit"], false);
    assert_eq!(detail["summary"]["filesVisited"], "1");
    assert_eq!(detail["summary"]["unknownFormat"], "1");
    assert_eq!(detail["roots"][0]["availability"], "unknown-format");
    assert_eq!(detail["perTool"][0]["availability"], "unknown-format");
    assert_eq!(detail["roots"][0]["counts"]["filesCaptured"], "1");
    assert_eq!(
        overview["perTool"].as_array().unwrap().iter().find(|r| r["tool"] == "pi").unwrap()["sourceBytes"],
        pi(1000).len().to_string()
    );
    let attention = || {
        backups::task_summary(&f.session, from_value(json!({})).unwrap()).unwrap()["attentionCount"]
            .clone()
    };
    f.session.close_vault().unwrap();
    assert_eq!(attention(), "1");
    let added = f.temp.path().join("another-pi");
    write(&added.join("sessions/new.jsonl"), pi(3).replace("one", "two").as_bytes());
    let mut frames = preview::start(
        &f.session,
        from_value(json!({"purpose":"rescan","pickedFolder":{"tool":"pi","path":added}})).unwrap(),
    )
    .unwrap();
    let review = runtime.block_on(async {
        loop {
            let frame = frames.next_frame().await.unwrap();
            if frame["type"] == "finished" {
                break frame["payload"].clone();
            }
        }
    });
    let patch = json!({"selectedRootIds":[source["rootId"],review["rows"][0]["rootId"]],"expectedToken":kinds::settings().unwrap().policy_token});
    let refused =
        kinds::update_sources(&f.session, from_value(patch.clone()).unwrap(), |_| {}).unwrap_err();
    assert_eq!(refused.downcast::<Refusal>().unwrap().code(), "enable-preview-required");
    let mut reviewed = patch;
    reviewed["previewId"] = review["previewId"].clone();
    kinds::update_sources(&f.session, from_value(reviewed).unwrap(), |_| {}).unwrap();
    write(&file, pi(3).as_bytes());
    let retry=backups::control(&f.session,from_value(json!({"taskId":first["taskId"],"action":"retry","expectedToken":first["snapshotToken"],"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(|_,_|{})).unwrap();
    assert_eq!(wait(&retry["replacementTaskId"])["state"], "succeeded");
    assert_eq!(attention(), "0");
    let roots = f
        .session
        .vault()
        .unwrap()
        .store()
        .read(vault_store::Db::AgentHistoryEvidence, |s| {
            s.query("SELECT DISTINCT root FROM ah_sources", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    assert_eq!(roots, [folder.canonicalize().unwrap().to_str().unwrap()]);
    let save = || {
        let d=backups::start(&f.session,from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(|_,_|{})).unwrap();
        let row = wait(&d["kinds"][0]["taskId"]);
        backups::get_run(&f.session, from_value(json!({"runRef":row["runRef"]})).unwrap()).unwrap()
    };
    std::fs::remove_file(&file).unwrap();
    let absent = save();
    assert_eq!(absent["summary"]["filesAbsent"], "1");
    assert_eq!(absent["result"], "success");
    std::fs::rename(&folder, folder.with_extension("offline-again")).unwrap();
    let offline = save();
    assert_eq!(offline["result"], "partial");
    assert_eq!(offline["summary"]["filesAbsent"], "0", "an inaccessible root cannot prove absence");
    assert_eq!(offline["roots"][0]["availability"], "root-missing");
}

/// Stop a large first save at a known raw commit. Pausing and turning off each
/// return the affected task. Locking the window keeps the authorized save alive.
#[test]
fn pause_off_and_app_lock_during_first_save() {
    use std::{
        sync::{Arc, Condvar, Mutex, mpsc},
        time::{Duration, Instant},
    };
    use vault_worker::data_kinds::backups;
    for action in ["paused", "off", "lock"] {
        let f = Fixture::new(true);
        let root = f.temp.path().join("claude");
        for id in ["a", "b"] {
            native::claude_main(
                &root,
                id,
                &[native::claude_user(id, "u", None, "/synthetic", "turn")],
            );
        }
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut frames = preview::start(
            &f.session,
            from_value(
                json!({"purpose":"enable","pickedFolder":{"tool":"claude-code","path":root}}),
            )
            .unwrap(),
        )
        .unwrap();
        let inventory = rt.block_on(async {
            loop {
                let frame = frames.next_frame().await.unwrap();
                if frame["type"] == "finished" {
                    break frame["payload"].clone();
                }
            }
        });
        let latch = Arc::new((Mutex::new(false), Condvar::new()));
        let release = latch.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let events = Arc::new(move |name: &'static str, v: serde_json::Value| {
            if name == "pathkeep://kind-backup-progress" && v["unit"] == "bytes" {
                tx.try_send(()).ok();
                let (go, _) = release
                    .1
                    .wait_timeout_while(release.0.lock().unwrap(), Duration::from_secs(10), |v| !*v)
                    .unwrap();
                assert!(*go);
            }
        });
        let enable=backups::enable(&f.session,from_value(json!({"requestId":uuid::Uuid::new_v4().to_string(),"previewId":inventory["previewId"],
            "selectedRootIds":[inventory["rows"][0]["rootId"]],"expectedPolicyToken":kinds::settings().unwrap().policy_token,"readGit":false})).unwrap(),events).unwrap();
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let read = || {
            backups::get_run(
                &f.session,
                from_value(json!({"runRef":enable["firstSave"]["runRef"]})).unwrap(),
            )
        };
        if action == "lock" {
            let mut config = load_config(&f.paths()).unwrap();
            config.app_lock.enabled = true;
            save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
            vault_core::lock_app_session(&f.paths(), &config, Some("manual")).unwrap();
            assert_eq!(read().unwrap_err().downcast::<Refusal>().unwrap().code(), "lock-required");
        } else {
            let change=kinds::set_state(&f.session,from_value(json!({"kind":"agent-history","state":action,"expectedPolicyToken":enable["policyToken"]})).unwrap(),|_|{}).unwrap();
            assert_eq!(change.affected_tasks, [enable["firstSave"]["taskId"].as_str().unwrap()]);
            if action == "off" {
                assert_eq!(read().unwrap_err().downcast::<Refusal>().unwrap().code(), "kind-off");
            } else {
                assert_eq!(read().unwrap()["result"], "running");
            }
        }
        *latch.0.lock().unwrap() = true;
        latch.1.notify_one();
        let start = Instant::now();
        loop {
            let (state,heads)=f.vault().store().read(vault_store::Db::AgentHistoryEvidence,|s|s.query_one(
                "SELECT status,(SELECT COUNT(*) FROM ah_sources WHERE latest_revision_id IS NOT NULL) FROM ah_backup_runs",(),|r|Ok((r.get::<String>(0)?,r.get::<i64>(1)?)))).unwrap();
            if state != "running" {
                assert_eq!(
                    (state.as_str(), heads),
                    if action == "lock" { ("complete", 2) } else { ("interrupted", 1) }
                );
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(10));
        }
        f.session.close_vault().unwrap();
    }
}

/// A backup page includes the archive doctor alongside conversation runs. A
/// legacy numeric link still opens the browsing run with that number, while
/// MCP audit records stay private to their existing view.
#[test]
fn merged_runs_page_preserves_browsing_links_and_scope() {
    use vault_store::{Db, Priority};
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(true);
    let mut config = load_config(&f.paths()).unwrap();
    config.capture_policy.agent_history = KindState::On;
    save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
    f.vault().store().write(Db::Archive,Priority::Interactive,|s|{
        for (id,kind) in [(1,"backup"),(2,"doctor"),(3,"mcp_query")]{
            s.execute("INSERT INTO runs(id,run_type,started_at,trigger,status) VALUES(?1,?2,'2026-01-01T00:00:00Z','manual','success')",vault_store::params![id,kind])?;
        }
        Ok(())
    }).unwrap();
    let native = f.vault().agent_history().unwrap();
    let id = native.start_backup_run().unwrap();
    native.end_backup_run(id, "complete", &json!({"captured":2,"bytes":80,"unchanged":0})).unwrap();
    let list = |cursor: Option<&str>| {
        backups::list_runs(&f.session,from_value(json!({"space":"agent-history","includeArchiveWide":true,"limit":1,"cursor":cursor})).unwrap()).unwrap()
    };
    let first = list(None);
    assert_eq!(first["items"][0]["scope"], "agent-history");
    assert!(first["items"][0]["summary"]["newRoots"].is_null());
    let after = first["nextCursor"].as_str().expect("doctor is on the next page");
    let second = list(Some(after));
    assert_eq!(second["items"][0]["operation"], "doctor");
    assert_eq!(second["items"][0]["scope"], "archive");
    assert!(second.get("nextCursor").is_none());
    let detail = |reference: &str| {
        backups::get_run(&f.session, from_value(json!({"runRef":reference})).unwrap())
    };
    assert_eq!(detail("1").unwrap()["run"]["runType"], "backup");
    assert_eq!(
        detail(second["items"][0]["runRef"].as_str().unwrap()).unwrap()["runRef"],
        second["items"][0]["runRef"]
    );
    assert!(detail("3").is_err());
    assert!(
        detail(first["items"][0]["runRef"].as_str().unwrap()).unwrap()["summary"]["newChildren"]
            .is_null()
    );
}

/// Replace the reviewed directory after the worker has discovered a filename.
/// An identical name in the new directory must never become a captured head.
#[test]
fn replacement_at_capture_cannot_supply_bytes_for_an_approved_root() {
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(true);
    let source = f.temp.path().join("claude");
    let file = native::claude_main(
        &source,
        "s",
        &[native::claude_user("s", "u", None, "/synthetic", "original")],
    );
    let relative = file.strip_prefix(&source).unwrap().to_owned();
    let replacement = f.temp.path().join("other");
    write(&replacement.join(&relative), b"not approved");
    write(&replacement.join("uploads/other/sensitive-name"), b"outside");
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(123);
    std::fs::OpenOptions::new()
        .write(true)
        .open(replacement.join(&relative))
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_accessed(old))
        .unwrap();
    let mut frames = preview::start(
        &f.session,
        from_value(json!({"purpose":"enable","pickedFolder":{"tool":"claude-code","path":source}}))
            .unwrap(),
    )
    .unwrap();
    let p = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(
        async {
            loop {
                let frame = frames.next_frame().await.unwrap();
                if frame["type"] == "finished" {
                    break frame["payload"].clone();
                }
            }
        },
    );
    let swap = source.clone();
    let decoy_file = replacement.join(&relative);
    let (tx, rx) = std::sync::mpsc::channel();
    let events = std::sync::Arc::new(move |name: &'static str, v: serde_json::Value| {
        if name == "pathkeep://kind-backup-progress"
            && v["phase"] == "discovery"
            && replacement.exists()
        {
            match std::fs::rename(&swap, swap.with_extension("original")) {
                Ok(()) => std::fs::rename(&replacement, &swap).unwrap(),
                Err(e) if cfg!(windows) && matches!(e.raw_os_error(), Some(5 | 32 | 33)) => {}
                Err(e) => panic!("replacing test directory: {e}"),
            }
        }
        if name == "pathkeep://kind-backup-finished" {
            tx.send(v).unwrap();
        }
    });
    let req=from_value(json!({"requestId":uuid::Uuid::new_v4().to_string(),"previewId":p["previewId"],"selectedRootIds":[p["rows"][0]["rootId"]],
        "expectedPolicyToken":kinds::settings().unwrap().policy_token,"readGit":false})).unwrap();
    backups::enable(&f.session, req, events).unwrap();
    let done = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
    if cfg!(windows) && decoy_file.exists() {
        assert_eq!(done["result"], "success");
        assert_eq!(done["summary"]["filesCaptured"], "1");
        assert_eq!(std::fs::metadata(decoy_file).unwrap().accessed().unwrap(), old);
    } else {
        assert_eq!(done["result"], "partial");
        assert!(!decoy_file.exists(), "the source must have been exchanged");
        assert_eq!(done["summary"]["filesCaptured"], "0");
        assert_eq!(std::fs::metadata(source.join(relative)).unwrap().accessed().unwrap(), old);
    }
    let leaked = f
        .vault()
        .store()
        .read(vault_store::Db::AgentHistoryEvidence, |tx| {
            tx.query_one(
                "SELECT COUNT(*) FROM ah_sources WHERE relative_path LIKE '%sensitive-name'",
                (),
                |r| Ok(r.get::<i64>(0)?),
            )
        })
        .unwrap();
    assert_eq!(leaked, 0, "discovery continued into a replacement directory");
}

/// Development archives have native revisions and published conversations but
/// no desktop inventory yet. Rediscovering one unchanged does not make it new.
#[test]
fn unchanged_legacy_capture_does_not_create_a_new_conversation() {
    use agent_history::{Harness, SourceRoot, capture::BackupOptions};
    use vault_store::{Db, Priority};
    let f = Fixture::new(true);
    let source = f.temp.path().join(".claude");
    native::claude_main(
        &source,
        "old",
        &[native::claude_user("old", "a", None, "/synthetic", "existing")],
    );
    let roots = [SourceRoot { root: source, harness: Harness::ClaudeCode }];
    let archive = f.vault().agent_history().unwrap();
    let options = BackupOptions { read_git: false, ..Default::default() };
    archive.backup(&roots, &options).unwrap();
    f.vault()
        .store()
        .write(Db::AgentHistoryEvidence, Priority::Interactive, |tx| {
            tx.execute("DELETE FROM ah_source_conversations", ())?;
            Ok(())
        })
        .unwrap();
    let run = archive.backup(&roots, &options).unwrap().run_id;
    let counts=f.vault().store().read(Db::AgentHistoryEvidence,move |tx|tx.query_one(
        "SELECT (SELECT COUNT(*) FROM ah_source_conversations),(SELECT COUNT(*) FROM ah_run_conversations WHERE run_id=?1)",[run],
        |r|Ok((r.get::<i64>(0)?,r.get::<i64>(1)?)))).unwrap();
    assert_eq!(counts, (1, 0));
}

/// A transient destination refusal after policy publication must be recoverable
/// by the same request on the live owner. The unique index injects a real run
/// insertion failure without failing reads, consent publication or the preview.
#[test]
fn resending_authorized_first_save_recovers_a_live_insert_failure() {
    use std::sync::Arc;
    use vault_store::{Db, Priority};
    use vault_worker::data_kinds::backups;
    let f = Fixture::new(true);
    let root = f.temp.path().join("claude");
    native::claude_main(&root, "s", &[native::claude_user("s", "u", None, "/synthetic", "turn")]);
    let mut frames = preview::start(
        &f.session,
        from_value(json!({"purpose":"enable","pickedFolder":{"tool":"claude-code","path":root}}))
            .unwrap(),
    )
    .unwrap();
    let p = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(
        async {
            loop {
                let frame = frames.next_frame().await.unwrap();
                if frame["type"] == "finished" {
                    break frame["payload"].clone();
                }
            }
        },
    );
    let archive = f.vault().agent_history().unwrap();
    let previous = archive.start_backup_run().unwrap();
    archive.end_backup_run(previous, "complete", &json!({})).unwrap();
    let sql = |statement: &'static str| {
        f.vault()
            .store()
            .write(Db::AgentHistoryEvidence, Priority::Interactive, move |tx| {
                tx.execute(statement, ())?;
                Ok(())
            })
            .unwrap()
    };
    sql("CREATE UNIQUE INDEX refuse_next_test_run ON ah_backup_runs((1))");
    let id = uuid::Uuid::new_v4().to_string();
    let req = json!({"requestId":id,"previewId":p["previewId"],"selectedRootIds":[p["rows"][0]["rootId"]],
        "expectedPolicyToken":kinds::settings().unwrap().policy_token,"readGit":false});
    assert!(
        backups::enable(&f.session, from_value(req.clone()).unwrap(), Arc::new(|_, _| {})).is_err()
    );
    let policy = kinds::settings().unwrap().policy;
    assert_eq!(policy.agent_history, KindState::On);
    assert_eq!(policy.pending_first_save.as_deref(), Some(id.as_str()));
    sql("DROP INDEX refuse_next_test_run");
    let recovered =
        backups::enable(&f.session, from_value(req).unwrap(), Arc::new(|_, _| {})).unwrap();
    assert_eq!(recovered["firstSave"]["requestId"], id);
    assert!(kinds::settings().unwrap().policy.pending_first_save.is_none());
    let start = std::time::Instant::now();
    loop {
        let row = backups::get_run(
            &f.session,
            from_value(json!({"runRef":recovered["firstSave"]["runRef"]})).unwrap(),
        )
        .unwrap();
        if row["result"] != "running" {
            assert_eq!(row["result"], "success");
            break;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
