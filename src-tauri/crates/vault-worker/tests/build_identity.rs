//! Responsibilities: prove the real worker's identity probe precedes root resolution and IPC.
//! Not responsible for: launching a GUI, native keychain/scheduler access or release packaging.
use std::process::Command;

#[test]
fn worker_reports_development_without_resolving_even_an_empty_root() {
    let output = Command::new(env!("CARGO_BIN_EXE_pathkeep-worker"))
        .arg("--build-identity")
        .env("CHB_PROJECT_ROOT", "")
        .env("PATHKEEP_DISTRIBUTION", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let shared: serde_json::Value =
        serde_json::from_str(&vault_build::identity_report(vault_build::BinaryKind::Worker))
            .unwrap();
    assert_eq!(value, shared);
    assert_eq!(value["binary"], "worker");
    assert_eq!(value["classification"], "development");
    assert_eq!(value["defaultRootApplication"], "pathkeep-dev");
    assert_eq!(value["keyringService"], "com.yi-ting.pathkeep.dev");
    assert_eq!(value["scheduleLabel"], "com.yi-ting.pathkeep.dev.backup");
}
