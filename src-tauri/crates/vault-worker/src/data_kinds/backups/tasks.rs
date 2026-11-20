//! Project native backup attempts into the shared Tasks contract. An action is
//! checked against the latest task state and the current device grant.
use super::runs::{Record, decode, limit, page, record};
use super::*;
use chrono::{Local, TimeZone};
use vault_store::params;

/// Select one group of attempts from the Tasks dashboard.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListTasks {
    pub(super) group: Group,
    kind: Option<TaskKind>,
    pub(super) cursor: Option<String>,
    pub(super) limit: usize,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TaskKind {
    Browsing,
    AgentHistory,
    Archive,
}
impl ListTasks {
    pub(super) fn archive_only(&self) -> bool {
        matches!(self.kind, Some(TaskKind::Archive))
    }
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Group {
    Running,
    Attention,
    Waiting,
    Today,
}
/// Bind an action to the attempt and phase the user reviewed.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Control {
    task_id: String,
    action: Action,
    #[serde(default)]
    expected_token: String,
    #[serde(default)]
    request_id: String,
    confirmation: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notice: Option<Notice>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Notice {
    tool: AgentTool,
    version: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Action {
    Cancel,
    Retry,
    Dismiss,
    #[serde(rename = "dismiss-notice")]
    DismissNotice,
    Pause,
    Resume,
}

pub(super) fn task_row(runtime: &Runtime, r: &Record) -> Result<Value> {
    let id = r.task.as_deref().ok_or(Refusal::Missing)?;
    let mut state = match r.status.as_str() {
        "complete" => "succeeded",
        other => other,
    };
    let attention =
        (matches!(state, "partial" | "failed") && !r.ack) || !r.format_notices.is_empty();
    let mut progress = None;
    let active = runtime.active.lock().unwrap();
    if state == "running"
        && let Some(a) = active.as_ref().filter(|a| a.intent.task == id)
    {
        if a.token.check().is_err() {
            state = "cancelling";
        } else if a.index_pause.paused() {
            state = "paused";
        }
        progress = Some(a.progress.clone());
    }
    drop(active);
    let phase = progress.as_ref().and_then(|p| p["phase"].as_str()).unwrap_or(match state {
        "running" => "discovery",
        "queued" => "queued",
        _ => "finished",
    });
    let mut actions = Vec::new();
    match state {
        "running" if phase != "finishing" => actions.push("cancel"),
        "partial" | "failed" | "interrupted" => actions.push("retry"),
        _ => (),
    }
    if matches!(state, "partial" | "failed") && !r.ack {
        actions.push("dismiss");
    }
    if phase == "index" {
        match state {
            "running" => actions.push("pause"),
            "paused" => actions.extend(["cancel", "resume"]),
            _ => {}
        }
    }
    let token = if r.format_notices.is_empty() {
        hash(&(id, state, phase, &actions, attention))?
    } else {
        actions.push("dismiss-notice");
        hash(&(id, state, phase, &actions, attention, &r.format_notices))?
    };
    let mut row = json!({"taskId":id,"kind":"agent-history","operation":"backup","trigger":r.trigger,"state":state,
        "phase":phase,"runRef":format!("agent-history:{}",r.public),"startedAt":r.started,"attention":attention,
        "actions":actions,"holdsQuit":matches!(state,"running"|"cancelling"|"paused"),"snapshotToken":token});
    if !r.format_notices.is_empty() {
        row["formatNotices"] = json!(r.format_notices);
    }
    if let Some(mut p) = progress {
        if let Some(tools) = p.as_object_mut().and_then(|v| v.remove("perTool")) {
            row["perTool"] = tools;
        }
        row["progress"] = p;
    }
    if let Some(end) = r.finished {
        row["endedAt"] = json!(end);
    }
    if let Some(retry) = &r.retry_of {
        row["retryOf"] = json!(retry);
    }
    if let Some(reason) = r
        .stop_reason
        .as_deref()
        .or_else(|| r.report["reason"].as_str())
        .or_else(|| r.report["error_code"].as_str())
    {
        row["reason"] = json!(if reason == "owner_restarted" { "owner-restarted" } else { reason });
    }
    Ok(row)
}

pub(super) fn local_day(date: chrono::NaiveDate) -> Result<(i64, i64)> {
    let midnight = |date: chrono::NaiveDate| -> Result<i64> {
        let naive = date.and_hms_opt(0, 0, 0).ok_or(Refusal::InvalidRequest)?;
        (0..=180)
            .find_map(|mins| {
                Local.from_local_datetime(&(naive + chrono::Duration::minutes(mins))).earliest()
            })
            .map(|v| v.timestamp_millis())
            .ok_or_else(|| Refusal::InvalidRequest.into())
    };
    Ok((midnight(date)?, midnight(date.succ_opt().ok_or(Refusal::InvalidRequest)?)?))
}
/// Project native receipts into a page of actionable attempts.
pub fn tasks(session: &VaultSession, req: ListTasks) -> Result<Value> {
    if matches!(req.kind, Some(TaskKind::Browsing | TaskKind::Archive)) {
        return super::browsing_tasks::list(session, req);
    }
    let (vault, config) = admitted(session, true)?;
    let n = limit(req.limit)?;
    if vault.agent_visibility(&config.capture_policy)? == KindState::Off {
        return page(vec![], None);
    }
    if matches!(req.group, Group::Waiting) {
        let task = session
            .conversations
            .active
            .lock()
            .unwrap()
            .as_ref()
            .filter(|a| a.index_pause.paused())
            .map(|a| a.intent.task.clone());
        let mut rows = Vec::new();
        if let Some(id) = task {
            rows.push(task_row(&session.conversations, &record(&vault, &id)?)?);
        }
        check_reply(&vault, session, false)?;
        if let Some(row) = super::super::semantic::task(&vault).filter(|r| r["state"] == "paused") {
            rows.push(row);
        }
        return page(rows, None);
    }
    let after: Option<i64> = decode(req.cursor.as_deref())?;
    let (from, to) = local_day(Local::now().date_naive())?;
    let predicate = match req.group {
        Group::Running => "r.status='running'",
        Group::Attention => "r.id IN (SELECT run_id FROM ah_pending_backup_attention)",
        Group::Today => "r.finished_at>=?3 AND r.finished_at<?4",
        Group::Waiting => unreachable!(),
    };
    let ids = vault.store().read(Db::AgentHistoryEvidence, move |s| {
        let sql = format!(
            "SELECT r.id,d.task_id FROM ah_backup_runs r JOIN ah_desktop_backups d ON r.id=d.run_id
            WHERE r.id<?1 AND {predicate} ORDER BY r.id DESC LIMIT ?2"
        );
        let read = |r: &vault_store::Row| Ok((r.get::<i64>(0)?, r.get::<String>(1)?));
        if matches!(req.group, Group::Today) {
            s.query(&sql, params![after.unwrap_or(i64::MAX), n + 1, from, to], read)
        } else {
            s.query(&sql, params![after.unwrap_or(i64::MAX), n + 1], read)
        }
    })?;
    let mut rows = Vec::new();
    for (_, id) in ids.iter().take(req.limit) {
        let row = task_row(&session.conversations, &record(&vault, id)?)?;
        if !matches!(req.group, Group::Running) || row["state"] != "paused" {
            rows.push(row);
        }
    }
    let next = (ids.len() > req.limit).then(|| ids[req.limit - 1].0.to_string());
    check_reply(&vault, session, false)?;
    if matches!(req.group, Group::Running)
        && req.cursor.is_none()
        && let Some(row) = super::super::semantic::task(&vault).filter(|r| r["state"] != "paused")
    {
        rows.push(row);
    }
    page(rows, next)
}
/// Serve the rail from active work and unacknowledged terminal receipts.
pub fn task_summary(session: &VaultSession, _: super::super::Empty) -> Result<Value> {
    let config = admitted_config(&vault_core::project_paths()?)?;
    if !config.initialized {
        return Err(Refusal::NotInitialized.into());
    }
    let vault = session.vault().map_err(anyhow::Error::msg)?;
    let with_browsing = config.capture_policy.browsing != KindState::Off;
    let (mut running, mut queued, browsing_attention) =
        super::browsing_tasks::summary(&vault, with_browsing)?;
    let unavailable = vault.conversation_availability().is_err();
    if unavailable || vault.agent_visibility(&config.capture_policy)? == KindState::Off {
        if check_reply(&vault, session, true)?.browsing != config.capture_policy.browsing {
            return Err(Refusal::PolicyChanged.into());
        }
        let mut summary = json!({"runningCount":running.len(),"queuedCount":queued,"attentionCount":browsing_attention.to_string(),"running":running});
        if unavailable {
            summary["unavailableKinds"] = json!(["agent-history"]);
        }
        return Ok(summary);
    }
    let (current, _) = admitted(session, false)?;
    if !Arc::ptr_eq(vault.store(), current.store()) {
        return Err(Refusal::Replaced.into());
    }
    let count =
        vault.store().read(Db::AgentHistoryEvidence, |s| {
            s.query_one("SELECT COUNT(*) FROM ah_pending_backup_attention", (), |r| {
                Ok(r.get::<i64>(0)?)
            })
        })?;
    let id = session.conversations.active.lock().unwrap().as_ref().map(|a| a.intent.task.clone());

    if let Some(id) = id {
        let r = record(&vault, &id)?;
        if r.status == "running" {
            let row = task_row(&session.conversations, &r)?;
            if row["state"] == "paused" {
                queued += 1;
            } else {
                running.push(row);
            }
        }
    }
    if let Some(row) = super::super::semantic::task(&vault) {
        if row["state"] == "paused" {
            queued += 1;
        } else {
            running.push(row);
        }
    }
    if check_reply(&vault, session, false)?.browsing != config.capture_policy.browsing {
        return Err(Refusal::PolicyChanged.into());
    }
    Ok(
        json!({"runningCount":running.len(),"queuedCount":queued,"attentionCount":(count+browsing_attention).to_string(),"running":running}),
    )
}

/// Resolve duplicate delivery before changing a task or scheduling its retry.
pub fn control(session: &VaultSession, req: Control, events: Events) -> Result<Value> {
    if req.task_id.starts_with("browsing:") || req.task_id.starts_with("archive:") {
        if !matches!(req.action, Action::Dismiss) {
            return Err(Refusal::ActionUnavailable.into());
        }
        return super::browsing_tasks::dismiss(session, &req.task_id);
    }
    let (vault, _) = admitted(session, false)?;
    let _admission = session.conversations.enter();
    let policy = check_reply(&vault, session, false)?;
    check_id(&req.request_id)?;
    if matches!(req.action, Action::DismissNotice) != req.notice.is_some() {
        return Err(Refusal::InvalidRequest.into());
    }
    let fingerprint = hash(&req)?;
    if let Some(intent) = ledger::receipt(&vault, &req.request_id, &fingerprint)? {
        return Ok(
            json!({"task":task_row(&session.conversations,&record(&vault,&intent.task)?)?,"replacementTaskId":intent.task}),
        );
    }
    if let Some((task, _)) = ledger::delivery(&vault, &req.request_id, &fingerprint)? {
        return Ok(json!({"task":task_row(&session.conversations,&record(&vault,&task)?)?}));
    }
    let r = record(&vault, &req.task_id)?;
    let row = task_row(&session.conversations, &r)?;
    let action = serde_json::to_value(&req.action)?;
    if row["phase"] == "finishing" && matches!(req.action, Action::Cancel) {
        return Err(Refusal::Finishing.into());
    }
    if row["snapshotToken"] != req.expected_token {
        return Err(Refusal::PolicyChanged.into());
    }
    if !row["actions"].as_array().is_some_and(|a| a.contains(&action)) {
        if (matches!(req.action, Action::Cancel) && row["state"] == "cancelling")
            || (matches!(req.action, Action::Dismiss) && r.ack)
        {
            return Ok(json!({"task":row}));
        }
        return Err(Refusal::ActionUnavailable.into());
    }
    match req.action {
        Action::Cancel => {
            if cancel_active(session, Some(&req.task_id)).is_empty() {
                return Err(Refusal::Finishing.into());
            }
            ledger::remember(
                &vault,
                req.request_id,
                fingerprint,
                req.task_id.clone(),
                "cancel",
                None,
            )?;
        }
        Action::Dismiss => ledger::remember(
            &vault,
            req.request_id,
            fingerprint,
            req.task_id.clone(),
            "dismiss",
            None,
        )?,
        Action::DismissNotice => {
            let notice = req.notice.ok_or(Refusal::InvalidRequest)?;
            if !r.format_notices.contains(&serde_json::to_value(&notice)?) {
                return Err(Refusal::ActionUnavailable.into());
            }
            ledger::remember(
                &vault,
                req.request_id,
                fingerprint,
                req.task_id.clone(),
                "dismiss-notice",
                Some((notice.tool.harness().to_string(), notice.version)),
            )?;
        }
        Action::Retry => {
            let scope = r.scope.ok_or(Refusal::ScopeDenied)?;
            let allowed = selection::device_roots(&vault, &policy)?;
            if allowed.is_empty()
                || policy.read_git != scope.read_git
                || scope.roots.iter().any(|root| {
                    !policy.enabled_tools.contains(&root.tool)
                        || !allowed.contains(root)
                        || root.issue(&vault) == Some("root-changed")
                })
            {
                return Ok(json!({"task":row,"previewRequired":true}));
            }
            if session.conversations.active.lock().unwrap().is_some() {
                return Err(Refusal::Busy.into());
            }
            let task = uuid::Uuid::new_v4().to_string();
            let intent = Intent {
                task: task.clone(),
                request: req.request_id,
                hash: fingerprint,
                scope,
                trigger: "retry".into(),
                retry_of: Some(req.task_id),
            };
            launch(session, &vault, intent, events)?;
            let replacement = task_row(&session.conversations, &record(&vault, &task)?)?;
            check_reply(&vault, session, false)?;
            return Ok(json!({"task":replacement,"replacementTaskId":task}));
        }
        Action::Pause | Action::Resume => {
            let pause = matches!(req.action, Action::Pause);
            let active = session.conversations.active.lock().unwrap();
            if !active
                .as_ref()
                .is_some_and(|a| a.intent.task == req.task_id && a.index_pause.set(pause))
            {
                return Err(Refusal::ActionUnavailable.into());
            }
            drop(active);
            ledger::remember(
                &vault,
                req.request_id,
                fingerprint,
                req.task_id.clone(),
                if pause { "pause" } else { "resume" },
                None,
            )?;
        }
    }
    let row = task_row(&session.conversations, &record(&vault, &req.task_id)?)?;
    check_reply(&vault, session, false)?;
    events("pathkeep://background-task-changed", event(&row));
    Ok(json!({"task":row}))
}
pub(crate) fn cancel_active(session: &VaultSession, task: Option<&str>) -> Vec<String> {
    let active = session.conversations.active.lock().unwrap();
    if let Some(active) = active.as_ref().filter(|a| task.is_none_or(|id| id == a.intent.task)) {
        if active.progress["phase"] == "finishing" {
            return vec![];
        }
        active.token.cancel();
        return vec![active.intent.task.clone()];
    }
    vec![]
}

pub(super) fn event(row: &Value) -> Value {
    let mut event = json!({});
    for field in [
        "taskId",
        "state",
        "phase",
        "progress",
        "actions",
        "attention",
        "formatNotices",
        "snapshotToken",
    ] {
        if let Some(value) = row.get(field) {
            event[field] = value.clone();
        }
    }
    event
}
