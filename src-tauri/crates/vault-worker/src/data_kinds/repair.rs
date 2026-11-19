//! Admits an explicit repair on the unlocked owner, then publishes its replacement.
//! Snapshot verification, retained evidence and crash recovery belong to vault-core.

use super::{PolicyChange, Refusal, admitted_config};
use crate::{ForegroundWork, VaultSession};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use vault_core::{DataKind, maintenance};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preview {
    copy_path: Option<PathBuf>,
    copy_password: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Execute {
    copy_path: Option<PathBuf>,
    copy_password: Option<String>,
    preview_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Repaired {
    pub kept_copy_path: String,
    pub availability: &'static str,
}

fn owner(session: &VaultSession) -> Result<vault_core::Vault> {
    let paths = vault_core::project_paths()?;
    if !admitted_config(&paths)?.initialized {
        return Err(Refusal::NotInitialized.into());
    }
    session.vault().map_err(anyhow::Error::msg)
}

/// Previews a named listed copy or the recovery possible from local evidence alone.
pub fn preview(
    session: &VaultSession,
    request: Preview,
) -> Result<maintenance::ConversationRepairPreview> {
    maintenance::preview_conversation_repair(
        &owner(session)?,
        request.copy_path.as_deref(),
        request.copy_password.as_deref(),
    )
}

/// Execute only the previewed replacement. Its policy notification follows publication.
pub fn execute(
    session: &VaultSession,
    request: Execute,
    emit: impl FnOnce(PolicyChange),
) -> Result<Repaired> {
    let vault = owner(session)?;
    let _running = crate::foreground_work::begin(ForegroundWork::Restore);
    let password = session.get_key().map_err(anyhow::Error::msg)?;
    let _lease = session.begin_maintenance().map_err(anyhow::Error::msg)?;
    if session.current_vault().is_none_or(|v| !std::sync::Arc::ptr_eq(v.store(), vault.store())) {
        return Err(Refusal::Replaced.into());
    }
    let installed = crate::maintenance_jobs::run(
        &vault.paths().app_root,
        "repairingConversations",
        ForegroundWork::Restore,
        |cancel, _| {
            maintenance::repair_conversations(
                &vault,
                request.copy_path.as_deref(),
                request.copy_password.as_deref(),
                &request.preview_token,
                cancel,
            )
        },
    )?;
    let kept_copy_path = installed.previous_files.display().to_string();
    let policy_token = installed.vault.config().capture_policy.token();
    session.install_archive(installed, password, crate::KeychainAfterInstall::AsSaved)?;
    emit(PolicyChange { policy_token, changed_kinds: vec![DataKind::AgentHistory] });
    Ok(Repaired { kept_copy_path, availability: "available" })
}

/// Describe the committed generation a manual safety copy will protect. The copy
/// takes the latest browsing commit when execute acquires the archive operation gate.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyCopyPreview {
    pub directory: String,
    pub conversations_unreadable: bool,
    pub preview_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveCopy {
    preview_token: String,
}

fn copy_plan(vault: &vault_core::Vault) -> Result<SafetyCopyPreview> {
    vault.recheck_root()?;
    vault.check_browsing_files()?;
    let directory = vault
        .paths()
        .artifact_path(vault_layout::ArtifactId::NativeSnapshots)
        .display()
        .to_string();
    let conversations_unreadable = vault.conversation_availability().is_err();
    let preview_token = vault_core::utils::sha256_hex(&serde_json::to_vec(&(
        &directory,
        conversations_unreadable,
        &vault.config().archive_mode,
    ))?);
    Ok(SafetyCopyPreview { directory, conversations_unreadable, preview_token })
}

pub fn preview_safety_copy(session: &VaultSession) -> Result<SafetyCopyPreview> {
    copy_plan(&owner(session)?)
}

pub fn create_safety_copy(
    session: &VaultSession,
    request: SaveCopy,
) -> Result<vault_core::RecoverySnapshot> {
    let vault = owner(session)?;
    let plan = copy_plan(&vault)?;
    anyhow::ensure!(
        request.preview_token == plan.preview_token,
        "conversation-repair-preview-stale"
    );
    maintenance::manual_snapshot(&vault, plan.conversations_unreadable)
}
