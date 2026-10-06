//! Responsibilities: mirror search commands and channel events over local HTTP/SSE.
//! Not responsible for: query execution, coverage limits or a browser-only backend.
//! Failure modes: a slow client accumulates provisional pages; disconnect leaves a session;
//! final results disappear when provisional updates are coalesced.
//! Tests: queue replacement and command/channel serialization below.
use super::DevIpcBridgeState;
use crate::{command_error::CommandError, worker_bridge};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    convert::Infallible,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll, Waker},
};
use vault_core::search::SearchEvent;

pub(super) type Streams = Arc<Mutex<HashMap<String, Arc<Mutex<Queue>>>>>;
#[derive(Default)]
pub(super) struct Queue {
    events: VecDeque<SearchEvent>,
    waker: Option<Waker>,
    closed: bool,
}
impl Queue {
    fn push(&mut self, event: SearchEvent) -> bool {
        if self.closed {
            return false;
        }
        if matches!(event, SearchEvent::Cancelled | SearchEvent::Error(_)) {
            self.events.clear();
            self.closed = true;
        }
        if matches!(
            (&event, self.events.back()),
            (SearchEvent::Step(_), Some(SearchEvent::Step(previous))) if previous.provisional || !previous.count.exact
        ) {
            self.events.pop_back();
        }
        self.events.push_back(event);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
        true
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    session_id: String,
    cursor: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    session_id: String,
}
#[derive(Deserialize)]
struct Start {
    request: vault_core::search::SearchRequest,
}

pub(super) async fn dispatch(
    state: &DevIpcBridgeState,
    command: &str,
    payload: Value,
) -> Result<Value, CommandError> {
    let state = state.clone();
    let command = command.to_owned();
    tokio::task::spawn_blocking(move || {
        let decode = |e: serde_json::Error| CommandError::classified(e.to_string());
        match command.as_str() {
            "start_search" => {
                let start: Start = serde_json::from_value(payload).map_err(decode)?;
                let queue = Arc::new(Mutex::new(Queue::default()));
                let producer = queue.clone();
                let started = worker_bridge::start_search_impl(
                    &state.session,
                    start.request,
                    move |event| producer.lock().unwrap().push(event),
                )?;
                state.search_streams.lock().unwrap().insert(started.session_id.clone(), queue);
                Ok(json!(started))
            }
            "next_search_page" => {
                let page: Page = serde_json::from_value(payload).map_err(decode)?;
                worker_bridge::next_search_page_impl(
                    &state.session,
                    &page.session_id,
                    page.cursor,
                )?;
                Ok(Value::Null)
            }
            "cancel_search" => {
                let session: Session = serde_json::from_value(payload).map_err(decode)?;
                worker_bridge::cancel_search_impl(&state.session, &session.session_id)?;
                state.search_streams.lock().unwrap().remove(&session.session_id);
                Ok(Value::Null)
            }
            "search_indexer_state" => {
                Ok(json!(worker_bridge::search_indexer_state_impl(&state.session)?))
            }
            _ => unreachable!("search commands routed by dispatch"),
        }
    })
    .await
    .map_err(|e| CommandError::classified(e.to_string()))?
}

pub(super) async fn events(
    State(state): State<DevIpcBridgeState>,
    Path(id): Path<String>,
) -> Result<Sse<Events>, StatusCode> {
    let queue =
        state.search_streams.lock().unwrap().get(&id).cloned().ok_or(StatusCode::NOT_FOUND)?;
    let vault = state.session.current_vault().ok_or(StatusCode::CONFLICT)?;
    Ok(Sse::new(Events {
        queue,
        owner: Arc::downgrade(vault.search_sessions()),
        id,
        streams: state.search_streams,
    }))
}
pub(super) struct Events {
    queue: Arc<Mutex<Queue>>,
    owner: Weak<vault_core::search::SearchSessions>,
    id: String,
    streams: Streams,
}
impl futures_core::Stream for Events {
    type Item = Result<Event, Infallible>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut queue = self.queue.lock().unwrap();
        if let Some(event) = queue.events.pop_front() {
            return Poll::Ready(Some(Ok(Event::default()
                .json_data(event)
                .expect("serializable search event"))));
        }
        if queue.closed {
            Poll::Ready(None)
        } else {
            queue.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl Drop for Events {
    fn drop(&mut self) {
        self.queue.lock().unwrap().closed = true;
        self.streams.lock().unwrap().remove(&self.id);
        if let Some(owner) = self.owner.upgrade() {
            let id = self.id.clone();
            tokio::task::spawn_blocking(move || {
                let _ = owner.cancel(&id);
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_later_page_never_coalesces_an_unread_final_page() {
        let mut queue = Queue::default();
        let step: vault_core::search::SearchStep = serde_json::from_value(json!({
            "page":1,"items":[], "provisional":false, "progress":{"documentsProcessed":1,"segmentsCompleted":1,"segmentsTotal":1,"statisticsFieldsProcessed":0,"statisticsFieldsTotal":0},
            "count":{"value":1,"exact":true}, "visits":{"value":1,"exact":true},
            "countProgress":{"documentsProcessed":1,"segmentsCompleted":1,"segmentsTotal":1,"statisticsFieldsProcessed":0,"statisticsFieldsTotal":0},
            "cursor":"first", "indexer":{"phase":"ready","documentsProcessed":1,"documentsTotal":1,"appliedSeq":1,"targetSeq":1,"error":null},
            "snapshotSeq":1,"coverageComplete":true
        })).unwrap();
        queue.push(SearchEvent::Step(Box::new(step.clone())));
        let mut next = step;
        next.provisional = true;
        next.cursor = None;
        queue.push(SearchEvent::Step(Box::new(next.clone())));
        queue.push(SearchEvent::Step(Box::new(next)));
        assert_eq!(queue.events.len(), 2);
        assert!(
            matches!(queue.events.front(),Some(SearchEvent::Step(step)) if step.cursor.as_deref()==Some("first"))
        );
    }
    #[test]
    fn terminal_events_clear_retained_search_data() {
        let mut queue = Queue::default();
        assert!(queue.push(SearchEvent::Error("failed".into())));
        assert!(queue.closed);
        assert_eq!(queue.events.len(), 1);
        assert!(!queue.push(SearchEvent::Cancelled));
        assert_eq!(
            serde_json::to_value(queue.events.pop_front().unwrap()).unwrap(),
            json!({"type":"error","data":"failed"})
        );
    }
    #[test]
    fn requests_use_the_same_names_as_tauri() {
        let page: Page =
            serde_json::from_value(json!({"sessionId":"search","cursor":"opaque"})).unwrap();
        assert_eq!(page.session_id, "search");
        assert_eq!(page.cursor, "opaque");
        let start: Start = serde_json::from_value(
            json!({"request":{"query":"42*","pageSize":100,"profileId":"chrome:Default"}}),
        )
        .unwrap();
        assert_eq!(start.request.page_size, 100);
        assert_eq!(start.request.profile_id.as_deref(), Some("chrome:Default"));
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod integration_tests;
