//! Drive the worker with the real installer and an offline, pinned local file. The archive
//! gate keeps the workers in flight until both starts (and any cancellation) have returned.
use super::*;
use crate::{ModelFile, ProjectPaths, archive::ArchiveOpGate};
use serde_json::{Value, json};
use std::sync::mpsc;

const REPO: &str = "test/model";
const FILES: &[ModelFile] = &[ModelFile {
    name: "config.json",
    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    repo: REPO,
}];

fn offline_archive() -> (tempfile::TempDir, ProjectPaths) {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    crate::save_config(&crate::ControlGate::acquire(&paths), &enabled_config()).unwrap();
    let flat = crate::model_dir_for_repo(&paths, REPO);
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
    let first_events: Vec<_> =
        first_events.into_iter().filter(|e| e["kind"] != "waiting").collect();
    let second_events: Vec<_> =
        second_events.into_iter().filter(|e| e["kind"] != "waiting").collect();
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
    let events: Vec<_> = events.into_iter().filter(|e| e["kind"] != "waiting").collect();
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
    assert!(events.contains(&json!({"kind":"cancelled", "transferId":old_id})), "{events:?}");
    assert!(!cancel_model_download(new_id));
}

#[test]
fn model_file_progress_identifies_the_transfer_on_every_phase() {
    let (sink, events) = mpsc::channel();
    let mut progress = EmitProgress {
        emit: move |event| sink.send(serde_json::to_value(event).unwrap()).unwrap(),
        transfer: Arc::new(Transfer::new(ModelDownloadTransfer { transfer_id: "fixture".into() })),
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

// Pins for the same deterministic byte streams the installer reads in the test build.
const DOWNLOAD_FILES: &[ModelFile] = &[
    ModelFile {
        name: "model.safetensors",
        repo: DEFAULT_STATIC_MODEL_REPO,
        sha256: "61ac855f3c09ad83e12e82ec78febefa65a6c7e22f8542f8b0dae0ecca6d0677",
    },
    ModelFile {
        name: "tokenizer.json",
        repo: DEFAULT_STATIC_MODEL_REPO,
        sha256: "57e6dd8bf58cec0fce50efd154940798d391016f9b32e26f984602617de10aad",
    },
];

fn enabled_config() -> crate::AppConfig {
    let mut config = crate::AppConfig { initialized: true, ..Default::default() };
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    config.ai.embedding_provider_id = Some(crate::BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID.into());
    config
}

fn choose_service(config: &mut crate::AppConfig) {
    let service = crate::AiProviderConfig { id: "hosted".into(), ..Default::default() };
    config.ai.embedding_provider_id = Some(service.id.clone());
    config.ai.embedding_providers.push(service);
}

/// A direct durable save bypasses the Settings hook. Keep the installer at the archive gate
/// so the save lands before the worker can return Done for the already-verified local model.
fn save_during_verified_transfer(change: impl FnOnce(&mut crate::AppConfig), cancelled: bool) {
    use crate::{ControlGate, Vault, save_config};

    let vault = Vault::open_temp().unwrap();
    let paths = vault.paths();
    let mut config = enabled_config();
    save_config(&ControlGate::acquire(paths), &config).unwrap();
    let repo = model_dir_for_repo(paths, DEFAULT_STATIC_MODEL_REPO);
    let installed = ensure_model_downloaded(
        paths,
        DEFAULT_STATIC_MODEL_REPO,
        DOWNLOAD_FILES,
        true,
        &mut crate::NoopDownloadProgress,
    )
    .unwrap();
    let pointer = std::fs::read(repo.join("current")).unwrap();
    assert!(crate::model_is_present_and_verified(&installed, DOWNLOAD_FILES));

    let gate = ArchiveOpGate::acquire(paths);
    let (tx, rx) = mpsc::channel();
    let transfer = start_model_download(
        paths.clone(),
        DEFAULT_STATIC_MODEL_REPO,
        DOWNLOAD_FILES,
        move |event| tx.send(serde_json::to_value(event).unwrap()).unwrap(),
    )
    .unwrap();
    change(&mut config);
    save_config(&ControlGate::acquire(paths), &config).unwrap();
    drop(gate);

    let events: Vec<_> = rx.into_iter().filter(|e| e["kind"] != "waiting").collect();
    assert_eq!(events.len(), 1);
    let kind = if cancelled { "cancelled" } else { "done" };
    assert_eq!(events[0], json!({"kind":kind, "transferId":transfer.transfer_id}));
    assert_eq!(std::fs::read(repo.join("current")).unwrap(), pointer);
    assert!(crate::model_is_present_and_verified(&installed, DOWNLOAD_FILES));
    if cancelled {
        start_model_download(paths.clone(), DEFAULT_STATIC_MODEL_REPO, DOWNLOAD_FILES, |_| {})
            .expect_err("a stale start cannot revive the cancelled transfer");
        save_config(&ControlGate::acquire(paths), &enabled_config()).unwrap();
        let (tx, rx) = mpsc::channel();
        start_model_download(paths.clone(), DEFAULT_STATIC_MODEL_REPO, DOWNLOAD_FILES, move |e| {
            tx.send(e.update).unwrap()
        })
        .unwrap();
        assert_eq!(
            rx.into_iter().collect::<Vec<_>>(),
            [ModelDownloadUpdate::Done],
            "reenabling reused the verified files offline"
        );
    }
    assert_no_index_build(&vault);
}

#[test]
fn semantic_off_save_cancels_the_transfer_and_keeps_verified_files() {
    save_during_verified_transfer(|config| config.ai.semantic_index_enabled = false, true);
}

#[test]
fn service_switch_save_cancels_the_builtin_transfer() {
    save_during_verified_transfer(choose_service, true);
}

#[test]
fn master_ai_off_save_cancels_the_builtin_transfer() {
    save_during_verified_transfer(|config| config.ai.enabled = false, true);
}

#[test]
fn backup_schedule_save_keeps_the_builtin_transfer_running() {
    save_during_verified_transfer(|config| config.due_after_hours += 1.0, false);
}

fn assert_no_index_build(vault: &crate::Vault) {
    let builds: i64 = vault
        .store()
        .read(vault_store::Db::Intelligence, |s| {
            s.query_one("SELECT count(*) FROM ai_jobs WHERE job_type='index-build'", (), |row| {
                Ok(row.get(0)?)
            })
        })
        .unwrap();
    assert_eq!(builds, 0);
}

/// Pause after the first file reaches the unpublished candidate, before the second starts.
/// The caller saves settings or restores an archive with no frontend cancellation involved.
fn during_candidate<T>(paths: &ProjectPaths, operation: impl FnOnce() -> T, cancelled: bool) -> T {
    use std::time::Duration;
    let (events, received) = mpsc::channel();
    let (resume, wait) = mpsc::channel();
    let transfer =
        start_model_download(paths.clone(), DEFAULT_STATIC_MODEL_REPO, DOWNLOAD_FILES, move |event| {
            let pause = matches!(&event.update, ModelDownloadUpdate::FileProgress { file, .. } if file == DOWNLOAD_FILES[0].name);
            events.send(event).unwrap();
            if pause {
                wait.recv_timeout(Duration::from_secs(30)).unwrap();
            }
        })
        .unwrap();
    loop {
        let event = received.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(event.transfer_id, transfer.transfer_id);
        if matches!(event.update, ModelDownloadUpdate::FileProgress { .. }) {
            break;
        }
        assert!(matches!(event.update, ModelDownloadUpdate::FileStarted { .. }));
    }
    let repo = model_dir_for_repo(paths, DEFAULT_STATIC_MODEL_REPO);
    let candidate = std::fs::read_dir(&repo).unwrap().next().unwrap().unwrap().path();
    assert!(candidate.join(DOWNLOAD_FILES[0].name).metadata().unwrap().len() > 0);
    assert!(!candidate.join(DOWNLOAD_FILES[1].name).exists(), "the second file has not started");
    assert!(!repo.join("current").exists());
    let result = operation();
    resume.send(()).unwrap();
    let rest: Vec<_> = received.into_iter().collect();
    assert!(rest.iter().all(|e| e.transfer_id == transfer.transfer_id));
    if cancelled {
        let (terminal, preceding) = rest.split_last().unwrap();
        assert!(
            preceding.iter().all(|e| matches!(e.update, ModelDownloadUpdate::FileFinished { .. }))
        );
        assert_eq!(terminal.update, ModelDownloadUpdate::Cancelled);
        assert_eq!(std::fs::read_dir(repo).unwrap().count(), 0, "partial generation survived");
    } else {
        assert_eq!(rest.last().unwrap().update, ModelDownloadUpdate::Done);
        assert!(crate::model_is_present_and_verified(&candidate, DOWNLOAD_FILES));
    }
    result
}

fn save_during_candidate(
    save: impl FnOnce(&crate::ControlGate, &mut crate::AppConfig),
    cancelled: bool,
) {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths();
    let mut config = enabled_config();
    crate::save_config(&crate::ControlGate::acquire(paths), &config).unwrap();
    during_candidate(paths, || save(&crate::ControlGate::acquire(paths), &mut config), cancelled);
    assert_no_index_build(&vault);
}

#[test]
fn semantic_off_save_discards_the_in_flight_candidate() {
    save_during_candidate(
        |gate, config| {
            config.ai.semantic_index_enabled = false;
            crate::save_config(gate, config).unwrap();
        },
        true,
    );
}

#[test]
fn embedding_service_save_discards_the_in_flight_candidate() {
    save_during_candidate(
        |gate, config| {
            choose_service(config);
            crate::save_config(gate, config).unwrap();
        },
        true,
    );
}

#[test]
fn unrelated_save_permits_the_candidate_to_publish() {
    save_during_candidate(
        |gate, config| {
            config.due_after_hours += 1.0;
            crate::save_config(gate, config).unwrap();
        },
        false,
    );
}

#[cfg(unix)]
#[test]
fn failed_off_save_preserves_download_consent() {
    save_during_candidate(
        |gate, config| {
            config.ai.enabled = false;
            crate::durable_io::inject_fsync_faults(vec![Some(libc::EIO)]);
            crate::save_config(gate, config).expect_err("failed durability barrier");
            assert!(crate::load_config(gate.paths()).unwrap().ai.enabled);
        },
        false,
    );
}

#[test]
fn a_staged_off_config_preserves_the_live_transfer() {
    save_during_candidate(
        |gate, config| {
            let stage = crate::maintenance::staging_root(gate.paths()).unwrap();
            config.ai.semantic_index_enabled = false;
            crate::config::write_staged_control(
                &crate::project_paths_with_root(stage.path()),
                config,
                None,
            )
            .unwrap();
        },
        false,
    );
}

#[test]
fn restoring_a_semantic_off_config_cancels_the_live_transfer() {
    let vault = crate::Vault::open_temp().unwrap();
    let copy = vault.paths().raw_snapshots_dir.join("maintenance/before-enabling");
    crate::maintenance::snapshot(&vault, &copy).unwrap();
    let config = enabled_config();
    crate::save_config(&crate::ControlGate::acquire(vault.paths()), &config).unwrap();
    let live = vault.with_config(config);
    let restored = during_candidate(
        live.paths(),
        || {
            crate::maintenance::restore_snapshot(&live, &copy, None, Default::default())
                .unwrap()
                .vault
        },
        true,
    );
    assert!(!crate::load_config(restored.paths()).unwrap().ai.semantic_index_enabled);
    assert_no_index_build(&restored);
}

thread_local! {
    static DECISION_PROBE: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

pub(in crate::ai) fn decision_probe() {
    if let Some(probe) = DECISION_PROBE.take() {
        probe();
    }
}

#[test]
fn cancel_cannot_change_a_publication_or_a_decided_failure() {
    const BAD: &[ModelFile] = &[ModelFile { name: "config.json", repo: REPO, sha256: "wrong" }];
    for (files, expected) in [(DOWNLOAD_FILES, "done"), (BAD, "error")] {
        let (_root, paths) = offline_archive();
        let (tx, rx) = mpsc::channel();
        let transfer = start_model_download(paths, "decision", files, move |event| {
            if matches!(event.update, ModelDownloadUpdate::FileStarted { .. }) {
                let id = event.transfer_id.clone();
                let tx = tx.clone();
                DECISION_PROBE.set(Some(Box::new(move || {
                    tx.send(json!(cancel_model_download(&id))).unwrap()
                })));
            }
            tx.send(serde_json::to_value(event).unwrap()).unwrap();
        })
        .unwrap();
        let events: Vec<_> = rx.into_iter().collect();
        assert!(events.contains(&json!(false)), "late cancel accepted: {events:?}");
        let terminal = events.last().unwrap();
        assert_eq!(terminal["kind"], expected);
        assert_eq!(terminal["transferId"], transfer.transfer_id);
    }
}

#[test]
fn replacement_waits_for_cancelled_candidate_and_response() {
    use std::time::Duration;
    let (_root, paths) = offline_archive();
    let (entered, held) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (old_events, old) = mpsc::channel();
    let first = start_model_download(paths.clone(), "serial", DOWNLOAD_FILES, move |event| {
        if matches!(event.update, ModelDownloadUpdate::FileProgress { .. }) {
            entered.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(15)).unwrap();
        }
        old_events.send(event).unwrap();
    })
    .unwrap();
    held.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(cancel_model_download(&first.transfer_id));
    let (tx, rx) = mpsc::channel();
    let second = start_model_download(paths.clone(), "serial", DOWNLOAD_FILES, move |event| {
        tx.send(event).unwrap()
    })
    .unwrap();
    let first_event =
        serde_json::to_value(rx.recv_timeout(Duration::from_secs(5)).unwrap()).unwrap();
    release.send(()).unwrap();
    assert_eq!(old.into_iter().last().unwrap().update, ModelDownloadUpdate::Cancelled);
    assert_eq!(rx.into_iter().last().unwrap().update, ModelDownloadUpdate::Done);
    assert_eq!(
        first_event,
        json!({"kind":"waiting","reason":"previous-transfer","transferId":second.transfer_id})
    );
}

#[test]
fn archive_gate_wait_reports_the_transfer_before_the_gate_opens() {
    let (_root, paths) = offline_archive();
    let gate = ArchiveOpGate::acquire(&paths);
    let (tx, rx) = mpsc::channel();
    let start = start(&paths, move |event| tx.send(serde_json::to_value(event).unwrap()).unwrap());
    let waiting = rx.recv_timeout(std::time::Duration::from_secs(2));
    let heartbeat = rx.recv_timeout(std::time::Duration::from_secs(30));
    drop(gate);
    assert_eq!(heartbeat.expect("missing heartbeat"), waiting.as_ref().unwrap().clone());
    assert_eq!(rx.into_iter().last().unwrap()["kind"], "done");
    assert_eq!(
        waiting.expect("silent archive wait"),
        json!({"kind":"waiting","reason":"archive-busy","transferId":start["transferId"]})
    );
}

#[cfg(unix)]
#[test]
fn a_failed_directory_sync_applies_the_visible_config_and_cancels() {
    save_during_candidate(
        |gate, config| {
            config.ai.enabled = false;
            crate::durable_io::inject_fsync_faults(vec![None, Some(libc::EIO)]);
            let saved = crate::config::save_config_with_durability_warning(gate, config);
            assert!(!crate::load_config(gate.paths()).unwrap().ai.enabled);
            assert!(
                saved
                    .expect("rename already applied the config")
                    .expect("durability warning")
                    .contains("syncing")
            );
        },
        true,
    );
}
