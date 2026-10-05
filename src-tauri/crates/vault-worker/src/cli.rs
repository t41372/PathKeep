//! Responsibilities: translate CLI arguments to owner requests and print progress.
//! Not responsible for: database access, keyring, config loading or business work.
//! Failure modes: unknown command starts app; reconnect repeats backup; progress corrupts
//! JSON stdout; sidecar opens archive. Tests: owner_requests and desktop owner_process.
use anyhow::Result;
use serde_json::json;
use vault_owner_ipc::{Client, Request};

/// Preserves CLI JSON output and 0/1 exit semantics while the app owns all work.
pub fn run_worker_cli(arguments: &[String]) -> Result<String> {
    let command = arguments.first().map(String::as_str).unwrap_or("snapshot");
    if command == "mcp-server" {
        crate::mcp::run_mcp_stdio_server()?;
        return Ok(String::new());
    }
    let request = match command {
        "backup" => {
            Request::new("backup", json!({"dueOnly":arguments.iter().any(|a| a == "--due-only")}))
        }
        "doctor" | "ai-index" | "ai-queue" | "ping" => Request::new(command, json!({})),
        other => anyhow::bail!("unknown worker command: {other}"),
    };
    let root = vault_core::project_paths()?.app_root;
    let client = Client::new(&root);
    let value = crate::context::tokio_runtime()?
        .block_on(client.request_or_start(request, |progress| eprintln!("{progress}")))?;
    Ok(serde_json::to_string_pretty(&value)?)
}
