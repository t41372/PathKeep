//! Device-local display and access preferences. Archive-owned cost settings keep their existing command.
use super::{Empty, Refusal, admitted_config, backups};
use agent_history::projects::Grouping;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use vault_core::{AppConfig, ControlGate};

pub(super) fn grouping(config: &AppConfig) -> Grouping {
    let mut group = Grouping { read_git: config.capture_policy.read_git, ..Default::default() };
    if let Some(locations) = &config.agent_viewer.scratch_locations {
        group.home = group.home.filter(|p| locations.contains(p));
        group.temporary = locations
            .iter()
            .filter(|p| Some(p.as_str()) != group.home.as_deref())
            .cloned()
            .collect();
    }
    group
}
fn response(config: &AppConfig) -> Result<Value> {
    let mut value = serde_json::to_value(&config.agent_viewer)?;
    let group = grouping(config);
    let mut locations = group.temporary;
    locations.extend(group.home);
    locations.sort();
    locations.dedup();
    value["scratchLocations"] = json!(locations);
    value["priceTableVersion"] = json!(agent_history::chats::PRICE_TABLE_VERSION);
    Ok(value)
}
pub fn get(_: Empty) -> Result<Value> {
    response(&admitted_config(&vault_core::project_paths()?)?)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    scratch_locations: Option<Vec<String>>,
    reader_collapse_tool_calls: Option<bool>,
    reader_show_thinking: Option<bool>,
    assistant_may_read: Option<bool>,
    build_semantic_index: Option<bool>,
    mcp_may_read: Option<bool>,
}
pub fn set(session: &crate::VaultSession, p: Patch, events: backups::Events) -> Result<Value> {
    let paths = vault_core::project_paths()?;
    let gate = ControlGate::acquire(&paths);
    let mut cfg = admitted_config(&paths)?;
    let v = &mut cfg.agent_viewer;
    let previous_locations = v.scratch_locations.clone();
    if let Some(mut locations) = p.scratch_locations {
        if locations.iter().any(|p| !std::path::Path::new(p).is_absolute()) {
            return Err(Refusal::InvalidRequest.into());
        }
        locations.sort();
        locations.dedup();
        v.scratch_locations = Some(locations);
    }
    if let Some(value) = p.reader_collapse_tool_calls {
        v.reader_collapse_tool_calls = value;
    }
    if let Some(value) = p.reader_show_thinking {
        v.reader_show_thinking = value;
    }
    if let Some(value) = p.assistant_may_read {
        v.assistant_may_read = value;
    }
    if let Some(value) = p.build_semantic_index {
        v.build_semantic_index = value;
    }
    if let Some(value) = p.mcp_may_read {
        v.mcp_may_read = value;
    }
    vault_core::save_config(&gate, &cfg)?;
    drop(gate);
    let result = response(&cfg)?;
    events("pathkeep://agent-viewer-preferences-changed", result.clone());
    if let Some(enabled) = p.build_semantic_index {
        super::semantic::preferences_changed(session, enabled, events.clone());
    }
    if cfg.agent_viewer.scratch_locations != previous_locations {
        events("pathkeep://agent-projects-changed", json!({}));
        events("pathkeep://agent-conversations-changed", json!({"id":null}));
    }
    Ok(result)
}
