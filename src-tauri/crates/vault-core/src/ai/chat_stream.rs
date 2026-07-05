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

/// Returns the number of live runs (test/diagnostic helper).
#[cfg(test)]
pub fn live_run_count() -> usize {
    registry().lock().expect("ai chat run registry poisoned").len()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::traits::{
        LlmCapabilities, LlmChatResponse, LlmChunkStream, LlmMessage, LlmRole,
    };
    use anyhow::Result;
    use std::pin::Pin;

    /// A provider whose stream is configurable per-test: a script of chunks, optional trailing
    /// error, and an optional eager open failure.
    struct ScriptedProvider {
        open_error: bool,
        chunks: Vec<LlmStreamChunk>,
        trailing_error: bool,
    }

    impl LlmProvider for ScriptedProvider {
        async fn chat(&self, _req: LlmChatRequest) -> Result<LlmChatResponse> {
            Ok(LlmChatResponse::default())
        }

        async fn chat_stream(&self, _req: LlmChatRequest) -> Result<LlmChunkStream> {
            if self.open_error {
                anyhow::bail!("open failure");
            }
            let mut items: Vec<Result<LlmStreamChunk>> =
                self.chunks.iter().cloned().map(Ok).collect();
            if self.trailing_error {
                items.push(Err(anyhow::anyhow!("mid-stream failure")));
            }
            struct VecStream(std::vec::IntoIter<Result<LlmStreamChunk>>);
            impl futures_core::Stream for VecStream {
                type Item = Result<LlmStreamChunk>;
                fn poll_next(
                    mut self: Pin<&mut Self>,
                    _cx: &mut std::task::Context<'_>,
                ) -> std::task::Poll<Option<Self::Item>> {
                    std::task::Poll::Ready(self.0.next())
                }
            }
            Ok(Box::pin(VecStream(items.into_iter())))
        }

        fn capabilities(&self) -> LlmCapabilities {
            LlmCapabilities::default()
        }
    }

    fn request() -> LlmChatRequest {
        LlmChatRequest::new(vec![LlmMessage::new(LlmRole::User, "hi")], None, None)
    }

    #[tokio::test]
    async fn forwards_chunks_and_emits_done() {
        let provider = ScriptedProvider {
            open_error: false,
            chunks: vec![
                LlmStreamChunk::Reasoning("thinking".to_string()),
                LlmStreamChunk::Token("hello".to_string()),
                LlmStreamChunk::ToolCall {
                    call_id: "call-9".to_string(),
                    name: "search".to_string(),
                    arguments: "{}".to_string(),
                },
                LlmStreamChunk::Usage(crate::ai::traits::LlmUsage {
                    prompt_tokens: 3,
                    completion_tokens: 4,
                }),
            ],
            trailing_error: false,
        };
        // Exercise the fixture's `capabilities` so it is not reported uncovered under the
        // `verify-rust-coverage full` gate; the driver itself never calls it.
        assert_eq!(provider.capabilities(), LlmCapabilities::default());
        let mut emitted = Vec::new();
        drive_chat_stream(&provider, request(), Arc::new(AtomicBool::new(false)), |chunk| {
            emitted.push(chunk)
        })
        .await;
        assert_eq!(
            emitted,
            vec![
                AiChatStreamChunk::Reasoning { text: "thinking".to_string() },
                AiChatStreamChunk::Token { text: "hello".to_string() },
                AiChatStreamChunk::ToolCall {
                    name: "search".to_string(),
                    arguments: "{}".to_string(),
                    call_id: Some("call-9".to_string()),
                },
                AiChatStreamChunk::Usage { prompt_tokens: 3, completion_tokens: 4 },
                AiChatStreamChunk::Done,
            ]
        );
    }

    #[tokio::test]
    async fn to_ipc_chunk_reports_empty_call_id_as_none() {
        // The plain W-AI-1 path keeps the no-`callId` wire shape when the provider omits a call id.
        let chunk = to_ipc_chunk(LlmStreamChunk::ToolCall {
            call_id: String::new(),
            name: "search".to_string(),
            arguments: "{}".to_string(),
        });
        assert_eq!(
            chunk,
            AiChatStreamChunk::ToolCall {
                name: "search".to_string(),
                arguments: "{}".to_string(),
                call_id: None,
            }
        );
    }

    #[tokio::test]
    async fn emits_error_on_open_failure() {
        let provider =
            ScriptedProvider { open_error: true, chunks: Vec::new(), trailing_error: false };
        let mut emitted = Vec::new();
        drive_chat_stream(&provider, request(), Arc::new(AtomicBool::new(false)), |chunk| {
            emitted.push(chunk)
        })
        .await;
        assert_eq!(emitted.len(), 1);
        assert!(
            matches!(&emitted[0], AiChatStreamChunk::Error { message } if message.contains("open failure"))
        );
    }

    #[tokio::test]
    async fn emits_error_on_mid_stream_failure_without_done() {
        let provider = ScriptedProvider {
            open_error: false,
            chunks: vec![LlmStreamChunk::Token("a".to_string())],
            trailing_error: true,
        };
        let mut emitted = Vec::new();
        drive_chat_stream(&provider, request(), Arc::new(AtomicBool::new(false)), |chunk| {
            emitted.push(chunk)
        })
        .await;
        assert_eq!(emitted.len(), 2);
        assert_eq!(emitted[0], AiChatStreamChunk::Token { text: "a".to_string() });
        assert!(matches!(&emitted[1], AiChatStreamChunk::Error { .. }));
    }

    #[tokio::test]
    async fn pre_cancelled_run_emits_done_immediately() {
        let provider = ScriptedProvider {
            open_error: false,
            chunks: vec![LlmStreamChunk::Token("never".to_string())],
            trailing_error: false,
        };
        let mut emitted = Vec::new();
        drive_chat_stream(&provider, request(), Arc::new(AtomicBool::new(true)), |chunk| {
            emitted.push(chunk)
        })
        .await;
        assert_eq!(emitted, vec![AiChatStreamChunk::Done]);
    }

    #[tokio::test]
    async fn cancel_between_chunks_stops_with_done() {
        let provider = ScriptedProvider {
            open_error: false,
            chunks: vec![
                LlmStreamChunk::Token("first".to_string()),
                LlmStreamChunk::Token("second".to_string()),
            ],
            trailing_error: false,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_for_sink = cancel.clone();
        let mut emitted = Vec::new();
        drive_chat_stream(&provider, request(), cancel.clone(), |chunk| {
            // Flip cancel after the first forwarded chunk so the loop stops before the second.
            if matches!(chunk, AiChatStreamChunk::Token { .. }) {
                cancel_for_sink.store(true, Ordering::SeqCst);
            }
            emitted.push(chunk);
        })
        .await;
        assert_eq!(
            emitted,
            vec![AiChatStreamChunk::Token { text: "first".to_string() }, AiChatStreamChunk::Done,]
        );
    }

    #[test]
    fn registry_register_cancel_and_deregister() {
        let run_id = "run-registry-test";
        let token = register_run(run_id);
        assert!(!token.load(Ordering::SeqCst));
        assert!(request_cancel(run_id));
        assert!(token.load(Ordering::SeqCst));
        deregister_run(run_id);
        assert!(!request_cancel(run_id));
    }

    #[test]
    fn cancel_unknown_run_returns_false() {
        assert!(!request_cancel("never-registered"));
    }

    #[test]
    fn live_run_count_tracks_registration() {
        let before = live_run_count();
        let _token = register_run("count-test");
        assert_eq!(live_run_count(), before + 1);
        deregister_run("count-test");
        assert_eq!(live_run_count(), before);
    }
}
