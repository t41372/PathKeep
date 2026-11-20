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
    semantic: bool,
    inbox: Weak<Inbox>,
    search_view: Option<(Weak<agent_history::Archive>, i64)>,
    paths: ProjectPaths,
    session: VaultSession,
    generation: Option<Weak<Store>>,
    owner_permit: super::delivery::Permit,
    initialized: bool,
    policy: String,
    enable_choices: bool,
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
        if self.semantic {
            vault_core::ensure_ai_capability_enabled(
                &current,
                vault_core::AiCapability::SemanticIndex,
            )?;
            if !current.agent_viewer.build_semantic_index {
                return Err(vault_core::AiRefusal::AgentAccessDisabled.into());
            }
        }
        self.cancel.check().map_err(|_| Refusal::Cancelled)?;
        if self.expired() {
            return Err(Refusal::Expired.into());
        }
        if current.capture_policy.token() != self.policy {
            if self.semantic
                && let Some(v) = self.session.current_vault()
                && v.agent_visibility(&current.capture_policy)? == vault_core::KindState::Off
            {
                return Err(vault_core::AiRefusal::KindOff.into());
            }
            return Err(Refusal::PolicyChanged.into());
        }
        let generation = self.session.current_vault().map(|v| Arc::downgrade(v.store()));
        if !self.owner_permit.valid() {
            return Err(Refusal::Replaced.into());
        }
        let same_store = match (&self.generation, &generation) {
            (Some(original), Some(current)) => original.ptr_eq(current),
            (None, None) => true,
            (None, Some(_)) => self.initialized && self.owner_permit.valid(),
            _ => false,
        };
        if current.initialized != self.initialized || !same_store {
            return Err(Refusal::Replaced.into());
        }
        if let Some((owner, marks)) = &self.search_view {
            let owner = owner.upgrade().ok_or(Refusal::Replaced)?;
            if owner.search_marks_generation()? != *marks {
                return Err(agent_history::Error::StaleCursor.into());
            }
        }
        Ok(())
    }
}

// A completed ticket keeps the reviewed bindings. Step two can resolve them
// against the same live generation before granting any capture rights.
#[derive(Clone)]
pub(super) struct Binding {
    pub(super) root_id: String,
    pub(super) source: SourceRoot,
    pub(super) canonical: std::path::PathBuf,
    pub(super) directory: Arc<same_file::Handle>,
}

/// Equality distinguishes a reviewed replacement from an unchanged grant without
/// exposing platform file identifiers or allowing the caller to manufacture a grant.
pub(super) fn directory_token(root_id: &str, identity: &str) -> String {
    vault_core::utils::sha256_hex(format!("{root_id}/{identity}").as_bytes())
}

use capture::directory_handle;
struct Ticket {
    permit: Permit,
    roots: Vec<Binding>,
}
#[derive(Default)]
struct Registry {
    streams: HashMap<String, Permit>,
    tickets: HashMap<String, Ticket>,
    reaping: bool,
}
static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Default::default);
impl Registry {
    fn prune(&mut self) {
        let live = |p: &Permit| !p.expired() && p.cancel.check().is_ok();
        self.streams.retain(|_, p| live(p));
        self.tickets.retain(|_, t| live(&t.permit));
    }
}

static TICKETS_CHANGED: std::sync::Condvar = std::sync::Condvar::new();

fn schedule_expiry(registry: &mut Registry) {
    if registry.reaping {
        TICKETS_CHANGED.notify_one();
        return;
    }
    registry.reaping = true;
    std::thread::spawn(|| {
        let mut registry = REGISTRY.lock().unwrap();
        loop {
            registry.prune();
            let Some(next) =
                registry.tickets.values().map(|t| *t.permit.deadline.lock().unwrap()).min()
            else {
                registry.reaping = false;
                return;
            };
            registry = TICKETS_CHANGED
                .wait_timeout(registry, next.saturating_duration_since(Instant::now()))
                .unwrap()
                .0;
        }
    });
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

pub(super) struct ChoiceEdit<'a> {
    pub ticket: &'a str,
    pub tools: &'a std::collections::BTreeMap<vault_core::AgentTool, bool>,
}

/// Rotate only the enable dialog's named receipt across an edit to its checklist or Git choice.
pub(super) fn policy_changed(root: &Path, before: &str, after: &str, keep: Option<ChoiceEdit<'_>>) {
    let mut registry = REGISTRY.lock().unwrap();
    registry.prune();
    let keep = keep.filter(|edit| {
        registry.tickets.get(edit.ticket).is_some_and(|t| {
            t.permit.paths.app_root == root
                && t.permit.enable_choices
                && edit
                    .tools
                    .keys()
                    .all(|tool| t.roots.iter().any(|b| b.source.harness == tool.harness()))
                && t.permit.policy == before
                && t.permit.owner_permit.valid()
        })
    });
    let retained = keep.and_then(|edit| registry.tickets.remove_entry(edit.ticket));
    for p in registry.streams.values().chain(registry.tickets.values().map(|t| &t.permit)) {
        if p.paths.app_root == root {
            p.stop()
        }
    }
    registry.prune();
    if let Some((id, mut ticket)) = retained {
        ticket.permit.policy = after.into();
        ticket.permit.cancel = CancelToken::new();
        registry.tickets.insert(id, ticket);
    }
}

/// Repeated cancellation of a closed picker is harmless.
pub fn cancel(request: CancelRequest) -> Result<Value> {
    let paths = vault_core::project_paths()?;
    admitted_config(&paths)?;
    super::chats::cancel_window(&paths.app_root, &request.request_id_or_stream_id);
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
    reviewed_bindings(id, selected, &[])?
        .into_iter()
        .map(|binding| {
            let current =
                directory_handle(&binding.source.root).map_err(|_| Refusal::RootChanged)?;
            if *binding.directory != current
                || capture::root_identity(&binding.source)? != binding.root_id
            {
                return Err(Refusal::RootChanged.into());
            }
            Ok(binding.source)
        })
        .collect()
}

pub(super) fn reviewed_bindings(
    id: &str,
    selected: &[String],
    retained: &[String],
) -> Result<Vec<Binding>> {
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
        approved.push(binding);
    }
    if approved.is_empty()
        && (retained.is_empty() || selected.iter().any(|id| !retained.contains(id)))
    {
        return Err(Refusal::EmptySelection.into());
    }
    if selected.iter().collect::<std::collections::HashSet<_>>().len() != selected.len()
        || selected
            .iter()
            .any(|id| !retained.contains(id) && !approved.iter().any(|b| &b.root_id == id))
    {
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
        .or_else(|| reason?.downcast_ref::<vault_core::AiRefusal>().map(|e| e.code()))
        .or_else(|| {
            reason?.downcast_ref::<vault_store::Error>().and_then(|e| {
                matches!(e, vault_store::Error::Cancelled { .. }).then_some("cancelled")
            })
        })
        .or_else(|| reason?.downcast_ref::<agent_history::Error>().map(|e| e.code()))
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
    let config = admitted_config(&paths)?;
    let enable_choices = matches!(request.purpose, Purpose::Enable);
    let roots = sources::resolve(request, &paths.app_root, session, &config)?;
    let mode = config.archive_mode.clone();
    private_stream(
        session,
        config,
        enable_choices,
        None,
        "agent-source-preview",
        move |permit, send| enumerate(permit, roots, mode, send),
    )
}

/// Search and preview use the same bounded channel, receipts and lifetime checks.
pub(super) fn search_stream(
    session: &VaultSession,
    archive: Arc<agent_history::Archive>,
    scope: Option<String>,
    q: agent_history::search::ScreenQuery,
) -> Result<Frames> {
    q.validate()?;
    conversation_stream(session, archive, "agent-conversation-search", move |a, c, send| {
        a.stream_conversations(scope.as_deref(), &q, c, send)
    })
}

pub(super) fn conversation_stream(
    session: &VaultSession,
    archive: Arc<agent_history::Archive>,
    name: &str,
    work: impl FnOnce(
        &agent_history::Archive,
        &CancelToken,
        &mut dyn FnMut(&str, Value) -> agent_history::Result<()>,
    ) -> agent_history::Result<()>
    + Send
    + 'static,
) -> Result<Frames> {
    let config = admitted_config(&vault_core::project_paths()?)?;
    let view = (Arc::downgrade(&archive), archive.search_marks_generation()?);
    private_stream(session, config, false, Some(view), name, move |permit, send| {
        let mut failure = None;
        let result = work(&archive, &permit.cancel, &mut |ty, p| {
            send(ty, p).map_err(|err| {
                failure = Some(err);
                agent_history::Error::Cancelled
            })
        });
        if let Some(err) = failure {
            return Err(err);
        }
        Ok(result?)
    })
}

pub(super) fn semantic_stream(
    session: &VaultSession,
    config: vault_core::AppConfig,
    archive: Arc<agent_history::Archive>,
    work: impl FnOnce(&agent_history::Archive, &CancelToken, &mut Emit<'_>) -> Result<()>
    + Send
    + 'static,
) -> Result<Frames> {
    let view = (Arc::downgrade(&archive), archive.search_marks_generation()?);
    private_stream(
        session,
        config,
        false,
        Some(view),
        "agent-semantic-search",
        move |permit, send| work(&archive, &permit.cancel, send),
    )
}
type Emit<'a> = dyn FnMut(&str, Value) -> Result<()> + 'a;
fn private_stream(
    session: &VaultSession,
    config: vault_core::AppConfig,
    enable_choices: bool,
    search_view: Option<(Weak<agent_history::Archive>, i64)>,
    name: &str,
    produce: impl FnOnce(&Permit, &mut Emit<'_>) -> Result<()> + Send + 'static,
) -> Result<Frames> {
    let paths = vault_core::project_paths()?;
    let _gate = vault_core::ControlGate::acquire(&paths);
    if admitted_config(&paths)?.capture_policy != config.capture_policy {
        return Err(Refusal::PolicyChanged.into());
    }
    let (sender, receiver) = tokio::sync::mpsc::channel(4);
    let receiver = Arc::new(Inbox { receiver: Mutex::new(receiver), room: Default::default() });
    let permit = Permit {
        semantic: name == "agent-semantic-search",
        inbox: Arc::downgrade(&receiver),
        search_view,
        owner_permit: super::delivery::Permit::at(&paths.app_root),
        paths,
        session: session.clone(),
        initialized: config.initialized,

        generation: session.current_vault().map(|v| Arc::downgrade(v.store())),
        policy: config.capture_policy.token(),
        enable_choices,
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
    std::thread::Builder::new().name(name.into()).spawn(move || {
        let mut seq = 0;
        let result = produce(&permit, &mut |ty, payload| {
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

#[cfg(test)]
#[test]
fn idle_ticket_releases_pinned_directory_at_expiry() {
    let temp = tempfile::tempdir().unwrap();
    let paths = vault_core::project_paths_with_root(temp.path());
    let permit = Permit {
        semantic: false,
        inbox: Weak::new(),
        search_view: None,
        session: VaultSession::default(),
        generation: None,
        owner_permit: super::delivery::Permit::at(temp.path()),
        initialized: false,
        enable_choices: false,
        policy: String::new(),
        cancel: CancelToken::new(),
        deadline: Arc::new(Mutex::new(Instant::now() + LIFETIME)),
        paths,
    };
    let dir = Arc::new(directory_handle(temp.path()).unwrap());
    let observed = Arc::downgrade(&dir);
    let root = Binding {
        root_id: "test".into(),
        canonical: temp.path().into(),
        source: SourceRoot { root: temp.path().into(), harness: agent_history::Harness::Pi },
        directory: dir,
    };
    summary::retain_ticket(
        Uuid::new_v4().to_string(),
        &permit,
        vec![root],
        Duration::from_millis(40),
    );
    let timeout = Instant::now() + Duration::from_secs(2);
    while observed.strong_count() > 0 && Instant::now() < timeout {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(observed.strong_count(), 0);
}
