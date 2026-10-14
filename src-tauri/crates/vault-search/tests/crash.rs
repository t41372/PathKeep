//! Responsibilities: kill real encrypted writers at commit publication boundaries.
//! Not responsible for: injecting faults into cryptography or simulating disk writes.
//! Failure modes: watermark and documents disagree; a dead process leaves a locked
//! writer; replay duplicates records; a killed rebuild exposes partial replacement.
mod support;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;
use support::*;
use vault_search::*;

#[test]
fn killed_commits_and_rebuilds_reopen_at_a_whole_sequence_and_replay() {
    for rebuild in [false, true] {
        for (point, committed) in
            [("temp-written", false), ("temp-synced", false), ("renamed", true)]
        {
            let tmp = tempfile::tempdir().unwrap();
            drop(create(tmp.path(), vec![page(1, "old"), page(2, "old")]));
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["crash_child", "--exact", "--ignored", "--nocapture"])
                .env("SEARCH_CRASH_DIR", tmp.path())
                .env("SEARCH_REBUILD", rebuild.to_string())
                .env("VAULT_CRYPTO_CRASH_AT", format!("atomic-replace/{point}@meta.json:1"))
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap();
            let stdout = child.stdout.take().unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    if tx.send(line.unwrap()).is_err() {
                        break;
                    }
                }
            });
            loop {
                match rx.recv_timeout(Duration::from_secs(60)) {
                    Ok(line) if line.contains("crash-point") => break,
                    Ok(_) => {}
                    Err(e) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        panic!("crash hook not reached: {e}")
                    }
                }
            }
            child.kill().unwrap();
            child.wait().unwrap();
            let index = SearchIndex::open(tmp.path(), Some(&key()), config()).unwrap();
            assert_eq!(index.last_applied_seq().unwrap(), if committed { 10 } else { 1 });
            assert_eq!(
                ids(&session(&index, Ast::And(vec![]))),
                if committed { vec![3, 4] } else { vec![1, 2] }
            );
            index
                .apply(Batch {
                    last_seq: 10,
                    upserts: vec![page(3, "new"), page(4, "new")],
                    deletes: vec![1, 2],
                })
                .unwrap();
            assert_eq!(index.last_applied_seq().unwrap(), 10);
            assert_eq!(ids(&session(&index, term("new"))), vec![3, 4]);
            assert_no_plaintext(tmp.path());
        }
    }
}

#[test]
#[ignore = "child process entered only by the crash campaign"]
fn crash_child() {
    let Some(path) = std::env::var_os("SEARCH_CRASH_DIR") else { return };
    let index = SearchIndex::open(std::path::Path::new(&path), Some(&key()), config()).unwrap();
    if std::env::var("SEARCH_REBUILD").unwrap() == "true" {
        index.rebuild([Ok(page(3, "new")), Ok(page(4, "new"))], 10, |_| {}, || false).unwrap();
    } else {
        index
            .apply(Batch {
                last_seq: 10,
                upserts: vec![page(3, "new"), page(4, "new")],
                deletes: vec![1, 2],
            })
            .unwrap();
    }
}
