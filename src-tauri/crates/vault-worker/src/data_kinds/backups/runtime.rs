//! Short owner transitions serialize publication, admission and cancellation.
//! The capture lease stays with the background thread until its native report is final.
use super::{Events, Refusal, ledger::Intent};
use agent_history::{
    Archive,
    capture::{BackupOptions, ReservedBackup},
};
use anyhow::Result;
use serde_json::{Value, json};
use std::sync::{Arc, Condvar, Mutex};
use vault_core::Vault;
use vault_store::CancelToken;

#[derive(Default)]
pub(crate) struct Runtime {
    admitting: Mutex<bool>,
    pub(super) recovery_events: std::sync::OnceLock<Events>,
    ready: Condvar,
    pub(super) active: Mutex<Option<Active>>,
}
pub(crate) struct Admission<'a>(&'a Runtime);
impl Drop for Admission<'_> {
    fn drop(&mut self) {
        *self.0.admitting.lock().unwrap() = false;
        self.0.ready.notify_one();
    }
}
impl Runtime {
    pub(crate) fn enter(&self) -> Admission<'_> {
        let mut held = self.ready.wait_while(self.admitting.lock().unwrap(), |held| *held).unwrap();
        *held = true;
        Admission(self)
    }
}
pub(super) struct Active {
    pub intent: Intent,
    pub token: CancelToken,
    pub progress: Value,
}
pub(super) struct Producer {
    lease: ReservedBackup,
    archive: Arc<Archive>,
    token: CancelToken,
}
impl Producer {
    pub fn reserve(
        vault: &Vault,
        owner: &Arc<Runtime>,
        intent: &Intent,
        events: &Events,
    ) -> Result<Self> {
        let archive = vault.agent_history().map_err(|_| Refusal::Unavailable)?;
        let token = CancelToken::new();
        let runtime = owner.clone();
        let task = intent.task.clone();
        let trigger = intent.trigger.clone();
        let notify = events.clone();
        let paths = vault.paths().clone();
        let counters = Mutex::new(super::progress::Progress::default());
        let options = BackupOptions {
            approved_directories: Some(
                intent.scope.roots.iter().map(|r| (r.path.clone(), r.directory.clone())).collect(),
            ),
            read_git: intent.scope.read_git,
            cancel: token.clone(),
            progress: Some(Arc::new(move |p| {
                #[cfg(test)]
                if p.phase == "raw_committed" {
                    super::crash_tests::boundary("raw");
                }
                let Some(mut progress) = counters.lock().unwrap().advance(&p) else {
                    return;
                };
                let phase = progress["phase"].clone();
                let actions = if phase == "finishing" { vec![] } else { vec!["cancel"] };
                let snapshot = super::hash(&(&task, "running", &phase, &actions, false))
                    .expect("task identity is JSON");
                if let Some(active) = runtime.active.lock().unwrap().as_mut() {
                    active.progress = progress.clone();
                }
                if super::can_deliver(&paths) {
                    notify(
                        "pathkeep://background-task-changed",
                        json!({"taskId":task,"state":"running","phase":phase,"progress":progress,
                        "actions":actions,"attention":false,"snapshotToken":snapshot}),
                    );
                    progress["batchId"] = json!(task);
                    progress["taskId"] = json!(task);
                    progress["kind"] = json!("agent-history");
                    progress["trigger"] = json!(trigger);
                    notify("pathkeep://kind-backup-progress", progress);
                }
            })),
            ..Default::default()
        };
        let lease = archive.reserve_backup(options)?;
        Ok(Self { lease, archive, token })
    }

    pub fn launch(
        self,
        vault: Vault,
        runtime: Arc<Runtime>,
        intent: Intent,
        run: i64,
        events: Events,
    ) -> Result<()> {
        let active = Active {
            intent: intent.clone(),
            token: self.token.clone(),
            progress: json!({"phase":"discovery","unit":"files","done":"0","total":null}),
        };
        *runtime.active.lock().unwrap() = Some(active);
        let owner = runtime.clone();
        let archive = self.archive.clone();
        let spawn = std::thread::Builder::new()
            .name("conversation-backup".into())
            .spawn(move || self.run(vault, owner, intent, run, events));
        if let Err(err) = spawn {
            runtime.active.lock().unwrap().take();
            archive.end_backup_run(run, "failed", &json!({"error_code":"worker-unavailable"}))?;
            return Err(err.into());
        }
        Ok(())
    }
    fn run(self, vault: Vault, owner: Arc<Runtime>, intent: Intent, run: i64, events: Events) {
        if let Err(error) = self.capture(&vault, &intent, run) {
            // The library finalizes attempts it enters. Failure before it enters
            // still has an admitted row. The conditional update preserves either.
            let status = if self.token.check().is_err() { "interrupted" } else { "failed" };
            let reason = error
                .downcast_ref::<agent_history::Error>()
                .map_or("destination-unavailable", agent_history::Error::code);
            let _ = self.archive.end_backup_run(run, status, &json!({"error_code":reason}));
        }
        #[cfg(test)]
        super::crash_tests::boundary("finished");
        let terminal = super::runs::record(&vault, &intent.task);
        let summary = terminal.as_ref().ok().and_then(|record| record.row(&vault).ok());
        drop(self);
        {
            let mut active = owner.active.lock().unwrap();
            if active.as_ref().is_some_and(|active| active.intent.task == intent.task) {
                active.take();
            }
        }
        let (Ok(row), Some(detail)) = (terminal, summary) else {
            return;
        };
        if !vault.store().is_open() || !super::can_deliver(vault.paths()) {
            return;
        }
        if let Ok(task) = super::tasks::task_row(&owner, &row) {
            events("pathkeep://background-task-changed", super::tasks::event(&task));
            events(
                "pathkeep://agent-catalog-changed",
                json!({
                    "viewToken":task["snapshotToken"], "domains":["backup","sources"]
                }),
            );
        }
        let reasons = row
            .report
            .get("captureOutcome")
            .map_or(&row.report["issues"], |v| &v["warnings"])
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|issue| issue["code"].as_str())
            .chain(row.report["error_code"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        events(
            "pathkeep://kind-backup-finished",
            json!({
                "batchId":intent.task,"taskId":intent.task,"runRef":detail["runRef"],
                "kind":"agent-history","trigger":intent.trigger,"result":detail["result"],
                "summary":detail["summary"],"reasonCodes":reasons,
                "noteCount":detail["noteCount"],"newNoteCount":detail["newNoteCount"],"formatNotices":row.format_notices
            }),
        );
    }

    fn capture(&self, vault: &Vault, intent: &Intent, run: i64) -> Result<()> {
        let mut sources = Vec::new();
        let mut issues = Vec::new();
        for root in &intent.scope.roots {
            if let Some(reason) = root.issue(vault) {
                self.archive.root_outcome(run, &root.id, &root.source(), reason)?;
                issues.push(agent_history::Diagnostic {
                    code: reason.into(),
                    path: Some(root.path.to_string_lossy().into_owned()),
                    ordinal: None,
                    detail: None,
                });
            } else {
                sources.push(root.source());
            }
        }
        self.lease.execute(run, &sources, issues)?;
        Ok(())
    }
}
