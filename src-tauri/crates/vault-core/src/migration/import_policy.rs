//! Import preserves conversations. Keeping a backup grant also requires that the move
//! file carried the same immutable selection the destination approved.
//! Source folders need not be connected during an import.
use crate::{AppConfig, ControlGate, ProjectPaths, Vault};
use anyhow::Result;
use std::path::Path;
use vault_store::{Db, OptionalRow};

fn descriptor(vault: &Vault, id: &str) -> Result<Option<String>> {
    if vault.layout() == vault_layout::ArchiveLayout::BrowsingV1 {
        return Ok(None);
    }
    let id = id.to_owned();
    Ok(vault.store().read(Db::AgentHistoryEvidence, move |s| selection(s, &id))?)
}

fn selection(s: &vault_store::Session<'_>, id: &str) -> vault_store::Result<Option<String>> {
    s.query_one("SELECT descriptor_json FROM ah_capture_selections WHERE id=?1", [id], |r| {
        Ok(r.get(0)?)
    })
    .optional()
}

/// Read databases outside the control gate. A selection edited during the
/// check gets checked in its turn before we publish the replacement control.
pub(super) fn for_install(
    paths: &ProjectPaths,
    current: Option<&Vault>,
    incoming_root: &Path,
    config: &mut AppConfig,
    key: Option<&vault_crypto::DataKey>,
) -> Result<ControlGate> {
    let mut checked: Option<(String, Option<String>)> = None;
    loop {
        let gate = ControlGate::acquire(paths);
        let here = super::native::settings_here(paths)?;
        super::native::keep_machine_settings(config, &here);
        let policy = &mut config.capture_policy;
        let Some(id) = policy.selection_ref.as_ref() else {
            if policy.agent_history == crate::KindState::On {
                policy.pending_first_save = None;
                policy.enabled_tools.clear();
            }
            return Ok(gate);
        };
        if let Some((checked_id, raw)) = &checked
            && checked_id == id
        {
            #[derive(serde::Deserialize)]
            struct Selected {
                tool: crate::AgentTool,
            }
            let resolves = match raw {
                Some(raw) => serde_json::from_str::<Vec<Selected>>(raw)?
                    .iter()
                    .any(|r| policy.enabled_tools.contains(&r.tool)),
                None => false,
            };
            if raw.is_none() || (policy.agent_history == crate::KindState::On && !resolves) {
                if policy.agent_history == crate::KindState::Off {
                    policy.explicitly_off = true;
                }
                policy.selection_ref = None;
                policy.pending_first_save = None;
                policy.enabled_tools.clear();
            }
            return Ok(gate);
        }
        let id = id.clone();
        drop(gate);
        let previous = current.map(|v| descriptor(v, &id)).transpose()?.flatten();
        let matching = if previous.is_some() {
            let storage = crate::vault::storage_config(
                &crate::project_paths_with_root(incoming_root),
                crate::admission::WRITTEN_LAYOUT,
                key,
                None,
            )?;
            let incoming =
                vault_store::Store::inspect_existing(&storage, Db::AgentHistoryEvidence, |s| {
                    selection(s, &id)
                })?;
            incoming.filter(|v| Some(v) == previous.as_ref())
        } else {
            None
        };
        checked = Some((id, matching));
    }
}
