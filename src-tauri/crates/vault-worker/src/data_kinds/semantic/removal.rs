//! Preview and confirm removal of the owner's retained conversation embeddings.
//! Permission to manage space never grants permission to search or call a provider.
use super::{Empty, Refusal, backups, job, pages};
use crate::VaultSession;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use vault_core::{AiCapability, ControlGate, Vault, ai::conversation_vectors as vectors};

struct Preview {
    id: uuid::Uuid,
    owner: Weak<vault_store::Store>,
    kept: vectors::KeptIndex,
    expires: Instant,
}
static PREVIEW: Mutex<Option<Preview>> = Mutex::new(None);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Confirm {
    preview_id: uuid::Uuid,
    confirm: bool,
}

fn check_off(v: &Vault) -> Result<()> {
    let cfg = crate::data_kinds::admitted_config(v.paths())?;
    if cfg.agent_viewer.build_semantic_index
        && vault_core::ensure_ai_capability_enabled(&cfg, AiCapability::SemanticIndex).is_ok()
    {
        return Err(Refusal::ActionUnavailable.into());
    }
    if job(v).is_some_and(|j| {
        matches!(j.state.lock().unwrap().phase, "running" | "paused" | "cancelling")
    }) {
        return Err(Refusal::Busy.into());
    }
    Ok(())
}
pub fn preview(s: &VaultSession, _: Empty) -> Result<Value> {
    let (v, _) = backups::admitted(s, true)?;
    let _gate = ControlGate::acquire(v.paths());
    check_off(&v)?;
    let kept = vectors::kept_index(v.agent_history()?.as_ref())?;
    let id = uuid::Uuid::new_v4();
    let receipt = json!({"previewId":id,"keptBytes":kept.bytes.to_string()});
    *PREVIEW.lock().unwrap() = Some(Preview {
        id,
        kept,
        owner: Arc::downgrade(v.store()),
        expires: Instant::now() + Duration::from_secs(300),
    });
    Ok(receipt)
}
pub fn free(s: &VaultSession, q: Confirm) -> Result<Value> {
    if !q.confirm {
        return Err(Refusal::InvalidRequest.into());
    }
    let (v, _) = backups::admitted(s, true)?;
    let _gate = ControlGate::acquire(v.paths());
    check_off(&v)?;
    let p = PREVIEW.lock().unwrap().take_if(|p| p.id == q.preview_id).ok_or(Refusal::Expired)?;
    if p.expires <= Instant::now() || !p.owner.ptr_eq(&Arc::downgrade(v.store())) {
        return Err(Refusal::Expired.into());
    }
    if !vectors::free_kept_index(v.agent_history()?.as_ref(), &p.kept)? {
        return Err(Refusal::Expired.into());
    }
    pages::forget(&v.paths().app_root);
    Ok(json!({"freedBytes":p.kept.bytes.to_string(),"keptBytes":"0"}))
}
