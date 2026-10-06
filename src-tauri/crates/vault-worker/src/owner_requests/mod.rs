//! Responsibilities: allowlisted owner operations using existing worker functions.
//! Not responsible for: database code, transport, owner lock or process startup.
//! Failure modes: arbitrary command exposure; invalid params accepted; stale key or
//! MCP consent; scheduled ledger/progress lost. Tests: tests.rs, scheduled.rs.
mod scheduled;
#[cfg(test)]
pub(crate) mod test_client;
#[cfg(test)]
mod tests;
use crate::{archive_flows, intelligence, mcp};
use serde::Deserialize;
use serde_json::{Value, json};
use vault_owner_ipc::{ErrorCode, Progress, Request, RpcError};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Backup {
    due_only: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mcp {
    tool: String,
    arguments: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
fn params<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value)
        .map_err(|e| RpcError::new(ErrorCode::InvalidRequest, e.to_string()))
}
fn failed(error: anyhow::Error) -> RpcError {
    RpcError::new(ErrorCode::OperationFailed, format!("{error:#}"))
}
fn encode(value: impl serde::Serialize) -> Result<Value, RpcError> {
    serde_json::to_value(value)
        .map_err(|e| RpcError::new(ErrorCode::OperationFailed, e.to_string()))
}

fn resolve_key(session_key: Result<Option<String>, String>) -> Result<Option<String>, RpcError> {
    session_key.map_err(|message| failed(anyhow::Error::msg(message)))
}

/// Called only by the app's locked owner service, on a blocking worker thread.
/// Session keys stay in the app; scheduled backups still resolve their keyring key.
pub fn dispatch(
    request: Request,
    progress: Progress,
    session_key: Result<Option<String>, String>,
    vault: impl Fn() -> anyhow::Result<vault_core::Vault>,
) -> Result<Value, RpcError> {
    match request.method.as_str() {
        "ping" => {
            let _: Empty = params(request.params)?;
            Ok(json!({"pid":std::process::id()}))
        }
        "backup" => {
            let backup: Backup = params(request.params)?;
            if backup.due_only {
                let report = scheduled::run_scheduled_backup_cli(
                    &progress,
                    &vault,
                    session_key.as_ref().ok().and_then(|key| key.as_deref()),
                )
                .map_err(failed)?;
                serde_json::from_str(&report)
                    .map_err(|e| RpcError::new(ErrorCode::OperationFailed, e.to_string()))
            } else {
                let key = session_key.ok().flatten();
                encode(
                    archive_flows::run_backup_now_with_progress(
                        &vault().map_err(failed)?,
                        key.as_deref(),
                        false,
                        |event| {
                            progress.send(
                                serde_json::to_value(event)
                                    .expect("backup progress is serializable"),
                            );
                        },
                    )
                    .map_err(failed)?,
                )
            }
        }
        "mcp" => {
            let call: Mcp = params(request.params)?;
            if call.tool == "initialize" {
                let _: Empty = params(call.arguments)?;
                mcp::validate_mcp_session().map_err(failed)?;
                resolve_key(session_key)?;
                return Ok(Value::Null);
            }
            // Keyring/config access and the tool's consent checks happen in the owner, every call.
            let key = resolve_key(session_key)?;
            match call.tool.as_str() {
                "search-history" => encode(
                    mcp::mcp_search_result(
                        &vault().map_err(failed)?,
                        key.as_deref(),
                        params(call.arguments)?,
                    )
                    .map_err(failed)?,
                ),
                "archive-status" => {
                    let _: Empty = params(call.arguments)?;
                    encode(
                        mcp::mcp_archive_status_result(vault().ok().as_ref(), key.as_deref())
                            .map_err(failed)?,
                    )
                }
                "usage-guide" => {
                    let _: Empty = params(call.arguments)?;
                    encode(mcp::mcp_usage_guide_result(key.as_deref()).map_err(failed)?)
                }
                _ => Err(RpcError::new(
                    ErrorCode::UnknownMethod,
                    format!("unknown MCP tool: {}", call.tool),
                )),
            }
        }
        "doctor" | "ai-index" | "ai-queue" => {
            let _: Empty = params(request.params)?;
            let key = resolve_key(session_key)?;
            match request.method.as_str() {
                "doctor" => encode(
                    archive_flows::doctor_report(&vault().map_err(failed)?, key.as_deref())
                        .map_err(failed)?,
                ),
                "ai-index" => encode(
                    intelligence::build_ai_index_now(
                        &vault().map_err(failed)?,
                        key.as_deref(),
                        &vault_core::AiIndexRequest::default(),
                    )
                    .map_err(failed)?,
                ),
                _ => encode(
                    intelligence::run_ai_queue_jobs(
                        &vault().map_err(failed)?,
                        key.as_deref(),
                        None,
                    )
                    .map_err(failed)?,
                ),
            }
        }
        _ => Err(RpcError::new(
            ErrorCode::UnknownMethod,
            format!("unknown owner method: {}", request.method),
        )),
    }
}
