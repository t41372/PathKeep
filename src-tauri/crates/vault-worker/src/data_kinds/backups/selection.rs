//! An immutable descriptor names the reviewed directory object. Only a device
//! control referencing this descriptor authorizes a new capture.
use super::super::{Refusal, preview};
use agent_history::{SourceRoot, capture};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vault_core::{AgentTool, Vault};
use vault_store::{Db, OptionalRow, Priority, params};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Root {
    pub id: String,
    pub tool: AgentTool,
    pub path: PathBuf,
    pub(super) directory: String,
}

impl Root {
    pub fn source(&self) -> SourceRoot {
        SourceRoot { harness: self.tool.harness(), root: self.path.clone() }
    }

    pub fn issue(&self, vault: &Vault) -> Option<&'static str> {
        let Ok(archive) = capture::normalized_root(&vault.paths().app_root) else {
            return Some("destination-unavailable");
        };
        capture::directory_issue(&self.source(), &self.id, &self.directory, &archive)
    }
}

pub(super) fn reviewed(
    vault: &Vault,
    preview_id: &str,
    ids: &[String],
    retained: &[String],
) -> Result<Vec<Root>> {
    let roots: Vec<Root> = preview::reviewed_bindings(preview_id, ids, retained)?
        .into_iter()
        .map(|b| {
            let root = Root {
                id: b.root_id,
                path: b.canonical,
                tool: AgentTool::ALL
                    .into_iter()
                    .find(|t| t.harness() == b.source.harness)
                    .ok_or(Refusal::ScopeDenied)?,
                directory: capture::directory_identity(&b.directory)?,
            };
            Ok::<_, anyhow::Error>(root)
        })
        .collect::<Result<_>>()?;
    if !roots.is_empty() && !roots.iter().any(|r| r.issue(vault).is_none()) {
        return Err(Refusal::RootChanged.into());
    }
    Ok(roots)
}

pub(super) fn descriptor_exists(vault: &Vault, policy: &vault_core::CapturePolicy) -> Result<bool> {
    let Some(id) = policy.selection_ref.clone() else { return Ok(false) };
    Ok(vault.store().read(Db::AgentHistoryEvidence, move |tx| {
        tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_capture_selections WHERE id=?1)", [id], |r| {
            Ok(r.get::<i64>(0)? != 0)
        })
    })?)
}
pub(super) fn stage(
    vault: &Vault,
    roots: &[Root],
    enable_hash: Option<String>,
    previous: Option<String>,
) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let key = id.clone();
    let json = serde_json::to_string(roots)?;
    vault.store().write(Db::AgentHistoryEvidence, Priority::Interactive, move |s| {
        s.execute(
            "INSERT INTO ah_capture_selections(id,descriptor_json,enable_hash,first_task_id)
             VALUES(?1,?2,?3,CASE WHEN ?3 IS NOT NULL THEN ?1 ELSE
                (SELECT first_task_id FROM ah_capture_selections WHERE id=?4) END)",
            params![key, json, enable_hash, previous],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub(super) fn load(vault: &Vault, id: &str) -> Result<Vec<Root>> {
    let id = id.to_owned();
    let json = vault
        .store()
        .read(Db::AgentHistoryEvidence, move |s| {
            s.query_one(
                "SELECT descriptor_json FROM ah_capture_selections WHERE id=?1",
                [id],
                |row| Ok(row.get::<String>(0)?),
            )
            .optional()
        })?
        .ok_or(Refusal::EnablePreview)?;
    Ok(serde_json::from_str(&json)?)
}

/// A move can leave a destination reference without its old descriptor. This
/// affects capture eligibility only; retained evidence is still readable.
pub(super) fn device_roots(vault: &Vault, policy: &vault_core::CapturePolicy) -> Result<Vec<Root>> {
    let Some(id) = &policy.selection_ref else {
        return Ok(Vec::new());
    };
    match load(vault, id) {
        Err(error) if matches!(error.downcast_ref::<Refusal>(), Some(Refusal::EnablePreview)) => {
            Ok(Vec::new())
        }
        result => result,
    }
}
