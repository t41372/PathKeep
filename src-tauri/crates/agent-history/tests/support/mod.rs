//! Responsibilities: drive the shipped CLI and retain synthetic E2E evidence.
//! Not responsible for: emulating archives, parsers or source consistency.
pub mod fixtures;

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;
use tempfile::TempDir;

pub const PASSWORD: &str = "pathkeep-synthetic-e2e-password";

pub struct Case {
    _temp: TempDir,
    pub root: PathBuf,
    pub archive: PathBuf,
    evidence: PathBuf,
    commands: Vec<Value>,
    checks: Vec<Value>,
}

impl Case {
    pub fn new(name: &str) -> Self {
        let temp = tempfile::tempdir().expect("isolated E2E directory");
        let root = temp.path().to_path_buf();
        let archive = root.join("archive");
        let evidence = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../artifacts/agent-history")
            .join(name);
        fs::create_dir_all(&evidence).expect("E2E evidence directory");
        Self { _temp: temp, root, archive, evidence, commands: vec![], checks: vec![] }
    }

    pub fn fixture(&self, fixture: Value) {
        fs::write(self.evidence.join("fixture.json"), serde_json::to_vec_pretty(&fixture).unwrap())
            .expect("repeatable fixture description");
    }

    pub fn init(&mut self) {
        self.call("init", vec![]);
    }

    pub fn call(&mut self, command: &str, arguments: Vec<String>) -> Value {
        self.invoke(command, arguments, PASSWORD, true)
    }

    pub fn fail(&mut self, command: &str, arguments: Vec<String>) -> Value {
        self.invoke(command, arguments, PASSWORD, false)
    }

    pub fn wrong_password(&mut self) -> Value {
        self.invoke("list", vec![], "different-password", false)
    }

    fn invoke(
        &mut self,
        command: &str,
        mut arguments: Vec<String>,
        password: &str,
        expect_success: bool,
    ) -> Value {
        if command != "plan" {
            arguments.extend([
                "--archive".into(),
                self.archive.to_string_lossy().into_owned(),
                "--password-stdin".into(),
            ]);
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_pathkeep-agent-history"))
            .arg(command)
            .args(&arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the real agent-history CLI");
        if let Err(error) = child.stdin.take().unwrap().write_all(password.as_bytes()) {
            // Validation may reject a command before it needs to consume a password.
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe, "write CLI password");
        }
        let output = child.wait_with_output().expect("complete CLI command");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 JSON stdout");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let parsed = serde_json::from_str::<Value>(&stdout);
        self.commands.push(json!({
            "command": command, "arguments": arguments,
            "expected_success": expect_success,
            "exit_code": output.status.code(),
            "stdout": parsed.as_ref().ok(),
            "invalid_json_stdout": parsed.as_ref().err().map(|_| &stdout),
            "stderr": stderr,
        }));
        assert_eq!(
            output.status.success(),
            expect_success,
            "{command}: stdout={stdout}; stderr={stderr}"
        );
        let parsed = parsed.expect("each CLI invocation returns one JSON document");
        if !expect_success {
            assert!(parsed["error"]["code"].is_string(), "stable error code: {parsed}");
        }
        parsed
    }

    pub fn equal(&mut self, name: &str, actual: Value, expected: Value) {
        self.checks.push(json!({"name":name,"expected":expected,"actual":actual}));
        assert_eq!(actual, expected, "{name}");
    }

    pub fn truth(&mut self, name: &str, actual: bool) {
        self.equal(name, json!(actual), json!(true));
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let report = json!({
            "schema_version":1,
            "outcome":if std::thread::panicking() { "failed" } else { "passed" },
            "commands":self.commands,
            "checks":self.checks,
            "private_data":false,
        });
        if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
            let _ = fs::write(self.evidence.join("report.json"), bytes);
        }
    }
}

pub fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub fn items(document: &Value) -> &[Value] {
    document["items"].as_array().expect("paged items")
}

pub fn string<'a>(document: &'a Value, field: &str) -> &'a str {
    document[field].as_str().unwrap_or_else(|| panic!("missing {field}: {document}"))
}

pub fn identifier(document: &Value, field: &str) -> String {
    match &document[field] {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        _ => panic!("missing identifier {field}: {document}"),
    }
}

pub fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Snapshot only the small synthetic source tree; archive scale is tested by the CLI.
pub fn source_state(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    fn visit(root: &Path, result: &mut BTreeMap<PathBuf, (Vec<u8>, SystemTime)>) {
        for entry in fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                visit(&entry.path(), result);
            } else if kind.is_file() {
                result.insert(
                    entry.path(),
                    (
                        fs::read(entry.path()).unwrap(),
                        entry.metadata().unwrap().modified().unwrap(),
                    ),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, &mut result);
    result
}

pub fn archive_contains(root: &Path, sentinel: &[u8]) -> bool {
    fs::read_dir(root).unwrap().any(|entry| {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            archive_contains(&entry.path(), sentinel)
        } else {
            fs::read(entry.path()).unwrap().windows(sentinel.len()).any(|part| part == sentinel)
        }
    })
}
