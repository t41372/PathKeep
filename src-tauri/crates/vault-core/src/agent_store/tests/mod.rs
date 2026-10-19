//! Responsibilities: preserve agent transcripts on real stores.
//! Not responsible for: provider calls or engine migrations.
//! Failure modes: truncated transcripts and conversations the list can no longer reach.

use super::*;

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

mod transcript;

fn execute(
    vault: &Vault,
    sql: &'static str,
    params: impl vault_store::IntoParams,
) -> vault_store::Result<u64> {
    let params = params.into_params().unwrap();
    vault.store().write(Db::Agent, Priority::Interactive, move |s| s.execute(sql, params))
}
