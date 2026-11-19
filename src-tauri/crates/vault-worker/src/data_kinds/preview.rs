//! Request-scoped metadata walks. Private streams have bounded buffering and
//! revocable, expiring receipts. This module never opens an archive database.
use super::{Refusal, admitted_config};
use crate::VaultSession;
use agent_history::{SourceRoot, capture};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, LazyLock, Mutex, Weak},
    time::{Duration, Instant},
};
use uuid::Uuid;
use vault_core::ProjectPaths;
use vault_store::{CancelToken, Store};

mod sources;
pub use sources::{Detect, PreviewRequest, Purpose, detect};
mod summary;
use summary::enumerate;
/// The stream ID lets a dismissed picker release its producer.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelRequest {
    pub request_id_or_stream_id: String,
}
/// Correlate one private delivery sequence with its initiating view.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    pub stream_id: String,
    pub view_token: String,
}

const LIFETIME: Duration = Duration::from_secs(300);
const CAPACITY: usize = 32;

#[derive(Clone)]
struct Permit {
    inbox: Weak<Inbox>,
    paths: ProjectPaths,
    session: VaultSession,
    generation: Option<Weak<Store>>,
    initialized: bool,
    policy: String,
    cancel: CancelToken,
    deadline: Arc<Mutex<Instant>>,
}
impl Permit {
    fn expired(&self) -> bool {
        *self.deadline.lock().unwrap() <= Instant::now()
    }
    fn stop(&self) {
        self.cancel.cancel();
        if let Some(inbox) = self.inbox.upgrade() {
            inbox.close();
        }
    }

    fn check(&self) -> Result<()> {
        let current = admitted_config(&self.paths)?;
        self.cancel.check().map_err(|_| Refusal::Cancelled)?;
        if self.expired() {
            return Err(Refusal::Expired.into());
        }
        if current.capture_policy.token() != self.policy {
            return Err(Refusal::PolicyChanged.into());
        }
        let generation = self.session.current_vault().map(|v| Arc::downgrade(v.store()));
        let same_store = match (&self.generation, &generation) {
            (Some(original), Some(current)) => original.ptr_eq(current),
            (None, None) => true,
            _ => false,
        };
        if current.initialized != self.initialized || !same_store {
            return Err(Refusal::Replaced.into());
        }
        Ok(())
    }
}

// A completed ticket keeps the reviewed bindings. Step two can resolve them
// against the same live generation before granting any capture rights.
#[derive(Clone)]
struct Binding {
    root_id: String,
    source: SourceRoot,
    directory: Arc<same_file::Handle>,
}

fn directory_handle(path: &Path) -> std::io::Result<same_file::Handle> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(path)?;
        same_file::Handle::from_file(file)
    }
    #[cfg(not(unix))]
    {
        if !path.symlink_metadata()?.is_dir() {
            return Err(std::io::ErrorKind::NotADirectory.into());
        }
        same_file::Handle::from_path(path)
    }
}
struct Ticket {
    permit: Permit,
    roots: Vec<Binding>,
}
#[derive(Default)]
struct Registry {
    streams: HashMap<String, Permit>,
    tickets: HashMap<String, Ticket>,
}
static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Default::default);
impl Registry {
    fn prune(&mut self) {
        let live = |p: &Permit| !p.expired() && p.cancel.check().is_ok();
        self.streams.retain(|_, p| live(p));
        self.tickets.retain(|_, t| live(&t.permit));
    }
}

pub(crate) fn revoke(root: &Path) {
    let mut registry = REGISTRY.lock().unwrap();
    for permit in registry.streams.values().chain(registry.tickets.values().map(|t| &t.permit)) {
        if permit.paths.app_root == root {
            permit.stop()
        }
    }
    registry.prune();
}

/// Repeated cancellation of a closed picker is harmless.
pub fn cancel(request: CancelRequest) -> Result<Value> {
    let paths = vault_core::project_paths()?;
    admitted_config(&paths)?;
    let mut registry = REGISTRY.lock().unwrap();
    if let Some(permit) = registry.streams.get(&request.request_id_or_stream_id)
        && permit.paths.app_root == paths.app_root
    {
        permit.stop()
    }
    registry.prune();
    Ok(json!({"state":"cancelled"}))
}

/// A preview reference can authorize only the exact subset its user reviewed.
pub fn reviewed_sources(id: &str, selected: &[String]) -> Result<Vec<SourceRoot>> {
    let paths = vault_core::project_paths()?;
    admitted_config(&paths)?;
    let (permit, roots) = {
        let mut registry = REGISTRY.lock().unwrap();
        registry.prune();
        let ticket = registry
            .tickets
            .get(id)
            .filter(|t| t.permit.paths.app_root == paths.app_root)
            .ok_or(Refusal::Expired)?;
        (ticket.permit.clone(), ticket.roots.clone())
    };
    permit.check()?;
    let mut approved = Vec::new();
    for binding in roots {
        if !selected.contains(&binding.root_id) {
            continue;
        }
        let current = directory_handle(&binding.source.root).map_err(|_| Refusal::RootChanged)?;
        if *binding.directory != current
            || capture::root_identity(&binding.source)? != binding.root_id
        {
            return Err(Refusal::RootChanged.into());
        }
        approved.push(binding.source);
    }
    if approved.is_empty() {
        return Err(Refusal::EmptySelection.into());
    }
    if approved.len() != selected.len() {
        return Err(Refusal::ScopeDenied.into());
    }
    Ok(approved)
}

struct Inbox {
    receiver: Mutex<tokio::sync::mpsc::Receiver<Value>>,
    room: std::sync::Condvar,
}
impl Inbox {
    fn close(&self) {
        let mut receiver = self.receiver.lock().unwrap();
        receiver.close();
        while receiver.try_recv().is_ok() {}
        self.room.notify_all();
    }
    async fn recv(&self) -> Option<Value> {
        let frame = std::future::poll_fn(|cx| self.receiver.lock().unwrap().poll_recv(cx)).await;
        self.room.notify_one();
        frame
    }
    fn wait_for_room(&self, sender: &tokio::sync::mpsc::Sender<Value>, deadline: Instant) {
        let guard = self.receiver.lock().unwrap();
        drop(
            self.room
                .wait_timeout_while(
                    guard,
                    deadline.saturating_duration_since(Instant::now()),
                    |_| sender.capacity() == 0 && !sender.is_closed(),
                )
                .unwrap(),
        );
    }
}

/// Private sequence shared by Tauri Channel and the single-consumer dev SSE endpoint.
pub struct Frames {
    pub started: Started,
    permit: Permit,
    receiver: Arc<Inbox>,
    completion: Option<tokio::sync::oneshot::Receiver<Result<()>>>,
    ended: bool,
    next: u64,
}
impl Frames {
    pub fn expired(&self) -> bool {
        self.permit.expired()
    }
    /// Recheck after dequeue as the window may have locked while its client was stalled.
    pub async fn next_frame(&mut self) -> Option<Value> {
        if self.ended {
            return None;
        }
        let frame = match self.receiver.recv().await {
            Some(frame) => frame,
            None => {
                let failure = self.completion.take()?.await.ok().and_then(Result::err);
                terminal_error(&self.started.stream_id, self.next, failure.as_ref())
            }
        };
        let permit = self.permit.clone();
        let admission = tokio::task::spawn_blocking(move || permit.check()).await;
        let frame = match admission {
            Ok(Ok(())) => {
                *self.permit.deadline.lock().unwrap() = Instant::now() + LIFETIME;
                frame
            }
            denied => terminal_error(
                &self.started.stream_id,
                self.next,
                denied.ok().and_then(|r| r.err()).as_ref(),
            ),
        };
        self.next += 1;
        self.ended = matches!(frame["type"].as_str(), Some("finished" | "cancelled" | "error"));
        Some(frame)
    }
}
impl Drop for Frames {
    fn drop(&mut self) {
        if !self.ended {
            self.permit.stop()
        }
        REGISTRY.lock().unwrap().streams.remove(&self.started.stream_id);
    }
}

fn terminal_error(id: &str, seq: u64, reason: Option<&anyhow::Error>) -> Value {
    let code = reason
        .and_then(|e| e.downcast_ref::<Refusal>())
        .map(|e| e.code())
        .or_else(|| {
            reason?
                .downcast_ref::<vault_core::admission::Refusal>()
                .map(|error| error.reason.code())
        })
        .unwrap_or("root-unreadable");
    let frame_type = if code == "cancelled" { "cancelled" } else { "error" };
    json!({
        "streamId": id,
        "sequence": seq.to_string(),
        "type": frame_type,
        "payload": {"code": code}
    })
}

/// Backpressure applies only to this producer. Closing the receiver wakes a blocked send.
pub fn start(session: &VaultSession, request: PreviewRequest) -> Result<Frames> {
    let paths = vault_core::project_paths()?;
    let _gate = vault_core::ControlGate::acquire(&paths);
    let config = admitted_config(&paths)?;
    let roots = sources::resolve(request, &paths.app_root)?;
    let (sender, receiver) = tokio::sync::mpsc::channel(4);
    let receiver = Arc::new(Inbox { receiver: Mutex::new(receiver), room: Default::default() });
    let permit = Permit {
        inbox: Arc::downgrade(&receiver),
        paths,
        session: session.clone(),
        initialized: config.initialized,
        generation: session.current_vault().map(|v| Arc::downgrade(v.store())),
        policy: config.capture_policy.token(),
        cancel: CancelToken::new(),
        deadline: Arc::new(Mutex::new(Instant::now() + LIFETIME)),
    };
    let id = Uuid::new_v4().to_string();
    {
        let mut registry = REGISTRY.lock().unwrap();
        registry.prune();
        if registry.streams.len() >= CAPACITY {
            return Err(Refusal::Busy.into());
        }
        registry.streams.insert(id.clone(), permit.clone());
    }
    let (completed, completion) = tokio::sync::oneshot::channel();
    let frames = Frames {
        completion: Some(completion),
        started: Started { stream_id: id.clone(), view_token: Uuid::new_v4().to_string() },
        receiver: receiver.clone(),
        permit: permit.clone(),
        ended: false,
        next: 0,
    };
    std::thread::Builder::new().name("agent-source-preview".into()).spawn(move || {
        let mut seq = 0;
        let result = enumerate(&permit, roots, config.archive_mode, |ty, payload| {
            let mut frame =
                json!({"streamId":id,"sequence":seq.to_string(),"type":ty,"payload":payload});
            loop {
                permit.check()?;
                match sender.try_send(frame) {
                    Ok(()) => break,
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        return Err(Refusal::Cancelled.into());
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Full(unsent)) => {
                        frame = unsent;
                        let until = *permit.deadline.lock().unwrap();
                        receiver.wait_for_room(&sender, until);
                    }
                }
            }
            seq += 1;
            Ok(())
        });
        let _ = completed.send(result);
    })?;
    Ok(frames)
}
