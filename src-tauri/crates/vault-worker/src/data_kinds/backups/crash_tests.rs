//! Kill a real owner at each publication boundary. Only the parent kills the
//! process. Reopen twice to detect replay of an already consumed intent.
use super::*;
use std::{
    fs,
    process::Command,
    time::{Duration, Instant},
};
const MARK: &str = "PATHKEEP_FIRST_SAVE_KILL_AT";

pub(super) fn boundary(at: &str) {
    if std::env::var(MARK).as_deref() == Ok(at) {
        fs::write("boundary", at).unwrap();
        loop {
            std::thread::park();
        }
    }
}

#[test]
fn first_save_recovers_each_process_death_boundary() {
    if let Ok(role) = std::env::var("PATHKEEP_FIRST_SAVE_CHILD") {
        child(&role);
        return;
    }
    for point in ["reserved", "published", "created", "claimed", "raw", "finished"] {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("root")).unwrap();
        fs::write(
            temp.path().join("root/config.json"),
            include_bytes!("../../../../../../tests/root_admission/fixtures/legacy.json"),
        )
        .unwrap();
        let transcript = temp.path().join("tools/.claude/projects/project/session.jsonl");
        fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        fs::write(transcript,b"{\"type\":\"user\",\"uuid\":\"u\",\"sessionId\":\"s\",\"message\":{\"role\":\"user\",\"content\":\"recover me\"}}\n").unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "data_kinds::backups::crash_tests::first_save_recovers_each_process_death_boundary",
            "--nocapture",
        ])
        .current_dir(temp.path())
        .env("CHB_PROJECT_ROOT", "root")
        .env("CHB_AGENT_TOOLS_HOME", temp.path().join("tools"))
        .env("CHB_TEST_KEYRING_DIR", temp.path().join("keyring"))
        .env("PATHKEEP_FIRST_SAVE_CHILD", "capture")
        .env(MARK, point);
        let mut process = cmd.spawn().unwrap();
        let start = Instant::now();
        while !temp.path().join("boundary").exists() {
            assert!(process.try_wait().unwrap().is_none(), "owner exited before {point}");
            if start.elapsed() > Duration::from_secs(15) {
                process.kill().unwrap();
                process.wait().unwrap();
                panic!("owner did not reach {point}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        process.kill().unwrap();
        process.wait().unwrap();
        cmd.env_remove(MARK).env("PATHKEEP_FIRST_SAVE_CHILD", point);
        for _ in 0..2 {
            assert!(cmd.status().unwrap().success(), "recover {point}");
        }
    }
}

fn child(point: &str) {
    let session = VaultSession::default();
    let vault = session.background_vault().unwrap();
    if point == "capture" {
        let mut frames = preview::start(
            &session,
            serde_json::from_value(json!({"purpose":"enable","tools":["claude-code"]})).unwrap(),
        )
        .unwrap();
        let final_frame =
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(
                async {
                    loop {
                        let frame = frames.next_frame().await.unwrap();
                        if frame["type"] == "finished" {
                            break frame["payload"].clone();
                        }
                    }
                },
            );
        let root =
            final_frame["rows"].as_array().unwrap().iter().find(|r| r["readable"] == true).unwrap();
        let req = json!({"requestId":uuid::Uuid::new_v4().to_string(),"previewId":final_frame["previewId"],"selectedRootIds":[root["rootId"]],"readGit":false,
            "expectedPolicyToken":super::super::settings().unwrap().policy_token});
        fs::write("request.json", serde_json::to_vec(&req).unwrap()).unwrap();
        enable(&session, serde_json::from_value(req).unwrap(), Arc::new(|_, _| {})).unwrap();
        while session.conversations.active.lock().unwrap().is_some() {
            std::thread::sleep(Duration::from_millis(10));
        }
        return;
    }
    let start = Instant::now();
    while session.conversations.active.lock().unwrap().is_some() {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    let runs = vault
        .store()
        .read(Db::AgentHistoryEvidence, |s| {
            s.query("SELECT status FROM ah_backup_runs", (), |r| Ok(r.get::<String>(0)?))
        })
        .unwrap();
    let heads = vault
        .store()
        .read(Db::AgentHistoryEvidence, |s| {
            s.query_one("SELECT COUNT(*) FROM ah_revisions WHERE state='complete'", (), |r| {
                Ok(r.get::<i64>(0)?)
            })
        })
        .unwrap();
    let policy = super::super::settings().unwrap().policy;
    let req = serde_json::from_slice(&fs::read("request.json").unwrap()).unwrap();
    if point == "reserved" {
        assert_eq!(policy.agent_history, KindState::Off);
        assert!(runs.is_empty());
        assert_eq!(heads, 0);
        assert!(enable(&session, req, Arc::new(|_, _| {})).is_err());
    } else {
        assert_eq!(policy.agent_history, KindState::On);
        assert_eq!(
            runs,
            [if matches!(point, "published" | "created" | "finished") {
                "complete"
            } else {
                "interrupted"
            }]
        );
        assert_eq!(heads, i64::from(point != "claimed"));
        assert_eq!(enable(&session, req, Arc::new(|_, _| {})).unwrap()["state"], "on");
    }
    session.close_vault().unwrap();
}
