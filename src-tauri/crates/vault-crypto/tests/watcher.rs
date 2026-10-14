//! Responsibilities: keep plaintext-leak scans valid when a filename is atomically replaced.
//! Not responsible for: cryptography or capturing unobserved transient writes.
//! Failure modes: an initial hard link hides the final file at the same path.
mod common;

#[test]
fn an_atomic_replacement_is_scanned_alongside_the_retained_original() {
    let root = tempfile::tempdir().unwrap();
    let watched = root.path().join("watched");
    let quarantine = root.path().join("quarantine");
    std::fs::create_dir(&watched).unwrap();
    let path = watched.join("meta.json");
    std::fs::write(&path, b"initial-control").unwrap();
    let watcher = common::Watcher::start(vec![watched.clone()], quarantine.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::fs::read_dir(&quarantine).unwrap().next().is_none() {
        assert!(std::time::Instant::now() < deadline, "watcher did not observe initial file");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let replacement = root.path().join("replacement");
    std::fs::write(&replacement, b"final-control").unwrap();
    std::fs::rename(replacement, &path).unwrap();
    let files = watcher.finish(&[b"initial-control", b"final-control"]);
    let hits: Vec<_> = files.into_iter().flat_map(|(_, hits)| hits).collect();
    assert!(hits.iter().any(|hit| hit == "initial-control"));
    assert!(hits.iter().any(|hit| hit == "final-control"));
}
