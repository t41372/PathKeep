//! Presence probes and root selection for an explicit preview. This module
//! never enumerates a conversation tree or grants permission to capture it.
use super::{Refusal, admitted_config};
use agent_history::{Harness, SourceRoot, capture};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use vault_core::{AgentTool, DataKind};
fn harness(tool: AgentTool) -> Harness {
    match tool {
        AgentTool::ClaudeCode => Harness::ClaudeCode,
        AgentTool::Codex => Harness::Codex,
        AgentTool::Pi => Harness::Pi,
        AgentTool::Deepseek => Harness::Deepseek,
    }
}

/// Limit presence probes to the kinds requested by onboarding.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    kinds: Vec<DataKind>,
}

/// Presence for the intro and onboarding without starting a transcript walk.
pub fn detect(request: Detect) -> Result<Value> {
    let paths = vault_core::project_paths()?;
    admitted_config(&paths)?;
    let mut sources = Vec::new();
    if request.kinds.contains(&DataKind::AgentHistory) {
        for root in capture::known_sources() {
            let reason = capture::root_status(&root.root);
            let mut row = json!({
                "kind": "agent-history",
                "tool": root.harness,
                "found": reason != Some("root-missing"),
                "readable": reason.is_none(),
                "rootIds": [capture::root_identity(&root)?],
                "path": root.root
            });
            if let Some(reason) = reason {
                row["reason"] = json!(reason)
            }
            sources.push(row);
        }
    }
    let mut profiles = Vec::new();
    let mut issue = None;
    if request.kinds.contains(&DataKind::Browsing) {
        let discovery = crate::app::snapshot_browser_profiles();
        issue = discovery.issue;
        for p in discovery.profiles {
            let blocked_safari = p.browser_family == "safari"
                && (p.access_issue.as_deref()
                    == Some(vault_core::DISCOVERY_ISSUE_FULL_DISK_ACCESS)
                    || !p.history_readable
                        && issue.as_deref() == Some(vault_core::DISCOVERY_ISSUE_FULL_DISK_ACCESS));
            if !p.history_exists && !blocked_safari {
                continue;
            }
            profiles.push(json!({"profileId":p.profile_id,"browserName":p.browser_name,"profileName":p.profile_name,"readable":p.history_readable}));
            let mut row = json!({"kind":"browsing","found":true,"readable":p.history_readable,"profileId":p.profile_id,"rootIds":[]});
            if let Some(reason) =
                p.access_issue.as_ref().or(issue.as_ref()).filter(|_| !p.history_readable)
            {
                row["reason"] = json!(reason)
            }
            sources.push(row);
        }
        if profiles.is_empty()
            && issue.as_deref() == Some(vault_core::DISCOVERY_ISSUE_FULL_DISK_ACCESS)
        {
            sources.push(json!({"kind":"browsing","found":true,"readable":false,"reason":issue,"rootIds":[]}));
        }
    }
    admitted_config(&paths)?;
    let mut result = json!({"sources":sources,"profiles":profiles});
    if let Some(reason) = issue {
        result["browserDiscoveryIssue"] = json!(reason)
    }
    Ok(result)
}

/// Distinguish explicit source discovery entry points from ordinary screen reads.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Purpose {
    Onboarding,
    Enable,
    Rescan,
}
/// An absolute native picker result needs an explicit tool to interpret its layout.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PickedFolder {
    tool: AgentTool,
    path: PathBuf,
}
/// Restrict enumeration to selected tools, known root identities, or a picked folder.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    pub purpose: Purpose,
    tools: Option<Vec<AgentTool>>,
    root_ids: Option<Vec<String>>,
    picked_folder: Option<PickedFolder>,
}
pub(super) fn resolve(request: PreviewRequest, archive: &Path) -> Result<Vec<SourceRoot>> {
    let roots = if let Some(picked) = request.picked_folder {
        if !picked.path.is_absolute()
            || picked.path.components().any(|p| p == std::path::Component::ParentDir)
        {
            return Err(Refusal::ScopeDenied.into());
        }
        vec![SourceRoot { harness: harness(picked.tool), root: picked.path }]
    } else {
        capture::known_sources()
    };
    let target = capture::normalized_root(archive)?;
    let mut filtered = Vec::new();
    for root in roots {
        let id = capture::root_identity(&root)?;
        if request
            .tools
            .as_ref()
            .is_some_and(|tools| !tools.iter().any(|&t| harness(t) == root.harness))
            || request.root_ids.as_ref().is_some_and(|ids| !ids.contains(&id))
        {
            continue;
        }
        let real = capture::normalized_root(&root.root)?;
        if real.components().any(|c| c.as_os_str() == ".git") {
            return Err(Refusal::ScopeDenied.into());
        }
        if real.starts_with(&target) || target.starts_with(real) {
            return Err(Refusal::ScopeDenied.into());
        }
        filtered.push(root)
    }
    if filtered.is_empty() {
        return Err(Refusal::EmptySelection.into());
    }
    if request.root_ids.as_ref().is_some_and(|ids| ids.len() != filtered.len()) {
        return Err(Refusal::ScopeDenied.into());
    }
    Ok(filtered)
}
