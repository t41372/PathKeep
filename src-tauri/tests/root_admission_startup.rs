//! Responsibilities: exercise actual desktop startup and cold CLI handoff against synthetic roots.
//! Not responsible for: rendering refusal UI, running E2E, or mocking the owner bootstrap.
#![cfg(unix)]
use std::os::unix::process::CommandExt;
use std::{
    fs,
    process::{Command, Stdio},
    time::Duration,
};
#[path = "../../tests/root_admission/support/tree.rs"]
mod tree;
const FUTURE: &str = include_str!("../../tests/root_admission/fixtures/reader-floor.json");

fn run(args: &[&str], wipe: bool) -> (String, Option<i32>, tree::Census, tree::Census) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("config.json"), FUTURE).unwrap();
    if wipe {
        fs::create_dir(root.join("archive")).unwrap();
        fs::write(root.join("archive/source-evidence.db"), "must survive refused startup").unwrap();
        fs::write(root.join(".pk-wipe-in-progress.json"), r#"{"providerIds":[],"schedule":null}"#)
            .unwrap();
    }
    start(args, temp)
}

/// Starts the real desktop binary on `temp/root` and returns once that process has ended.
fn start(
    args: &[&str],
    temp: tempfile::TempDir,
) -> (String, Option<i32>, tree::Census, tree::Census) {
    let root = temp.path().join("root");
    let before = tree::census(&root, vault_core::utils::sha256_hex);
    let child = Command::new(env!("CARGO_BIN_EXE_pathkeep-desktop"))
        .args(args)
        .process_group(0)
        .current_dir(temp.path())
        .env("CHB_PROJECT_ROOT", "root")
        .env("CHB_CHROME_USER_DATA_DIR", temp.path().join("chrome"))
        .env("CHB_FIREFOX_PROFILES_DIR", temp.path().join("firefox"))
        .env("CHB_SAFARI_ROOT", temp.path().join("safari"))
        .env("CHB_TEST_KEYRING_DIR", temp.path().join("keyring"))
        .env("PATHKEEP_PLATFORM_TEST_KEYRING_DIR", temp.path().join("keyring"))
        .env("PATHKEEP_PLATFORM_TEST_KEYRING_SERVICE", temp.path().join("key-service"))
        .env("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", temp.path().join("os"))
        .env("PATHKEEP_TEST_BLOCK_NETWORK", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output().unwrap());
    });
    // This is a process-termination bound, not a sleep/retry around an assertion.
    let (detail, code) = match rx.recv_timeout(Duration::from_secs(40)) {
        Ok(output) => (
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            output.status.code(),
        ),
        Err(_) => {
            Command::new("kill").args(["-KILL", &pid.to_string()]).status().unwrap();
            let output = rx.recv().unwrap();
            (
                format!(
                    "refused process did not terminate: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
                output.status.code(),
            )
        }
    };
    waiter.join().unwrap();
    // The cold launcher can leave a child before any owner lock/PID file exists. All
    // descendants stay in this command's private group, so cleanup never relies on
    // root contents or process-list access and never targets the test runner's group.
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let after = tree::census(&root, vault_core::utils::sha256_hex);
    (detail, code, before, after)
}

#[test]
fn background_desktop_refuses_before_owner_lock_scaffolding_and_root_logging() {
    let (detail, code, before, after) = run(&["--background"], false);
    assert!(
        code == Some(1) && detail.contains("archive-reader-too-old") && before == after,
        "exit={code:?}; {detail}; before={before:?}; after={after:?}"
    );
}

#[test]
fn cold_worker_handoff_refuses_before_launching_or_writing_startup_logs() {
    let (detail, code, before, after) = run(&["--worker", "ping"], false);
    assert!(
        code == Some(1) && detail.contains("archive-reader-too-old") && before == after,
        "exit={code:?}; {detail}; before={before:?}; after={after:?}"
    );
}

#[test]
fn refused_desktop_start_never_replays_an_interrupted_wipe() {
    let (detail, code, before, after) = run(&["--background"], true);
    assert!(
        code == Some(1) && detail.contains("archive-reader-too-old") && before == after,
        "exit={code:?}; {detail}; before={before:?}; after={after:?}"
    );
}

// Whatever the app creates when it starts on an empty data folder, before the user has set
// anything up, must still read as a first run on the next start. A file added to the start
// sequence that counted as an archive would lock every new user out of onboarding.
#[test]
fn what_a_first_start_creates_still_reads_as_a_first_run() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir(&root).unwrap();
    // The cold worker starts the app in the background and returns once its owner answers,
    // which is after the whole start sequence has run.
    let (detail, code, _, after) = start(&["--worker", "ping"], temp);
    assert_eq!(code, Some(0), "{detail}");
    assert!(
        after.entries.len() > 1,
        "the start sequence should have created its layout: {after:?}"
    );
    // `start` has dropped the folder by now, so the listing it took is replayed into a new one.
    let replay = tempfile::tempdir().unwrap();
    for (path, kind) in &after.entries {
        let target = replay.path().join(path);
        if kind.starts_with("directory") {
            fs::create_dir_all(&target).unwrap();
        } else {
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, "").unwrap();
        }
    }
    let state = vault_core::admission::inspect(replay.path());
    assert!(
        matches!(state, Ok(vault_core::admission::RootState::Uninitialized)),
        "{state:?}; a first start left: {:?}",
        after.entries.keys().collect::<Vec<_>>()
    );
}
