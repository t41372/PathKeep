//! Owner-scoped life of a save-dialog export. The staged writer claims publication only after
//! the last bytes and the final access check. An accepted cancellation keeps the destination intact.
use super::{Refusal, admitted_config, backups, delivery};
use crate::VaultSession;
use agent_history::chats::{ConversationExport, ExportLocale};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;
use vault_core::LanguagePreference;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub export_id: Option<Uuid>,
    #[serde(flatten)]
    pub conversation: ConversationExport,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    pub export_id: Uuid,
}

const WRITING: u8 = 0;
const CANCELLED: u8 = 1;
const PUBLISHING: u8 = 2;
struct Control {
    phase: AtomicU8,
    read: agent_history::read_model::JobControl,
}
impl Control {
    fn cancel(&self) -> bool {
        match self.phase.compare_exchange(WRITING, CANCELLED, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) | Err(CANCELLED) => {
                self.read.cancel.cancel();
                true
            }
            Err(_) => false,
        }
    }
}
type Key = (PathBuf, Uuid);
static ACTIVE: LazyLock<Mutex<HashMap<Key, Arc<Control>>>> = LazyLock::new(Default::default);

pub struct Job {
    key: Key,
    state: Arc<Control>,
    events: backups::Events,
}
impl Drop for Job {
    fn drop(&mut self) {
        ACTIVE.lock().unwrap().remove(&self.key);
    }
}
impl Job {
    pub fn begin(
        session: &VaultSession,
        id: &str,
        export_id: Option<Uuid>,
        events: backups::Events,
    ) -> Result<Self> {
        let (vault, _) = backups::admitted(session, false)?;
        vault.agent_history()?.reader_header(id)?;
        let key = (vault.paths().app_root.clone(), export_id.unwrap_or_else(Uuid::new_v4));
        let state = Arc::new(Control { phase: AtomicU8::new(WRITING), read: Default::default() });
        {
            let mut jobs = ACTIVE.lock().unwrap();
            if jobs.contains_key(&key) {
                return Err(Refusal::Busy.into());
            }
            jobs.insert(key.clone(), state.clone());
        }
        let job = Self { key, state, events };
        job.progress(0, None);
        Ok(job)
    }
    pub fn check(&self) -> Result<()> {
        if self.state.phase.load(Ordering::Acquire) == CANCELLED {
            return Err(Refusal::Cancelled.into());
        }
        Ok(())
    }
    fn progress(&self, bytes: u64, total: Option<u64>) {
        let mut event = json!({"exportId":self.key.1,"bytesWritten":bytes.to_string()});
        if let Some(total) = total {
            event["totalBytes"] = json!(total.to_string());
        }
        (self.events)("pathkeep://agent-export-progress", event);
    }
    pub fn write(
        &self,
        session: &VaultSession,
        mut q: ConversationExport,
        target: &Path,
    ) -> Result<Value> {
        self.check()?;
        let result = delivery::read(|| {
            let (vault, config) = backups::admitted(session, false)?;
            if self.key.0 != vault.paths().app_root {
                return Err(Refusal::Replaced.into());
            }
            q.locale.get_or_insert(match config.preferred_language {
                LanguagePreference::ZhCn => ExportLocale::Simplified,
                LanguagePreference::ZhTw => ExportLocale::Traditional,
                LanguagePreference::System | LanguagePreference::En => ExportLocale::English,
            });
            let folder = std::fs::canonicalize(target.parent().ok_or(Refusal::Destination)?)?;
            let archive = vault.agent_history()?;
            if folder.starts_with(std::fs::canonicalize(archive.root())?)
                || backups::selection::device_roots(&vault, &config.capture_policy)?
                    .iter()
                    .any(|r| folder.starts_with(&r.path))
            {
                return Err(Refusal::Destination.into());
            }
            let mut bytes = 0;
            vault_core::durable_io::atomic_durable_write_with(target, |out| {
                let mut writer =
                    ProgressWriter { job: self, out: BufWriter::new(out), bytes: 0, last: None };
                archive.export_conversation_with_job(
                    &q,
                    &mut writer,
                    config.agent_viewer.reader_collapse_tool_calls,
                    config.agent_viewer.reader_show_thinking,
                    &self.state.read,
                )?;
                writer.flush()?;
                bytes = writer.bytes;
                backups::check_reply(&vault, session, false)?;
                self.state
                    .phase
                    .compare_exchange(WRITING, PUBLISHING, Ordering::AcqRel, Ordering::Acquire)
                    .map_err(|_| Refusal::Cancelled)?;
                Ok(())
            })?;
            self.progress(bytes, Some(bytes));
            Ok(json!({"exportId":self.key.1,"path":target,"bytes":bytes.to_string()}))
        });
        if self.state.phase.load(Ordering::Acquire) == CANCELLED {
            return Err(Refusal::Cancelled.into());
        }
        result
    }
}
struct ProgressWriter<'a> {
    job: &'a Job,
    out: BufWriter<&'a mut dyn Write>,
    bytes: u64,
    last: Option<Instant>,
}
impl Write for ProgressWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.job.state.phase.load(Ordering::Acquire) == CANCELLED {
            return Err(io::Error::other("cancelled"));
        }
        let n = self.out.write(&buf[..buf.len().min(64 * 1024)])?;
        self.bytes += n as u64;
        if n > 0 && self.last.is_none_or(|t| t.elapsed() >= Duration::from_millis(100)) {
            self.job.progress(self.bytes, None);
            self.last = Some(Instant::now());
        }
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}
pub fn cancel(q: Cancel) -> Result<Value> {
    let paths = vault_core::project_paths()?;
    admitted_config(&paths)?;
    let accepted =
        ACTIVE.lock().unwrap().get(&(paths.app_root, q.export_id)).is_some_and(|s| s.cancel());
    Ok(json!({"exportId":q.export_id,"cancelled":accepted}))
}

pub(super) fn revoke(root: &Path) {
    for ((path, _), job) in ACTIVE.lock().unwrap().iter() {
        if path == root {
            job.cancel();
        }
    }
}
