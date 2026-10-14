//! Responsibilities: real sidecar and stdio MCP processes against the owner dispatcher.
//! Not responsible for: native Tauri startup (desktop owner_process.rs covers that).
//! Failure modes: clients bypass IPC or open archives; stdout is held across MCP;
//! disabled MCP loses its startup exit code; consent changes are ignored mid-session.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};
use vault_core::{AppConfig, ArchiveMode};

#[test]
fn sidecar_and_mcp_forward_to_owner_and_recheck_consent() {
    let root = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(root.path());
    let env = [
        ("CHB_PROJECT_ROOT", root.path().to_owned()),
        ("CHB_TEST_KEYRING_DIR", root.path().join("keyring")),
        ("CHB_CHROME_USER_DATA_DIR", root.path().join("chrome")),
        ("CHB_FIREFOX_PROFILES_DIR", root.path().join("no-firefox")),
        ("CHB_SAFARI_ROOT", root.path().join("no-safari")),
        ("PATHKEEP_PLATFORM_TEST_SANDBOX_DIR", root.path().join("platform")),
    ];
    // This integration-test executable contains one test and owns its environment.
    for (key, value) in &env {
        unsafe {
            std::env::set_var(key, value);
        }
    }
    let lock = Arc::new(vault_store::OwnerLock::acquire(root.path()).unwrap());
    let config = AppConfig {
        initialized: true,
        archive_mode: ArchiveMode::Plaintext,
        git_enabled: false,
        ..Default::default()
    };
    vault_core::save_config(&paths, &config).unwrap();
    vault_core::config::ensure_paths(&paths).unwrap();
    let vault =
        vault_core::Vault::open_owned(paths.clone(), config.clone(), None, Some(lock)).unwrap();
    let methods = Arc::new(Mutex::new(Vec::new()));
    let recorded = methods.clone();
    let _server = vault_owner_ipc::Server::start(root.path(), move |request, progress| {
        recorded.lock().unwrap().push(request.method.clone());
        vault_worker::owner_requests::dispatch(request, progress, Ok(None), || Ok(vault.clone()))
    })
    .unwrap();
    let command = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pathkeep-worker"));
        for (key, value) in &env {
            command.env(key, value);
        }
        command
    };
    let ping = command().arg("ping").output().unwrap();
    assert!(ping.status.success(), "{}", String::from_utf8_lossy(&ping.stderr));
    let ping: Value = serde_json::from_slice(&ping.stdout).unwrap();
    assert_eq!(ping["pid"], std::process::id());
    let doctor = command().arg("doctor").output().unwrap();
    assert!(doctor.status.success(), "{}", String::from_utf8_lossy(&doctor.stderr));
    let disabled = command().arg("mcp-server").output().unwrap();
    assert_eq!(disabled.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&disabled.stderr).contains("Enable AI and the MCP server"));
    let mut config = config;
    config.ai.enabled = true;
    config.ai.mcp_enabled = true;
    config.ai.skill_enabled = true;
    vault_core::save_config(&paths, &config).unwrap();
    let mut child = command()
        .arg("mcp-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let read = || -> Value {
        serde_json::from_str(
            &rx.recv_timeout(Duration::from_secs(10)).expect("MCP reply deadline").unwrap(),
        )
        .unwrap()
    };
    writeln!(input,"{}",json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"owner-test","version":"1"}}})).unwrap();
    assert!(read()["result"].is_object());
    writeln!(input, "{}", json!({"jsonrpc":"2.0","method":"notifications/initialized"})).unwrap();
    let call = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"archive-status","arguments":{}}});
    writeln!(input, "{call}").unwrap();
    let tool = read();
    assert!(tool["error"].is_null(), "{tool}");
    assert_ne!(tool["result"]["isError"], true, "{tool}");
    config.ai.mcp_enabled = false;
    vault_core::save_config(&paths, &config).unwrap();
    writeln!(input, "{call}").unwrap();
    let refused = read();
    assert!(refused["error"].is_object() || refused["result"]["isError"] == true, "{refused}");
    drop(input);
    child.kill().ok();
    child.wait().unwrap();
    assert_eq!(*methods.lock().unwrap(), ["ping", "doctor", "mcp", "mcp", "mcp", "mcp"]);
    let artifact =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../artifacts/owner-process");
    std::fs::create_dir_all(&artifact).unwrap();
    std::fs::write(artifact.join("sidecar-mcp.json"), serde_json::to_vec_pretty(&json!({"ownerPid":std::process::id(),"ping":ping,"mcpTool":tool,"revoked":refused,"methods":*methods.lock().unwrap()})).unwrap()).unwrap();
}
