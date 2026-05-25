//! Worker CLI routing.
//!
//! The desktop binary can launch this crate in `--worker` mode. That path is a
//! thin transport boundary over the same worker functions the Tauri facade
//! uses, so this module only dispatches commands and serializes the result.

use crate::{
    archive_flows::{doctor_report, run_backup_now},
    intelligence::{build_ai_index_now, run_ai_queue_jobs},
    mcp::run_mcp_stdio_server,
    security::read_database_key_from_keyring,
};
use anyhow::Result;
use vault_core::AiIndexRequest;

/// Executes one worker CLI command and returns a JSON payload when applicable.
pub fn run_worker_cli(arguments: &[String]) -> Result<String> {
    let command = arguments.first().map(String::as_str).unwrap_or("snapshot");
    match command {
        "backup" => {
            let due_only = arguments.iter().any(|arg| arg == "--due-only");
            let key = read_database_key_from_keyring()?;
            let report = run_backup_now(key.as_deref(), due_only)?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "doctor" => {
            let key = read_database_key_from_keyring()?;
            let report = doctor_report(key.as_deref())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "ai-index" => {
            let key = read_database_key_from_keyring()?;
            let report = build_ai_index_now(key.as_deref(), &AiIndexRequest::default())?;
            Ok(serde_json::to_string_pretty(&report)?)
        }
        "ai-queue" => {
            let key = read_database_key_from_keyring()?;
            let status = run_ai_queue_jobs(key.as_deref(), None)?;
            Ok(serde_json::to_string_pretty(&status)?)
        }
        "mcp-server" => {
            run_mcp_stdio_server()?;
            Ok(String::new())
        }
        other => anyhow::bail!("unknown worker command: {other}"),
    }
}
