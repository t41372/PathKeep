//! Streaming chat run driver + cooperative-cancel registry.
//!
//! ## Responsibilities
//! - own a process-global registry of in-flight streaming chat runs, keyed by run id
//! - drive one `LlmProvider::chat_stream` to completion, forwarding each chunk to a caller
//!   supplied sink as a serde-ready [`AiChatStreamChunk`]
//! - honor cooperative cancellation (`request_cancel`) by stopping between chunks
//! - always terminate the sink with exactly one `Done` or `Error` marker
//!
//! ## Not responsible for
//! - resolving provider config/secrets (the worker passes a built provider)
//! - the Tauri event emission itself (the desktop command supplies the sink closure)
//! - persistence of the conversation (W-AI-3)
//!
//! ## Why this module exists
//! W-AI-1 needs a place that converts the boundary `LlmStreamChunk`s into the IPC
//! `AiChatStreamChunk` shape and manages run lifetime/cancellation, without leaking rig or the
//! Tauri `AppHandle` into `vault-core`.

use super::traits::{LlmChatRequest, LlmProvider, LlmStreamChunk};
use crate::models::AiChatStreamChunk;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Process-global table of cancellation flags for live streaming chat runs.
///
/// Keyed by run id so `ai_chat_cancel` can find and flip the flag for a run started by an
/// earlier `ai_chat_send`. Entries are removed when the run finishes.
fn registry() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Registers a new run and returns its cancellation token.
///
/// The token starts un-cancelled; the driver checks it between chunks and `request_cancel`
/// flips it. Invariant: run ids are process-unique (the worker mints them via `next_run_id`),
/// so re-registration of a live id never happens in practice; deregistration removes the entry
/// by exact id when the run ends.
pub fn register_run(run_id: &str) -> Arc<AtomicBool> {
    let token = Arc::new(AtomicBool::new(false));
    registry()
        .lock()
        .expect("ai chat run registry poisoned")
        .insert(run_id.to_string(), token.clone());
    token
}

/// Removes a run from the registry once it has finished.
pub fn deregister_run(run_id: &str) {
    registry().lock().expect("ai chat run registry poisoned").remove(run_id);
}

/// Requests cooperative cancellation of a live run; returns whether one was found.
pub fn request_cancel(run_id: &str) -> bool {
    match registry().lock().expect("ai chat run registry poisoned").get(run_id) {
        Some(token) => {
            token.store(true, Ordering::SeqCst);
            true
        }
        None => false,
    }
}

/// Requests cooperative cancellation of every live run, e.g. before "Delete all data" removes the
/// conversation store under them. Returns how many runs were asked to stop.
pub fn request_cancel_all() -> usize {
    let runs = registry().lock().expect("ai chat run registry poisoned");
    for token in runs.values() {
        token.store(true, Ordering::SeqCst);
    }
    runs.len()
}

/// Drives one streaming chat run to completion, forwarding chunks to `sink`.
///
/// Lifecycle guarantees:
/// - opens the stream via `provider.chat_stream`; an open failure emits exactly one `Error`.
/// - forwards each `Token`/`Reasoning`/`ToolCall` as the matching `AiChatStreamChunk`.
/// - a mid-stream error emits one `Error` and stops (no `Done` afterward).
/// - cooperative cancel (the `cancel` token) stops between chunks and emits `Done`.
/// - normal completion emits exactly one `Done`.
///
/// `sink` is called for every emitted chunk including the terminal marker, so the desktop layer
/// can forward each as a Tauri event. This is an `async fn`; the worker runs it on its runtime.
pub async fn drive_chat_stream<P, S>(
    provider: &P,
    request: LlmChatRequest,
    cancel: Arc<AtomicBool>,
    mut sink: S,
) where
    P: LlmProvider,
    S: FnMut(AiChatStreamChunk),
{
    if cancel.load(Ordering::SeqCst) {
        sink(AiChatStreamChunk::Done);
        return;
    }
    let mut stream = match provider.chat_stream(request).await {
        Ok(stream) => stream,
        Err(error) => {
            sink(AiChatStreamChunk::Error { message: error.to_string() });
            return;
        }
    };

    use std::future::poll_fn;
    loop {
        if cancel.load(Ordering::SeqCst) {
            sink(AiChatStreamChunk::Done);
            return;
        }
        let next = poll_fn(|cx| stream.as_mut().poll_next(cx)).await;
        match next {
            None => {
                sink(AiChatStreamChunk::Done);
                return;
            }
            Some(Ok(chunk)) => sink(to_ipc_chunk(chunk)),
            Some(Err(error)) => {
                sink(AiChatStreamChunk::Error { message: error.to_string() });
                return;
            }
        }
    }
}

/// Maps a boundary stream chunk onto the serde-ready IPC chunk.
///
/// Shared by the plain W-AI-1 chat path and the W-AI-7 agent harness so the wire encoding lives in
/// one place. The plain path only ever produces Token/Reasoning/ToolCall; `Usage` is forwarded
/// (the FE ignores it on the non-agent path) and the harness additionally emits `ToolResult`
/// directly (it is not a boundary chunk, so it has no arm here).
pub(super) fn to_ipc_chunk(chunk: LlmStreamChunk) -> AiChatStreamChunk {
    match chunk {
        LlmStreamChunk::Token(text) => AiChatStreamChunk::Token { text },
        LlmStreamChunk::Reasoning(text) => AiChatStreamChunk::Reasoning { text },
        LlmStreamChunk::ToolCall { call_id, name, arguments } => AiChatStreamChunk::ToolCall {
            name,
            arguments,
            // An empty provider call id is reported as `None` so the plain path keeps the W-AI-1
            // (no `callId`) wire shape; the harness supplies a real id when present.
            call_id: (!call_id.is_empty()).then_some(call_id),
        },
        LlmStreamChunk::Usage(usage) => AiChatStreamChunk::Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
        },
    }
}
