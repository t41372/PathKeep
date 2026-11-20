//! Interactive reads and guarded changes to device capture policy. This module
//! does not start a backup or grant access to an unreviewed source directory.
pub mod backups;
pub mod chats;
pub mod delivery;
mod hints;
pub mod insights;
pub mod projects;
mod snapshot;
pub use hints::{DismissHint, HintAcknowledgement, dismiss_hint};
pub use snapshot::{KindList, KindRow, Transition, list, list_with, retry_agent_store};
pub mod preview;
pub mod repair;
use crate::VaultSession;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use vault_core::{
    AgentTool, AppConfig, CapturePolicy, ControlGate, DataKind, KindState, ProjectPaths,
};

/// Require an explicit empty request on both transports.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

/// Manage Kinds requests physical sizes after the shell has painted.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ListKinds {
    pub include_archive_bytes: bool,
}

/// Carry the policy fingerprint the Manage Kinds control displayed.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetState {
    pub kind: DataKind,
    pub state: KindState,
    pub expected_policy_token: String,
}
/// Apply only the switches the user changed in this edit.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourcesPatch {
    pub expected_token: String,
    pub selected_root_ids: Option<Vec<String>>,
    pub preview_id: Option<String>,
    pub read_git: Option<bool>,
    #[serde(default)]
    pub tool_changes: std::collections::BTreeMap<AgentTool, bool>,
}

/// Stable domain causes shared by command replies and terminal preview frames.
#[derive(Debug, Clone, Copy)]
pub enum Refusal {
    LastActive,
    EnablePreview,
    PolicyChanged,
    Locked,
    ScopeDenied,
    EmptySelection,
    Destination,
    Cancelled,
    Busy,
    Replaced,
    RootChanged,
    Expired,
    NotInitialized,
    Unavailable,
    Off,
    InvalidRequest,
    Missing,
    InvalidCursor,
    ActionUnavailable,
    Finishing,
}
impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Self::LastActive => "last-active-kind",
            Self::EnablePreview => "enable-preview-required",
            Self::PolicyChanged => "policy-changed",
            Self::Locked => "lock-required",
            Self::ScopeDenied => "scope-denied",
            Self::EmptySelection => "source-selection-empty",
            Self::Destination => "destination-unavailable",
            Self::Cancelled => "cancelled",
            Self::Busy => "archive-busy",
            Self::Replaced => "archive-replaced",
            Self::RootChanged => "root-changed",
            Self::Expired => "preview-expired",
            Self::Unavailable => "kind-unavailable",
            Self::Off => "kind-off",
            Self::InvalidRequest => "invalid-request",
            Self::Missing => "record-unavailable",
            Self::ActionUnavailable => "task-action-unavailable",
            Self::InvalidCursor => "invalid-cursor",
            Self::Finishing => "operation-finishing",
            Self::NotInitialized => "archive-uninitialized",
        }
    }
}
impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for Refusal {}

/// Called both at admission and before delivering private metadata.
pub(super) fn admitted_config(paths: &ProjectPaths) -> Result<AppConfig> {
    let config = vault_core::load_config(paths)?;
    if vault_core::app_lock_status(paths, &config)?.locked {
        return Err(Refusal::Locked.into());
    }
    Ok(config)
}

/// Windows refetch these kinds after the guarded write succeeds.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyChange {
    pub policy_token: String,
    pub changed_kinds: Vec<DataKind>,
}
pub const POLICY_EVENT: &str = "pathkeep://data-kinds-changed";

/// Source controls share the same policy and comparison token as Manage Kinds.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub tools_chosen: bool,
    pub dismissed_hints: std::collections::BTreeSet<vault_core::AgentHint>,
    pub policy: CapturePolicy,
    pub policy_token: String,
}

/// Capture choices remain accessible to the unlocked manager while the kind is off.
pub fn settings() -> Result<Settings> {
    Ok(Settings::from(admitted_config(&vault_core::project_paths()?)?))
}

impl Settings {
    pub fn settings_token(&self) -> String {
        hints::settings_token(&self.policy, &self.dismissed_hints)
    }
}

impl From<AppConfig> for Settings {
    fn from(config: AppConfig) -> Self {
        let policy = config.capture_policy;
        Self {
            tools_chosen: policy.tools_chosen(),
            policy_token: policy.token(),
            policy,
            dismissed_hints: config.dismissed_agent_hints,
        }
    }
}

fn change(
    kind: DataKind,
    expected: &str,
    keep_ticket: Option<preview::ChoiceEdit<'_>>,
    mutate: impl FnOnce(&mut CapturePolicy, &[String]) -> Result<()>,
    emit: impl FnOnce(PolicyChange),
) -> Result<Settings> {
    let paths = vault_core::project_paths()?;
    let gate = ControlGate::acquire(&paths);
    let mut config = admitted_config(&paths)?;
    let before = config.capture_policy.token();
    if before != expected {
        return Err(Refusal::PolicyChanged.into());
    }
    mutate(&mut config.capture_policy, &config.selected_profile_ids)?;
    config.capture_policy.validate().map_err(|_| Refusal::LastActive)?;
    let token = config.capture_policy.token();
    if token != before {
        if !config.initialized {
            return Err(Refusal::NotInitialized.into());
        }
        vault_core::save_config(&gate, &config)?;
        preview::policy_changed(&paths.app_root, &before, &token, keep_ticket);
        drop(gate);
        emit(PolicyChange { policy_token: token.clone(), changed_kinds: vec![kind] });
    }
    Ok(Settings::from(config))
}

/// The interactive transition cannot stand in for first-save consent.
pub fn set_state(
    session: &VaultSession,
    req: SetState,
    emit: impl FnOnce(PolicyChange),
) -> Result<Transition> {
    let switching_to_agent_only = req.kind == DataKind::Browsing
        && req.state != KindState::On
        && admitted_config(&vault_core::project_paths()?)?.capture_policy.agent_history
            == KindState::On;
    let vault = if req.kind == DataKind::AgentHistory || switching_to_agent_only {
        let paths = vault_core::project_paths()?;
        let config = admitted_config(&paths)?;
        if config.initialized {
            let vault = session.vault().map_err(anyhow::Error::msg)?;
            backups::check_service(&vault)?;
            if config.capture_policy.pending_first_save.is_some() {
                backups::recover(session, &vault)?;
            }
            Some(vault)
        } else {
            None
        }
    } else {
        None
    };
    let _admission = session.conversations.enter();
    let granted = if switching_to_agent_only
        || req.kind == DataKind::AgentHistory && req.state == KindState::On
    {
        let policy = admitted_config(&vault_core::project_paths()?)?.capture_policy;
        vault.as_ref().map(|v| backups::has_grant(v, &policy)).transpose()?.unwrap_or(false)
    } else {
        false
    };
    let mut event = None;
    change(
        req.kind,
        &req.expected_policy_token,
        None,
        |policy, profiles| {
            let previous = policy.state(req.kind);
            match req.kind {
                DataKind::Browsing => policy.browsing = req.state,
                DataKind::AgentHistory => {
                    if req.state == KindState::On
                        && policy.agent_history != KindState::On
                        && !granted
                    {
                        return Err(Refusal::EnablePreview.into());
                    }
                    if req.state == KindState::Paused && policy.agent_history == KindState::Off {
                        policy.empty_tools_chosen =
                            policy.tools_chosen() && policy.enabled_tools.is_empty();
                        policy.selection_ref = None;
                    }
                    policy.explicitly_off = req.state == KindState::Off;
                    policy.agent_history = req.state;
                }
            }
            if previous == KindState::On && req.state != KindState::On {
                match req.kind {
                    DataKind::Browsing if policy.agent_history == KindState::On && !granted => {
                        return Err(Refusal::EmptySelection.into());
                    }
                    DataKind::AgentHistory
                        if policy.browsing == KindState::On && profiles.is_empty() =>
                    {
                        return Err(Refusal::EmptySelection.into());
                    }
                    _ => {}
                }
            }
            Ok(())
        },
        |changed| event = Some(changed),
    )?;
    let affected_tasks =
        if event.is_some() && req.kind == DataKind::AgentHistory && req.state != KindState::On {
            backups::cancel_active(session, None)
        } else {
            Vec::new()
        };
    if event.is_some() && req.kind == DataKind::AgentHistory && req.state == KindState::Off {
        delivery::revoke(&vault_core::project_paths()?.app_root);
    }
    if event.is_some()
        && let Some(vault) = &vault
    {
        if req.state == KindState::Off {
            vault.set_view_search_preparation(false)?;
        } else if req.state == KindState::Paused {
            prepare_view(vault, &admitted_config(vault.paths())?.capture_policy)?;
        }
    }
    drop(_admission);
    if let Some(event) = event {
        emit(event);
    }
    Ok(Transition { snapshot: list(session)?, affected_tasks })
}

/// Tool switches narrow future capture. While off they are choices for the
/// enable dialog. A switch alone never grants a directory.
pub fn update_sources(
    session: &VaultSession,
    req: SourcesPatch,
    emit: impl FnOnce(PolicyChange),
) -> Result<Settings> {
    let paths = vault_core::project_paths()?;
    let config = admitted_config(&paths)?;
    let vault = if config.initialized {
        let vault = session.vault().map_err(anyhow::Error::msg)?;
        backups::check_service(&vault)?;
        if config.capture_policy.pending_first_save.is_some() {
            backups::recover(session, &vault)?;
        }
        Some(vault)
    } else {
        None
    };
    let _admission = session.conversations.enter();
    if config.capture_policy.token() != req.expected_token {
        return Err(Refusal::PolicyChanged.into());
    }
    if req.selected_root_ids.as_ref().is_some_and(|ids| !ids.is_empty())
        && config.capture_policy.selection_ref.is_none()
    {
        return Err(Refusal::EnablePreview.into());
    }
    let selection = backups::edit_selection(vault.as_ref(), &config.capture_policy, &req)?;
    let approved_tools = if config.capture_policy.agent_history == KindState::On
        && config.capture_policy.browsing != KindState::On
    {
        if let Some((_, tools)) = &selection {
            Some(tools.clone())
        } else if let Some(v) = &vault {
            Some(
                backups::selection::device_roots(v, &config.capture_policy)?
                    .into_iter()
                    .map(|r| r.tool)
                    .collect(),
            )
        } else {
            None
        }
    } else {
        None
    };
    let settings = change(
        DataKind::AgentHistory,
        &req.expected_token,
        req.preview_id
            .as_deref()
            .filter(|_| req.selected_root_ids.is_none())
            .map(|ticket| preview::ChoiceEdit { ticket, tools: &req.tool_changes }),
        |policy, _| {
            if let Some((id, tools)) = &selection {
                policy.selection_ref = Some(id.clone());
                policy.enabled_tools.retain(|t| tools.contains(t));
            }
            let tools_edited = !req.tool_changes.is_empty();
            for (&tool, &on) in &req.tool_changes {
                if on {
                    if policy.agent_history != KindState::Off
                        && policy.selection_ref.is_some()
                        && !policy.enabled_tools.contains(&tool)
                        && !selection.as_ref().is_some_and(|(_, tools)| tools.contains(&tool))
                    {
                        return Err(Refusal::EnablePreview.into());
                    }
                    policy.enabled_tools.insert(tool);
                } else {
                    policy.enabled_tools.remove(&tool);
                }
            }
            if tools_edited {
                policy.empty_tools_chosen = policy.enabled_tools.is_empty();
            }
            if (tools_edited || req.selected_root_ids.is_some())
                && approved_tools
                    .as_ref()
                    .is_some_and(|tools| policy.enabled_tools.is_disjoint(tools))
            {
                return Err(Refusal::EmptySelection.into());
            }
            if let Some(read_git) = req.read_git {
                policy.read_git = read_git
            }
            Ok(())
        },
        emit,
    )?;
    if settings.policy != config.capture_policy {
        backups::cancel_active(session, None);
    }
    Ok(settings)
}

/// Whole-settings callers cannot construct a policy that the transition would refuse.
pub fn validate_config_input(config: &AppConfig) -> Result<()> {
    config.capture_policy.validate().map_err(|_| Refusal::LastActive)?;
    Ok(())
}

/// Newly visible retained records may have arrived without an index. Only the
/// search reader waits for the owner-started preparation.
pub(crate) fn prepare_view(vault: &vault_core::Vault, policy: &CapturePolicy) -> Result<()> {
    if vault.conversation_availability().is_ok()
        && policy.agent_state_if_present(true) != KindState::Off
        && !backups::has_grant(vault, policy)?
        && vault.has_saved_transcripts()?
    {
        vault.set_view_search_preparation(true)?;
    }
    Ok(())
}
