//! Menu-bar and scheduled Agent runs use the same reviewed roots, admission slot and task receipts
//! as window-triggered runs. The owner keeps a scheduled request alive until capture finishes.
use super::*;
use chrono::{DateTime, Utc};

pub(crate) fn due_anchor(vault: &Vault) -> Result<Option<DateTime<Utc>>> {
    Ok(vault
        .store()
        .read(Db::AgentHistoryEvidence, |tx| {
            tx.query_one(
                "SELECT max(finished_at) FROM ah_backup_runs WHERE status='complete'",
                (),
                |r| Ok(r.get::<Option<i64>>(0)?),
            )
        })?
        .and_then(DateTime::from_timestamp_millis))
}

/// A shared command has no grant in either off state. If this installation is viewing
/// retained transcripts, its skip receipt carries the same paused state as the window.
pub(super) fn skip_reason(session: &VaultSession, policy: &CapturePolicy) -> Option<&'static str> {
    if policy.agent_state_if_present(true) != policy.agent_history
        && session
            .current_vault()
            .is_some_and(|v| v.agent_visibility(policy).ok() == Some(KindState::Paused))
    {
        return Some("kind-paused");
    }
    policy.backup_skip_reason(DataKind::AgentHistory, false)
}

/// A scheduled owner can collect the result after running the browsing part. The menu bar
/// observes completion through the normal task events and can release this handle immediately.
pub enum SharedRun {
    Answer(Value),
    Capturing { vault: Box<Vault>, task: String, handle: std::thread::JoinHandle<()> },
}
impl SharedRun {
    pub fn wait(self) -> Result<Value> {
        match self {
            Self::Answer(answer) => Ok(answer),
            Self::Capturing { vault, task, handle } => {
                handle.join().map_err(|_| anyhow::anyhow!("conversation-backup-worker-stopped"))?;
                runs::record(&vault, &task)?.row(&vault)
            }
        }
    }
}
pub fn start_shared(session: &VaultSession, scheduled: bool) -> Result<SharedRun> {
    let paths = crate::context::usable_root()?;
    let policy = vault_core::load_config(&paths)?.capture_policy;
    let skipped =
        |reason: &str| json!({"kind":"agent-history","decision":"skipped","reason":reason});
    if let Some(reason) = skip_reason(session, &policy) {
        return Ok(SharedRun::Answer(skipped(reason)));
    }
    let vault = if scheduled { session.background_vault() } else { session.vault() }
        .map_err(anyhow::Error::msg)?;
    let gate = session.conversations.enter();
    let cfg = vault_core::load_config(&paths)?;
    let policy = &cfg.capture_policy;
    if let Some(reason) = skip_reason(session, policy) {
        return Ok(SharedRun::Answer(skipped(reason)));
    }
    if let Some(active) = session.conversations.active.lock().unwrap().as_ref() {
        return Ok(SharedRun::Answer(active.intent.dispatch("already-running")));
    }
    let Some(selection_id) = policy.selection_ref.clone() else {
        return Ok(SharedRun::Answer(skipped("no-source")));
    };
    if scheduled {
        let interval = vault_platform::ScheduleParameters::from(&cfg).wake_interval(None);
        let delay = vault_core::archive::backup_due_delay(cfg.due_after_hours, interval);
        if due_anchor(&vault)?.is_some_and(|last| {
            Utc::now() - last < chrono::Duration::from_std(delay).unwrap_or(chrono::Duration::MAX)
        }) {
            return Ok(SharedRun::Answer(skipped("not-due")));
        }
    }
    let roots = selection::device_roots(&vault, policy)?
        .into_iter()
        .filter(|r| policy.enabled_tools.contains(&r.tool))
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Ok(SharedRun::Answer(skipped("no-source")));
    }
    let task = uuid::Uuid::new_v4().to_string();
    let trigger = if scheduled { "scheduled" } else { "menu-bar" };
    let intent = Intent {
        request: task.clone(),
        hash: hash(&(&task, trigger))?,
        task: task.clone(),
        scope: Scope { selection_id, roots, read_git: policy.read_git },
        trigger: trigger.into(),
        retry_of: None,
    };
    let events =
        session.conversations.recovery_events.get().cloned().unwrap_or_else(|| Arc::new(|_, _| {}));
    let producer = Producer::reserve(&vault, &session.conversations, &intent, &events)?;
    let run = intent.insert(&vault)?;
    let handle =
        producer.launch(vault.clone(), session.conversations.clone(), intent, run, events)?;
    drop(gate);
    Ok(SharedRun::Capturing { vault: Box::new(vault), task, handle })
}
