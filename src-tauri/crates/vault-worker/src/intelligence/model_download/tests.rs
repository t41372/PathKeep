//! Drive the worker with the real installer and an offline, pinned local file. The archive
//! gate keeps the workers in flight until both starts (and any cancellation) have returned.
use super::*;
use serde_json::{Value, json};
use std::sync::mpsc;
use vault_core::{ModelFile, ProjectPaths, archive::ArchiveOpGate};

const REPO: &str = "test/model";
const FILES: &[ModelFile] = &[ModelFile {
    name: "config.json",
    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    repo: REPO,
}];

fn offline_archive() -> (tempfile::TempDir, ProjectPaths) {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let flat = vault_core::model_dir_for_repo(&paths, REPO);
    std::fs::create_dir_all(&flat).unwrap();
    std::fs::write(flat.join("config.json"), b"abc").unwrap();
    (root, paths)
}

fn start(
    paths: &ProjectPaths,
    broadcast: impl Fn(ModelDownloadProgressEvent) + Send + 'static,
) -> Value {
    serde_json::to_value(start_model_download(paths.clone(), REPO, FILES, broadcast).unwrap())
        .unwrap()
}

#[test]
fn overlapping_model_starts_share_one_transfer_and_terminal_event() {
    let (_root, paths) = offline_archive();
    let gate = ArchiveOpGate::acquire(&paths);
    let (a, first_events) = mpsc::channel();
    let (b, second_events) = mpsc::channel();
    let broadcast = move |event| {
        let event = serde_json::to_value(event).unwrap();
        a.send(event.clone()).unwrap();
        b.send(event).unwrap();
    };
    let first = start(&paths, broadcast.clone());
    let second = start(&paths, broadcast);
    drop(gate);
    let first_events: Vec<_> = first_events.into_iter().collect();
    let second_events: Vec<_> = second_events.into_iter().collect();
    assert_eq!(first_events.len(), 1, "an offline transfer emits exactly one terminal broadcast");
    assert_eq!(first_events, second_events);
    assert_eq!(first, second);
    let id = first["transferId"].as_str().expect("start returned a transfer ID");
    assert_eq!(first_events, [json!({"kind":"done", "transferId":id})]);
    assert!(!cancel_model_download(id), "finished transfer still accepts cancellation");
}

#[test]
fn cancelling_then_restarting_a_model_keeps_both_outcomes_scoped() {
    let (_root, paths) = offline_archive();
    let gate = ArchiveOpGate::acquire(&paths);
    let (sink, events) = mpsc::channel();
    let broadcast = move |event| sink.send(serde_json::to_value(event).unwrap()).unwrap();
    let first = start(&paths, broadcast.clone());
    let old_id = first["transferId"].as_str().expect("start returned a transfer ID");
    assert!(cancel_model_download(old_id));
    let next = start(&paths, broadcast);
    let stale_accepted = cancel_model_download(old_id);
    drop(gate);
    let events: Vec<_> = events.into_iter().collect();
    assert_eq!(
        events.iter().filter(|e| e["kind"] == "done").count(),
        1,
        "old cancellation must survive restart, and a stale cancel must spare the new run"
    );
    let new_id = next["transferId"].as_str().unwrap();
    assert_ne!(new_id, old_id);
    assert!(!stale_accepted);
    assert_eq!(events.len(), 2);
    assert!(events.contains(&json!({"kind":"done","transferId":new_id})));
    assert!(events.iter().any(|e| e["transferId"] == old_id
        && e["kind"] == "error"
        && e["message"].as_str().unwrap().contains("cancelled")));
    assert!(!cancel_model_download(new_id));
}

#[test]
fn model_file_progress_identifies_the_transfer_on_every_phase() {
    let (sink, events) = mpsc::channel();
    let mut progress = EmitProgress {
        emit: move |event| sink.send(serde_json::to_value(event).unwrap()).unwrap(),
        transfer: Arc::new(Transfer {
            identity: ModelDownloadTransfer { transfer_id: "fixture".into() },
            cancelled: AtomicBool::new(false),
        }),
    };
    progress.file_started("weights", 42);
    progress.file_progress("weights", 11, 42);
    progress.file_finished("weights");
    drop(progress);
    assert_eq!(
        events.into_iter().collect::<Vec<_>>(),
        [
            json!({"transferId":"fixture","kind":"fileStarted","file":"weights","totalBytes":42}),
            json!({"transferId":"fixture","kind":"fileProgress","file":"weights","downloadedBytes":11,"totalBytes":42}),
            json!({"transferId":"fixture","kind":"fileFinished","file":"weights"}),
        ]
    );
}
