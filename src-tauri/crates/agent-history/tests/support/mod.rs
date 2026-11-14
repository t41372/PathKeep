//! Responsibilities: drive the shipped CLI and retain synthetic E2E evidence.
//! Not responsible for: emulating archives, parsers or source consistency.
pub mod fixtures;
pub mod harness;
pub mod native;

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
    /// Whether this case's archive takes a password; see `Case::unencrypted`.
    encrypted: bool,
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
        Self {
            _temp: temp,
            root,
            archive,
            encrypted: true,
            evidence,
            commands: vec![],
            checks: vec![],
        }
    }

    pub fn fixture(&self, fixture: Value) {
        fs::write(self.evidence.join("fixture.json"), serde_json::to_vec_pretty(&fixture).unwrap())
            .expect("repeatable fixture description");
    }

    pub fn init(&mut self) {
        if self.encrypted {
            self.call("init", vec![]);
        } else {
            self.call("init", args(&["--no-encryption"]));
        }
    }

    pub fn call(&mut self, command: &str, arguments: Vec<String>) -> Value {
        let password = self.password();
        self.invoke(command, arguments, password, true)
    }

    pub fn fail(&mut self, command: &str, arguments: Vec<String>) -> Value {
        let password = self.password();
        self.invoke(command, arguments, password, false)
    }

    pub fn wrong_password(&mut self) -> Value {
        self.invoke("list", vec![], Some("different-password"), false)
    }

    fn password(&self) -> Option<&'static str> {
        self.encrypted.then_some(PASSWORD)
    }

    fn invoke(
        &mut self,
        command: &str,
        arguments: Vec<String>,
        password: Option<&str>,
        expect_success: bool,
    ) -> Value {
        let archive = self.archive.clone();
        let run = self.run(&archive, command, arguments, password, &[], Some(expect_success));
        let (success, stdout) = (run.success, run.stdout);
        let parsed = serde_json::from_str::<Value>(&stdout)
            .expect("each CLI invocation returns one JSON document");
        if !success {
            assert!(parsed["error"]["code"].is_string(), "stable error code: {parsed}");
        }
        parsed
    }

    /// Runs the real binary once and records it. `expect_success: None` records
    /// the outcome without asserting it, for callers that judge it themselves.
    fn run(
        &mut self,
        archive: &Path,
        command: &str,
        mut arguments: Vec<String>,
        password: Option<&str>,
        environment: &[(&str, &str)],
        expect_success: Option<bool>,
    ) -> Run {
        if command != "plan" {
            arguments.extend(["--archive".into(), archive.to_string_lossy().into_owned()]);
            if password.is_some() {
                arguments.push("--password-stdin".into());
            }
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_pathkeep-agent-history"))
            .arg(command)
            .args(&arguments)
            .envs(environment.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start the real agent-history CLI");
        let mut stdin = child.stdin.take().unwrap();
        if let Some(password) = password {
            if let Err(error) = stdin.write_all(password.as_bytes()) {
                // Validation may reject a command before it needs to consume a password.
                assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe, "write CLI password");
            }
        }
        drop(stdin);
        let output = child.wait_with_output().expect("complete CLI command");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 JSON stdout");
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let parsed = serde_json::from_str::<Value>(&stdout);
        self.commands.push(json!({
            "command": command, "arguments": arguments,
            "environment": environment.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
            "archive": archive,
            "expected_success": expect_success,
            "exit_code": output.status.code(),
            "stdout": parsed.as_ref().ok(),
            "invalid_json_stdout": parsed.as_ref().err().map(|_| &stdout),
            "stderr": stderr,
        }));
        if let Some(expect_success) = expect_success {
            assert_eq!(
                output.status.success(),
                expect_success,
                "{command}: stdout={stdout}; stderr={stderr}"
            );
        }
        Run { success: output.status.success(), stdout }
    }

    pub fn equal(&mut self, name: &str, actual: Value, expected: Value) {
        self.checks.push(json!({"name":name,"expected":expected,"actual":actual}));
        assert_eq!(actual, expected, "{name}");
    }

    pub fn truth(&mut self, name: &str, actual: bool) {
        self.equal(name, json!(actual), json!(true));
    }
}

struct Run {
    success: bool,
    stdout: String,
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
