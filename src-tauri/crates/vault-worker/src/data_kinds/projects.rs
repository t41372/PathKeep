//! Project commands use the same saved-conversation admission as the Records workbench.
use super::{
    backups::Events,
    chats::{Conversation, admitted},
};
use crate::VaultSession;
pub use agent_history::projects::{ProjectEdit, ProjectQuery};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Suggestions {
    pub into_project_id: Option<String>,
}
pub fn list(s: &VaultSession, mut q: ProjectQuery) -> Result<Value> {
    admitted(s, |a, v| {
        if q.status == agent_history::projects::Status::Recent && q.time_zone.is_none() {
            q.time_zone = Some(vault_core::insights_time_zone(v)?.shown);
        }
        Ok(a.project_page(&q)?)
    })
}
pub fn get(s: &VaultSession, q: Conversation) -> Result<Value> {
    admitted(s, |a, _| Ok(a.project_detail(&q.id)?))
}
pub fn suggestions(s: &VaultSession, q: Suggestions) -> Result<Value> {
    admitted(s, |a, _| Ok(a.project_suggestions(q.into_project_id.as_deref())?))
}
pub fn edit(s: &VaultSession, q: ProjectEdit, events: Events) -> Result<Value> {
    let result = admitted(s, |a, _| Ok(a.edit_project(q)?))?;
    events("pathkeep://agent-projects-changed", json!({"version":result["version"]}));
    events(
        "pathkeep://agent-conversations-changed",
        json!({"id":null,"viewToken":result["viewToken"]}),
    );
    Ok(result)
}
