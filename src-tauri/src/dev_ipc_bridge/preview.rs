//! Delivers each source preview to its requesting browser through a private,
//! single-consumer SSE endpoint. The worker owns buffering and lock checks.
use super::DevIpcBridgeState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
};
use vault_worker::data_kinds::preview::{Frames, Started};

pub(super) type Streams = Arc<Mutex<HashMap<String, Frames>>>;
pub(super) fn register(state: &DevIpcBridgeState, frames: Frames) -> Started {
    let started = frames.started.clone();
    let mut pending = state.previews.lock().unwrap();
    pending.retain(|_, stream| !stream.expired());
    pending.insert(started.stream_id.clone(), frames);
    started
}
pub(super) fn subscribe(state: &DevIpcBridgeState, id: &str) -> Result<Frames, StatusCode> {
    state.previews.lock().unwrap().remove(id).ok_or(StatusCode::NOT_FOUND)
}
pub(super) async fn events(
    State(state): State<DevIpcBridgeState>,
    Path(id): Path<String>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let frames = subscribe(&state, &id)?;
    Ok(Sse::new(futures_util::stream::unfold(frames, |mut frames| async move {
        let frame = frames.next_frame().await?;
        Some((Ok(Event::default().data(frame.to_string())), frames))
    })))
}
