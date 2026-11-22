//! Issue and consume one owner-bound receipt for freeing browsing embeddings. The generation
//! collector owns publication, file leases and accounting. No provider is needed for removal.
use super::{ConfirmFreeIndex, Empty, Refusal, admitted_config};
use crate::{VaultSession, intelligence::INDEX_EXECUTION};
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use uuid::Uuid;
use vault_core::{AiCapability, ControlGate, Vault, ai, archive::ArchiveOpGate};

struct Preview {
    id: Uuid,
    owner: Weak<vault_store::Store>,
    kept: ai::KeptIndex,
    expires: Instant,
    config_stamp: String,
}
static PREVIEW: Mutex<Option<Preview>> = Mutex::new(None);

fn owner(s: &VaultSession) -> Result<(Vault, ArchiveOpGate)> {
    let paths = vault_core::project_paths()?;
    if !admitted_config(&paths)?.initialized {
        return Err(Refusal::NotInitialized.into());
    }
    let v = match s.idle_vault()? {
        Some(v) => v,
        None => s.vault().map_err(anyhow::Error::msg)?,
    };
    let op = ArchiveOpGate::try_acquire(v.paths()).ok_or(Refusal::Busy)?;
    v.recheck_root()?;
    if !s.idle_vault()?.is_some_and(|current| Arc::ptr_eq(current.store(), v.store())) {
        return Err(Refusal::Busy.into());
    }
    Ok((v, op))
}

fn check_off(v: &Vault) -> Result<String> {
    let cfg = admitted_config(v.paths())?;
    if vault_core::ensure_ai_capability_enabled(&cfg, AiCapability::SemanticIndex).is_ok() {
        return Err(Refusal::ActionUnavailable.into());
    }
    vault_core::utils::file_sha256_hex(&v.paths().config_path)
}

fn refusal(error: anyhow::Error) -> anyhow::Error {
    match error.downcast_ref::<ai::RemovalRefusal>() {
        Some(ai::RemovalRefusal::Busy) => Refusal::Busy.into(),
        Some(ai::RemovalRefusal::Expired) => Refusal::Expired.into(),
        None => error,
    }
}

/// Review all retained models on this archive, replacing its previous confirmation.
pub fn preview(s: &VaultSession, _: Empty) -> Result<Value> {
    let (v, _op) = owner(s)?;
    let _execution = INDEX_EXECUTION.try_enter().ok_or(Refusal::Busy)?;
    let config_stamp = check_off(&v)?;
    let kept = ai::kept_browsing_index(&v).map_err(refusal)?;
    let id = Uuid::new_v4();
    let reply = json!({"previewId":id,"keptBytes":kept.bytes.to_string()});
    *PREVIEW.lock().unwrap() = Some(Preview {
        id,
        kept,
        owner: Arc::downgrade(v.store()),
        config_stamp,
        expires: Instant::now() + Duration::from_secs(300),
    });
    Ok(reply)
}

/// Consume the reviewed receipt. Settings can change again once the empty pointers commit.
pub fn free(s: &VaultSession, q: ConfirmFreeIndex) -> Result<Value> {
    if !q.confirm {
        return Err(Refusal::InvalidRequest.into());
    }
    let (v, _op) = owner(s)?;
    let _execution = INDEX_EXECUTION.try_enter().ok_or(Refusal::Busy)?;
    let config_stamp = check_off(&v)?;
    let receipt =
        PREVIEW.lock().unwrap().take_if(|p| p.id == q.preview_id).ok_or(Refusal::Expired)?;
    if receipt.expires <= Instant::now()
        || !receipt.owner.ptr_eq(&Arc::downgrade(v.store()))
        || config_stamp != receipt.config_stamp
    {
        return Err(Refusal::Expired.into());
    }
    let _work = crate::foreground_work::begin(crate::ForegroundWork::FreeUpSpace);
    ai::free_browsing_index(&v, &receipt.kept, || {
        let control = ControlGate::acquire(v.paths());
        if check_off(&v)? != receipt.config_stamp || receipt.expires <= Instant::now() {
            return Err(Refusal::Expired.into());
        }
        Ok(control)
    })
    .map_err(refusal)?;
    Ok(json!({"freedBytes":receipt.kept.bytes.to_string(),"keptBytes":"0"}))
}
