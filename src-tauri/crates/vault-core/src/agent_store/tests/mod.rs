//! Responsibilities: preserve agent transcript and journal behaviours on real stores.
//! Not responsible for: provider calls or engine migrations.
//! Failure modes: overwritten timestamps, truncated transcripts, lost citations,
//! duplicate replay, missing parents and incomplete deletion cascades.

use super::*;
use std::time::Duration as StdDuration;

fn user_message(id: &str, content: &str) -> AgentMessage {
    AgentMessage {
        id: id.to_string(),
        role: "user".to_string(),
        content: content.to_string(),
        reasoning: None,
        tool_calls_json: None,
        status: None,
        ..Default::default()
    }
}

fn assistant_message(id: &str, content: &str) -> AgentMessage {
    AgentMessage {
        id: id.to_string(),
        role: "assistant".to_string(),
        content: content.to_string(),
        reasoning: Some("thinking".to_string()),
        tool_calls_json: Some(
            "[{\"id\":\"t1\",\"name\":\"search_bm25\",\"arguments\":\"{}\"}]".to_string(),
        ),
        status: Some("done".to_string()),
        ..Default::default()
    }
}

mod journal;
mod transcript;

fn query<T: Send + 'static>(
    vault: &Vault,
    sql: &'static str,
    params: impl vault_store::IntoParams,
    map: impl FnOnce(&vault_store::Row) -> vault_store::Result<T> + Send + 'static,
) -> vault_store::Result<T> {
    let params = params.into_params().unwrap();
    vault.store().read(Db::Agent, move |s| s.query_one(sql, params, map))
}
fn execute(
    vault: &Vault,
    sql: &'static str,
    params: impl vault_store::IntoParams,
) -> vault_store::Result<u64> {
    let params = params.into_params().unwrap();
    vault.store().write(Db::Agent, Priority::Interactive, move |s| s.execute(sql, params))
}
