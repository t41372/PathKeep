//! Responsibilities: expose operation-wide read serialization without timing a real backup.
//! Not responsible for: simulating database engine admission or filesystem performance.
//! Failure mode: a publication owner blocks an otherwise independent metadata read.
use super::Archive;
use std::{
    sync::{Arc, mpsc},
    time::Duration,
};
use vault_crypto::KdfParams;

#[test]
fn a_metadata_read_does_not_wait_for_the_publication_owner() {
    let temp = tempfile::tempdir().unwrap();
    let archive = Arc::new(
        Archive::create_with_params(
            temp.path(),
            b"synthetic read concurrency password",
            KdfParams { memory_kib: 32, iterations: 1, lanes: 1 },
        )
        .unwrap(),
    );
    let publication = archive.publication_gate.lock().unwrap();
    let (ready, result) = mpsc::channel();
    let reader = archive.clone();
    let thread = std::thread::spawn(move || {
        ready.send(reader.list_sessions(&Default::default())).unwrap();
    });
    let read = result.recv_timeout(Duration::from_secs(2));
    drop(publication);
    thread.join().unwrap();
    assert!(read.expect("metadata must not wait for publication").is_ok());
}
