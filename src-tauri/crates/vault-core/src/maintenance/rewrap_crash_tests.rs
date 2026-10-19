//! Responsibilities: kill the real password-change path at each atomic envelope boundary.
//! Not responsible for: testing AES or simulating a disk controller power failure.
//! Failure modes: neither password works after a crash; data key changes; a stale temp
//! prevents the next unlock. Uses real project roots and native encrypted Vaults.
use crate::{Vault, project_paths_with_root};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

#[test]
fn rewrap_survives_a_process_kill_at_every_envelope_boundary() {
    for step in ["temp-written", "temp-synced", "renamed", "directory-synced"] {
        let (_root, vault, key) = super::tests::encrypted();
        let paths = vault.paths().clone();
        vault.close().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "maintenance::rewrap_crash_tests::rewrap_child",
                "--ignored",
                "--nocapture",
            ])
            .env("PATHKEEP_REWRAP_ROOT", &paths.app_root)
            .env("VAULT_CRYPTO_CRASH_AT", format!("atomic-replace/{step}@keys.envelope:1"))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.unwrap();
                if line.contains("crash-point") {
                    let _ = send.send(());
                }
            }
        });
        let reached = receive.recv_timeout(Duration::from_secs(60));
        child.kill().unwrap();
        child.wait().unwrap();
        reader.join().unwrap();
        reached.expect("the named production envelope boundary must be reached");
        let recovered = Vault::unlock_data_key(&paths, "old password")
            .or_else(|_| Vault::unlock_data_key(&paths, "new password"))
            .unwrap();
        assert_eq!(recovered.expose_bytes(), key.expose_bytes());
        let opened = Vault::open(paths, vault.config().clone(), Some(*key.expose_bytes())).unwrap();
        opened.close().unwrap();
    }
}

#[test]
#[ignore = "child process for the envelope crash test"]
fn rewrap_child() {
    let paths = project_paths_with_root(std::path::Path::new(
        &std::env::var("PATHKEEP_REWRAP_ROOT").unwrap(),
    ));
    let config = crate::load_config(&paths).unwrap();
    let (vault, _) = Vault::open_password(paths, config, "old password", None).unwrap();
    super::rewrap_password(&vault, "old password", "new password").unwrap();
}
