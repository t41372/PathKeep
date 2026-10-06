//! Responsibilities: kill real owner applies and recover missing/corrupt derived indexes.
//! Not responsible for: mocking storage publication or testing cryptographic primitives.
//! Failure modes: commit/ack crash loses an event, old locks survive death, missing sidecar
//! looks complete, or corruption destroys canonical facts. Runs both storage modes.
use super::{SearchRequest, SearchSession};
use crate::{SetNotesRequest, Vault};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    time::Duration,
};

#[test]
fn owner_replays_a_killed_apply_to_the_same_canonical_result() {
    for key in [None, Some([97; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        super::tests::seed(&vault, 25);
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.close().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["search::recovery_tests::apply_child", "--exact", "--ignored", "--nocapture"])
            .env("PATHKEEP_SEARCH_CRASH_ROOT", &paths.app_root)
            .env("PATHKEEP_SEARCH_CRASH_KEY", if key.is_some() { "encrypted" } else { "plain" })
            .env("PATHKEEP_SEARCH_COMMIT_PAUSE", "prepared")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = command.spawn().unwrap();
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
            match rx.recv_timeout(Duration::from_secs(45)) {
                Ok(line) if line.contains("search-commit-prepared") => break,
                Ok(_) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("apply did not reach publication: {error}")
                }
            }
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let reopened = Vault::open(paths, config, key).unwrap();
        reopened.search().cycle().unwrap();
        let result = SearchSession::start(
            &reopened,
            SearchRequest { query: "crashsentinel".into(), ..Default::default() },
        )
        .unwrap()
        .finish_page()
        .unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].page_id, 1);
        crate::test_support::assert_search_handoff(&reopened);
    }
}
#[test]
#[ignore = "child process for owner crash campaign"]
fn apply_child() {
    let Some(root) = std::env::var_os("PATHKEEP_SEARCH_CRASH_ROOT") else { return };
    let key =
        (std::env::var("PATHKEEP_SEARCH_CRASH_KEY").unwrap() == "encrypted").then_some([97; 32]);
    let vault = Vault::open(
        crate::project_paths_with_root(std::path::Path::new(&root)),
        crate::AppConfig { initialized: true, git_enabled: false, ..Default::default() },
        key,
    )
    .unwrap();
    crate::set_notes(
        &vault,
        SetNotesRequest {
            url: "https://example.com/page/1".into(),
            notes: "crashsentinel".into(),
            source_profile: None,
        },
    )
    .unwrap();
    vault.search().cycle().unwrap();
    panic!("commit pause did not fire");
}
#[test]
fn missing_and_corrupt_indexes_rebuild_from_documents_even_after_outbox_pruning() {
    for key in [None, Some([98; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        super::tests::seed(&vault, 25);
        let paths = vault.paths().clone();
        let config = vault.config().clone();
        vault.close().unwrap();
        let index = paths.derived_dir.join("search-index");
        std::fs::remove_dir_all(&index).unwrap();
        let reopened = Vault::open(paths.clone(), config.clone(), key).unwrap();
        reopened.search().cycle().unwrap();
        crate::test_support::assert_search_handoff(&reopened);
        reopened.close().unwrap();
        std::fs::write(
            index.join(std::fs::read_to_string(index.join("current")).unwrap()).join("meta.json"),
            b"broken derived metadata",
        )
        .unwrap();
        let repaired = Vault::open(paths, config, key).unwrap();
        repaired.search().cycle().unwrap();
        crate::test_support::assert_search_handoff(&repaired);
        assert_eq!(
            SearchSession::start(
                &repaired,
                SearchRequest { query: "common".into(), ..Default::default() }
            )
            .unwrap()
            .finish_page()
            .unwrap()
            .count
            .value,
            25
        );
    }
}

#[test]
fn a_rebuild_serves_the_old_generation_without_holding_a_store_reader() {
    let vault = Vault::open_temp_with_key(Some([106; 32])).unwrap();
    super::tests::seed(&vault, 2200);
    let indexer = vault.search().clone();
    let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
    let progress_log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = progress_log.clone();
    let thread = std::thread::spawn(move || {
        indexer.rebuild_with_progress(move |progress| {
            log.lock().unwrap().push((progress.documents, progress.complete));
            if progress.documents == 0 && !progress.complete {
                entered_tx.send(()).unwrap();
                resume_rx.recv().unwrap();
            }
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let reader = vault.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let query = std::thread::spawn(move || {
        let result = SearchSession::start(
            &reader,
            SearchRequest { query: "common".into(), ..Default::default() },
        )
        .unwrap()
        .finish_page()
        .unwrap();
        tx.send(result.count.value).unwrap();
    });
    let during = rx.recv_timeout(Duration::from_secs(10));
    resume_tx.send(()).unwrap();
    thread.join().unwrap().unwrap();
    query.join().unwrap();
    assert_eq!(during.unwrap(), 2200);
    crate::test_support::assert_search_handoff(&vault);
    let progress = progress_log.lock().unwrap();
    assert_eq!(progress.first(), Some(&(0, false)));
    assert!(progress.iter().any(|(documents, complete)| *documents > 0 && !complete));
    assert_eq!(progress.last(), Some(&(2200, true)));
}

#[test]
fn an_open_owner_recovers_corrupt_metadata_without_a_read_side_rebuild() {
    for key in [None, Some([107; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        super::tests::seed(&vault, 25);
        let root = vault.paths().derived_dir.join("search-index");
        let old = std::fs::read_to_string(root.join("current")).unwrap();
        std::fs::write(root.join(&old).join("meta.json"), b"damaged metadata").unwrap();
        let start = std::time::Instant::now();
        loop {
            let state = vault.search().state();
            if std::fs::read_to_string(root.join("current")).unwrap() != old
                && state.phase == super::IndexerPhase::Ready
            {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(30), "owner did not recover: {state:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
        crate::test_support::assert_search_handoff(&vault);
    }
}

#[test]
fn cancelling_a_rebuild_preserves_search_and_a_later_rebuild_can_finish() {
    for key in [None, Some([111; 32])] {
        let vault = Vault::open_temp_with_key(key).unwrap();
        super::tests::seed(&vault, 30);
        let cancel = vault.search().clone();
        assert!(
            vault
                .search()
                .rebuild_with_progress(move |progress| {
                    if !progress.complete {
                        cancel.cancel_rebuild();
                    }
                })
                .is_err()
        );
        let result = SearchSession::start(
            &vault,
            SearchRequest { query: "common".into(), ..Default::default() },
        )
        .unwrap()
        .finish_page()
        .unwrap();
        assert_eq!(result.count.value, 30);
        vault.search().rebuild().unwrap();
        crate::test_support::assert_search_handoff(&vault);
    }
}
