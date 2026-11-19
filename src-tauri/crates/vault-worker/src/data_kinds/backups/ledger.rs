//! Couple the desktop receipt to the native run in the same evidence transaction.
//! Scope and request digests survive the window and are available after owner loss.
use super::{Refusal, selection::Root};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vault_core::Vault;
use vault_store::{Db, OptionalRow, Priority, params};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Scope {
    pub selection_id: String,
    pub roots: Vec<Root>,
    pub read_git: bool,
}

#[derive(Clone)]
pub(super) struct Intent {
    pub task: String,
    pub request: String,
    pub hash: String,
    pub scope: Scope,
    pub trigger: String,
    pub retry_of: Option<String>,
}

impl Intent {
    pub fn start(&self) -> Value {
        json!({"taskId":self.task,"runRef":format!("agent-history:{}",self.task),"requestId":self.request})
    }
    pub fn dispatch(&self, decision: &str) -> Value {
        dispatch(&self.task, decision)
    }
    pub fn insert(&self, vault: &Vault) -> Result<i64> {
        let intent = self.clone();
        let scope = serde_json::to_string(&self.scope)?;
        Ok(vault.store().write(Db::AgentHistoryEvidence,Priority::Interactive,move |tx|{
            let run = tx.insert("INSERT INTO ah_backup_runs(status,started_at,public_id) VALUES(?3,?1,?2)",
                params![chrono::Utc::now().timestamp_millis(),intent.task.clone(),if intent.trigger=="enable"{"queued"}else{"running"}])?.ok_or(vault_store::Error::Closed)?;
            tx.execute("INSERT INTO ah_desktop_backups(run_id,task_id,request_id,request_hash,selection_id,scope_json,trigger,retry_of)
                VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![run,intent.task,intent.request,intent.hash,intent.scope.selection_id,scope,intent.trigger,intent.retry_of])?;
            for root in &intent.scope.roots {
                tx.execute("INSERT INTO ah_run_roots(run_id,root_id,tool,state) VALUES(?1,?2,?3,'stale')",params![run,root.id.clone(),root.tool.harness().as_str()])?;
                tx.execute("INSERT OR REPLACE INTO ah_root_state(harness,root,state,run_id) VALUES(?1,?2,'stale',?3)",
                    params![root.source().harness.to_string(),root.path.to_string_lossy().into_owned(),run])?;
            }
            Ok(run)
        })?)
    }
}

pub(super) fn receipt(vault: &Vault, request: &str, expected_hash: &str) -> Result<Option<Intent>> {
    let request = request.to_owned();
    let row = vault.store().read(Db::AgentHistoryEvidence,move |tx|{
        tx.query_one("SELECT task_id,request_id,request_hash,scope_json,trigger,retry_of FROM ah_desktop_backups WHERE request_id=?1",
            [request],|r|Ok((r.get::<String>(0)?,r.get::<String>(1)?,r.get::<String>(2)?,r.get::<String>(3)?,r.get::<String>(4)?,r.get::<Option<String>>(5)?))).optional()
    })?;
    let Some((task, request, hash, scope, trigger, retry_of)) = row else { return Ok(None) };
    if hash != expected_hash {
        return Err(Refusal::InvalidRequest.into());
    }
    Ok(Some(Intent {
        task,
        request,
        hash,
        scope: serde_json::from_str(&scope)?,
        trigger,
        retry_of,
    }))
}

/// Actions and busy dispatches refer to an existing attempt. They have a
/// delivery receipt, but do not create another native backup run.
pub(super) fn delivery(
    vault: &Vault,
    request: &str,
    expected: &str,
) -> Result<Option<(String, String)>> {
    let request = request.to_owned();
    let saved = vault.store().read(Db::AgentHistoryEvidence, move |tx| {
        tx.query_one(
            "SELECT task_id,decision,request_hash FROM ah_backup_deliveries WHERE request_id=?1",
            [request],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
        )
        .optional()
    })?;
    match saved {
        Some((task, decision, hash)) if hash == expected => Ok(Some((task, decision))),
        Some(_) => Err(Refusal::InvalidRequest.into()),
        None => Ok(None),
    }
}

pub(super) fn remember(
    vault: &Vault,
    request: String,
    hash: String,
    task: String,
    decision: &'static str,
    notice: Option<(String, String)>,
) -> Result<()> {
    vault.store().write(Db::AgentHistoryEvidence,Priority::Interactive,move |tx|{
        tx.execute("INSERT INTO ah_backup_deliveries(request_id,request_hash,task_id,decision) VALUES(?1,?2,?3,?4)",
            params![request,hash,task.clone(),decision])?;
        if let Some((tool,version))=notice {
            tx.execute("UPDATE ah_format_notices SET dismissed=1 WHERE tool=?2 AND version=?3 AND run_id=(SELECT run_id FROM ah_desktop_backups WHERE task_id=?1)",params![task,tool,version])?;
        }else if decision=="dismiss" {
            tx.execute("UPDATE ah_desktop_backups SET acknowledged=1 WHERE task_id=?1",[task])?;
        }else if decision=="cancel" {
            tx.execute("UPDATE ah_desktop_backups SET stop_reason='cancelled' WHERE task_id=?1",[task])?;
        }
        Ok(())
    })?;
    Ok(())
}

pub(super) fn dispatch(task: &str, decision: &str) -> Value {
    json!({"batchId":task,"kinds":[{"kind":"agent-history","decision":decision,"taskId":task,"runRef":format!("agent-history:{task}")}]})
}

/// A copied queue row is history. Only the live device's pending request and
/// descriptor together authorize claiming it.
pub(super) fn interrupt_foreign_queue(
    vault: &Vault,
    policy: &vault_core::CapturePolicy,
) -> Result<()> {
    let queued = vault.store().read(Db::AgentHistoryEvidence, |tx| {
        tx.query_one("SELECT EXISTS(SELECT 1 FROM ah_backup_runs WHERE status='queued')", (), |r| {
            Ok(r.get::<i64>(0)? != 0)
        })
    })?;
    if queued {
        let pending = if policy.agent_history == vault_core::KindState::On {
            policy.pending_first_save.clone()
        } else {
            None
        };
        let selection = policy.selection_ref.clone();
        vault.store().write(Db::AgentHistoryEvidence, Priority::Interactive, move |tx| {
            tx.execute(
                "UPDATE ah_backup_runs SET status='interrupted',finished_at=?3,report_json=?4
                WHERE status='queued' AND id NOT IN (SELECT run_id FROM ah_desktop_backups
                WHERE selection_id=?1 AND request_id=?2 AND trigger='enable')",
                params![
                    selection,
                    pending,
                    chrono::Utc::now().timestamp_millis(),
                    r#"{"reason":"owner_restarted"}"#
                ],
            )?;
            Ok(())
        })?;
    }
    Ok(())
}

/// The producer claims the existing receipt before touching any source. The
/// owner can clear its pending control as soon as this transaction commits.
pub(super) fn claim_first_save(vault: &Vault, run: i64) -> Result<()> {
    let claimed =
        vault.store().write(Db::AgentHistoryEvidence, Priority::Interactive, move |tx| {
            tx.execute(
                "UPDATE ah_backup_runs SET status='running' WHERE id=?1 AND status='queued'",
                [run],
            )
        })?;
    if claimed != 1 {
        return Err(Refusal::PolicyChanged.into());
    }
    Ok(())
}
