//! Responsibilities: compile mutated copies of the real declarations and catalog to prove omissions fail.
//! Not responsible for: runtime archive coverage, cryptography or the independent disk oracle.
//! Failure modes: a declared artifact has no arm, a database has no mapping, an operation gains
//! a field without row answers, or one row omits an existing mandatory answer.

use std::{fs, path::PathBuf, process::Command};

struct Sources {
    root: PathBuf,
}
impl Sources {
    fn new(case: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/layout-compile-fail")
            .join(format!("{}-{case}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for name in ["lib.rs", "identity.rs", "policies.rs", "catalog.rs", "paths.rs"] {
            fs::copy(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join(name),
                root.join(name),
            )
            .unwrap();
        }
        Self { root }
    }
    fn replace(&self, name: &str, from: &str, to: &str) {
        let path = self.root.join(name);
        let source = fs::read_to_string(&path).unwrap();
        assert!(source.contains(from), "mutation anchor missing: {name}: {from}");
        fs::write(path, source.replacen(from, to, 1)).unwrap();
    }
    fn compile(&self, case: &str) -> (bool, String) {
        let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args([
                "--edition=2024",
                "--crate-name=layout_negative",
                "--crate-type=lib",
                "--emit=metadata",
            ])
            .arg(self.root.join("lib.rs"))
            .arg("-o")
            .arg(self.root.join("negative.rmeta"))
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        let artifacts =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../artifacts/vault-layout");
        fs::create_dir_all(&artifacts).unwrap();
        fs::write(artifacts.join(format!("compile-fail-{case}.txt")), &stderr).unwrap();
        (output.status.success(), stderr)
    }
}
impl Drop for Sources {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_declared_artifact_without_a_policy_arm_does_not_compile() {
    let source = Sources::new("artifact");
    source.replace("identity.rs", "ArtifactId {", "ArtifactId { Unclassified = 999,");
    let (success, error) = source.compile("artifact");
    assert!(!success);
    assert!(error.contains("E0004") && error.contains("Unclassified"), "{error}");
}

#[test]
fn a_database_without_an_artifact_mapping_does_not_compile() {
    let source = Sources::new("database");
    source.replace("identity.rs", "Scratch = 4", "Scratch = 4, Unmapped = 5");
    let (success, error) = source.compile("database");
    assert!(!success);
    assert!(error.contains("E0004") && error.contains("Unmapped"), "{error}");
}

#[test]
fn a_new_operation_requires_an_answer_in_every_row() {
    let source = Sources::new("operation");
    source.replace(
        "policies.rs",
        "InitializeEnvelope => prevents_new_envelope: bool,",
        "InitializeEnvelope => prevents_new_envelope: bool, Inventory => inventory: bool,",
    );
    let (success, error) = source.compile("operation");
    assert!(!success);
    assert!(error.contains("E0063") && error.contains("inventory"), "{error}");
}

#[test]
fn a_row_cannot_omit_its_snapshot_answer() {
    let source = Sources::new("row");
    source.replace("catalog.rs", "snapshot: SnapshotPolicy::CopyDatabase,", "");
    let (success, error) = source.compile("row");
    assert!(!success);
    assert!(error.contains("no rules expected") && error.contains("restore"), "{error}");
}
