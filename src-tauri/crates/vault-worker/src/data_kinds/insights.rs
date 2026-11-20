//! Insight streams share the reader's admission and the browsing report's published time zone.
use super::{Empty, backups, chats, preview};
use crate::VaultSession;
pub use agent_history::insights::{PreferencePatch, Query};
use anyhow::Result;
use serde_json::{Value, json};
pub fn start(s: &VaultSession, q: Query) -> Result<preview::Frames> {
    let (vault, config) = backups::admitted(s, false)?;
    let archive = vault.agent_history()?;
    archive.set_project_grouping(super::viewer::grouping(&config));
    let zone = vault_core::insights_time_zone(&vault)?.shown;
    let prefs = archive.insight_preferences()?;
    let first_running = backups::first_backup(&vault, &config.capture_policy)? == "running";
    let now = chrono::Utc::now().timestamp_millis();
    let frames = preview::conversation_stream(s, archive, "agent-insights", move |a, c, send| {
        a.agent_insights(&q, &zone, now, &prefs, c, |ty, mut payload| {
            if ty == "started" {
                payload["firstBackupRunning"] = first_running.into();
            }
            send(ty, payload)
        })
    })?;
    backups::check_reply(&vault, s, false)?;
    Ok(frames)
}
pub fn preferences(s: &VaultSession, _: Empty) -> Result<Value> {
    chats::admitted(s, |a, _| Ok(json!(a.insight_preferences()?)))
}
pub fn update(s: &VaultSession, q: PreferencePatch, events: backups::Events) -> Result<Value> {
    let v = chats::admitted(s, |a, _| Ok(json!(a.update_insight_preferences(q)?)))?;
    events("pathkeep://agent-insights-preferences-changed", v.clone());
    Ok(v)
}
