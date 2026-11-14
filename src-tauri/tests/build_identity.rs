//! Responsibilities: prove the actual desktop binary reports before opening any root or GUI.
//! Not responsible for: building or publishing distribution packages.
use std::process::Command;

#[test]
fn desktop_reports_the_same_development_defaults_as_the_shared_worker_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_pathkeep-desktop"))
        .arg("--build-identity")
        .env("CHB_PROJECT_ROOT", "")
        .env("PATHKEEP_DISTRIBUTION", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut worker: serde_json::Value =
        serde_json::from_str(&vault_build::identity_report(vault_build::BinaryKind::Worker))
            .unwrap();
    worker["binary"] = "desktop".into();
    assert_eq!(actual, worker);
    assert_eq!(actual["classification"], "development");
    assert_eq!(actual["defaultRootApplication"], "pathkeep-dev");
}
