//! Exercise setup before a store exists, committed task invalidations, and a cancelled
//! window export with real owner admission, native task ledgers and a saved Pi transcript.
use super::*;
use serde_json::Value;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use vault_store::{Db, Priority, params};

#[test]
fn onboarding_draft_survives_reload_and_is_consumed_without_granting_a_root() {
    let f = Fixture::new(false);
    let empty = json!({"kinds":[],"agentTools":[],"readGit":true});
    kinds::save_onboarding_choices(from_value(empty.clone()).unwrap()).unwrap();
    let previous = load_config(&f.paths()).unwrap();
    assert!(
        vault_worker::initialize_archive_database_with_progress(
            &f.session,
            &previous,
            None,
            |_| {}
        )
        .is_err()
    );
    assert_eq!(to_value(load_config(&f.paths()).unwrap().onboarding_choices).unwrap(), empty);
    let chosen = json!({"kinds":["agent-history"],"agentTools":["pi"],"readGit":false});
    let draft = kinds::save_onboarding_choices(from_value(chosen.clone()).unwrap()).unwrap();
    assert_eq!(to_value(draft).unwrap(), chosen);
    assert!(!f.paths().archive_db.exists());
    let snapshot = vault_worker::app_snapshot(None).unwrap();
    assert_eq!(to_value(&snapshot.config.onboarding_choices).unwrap(), chosen);
    let snapshot = vault_worker::initialize_archive_database_with_progress(
        &f.session,
        &snapshot.config,
        None,
        |_| {},
    )
    .unwrap();
    let cfg = snapshot.config;
    assert!(cfg.initialized && cfg.onboarding_choices.is_none());
    assert_eq!(cfg.capture_policy.browsing, KindState::Off);
    assert_eq!(cfg.capture_policy.agent_history, KindState::On);
    assert!(!cfg.capture_policy.read_git && cfg.capture_policy.selection_ref.is_none());
    assert_eq!(to_value(&cfg.capture_policy.enabled_tools).unwrap(), json!(["pi"]));
    assert_eq!(
        kinds::save_onboarding_choices(from_value(chosen).unwrap()).unwrap_err().to_string(),
        "archive-already-initialized"
    );
}

fn task_page(f: &Fixture, kind: &str, group: &str) -> Value {
    kinds::backups::tasks(
        &f.session,
        from_value(json!({"kind":kind,"group":group,"limit":100})).unwrap(),
    )
    .unwrap()
}
fn summary(f: &Fixture) -> Value {
    kinds::backups::task_summary(&f.session, from_value(json!({})).unwrap()).unwrap()
}
#[test]
fn committed_runs_and_queue_jobs_push_counts_and_keep_their_dismissed_history() {
    use std::sync::mpsc;
    let f = Fixture::new(true);
    let (tx, rx) = mpsc::channel();
    kinds::backups::register_recovery_events(
        &f.session,
        Arc::new(move |name, value| {
            if name == "pathkeep://background-tasks-changed" {
                tx.send(value).unwrap();
            }
        }),
    );
    let store = f.vault().store();
    let now = chrono::Utc::now().to_rfc3339();
    let stamp = now.clone();
    store.write(Db::Archive,Priority::Interactive,move|s|{
        for(id,op)in [(701,"backup"),(702,"doctor"),(703,"rekey"),(704,"restore"),(705,"retention_prune"),(706,"archive_restore")]{
            s.execute("INSERT INTO runs(id,run_type,trigger,status,started_at) VALUES(?1,?2,'manual','running',?3)",params![id,op,stamp.as_str()])?;
        }
        Ok(())
    }).unwrap();
    let changed = || rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(changed()["kind"], "browsing");
    assert_eq!(summary(&f)["runningCount"], 6);
    let wide = task_page(&f, "archive", "running");
    assert_eq!(wide["items"].as_array().unwrap().len(), 5);
    assert!(wide["items"].as_array().unwrap().iter().all(|v| v["kind"] == "archive"));
    let before = summary(&f)["attentionCount"].as_str().unwrap().parse::<usize>().unwrap();
    let aborted = store.write(Db::Archive, Priority::Interactive, |s| -> vault_store::Result<()> {
        s.execute("UPDATE runs SET status='failed' WHERE id=701", ())?;
        Err(vault_store::Error::Closed)
    });
    assert!(aborted.is_err());
    assert!(rx.try_recv().is_err());
    let time = now.clone();
    store
        .write(Db::Archive, Priority::Interactive, move |s| {
            s.execute(
                "UPDATE runs SET status='partial',finished_at=?1 WHERE id BETWEEN 701 AND 706",
                [time],
            )?;
            Ok(())
        })
        .unwrap();
    changed();
    let stamp = now.clone();
    store.write(Db::Intelligence,Priority::Interactive,move|s|{
        s.execute("INSERT INTO ai_jobs(id,job_type,state,priority,attempt,max_attempts,payload_json,available_at,created_at,updated_at) VALUES(808,'index-build','running',1,1,2,'{}',?1,?1,?1)",[stamp.as_str()])?;
        s.execute("INSERT INTO intelligence_jobs(id,job_type,state,dedupe_key,payload_json,created_at,scheduled_at,updated_at,list_order)
        VALUES(809,'rebuild','failed','fixture','{}',?1,?1,?1,809)",[stamp])?;Ok(())
    }).unwrap();
    changed();
    assert_eq!(summary(&f)["runningCount"], 1);
    assert_eq!(summary(&f)["attentionCount"], (before + 7).to_string());
    store
        .write(Db::Intelligence, Priority::Interactive, move |s| {
            s.prepare("UPDATE ai_jobs SET state='failed',finished_at=?1 WHERE id=808")?
                .execute([now])?;
            Ok(())
        })
        .unwrap();
    changed();
    for id in
        ["browsing:run/701", "archive:run/702", "browsing:index/808", "browsing:intelligence/809"]
    {
        let attention = task_page(&f, "browsing", "attention");
        let row =
            attention["items"].as_array().unwrap().iter().find(|r| r["taskId"] == id).unwrap();
        assert!(row["commands"].as_array().unwrap().contains(&json!("dismiss")));
        let reply = kinds::backups::control(
            &f.session,
            from_value(json!({"taskId":id,"action":"dismiss"})).unwrap(),
            Arc::new(|_, _| {}),
        )
        .unwrap();
        assert_eq!(reply["dismissed"], true);
        changed();
        assert!(
            !task_page(&f, "browsing", "attention")["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["taskId"] == id)
        );
    }
    assert_eq!(summary(&f)["attentionCount"], (before + 4).to_string());
    let today = task_page(&f, "browsing", "today");
    let kept = today["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["taskId"] == "browsing:run/701")
        .unwrap();
    assert_eq!(kept["state"], "partial");
    assert_eq!(kept["attention"], false);
    store
        .write(Db::Intelligence, Priority::Interactive, |s| {
            s.execute("UPDATE ai_jobs SET attempt=attempt+1 WHERE id=808", ())?;
            Ok(())
        })
        .unwrap();
    changed();
    assert!(
        task_page(&f, "browsing", "attention")["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["taskId"] == "browsing:index/808")
    );
    let mut cfg = load_config(&f.paths()).unwrap();
    cfg.capture_policy.browsing = KindState::Off;
    cfg.capture_policy.agent_history = KindState::On;
    save_config(&ControlGate::acquire(&f.paths()), &cfg).unwrap();
    assert!(
        task_page(&f, "browsing", "attention")["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["kind"] == "archive")
    );
    assert_eq!(summary(&f)["attentionCount"], "4");
}

#[test]
fn progress_can_cancel_a_streamed_export_and_preserves_the_previous_destination() {
    let f = Fixture::new(true);
    let source = f.temp.path().join("pi");
    let file = source.join("sessions/one.jsonl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file,format!("{}\n{}\n",json!({"type":"session","version":3,"id":"export","cwd":"/test"}),
        json!({"type":"message","id":"u","message":{"role":"user","content":"text".repeat(200000)}}))).unwrap();
    let archive = f.vault().agent_history().unwrap();
    archive
        .backup(
            &[agent_history::SourceRoot { harness: agent_history::Harness::Pi, root: source }],
            &Default::default(),
        )
        .unwrap();
    let id = archive.chat_list(&Default::default()).unwrap()["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let folder = f.temp.path().join("exports");
    std::fs::create_dir(&folder).unwrap();
    let dest = folder.join("conversation.tar");
    for existing in [false, true] {
        if existing {
            std::fs::write(&dest, b"previous").unwrap();
        }
        let token = uuid::Uuid::new_v4();
        let events = Arc::new(Mutex::new(Vec::new()));
        let seen = events.clone();
        let job = kinds::exports::Job::begin(
            &f.session,
            &id,
            Some(token),
            Arc::new(move |event, v| {
                assert_eq!(event, "pathkeep://agent-export-progress");
                assert_eq!(v["exportId"], token.to_string());
                let bytes = v["bytesWritten"].as_str().unwrap().parse::<u64>().unwrap();
                seen.lock().unwrap().push(bytes);
                if bytes > 0 {
                    assert_eq!(
                        kinds::exports::cancel(from_value(json!({"exportId":token})).unwrap())
                            .unwrap()["cancelled"],
                        true
                    );
                }
            }),
        )
        .unwrap();
        let q = from_value(json!({"id":id,"format":"originals"})).unwrap();
        assert_eq!(job.write(&f.session, q, &dest).unwrap_err().to_string(), "cancelled");
        assert_eq!(events.lock().unwrap()[0], 0);
        if existing {
            assert_eq!(std::fs::read(&dest).unwrap(), b"previous")
        } else {
            assert!(!dest.exists())
        }
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), usize::from(existing));
    }
    let progress = Arc::new(Mutex::new(Vec::new()));
    let received = progress.clone();
    let job = kinds::exports::Job::begin(
        &f.session,
        &id,
        None,
        Arc::new(move |_, p| received.lock().unwrap().push(p)),
    )
    .unwrap();
    let done = job
        .write(&f.session, from_value(json!({"id":id,"format":"originals"})).unwrap(), &dest)
        .unwrap();
    let progress = progress.lock().unwrap();
    assert_eq!(progress.last().unwrap()["totalBytes"], done["bytes"]);
    assert_eq!(done["bytes"], dest.metadata().unwrap().len().to_string());
    assert!(progress.iter().all(|p| p["exportId"] == done["exportId"]));
    let bytes = progress
        .iter()
        .map(|p| p["bytesWritten"].as_str().unwrap().parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    assert!(bytes.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
#[cfg(debug_assertions)]
fn debug_index_pacing_holds_the_real_index_phase() {
    use std::time::Instant;
    let f = Fixture::new(true);
    let source = f.temp.path().join("pi");
    std::fs::create_dir_all(source.join("sessions")).unwrap();
    std::fs::write(
        source.join("sessions/one.jsonl"),
        b"{\"type\":\"session\",\"version\":3,\"id\":\"paced\"}\n",
    )
    .unwrap();
    let times = Arc::new(Mutex::new(Vec::new()));
    let observed = times.clone();
    let prev = std::env::var_os("PATHKEEP_TEST_AGENT_INDEX_DELAY_US");
    unsafe {
        std::env::set_var("PATHKEEP_TEST_AGENT_INDEX_DELAY_US", "250000");
    }
    let run = f.vault().agent_history().unwrap().backup(
        &[agent_history::SourceRoot { harness: agent_history::Harness::Pi, root: source }],
        &agent_history::capture::BackupOptions {
            progress: Some(Arc::new(move |p| {
                if matches!(p.phase.as_str(), "index" | "finishing") {
                    observed.lock().unwrap().push((p.phase, Instant::now()));
                }
            })),
            ..Default::default()
        },
    );
    unsafe {
        match prev {
            Some(v) => std::env::set_var("PATHKEEP_TEST_AGENT_INDEX_DELAY_US", v),
            None => std::env::remove_var("PATHKEEP_TEST_AGENT_INDEX_DELAY_US"),
        }
    }
    run.unwrap();
    let times = times.lock().unwrap();
    assert_eq!(times[0].0, "index");
    assert_eq!(times[1].0, "finishing");
    assert!(times[1].1.duration_since(times[0].1) >= Duration::from_millis(250));
}
