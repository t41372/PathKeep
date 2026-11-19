//! Model verification, installation and restart contracts, using small local bundles.
use super::*;

fn leak_manifest(entries: &[(&'static str, &[u8])]) -> Vec<ModelFile> {
    entries
        .iter()
        .map(|(name, bytes)| {
            let digest = crate::utils::sha256_hex(bytes);
            ModelFile { name, sha256: Box::leak(digest.into_boxed_str()), repo: "Stub/Repo" }
        })
        .collect()
}

#[test]
fn model_is_present_and_verified_requires_all_files_and_matching_digests() {
    let dir = tempfile::tempdir().expect("tempdir");
    let model_dir = dir.path();
    let files = leak_manifest(&[("a.txt", b"alpha"), ("b.txt", b"beta")]);

    // Missing files → not present.
    assert!(!model_is_present_and_verified(model_dir, &files));

    std::fs::write(model_dir.join("a.txt"), b"alpha").expect("write a");
    // Still missing b → not present.
    assert!(!model_is_present_and_verified(model_dir, &files));

    std::fs::write(model_dir.join("b.txt"), b"WRONG").expect("write b wrong");
    // b present but wrong digest → not verified.
    assert!(!model_is_present_and_verified(model_dir, &files));

    std::fs::write(model_dir.join("b.txt"), b"beta").expect("write b right");
    // All present + verified.
    assert!(model_is_present_and_verified(model_dir, &files));
}

// How `model_files_verified` could go wrong, written down before it was:
// - an unchanged model is read again on every check (the hundreds of MB it exists to save);
// - a file whose length or modification time changed is trusted without a hash;
// - a marker from before the stamps, or none at all, keeps a good model unavailable or keeps
//   it hashed on every check;
// - a manifest that pins another digest trusts files that matched the old one;
// - a missing file passes.

/// Writes `bytes` over `path`, then gives it `modified` as its modification time.
fn write_with_mtime(path: &std::path::Path, bytes: &[u8], modified: std::time::SystemTime) {
    std::fs::write(path, bytes).expect("write file");
    std::fs::File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(modified))
        .expect("set modification time");
}

fn mtime(path: &std::path::Path) -> std::time::SystemTime {
    std::fs::metadata(path).and_then(|meta| meta.modified()).expect("modification time")
}

/// Two files that verify, and a time unlike any the test writes them with.
fn verified_pair() -> (tempfile::TempDir, Vec<ModelFile>, std::time::SystemTime) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("a.txt"), b"alpha").expect("write a");
    std::fs::write(dir.path().join("b.txt"), b"beta").expect("write b");
    let files = leak_manifest(&[("a.txt", b"alpha"), ("b.txt", b"beta")]);
    let other_time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(86_400);
    (dir, files, other_time)
}

// Swapping bytes while keeping a file's length and time stands in for "was it read?": a check
// that hashed would see the swap, one that trusted the stamp does not.
#[test]
fn model_files_verified_trusts_unchanged_stamps_and_hashes_changed_files() {
    let (dir, files, other_time) = verified_pair();
    let b = dir.path().join("b.txt");
    assert!(model_files_verified(dir.path(), &files), "first check hashes and passes");

    write_with_mtime(&b, b"BETA", mtime(&b));
    assert!(model_files_verified(dir.path(), &files), "same length and time: not read again");

    write_with_mtime(&b, b"BETA", other_time);
    assert!(!model_files_verified(dir.path(), &files), "another time: hashed, and wrong");
    write_with_mtime(&b, b"beta", other_time);
    assert!(model_files_verified(dir.path(), &files), "hashed again and right");
    write_with_mtime(&b, b"BETA", other_time);
    assert!(model_files_verified(dir.path(), &files), "the new stamp was recorded");

    write_with_mtime(&b, b"beta!", other_time);
    assert!(!model_files_verified(dir.path(), &files), "another length: hashed, and wrong");
    std::fs::remove_file(&b).expect("remove b");
    assert!(!model_files_verified(dir.path(), &files), "a missing file never passes");
}

#[test]
fn model_files_verified_replaces_an_old_or_missing_marker_after_one_hash() {
    for old_marker in [Some(&b"ok"[..]), None] {
        let (dir, files, _) = verified_pair();
        if let Some(contents) = old_marker {
            std::fs::write(dir.path().join(VERIFIED_MARKER_FILE), contents).expect("marker");
        }
        assert!(model_files_verified(dir.path(), &files), "a good model stays available");
        let b = dir.path().join("b.txt");
        write_with_mtime(&b, b"BETA", mtime(&b));
        assert!(model_files_verified(dir.path(), &files), "the stamps replaced {old_marker:?}");
    }
}

#[test]
fn model_files_verified_does_not_carry_stamps_to_another_pinned_digest() {
    let (dir, files, _) = verified_pair();
    assert!(model_files_verified(dir.path(), &files));
    let repinned = leak_manifest(&[("a.txt", b"alpha"), ("b.txt", b"BETA")]);
    assert!(!model_files_verified(dir.path(), &repinned), "b.txt never matched the new pin");
    assert!(model_files_verified(dir.path(), &files), "the old pin still holds");
}

#[test]
fn model_files_present_requires_every_file_non_empty_and_no_marker() {
    let dir = tempfile::tempdir().expect("tempdir");
    let model_dir = dir.path();
    let files = leak_manifest(&[("model.safetensors", b"weights"), ("tokenizer.json", b"tok")]);
    assert!(!model_files_present(model_dir, &files));
    std::fs::write(model_dir.join("model.safetensors"), b"weights").expect("write w");
    assert!(!model_files_present(model_dir, &files), "one file is still missing");
    std::fs::write(model_dir.join("tokenizer.json"), b"tok").expect("write tok");
    assert!(model_files_present(model_dir, &files), "verification is the load's job");
    std::fs::write(model_dir.join("model.safetensors"), b"").expect("truncate");
    assert!(!model_files_present(model_dir, &files), "an empty file is not a download");
}

// ---- download path (consent / offline-first / cancel / verify / progress) ----

/// A test manifest whose pinned digests match the download stub's synthetic bytes.
fn stub_manifest() -> Vec<ModelFile> {
    // The digests must equal the SHA-256 of `stub_file_bytes(name)` so a stub download verifies.
    ["config.json", "tokenizer.json", "model.safetensors"]
        .into_iter()
        .map(|name| {
            let digest = crate::utils::sha256_hex(&stub_file_bytes(name));
            ModelFile { name, sha256: Box::leak(digest.into_boxed_str()), repo: "Stub/Repo" }
        })
        .collect()
}

#[derive(Default)]
struct RecordingProgress {
    started: Vec<String>,
    finished: Vec<String>,
    cancel_after: Option<usize>,
}

impl ModelDownloadProgress for RecordingProgress {
    fn file_started(&mut self, file: &str, _total_bytes: u64) {
        self.started.push(file.to_string());
    }
    fn file_finished(&mut self, file: &str) {
        self.finished.push(file.to_string());
    }
    fn cancelled(&self) -> bool {
        self.cancel_after.is_some_and(|threshold| self.started.len() >= threshold)
    }
}

#[test]
fn ensure_download_refuses_without_consent_when_absent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = crate::config::project_paths_with_root(dir.path());
    let manifest = stub_manifest();
    let mut progress = NoopDownloadProgress;
    let error = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, false, &mut progress)
        .expect_err("no consent");
    assert!(error.to_string().contains("requires explicit consent"));
}

#[test]
fn ensure_download_is_offline_first_when_already_verified() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = crate::config::project_paths_with_root(dir.path());
    let manifest = stub_manifest();
    // First download places + verifies the files.
    let mut first = RecordingProgress::default();
    let initial =
        ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut first).expect("first");
    // The download's stamps are ones loads and status reads trust: bytes swapped under the
    // same length and time are not read again.
    let config = initial.join("config.json");
    let (original, modified) = (std::fs::read(&config).expect("config"), mtime(&config));
    write_with_mtime(&config, &vec![b'x'; original.len()], modified);
    assert!(model_files_verified(config.parent().expect("model dir"), &manifest));
    write_with_mtime(&config, &original, modified);
    // A second call WITHOUT consent succeeds (already present) and reports no new progress —
    // proving the offline-first reuse path runs without touching the fetch.
    let mut second = RecordingProgress::default();
    let model_dir = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, false, &mut second)
        .expect("offline reuse");
    assert!(model_is_present_and_verified(&model_dir, &manifest));
    assert!(second.started.is_empty(), "no file should be re-fetched");
}

#[test]
fn ensure_download_rejects_digest_mismatch_and_removes_blob() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = crate::config::project_paths_with_root(dir.path());
    // A manifest whose digest does NOT match the stub bytes → verification fails, blob removed.
    let wrong: &'static str = Box::leak("a".repeat(64).into_boxed_str());
    let manifest = vec![ModelFile { name: "config.json", sha256: wrong, repo: "Stub/Repo" }];
    let mut progress = NoopDownloadProgress;
    let error = ensure_model_downloaded(&paths, "Stub/Repo", &manifest, true, &mut progress)
        .expect_err("digest mismatch");
    assert!(error.to_string().contains("failed SHA-256 verification"));
    let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
    assert!(!model_dir.join("config.json").exists(), "bad blob must be removed");
}

// A repair must not truncate an inode that an active embedding engine still maps. Failed
// verification must leave that inode alone too, and temporary downloads must be removed.
#[test]
fn ensure_download_replaces_files_without_changing_open_readers() {
    use std::io::Read;

    let dir = tempfile::tempdir().expect("tempdir");
    let paths = crate::config::project_paths_with_root(dir.path());
    let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
    std::fs::create_dir_all(&model_dir).expect("create model dir");
    let path = model_dir.join("model.safetensors");
    std::fs::write(&path, b"old model bytes").expect("write old file");
    let mut reader = std::fs::File::open(&path).expect("open old file");
    let installed = ensure_model_downloaded(
        &paths,
        "Stub/Repo",
        &stub_manifest(),
        true,
        &mut NoopDownloadProgress,
    )
    .expect("repair model");
    let mut old_bytes = Vec::new();
    reader.read_to_end(&mut old_bytes).expect("read old file");
    assert_eq!(old_bytes, b"old model bytes");
    assert_ne!(installed, model_dir);
    assert_eq!(
        crate::ai::open_model_directory(&paths, "Stub/Repo").unwrap().path(),
        installed.canonicalize().unwrap()
    );
    assert_eq!(
        std::fs::read(installed.join("model.safetensors")).expect("new file"),
        stub_file_bytes("model.safetensors")
    );
    assert_eq!(std::fs::read_dir(&installed).expect("directory").count(), 4);
}

#[test]
fn ensure_download_preserves_mapped_bytes_and_switches_the_whole_bundle() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let files = stub_manifest();
    let flat = model_dir_for_repo(&paths, "Stub/Repo");
    std::fs::create_dir_all(&flat).unwrap();
    std::fs::write(flat.join("model.safetensors"), b"previous release").unwrap();
    let mut previous = files.clone();
    previous[2].sha256 = Box::leak(crate::utils::sha256_hex(b"previous release").into_boxed_str());
    let old =
        ensure_model_downloaded(&paths, "Stub/Repo", &previous, true, &mut NoopDownloadProgress)
            .unwrap();
    assert!(model_is_present_and_verified(&old, &previous));
    let path = old.join("model.safetensors");
    let file = std::fs::File::open(&path).unwrap();
    // SAFETY: the old weights are immutable throughout publication.
    let mapping = unsafe { memmap2::Mmap::map(&file) }.unwrap();
    drop(file);
    let new = ensure_model_downloaded(&paths, "Stub/Repo", &files, true, &mut NoopDownloadProgress)
        .unwrap();
    assert_eq!(&mapping[..], b"previous release");
    assert_ne!(new, old);
    assert!(model_is_present_and_verified(&new, &files));
    assert_eq!(std::fs::read_dir(&new).unwrap().count(), 4);
    let found =
        ensure_model_downloaded(&paths, "Stub/Repo", &files, false, &mut NoopDownloadProgress)
            .unwrap();
    assert_eq!(found, new);
    drop(mapping);
    ensure_model_downloaded(&paths, "Stub/Repo", &files, false, &mut NoopDownloadProgress).unwrap();
    assert!(!old.exists(), "retired files are collected once an external mapping is gone");
}

#[test]
fn valid_flat_bundle_is_adopted_without_fetching() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let flat = model_dir_for_repo(&paths, crate::DEFAULT_STATIC_MODEL_REPO);
    std::fs::create_dir_all(&flat).unwrap();
    let files = stub_manifest();
    for file in &files {
        std::fs::write(flat.join(file.name), stub_file_bytes(file.name)).unwrap();
    }
    let abandoned = flat.join("gen-00000000000000000000000000000000");
    std::fs::create_dir(&abandoned).unwrap();
    let config = crate::AppConfig { initialized: true, ..Default::default() };
    std::fs::write(&paths.config_path, crate::config::encode_config(&config, None).unwrap())
        .unwrap();
    let vault = crate::Vault::open(paths.clone(), config, None).unwrap();
    assert!(!abandoned.exists(), "the live owner recovered the model on archive open");
    let selected =
        crate::ai::open_model_directory(&paths, crate::DEFAULT_STATIC_MODEL_REPO).unwrap();
    assert_eq!(selected.path(), flat.canonicalize().unwrap());
    assert!(model_files_verified(selected.path(), &files));
    let mut progress = RecordingProgress::default();
    let installed = ensure_model_downloaded(
        &paths,
        crate::DEFAULT_STATIC_MODEL_REPO,
        &files,
        false,
        &mut progress,
    )
    .unwrap();
    assert!(model_is_present_and_verified(&installed, &files));
    assert!(progress.started.is_empty());
    vault.close().unwrap();
}

#[test]
fn restart_recovers_each_model_publication_boundary() {
    const TEST: &str = "ai::model_files::tests::restart_recovers_each_model_publication_boundary";
    const CHILD: &str = "PATHKEEP_MODEL_CRASH_TEST_ROOT";
    const CUT: &str = "PATHKEEP_MODEL_CRASH_TEST_CUT";
    if let Some(root) = std::env::var_os(CHILD) {
        let paths = crate::project_paths_with_root(std::path::Path::new(&root));
        crate::fault_inject::exit_process_at_next_checkpoint(&std::env::var(CUT).unwrap());
        ensure_model_downloaded(
            &paths,
            "Stub/Repo",
            &stub_manifest(),
            true,
            &mut NoopDownloadProgress,
        )
        .unwrap();
        panic!("installer missed the crash point");
    }
    for cut in ["model-generation-verified", "model-generation-published"] {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(root.path());
        let files = stub_manifest();
        let old = ensure_model_downloaded(
            &paths,
            "Stub/Repo",
            &files[..1],
            true,
            &mut NoopDownloadProgress,
        )
        .unwrap();
        let exit = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST])
            .env(CHILD, root.path())
            .env(CUT, cut)
            .status()
            .unwrap();
        assert_eq!(exit.code(), Some(86), "{cut}");
        let repo = model_dir_for_repo(&paths, "Stub/Repo");
        let pointed = std::fs::read_to_string(repo.join("current")).unwrap();
        assert_eq!(repo.join(pointed) == old, cut == "model-generation-verified");
        assert!(old.is_dir());
        let partial = repo.join("gen-00000000000000000000000000000000");
        std::fs::create_dir(&partial).unwrap();
        std::fs::write(partial.join("config.json"), b"partial").unwrap();
        // Run the same recovery as live archive open, before any new install request.
        super::super::model_generations::recover(&repo, &files, &transfers::active()).unwrap();
        let selection = crate::ai::open_model_directory(&paths, "Stub/Repo").unwrap();
        let new = selection.path();
        assert!(model_is_present_and_verified(new, &files));
        assert!(!old.exists());
        assert!(!partial.exists());
        assert_eq!(std::fs::read_dir(repo).unwrap().count(), 2, "only current and its directory");
    }
}

#[test]
fn failed_verification_discards_the_entire_candidate() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::project_paths_with_root(root.path());
    let mut files = stub_manifest();
    let old =
        ensure_model_downloaded(&paths, "Stub/Repo", &files[..1], true, &mut NoopDownloadProgress)
            .unwrap();
    files[2].sha256 = "wrong";
    assert!(
        ensure_model_downloaded(&paths, "Stub/Repo", &files, true, &mut NoopDownloadProgress)
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&old).unwrap().count(), 2);
    assert!(model_is_present_and_verified(&old, &files[..1]));
    let repo = model_dir_for_repo(&paths, "Stub/Repo");
    assert_eq!(repo.join(std::fs::read_to_string(repo.join("current")).unwrap()), old);
    assert_eq!(std::fs::read_dir(repo).unwrap().count(), 2);
}

#[test]
fn ensure_download_failed_verification_preserves_existing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = crate::config::project_paths_with_root(dir.path());
    let model_dir = model_dir_for_repo(&paths, "Stub/Repo");
    std::fs::create_dir_all(&model_dir).expect("create model dir");
    let path = model_dir.join("model.safetensors");
    std::fs::write(&path, b"old model bytes").expect("write old file");
    let files = [ModelFile { name: "model.safetensors", sha256: "wrong", repo: "Stub/Repo" }];
    ensure_model_downloaded(&paths, "Stub/Repo", &files, true, &mut NoopDownloadProgress)
        .expect_err("reject bad download");
    assert_eq!(std::fs::read(path).expect("old file"), b"old model bytes");
    assert_eq!(std::fs::read_dir(model_dir).expect("directory").count(), 1);
}

/// Pause with a candidate file open. The test thread runs a real archive operation before
/// the transfer can finish, without depending on scheduling or the speed of a network.
fn during_model_transfer(
    paths: &ProjectPaths,
    operation: impl FnOnce(),
) -> Result<std::path::PathBuf> {
    use std::sync::mpsc;
    use std::time::Duration;

    struct PauseOnce {
        arrived: Option<mpsc::Sender<()>>,
        resume: mpsc::Receiver<()>,
    }
    impl ModelDownloadProgress for PauseOnce {
        fn file_progress(&mut self, _: &str, _: u64, _: u64) {
            if let Some(arrived) = self.arrived.take() {
                arrived.send(()).unwrap();
                self.resume.recv_timeout(Duration::from_secs(60)).unwrap();
            }
        }
    }

    std::thread::scope(|scope| {
        let (arrived, paused) = mpsc::channel();
        let (resume, wait) = mpsc::channel();
        let mut progress = PauseOnce { arrived: Some(arrived), resume: wait };
        let install = scope.spawn(move || {
            ensure_model_downloaded(
                paths,
                crate::DEFAULT_STATIC_MODEL_REPO,
                &stub_manifest(),
                true,
                &mut progress,
            )
        });
        paused.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(
            crate::archive::ArchiveOpGate::try_acquire(paths).is_some(),
            "paused transfer owns the archive gate"
        );
        operation();
        resume.send(()).unwrap();
        install.join().unwrap()
    })
}

#[test]
fn wipe_finishes_while_a_model_transfer_is_paused() {
    for install_again in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::project_paths_with_root(root.path());
        let repo = model_dir_for_repo(&paths, crate::DEFAULT_STATIC_MODEL_REPO);
        let mut replacement = None;
        let error = during_model_transfer(&paths, || {
            crate::wipe_all_data(
                &paths,
                |_| Ok(Default::default()),
                || Ok(()),
                |_| Ok(()),
                |_| Ok(()),
            )
            .expect("wipe finishes before the transfer resumes");
            assert!(!paths.models_dir.exists());
            if install_again {
                replacement = Some(
                    ensure_model_downloaded(
                        &paths,
                        crate::DEFAULT_STATIC_MODEL_REPO,
                        &stub_manifest(),
                        true,
                        &mut NoopDownloadProgress,
                    )
                    .expect("a fresh installation after wipe"),
                );
            }
        })
        .expect_err("a wiped transfer cannot publish");
        assert!(error.to_string().contains("wiped"), "{error:#}");
        if let Some(replacement) = replacement {
            assert_eq!(
                repo.join(std::fs::read_to_string(repo.join("current")).unwrap()),
                replacement
            );
            assert!(model_is_present_and_verified(&replacement, &stub_manifest()));
            assert_eq!(
                std::fs::read_dir(repo).unwrap().count(),
                2,
                "only the new install survived"
            );
        } else {
            assert!(!paths.models_dir.exists(), "installer recreated the model tree");
        }
    }
}

#[test]
fn snapshot_and_restore_preserve_an_in_flight_model_transfer() {
    let vault = crate::Vault::open_temp().unwrap();
    let paths = vault.paths();
    let repo = model_dir_for_repo(paths, crate::DEFAULT_STATIC_MODEL_REPO);
    let installed = during_model_transfer(paths, || {
        let copy = paths.raw_snapshots_dir.join("maintenance/model-transfer");
        crate::maintenance::snapshot(&vault, &copy).unwrap();
        let copy_paths = crate::project_paths_with_root(&copy);
        assert!(
            !model_dir_for_repo(&copy_paths, crate::DEFAULT_STATIC_MODEL_REPO).exists(),
            "the safety copy must omit the live model repository"
        );
        let restored =
            crate::maintenance::restore_snapshot(&vault, &copy, None, Default::default()).unwrap();
        assert_eq!(
            std::fs::read_dir(&repo).unwrap().count(),
            1,
            "restore retained the private candidate"
        );
        assert!(!repo.join("current").exists(), "restore published an incomplete model");
        restored.vault.close().unwrap();
    })
    .expect("the transfer publishes after the archive reopens");
    assert!(model_is_present_and_verified(&installed, &stub_manifest()));
    assert_eq!(repo.join(std::fs::read_to_string(repo.join("current")).unwrap()), installed);
}
