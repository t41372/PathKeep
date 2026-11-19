//! Exercise the real worker against a server that keeps its socket open during a stall.
#![cfg(debug_assertions)]

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::mpsc,
    time::Duration,
};
use vault_core::{ModelDownloadUpdate, ai::download_static_embedding_model};

#[test]
fn stalled_model_response_fails_and_releases_its_candidate() {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let mut config = vault_core::AppConfig::default();
    config.ai.enabled = true;
    config.ai.semantic_index_enabled = true;
    vault_core::save_config(&vault_core::ControlGate::acquire(&paths), &config).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    // This integration binary has a private environment and starts just this worker.
    unsafe {
        std::env::set_var("CHB_PROJECT_ROOT", root.path());
        std::env::set_var(
            "PATHKEEP_TEST_MODEL_SOURCE",
            format!("http://{}/v1", listener.local_addr().unwrap()),
        );
    }
    let (release, held) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let (mut peer, _) = listener.accept().unwrap();
        for line in BufReader::new(&peer).lines() {
            if line.unwrap().is_empty() {
                break;
            }
        }
        peer.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nx").unwrap();
        held.recv().ok();
    });
    let (tx, rx) = mpsc::channel();
    let _transfer = download_static_embedding_model(move |event| {
        let _ = tx.send(event.update);
    })
    .unwrap();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ModelDownloadUpdate::FileStarted { .. }
    ));
    let terminal = rx.recv_timeout(Duration::from_secs(35));
    drop(release);
    server.join().unwrap();
    assert!(
        matches!(terminal, Ok(ModelDownloadUpdate::Error { .. })),
        "stalled read must fail while the socket is still open: {terminal:?}"
    );
    let repo = vault_core::model_dir_for_repo(&paths, vault_core::DEFAULT_STATIC_MODEL_REPO);
    assert_eq!(std::fs::read_dir(repo).unwrap().count(), 0);
}
