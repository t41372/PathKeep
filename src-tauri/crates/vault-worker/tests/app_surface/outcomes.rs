//! The task list distinguishes loss of selected bytes from limits of the parser.
//! All samples use native files and real owner completion events.
use super::*;
use serde_json::Value;
use std::{
    path::Path,
    sync::{Arc, mpsc},
    time::Duration,
};
use vault_worker::data_kinds::backups;

pub(super) fn review(f: &Fixture, root: &Path, tool: &str) -> Value {
    let mut frames = preview::start(
        &f.session,
        from_value(json!({"purpose":"enable","pickedFolder":{"tool":tool,"path":root}})).unwrap(),
    )
    .unwrap();
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        loop {
            let frame = frames.next_frame().await.unwrap();
            if frame["type"] == "finished" {
                break frame["payload"].clone();
            }
        }
    })
}
pub(super) fn enable_request(p: &Value) -> backups::Enable {
    from_value(json!({"previewId":p["previewId"],"selectedRootIds":[p["rows"][0]["rootId"]],"readGit":false,
        "expectedPolicyToken":kinds::settings().unwrap().policy_token,"requestId":uuid::Uuid::new_v4().to_string()})).unwrap()
}
pub(super) fn finished(
    f: &Fixture,
    submit: impl FnOnce(backups::Events) -> anyhow::Result<Value>,
) -> (Value, Value) {
    let (tx, rx) = mpsc::channel();
    submit(Arc::new(move |name, event| {
        if name == "pathkeep://kind-backup-finished" {
            tx.send(event).unwrap();
        }
    }))
    .unwrap();
    let event = rx.recv_timeout(Duration::from_secs(15)).unwrap();
    let rows = backups::tasks(&f.session, from_value(json!({"group":"today","limit":20})).unwrap())
        .unwrap();
    let task = rows["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["taskId"] == event["taskId"])
        .unwrap()
        .clone();
    let run = backups::get_run(&f.session, from_value(json!({"runRef":task["runRef"]})).unwrap())
        .unwrap();
    assert_eq!(run["result"], event["result"]);
    (task, run)
}
fn start(f: &Fixture) -> (Value, Value) {
    finished(f, |events| {
        backups::start(&f.session,from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),events)
    })
}
fn count(f: &Fixture) -> Value {
    backups::task_summary(&f.session, from_value(json!({})).unwrap()).unwrap()["attentionCount"]
        .clone()
}
fn control(task: &Value, action: &str) -> backups::Control {
    from_value(json!({"taskId":task["taskId"],"expectedToken":task["snapshotToken"],"action":action,"requestId":uuid::Uuid::new_v4().to_string()})).unwrap()
}
pub(super) fn pi(path: &Path, id: &str, version: u64) {
    write(path,format!("{{\"type\":\"session\",\"id\":\"{id}\",\"version\":{version},\"cwd\":\"/synthetic\"}}\n").as_bytes())
}
#[test]
fn source_loss_partial_attention_and_successful_retry() {
    let f = Fixture::new(true);
    let folder = f.temp.path().join("pi");
    let file = folder.join("sessions/one.jsonl");
    pi(&file, "one", 1000);
    let req = enable_request(&review(&f, &folder, "pi"));
    let other = folder.with_extension("offline");
    std::fs::rename(&folder, &other).unwrap();
    assert!(backups::enable(&f.session, req, Arc::new(|_, _| {})).is_err());
    assert_eq!(kinds::settings().unwrap().policy.agent_history, KindState::Off);
    std::fs::rename(&other, &folder).unwrap();
    pi(&folder.join("sessions/two.jsonl"), "two", 1000);
    let (task, run) = finished(&f, |e| {
        backups::enable(&f.session, enable_request(&review(&f, &folder, "pi")), e)
    });
    assert_eq!(task["state"], "succeeded");
    assert_eq!(task["attention"], true);
    assert_eq!(task["formatNotices"], json!([{"tool":"pi","version":"1000"}]));
    assert_eq!(run["summary"]["unknownFormat"], "2");
    assert_eq!(run["roots"][0]["availability"], "unknown-format");
    assert_eq!(count(&f), "1");
    f.session.close_vault().unwrap();
    let (repeat, _) = start(&f);
    assert_eq!(repeat["attention"], false);
    assert_eq!(count(&f), "1");
    review_findings::dismiss_notice(&f, &task, "1000");
    assert_eq!(count(&f), "0");
    assert_eq!(start(&f).0["attention"], false);
    pi(&file, "one", 1001);
    let (next, _) = start(&f);
    assert_eq!(next["formatNotices"], json!([{"tool":"pi","version":"1001"}]));
    assert_eq!(count(&f), "1");
}

#[test]
fn folder_lost_after_enable_remains_actionable_until_a_successful_retry() {
    let f = Fixture::new(true);
    let folder = f.temp.path().join("pi");
    let file = folder.join("sessions/one.jsonl");
    pi(&file, "one", 3);
    let review_one = review(&f, &folder, "pi");
    finished(&f, |e| backups::enable(&f.session, enable_request(&review_one), e));
    let offline = folder.with_extension("offline");
    std::fs::rename(&folder, &offline).unwrap();
    let (partial, run) = start(&f);
    assert_eq!(partial["state"], "partial");
    assert_eq!(partial["attention"], true);
    assert_eq!(run["roots"][0]["availability"], "root-missing");
    assert_eq!(run["summary"]["filesAbsent"], "0");
    f.session.close_vault().unwrap();
    assert_eq!(count(&f), "1");
    std::fs::rename(&offline, &folder).unwrap();
    let added = f.temp.path().join("second-pi");
    pi(&added.join("sessions/two.jsonl"), "two", 3);
    let other = review(&f, &added, "pi");
    let mut patch = json!({"selectedRootIds":[review_one["rows"][0]["rootId"],other["rows"][0]["rootId"]],"expectedToken":kinds::settings().unwrap().policy_token});
    assert_eq!(
        kinds::update_sources(&f.session, from_value(patch.clone()).unwrap(), |_| {})
            .unwrap_err()
            .to_string(),
        "enable-preview-required"
    );
    patch["previewId"] = other["previewId"].clone();
    kinds::update_sources(&f.session, from_value(patch).unwrap(), |_| {}).unwrap();
    let (retried, detail) =
        finished(&f, |e| backups::control(&f.session, control(&partial, "retry"), e));
    assert_eq!(retried["state"], "succeeded");
    assert_eq!(detail["summary"]["filesVisited"], "1");
    assert_eq!(count(&f), "0");
    std::fs::remove_file(file).unwrap();
    assert_eq!(start(&f).1["summary"]["filesAbsent"], "1");
}

#[test]
fn empty_and_malformed_files_show_the_same_notes_without_new_attention() {
    let f = Fixture::new(true);
    let folder = f.temp.path().join("claude");
    write(&folder.join("projects/test/empty.jsonl"), b"");
    let file = native::claude_main(
        &folder,
        "mixed",
        &[native::claude_user("mixed", "u", None, "/synthetic", "kept")],
    );
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&file)
        .unwrap()
        .write_all(b"broken-json\n")
        .unwrap();
    let first = finished(&f, |e| {
        backups::enable(&f.session, enable_request(&review(&f, &folder, "claude-code")), e)
    });
    f.session.close_vault().unwrap();
    for (index, (task, run)) in [first, start(&f)].into_iter().enumerate() {
        assert_eq!(task["state"], "succeeded", "{run}");
        assert_eq!(task["attention"], false);
        assert_eq!(run["noteCount"], "2");
        assert_eq!(run["newNoteCount"], if index == 0 { "2" } else { "0" });
        assert_eq!(run["warningCount"], "0");
        assert_eq!(run["notes"].as_array().unwrap().len(), 2);
        assert_eq!(count(&f), "0");
    }
    let rows = backups::sources(&f.session, from_value(json!({"limit":10})).unwrap()).unwrap();
    let root = rows["items"].as_array().unwrap().iter().find(|r| r["enabled"] == true).unwrap();
    assert_eq!(root["noteCount"], "2");
    assert_eq!(root["newNoteCount"], "0");
    assert_eq!(root["notes"].as_array().unwrap().len(), 2);
}

#[test]
fn older_deepseek_files_are_notes_and_only_a_newer_version_needs_attention() {
    let f = Fixture::new(true);
    let root = f.temp.path().join("deepseek");
    let put = |id: &str, version: u64| {
        let path = native::deepseek_folder(&root, id).join(format!("session.v{version}.jsonl"));
        write(
            &path,
            format!("{}\n", native::deepseek_header(id, version, "/synthetic")).as_bytes(),
        );
    };
    put("old", 3);
    let (task, old) = finished(&f, |e| {
        backups::enable(&f.session, enable_request(&review(&f, &root, "deepseek")), e)
    });
    assert_eq!(task["attention"], false);
    assert_eq!(old["result"], "success");
    assert_eq!(old["notes"][0]["code"], "deepseek-format-limited");
    put("future", 8);
    let (task, new) = start(&f);
    assert_eq!(new["result"], "success");
    assert_eq!(task["formatNotices"], json!([{"tool":"deepseek","version":"8"}]));
    let (repeat, notes) = start(&f);
    assert_eq!(repeat["attention"], false);
    assert_eq!(notes["newNoteCount"], "0");
}

#[cfg(unix)]
#[test]
fn a_selected_file_with_denied_read_permission_requires_attention() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(true);
    let root = f.temp.path().join("pi");
    pi(&root.join("sessions/readable.jsonl"), "readable", 3);
    let grant = review(&f, &root, "pi");
    let protected = root.join("sessions/private.jsonl");
    pi(&protected, "private", 3);
    std::fs::set_permissions(&protected, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::File::open(&protected).is_ok() {
        // A privileged test runner can read this inode despite its mode.
        return;
    }
    let (task, run) = finished(&f, |e| backups::enable(&f.session, enable_request(&grant), e));
    assert_eq!(task["attention"], true);
    assert_eq!(run["result"], "partial");
    assert_eq!(run["summary"]["filesCaptured"], "1");
    assert_eq!(run["summary"]["unavailable"], "1");
    assert_eq!(run["warnings"][0]["code"], "source_capture_unavailable");
}

#[test]
fn publication_failure_stops_capture_and_keeps_the_committed_prefix() {
    use vault_store::{Db, Priority};
    let f = Fixture::new(true);
    let root = f.temp.path().join("pi");
    for id in ["one", "two", "three"] {
        pi(&root.join(format!("sessions/{id}.jsonl")), id, 3);
    }
    let request = enable_request(&review(&f, &root, "pi"));
    f.session
        .vault()
        .unwrap()
        .store()
        .write(Db::AgentHistoryReadModel, Priority::Interactive, |s| {
            s.execute("CREATE UNIQUE INDEX fixture_one_session ON ah_sessions(harness)", ())?;
            Ok(())
        })
        .unwrap();
    let (task, run) = finished(&f, |e| backups::enable(&f.session, request, e));
    assert_eq!(run["result"], "partial");
    assert_eq!(task["attention"], true);
    assert_eq!(run["warningCount"], "1");
    assert_eq!(run["warnings"][0]["code"], "archive-publication-failed");
    assert_eq!(run["summary"]["filesCaptured"], "2");
    assert_eq!(run["summary"]["newRoots"], "1");
}

#[path = "consent.rs"]
mod consent;

#[path = "review.rs"]
mod review_findings;

#[test]
fn only_the_index_phase_can_wait_and_resume() {
    let f = Fixture::new(true);
    super::source_rules::grant(&f);
    pi(&f.temp.path().join("chosen-pi/sessions/test.jsonl"), "pause-index", 3);
    let (index_tx, index_rx) = std::sync::mpsc::channel();
    let (finished_tx, finished_rx) = std::sync::mpsc::channel();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let release = barrier.clone();
    backups::start(&f.session,from_value(json!({"scope":{"type":"kind","kind":"agent-history"},"requestId":uuid::Uuid::new_v4().to_string()})).unwrap(),Arc::new(move|name,row|{
        if name=="pathkeep://background-task-changed" && row["phase"]=="index"{
            index_tx.send(row).unwrap();release.wait();
        }else if name=="pathkeep://kind-backup-finished"{finished_tx.send(row).unwrap();}
    })).unwrap();
    let indexing = index_rx.recv_timeout(Duration::from_secs(15)).unwrap();
    let paused =
        backups::control(&f.session, control(&indexing, "pause"), Arc::new(|_, _| {})).unwrap();
    barrier.wait();
    let waiting =
        backups::tasks(&f.session, from_value(json!({"group":"waiting","limit":10})).unwrap())
            .unwrap();
    assert_eq!(waiting["items"][0]["taskId"], indexing["taskId"]);
    assert_eq!(paused["task"]["actions"], json!(["cancel", "resume"]));
    assert!(finished_rx.try_recv().is_err());
    backups::control(&f.session, control(&paused["task"], "resume"), Arc::new(|_, _| {})).unwrap();
    assert_eq!(finished_rx.recv_timeout(Duration::from_secs(15)).unwrap()["result"], "success");
}
