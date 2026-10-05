//! Responsibilities: verify real app/client processes against an isolated archive.
//! Not responsible for: mocking Tauri, storage engines or OS schedulers.
//! Failure modes: cold client cannot start app; second instance takes owner lock;
//! background launch creates a window; client opens databases; MCP avoids endpoint;
//! crashed owner leaves an unusable socket. Artifact: artifacts/owner-process/result.json.
#![cfg(unix)]
mod support;
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::Stdio,
    time::{Duration, Instant},
};
use vault_owner_ipc::{Client, Request};

#[test]
fn cold_client_backup_single_instance_handoff_and_mcp_use_only_owner() {
    let fixture = support::Fixture::new();
    let output = fixture.command().args(["--worker", "ping"]).output().unwrap();
    let startup_log = std::fs::read_to_string(fixture.root.path().join("logs/owner-startup.log"))
        .unwrap_or_default();
    fixture.artifact(json!({"phase":"cold-start","status":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr),"startupLog":startup_log}));
    assert!(output.status.success(), "{}\n{startup_log}", String::from_utf8_lossy(&output.stderr));
    let ping: Value = serde_json::from_slice(&output.stdout).unwrap();
    let pid = ping["pid"].as_u64().unwrap() as u32;
    let _app = support::AppProcess(pid);
    assert_eq!(ping["background"], true);
    assert_eq!(ping["startupWindows"], 0);
    assert!(vault_store::OwnerLock::acquire(fixture.root.path()).is_err());
    let second = fixture.command().args(["--background", "handoff-proof"]).output().unwrap();
    assert!(second.status.success(), "{}", String::from_utf8_lossy(&second.stderr));
    let rt = tokio::runtime::Runtime::new().unwrap();
    let client = Client::new(fixture.root.path());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ping = rt.block_on(client.request(Request::new("ping", json!({})), |_| {})).unwrap();
        assert_eq!(ping["pid"], pid);
        if ping["lastLaunchArguments"].as_array().unwrap().iter().any(|v| v == "handoff-proof") {
            break;
        }
        assert!(Instant::now() < deadline, "single-instance arguments were not handed off");
        std::thread::sleep(Duration::from_millis(30));
    }
    let backup = fixture
        .database_denied_command()
        .args(["--worker", "backup", "--due-only"])
        .output()
        .unwrap();
    assert!(backup.status.success(), "{}", String::from_utf8_lossy(&backup.stderr));
    let report: Value = serde_json::from_slice(&backup.stdout).unwrap();
    let again = fixture.command().args(["--worker", "backup", "--due-only"]).output().unwrap();
    assert!(again.status.success());
    let skipped: Value = serde_json::from_slice(&again.stdout).unwrap();
    assert_eq!(skipped["dueSkipped"], true);

    let mut mcp = fixture
        .database_denied_command()
        .args(["--worker", "mcp-server"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = mcp.stdin.take().unwrap();
    let mut output = BufReader::new(mcp.stdout.take().unwrap());
    writeln!(input, "{}", json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"owner-test","version":"1"}}})).unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert!(serde_json::from_str::<Value>(&line).unwrap()["result"].is_object());
    writeln!(input, "{}", json!({"jsonrpc":"2.0","method":"notifications/initialized"})).unwrap();
    writeln!(input, "{}", json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"archive-status","arguments":{}}})).unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let tool: Value = serde_json::from_str(&line).unwrap();
    assert!(tool["error"].is_null(), "{tool}");
    assert_ne!(tool["result"]["isError"], true, "{tool}");
    let files = support::open_files(mcp.id());
    assert!(
        !files.contains(".sqlite") && !files.contains(".db") && !files.contains(".pk-owner.lock"),
        "client opened archive files: {files}"
    );
    drop(input);
    mcp.kill().ok();
    mcp.wait().unwrap();
    fixture.artifact(json!({"ownerPid":pid,"coldPing":ping,"backup":report,"dueSkip":skipped,"mcp":tool,"clientOpenFiles":files,"progress":String::from_utf8_lossy(&backup.stderr)}));
}
