//! Owner admission for a reviewed first save and subsequent manual conversation
//! backups. The device control grants scope. The evidence role owns run history.
mod browsing_runs;
#[cfg(test)]
mod crash_tests;
mod ledger;
mod notes;
mod overview;
pub(crate) use overview::first_backup;
pub use overview::{ListSources, Overview, overview, sources};
mod runs;
mod tasks;
pub use runs::{GetRun, ListRuns, get_run, list_runs};
pub(crate) use tasks::cancel_active;
pub use tasks::{Control, ListTasks, control, task_summary, tasks};
mod progress;
mod runtime;
mod selection;
use super::{Refusal, admitted_config, preview};
use crate::VaultSession;
use anyhow::Result;
use ledger::{Intent, Scope};
use runtime::Producer;
pub(crate) use runtime::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use vault_core::{
    AgentTool, AppConfig, CapturePolicy, ControlGate, DataKind, KindState, ProjectPaths, Vault,
};
use vault_store::Db;

pub type Events = Arc<dyn Fn(&'static str, Value) + Send + Sync>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Enable {
    preview_id: String,
    selected_root_ids: Vec<String>,
    read_git: bool,
    expected_policy_token: String,
    request_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    scope: BackupScope,
    request_id: String,
    expected_policy_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum BackupScope {
    Kind { kind: DataKind },
    Tool { kind: DataKind, tool: AgentTool },
}

fn hash(request: &impl Serialize) -> Result<String> {
    Ok(vault_core::utils::sha256_hex(&serde_json::to_vec(request)?))
}
fn check_id(id: &str) -> Result<()> {
    uuid::Uuid::parse_str(id).map_err(|_| Refusal::InvalidRequest)?;
    Ok(())
}
fn can_deliver(paths: &ProjectPaths) -> bool {
    // Only capture producers call this. They already own an explicit device grant.
    admitted_config(paths).is_ok_and(|c| c.capture_policy.agent_history != KindState::Off)
}
fn admitted(session: &VaultSession, allow_off: bool) -> Result<(Vault, AppConfig)> {
    let mut config = admitted_config(&vault_core::project_paths()?)?;
    if !config.initialized {
        return Err(Refusal::NotInitialized.into());
    }
    let vault = session.vault().map_err(anyhow::Error::msg)?;
    check_service(&vault)?;
    session.conversations.reconcile_terminal(&vault)?;
    if config.capture_policy.pending_first_save.is_some() {
        recover(session, &vault)?;
        config = admitted_config(vault.paths())?;
    }
    if !allow_off && vault.agent_visibility(&config.capture_policy)? == KindState::Off {
        return Err(Refusal::Off.into());
    }
    Ok((vault, config))
}
fn check_reply(vault: &Vault, session: &VaultSession, allow_off: bool) -> Result<CapturePolicy> {
    let config = admitted_config(vault.paths())?;
    if session.current_vault().is_none_or(|v| !Arc::ptr_eq(v.store(), vault.store())) {
        return Err(Refusal::Replaced.into());
    }
    if !allow_off && vault.agent_visibility(&config.capture_policy)? == KindState::Off {
        return Err(Refusal::Off.into());
    }
    Ok(config.capture_policy)
}
fn clear_pending(vault: &Vault, request: &str) -> Result<()> {
    let gate = ControlGate::acquire(vault.paths());
    let mut config = vault_core::load_config(vault.paths())?;
    if config.capture_policy.pending_first_save.as_deref() == Some(request) {
        config.capture_policy.pending_first_save = None;
        vault_core::save_config(&gate, &config)?;
    }
    Ok(())
}

/// Publish consent before acknowledgement. A replay looks up the original
/// request before checking the expired preview or the pre-enable policy token.
pub fn enable(session: &VaultSession, req: Enable, events: Events) -> Result<Value> {
    let (vault, _) = admitted(session, true)?;
    let _admission = session.conversations.enter();
    let policy = check_reply(&vault, session, true)?;
    check_id(&req.request_id)?;
    let fingerprint = hash(&req)?;
    ledger::delivery(&vault, &req.request_id, &fingerprint)?;
    if let Some(existing) = ledger::receipt(&vault, &req.request_id, &fingerprint)? {
        if policy.agent_history != KindState::On {
            return Err(Refusal::PolicyChanged.into());
        }
        return Ok(json!({"state":"on","policyToken":policy.token(),"firstSave":existing.start()}));
    }
    if policy.token() != req.expected_policy_token
        || (policy.agent_history == KindState::On && selection::descriptor_exists(&vault, &policy)?)
        || (policy.agent_history == KindState::Paused && has_grant(&vault, &policy)?)
    {
        return Err(Refusal::PolicyChanged.into());
    }
    let roots = selection::reviewed(&vault, &req.preview_id, &req.selected_root_ids, &[])?;
    let selection_id = selection::stage(&vault, &roots, Some(fingerprint.clone()), None)?;
    let intent = Intent {
        task: selection_id.clone(),
        request: req.request_id.clone(),
        hash: fingerprint,
        scope: Scope { selection_id: selection_id.clone(), roots, read_git: req.read_git },
        trigger: "enable".into(),
        retry_of: None,
    };
    let producer = Producer::reserve(&vault, &session.conversations, &intent, &events)?;
    #[cfg(test)]
    crash_tests::boundary("reserved");
    if !intent.scope.roots.iter().any(|root| root.issue(&vault).is_none()) {
        return Err(Refusal::RootChanged.into());
    }
    let token = {
        let gate = ControlGate::acquire(vault.paths());
        let mut config = admitted_config(vault.paths())?;
        if config.capture_policy != policy {
            return Err(Refusal::PolicyChanged.into());
        }
        let policy = &mut config.capture_policy;
        policy.agent_history = KindState::On;
        policy.explicitly_off = false;
        policy.empty_tools_chosen = false;
        policy.read_git = req.read_git;
        policy.enabled_tools = intent.scope.roots.iter().map(|r| r.tool).collect();
        policy.selection_ref = Some(selection_id);
        policy.pending_first_save = Some(req.request_id.clone());
        let token = policy.token();
        vault_core::save_config(&gate, &config)?;
        token
    };
    #[cfg(test)]
    crash_tests::boundary("published");
    vault.set_view_search_preparation(false)?;
    preview::revoke(&vault.paths().app_root);
    let run = intent.insert(&vault)?;
    #[cfg(test)]
    crash_tests::boundary("created");
    let receipt = intent.start();
    producer.launch(vault.clone(), session.conversations.clone(), intent, run, events.clone())?;
    // A failure clearing control cannot strand an admitted native run.
    clear_pending(&vault, &req.request_id)?;
    check_reply(&vault, session, false)?;
    let settings = super::settings()?;
    events(
        "pathkeep://agent-settings-changed",
        json!({"settingsToken":settings.settings_token(),"domains":["sources"]}),
    );
    events(super::POLICY_EVENT, json!({"policyToken":token,"changedKinds":["agent-history"]}));
    Ok(json!({"state":"on","policyToken":token,"firstSave":receipt}))
}

/// Manual tool and kind requests share the same capture slot.
pub fn start(session: &VaultSession, req: Start, events: Events) -> Result<Value> {
    let (vault, _) = admitted(session, false)?;
    let _admission = session.conversations.enter();
    let policy = check_reply(&vault, session, false)?;
    check_id(&req.request_id)?;
    let fingerprint = hash(&req)?;
    if policy.selection_ref.is_none() {
        return Err(Refusal::EnablePreview.into());
    }
    if let Some(intent) = ledger::receipt(&vault, &req.request_id, &fingerprint)? {
        return Ok(intent.dispatch("started"));
    }
    if let Some((task, decision)) = ledger::delivery(&vault, &req.request_id, &fingerprint)? {
        return Ok(ledger::dispatch(&task, &decision));
    }
    if req.expected_policy_token.as_ref().is_some_and(|v| v != &policy.token()) {
        return Err(Refusal::PolicyChanged.into());
    }
    let tool = match req.scope {
        BackupScope::Kind { kind: DataKind::AgentHistory } => None,
        BackupScope::Tool { kind: DataKind::AgentHistory, tool }
            if policy.enabled_tools.contains(&tool) =>
        {
            Some(tool)
        }
        _ => return Err(Refusal::ScopeDenied.into()),
    };
    let active =
        session.conversations.active.lock().unwrap().as_ref().map(|a| a.intent.task.clone());
    if let Some(task) = active {
        ledger::remember(
            &vault,
            req.request_id,
            fingerprint,
            task.clone(),
            "already-running",
            None,
        )?;
        return Ok(ledger::dispatch(&task, "already-running"));
    }
    let selection_id = policy.selection_ref.ok_or(Refusal::EnablePreview)?;
    let roots: Vec<_> = selection::load(&vault, &selection_id)?
        .into_iter()
        .filter(|r| policy.enabled_tools.contains(&r.tool) && tool.is_none_or(|t| t == r.tool))
        .collect();
    if roots.is_empty() {
        return Err(Refusal::EmptySelection.into());
    }
    let intent = Intent {
        task: uuid::Uuid::new_v4().to_string(),
        request: req.request_id,
        hash: fingerprint,
        scope: Scope { selection_id, roots, read_git: policy.read_git },
        trigger: "manual".into(),
        retry_of: None,
    };
    let receipt = intent.dispatch("started");
    launch(session, &vault, intent, events)?;
    check_reply(&vault, session, false)?;
    Ok(receipt)
}
fn launch(session: &VaultSession, vault: &Vault, intent: Intent, events: Events) -> Result<()> {
    let producer = Producer::reserve(vault, &session.conversations, &intent, &events)?;
    let run = intent.insert(vault)?;
    producer.launch(vault.clone(), session.conversations.clone(), intent, run, events)
}

/// Reconcile the live control when this owner first opens the generation.
/// Copied descriptors cannot start work in the absence of a pending device intent.
pub(crate) fn recover(session: &VaultSession, vault: &Vault) -> Result<()> {
    let _admission = session.conversations.enter();
    let policy = vault_core::load_config(vault.paths())?.capture_policy;
    if vault.conversation_availability().is_ok() {
        ledger::interrupt_foreign_queue(vault, &policy)?;
    }
    let Some(id) = policy.selection_ref else {
        if vault.conversation_availability().is_ok()
            && vault.store().read(Db::AgentHistoryEvidence, |s| {
                s.query_one(
                    "SELECT EXISTS(SELECT 1 FROM ah_backup_runs WHERE status='running')",
                    (),
                    |r| Ok(r.get::<i64>(0)? != 0),
                )
            })?
        {
            vault.agent_history()?;
        }
        return Ok(());
    };
    vault.agent_history()?; // Marks an already-started run interrupted.
    let Some(request) = policy.pending_first_save else { return Ok(()) };
    if policy.agent_history != KindState::On {
        return clear_pending(vault, &request);
    }
    let key = id.clone();
    let fingerprint = vault.store().read(Db::AgentHistoryEvidence, move |s| {
        s.query_one("SELECT enable_hash FROM ah_capture_selections WHERE id=?1", [key], |r| {
            Ok(r.get::<String>(0)?)
        })
    })?;
    let events =
        session.conversations.recovery_events.get().cloned().unwrap_or_else(|| Arc::new(|_, _| {}));
    if let Some(intent) = ledger::receipt(vault, &request, &fingerprint)? {
        let saved = runs::record(vault, &intent.task)?;
        if saved.status == "queued" {
            let producer = Producer::reserve(vault, &session.conversations, &intent, &events)?;
            producer.launch(
                vault.clone(),
                session.conversations.clone(),
                intent,
                saved.id,
                events,
            )?;
        }
    } else {
        let roots = selection::load(vault, &id)?;
        let intent = Intent {
            task: id.clone(),
            request: request.clone(),
            hash: fingerprint,
            scope: Scope { selection_id: id, roots, read_git: policy.read_git },
            trigger: "enable".into(),
            retry_of: None,
        };
        launch(session, vault, intent, events)?;
    }
    clear_pending(vault, &request)
}

pub(super) fn result_name(status: &str) -> &str {
    match status {
        "complete" => "success",
        "queued" => "running",
        other => other,
    }
}

/// Retained bindings can be removed without a walk. Every added binding comes
/// from the same completed, still-valid preview as the explicit selection.
pub(super) fn edit_selection(
    vault: Option<&Vault>,
    policy: &CapturePolicy,
    patch: &super::SourcesPatch,
) -> Result<Option<(String, std::collections::BTreeSet<AgentTool>)>> {
    let Some(ids) = &patch.selected_root_ids else {
        if patch.tool_changes.values().any(|on| *on)
            && let (Some(vault), Some(key)) = (vault, &policy.selection_ref)
        {
            let roots = selection::load(vault, key)?;
            return Ok(Some((key.clone(), roots.iter().map(|r| r.tool).collect())));
        }
        return Ok(None);
    };
    let vault = vault.ok_or(Refusal::NotInitialized)?;
    if ids.is_empty() && policy.selection_ref.is_none() {
        return Ok(None);
    }
    let mut roots =
        match policy.selection_ref.as_deref().map(|key| selection::load(vault, key)).transpose() {
            Ok(value) => value.unwrap_or_default(),
            Err(e)
                if e.downcast_ref::<Refusal>()
                    .is_some_and(|r| matches!(r, Refusal::EnablePreview))
                    && patch.preview_id.is_some() =>
            {
                vec![]
            }
            Err(e) => return Err(e),
        };
    let previous = roots.clone();
    roots.retain(|r| ids.contains(&r.id));
    let retained = roots.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    if let Some(preview) = &patch.preview_id
        && !ids.is_empty()
    {
        let reviewed = selection::reviewed(vault, preview, ids, &retained)?;
        for binding in reviewed {
            if let Some(prior) = roots.iter_mut().find(|r| r.id == binding.id) {
                *prior = binding;
            } else {
                roots.push(binding);
            }
        }
    } else if ids.iter().any(|id| !retained.contains(id)) {
        return Err(Refusal::EnablePreview.into());
    }
    if roots.len() != ids.len() {
        return Err(Refusal::ScopeDenied.into());
    }
    let key = if roots == previous
        && let Some(key) = &policy.selection_ref
    {
        key.clone()
    } else {
        selection::stage(vault, &roots, None, policy.selection_ref.clone())?
    };
    Ok(Some((key, roots.iter().map(|r| r.tool).collect())))
}

pub(super) fn active_id(session: &VaultSession) -> Option<String> {
    session.conversations.active.lock().unwrap().as_ref().map(|a| a.intent.task.clone())
}

/// Install the desktop notification sink before startup recovery can claim a first save.
pub fn register_recovery_events(session: &VaultSession, events: Events) {
    let _ = session.conversations.recovery_events.set(events);
}

pub(super) fn check_service(vault: &Vault) -> Result<()> {
    vault.conversation_availability().map_err(|_| Refusal::Unavailable)?;
    vault.agent_history().map(|_| ()).map_err(|error| match error {
        agent_history::Error::Cancelled => Refusal::Busy.into(),
        _ => Refusal::Unavailable.into(),
    })
}

/// A local grant needs an enabled tool and its matching immutable descriptor.
/// Presence and readability of external folders are checked only during execution.
pub(super) fn has_grant(vault: &Vault, policy: &CapturePolicy) -> Result<bool> {
    if policy.agent_history == KindState::Off || policy.enabled_tools.is_empty() {
        return Ok(false);
    }
    Ok(selection::device_roots(vault, policy)?
        .iter()
        .any(|root| policy.enabled_tools.contains(&root.tool)))
}
