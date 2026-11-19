//! Reconcile durable tasks, consent and source observations through the owner.
use super::*;
use vault_store::{Db, Priority};

fn setup(f: &Fixture) -> (std::path::PathBuf, Value, Value) {
    let root = f.temp.path().join("pi");
    pi(&root.join("sessions/one.jsonl"), "one", 3);
    let p = review(f, &root, "pi");
    let (task, _) = finished(f, |e| backups::enable(&f.session, enable_request(&p), e));
    (root, p, task)
}
fn overview(f: &Fixture) -> Value {
    backups::overview(&f.session, from_value(json!({"purpose":"backup"})).unwrap()).unwrap()
}
fn detail(f: &Fixture, task: &Value) -> Value {
    backups::get_run(&f.session, from_value(json!({"runRef":task["runRef"]})).unwrap()).unwrap()
}
pub(super) fn dismiss_notice(f: &Fixture, task: &Value, version: &str) -> Value {
    backups::control(&f.session,from_value(json!({"taskId":task["taskId"],"action":"dismiss-notice","notice":{"tool":"pi","version":version},
        "expectedToken":task["snapshotToken"],"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(|_,_|{})).unwrap()["task"].clone()
}

#[test]
fn format_notices_and_failed_saves_are_acknowledged_independently() {
    let f = Fixture::new(true);
    let (root, _, _) = setup(&f);
    // Two future formats plus one selected local output whose bytes are missing.
    let file = root.join("sessions/one.jsonl");
    pi(&file, "one", 1000);
    use std::io::Write;
    writeln!(std::fs::OpenOptions::new().append(true).open(file).unwrap(),"{}",json!({"type":"message","id":"m","message":{"role":"bashExecution","fullOutputPath":"missing.txt","truncated":true}})).unwrap();
    pi(&root.join("sessions/two.jsonl"), "two", 1001);
    let (task, _) = start(&f);
    assert_eq!(task["state"], "partial", "{task}");
    let remaining = dismiss_notice(&f, &task, "1000");
    assert_eq!(remaining["formatNotices"], json!([{"tool":"pi","version":"1001"}]));
    assert!(remaining["actions"].as_array().unwrap().contains(&json!("dismiss")));
    let ack =
        backups::control(&f.session, control(&remaining, "dismiss"), Arc::new(|_, _| {})).unwrap();
    assert_eq!(ack["task"]["attention"], true);
    f.session.close_vault().unwrap();
    assert_eq!(count(&f), "1");
    assert_eq!(dismiss_notice(&f, &ack["task"], "1001")["attention"], false);
}

#[test]
fn replacement_requires_a_new_review_and_cannot_expand_a_retry() {
    let f = Fixture::new(true);
    let (root, p, _) = setup(&f);
    std::fs::rename(&root, root.with_extension("old")).unwrap();
    pi(&root.join("sessions/two.jsonl"), "two", 3);
    let (failed, run) = start(&f);
    assert_eq!(run["roots"][0]["availability"], "root-changed");
    let new = review(&f, &root, "pi");
    assert_eq!(p["rows"][0]["rootId"], new["rows"][0]["rootId"]);
    let previous = p["rows"][0]["directoryToken"].as_str().unwrap();
    let replacement = new["rows"][0]["directoryToken"].as_str().unwrap();
    assert_ne!(previous, replacement);
    assert_eq!(overview(&f)["sources"][0]["directoryToken"], previous);
    kinds::update_sources(&f.session,from_value(json!({"previewId":new["previewId"],"selectedRootIds":[new["rows"][0]["rootId"]],"expectedToken":kinds::settings().unwrap().policy_token})).unwrap(),|_|{}).unwrap();
    assert_eq!(overview(&f)["sources"][0]["directoryToken"], replacement);
    assert_eq!(start(&f).0["state"], "succeeded");
    assert_eq!(
        backups::control(&f.session, control(&failed, "retry"), Arc::new(|_, _| {})).unwrap()["previewRequired"],
        true
    );
}
#[test]
fn on_device_can_repair_a_missing_descriptor_and_retry_requests_review() {
    let f = Fixture::new(true);
    let (root, _, _) = setup(&f);
    let offline = root.with_extension("offline");
    std::fs::rename(&root, &offline).unwrap();
    let (failed, _) = start(&f);
    let mut config = load_config(&f.paths()).unwrap();
    config.capture_policy.selection_ref = Some(uuid::Uuid::new_v4().to_string());
    save_config(&ControlGate::acquire(&f.paths()), &config).unwrap();
    assert_eq!(
        backups::control(&f.session, control(&failed, "retry"), Arc::new(|_, _| {})).unwrap()["previewRequired"],
        true
    );
    std::fs::rename(&offline, &root).unwrap();
    assert_eq!(
        finished(&f, |e| backups::enable(&f.session, enable_request(&review(&f, &root, "pi")), e))
            .0["state"],
        "succeeded"
    );
}
#[test]
fn confirmed_absence_survives_another_files_format_note() {
    let f = Fixture::new(true);
    let (root, _, _) = setup(&f);
    std::fs::remove_file(root.join("sessions/one.jsonl")).unwrap();
    pi(&root.join("sessions/two.jsonl"), "two", 1000);
    let (_, run) = start(&f);
    assert_eq!(run["summary"]["filesAbsent"], "1");
    assert_eq!(overview(&f)["onlyInPathkeep"], "1");
}

#[test]
fn old_interrupted_attempt_exposes_actionable_task_in_run_detail() {
    let f = Fixture::new(true);
    let (_, _, task) = setup(&f);
    f.vault()
        .store()
        .write(Db::AgentHistoryEvidence, Priority::Interactive, |s| {
            s.execute("UPDATE ah_backup_runs SET status='interrupted',finished_at=1", ())?;
            Ok(())
        })
        .unwrap();
    f.session.close_vault().unwrap();
    let row = detail(&f, &task)["task"].clone();
    assert_eq!(row["actions"], json!(["retry"]));
    assert_eq!(
        finished(&f, |e| backups::control(&f.session, control(&row, "retry"), e)).0["retryOf"],
        task["taskId"]
    );
}
#[test]
fn retrying_original_resolves_first_save_and_sibling_failure() {
    let f = Fixture::new(true);
    let root = f.temp.path().join("claude");
    std::fs::create_dir(&root).unwrap();
    let (a, _) = finished(&f, |e| {
        backups::enable(&f.session, enable_request(&review(&f, &root, "claude-code")), e)
    });
    assert_eq!(a["state"], "partial");
    let (b, _) = finished(&f, |e| backups::control(&f.session, control(&a, "retry"), e));
    assert_eq!(b["state"], "partial");
    native::claude_main(&root, "ok", &[native::claude_user("ok", "u", None, "/p", "saved")]);
    let (success, _) = finished(&f, |e| backups::control(&f.session, control(&a, "retry"), e));
    assert_eq!(success["state"], "succeeded");
    f.session.close_vault().unwrap();
    assert_eq!(overview(&f)["firstBackup"], "success");
    assert_eq!(count(&f), "0");
}

#[test]
fn terminal_write_failure_is_reconciled_without_restarting() {
    let f = Fixture::new(true);
    let (root, _, _) = setup(&f);
    pi(&root.join("sessions/next.jsonl"), "next", 3);
    f.vault()
        .store()
        .write(Db::AgentHistoryEvidence, Priority::Interactive, |s| {
            // Synthetic uniqueness fault rejects every terminal transition, including
            // both fallback reports, while raw writes and reads still work.
            for status in ["failed", "partial", "interrupted"] {
                s.execute(
                    "INSERT INTO ah_backup_runs(public_id,status,started_at) VALUES(?1,?2,1)",
                    vault_store::params![uuid::Uuid::new_v4().to_string(), status],
                )?;
            }
            s.execute("CREATE UNIQUE INDEX deny_terminal ON ah_backup_runs(status)", ())?;
            Ok(())
        })
        .unwrap();
    let dispatch=backups::start(&f.session,from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(|_,_|{})).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(12);
    loop {
        let summary = backups::task_summary(&f.session, from_value(json!({})).unwrap());
        if summary.is_err() || summary.unwrap()["runningCount"] == 0 {
            break;
        }
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    f.vault()
        .store()
        .write(Db::AgentHistoryEvidence, Priority::Interactive, |s| {
            s.execute("DROP INDEX deny_terminal", ())?;
            Ok(())
        })
        .unwrap();
    let rows =
        backups::tasks(&f.session, from_value(json!({"group":"running","limit":10})).unwrap())
            .unwrap();
    assert_eq!(rows["items"], json!([]));
    let run = backups::get_run(
        &f.session,
        from_value(json!({"runRef":dispatch["kinds"][0]["runRef"]})).unwrap(),
    )
    .unwrap();
    assert_eq!(run["result"], "partial");
    assert_eq!(run["task"]["holdsQuit"], false);
    assert_eq!(run["task"]["reason"], "terminal-publication-failed");
    assert_eq!(run["warningCount"], "1");
    assert_eq!(count(&f), "1");
}

#[test]
#[ignore = "explicit Agent page latency measurement on synthetic committed metadata"]
fn measure_agent_page_requests() {
    let f = Fixture::new(true);
    let (root, first, _) = setup(&f);
    let mut paths = vec![("pi", root.canonicalize().unwrap())];
    let mut ids = vec![first["rows"][0]["rootId"].clone()];
    let mixed = std::env::var_os("PATHKEEP_MEASURE_MIXED_ROOTS").is_some();
    if mixed {
        for (i, tool) in
            ["claude-code", "codex", "deepseek", "pi", "claude-code", "codex", "deepseek"]
                .into_iter()
                .enumerate()
        {
            let path = f.temp.path().join(format!("source-{i}"));
            std::fs::create_dir(&path).unwrap();
            let p = review(&f, &path, tool);
            ids.push(p["rows"][0]["rootId"].clone());
            kinds::update_sources(&f.session,from_value(json!({"selectedRootIds":ids,"previewId":p["previewId"],"toolChanges":{(tool):true},"expectedToken":kinds::settings().unwrap().policy_token})).unwrap(),|_|{}).unwrap();
            paths.push((tool, path.canonicalize().unwrap()));
        }
    }
    f.vault().store().write(Db::AgentHistoryEvidence,Priority::Batch,move |tx|{
        let n=(12000/paths.len()) as i64;
        for (i,(tool,path)) in paths.into_iter().enumerate(){
            tx.execute("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<?3)
                INSERT INTO ah_sources(id,harness,root,relative_path,kind,retention,observed_at)
                SELECT 'bench-'||?4||'-'||x,?2,?1,'sessions/'||x||'.jsonl','transcript','appended',1 FROM n",
                vault_store::params![path.to_string_lossy().to_string(),tool,n,i as i64])?;
        }
        tx.execute("INSERT INTO ah_revisions(source_id,byte_len,captured_at,state,codec) SELECT id,1024,1,'complete','native' FROM ah_sources WHERE id LIKE 'bench-%'",())?;
        tx.execute("UPDATE ah_sources SET latest_revision_id=(SELECT max(id) FROM ah_revisions r WHERE r.source_id=ah_sources.id) WHERE id LIKE 'bench-%'",())?;
        tx.execute("INSERT INTO ah_source_conversations(source_id,conversation_id,is_child,revision_id) SELECT id,id,0,latest_revision_id FROM ah_sources WHERE id LIKE 'bench-%'",())?;
        Ok(())
    }).unwrap();
    f.session.close_vault().unwrap();
    f.session.vault().unwrap();
    for trial in 0..4 {
        let at = std::time::Instant::now();
        let info = overview(&f);
        let header = at.elapsed();
        let sources =
            backups::sources(&f.session, from_value(json!({"limit":100})).unwrap()).unwrap();
        eprintln!(
            "Agent page trial {trial}: overview {header:?}, overview + sources {:?}",
            at.elapsed()
        );
        assert_eq!(info["roots"], "12001");
        assert!(
            sources["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["counts"]["roots"] == if mixed { "1501" } else { "12001" })
        );
    }
    f.session.close_vault().unwrap();
    f.session.vault().unwrap();
    let at = std::time::Instant::now();
    let session = &f.session;
    std::thread::scope(|scope| {
        let header = scope.spawn(|| {
            let info = backups::overview(session, from_value(json!({"purpose":"backup"})).unwrap())
                .unwrap();
            assert_eq!(info["roots"], "12001");
            at.elapsed()
        });
        let rows = scope.spawn(|| {
            let page =
                backups::sources(session, from_value(json!({"limit":100})).unwrap()).unwrap();
            assert!(!page["items"].as_array().unwrap().is_empty());
            at.elapsed()
        });
        let first = header.join().unwrap();
        let second = rows.join().unwrap();
        eprintln!(
            "Concurrent cold owner: first {:?}, both {:?}",
            first.min(second),
            first.max(second)
        );
    });
}

#[cfg(unix)]
#[test]
fn denied_reference_only_output_makes_backup_partial() {
    use std::{io::Write, os::unix::fs::PermissionsExt};
    let f = Fixture::new(true);
    let root = f.temp.path().join("pi");
    let file = root.join("sessions/test.jsonl");
    pi(&file, "r", 3);
    let output = root.join("outputs/private.txt");
    write(&output, b"unretained");
    std::fs::set_permissions(output.parent().unwrap(), std::fs::Permissions::from_mode(0o000))
        .unwrap();
    if std::fs::File::open(&output).is_ok() {
        return;
    }
    writeln!(std::fs::OpenOptions::new().append(true).open(file).unwrap(),"{}",json!({"type":"message","id":"m","message":{"role":"bashExecution","fullOutputPath":"outputs/private.txt"}})).unwrap();
    let (task, run) =
        finished(&f, |e| backups::enable(&f.session, enable_request(&review(&f, &root, "pi")), e));
    std::fs::set_permissions(output.parent().unwrap(), std::fs::Permissions::from_mode(0o700))
        .unwrap();
    assert_eq!(task["state"], "partial");
    assert_eq!(run["warnings"][0]["detail"], "authority_unavailable");
}

#[test]
fn cowork_inventory_retires_replaced_audit_membership() {
    let f = Fixture::new(true);
    let root = f.temp.path().join("cowork");
    let body = root.join("local-agent-mode-sessions/account/org/local_task");
    write(&body.join("audit.jsonl"),format!("{}\n",json!({"type":"assistant","session_id":"native-cowork","uuid":"r","message":{"role":"assistant","content":"audit"}})).as_bytes());
    finished(&f, |e| {
        backups::enable(&f.session, enable_request(&review(&f, &root, "claude-code")), e)
    });
    assert_eq!(overview(&f)["roots"], "1");
    write(&body.join(".claude/projects/p/main.jsonl"),format!("{}\n",json!({"type":"assistant","sessionId":"native-cowork","uuid":"reply","entrypoint":"local-agent","message":{"role":"assistant","content":"primary"}})).as_bytes());
    start(&f);
    assert_eq!(
        f.vault().agent_history().unwrap().list_sessions(&Default::default()).unwrap().items.len(),
        1
    );
    assert_eq!(overview(&f)["roots"], "1");
}

#[test]
fn queued_history_without_local_pending_consent_is_inspectable_and_never_dispatched() {
    let f = Fixture::new(true);
    let (root, _, task) = setup(&f);
    pi(&root.join("sessions/unrequested.jsonl"), "unrequested", 3);
    f.vault()
        .store()
        .write(Db::AgentHistoryEvidence, Priority::Interactive, |s| {
            s.execute("UPDATE ah_backup_runs SET status='queued',finished_at=NULL", ())?;
            Ok(())
        })
        .unwrap();
    let waiting = detail(&f, &task);
    assert_eq!(waiting["result"], "running");
    assert_eq!(waiting["coverage"]["state"], "building");
    assert_eq!(waiting["task"]["phase"], "queued");
    assert_eq!(waiting["task"]["actions"], json!([]));
    f.session.close_vault().unwrap();
    assert_eq!(detail(&f, &task)["result"], "interrupted");
    assert_eq!(overview(&f)["transcriptFiles"], "1");
}

#[test]
fn first_backup_belongs_to_the_active_device_grant_even_after_an_edit() {
    let f = Fixture::new(true);
    let (_, keep, _) = setup(&f);
    let extra = f.temp.path().join("extra");
    pi(&extra.join("sessions/added.jsonl"), "extra", 3);
    let add = review(&f, &extra, "pi");
    kinds::update_sources(&f.session,from_value(json!({"selectedRootIds":[keep["rows"][0]["rootId"],add["rows"][0]["rootId"]],"previewId":add["previewId"],"expectedToken":kinds::settings().unwrap().policy_token})).unwrap(),|_|{}).unwrap();
    let device = load_config(&f.paths()).unwrap();
    kinds::set_state(
        &f.session,
        kinds::SetState {
            kind: DataKind::AgentHistory,
            state: KindState::Off,
            expected_policy_token: device.capture_policy.token(),
        },
        |_| {},
    )
    .unwrap();
    let empty = f.temp.path().join("empty-claude");
    std::fs::create_dir(&empty).unwrap();
    let (other, _) = finished(&f, |e| {
        backups::enable(&f.session, enable_request(&review(&f, &empty, "claude-code")), e)
    });
    assert_eq!(other["state"], "partial");
    save_config(&ControlGate::acquire(&f.paths()), &device).unwrap();
    assert_eq!(overview(&f)["firstBackup"], "success");
}

#[test]
fn shared_inventory_tracks_commits_and_defers_the_physical_size() {
    let f = Fixture::new(true);
    let (root, _, _) = setup(&f);
    let first = overview(&f);
    assert!(first.get("archiveBytes").is_none());
    assert_eq!(first["roots"], "1");
    pi(&root.join("sessions/two.jsonl"), "two", 3);
    start(&f);
    let sources = backups::sources(&f.session, from_value(json!({"limit":10})).unwrap()).unwrap();
    assert!(sources["items"].as_array().unwrap().iter().any(|r| r["counts"]["roots"] == "2"));
    let sized = backups::overview(
        &f.session,
        from_value(json!({"purpose":"backup","includeArchiveBytes":true})).unwrap(),
    )
    .unwrap();
    assert_eq!(sized["roots"], "2");
    assert!(sized["archiveBytes"].as_str().unwrap().parse::<u64>().unwrap() > 0);
}
