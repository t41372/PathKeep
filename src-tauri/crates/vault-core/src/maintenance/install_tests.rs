//! Responsibilities: exercise each multi-file install journal boundary.
//! Not responsible for: database engines or password derivation.
//! Failure modes: lost original, mixed committed files, stale WAL, non-idempotent recovery.
use super::install::*;
use crate::project_paths_with_root;

#[test]
fn every_interrupted_swap_rolls_back() {
    for interrupted_after in 0..=4 {
        let root = tempfile::tempdir().unwrap();
        let paths = project_paths_with_root(root.path());
        let stage = root.path().join("stage");
        std::fs::create_dir_all(stage.join("archive")).unwrap();
        std::fs::create_dir_all(root.path().join("archive")).unwrap();
        for name in ["archive/history-vault.db", "archive/source-evidence.db"] {
            std::fs::write(root.path().join(name), b"old").unwrap();
            std::fs::write(stage.join(name), b"new").unwrap();
        }
        std::fs::write(root.path().join("archive/.pk-owner.lock"), b"owner").unwrap();
        let files = vec!["archive/history-vault.db".into(), "archive/source-evidence.db".into()];
        let mut journal = prepare(&paths, &stage, files, true).unwrap();
        for step in 0..interrupted_after {
            install_step(&paths, &mut journal, step).unwrap();
        }
        recover(&paths).unwrap();
        recover(&paths).unwrap();
        for name in ["archive/history-vault.db", "archive/source-evidence.db"] {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), b"old");
        }
        assert_eq!(std::fs::read(root.path().join("archive/.pk-owner.lock")).unwrap(), b"owner");
    }
}

#[test]
fn a_published_generation_finishes_cleanup_without_restoring_old_files() {
    let root = tempfile::tempdir().unwrap();
    let paths = project_paths_with_root(root.path());
    let stage = root.path().join("stage");
    std::fs::create_dir_all(root.path().join("archive")).unwrap();
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(root.path().join("config.json"), b"old").unwrap();
    std::fs::write(stage.join("config.json"), b"new").unwrap();
    let mut journal = prepare(&paths, &stage, vec!["config.json".into()], true).unwrap();
    install_step(&paths, &mut journal, 0).unwrap();
    install_step(&paths, &mut journal, 1).unwrap();
    publish(&paths, &mut journal).unwrap();
    recover(&paths).unwrap();
    recover(&paths).unwrap();
    assert_eq!(std::fs::read(root.path().join("config.json")).unwrap(), b"new");
}
