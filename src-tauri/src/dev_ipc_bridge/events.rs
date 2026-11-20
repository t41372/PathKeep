//! Responsibilities: carry the desktop events (`pathkeep://…`) to a browser on the dev bridge as
//! server-sent events on `/events`, so E2E and `dev:demo` see what the app's window sees: Ask's
//! streamed answer, backup, import, upgrade and download progress, the quit question.
//! Not responsible for: which events exist (the commands emit them, under Tauri through the app
//! handle) or replaying old ones: a browser gets what is emitted after it connects, so it listens
//! before it starts the work it listens to.
//! Failure modes: an event the window gets but the browser never does (a bridge command that drops
//! its sink, an app event not forwarded); a browser that went away keeping a queue alive; a
//! browser that stops reading growing the queue without end.
//! Tests: below, and the E2E specs that wait for these events (Ask's answer).

use super::{DevIpcAppHandle, DevIpcBridgeState};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, KeepAliveStream, Sse},
};
use serde::Serialize;
use std::{
    collections::VecDeque,
    convert::Infallible,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
};
use tauri::Listener;
use vault_worker::data_kinds::delivery::Permit;

/// The events the app emits to its window itself (backups the app or the menu bar run, the
/// updater, the menu bar's search, the question asked when someone quits while work runs),
/// forwarded to the browser as they are.
const APP_EVENTS: &[&str] = &[
    vault_worker::data_kinds::POLICY_EVENT,
    "pathkeep://agent-settings-changed",
    "pathkeep://agent-viewer-preferences-changed",
    "pathkeep://agent-projects-changed",
    "pathkeep://agent-conversations-changed",
    "pathkeep://kind-backup-progress",
    "pathkeep://kind-backup-finished",
    "pathkeep://background-task-changed",
    "pathkeep://agent-catalog-changed",
    "pathkeep://backup-progress",
    "pathkeep://backup-finished",
    vault_core::AI_INDEX_JOB_ENDED_EVENT,
    vault_core::MODEL_DOWNLOAD_PROGRESS_EVENT,
    "pathkeep://updater-progress",
    "pathkeep://open-command-palette",
    crate::desktop_integration::QUIT_REQUEST_EVENT,
];

/// Events held for a browser that is not reading; past that the oldest go.
const QUEUE_LIMIT: usize = 100_000;

#[derive(Default)]
struct Queue {
    events: VecDeque<(&'static str, String, Option<Permit>)>,
    waker: Option<Waker>,
}

/// Every browser connected to `/events`.
#[derive(Clone, Default)]
pub(super) struct Hub {
    browsers: Arc<Mutex<Vec<Weak<Mutex<Queue>>>>>,
}

impl Hub {
    /// Sends `payload` as event `name` to every connected browser.
    pub(super) fn emit(&self, name: &'static str, payload: &impl Serialize) {
        if let Ok(json) = serde_json::to_string(payload) {
            self.emit_json(name, json);
        }
    }

    fn emit_json(&self, name: &'static str, json: String) {
        let permit = matches!(
            name,
            "pathkeep://agent-settings-changed"
                | "pathkeep://agent-viewer-preferences-changed"
                | "pathkeep://agent-projects-changed"
                | "pathkeep://agent-conversations-changed"
                | "pathkeep://kind-backup-progress"
                | "pathkeep://kind-backup-finished"
                | "pathkeep://background-task-changed"
                | "pathkeep://agent-catalog-changed"
        )
        .then(Permit::current);
        let mut browsers = self.browsers.lock().expect("bridge event browsers");
        browsers.retain(|browser| {
            let Some(queue) = browser.upgrade() else { return false };
            let mut queue = queue.lock().expect("bridge event queue");
            if queue.events.len() == QUEUE_LIMIT {
                queue.events.pop_front();
            }
            queue.events.push_back((name, json.clone(), permit.clone()));
            if let Some(waker) = queue.waker.take() {
                waker.wake();
            }
            true
        });
    }

    fn connect(&self) -> Browser {
        let queue = Arc::new(Mutex::new(Queue::default()));
        self.browsers.lock().expect("bridge event browsers").push(Arc::downgrade(&queue));
        Browser { queue }
    }

    /// Forwards what the app emits to its window to the browser too.
    pub(super) fn forward_app_events(&self, app: &DevIpcAppHandle) {
        for &name in APP_EVENTS {
            let hub = self.clone();
            app.listen_any(name, move |event| hub.emit_json(name, event.payload().to_owned()));
        }
    }
}

/// One connected browser; dropping it (the browser went away) stops its queue.
pub(super) struct Browser {
    queue: Arc<Mutex<Queue>>,
}

impl futures_core::Stream for Browser {
    type Item = Result<Event, Infallible>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            let next = {
                let mut queue = self.queue.lock().expect("bridge event queue");
                let Some(event) = queue.events.pop_front() else {
                    queue.waker = Some(cx.waker().clone());
                    return Poll::Pending;
                };
                event
            };
            let (name, json, permit) = next;
            if permit.as_ref().is_some_and(|permit| !permit.valid()) {
                continue;
            }
            if matches!(
                name,
                vault_worker::data_kinds::POLICY_EVENT
                    | "pathkeep://agent-settings-changed"
                    | "pathkeep://agent-viewer-preferences-changed"
            ) && vault_worker::data_kinds::settings().is_err()
            {
                continue;
            }
            if matches!(
                name,
                "pathkeep://agent-projects-changed"
                    | "pathkeep://agent-conversations-changed"
                    | "pathkeep://kind-backup-progress"
                    | "pathkeep://kind-backup-finished"
                    | "pathkeep://background-task-changed"
                    | "pathkeep://agent-catalog-changed"
            ) && !vault_worker::data_kinds::settings()
                .is_ok_and(|s| s.policy.agent_state_if_present(true) != vault_core::KindState::Off)
            {
                continue;
            }
            return Poll::Ready(Some(Ok(Event::default().event(name).data(json))));
        }
    }
}

/// Keep-alive comments stop an idle stream (no backup, no chat) from timing out.
pub(super) async fn events(
    State(state): State<DevIpcBridgeState>,
) -> Sse<KeepAliveStream<Browser>> {
    Sse::new(state.events.connect()).keep_alive(KeepAlive::default())
}

#[cfg(test)]
impl Hub {
    /// Observe the same queue an SSE subscriber drains, including event name and serialized payload.
    pub(super) fn observe(&self) -> impl Fn() -> Vec<(&'static str, serde_json::Value)> {
        let browser = self.connect();
        move || {
            browser
                .queue
                .lock()
                .unwrap()
                .events
                .drain(..)
                .map(|(name, data, _)| (name, serde_json::from_str(&data).unwrap()))
                .collect()
        }
    }
}
