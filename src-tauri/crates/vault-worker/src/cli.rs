//! Responsibilities: translate CLI arguments to owner requests and print progress.
//! Not responsible for: database access, keyring, config loading or business work.
//! Failure modes: unknown command starts app; reconnect repeats backup; progress corrupts
//! JSON stdout; sidecar opens archive. Tests: owner_requests and desktop owner_process.
use anyhow::Result;
use serde_json::json;
use vault_owner_ipc::{Client, Request};

/// The output of the native schedule's runs (`--due-only`), which launchd appends to for good
/// (`vault-platform`'s macOS plan), kept bounded. Trimming is best effort: the backup runs anyway.
fn cap_schedule_logs() {
    let Ok(paths) = vault_core::project_paths() else { return };
    let logs = paths.schedule_dir.join("logs");
    for name in ["worker.stdout.log", "worker.stderr.log"] {
        let _ = vault_owner_ipc::cap_log(&logs.join(name));
    }
}

/// Preserves CLI JSON output and 0/1 exit semantics while the app owns all work.
pub fn run_worker_cli(arguments: &[String]) -> Result<String> {
    let command = arguments.first().map(String::as_str).unwrap_or("snapshot");
    if command == "mcp-server" {
        crate::mcp::run_mcp_stdio_server()?;
        return Ok(String::new());
    }
    let request = match command {
        "backup" => {
            let due_only = arguments.iter().any(|a| a == "--due-only");
            if due_only {
                cap_schedule_logs();
            }
            Request::new("backup", json!({ "dueOnly": due_only }))
        }
        "doctor" | "ai-index" | "ai-queue" | "ping" => Request::new(command, json!({})),
        "ai-jobs" | "intelligence-jobs" => {
            let params = arguments
                .get(1)
                .map(|value| serde_json::from_str(value))
                .transpose()?
                .unwrap_or_else(|| json!({}));
            Request::new(command, params)
        }
        other => anyhow::bail!("unknown worker command: {other}"),
    };
    let root = vault_core::project_paths()?.app_root;
    let client = Client::new(&root);
    let value = crate::context::tokio_runtime()?
        .block_on(client.request_or_start(request, |progress| eprintln!("{progress}")))?;
    Ok(serde_json::to_string_pretty(&value)?)
}
